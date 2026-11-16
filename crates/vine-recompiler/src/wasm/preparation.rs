//! Bounded native preparation of the actual generated execution body.
//!
//! The core's null-pointer/zero-fuel entry returns before guest state, memory,
//! imports or unit identity are accessed. Repeated entries give Chrome an
//! opportunity to optimize that body before launch. This is a finite workload,
//! not an engine-specific tier threshold or a promise of optimization readiness.
use super::{sleb, uleb};

pub(super) fn function_body(core: u32) -> Vec<u8> {
    // The existing zero-argument preparation ABI is unchanged. No guest values
    // or host pointers enter this function; its two locals are private counters.
    let mut body = vec![1, 2, 0x7f]; // remaining rounds and returned status
    body.push(0x41);
    sleb(&mut body, 64);
    body.extend([0x21, 0, 0x03, 0x40]); // remaining = 64; loop
    for _ in 0..18 {
        body.extend([0x41, 0]);
    }
    body.push(0x10); // call the same core used by run/link, without tail recursion
    uleb(&mut body, core);
    body.extend([
        0x22, 1, // retain the status
        0x04, 0x40, // fail immediately on any nonzero result
        0x20, 1, 0x0f, 0x0b, 0x20, 0, 0x41, 1, 0x6b, 0x22, 0, 0x0d,
        0, // --remaining; br_if loop
        0x0b, 0x41, 0, 0x0b,
    ]);
    body
}
