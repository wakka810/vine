# BGI private-heap allocation and coalesced reuse

## Purpose

The primary continuation reaches four adjacent flags-zero `HeapAlloc` calls:
three small buffers followed by an ordinary multi-page buffer. The final call
reuses the low end of the two adjacent ranges retired by the preceding large
ordinary `HeapFree` checkpoint. This checkpoint proves the existing allocator's
identity, coalescing, and memory-publication invariants; it does not add a
title-specific allocation route. Current sequences, addresses, generations,
digests, artifact identities, and the next structured stop remain authoritative
only in [`current-checkpoint.json`](current-checkpoint.json).

## Runtime contract

`runtime/heaps.rs::RuntimeHeaps` remains the sole owner of free blocks,
allocation identities, live counts, per-heap allocation-ID cursors, and backing
relations. `RuntimeKernel` coordinates the existing cross-subsystem transaction:

1. Validate the heap, flags, nonzero bounded size, aligned first-fit candidate,
   allocation capacity, backing owner, and GuestMemory capacity without
   mutation.
2. Coalesce adjacent free ranges inside `RuntimeHeaps`; do not reconstruct this
   relation from guest bytes or Worker state.
3. Build the allocation plan with a fresh monotonic identity even when the
   selected address was previously live.
4. Publish deterministic uninitialized bytes through one GuestMemory
   transaction, then commit only the planned heap mutation.
5. Return the exact allocation base, preserve LastError, and perform no guest
   output write.

The small allocations reuse existing ordinary backing pages. The final
multi-page allocation starts at the lower of the two preceding freed ranges and
spans their former boundary, demonstrating that coalescing is allocator state,
not an address coincidence. Every allocation emits a dirty-only binary host
delta with no mapping operation or instruction-cache effect. Allocation or
memory preflight failure must preserve the free-block index, allocation cursor,
live count, guest bytes, mappings, and generations.

## Node and Chrome evidence contract

The Node runner and Chrome Worker continue through the complete adjacent
`HeapAlloc` batch and stop at the first different runtime-owned endpoint. Each
completion retains the Rust-produced allocation snapshot, live count, host
delta generation, dirty-page owner and range, and payload digest.

Node records its compact memory snapshot after acknowledging each delta. The
Worker records the Rust snapshot before host acknowledgement, applies the same
binary delta to the execution mirror, and then acknowledges it. Acceptance
requires matching allocation semantics, one-generation delta advances, page
ranges, payloads, final memory content, and explicit acknowledgement ordering.
Absolute Node and Chrome generations are not required to match because their
bounded CPU writebacks occur at different transport boundaries.

The grouped reproduction is:

```powershell
npm run check:runtime:bgi-browser
```

## Non-claims

This checkpoint does not add new `HeapAlloc` flags, zero-size behavior,
`HeapReAlloc`, concurrent allocation, LFH behavior, a third ordinary backing
segment, or support for the gap between the ordinary-size limit and the exact
64 MiB variant. It does not import native pointer values or claim that the
deterministic fill byte is Windows heap content. The following reached
`timeGetTime` call is already implemented but belongs to the next continuation
checkpoint.
