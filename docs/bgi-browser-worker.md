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

The post-Present continuation profile is `d3d9-post-present-continuation`, uses
protocol `vine.browser-runtime-worker.v45`, and emits browser BGI report schema
version 13.
It inherits the immutable boot-archive snapshot,
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
Protocol version 36 carries that completion and the immutable Rust-owned device
binding snapshot. Rust resolves the live device and static DEFAULT texture on
the same logical device, commits a non-owning stage-0 relation, and leaves COM
reference count 5 unchanged. Node and Chrome agree on sequence 127,371,
logical device 3／COM 21, texture 8／COM 24, device generation 54, zero guest
writes, no browser renderer resource, and the cumulative semantic and terminal
memory digests. Both stop seven instructions later at
`IDirect3DDevice9::SetFVF(0x00000144)` slot 89, endpoint `0x010115C0`, return
address `0x1005B291`. Protocol version 37 carries the resulting typed
`xyz-rhw-diffuse-texture1` snapshot. Rust commits it on logical device 3／COM 21
without changing reference count 5, writing guest memory, or creating a renderer
resource. Node and Chrome agree on sequence 127,372, instruction 28,761,298,
device generation 55, and the cumulative semantic digest. Both next stop at
instruction 28,761,315 on `IDirect3DDevice9::SetStreamSource(0,0x01050004,0,28)`
slot 100, endpoint `0x01011670`, return address `0x1005B3EC`.
Protocol version 38 carries that completion and the immutable Rust-owned stream
snapshot. Rust resolves logical device 3／COM 21 and the live same-device logical
vertex buffer 3, then commits a non-owning stream-0 relation at offset 0／stride
28 without changing reference count 5, writing guest memory, or creating a
renderer resource. Node and Chrome agree on sequence 127,373, instruction
28,761,315, device generation 56, and the cumulative semantic and terminal
memory digests. Both next stop nine instructions later at instruction 28,761,324
on `IDirect3DDevice9::DrawPrimitive(5,0,2)` slot 81, endpoint `0x01011540`,
return address `0x1005B402`.
Protocol version 39 carries the resulting immutable Rust-owned draw submission.
Rust validates the active scene and exact reached render／sampler／texture-stage／
texture／FVF／stream state, reads the checked 112-byte vertex range from
authoritative `GuestMemory`, and commits one owning snapshot to logical device
3／COM 21. The snapshot fixes device generation 57, back-buffer content
generation 2, submission／scene sequence 1, vertex buffer 3, texture 8, and
vertex SHA-256
`405bea74541d2aece445d0dac693da55fb404e777c91ff81d87d4c6272581362`.
Node and Chrome agree on sequence 127,374 at instruction 28,761,324, reference
count 5, zero guest writes, and no renderer resource or visible pixels. Both
next stop eight instructions later at `IDirect3DDevice9::EndScene` slot 42,
endpoint `0x010112D0`, return address `0x1005B43B`, with the same semantic and
terminal-memory digests.

Protocol version 40 carries the matching Rust-owned scene close. Device slot 42
validates the live active scene and commits only the active-to-inactive
lifecycle transition; back-buffer content generation 2 and the immutable draw
submission remain intact. Node and Chrome agree on sequence 127,375,
instruction 28,761,332, device generation 58, reference count 5, zero guest
writes, no active scene, and no renderer resource or visible pixels. The
versioned capability completes at instruction 28,761,389 before the next owned
COM endpoint `0x01011160`, identified from the stable D3D9 vtable as
`IDirect3DDevice9::GetRasterStatus` slot 19. This explicit completion prevents
the following ordinary wait loop from being misclassified as either an
unsupported endpoint or an unbounded checkpoint run.

Protocol version 41 carries the immediately following slot-19
`GetRasterStatus(0, 0x006FF280)` sample. Rust preflights the complete eight-byte
`D3DRASTER_STATUS` write, returns `{ InVBlank: FALSE, ScanLine: 1 }`, preserves
LastError and COM reference count 5, and does not mutate logical graphics state.
Node and Chrome agree on sequence 127,376 at instruction 28,761,389 and on the
cumulative 784 raster-status completions. The capability completes at instruction
28,761,446 before runtime export 118, `kernel32.dll!SetWaitableTimer` at
`0x7F000750`. Neither engine imports host／browser scanout state or creates a
renderer resource or visible pixels.

Protocol version 42 continues through the immediately following timer-1 pacing
cycle. Sequence 127,377 arms the existing manual-reset waitable timer for a
relative 5,000 × 100 ns interval at instruction 28,761,446. Sequence 127,378
then completes the matching finite alertable `WaitForSingleObjectEx` at
instruction 28,761,452 through `runtime.timer-delay.v1`; Rust timer clock
generation 9,187 and arm generation 1 authorize the completion. Node and Chrome
agree on the ordered calls, final memory content, and one requested／completed
host delay, while the Rust logical clock remains authoritative and no host wall
clock value is imported. The checkpoint stops at instruction 28,761,476 before
the next slot-19 `IDirect3DDevice9::GetRasterStatus` query.

That version-42 result was a valid timer transaction but not a valid progress
policy. A bounded Node diagnostic repeated the same raster／timer-arm／timer-wait
cycle 1,024 times with unchanged terminal memory. Focused native sampling added
nonblank lines 189 and 834 to the existing 1200／VBlank-0 observations, while
immutable caller disassembly established the actual exit rule: RVA `0x5B730`
returns when the clamped scanline wraps or reaches `ceil(max / 2)`. The fixed
sample 1 can do neither.

Protocol version 43 therefore carries D3D9 compatibility policy v29 and its
stateless virtual-display sample `{ InVBlank: FALSE, ScanLine: 1079 }`. It has no
callsite, occurrence, title-hash, host-scanout, or browser-scanout branch. Node
and Chrome complete sequence 127,376 at instruction 28,761,389, agree on semantic
digest `0d078f5f58c953ff8d8c952a33497e308c5e7632c6c03437c21ca04a85f711d6`
and memory digest
`c8e437febb7fd0b6b79ad5822f8185cc336e16b532c980aa39815479590f204e`,
then stop at instruction 28,761,426 before COM slot 17
`IDirect3DDevice9::Present` at `0x01011140`. No post-scene timer cycle, renderer
resource, or visible pixel is claimed by this checkpoint.

Protocol version 44 adds compatibility policy v30 and the bounded slot-17
`Present(NULL,NULL,NULL,NULL)` vertical slice. Rust publishes the closed
back-buffer content as immutable presented-frame sequence 1, advances logical
device generation 59, and renews an empty discard back buffer at content
generation 3. The Worker receives a transferred `OffscreenCanvas`, creates one
`browser-resource:1` WebGL2 record with an explicit non-owning binding to COM
object 21. Context, shaders, exact GPU storage, and bounded CPU staging are
preflighted before guest execution; the record is published only after the
immutable Rust snapshot and fixed-function state pass validation. The adapter
then materializes synchronously before the guest resumes, uploads 8,388,608
X8R8G8B8 texture bytes and 112 vertex bytes, draws one triangle strip into a
1280x720 frame, reads center RGBA
`[0,0,0,255]`, and observes WebGL `NO_ERROR`. Node creates no browser resource
but records the same Windows-visible Present and discard semantics. Both agree
on semantic digest
`792c9f0f6f620fdce04445f5d6138b317656bed87a392196fae2ed0d09bc3398`
and memory digest
`bff1bcdfd09d14143797630f57222aaf9a94ca08e29593a85010690da7e804b9`,
then stop at instruction 28,761,433 before `WINMM.dll!timeGetTime` endpoint 8.
The browser resource table, WebGL objects, and canvas are TypeScript authority;
the device, COM identity, back-buffer, frame sequence, and discard lifecycle
remain Rust authority. The black frame proves browser publication, not readable
title rendering or a general swap-chain／device-loss implementation.

Protocol version 45 resumes the guest after that synchronous publication. The
pending `WM_PAINT` callback calls the already-implemented zero-argument
`timeGetTime`, returns frame 30 with result zero, and leaves no active callback.
The next `GetForegroundWindow` reads the existing Rust-owned foreground relation,
returns logical HWND `0x48010001`, preserves LastError, writes no guest bytes, and
does not create or consult a browser window resource. Node and Chrome agree on
the ordered continuation at sequences 127,378 and 127,379, semantic digest
`ac1eee34c5e1f3e4746600db145ac70a2d0a023949a9094729572c7a04c41617`,
and terminal memory digest
`6ba9e12a6946dd09a1d673d76f7dab2a11436ad1f7fe5989d2c43a6698b82321`.
The profile then stops before `GetAsyncKeyState(VK_CONTROL)` at instruction
29,325,480. No keyboard state has been consumed yet; browser event ownership,
Rust input transition semantics, focus gating, and the high-bit／low-bit return
contract belong to the following input gateway.

The held-key input continuation profile
`user32-async-key-state-input-continuation` keeps browser runtime transport
`vine.browser-runtime-worker.v45`, advances the main／Worker message protocol to
version 47, and emits browser BGI report schema version 15. The main adapter
queues one focused `ControlLeft` key-down before runtime preparation; the Worker
forwards exactly one normalized receipt through Wasm to the Rust-owned keyboard
state. The first generic `VK_CONTROL` query returns signed `0xFFFF8001`. A later
query with no additional receipt returns `0xFFFF8000`, proving that the queried
pressed bit is consumed once while current-down remains set. Node and Chrome
also agree on three additional immutable source-object-3 reads. Those reads are
ordered continuation evidence, not evidence that input caused asset loading or
a visible gameplay response; no zero-input counterfactual is part of this
profile. Exact occurrence, instruction, digest, report identity, and next-stop
values remain in the current manifest.

The current zero-input startup continuation profile
`user32-async-key-state-initial-scan-continuation` advances browser runtime
transport to `vine.browser-runtime-worker.v51`, the main／Worker message
protocol to version 51, and browser BGI report schema to version 19. The main
adapter transfers the same `OffscreenCanvas` but queues no keyboard input. Rust
extends one growable private heap with a second bounded 16 MiB ordinary backing
and remains the sole authority for both segments. Execution reaches two
same-consumer `GetWindowRect`／`GetCursorPos` pairs. Node and Chrome agree on the
final logical outer window rectangle `[317,154,1603,903]` and the deterministic
Rust-owned screen cursor `[960,540]`; neither value is imported from browser
geometry or a host pointer.

After 51 leading zero `VK_CONTROL` polls, both engines complete the exact
native-derived 38-key scan with no input receipt and a zero signed SHORT for
every query. The continuation then completes the existing deterministic
`WINMM.dll!timeGetTime` and `USER32.dll!SetRect` contracts at sequences 148,196
and 148,197. The latter atomically publishes `[0,0,1280,720]`. Node and Chrome
complete the instruction 147,769,351 `IDirect3DTexture9::LockRect` call with
level 0, a writable `D3DLOCKED_RECT`, the full rectangle, and flags `0x8000`.
Rust lazily publishes the 8 MiB GuestMemory backing with pitch 8,192; the later
`UnlockRect`, bounded partial `UpdateTexture`, second scene／draw／Present cycle,
and browser byte materialization also complete. Both engines then complete the
mutation-free live-device `IDirect3DDevice9::TestCooperativeLevel` first query at
sequence 148,293／instruction 148,744,443 with `D3D_OK`, unchanged reference
count 5, preserved LastError, and zero guest writes. Ordered call semantics,
all 64 immutable source reads, terminal memory content, cursor／rectangle
writes, scan order, render completion counts, and the repeated query results are
equivalent. The polling loop then completes three current-thread unfiltered
empty-queue `PeekMessageA` calls and three `IsIconic` calls against logical HWND
`0x48010001`; each returns false, preserves LastError, and writes no guest
bytes. Both engines next stop at instruction 148,787,159 on a read page fault
at `0x500FE000` while the API-created thread reaches its stack guard.
Generations, report hashes, timing, and exact artifact identities remain
manifest-owned evidence rather than guest semantics.

Protocol version 119／BGI report schema 85 extends the active primary replay
through the fourth-scene `Present`. Node and Chrome retain the sequence 282,867
post-scene raster prerequisite, then agree on sequence 282,868 at instruction
392,354,183, device `0x01010004`, `D3D_OK`, reference count 5, logical device
generation 74, presented frame 4／content generation 11, and discarded empty
back-buffer generation 12. Chrome reuses `browser-resource:1`, synchronously
uploads 8,388,608 texture bytes plus 112 vertex bytes, observes WebGL error 0 and
center pixel `[0,0,0,255]`, and completes publication before guest resume. Both
complete at instruction 392,354,190 before `WINMM.dll!timeGetTime` at
`0x7F000070`. The black receipt proves publication, not readable title pixels;
resize and context／device loss recovery remain unclaimed.

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

- The checkpoint is not readable title rendering, general input, audio,
  complete save/load, or full-play acceptance. It proves one bounded keyboard
  ingress and held-key User32 query path, one zero-input startup scan, logical
  rectangle／initial-cursor reads, and one bounded durable directory mutation,
  not general gameplay input, pointer movement, or writable persistence.
- The controlled second thread does not claim broad Win32 thread, wait,
  synchronization, APC, SEH, or scheduling fairness compatibility.
- `Sleep` intervals greater than zero, no-competitor timing, host wall-clock delay,
  priority-weighted selection, and preemptive scheduling are not implemented.
- The versioned workload gate is not a full-play or long-idle soak.
- Unsupported behavior remains an explicit structured stop. A normal capability
  completion requires a versioned count and next-endpoint contract; reaching a
  later API by itself does not establish subsystem completeness.
- The bounded `IDirect3DTexture9::LockRect`／`UnlockRect` path claims only the
  reached level-zero X8R8G8B8 SYSTEMMEM rectangle and
  `D3DLOCK_NO_DIRTY_UPDATE`. Other levels, formats, rectangles, flags, concurrent
  locks, and renderer ownership of Windows texture state remain unsupported.
- `DragAcceptFiles` changes only the logical acceptance bit. Browser drag
  events, `HDROP`, `WM_DROPFILES`, file permission, and VFS import remain
  unimplemented.
- The reached paint leaf does not claim general invalidation, overlapping or nested paint,
  background erase, clipping, queued paint dispatch, browser presentation, or arbitrary
  HDC operations.
- The reached Clear itself does not claim partial rectangles, depth／stencil
  clears, or pixel storage. Upload, draw, and publication occur only through
  the separately bounded first-Present snapshot.
- The reached AddDirtyRect stores only a bounded Rust-owned dirty region. The
  first UpdateTexture copies Rust-owned logical commands; the later exact
  partial update publishes a generation-checked Rust GuestMemory pixel source.
  Arbitrary partial histories and the native dynamic-destination variant remain
  unsupported. The Worker performs only a bounded rendering-time byte copy and
  owns no Windows texture or lock state. The initial and reached post-partial BeginScene transitions create
  only Rust-owned logical scene state, the initial and fourth-scene sampler pairs add only Rust-owned typed
  sampler state, and the initial plus fourth-scene six-call texture-stage bands add only Rust-owned typed stage-0
  fixed-function state, the initial plus fourth-scene SetTexture calls use the same non-owning Rust-owned binding, and the
  initial plus fourth-scene SetFVF calls use the same Rust-owned typed vertex format. The initial plus fourth-scene
  SetStreamSource calls use the same exact non-owning stream-0／offset-0／stride-28 relation, the initial plus fourth-scene
  DrawPrimitive calls append bounded owning immutable Rust draw submissions, and the initial plus fourth-scene
  EndScene calls close their matching Rust-owned scenes while preserving those submissions. Other scene variants,
  other sampler／state／filter variants, the three rejected render states, other
  texture-stage／binding variants, other vertex formats, other stream bindings,
  NULL stream unbind, other draws, partial／redirected／dirty-region Present,
  additional／unbounded subsequent frames, resize, device／context loss and recovery, and readable
  title output remain unsupported.
