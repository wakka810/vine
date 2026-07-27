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
- Rust `RuntimeFiles` owns the immutable manifest, path／file／directory identity,
  typed handle, cursor, generation, and pending read／commit／directory-create plans.
  The Worker binds Rust IDs to the prepared manifest／profile and performs only
  title／snapshot／profile／object-addressed same-origin storage requests.
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

The active private profile is `d3d9-initial-texture-stage-states` and uses protocol
`vine.browser-runtime-worker.v35`. It inherits the immutable boot-archive snapshot,
writable-profile separation, 480 Rust-owned `Sleep(0)` yields, deterministic 32-byte
x86 `GlobalMemoryStatus`, and 23 `GetFileAttributesA` path queries. It then durably
creates the run-local allowlisted `UserData` directory through SQLite before Rust
publishes directory existence. The fifth reached `SetWindowPos` repeats the already-active
same geometry and completes exactly one `WM_WINDOWPOSCHANGING` callback without changing
window geometry, active／focus／foreground relations, or their generations. Node and Chrome
require the same BOOL／LastError, callback frame and scratch identity, zero guest writes,
and Rust `RuntimeUser32` authority. They then reproduce the three `2048x1024` D3D9
texture creations and the exact nine-call system-memory surface fill chain with
`RuntimeGraphics`／`RuntimeCom` authority, no browser renderer resource, and a
transactional publication boundary. The final semantic and memory digests agree at the
following frame-change call. That sixth `SetWindowPos` then completes direct frames 26–28
for `WM_WINDOWPOSCHANGING`, `WM_NCCALCSIZE`, and `WM_WINDOWPOSCHANGED`, plus nested frame 29
for `WM_MOVE`. Rust commits only the outer／client origin at NCCALCSIZE completion; active／
focus／foreground relations and GDI resources remain unchanged. Native non-client paint and
erase are explicit non-claims, and TypeScript creates no browser window resource.

The following already-visible `ShowWindow(hwnd, SW_SHOWNORMAL)` is a mutation-free logical
window read: it returns normalized TRUE for previous visibility, preserves LastError, invokes
no callback, and leaves visibility and active／focus／foreground relations unchanged. The
subsequent `UpdateWindow` synchronously enters frame 30 with `WM_PAINT`. `BeginPaint` consumes
the Rust-owned full-client update region, publishes one `RuntimeGdi` paint HDC and a 64-byte
x86 `PAINTSTRUCT` as a single preflighted transaction, and creates one Rust-owned paint
session. `EndPaint` accepts only the exact HWND／thread／structure relation and retires both
session and HDC. A generic `ReleaseDC` cannot retire the paint HDC. The enclosing
`UpdateWindow` callback is still active when execution reaches the first D3D9 `Clear`, so
the evidence distinguishes the pending outer call from the completed Begin／End pair.

Slot 43 then accepts only
`Clear(0, NULL, D3DCLEAR_TARGET, 0xFF000000, 1.0f, 0)`. Rust resolves the live COM
object to logical graphics device 3 and commits a uniform full-presentation clear to its
`1280x720` back buffer after device, content, and sequence preflight. Node and Chrome agree
on sequence 127,355, reference count 5, back-buffer content generation 1, clear sequence 1,
rectangle `[0,0,1280,720]`, color `0xFF000000`, device generation 44, and zero guest writes.
TypeScript creates no renderer resource or duplicate back-buffer state. Protocol version 29
adds this typed logical-device snapshot while retaining the v28 ShowWindow／UpdateWindow／paint
evidence.

The following `SetRect(0x006FF29C, 0, 0, 1280, 720)` leaf writes the complete
16-byte x86 `RECT` as one preflighted guest-memory update, returns normalized TRUE, and
preserves LastError. Signed `LONG32` coordinates and the NULL-output FALSE path are fixed by
the native oracle and focused Rust negative tests; an unmapped output faults before changing
any guest byte. Rust owns the guest-memory transaction and no persistent rectangle state is
created in either Rust or TypeScript. Protocol version 30 adds the typed arguments, result,
output address and bytes, stack transition, and failure-policy evidence. Node and Chrome agree
on sequence 127,356, instruction 28,761,157, output
`000000000000000000050000d0020000`, and the cumulative semantic and terminal-memory digests.
Both engines continue eight instructions later through `IDirect3DTexture9::AddDirtyRect`
slot 21 with the same texture identity and `RECT*`. Rust validates the recreated static
SYSTEMMEM source texture and stores its authoritative dirty region in `RuntimeGraphics`.
Because creation already marked the whole level dirty, the primary rectangle succeeds
idempotently without changing texture generation. Protocol version 31 adds the logical
texture descriptor, COM relation, dirty-region snapshot, mutation policy, and non-claims.
Node and Chrome agree on sequence 127,357, instruction 28,761,165, texture 7／COM 22,
device 3, `2048x1024`, whole-texture dirty state, texture generation 41, content
generation 1, one logical fill, zero guest writes, and no renderer resource.

Both engines continue ten instructions later through
`IDirect3DDevice9::UpdateTexture(source=0x01020004,destination=0x01030004)` slot 31.
Rust resolves both COM identities to textures on device 3, preflights the static SYSTEMMEM
source and matching static DEFAULT destination, copies the texture-owned level-zero logical
fill, consumes the source whole dirty region, and commits both resource generations once.
Protocol version 32 adds the authoritative source／destination snapshots and transfer metadata;
TypeScript records them without creating texel storage or renderer state. Node and Chrome agree
on sequence 127,358, instruction 28,761,175, source texture 7／COM 22 at generation 42,
destination texture 8／COM 24 at generation 45, content generation 1, fill sequence 3 over
`[0,0,1280,720]`, update sequence 1, clean source, zero guest writes, and no renderer resource.
Both continue six instructions later through `IDirect3DDevice9::BeginScene` slot 41.
Rust validates the live device and inactive scene, preflights the next scene sequence and
generation, then commits a single inactive-to-active transition. Protocol version 33 adds
the authoritative logical-device scene snapshot and lifecycle policy. Node and Chrome agree
on sequence 127,359, instruction 28,761,181, device 3／COM 21, reference count 5,
`1280x720` presentation, black-clear content generation 1, active scene sequence 1,
device generation 45, zero guest writes, and no renderer resource. Both stop fourteen
instructions later at `IDirect3DDevice9::SetSamplerState(0,6,2)` slot 69, endpoint
`0x01011480`, return address `0x1005B18A`. Both engines then complete the exact
sampler-0 `D3DSAMP_MINFILTER = D3DTEXF_LINEAR` and
`D3DSAMP_MAGFILTER = D3DTEXF_LINEAR` calls at sequences 127,360／127,361 and
instructions 28,761,195／28,761,204. Protocol version 34 carries immutable
Rust-owned sparse sampler-state snapshots and device generations 46／47; the
Worker validates them without creating browser sampler or renderer state. The
caller ignores explicit `D3DERR_NOTAVAILABLE` results for the following
unprofiled `SetRenderState(171,1)`, `(20,6)`, and `(19,5)` calls. Node and Chrome
agree on those three rejections and stop at instruction 28,761,237 on
`IDirect3DDevice9::SetTextureStageState(0,1,4)` slot 67, endpoint `0x01011460`,
return address `0x1005B1F5`. Both engines then complete the six reached stage-0
settings in state order 1–6 with values `4,2,0,2,2,0`. Protocol version 35 carries
the cumulative Rust-owned color operation／arguments and alpha operation／arguments
with device generations 48–53. Node and Chrome agree on sequences 127,365–127,370,
zero guest writes, reference count 5, and no renderer resource, then stop at
instruction 28,761,291 on `IDirect3DDevice9::SetTexture(0,0x01030004)` slot 65,
endpoint `0x01011440`, return address `0x1005B27C`.

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

- The checkpoint is not title rendering, input, audio, complete save/load, or full-play
  acceptance. It proves one bounded durable directory mutation, not general writable
  persistence.
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
- The reached paint leaf does not claim general invalidation, overlapping or nested paint,
  background erase, clipping, queued paint dispatch, browser presentation, or arbitrary
  HDC operations.
- The reached Clear does not claim partial rectangles, depth／stencil clears, pixel storage,
  upload, draw, Present, device loss, or visible browser output.
- The reached AddDirtyRect stores only a bounded Rust-owned dirty region. The following
  UpdateTexture consumes that region and copies only Rust-owned logical commands; it does
  not allocate pixel storage, copy texel bytes, support partial dirty regions or the native
  dynamic-destination variant, or upload to a renderer. The following BeginScene creates
  only Rust-owned logical scene state, the two reached sampler calls add only Rust-owned typed
  sampler state, and the six reached texture-stage calls add only Rust-owned typed stage-0
  fixed-function state. EndScene, other sampler／state／filter variants, the three rejected
  render states, other texture-stage variants, texture binding, draw, Present, device loss,
  and visible browser output remain unsupported.
