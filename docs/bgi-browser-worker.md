# BGI Chrome Runtime Worker

## Purpose

This document describes the stable execution contract for running the private BGI
checkpoint in Chrome. Current protocol versions, artifact identities, measurements,
and the exact structured fault or capability completion are recorded only in
[current-checkpoint.json](current-checkpoint.json).

The private executable is served from its fingerprinted installed path. It is not
copied into the browser build, `.work`, or Git, and the reproduction gate verifies
that its bytes remain unchanged.

## Authority boundary

- Rust/Wasm owns guest memory, process and thread state, scheduler lanes, module and
  object identity, heap state, synchronization, and Win32 compatibility semantics.
- The Runtime Worker owns bounded execution orchestration and browser adapters. It
  does not maintain a second Windows-state authority.
- Rust `RuntimeFiles` owns the immutable manifest, path／file identity, typed handle,
  cursor, and pending read. The Worker binds Rust IDs to the prepared manifest and
  performs only title／snapshot／object-addressed same-origin byte requests.
- Node is a deterministic semantic oracle, not a production scheduler or memory
  authority.
- The execution engine sees versioned CPU contexts, checked memory reflections and
  deltas, mapping operations, and cache effects. v86 paging details remain private to
  the adapter.

## Execution contract

The Worker keeps one opaque CPU context per registered guest thread. Rust chooses an
executable scheduler lane; the Worker saves the live context, restores the selected
thread, and then runs another bounded slice. A host completion is applied to the CPU
context of the request-owning thread even when a different thread is current.

The controlled fixture proves that the initial thread can wait on host work while a
second thread runs with its own EIP and ESP, writes a marker, exits, and yields back to
the resumed initial thread. Thread, request, callback depth, completion policy, and
cancellation identity are validated across the bridge. General Win32 thread creation,
TEB lifecycle, wait families, fairness policy, and exception delivery remain separate
gateway work.

`GuestMemory` remains the byte authority. The v86 mirror grows within the
Rust-provided physical-page limit, reuses released pages, and rejects exhaustion before
applying a mapping delta. Heap admission uses the versioned flags-and-size-range
allocator policy; historical exact-size profiles are accepted only as explicit legacy
evidence input.

Cancellation and watchdog stops occur only between complete CPU/writeback
transactions. They retire host requests and callbacks, stop the Rust process, detach
the module graph, and leave no runnable Worker session behind.

## Equivalence and acceptance

Chrome and the Node oracle are compared using guest-visible semantics: ordered API
occurrences, decoded arguments, return and stack state, LastError, guest writes,
directory and synchronization metadata, source request／cursor／payload identity,
final memory content, and the exact terminal evidence. Memory generations and whole-report hashes are synchronization or artifact
identity, not guest-visible equivalence.

The browser gate also covers success, cancellation, watchdog, callback, Canvas, audio,
IndexedDB, immutable source HTTP identity, memory mapping, physical-pool growth and reuse,
multi-thread execution, and source fingerprint preservation. Performance thresholds and their current results are
defined by the versioned workload and recorded in the current manifest.

## Reproduction

```powershell
npm run check:runtime:bgi-browser
```

The command rebuilds the Rust/Wasm and Worker artifacts, regenerates the Node oracle,
runs clean-profile Chrome probes, compares semantic results, and verifies the source
fingerprint before and after execution.

## Non-claims

- The checkpoint is not title rendering, input, audio, writable persistence, save/load, or full-play
  acceptance.
- The controlled second thread does not claim broad Win32 thread, wait,
  synchronization, APC, SEH, or scheduling fairness compatibility.
- The versioned workload gate is not a full-play or long-idle soak.
- Unsupported behavior remains an explicit structured stop. A normal capability
  completion requires a versioned count and next-endpoint contract; reaching a
  later API by itself does not establish subsystem completeness.
