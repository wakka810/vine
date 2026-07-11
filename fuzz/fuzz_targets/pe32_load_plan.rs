#![no_main]

use libfuzzer_sys::fuzz_target;
use vine_core::{ModuleSnapshot, Pe32Limits, build_pe32_load_plan};

fuzz_target!(|data: &[u8]| {
    let limits = Pe32Limits::default();
    if data.len() as u64 > limits.max_file_bytes {
        return;
    }
    if let Ok(snapshot) = ModuleSnapshot::from_bytes(data.to_vec(), &limits) {
        let _ = build_pe32_load_plan(&snapshot, limits);
    }
});
