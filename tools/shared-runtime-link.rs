//! Glue compiled by build-shared-runtime.mjs against the separately pinned CPU.
//! One linker/allocator, with v86's fixed low state reserved below --global-base.
pub use v86::*;
pub use vine_runtime_wasm::*;

use v86::cpu::{cpu as processor, global_pointers as state, vine};
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn vine_host_now() -> f64;
}
static mut CLOCK: Option<(u64, f64, f64)> = None;
static mut DIRECT_CALLS: u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn vine_shared_bind_cpu() -> u32 {
    let Some((base, size, offset)) = shared_arena_descriptor() else {
        return 1;
    };
    let Some(permissions) = shared_permissions_pointer() else {
        return 1;
    };
    // Runtime owns this range until the entire Wasm instance is discarded.
    let bound = unsafe { v86::cpu::memory::bind_runtime_memory(base, size) };
    if bound {
        unsafe {
            vine::set_common_memory(base + offset, permissions);
        }
    }
    u32::from(!bound)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_shared_set_clock(retired: u64, elapsed_ms: f64) -> u32 {
    unsafe {
        if processor::vine_execution_is_active()
            || !elapsed_ms.is_finite()
            || elapsed_ms < 0.0
            || elapsed_ms > (u64::MAX / 10000) as f64
        {
            return 1;
        }
        CLOCK = Some((retired, elapsed_ms, vine_host_now()));
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_shared_direct_calls() -> u32 {
    unsafe { DIRECT_CALLS }
}

fn context() -> X86CpuContext {
    unsafe {
        X86CpuContext {
            eax: *state::reg32 as u32,
            ecx: *state::reg32.add(1) as u32,
            edx: *state::reg32.add(2) as u32,
            ebx: *state::reg32.add(3) as u32,
            esp: *state::reg32.add(4) as u32,
            ebp: *state::reg32.add(5) as u32,
            esi: *state::reg32.add(6) as u32,
            edi: *state::reg32.add(7) as u32,
            eip: *state::instruction_pointer as u32,
            eflags: processor::get_eflags() as u32,
            es: *state::sreg as u32,
            cs: *state::sreg.add(1) as u32,
            ss: *state::sreg.add(2) as u32,
            ds: *state::sreg.add(3) as u32,
            fs: *state::sreg.add(4) as u32,
            gs: *state::sreg.add(5) as u32,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_shared_execute_budget(budget: u32, low: u32, high: u32) -> u32 {
    unsafe {
        let mut generation = u64::from(low) | u64::from(high) << 32;
        let initial = vine::vine_instruction_counter();
        let clock = CLOCK;
        CLOCK = None;
        let mut calls = 0;
        loop {
            let used = vine::vine_instruction_counter().wrapping_sub(initial);
            if used >= budget {
                return 1;
            }
            let reason =
                with_shared_execution(generation, || processor::vine_execute_budget(budget - used));
            let Some((retired, elapsed, stamp)) = clock else {
                return reason;
            };
            // Ordinary Win32 endpoints use UD2 in existing unmodified modules.
            // COM, return sentinels, callbacks and actual faults keep their exits.
            if reason != 4 && reason != 0x106 {
                return reason;
            }
            let used = vine::vine_instruction_counter().wrapping_sub(initial);
            let Some(retired) = retired.checked_add(u64::from(used)) else {
                return u32::MAX;
            };
            let now = vine_host_now();
            let elapsed = ((elapsed + (now - stamp).max(0.0)) * 10000.0) as u64;
            let dispatch = dispatch_shared(context(), retired, elapsed, |page| {
                vine::invalidate_common_page(page)
            });
            match dispatch {
                SharedDispatch::Unhandled => return reason,
                SharedDispatch::Output => return 5,
                SharedDispatch::Error => return u32::MAX,
                SharedDispatch::Resume([eip, esp, eax, write_eax]) => {
                    if processor::vine_cpu_apply_stopped_resume(eax, write_eax, esp, eip) != 0 {
                        return u32::MAX;
                    }
                    generation = shared_acknowledged_generation();
                    DIRECT_CALLS = DIRECT_CALLS.wrapping_add(1);
                    calls += 1;
                    // Retain a prompt browser/scheduler boundary even in a tight
                    // API loop; all completed instructions count against the slice.
                    if calls >= 4096 || now - stamp >= 4.0 {
                        return 1;
                    }
                }
            }
        }
    }
}
