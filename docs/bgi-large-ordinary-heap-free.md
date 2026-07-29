# BGI large ordinary private-heap free

## Purpose

The primary continuation reaches two consecutive `HeapFree` calls for ordinary
multi-page private-heap allocations, then reaches a different heap endpoint.
This is a semantic-leaf checkpoint: the general free transaction already
exists, while this checkpoint proves its allocation-lifetime and Worker
transport invariants for large reached buffers. Current call counts, addresses,
generations, report identities, and the next stop remain authoritative only in
[`current-checkpoint.json`](current-checkpoint.json).

The reached allocations belong to an ordinary private-heap backing segment.
They are distinct from the process-permanent exact 64 MiB large-allocation slot
described in [bgi-large-heap-allocation.md](bgi-large-heap-allocation.md).

## Runtime contract

`RuntimeHeaps` remains the sole owner of logical heap allocation identity,
liveness, allocation-ID cursors, and backing relations. `RuntimeKernel`
coordinates the cross-subsystem transaction without exposing or directly
mutating those collections:

1. Validate the live heap handle, flags, exact allocation base, allocation
   identity, backing owner, reserved range, and GuestMemory capacity without
   mutation.
2. Build the heap retirement plan and a GuestMemory transaction that fills the
   complete reserved allocation with Vine's deterministic freed byte `0xDD`.
3. Commit the memory transaction, then target only the planned heap allocation
   and reject a stale or duplicate commit.
4. Return `TRUE`, preserve LastError, and perform no guest output write.

The free publishes no mapping operation and no instruction-cache effect. Its
binary host delta contains only the dirty pages intersecting the poisoned
reserved range. Interior, foreign, stale, wrong-owner, unsupported-flag, and
memory-preflight failures leave allocation liveness, allocator state, guest
bytes, mappings, and generations unchanged. The poison is a Vine diagnostic
policy and is not a claim about bytes returned by the Windows heap.

## Node and Chrome evidence contract

The Node runner and Chrome Worker both retain the Rust-produced allocation
snapshot, live-count transition, memory-delta generation, dirty-page owner and
range, and payload digest for every free in this checkpoint batch. They
continue through all adjacent `HeapFree` calls and stop at the first different
runtime-owned endpoint, so one logical lifetime batch cannot be split into
misleading single-call checkpoints.

Node acknowledges each delta before recording its compact evidence. The Worker
records the Rust snapshot before host acknowledgement, applies the same binary
delta, then acknowledges it. Absolute generation numbers may differ because
Node and Chrome reconcile bounded CPU slices at different points; acceptance
therefore requires the same one-generation advance, dirty pages, payload, final
memory content, and explicit pre-ack／post-ack ordering rather than equal
absolute generations.

The canonical grouped reproduction is:

```powershell
npm run check:runtime:bgi-browser
```

Focused Rust heap tests cover exact live-base retirement, freed-byte
publication, reuse with a new allocation identity, and mutation-free negative
cases. The private Node and Chrome continuation supplies the reached caller and
transport oracle.

## Non-claims

This checkpoint does not enable new `HeapFree` flags, NULL or interior-pointer
behavior, arbitrary heap corruption behavior, `HeapReAlloc`, concurrent heap
mutation, new allocation sizes, or the next reached `HeapAlloc` variant. It
does not change the special 64 MiB slot's release-without-poison contract.
