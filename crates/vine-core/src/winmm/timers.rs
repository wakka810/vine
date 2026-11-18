//! WinMM callbacks execute on ordinary guest worker threads with owned stacks/TEBs.
use crate::api::{ApiCallFrame, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol};
use crate::memory::{
    GuestAddress, GuestRange, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, Runtime, RuntimeAddressRange, RuntimeCreateThreadRequest,
    RuntimeHandle, RuntimeObjectKind, RuntimeStateError, RuntimeThreadId,
};

#[derive(Debug)]
pub(super) struct Timer {
    id: u32,
    thread: RuntimeThreadId,
    thread_handle: RuntimeHandle,
    owner: MemoryOwnerId,
    allocation: ProcessAddressAllocationId,
    base: GuestAddress,
}

fn endpoint(runtime: &Runtime, name: &str) -> Option<GuestAddress> {
    runtime
        .runtime_system_export_endpoints()
        .find_map(|endpoint| {
            matches!(&endpoint.export, crate::RuntimeExport::Name { module, name: found }
            if module.eq_ignore_ascii_case("kernel32.dll") && found == name)
            .then_some(endpoint.address)
        })
}

fn release(runtime: &mut Runtime, timer: &Timer) -> Result<(), RuntimeStateError> {
    runtime
        .address_space_allocator
        .validate_releases(&[timer.allocation])?;
    let mut transaction = runtime.memory.begin_transaction();
    transaction
        .release_region(timer.owner, timer.base)
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .destroy_owner(timer.owner)
        .map_err(RuntimeStateError::from_memory)?;
    transaction.commit_transaction();
    runtime
        .address_space_allocator
        .remove_validated_allocations(vec![timer.allocation]);
    Ok(())
}

pub(crate) fn retire_timer_thread(
    runtime: &mut Runtime,
    thread: RuntimeThreadId,
) -> Result<(), RuntimeStateError> {
    let id = runtime
        .winmm
        .timers
        .values()
        .find(|timer| timer.thread == thread)
        .map(|timer| timer.id);
    if let Some(id) = id {
        let timer = runtime
            .winmm
            .timers
            .remove(&id)
            .expect("owned timer thread");
        release(runtime, &timer)?;
        runtime.close_handle(timer.thread_handle)?;
    }
    Ok(())
}

fn code(delay: u32, callback: u32, user: u32, id: u32, flags: u32, sleep: u32) -> Vec<u8> {
    // stdcall thread entry: EBX points to a Rust-owned cancellation word. The
    // checked Sleep endpoint supplies host pacing and the shared scheduler.
    let mut b = vec![0x53, 0x8b, 0x5c, 0x24, 0x08]; // push ebx; mov ebx,[esp+8]
    let start = b.len();
    b.push(0x68);
    b.extend(delay.to_le_bytes());
    b.push(0xb8);
    b.extend(sleep.to_le_bytes());
    b.extend([0xff, 0xd0]);
    b.extend([0x83, 0x3b, 0x00, 0x75, 0]); // cmp [ebx],0; jne done
    let branch = b.len() - 1;
    if flags & 0x10 != 0 {
        b.push(0x68);
        b.extend(user.to_le_bytes()); // SetEvent(event)
    } else {
        for value in [0, 0, user, 0, id] {
            b.push(0x68);
            b.extend(value.to_le_bytes());
        }
    }
    b.push(0xb8);
    b.extend(callback.to_le_bytes());
    b.extend([0xff, 0xd0]);
    if flags & 1 != 0 {
        b.push(0xeb);
        b.push((start as i32 - (b.len() + 1) as i32) as i8 as u8);
    }
    let done = b.len();
    b[branch] = (done - branch - 1) as u8;
    b.extend([0x5b, 0x31, 0xc0, 0xc2, 0x04, 0x00]);
    b
}

pub(super) fn dispatch(frame: &ApiCallFrame, runtime: &mut Runtime) -> ApiHandlerResult {
    let result = (|| -> Result<ApiImmediate, ApiFault> {
        let err = |e| ApiFault::from_runtime(e, &frame.endpoint);
        if matches!(&frame.endpoint.symbol, ApiSymbol::Name { name } if name == "timeKillEvent") {
            let id = frame
                .argument_u32(0)
                .ok_or_else(|| ApiFault::new("missing timer ID"))?;
            let Some(timer) = runtime.winmm.timers.get(&id) else {
                return Ok(ApiImmediate::new(Some(97)));
            };
            let flag = GuestAddress(timer.base.0 + 4096);
            let current = runtime
                .memory
                .read(flag, 4)
                .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
            if current != [0; 4] {
                return Ok(ApiImmediate::new(Some(97)));
            }
            let mut transaction = runtime.memory.begin_transaction();
            transaction
                .write(flag, &1u32.to_le_bytes())
                .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
            transaction.commit_transaction();
            return Ok(ApiImmediate::new(Some(0)));
        }
        let delay = frame.argument_u32(0).unwrap();
        let resolution = frame.argument_u32(1).unwrap();
        let mut callback = frame.argument_u32(2).unwrap();
        let mut user = frame.argument_u32(3).unwrap();
        let flags = frame.argument_u32(4).unwrap();
        if !(1..=1_000_000).contains(&delay) || resolution > 1_000_000 || callback == 0 {
            return Ok(ApiImmediate::new(Some(0)));
        }
        if flags & !0x11 != 0 {
            return Err(ApiFault::new(
                "WinMM timer flags require unsupported pulse or synchronous cancellation semantics",
            ));
        }
        let sleep = endpoint(runtime, "Sleep")
            .ok_or_else(|| ApiFault::new("WinMM worker requires the Runtime Sleep endpoint"))?;
        if flags & 0x10 != 0 {
            if !runtime
                .is_live_object_handle(RuntimeHandle(callback), RuntimeObjectKind::Event)
                .map_err(err)?
            {
                return Ok(ApiImmediate::new(Some(0)));
            }
            user = callback;
            callback = endpoint(runtime, "SetEvent")
                .ok_or_else(|| {
                    ApiFault::new("WinMM event timer requires the Runtime SetEvent endpoint")
                })?
                .0;
        }
        runtime
            .memory
            .validate_host_execute(GuestAddress(callback), 1)
            .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        if runtime.winmm.timers.len() >= 64 {
            return Ok(ApiImmediate::new(Some(0)));
        }
        let id = runtime
            .winmm
            .next_timer
            .checked_add(1)
            .ok_or_else(|| ApiFault::new("multimedia timer IDs exhausted"))?;
        let stack = runtime
            .thread_stack(runtime.current_thread_id())
            .map_err(err)?;
        let (reserve, commit) = (
            stack.reserved_range.byte_length(),
            stack.initial_commit_bytes,
        );
        let bytes = code(delay, callback, user, id, flags, sleep.0);
        let request = ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::MultimediaTimer,
            preferred_address: None,
            search_range: RuntimeAddressRange::new(GuestAddress(0x10000), 0x1fff0000)
                .map_err(err)?,
            reserve_size: 8192,
            alignment: 4096,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::RuntimeControl,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: true,
        };
        let memory = &mut runtime.memory;
        let installed = runtime
            .address_space_allocator
            .try_reserve_batch_with(vec![(None, request)], |allocations| {
                let allocation = &allocations[0];
                let mut transaction = memory.begin_transaction();
                let owner = transaction
                    .create_owner(MemoryOwnerKind::Runtime)
                    .map_err(RuntimeStateError::from_memory)?;
                let range = transaction
                    .reserve_fixed_for_address_allocation(
                        allocation.id.0,
                        owner,
                        MemoryRegionKind::Runtime,
                        allocation.range.base,
                        8192,
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .commit(
                        owner,
                        range,
                        PageProtection {
                            read: true,
                            write: true,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .write_initialization(owner, allocation.range.base, &bytes)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .protect(
                        owner,
                        GuestRange::new(allocation.range.base, 4096)
                            .map_err(RuntimeStateError::from_memory)?,
                        PageProtection {
                            read: true,
                            write: false,
                            execute: true,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .seal_owner(owner)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction.commit_transaction();
                Ok(Some((owner, allocation.id, allocation.range.base)))
            })
            .map_err(err)?;
        let Some((owner, allocation, base)) = installed else {
            return Ok(ApiImmediate::new(Some(0)));
        };
        let thread = runtime.create_multimedia_timer_thread(
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: reserve,
                stack_commit_bytes: commit,
                entry_point: base,
                parameter: base.0 + 4096,
                thread_id_output: None,
                initial_suspend_count: 0,
            },
            owner,
            allocation,
        );
        let (thread, thread_handle) = match thread {
            Ok(Some(result)) => result,
            other => {
                let timer = Timer {
                    id,
                    thread: RuntimeThreadId(0),
                    thread_handle: RuntimeHandle(0),
                    owner,
                    allocation,
                    base,
                };
                release(runtime, &timer).map_err(err)?;
                return match other {
                    Ok(None) => Ok(ApiImmediate::new(Some(0))),
                    Err(e) => Err(err(e)),
                    _ => unreachable!(),
                };
            }
        };
        runtime.winmm.timers.insert(
            id,
            Timer {
                id,
                thread,
                thread_handle,
                owner,
                allocation,
                base,
            },
        );
        runtime.winmm.next_timer = id;
        let mut immediate = ApiImmediate::new(Some(id));
        immediate.created_thread = Some(thread);
        Ok(immediate)
    })();
    match result {
        Ok(result) => ApiHandlerResult::Immediate(result),
        Err(e) => ApiHandlerResult::Fault(e),
    }
}
