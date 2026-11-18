//! Read-only DOS and memory MMIO streams. Windows stream and buffer state stay in Rust.
use std::collections::BTreeMap;

use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiFault, ApiHandlerResult, ApiImmediate,
    ApiSymbol, DispatchOutcome, HostCompletion, HostRequest,
};
use crate::memory::{GuestAddress, MemoryAccess, MemoryRegionInfo};
use crate::runtime::{
    RUNTIME_SOURCE_READ_MAX_BYTES, RUNTIME_SOURCE_READ_SERVICE, Runtime, RuntimeFileBacking,
    RuntimeHandle, RuntimeHeapInitialization, RuntimeSourceReadHostResponse,
};

const INFO_BYTES: usize = 72;
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL: usize = 128 * 1024 * 1024;
const MAX_STREAMS: usize = 64;
const BUFFER_SIZE: u32 = 8192;
const INVALPARAM: u32 = 11;
const INVALHANDLE: u32 = 5;
const OUTOFMEMORY: u32 = 257;
const CANNOTOPEN: u32 = 259;
const CHUNKNOTFOUND: u32 = 265;

#[derive(Debug)]
enum Backing {
    File {
        bytes: Vec<u8>,
        handle: RuntimeHandle,
    },
    Memory {
        address: GuestAddress,
        length: u32,
    },
}

#[derive(Debug)]
struct Stream {
    backing: Backing,
    position: u32,
    buffer: GuestAddress,
    capacity: u32,
    owned_heap: Option<u32>,
    buffer_offset: u32,
    buffer_length: u32,
    flags: u32,
}

impl Stream {
    fn length(&self) -> u32 {
        match &self.backing {
            Backing::File { bytes, .. } => bytes.len() as u32,
            Backing::Memory { length, .. } => *length,
        }
    }

    fn info(&self, handle: u32) -> Vec<u8> {
        let next = self
            .position
            .saturating_sub(self.buffer_offset)
            .min(self.buffer_length);
        [
            self.flags,
            match self.backing {
                Backing::Memory { .. } => 0x204d454d,
                _ => 0,
            },
            0,
            0,
            0,
            self.capacity,
            self.buffer.0,
            self.buffer.0 + next,
            self.buffer.0 + self.buffer_length,
            self.buffer.0 + self.capacity,
            self.buffer_offset,
            self.buffer_offset + self.buffer_length,
            0,
            0,
            0,
            0,
            0,
            handle,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
    }
}

#[derive(Debug)]
struct PendingOpen {
    stream: Stream,
    info: GuestAddress,
    info_regions: Vec<MemoryRegionInfo>,
    buffer_regions: Vec<MemoryRegionInfo>,
    source_id: u32,
    object_id: u32,
    length: u32,
    requested: u32,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeWinMm {
    pub(super) timers: BTreeMap<u32, super::timers::Timer>,
    pub(super) next_timer: u32,
    streams: BTreeMap<u32, Stream>,
    pending: BTreeMap<u64, PendingOpen>,
    next_handle: u32,
}

fn fault(frame: &ApiCallFrame, error: impl std::fmt::Display) -> ApiFault {
    ApiFault::from_runtime(
        crate::runtime::RuntimeStateError::new(error.to_string()),
        &frame.endpoint,
    )
}

fn word(bytes: &[u8], index: usize) -> u32 {
    u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
}

fn regions(runtime: &Runtime, address: GuestAddress, length: usize) -> Vec<MemoryRegionInfo> {
    runtime
        .memory
        .regions()
        .filter(|region| {
            u64::from(address.0) < region.range.end()
                && u64::from(region.range.start().0) < u64::from(address.0) + length as u64
        })
        .collect()
}

fn read(
    runtime: &Runtime,
    frame: &ApiCallFrame,
    address: u32,
    length: usize,
) -> Result<Vec<u8>, ApiFault> {
    runtime
        .memory
        .read(GuestAddress(address), length)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))
}

fn check_write(
    runtime: &Runtime,
    frame: &ApiCallFrame,
    address: u32,
    length: usize,
) -> Result<(), ApiFault> {
    runtime
        .memory
        .validate_access(GuestAddress(address), length, MemoryAccess::Write)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))
}

fn bytes_at(
    runtime: &Runtime,
    frame: &ApiCallFrame,
    stream: &Stream,
    offset: u32,
    count: u32,
) -> Result<Vec<u8>, ApiFault> {
    let end = offset
        .checked_add(count)
        .ok_or_else(|| fault(frame, "MMIO range overflow"))?;
    if end > stream.length() {
        return Err(fault(frame, "MMIO read exceeds its stream"));
    }
    match &stream.backing {
        Backing::File { bytes, .. } => Ok(bytes[offset as usize..end as usize].to_vec()),
        Backing::Memory { address, .. } => read(runtime, frame, address.0 + offset, count as usize),
    }
}

fn next_handle(runtime: &Runtime) -> Result<u32, ApiFault> {
    let id = runtime
        .winmm
        .next_handle
        .checked_add(1)
        .filter(|id| *id <= 0xffff)
        .ok_or_else(|| ApiFault::new("MMIO handle space exhausted"))?;
    Ok(0x6d00_0000 | id)
}

fn fill_buffer(
    runtime: &mut Runtime,
    frame: &ApiCallFrame,
    stream: &mut Stream,
) -> Result<(), ApiFault> {
    if let Backing::Memory { .. } = stream.backing {
        stream.buffer_offset = 0;
        stream.buffer_length = stream.length();
        return Ok(());
    }
    let length = stream
        .capacity
        .min(stream.length().saturating_sub(stream.position));
    let bytes = bytes_at(
        runtime,
        frame,
        stream,
        stream.position.min(stream.length()),
        length,
    )?;
    let mut transaction = runtime.memory.begin_transaction();
    transaction
        .write(stream.buffer, &bytes)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    transaction.commit_transaction();
    stream.buffer_offset = stream.position;
    stream.buffer_length = length;
    Ok(())
}

fn accept_info(stream: &Stream, bytes: &[u8], handle: u32) -> Result<u32, u32> {
    let next = word(bytes, 7);
    if word(bytes, 5) != stream.capacity || word(bytes, 6) != stream.buffer.0
        || word(bytes, 8) != stream.buffer.0 + stream.buffer_length
        || word(bytes, 10) != stream.buffer_offset || word(bytes, 17) != handle
        || word(bytes, 0) & 0x1000_0000 != 0 // MMIO_DIRTY cannot write read-only streams.
        || next < stream.buffer.0 || next > stream.buffer.0 + stream.buffer_length
    {
        return Err(INVALPARAM);
    }
    Ok(stream.buffer_offset + next - stream.buffer.0)
}

fn open_error(info: GuestAddress, code: u32) -> ApiImmediate {
    let result = ApiImmediate::new(Some(0));
    if info.0 == 0 {
        result
    } else {
        result.with_guest_write(GuestAddress(info.0 + 12), code.to_le_bytes().to_vec())
    }
}

fn publish(
    runtime: &mut Runtime,
    frame: &ApiCallFrame,
    mut stream: Stream,
    info: GuestAddress,
) -> Result<ApiImmediate, ApiFault> {
    let preparation = (|| {
        let handle = next_handle(runtime)?;
        if info.0 != 0 {
            check_write(runtime, frame, info.0, INFO_BYTES)?;
        }
        Ok(handle)
    })();
    let handle = match preparation {
        Ok(handle) => handle,
        Err(error) => {
            if let Backing::File { handle, .. } = stream.backing {
                runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
            }
            return Err(error);
        }
    };
    if stream.buffer.0 == 0 {
        let allocation = (|| {
            let heap = runtime.process_heap().map_err(|e| fault(frame, e))?;
            let allocation = runtime
                .allocate_private_heap_block(
                    heap,
                    stream.capacity,
                    RuntimeHeapInitialization::Uninitialized,
                )
                .map_err(|e| fault(frame, e))?;
            Ok(allocation.map(|allocation| (heap, allocation)))
        })();
        let (heap, allocation) = match allocation {
            Ok(Some(allocation)) => allocation,
            result => {
                if let Backing::File { handle, .. } = stream.backing {
                    runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
                }
                return match result {
                    Ok(None) => Ok(open_error(info, OUTOFMEMORY)),
                    Err(error) => Err(error),
                    Ok(Some(_)) => unreachable!(),
                };
            }
        };
        stream.buffer = allocation.address;
        stream.owned_heap = Some(heap);
    }
    if let Err(error) = fill_buffer(runtime, frame, &mut stream) {
        if let Some(heap) = stream.owned_heap {
            runtime
                .free_private_heap_block(heap, stream.buffer)
                .map_err(|e| fault(frame, e))?;
        }
        if let Backing::File { handle, .. } = stream.backing {
            runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
        }
        return Err(error);
    }
    let result = if info.0 == 0 {
        ApiImmediate::new(Some(handle))
    } else {
        ApiImmediate::new(Some(handle)).with_guest_write(info, stream.info(handle))
    };
    runtime.winmm.next_handle = handle & 0xffff;
    runtime.winmm.streams.insert(handle, stream);
    Ok(result)
}

fn open(frame: &ApiCallFrame, runtime: &mut Runtime) -> Result<ApiHandlerResult, ApiFault> {
    let [name, info, flags] = frame.arguments.as_slice() else {
        return Err(fault(frame, "MMIO open arguments"));
    };
    let (name, info, flags) = (*name, GuestAddress(*info), *flags);
    let input = if info.0 == 0 {
        vec![0; INFO_BYTES]
    } else {
        check_write(runtime, frame, info.0, INFO_BYTES)?;
        read(runtime, frame, info.0, INFO_BYTES)?
    };
    // Read-only MMIO, with default/deny-write/deny-none sharing and optional buffer allocation.
    if flags & !(0x10000 | 0x70) != 0
        || !matches!(flags & 0x70, 0 | 0x20 | 0x40)
        || word(&input, 2) != 0
        || !matches!(word(&input, 1), 0 | 0x20534f44 | 0x204d454d)
    {
        return Ok(ApiHandlerResult::Immediate(open_error(info, INVALPARAM)));
    }
    if runtime.winmm.streams.len() + runtime.winmm.pending.len() >= MAX_STREAMS {
        return Ok(ApiHandlerResult::Immediate(open_error(info, OUTOFMEMORY)));
    }
    next_handle(runtime)?;
    let buffer = GuestAddress(word(&input, 6));
    let capacity = if buffer.0 == 0 {
        BUFFER_SIZE
    } else {
        word(&input, 5)
    };
    if capacity == 0 || capacity > 1024 * 1024 {
        return Ok(ApiHandlerResult::Immediate(open_error(info, INVALPARAM)));
    }
    if buffer.0 != 0 {
        check_write(runtime, frame, buffer.0, capacity as usize)?;
    }
    if word(&input, 1) == 0x204d454d {
        if name != 0 || buffer.0 == 0 {
            return Ok(ApiHandlerResult::Immediate(open_error(info, INVALPARAM)));
        }
        let stream = Stream {
            backing: Backing::Memory {
                address: buffer,
                length: capacity,
            },
            position: 0,
            buffer,
            capacity,
            owned_heap: None,
            buffer_offset: 0,
            buffer_length: capacity,
            flags,
        };
        return publish(runtime, frame, stream, info).map(ApiHandlerResult::Immediate);
    }
    if name == 0 {
        return Ok(ApiHandlerResult::Immediate(open_error(info, INVALPARAM)));
    }
    let path = crate::win32::read_nul_terminated_source_path(
        &frame.endpoint,
        &runtime.memory,
        GuestAddress(name),
        260,
    )?;
    let handle = match runtime
        .open_file_a(
            &path,
            0x8000_0000,
            if flags & 0x70 == 0x40 { 3 } else { 1 },
            3,
            0x80,
        )
        .map_err(|e| fault(frame, e))?
    {
        Ok(Some(handle)) => handle,
        _ => return Ok(ApiHandlerResult::Immediate(open_error(info, CANNOTOPEN))),
    };
    let length = runtime
        .source_file_byte_length(handle)
        .map_err(|e| fault(frame, e))?;
    let backing = runtime
        .source_file_for_handle(handle)
        .map_err(|e| fault(frame, e))?
        .backing;
    let RuntimeFileBacking::ImmutableSource {
        source_id,
        object_id,
    } = backing
    else {
        runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
        return Err(fault(
            frame,
            "MMIO DOS stream requires immutable source backing",
        ));
    };
    let used: usize = runtime
        .winmm
        .streams
        .values()
        .map(|s| match &s.backing {
            Backing::File { bytes, .. } => bytes.len(),
            _ => 0,
        })
        .sum::<usize>()
        + runtime
            .winmm
            .pending
            .values()
            .map(|p| p.length as usize)
            .sum::<usize>();
    if length > MAX_FILE || length as usize > MAX_TOTAL.saturating_sub(used) {
        runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
        return Ok(ApiHandlerResult::Immediate(open_error(info, OUTOFMEMORY)));
    }
    let stream = Stream {
        backing: Backing::File {
            bytes: Vec::new(),
            handle,
        },
        position: 0,
        buffer,
        capacity,
        owned_heap: None,
        buffer_offset: 0,
        buffer_length: 0,
        flags,
    };
    if length == 0 {
        return publish(runtime, frame, stream, info).map(ApiHandlerResult::Immediate);
    }
    let requested = (length as u32).min(RUNTIME_SOURCE_READ_MAX_BYTES);
    let request = match runtime.begin_host_request(RUNTIME_SOURCE_READ_SERVICE) {
        Ok(request) => request,
        Err(error) => {
            runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
            return Err(fault(frame, error));
        }
    };
    let payload = [source_id, object_id, 0, 0, requested]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let info_regions = regions(runtime, info, if info.0 == 0 { 0 } else { INFO_BYTES });
    let buffer_regions = regions(
        runtime,
        buffer,
        if buffer.0 == 0 { 0 } else { capacity as usize },
    );
    runtime.winmm.pending.insert(
        request.request_id,
        PendingOpen {
            stream,
            info,
            info_regions,
            buffer_regions,
            source_id,
            object_id,
            length: length as u32,
            requested,
        },
    );
    Ok(ApiHandlerResult::Deferred(HostRequest {
        request_id: request.request_id,
        service: request.service,
        payload,
    }))
}

pub fn complete_winmm_source_read(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    response: RuntimeSourceReadHostResponse,
    runtime: &mut Runtime,
) -> DispatchOutcome {
    let result = (|| {
        let argument_words =
            dispatcher.deferred_argument_words(call, response.request_id, &runtime.memory)?;
        runtime
            .validate_owned_host_request(response.request_id, RUNTIME_SOURCE_READ_SERVICE)
            .map_err(|e| fault(&call.frame, e))?;
        let pending = runtime
            .winmm
            .pending
            .get(&response.request_id)
            .ok_or_else(|| fault(&call.frame, "MMIO open is no longer pending"))?;
        let Backing::File { bytes, .. } = &pending.stream.backing else {
            unreachable!()
        };
        if !call.frame.endpoint.module.eq_ignore_ascii_case("winmm.dll")
            || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "mmioOpenA")
            || call.request.service != RUNTIME_SOURCE_READ_SERVICE
            || response.source_id != pending.source_id
            || response.object_id != pending.object_id
            || response.offset != bytes.len() as u64
            || response.requested_bytes != pending.requested
            || response.transferred_bytes as usize != response.bytes.len()
            || response.transferred_bytes > pending.requested
            || (response.win32_error != 0 && !response.bytes.is_empty())
            || regions(
                runtime,
                pending.info,
                if pending.info.0 == 0 { 0 } else { INFO_BYTES },
            ) != pending.info_regions
            || regions(
                runtime,
                pending.stream.buffer,
                if pending.stream.buffer.0 == 0 {
                    0
                } else {
                    pending.stream.capacity as usize
                },
            ) != pending.buffer_regions
        {
            return Err(fault(
                &call.frame,
                "MMIO source completion identity, bounds or output lifetime changed",
            ));
        }
        if pending.info.0 != 0 {
            check_write(runtime, &call.frame, pending.info.0, INFO_BYTES)?;
        }
        if pending.stream.buffer.0 != 0 {
            check_write(
                runtime,
                &call.frame,
                pending.stream.buffer.0,
                pending.stream.capacity as usize,
            )?;
        }
        let mut pending = runtime.winmm.pending.remove(&response.request_id).unwrap();
        let Backing::File { bytes, handle } = &mut pending.stream.backing else {
            unreachable!()
        };
        if response.win32_error != 0 || response.transferred_bytes != pending.requested {
            runtime
                .close_handle(*handle)
                .map_err(|e| fault(&call.frame, e))?;
            let immediate = open_error(pending.info, CANNOTOPEN);
            let outcome = dispatcher.complete_deferred(
                call,
                HostCompletion {
                    request_id: response.request_id,
                    result: immediate,
                },
                runtime,
            );
            if matches!(outcome, DispatchOutcome::Immediate(_)) {
                runtime
                    .complete_host_request(response.request_id)
                    .map_err(|e| fault(&call.frame, e))?;
            }
            return Ok(outcome);
        }
        if bytes.try_reserve(response.bytes.len()).is_err() {
            runtime
                .close_handle(*handle)
                .map_err(|e| fault(&call.frame, e))?;
            let outcome = dispatcher.complete_deferred(
                call,
                HostCompletion {
                    request_id: response.request_id,
                    result: open_error(pending.info, OUTOFMEMORY),
                },
                runtime,
            );
            if matches!(outcome, DispatchOutcome::Immediate(_)) {
                runtime
                    .complete_host_request(response.request_id)
                    .map_err(|e| fault(&call.frame, e))?;
            }
            return Ok(outcome);
        }
        bytes.extend(response.bytes);
        if bytes.len() < pending.length as usize {
            let offset = bytes.len() as u32;
            pending.requested = (pending.length - offset).min(RUNTIME_SOURCE_READ_MAX_BYTES);
            let payload = [
                pending.source_id,
                pending.object_id,
                offset,
                0,
                pending.requested,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
            runtime.winmm.pending.insert(response.request_id, pending);
            return Ok(DispatchOutcome::Deferred(ApiDeferredCall {
                frame: call.frame.clone(),
                request: HostRequest {
                    request_id: response.request_id,
                    service: RUNTIME_SOURCE_READ_SERVICE.to_owned(),
                    payload,
                },
            }));
        }
        let immediate = publish(runtime, &call.frame, pending.stream, pending.info)?;
        let outcome = match runtime.complete_api_immediate(argument_words, &call.frame, immediate) {
            Ok(completion) => DispatchOutcome::Immediate(completion),
            Err(error) => DispatchOutcome::Fault(error),
        };
        if matches!(outcome, DispatchOutcome::Immediate(_)) {
            runtime
                .complete_host_request(response.request_id)
                .map_err(|e| fault(&call.frame, e))?;
        }
        Ok(outcome)
    })();
    result.unwrap_or_else(DispatchOutcome::Fault)
}

pub(super) fn dispatch(frame: &ApiCallFrame, runtime: &mut Runtime) -> ApiHandlerResult {
    let result = (|| {
        let ApiSymbol::Name { name } = &frame.endpoint.symbol else {
            return Err(fault(frame, "MMIO export name"));
        };
        if name == "mmioOpenA" {
            return open(frame, runtime);
        }
        let handle = frame
            .argument_u32(0)
            .ok_or_else(|| fault(frame, "MMIO handle argument"))?;
        let Some(stream) = runtime.winmm.streams.get(&handle) else {
            let value = if matches!(name.as_str(), "mmioRead" | "mmioSeek") {
                u32::MAX
            } else {
                INVALHANDLE
            };
            return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(value))));
        };
        let arg = |index| {
            frame
                .argument_u32(index)
                .ok_or_else(|| fault(frame, "MMIO argument"))
        };
        let mut result = ApiImmediate::new(Some(0));
        match name.as_str() {
            "mmioClose" => {
                if arg(1)? != 0 {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                }
                if let Some(heap) = stream.owned_heap {
                    runtime
                        .free_private_heap_block(heap, stream.buffer)
                        .map_err(|e| fault(frame, e))?;
                }
                let stream = runtime.winmm.streams.remove(&handle).unwrap();
                if let Backing::File { handle, .. } = stream.backing {
                    runtime.close_handle(handle).map_err(|e| fault(frame, e))?;
                }
            }
            "mmioGetInfo" => {
                if arg(2)? != 0 {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                }
                let output = arg(1)?;
                check_write(runtime, frame, output, INFO_BYTES)?;
                let mut stream = runtime.winmm.streams.remove(&handle).unwrap();
                let filled = fill_buffer(runtime, frame, &mut stream);
                result = result.with_guest_write(GuestAddress(output), stream.info(handle));
                runtime.winmm.streams.insert(handle, stream);
                filled?;
            }
            "mmioSetInfo" | "mmioAdvance" => {
                let pointer = arg(1)?;
                if arg(2)? != 0 || pointer == 0 {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                }
                let input = read(runtime, frame, pointer, INFO_BYTES)?;
                let position = match accept_info(stream, &input, handle) {
                    Ok(p) => p,
                    Err(e) => return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(e)))),
                };
                if name == "mmioAdvance" {
                    check_write(runtime, frame, pointer, INFO_BYTES)?;
                    let mut stream = runtime.winmm.streams.remove(&handle).unwrap();
                    let old = stream.position;
                    stream.position = position;
                    let filled = fill_buffer(runtime, frame, &mut stream);
                    if filled.is_err() {
                        stream.position = old;
                    }
                    result = result.with_guest_write(GuestAddress(pointer), stream.info(handle));
                    runtime.winmm.streams.insert(handle, stream);
                    filled?;
                } else {
                    runtime.winmm.streams.get_mut(&handle).unwrap().position = position;
                }
            }
            "mmioRead" => {
                let count = arg(2)?;
                if count > 16 * 1024 * 1024 {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        u32::MAX,
                    ))));
                }
                let length = count.min(stream.length().saturating_sub(stream.position));
                let bytes = bytes_at(
                    runtime,
                    frame,
                    stream,
                    stream.position.min(stream.length()),
                    length,
                )?;
                check_write(runtime, frame, arg(1)?, length as usize)?;
                result =
                    ApiImmediate::new(Some(length)).with_guest_write(GuestAddress(arg(1)?), bytes);
                runtime.winmm.streams.get_mut(&handle).unwrap().position += length;
            }
            "mmioSeek" => {
                let origin = arg(2)?;
                let base = match origin {
                    0 => 0,
                    1 => stream.position,
                    2 => stream.length(),
                    _ => {
                        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                            u32::MAX,
                        ))));
                    }
                };
                let position = i64::from(base) + i64::from(arg(1)? as i32);
                if !(0..=i64::from(i32::MAX)).contains(&position) {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        u32::MAX,
                    ))));
                }
                runtime.winmm.streams.get_mut(&handle).unwrap().position = position as u32;
                result.eax = Some(position as u32);
            }
            "mmioDescend" => {
                let output = arg(1)?;
                let parent = arg(2)?;
                let flags = arg(3)?;
                let input = read(runtime, frame, output, 20)?;
                check_write(runtime, frame, output, 20)?;
                if !matches!(flags, 0 | 0x10 | 0x20 | 0x40) {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                }
                let limit = if parent == 0 {
                    stream.length()
                } else {
                    let p = read(runtime, frame, parent, 20)?;
                    word(&p, 3)
                        .checked_add(word(&p, 1))
                        .filter(|end| *end <= stream.length())
                        .ok_or_else(|| fault(frame, "MMIO parent chunk exceeds the stream"))?
                };
                let mut position = stream.position;
                let mut found = None;
                while position.checked_add(8).is_some_and(|end| end <= limit) {
                    let header = bytes_at(runtime, frame, stream, position, 8)?;
                    let (id, size) = (word(&header, 0), word(&header, 1));
                    let offset = position + 8;
                    let Some(end) = offset.checked_add(size).filter(|end| *end <= limit) else {
                        break;
                    };
                    let container = matches!(id, 0x46464952 | 0x5453494c);
                    let kind = if container && size >= 4 {
                        word(&bytes_at(runtime, frame, stream, offset, 4)?, 0)
                    } else {
                        0
                    };
                    let matches = match flags {
                        0 => true,
                        0x10 => id == word(&input, 0),
                        0x20 => id == 0x46464952 && kind == word(&input, 2),
                        0x40 => id == 0x5453494c && kind == word(&input, 2),
                        _ => false,
                    };
                    if matches && (!container || size >= 4) {
                        found = Some((
                            id,
                            size,
                            kind,
                            offset,
                            offset + if container { 4 } else { 0 },
                        ));
                        break;
                    }
                    let Some(next) = end.checked_add(size & 1) else {
                        break;
                    };
                    position = next;
                }
                if let Some((id, size, kind, offset, position)) = found {
                    result = result.with_guest_write(
                        GuestAddress(output),
                        [id, size, kind, offset, 0]
                            .into_iter()
                            .flat_map(u32::to_le_bytes)
                            .collect(),
                    );
                    runtime.winmm.streams.get_mut(&handle).unwrap().position = position;
                } else {
                    result.eax = Some(CHUNKNOTFOUND);
                }
            }
            "mmioAscend" => {
                let input = read(runtime, frame, arg(1)?, 20)?;
                let (size, offset) = (word(&input, 1), word(&input, 3));
                if arg(2)? != 0 || word(&input, 4) != 0 {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                }
                let Some(end) = offset
                    .checked_add(size)
                    .and_then(|end| end.checked_add(size & 1))
                    .filter(|end| *end <= stream.length())
                else {
                    return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                        INVALPARAM,
                    ))));
                };
                runtime.winmm.streams.get_mut(&handle).unwrap().position = end;
            }
            _ => return Err(fault(frame, "unsupported MMIO export")),
        }
        Ok(ApiHandlerResult::Immediate(result))
    })();
    result.unwrap_or_else(ApiHandlerResult::Fault)
}
