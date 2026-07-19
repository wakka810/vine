#![no_main]

use libfuzzer_sys::fuzz_target;
use vine_core::CpuWritebackBatch;

const MAX_FUZZ_PAGES: usize = 64;
const SELECTOR_BYTES: usize = 2;
const MAX_INPUT_BYTES: usize = SELECTOR_BYTES + 32 + MAX_FUZZ_PAGES * 0x1000;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_BYTES {
        return;
    }
    let encoded_split = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let bytes = &data[2..];
    let split = encoded_split.min(bytes.len());
    let _ = CpuWritebackBatch::decode_binary(
        &bytes[..split],
        &bytes[split..],
        MAX_FUZZ_PAGES,
    );
});
