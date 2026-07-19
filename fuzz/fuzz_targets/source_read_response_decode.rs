#![no_main]

use libfuzzer_sys::fuzz_target;
use vine_core::{
    RUNTIME_SOURCE_READ_MAX_BYTES, RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES,
    RuntimeSourceReadHostResponse,
};

const SELECTOR_BYTES: usize = 2;
const MAX_INPUT_BYTES: usize = SELECTOR_BYTES
    + RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES
    + RUNTIME_SOURCE_READ_MAX_BYTES as usize
    + 1;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_BYTES {
        return;
    }
    let encoded_split = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let bytes = &data[2..];
    let split = encoded_split.min(bytes.len());
    let _ = RuntimeSourceReadHostResponse::decode_binary(
        1,
        &bytes[..split],
        &bytes[split..],
    );
});
