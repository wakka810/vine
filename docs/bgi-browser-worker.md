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

An immediate API completion also carries a Rust-owned execution-boundary disposition.
The normal value continues the 64-boundary cooperative quantum; `yield-current-thread`
selects the next runnable competitor and resets that quantum without creating a host
request. The Worker transports the disposition and applies the returned opaque CPU
selection, but does not decide whether an API yields. Callback non-interleaving and the
same mutation-free plan／stale／duplicate-commit checks apply to both dispositions.

The reached `runtime.timer-delay.v1` service is a bounded scheduling exception.
Rust owns the logical target and completion result; the Worker owns only the
browser delay and page-lifecycle resource. While that delay is pending, the
single emulated CPU does not execute another guest lane. Completion consumes the
Rust-selected owner lane once before normal scheduling resumes, preventing
browser wall timing from changing ordered guest semantics. Other host services
continue to allow a different runnable guest lane.

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

The active private profile is `kernel32-get-file-attributes-a` and uses protocol
`vine.browser-runtime-worker.v23`. It inherits the immutable boot-archive snapshot,
writable-profile separation, 240 Rust-owned `Sleep(0)` yields, and the deterministic
32-byte x86 `GlobalMemoryStatus` output, then accepts 23 `GetFileAttributesA` path
queries. Node and Chrome require the same call identity, missing-file／missing-path
errors, zero guest writes, Rust VFS authority, semantic digest, memory digest, and
`CreateDirectoryA` terminal endpoint. Protocol version 22 remains the frozen
predecessor for the typed memory-status snapshot; version 23 adds the typed
file-attribute snapshot without moving Windows path or filesystem state to
TypeScript.

The semantic gate passes independently from the repeated workload gate. These bounded
scheduler, system-information, and VFS query leaves do not change the host-service transport, memory topology, browser-resource
lifetime, graphics, or audio path, so the last versioned archive workload remains risk
evidence and is not relabelled as a measurement of the longer active profile. Exact
current semantic and artifact values remain manifest-owned.

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
- `Sleep` intervals greater than zero, no-competitor timing, host wall-clock delay,
  priority-weighted selection, and preemptive scheduling are not implemented.
- The versioned workload gate is not a full-play or long-idle soak.
- Unsupported behavior remains an explicit structured stop. A normal capability
  completion requires a versioned count and next-endpoint contract; reaching a
  later API by itself does not establish subsystem completeness.
- `DragAcceptFiles` changes only the logical acceptance bit. Browser drag
  events, `HDROP`, `WM_DROPFILES`, file permission, and VFS import remain
  unimplemented.
