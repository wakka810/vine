# Chrome Runtime Worker vertical slice

## Status

- Status: Verified controlled fixture and private BGI semantic checkpoint
- Date: 2026-07-31
- Browser: current acceptance build is manifest-owned
- Protocol: `vine.browser-runtime-worker`; current version is manifest-owned
- Runtime core: Rust/Wasm, zero imports; current artifact identity is manifest-owned
- CPU mirror: pinned source-v86 adapter in a dedicated Runtime Worker
- Controlled PE32 fixture: `browser-vertical.exe`
- Evidence: `.work/browser-runtime/browser-report.json`
- BGI evidence: `.work/browser-runtime/bgi-browser-report.json`
- BGI benchmark evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`
- Stress evidence: `.work/browser-runtime/browser-stress-report.json`

This checkpoint runs guest CPU execution inside Chrome rather than presenting a
Node-generated event. It crosses PE32 load, Rust-owned memory, v86 execution,
generic API dispatch, asynchronous host completion, nested guest callbacks,
Canvas, Web Audio, IndexedDB, cancellation, watchdog termination, and exact
process exit in one controlled browser application.

It does not claim a general Win32 process, title rendering, a production
renderer or mixer, or full-play steady performance. The fixture APIs are
checkpoint services and are not counted as implemented Windows APIs.

## Runtime path

```text
Chrome main thread
  operational UI / Canvas / AudioContext / IndexedDB / heartbeat
                         |
                         | versioned messages and host completions
                         v
Runtime Worker
  browser event delivery / watchdog / cancellation
                         |
             +-----------+-----------+
             |                       |
             v                       v
Rust/Wasm runtime core          ExecutionEngine adapter
  PE loader                       source-v86 Wasm
  GuestMemory authority           paging / dirty discovery
  RuntimeState                    CPU context / cache effects
  ApiDispatcher
```

`crates/vine-runtime-wasm` owns the immutable PE snapshot, load plan,
materialized bytes, import endpoints, initial stack, `RuntimeState`, decoded API
frames, suspended calls, request identity, scheduler policy, and callback stack.
Its raw buffer ABI keeps JSON metadata separate from `VINERF01`, `VINEDL01`,
and `VINEWB01` page payloads and has no Wasm imports. Fast ABI version 1 handles
hot acknowledgement and CPU writeback receipts as scalar exports, so those
crossings do not copy the whole bridge input or serialize／parse JSON; faults
still return the normal structured metadata. TypeScript does not become
guest byte, pointer, handle, LastError, wait, message, module, or scheduler
authority.

The Worker passes the initial reflection to `ExecutionEngine`, acknowledges
cache effects, and restores a versioned CPU context. Every CPU run has an
explicit caller budget and returns structured `budget-yield`／`trap`／`fault`／
`halt` state. At each slice boundary the engine returns address／owner／bytes for
changed pages, which the Worker atomically reconciles into Rust before
classifying or resuming the guest. The upper loop does not know how changes are
discovered.

Protocol version 79 carries normalized pointer-button events through one
Rust-owned foreground-client message transaction and the same-window `SetFocus`
continuation. Rust validates the input profile, focused foreground window, client
mapping, current-thread queue capacity, logical cursor, message identity, and
scheduler wake before committing any owner. The primary button down becomes
`WM_LBUTTONDOWN`, is published by `PeekMessageA`, validated by `TranslateMessage`,
and dispatched to the registered WndProc; the matching `WM_LBUTTONUP` remains in
the authoritative Rust queue while `SetFocus(hwnd)` is checked as an idempotent
same-current-thread/active/focused/foreground operation. The Worker records
receipts, the queued-release snapshot, and the SetFocus completion but creates no
Windows queue, HWND, cursor, key table, message ID, callback stack, or second
generation. Protocol version 78 remains the pointer-message stop; version 77
remains the integrated action／window-timer regression; versions 76, 75, and 74
retain the earlier recurring, single-delivery, and pointer-polling profiles.

Protocol version 80's `winmm-time-get-time-action-continuation` keeps that Rust-owned
pointer, message, window, and callback state intact while the active WndProc
executes the existing `winmm.time-get-time.v1` handler. `RuntimeProcessState`
returns and advances the shared DWORD tick by its policy increment; the queued
button release and callback depth remain unchanged until the next User32
endpoint. The Worker compares the immutable completion evidence but samples no
browser clock and owns no WinMM or message-queue state.

Protocol version 81's `user32-def-window-proc-a-action-continuation` completes
the reached `DefWindowProcA(WM_LBUTTONDOWN)` inside that same WndProc. An
explicit User32 v34 default-processing profile preserves the v33 SetFocus-only
boundary. Rust validates the live／created／visible same-owner window,
active／focused／foreground relations, pointer-message key mask, and logical cursor-derived
client `lParam`, then returns LRESULT 0 while preserving LastError, guest
memory, window／input／queue state, the queued button release, and callback
depth. No capture authority or transition is introduced. The Worker records
immutable completion evidence only; it neither invokes
DOM default handling nor creates browser capture, HWND, cursor, queue, or User32
state.

Because the same `DispatchMessageA` semantic handler covers both timer and
pointer callbacks, callback-completion evidence distinguishes them by the
Rust-issued completion policy. A timer snapshot is attached only to the
constant-result timer callback, never merely because a `DispatchMessageA`
callback completed; pointer callbacks retain their own result and do not
masquerade as timer delivery.

Protocol version 82's
`user32-peek-message-a-pointer-release-continuation` resumes only after the
pointer WndProc and its outer `DispatchMessageA` frame have retired. Rust then
removes the already queued `WM_LBUTTONUP` with the existing
`PeekMessageA(PM_REMOVE)` transaction, writes one 28-byte `MSG32`, preserves
LastError, and leaves the callback depth at zero. The Worker arms this exact
post-callback occurrence and records immutable completion evidence; it does not
poll a DOM queue, construct the message, or own its HWND, timestamp,
coordinates, removal, or queue lifetime.
Successful Peek completion metadata is classified from the Rust-produced
`MSG32`: only `WM_TIMER` carries a `windowTimer` snapshot, while the reached
`WM_LBUTTONUP` carries the authoritative post-commit `queuedThreadMessages`
snapshot used to prove that the release queue is empty.

Protocol version 83's
`user32-translate-message-pointer-release-continuation` completes the
immediately following `TranslateMessage` against that published
`WM_LBUTTONUP`. Rust reads and validates the 28-byte `MSG32` against the
logical window and pointer-message profile, returns `FALSE`, preserves
LastError, and changes neither guest memory nor the already empty queue. The
Worker records the exact validation-only completion and next
`DispatchMessageA` stop; it does not synthesize text, retain a second message
copy, or acquire Windows queue authority.

Protocol version 84's
`user32-dispatch-message-a-pointer-release-continuation` enters the existing
Rust-owned WndProc callback gateway for that same `WM_LBUTTONUP`. Rust
revalidates the live HWND, owner thread, registered WndProc, pointer profile,
and immutable guest `MSG32`, then prepares the stdcall
`(hwnd, WM_LBUTTONUP, 0, 0x01680280)` frame with a Rust-issued frame identity
and return sentinel. The Worker records the pending API entry and callback
entry, then executes guest instructions only to the first owned
`DefWindowProcA` entry without dispatching it. The release queue remains empty
and the published `MSG32` remains unchanged; no completed
`DispatchMessageA` call is added to the semantic digest at this boundary.
TypeScript does not create a callback stack, message queue, HWND relation, or
release default-processing result.

Protocol version 85's
`user32-def-window-proc-a-pointer-release-continuation` completes the reached
`DefWindowProcA(WM_LBUTTONUP)` and the enclosing callback path. User32 v35
revalidates the live／visible same-owner active／focused／foreground window,
released-button `wParam`, and logical-cursor-derived client `lParam`, then
returns zero without guest-memory, LastError, window, queue, input, or capture
mutation. The Worker observes that completion at callback depth one, runs the
guest return, verifies the Rust-issued frame identity, and records the WndProc
and outer `DispatchMessageA` result as zero after the callback stack reaches
depth zero. It then stops at the following unfiltered `PeekMessageA`; no DOM
default action, browser capture, HWND, message queue, or callback authority is
introduced.

Protocol version 86's
`user32-peek-message-a-post-pointer-release-continuation` completes that exact
post-unwind `PeekMessageA(MSG*, 0, 0, 0, PM_REMOVE)` against the authoritative
empty current-thread queue. Rust returns `FALSE` with no guest write or queue
mutation and preserves LastError plus the prior 28-byte `WM_LBUTTONUP` output.
The Worker records the empty queue snapshot and unchanged bytes at callback depth
zero, then stops at the following `WINMM.dll!timeGetTime`; it neither polls a DOM
queue nor creates a browser-side Windows message or clock authority.

Protocol version 87's
`winmm-time-get-time-post-pointer-release-continuation` resumes from that empty
queue boundary and completes the reached zero-argument `timeGetTime` through the
existing Rust `RuntimeProcessState` clock plan/commit. The returned DWORD is the
previous next tick, the next sample advances by the configured increment of one,
and LastError, guest memory, callback depth zero, and the empty queue remain
unchanged. The Worker records the immutable call and tick relation, then stops at
the following owned `timeGetTime`; it does not sample `performance.now()` or own a
parallel WinMM clock.

Protocol version 88's
`winmm-time-get-time-post-pointer-release-repeat-batch` keeps the same Rust-owned
process-clock contract active through four contiguous reached samples. Each
completion must consume the previous next tick, advance it by one, preserve
stdcall cleanup, LastError, guest memory, callback depth zero, and the empty
queue, and remain ordered on the same runtime thread. The Worker retains the
four immutable call snapshots under a bounded maximum of 16 and stops at the
first heterogeneous owned endpoint, `USER32.dll!GetWindowRect`; it still owns no
clock, message queue, or WinMM state.

Protocol version 89's
`user32-get-window-rect-post-pointer-release-continuation` completes that reached
`GetWindowRect(HWND, RECT*)` against the Rust-owned live logical window. The
User32 handler validates the window and complete output span before atomically
publishing the authoritative outer rectangle `[317, 154, 1603, 903]`, returns
`TRUE`, and preserves LastError and callback depth zero. The Worker retains the
four inherited clock samples plus the immutable window-rectangle call snapshot,
then stops at `USER32.dll!GetCursorPos`; it does not read DOM or browser window
geometry and owns no HWND, RECT, or window lifetime state.

After a guest API completion, Rust may return a `thread-switch` selected by the
authoritative scheduler. The Worker saves the outgoing opaque engine context and
restores the context associated with the selected runtime TID; it does not choose
the lane. The current policy counts 64 completed API boundaries only while another
lane is runnable, resets the count when there is no competitor, and excludes active
callback frames. This keeps switch points independent of Node／Chrome slice history.
Thread return similarly reports `thread-exit`, after which the Worker retires only
the matching context and applies the Rust-produced mapping delta.

Before guest entry, the same Worker applies seven ordered `VINEDL01` batches.
The first real private-heap allocation lazily reserves its 16 MiB backing,
commits one page, and transfers that page; the next batch is dirty-only. The
remaining batches protect read-only／read-write, decommit, recommit, and release
the same backing. Mapping operations and dirty pages are preflighted against a
cloned engine registry before any mapping or byte mutation. Protect invalidation
is applied through generic data-TLB, instruction-fetch, and generated-code
effects.

The private BGI path loads the active Kernel32 schema-v2 policy selected by the
checkpoint manifest. `HeapAlloc` accepts implemented flags `0`／
`HEAP_ZERO_MEMORY` for ordinary nonzero sizes through the 16 MiB per-heap
ceiling and the explicit exact 64 MiB process-wide large variant;
observed allocation sizes are evidence, not Worker or Rust dispatch branches.
Decommit and release both produce a code-bearing unmapped-access stop on a stale
probe. A current full reflection rebuild preserves the CPU context and proves
the receipt-loss recovery path without changing Rust acknowledgement state.

For the large variant, Rust first emits a reserve-only delta for
`[0x40000000,0x44000000)`. A later execution fault is sent to one generic Wasm
page-fault bridge, which may resolve either current-thread stack growth or a
Rust-owned large-heap page. The Worker validates resolution identity,
generation, one 4 KiB RW／NX commit, owner, and initialization bytes before
applying the delta. It records evidence metadata but does not retain a Windows
allocation or materialized-page authority. Rejected faults return no memory
delta and resume the existing structured-stop path.

## Storage boundary

The private BGI path obtains a `vine.writable-profile-snapshot` from the same
origin instead of registering `BGI.gdb` as immutable source. The Worker compares
title fingerprint, profile ID, root, quota, file／directory path and identity,
existence, generation, length, and hash with Rust prepared metadata. Version 3
keeps file bytes out of this JSON bootstrap and adds a separate directory allowlist;
an allowlisted absent directory is not treated as existing.
The Worker acquires a server epoch lease, fetches existing content in bounded binary
ranges, verifies its complete hash, and hydrates one Rust-owned object at a time
before guest entry. Clean reads carry reserved source ID zero and fetch a
generation-bound object range. Dirty reads never cross this adapter; Rust serves its
staged bytes directly.

For a flush or dirty close, Rust emits `storage.profile-commit.v1` metadata while
retaining payload and Win32 state authority. The Worker obtains the payload through
the Wasm bridge, verifies its length and SHA-256, and posts it to the same-origin
SQLite profile-v3 endpoint with an epoch lease and session/request idempotency key.
The server revalidates the lease inside the publication transaction. One
transport/5xx retry uses the same key. Recoverable quota, conflict, lock, and storage
failures become binary Win32 failure completions so Rust retains the dirty handle
and staged bytes for guest retry; malformed identity or control data remains fatal.
An exhausted transport without a definitive response is fatal as an unknown commit
outcome; it is not reported to Rust as a known failed publication.
Every terminal path drains commit tasks and revokes the lease before reporting the
session finished. The Worker updates its read-only profile metadata only after
SQLite and Rust both accept the completion; it does not decide generation, dirty
state, or handle lifetime. The local runner seeds a new database from an existing
distribution `BGI.gdb` only once and asserts the distribution file is unchanged
after Node and Chrome execution.

For `storage.profile-directory-create.v1`, Rust retains the directory identity,
base generation, existence decision, pending request, and Win32 result authority.
The Worker forwards only prepared profile/session/request/directory metadata. SQLite
revalidates the lease, generation, and idempotency key in one transaction; Rust
publishes existence only after validating the durable completion. Storage failure,
stale or malformed completion, cancellation, and terminal cleanup cannot leave a
TypeScript-owned Windows directory state. Private target checks use a run-local
database so neither the shared profile nor the distribution directory is mutated.

Protocol version 12 adds writable profile configuration and commit completion
metadata. The current private checkpoint reads the profile but does not reach a
guest write. A controlled Chrome write/restart/load fixture and the primary title
save scenario remain separate acceptance work.

Protocol version 13 adds a normal `checkpoint-complete` terminal with immutable
capability identity, required and observed completion counts, last completion
sequence/instruction, next owned endpoint, and final memory digest. The Worker
captures it only after the required completions and the final CPU writeback, then
uses the common clean stop path. It is distinct from `firstUnsupported`; neither
field may be populated as a substitute for the other.

Protocol version 14 adds `runtime.timer-delay.v1` pacing and page lifecycle
messages. Rust supplies the versioned request, registration, timer, logical
target, clock generation, arm generation, and outcome snapshot; the Worker owns
only a cancellable delay resource and returns ack `1`. Freeze preserves the
remaining delay, resume rearms it, lifecycle/request generations reject stale
callbacks, and catch-up is bounded to one completion per task. During this
service only, the Worker holds guest CPU execution until completion and consumes
the Rust-authorized owner lane once, so browser timing cannot select a different
guest order. Other host services retain their existing multi-lane behavior.

Protocol version 15 makes the private-BGI capability profile explicit in the
session configuration. The profile selects which evidence checkpoint the Worker
arms (`kernel32-wait-for-single-object-ex` or
`kernel32-enter-critical-section`); the compatibility policy ID remains a
dependency policy and cannot select a stop by itself. The profile is validated
only for the private BGI workload, while Rust continues to own guest bytes,
Windows object identity, synchronization state, and lifecycle. The current
critical-section profile requires three balanced acquisition/release roundtrips
and reports the next owned `SetWaitableTimer` endpoint through the same
checkpoint-complete protocol.

Protocol version 16 adds the `primary-main-loop-progression` profile and a
typed `threadSelections` completion field. The profile requires two
Rust-authorized API-boundary selections, validates the `8192 -> 8193 -> 8192`
round trip, and reports the next owned endpoint
`KERNEL32.dll!LeaveCriticalSection`. Selection metadata is evidence of the
Rust scheduler transaction; it does not make browser timing or TypeScript a
second thread-state authority.

Protocol version 17 adds the typed main-thread `LeaveCriticalSection`
completion. Protocol version 18 adds
`kernel32-enter-critical-section-continuation`, verifies that the immediately
following acquisition uses the same critical-section address and selected
main thread, and carries `acquisitionThreadId` in the terminal completion.
Protocol version 19 adds
`kernel32-leave-critical-section-continuation`, verifies that the immediately
following release uses the same address and selected main thread, and carries
`releaseThreadId` in the terminal completion. These versions observe
Rust-owned synchronization transactions; they do not move recursion,
ownership, guest bytes, or lifecycle state into TypeScript.

Protocol version 20 adds the `shell32-drag-accept-files` profile and a compact
typed `dragAcceptFiles` observation, and the `kernel32-archive-source-read`
profile reuses the same protocol with the primary boot-archive snapshot mount. The Worker verifies one reached stdcall,
EAX preservation, LastError, zero guest writes, created logical-window
lifecycle, and the effective `WS_EX_ACCEPTFILES` bit. It records that no
browser resource or `WM_DROPFILES` path was created. The style bit and window
generation remain private `RuntimeUser32` authority.

Protocol version 24 inherits the version 23 file-attribute snapshot and adds
writable-profile version 3 directory metadata plus the typed
`storage.profile-directory-create.v1` request／completion. It carries opaque Rust
request and directory IDs, base／committed generations, and Win32 error, without
carrying a guest pointer, Win32 handle, or host path.

Timer pacing, immutable source reads, writable profile reads, and writable
commits are synchronous-stream services: the issuing guest thread parks on a
request-and-thread-validated completion barrier, and the Worker delivers
host completions to Rust in request-issue order, deferring a younger
completion while an older request is in flight. This keeps the Rust-visible
scheduling stream identical to the synchronous Node oracle when wall-clock
fetch latency and pacing deadlines would otherwise race. The controlled
fixture's browser host services keep the existing
execute-another-thread offer path.

The browser delay adapter now uses one `MessageChannel`-driven deadline queue
instead of creating a clamped timeout task for every one-millisecond pacing
request. Synchronous API-boundary yields may be coalesced only to the bounded
64-boundary scheduler quantum. Pending host waits, timer completions,
cancellation, and watchdog checks retain immediate task boundaries, so this is
a host scheduling optimization rather than a change to guest-visible time or
thread selection.

Protocol version 73 adds the bounded steady-state keyboard-ingress evidence
profile. Two normalized keyboard messages may queue before preparation, but the
Worker does not publish either as initial Rust state. It commits key-down only
after the profile's completed no-input cycle boundary, validates the first
later Rust-owned consuming `GetAsyncKeyState` completion, and commits key-up at
the immediately following boundary. Each bridge completion must return the
reserved input sequence, event kind, virtual key, focus fact, Rust generation,
and affected-key snapshot expected by the profile before guest execution may
continue. This boundary deferral is evidence orchestration only: TypeScript
retains no current-down／pressed bit, foreground relation, or independent input
generation, and ordinary post-prepare DOM events still enter the same bounded
Rust authority path.

## Execution engine contract

Protocol version 9 binds the production loop to `ExecutionEngine`, not to v86
exports or numeric stop reasons. The interface owns budgeted execution,
structured stops, versioned opaque CPU-context creation／save／restore, guest mapping
operations, owner-tagged changed-page batches, CPU resume／callback entry, and
data-TLB／instruction-fetch／generated-code effects. `runtime-worker.ts` contains
no PTE address, v86 private offset, `full_clear_tlb`, or v86 reason constant.

The current implementation remains the source-v86 adapter. Its non-PAE tables,
PTE Dirty scan, TLB flush, and generated-code hook are confined to
`v86-mirror.ts`. Adapter ABI version `1` exposes memory-size setup, the
instruction counter, and a 4 GiB page execute bitmap without exposing v86
private offsets to the Worker or runtime runners. The pinned profile is
[`fixtures/execution-engines/v86-browser-v1.json`](../fixtures/execution-engines/v86-browser-v1.json):
source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f` and CPU-context ABI version `1`／
640 bytes are common. Its artifact allowlist contains exact host builds: Windows
GNU is 1,857,800 bytes／SHA-256
`99a0948c222a6ac2f21984798c9e9213dec9e43b95dff6fc5c8acf1f4dc7954f`;
Linux GNU is 2,116,017 bytes／SHA-256
`21b0fb4f63e9604a66e6522839c9744a4ae17f9ead1c2e201ff2e46fccff20c4`.
Build preparation matches the actual Wasm to one allowlist entry and records
that selected identity, rather than the whole allowlist, in the build manifest.
Worker startup checks artifact bytes／SHA-256 before compilation and exported
adapter／context ABI versions and context size before guest initialization.

Rust remains the execute-permission authority. The mirror resets and rebuilds
the adapter bitmap from reflected mappings, then updates it for every
commit／protect／decommit／release operation. The interpreter validates the
opcode and each additional decode byte when it is consumed. A one-byte
instruction at the end of an executable page therefore completes, while the
next opcode on an NX page and an immediate crossing into that page stop with
reason `3`, PFEC-equivalent `0x15`, and CR2 set to the denied byte. The crossing
case restores the instruction EIP before exposing the structured stop and does
not update its destination register. This is a host-visible execution stop; it
does not yet claim general guest SEH delivery for execute faults.

Rust prepared metadata also supplies the semantic committed-page limit. The
v86 mirror starts with a 64 KiB physical pool and grows geometrically only after
a complete reflection／host-delta preflight. Growth reinstantiates the cached
adapter module and preserves CPU context、mappings、bytes、protection、PTE Dirty
state; Rust remains byte authority. The BGI limit is 16,384 guest pages
（64 MiB）and the corresponding page-directory／all-page-table／VGA-hole-inclusive
mirror ceiling is 71,442,432 bytes. Decommit／release zero and free data pages,
empty page tables are reclaimed, and recommit reuses zeroed pages. Capacity
failure is the mutation-free typed code `v86-physical-pool-exhausted`.

The verified adapter module is cached once per Worker; each runtime session
still creates a fresh Wasm instance, CPU context, mapping registry, and guest
memory. This placement removes repeated adapter fetch／hash／compile backing
storage without sharing guest state. `npm run check:execution-engine-contract:prepared`
uses the actual Wasm to fix CPU context, structured yield／trap, all five mapping
operations, changed-page owner/address, cache effects, and revision／ABI／hash
mismatch rejection. The same contract expands a live mirror beyond 2 MiB,
resumes its CPU context, verifies release／reuse, and rejects over-limit mappings
without changing the mirror.

## Execution state contract

Protocol version 9 represents execution through an explicit state machine:
`ready`, `running`, `budget-yield`, `memory-fault`, `api-trap`, `thread-switch`,
`thread-exit`, `waiting-host`, `host-completion`, `guest-callback`,
`callback-return`, `stopping`, and `stopped`. Prepared Rust metadata supplies
the owning process and registered thread bootstraps. TypeScript combines the
selected lane with Worker lifecycle state; it does not replace Rust scheduler
authority.

Every transition is checked against a closed adjacency table before state,
sequence, or last-transition metadata is changed. API traps remember whether a
synchronous completion resumes `ready` or a nested `guest-callback`. Permitted
async starts enter a table whose records contain request ID, thread ID, service,
cancel state, and `resume-ready`／`resume-callback` policy. Completion must match
both identities. Wrong-thread, duplicate, cancelled-late, unknown, and reused
request completions are rejected without completing another record.

The Worker stores one opaque CPU context per live thread. A blocking host
request saves its owner context, asks Rust for the next executable lane, and
restores only the selected context. Host completion targets the request owner
even while another lane is selected; register resume is applied to that saved
context before Rust can schedule it again. The controlled fixture proves this
with a second guest EIP／ESP that writes a marker and exits while the initial
thread remains in `WaitingHost`.

Cancellation retires every active delivery record before final reporting.
Success, capability checkpoint, cancellation, structured stop, watchdog, and fault all pass through
`stopping -> stopped`; terminal reports require zero pending request records.
Only a watchdog report may contain diagnostic context, which records the cause
thread, current request when one exists, and the last transition observed
before termination. Pending host calls and callback stacks are keyed by owner
thread; neither is represented by a global single slot.

Protocol v7 also makes diagnostic retention explicit. Every private BGI session
feeds normalized observable calls to one ordered digest. Its next-sequence and
bounded pending map reject duplicate or conflicting sequence identity without a
second completed-sequence collection. Canonical top-level fields stream directly
into reusable SHA-256 state; ASCII fragments bypass `TextEncoder`, while
non-ASCII uses the bounded UTF-8 fallback. Normal semantic gates and repeated
benchmarks select summary evidence: they retain the exact call count, algorithm,
digest, source completions, memory digest, exact terminal evidence, and teardown checks
without publishing detailed call objects. Full evidence remains an explicit
diagnostic mode. Summary evidence is rejected for other workloads.

## Fixture contract

The reproducible PE32/i386 fixture imports three symbols from
`vine_browser_fixture.dll`:

1. `fixture_deferred_add(19, 23)` suspends the guest, sends one coarse host
   request, and resumes with `42`.
2. `fixture_call_callback` enters an outer guest callback. That callback calls
   the same API for an inner callback, producing strict depth `2` and result
   `44` before both frames unwind.
3. `fixture_commit_outputs(0xff0000ff, 880, 0x5a)` suspends again. The main
   thread presents an RGBA8 test pixel, schedules an 880 Hz Web Audio pulse,
   writes byte `0x5a` to IndexedDB, reads it back, and returns success.

The fixture returns through the exact Rust-owned process-entry sentinel with
exit code `0`. The native Windows self-oracle also exits `0`.

The executable also exports a controlled second-thread entry and marker. Rust
registers a distinct thread stack and scheduler lane, while the Worker creates
an independent opaque CPU context. When the initial thread begins the first
deferred request, Rust selects the second lane. That guest code writes its
marker, returns through the thread sentinel with its own exit code, and is
retired before the initial host completion restores the first context.

Host operations never expose a Promise to the guest stack. `RuntimeState`
enters `WaitingHost`, records a request ID and resume state, and returns to the
Worker. Protocol version 9 transports the lossless decimal request ID, owning
thread ID, bounded copied scalar payload, and matching Rust `waiting-host`
authorization. TypeScript retains a bounded delivery record, not guest stack
state. A typed
completion restores the Rust scheduler state and completes the saved generic
dispatcher frame before CPU registers are resumed. The fixture retains no guest
pointer across the asynchronous boundary.

## Browser result

The clean-profile Chrome gate requires all of the following semantic evidence:

- both registered guest threads execute with distinct CPU contexts;
- the second thread runs and exits while the initial thread owns a pending host request;
- the initial context resumes and completes both deferred browser services and nested callbacks;
- changed guest pages and mapping operations reconcile into Rust authority;
- Canvas, Web Audio, and IndexedDB outputs read back correctly;
- success, cancellation, watchdog, and fault paths finish in `stopped` with no pending delivery,
  callback, or Rust host work.

The private BGI gate uses the same Worker and Rust/Wasm authority. Node generates only a semantic
oracle; Chrome must match ordered completions, caller-visible results, final guest bytes, and the
same structured fault or capability completion. Protocol v12 carries immutable snapshots for the Rust-owned NULL-source
memory DC, selected font and DIB, DIB content generation, bounded `TextOutA` raster completion,
restore selections, and deletion notifications for `DeleteObject`／`DeleteDC`. The Worker validates
these as GDI metadata while the changed pixel pages still cross the generic GuestMemory transport.
These messages create no DOM, Canvas, native HDC, browser font, Canvas text, or browser bitmap state;
the Worker remains an observer until a later renderer adapter consumes an explicit Rust resource.

The reached D3D9 path now includes one Rust-owned logical device; three
logical textures covering the exact system-memory／default／dynamic-default initial band; the first
texture's level-zero surface; a typed surface HDC; one bounded graphics fill command; and their
COM／GDI lifetime relations. It also creates one Rust-owned 112-byte logical vertex buffer, exposes
its guest backing through the exact full-range Lock, consumes the guest write, and returns the backing
to read-only on Unlock. The device then consumes the exact four fixed-function `SetRenderState`
settings through Rust-owned typed state and completes the bounded `GetRasterStatus` output before
sampling the Rust-owned high-resolution performance frequency and deterministic counter. The
reached loop repeatedly consumes raster status and counter values before continuing to the next
User32 contract. Protocol v38 carries the bounded `IDirect3DTexture9::AddDirtyRect`,
`IDirect3DDevice9::UpdateTexture`, and `IDirect3DDevice9::BeginScene` completions. Its immutable
snapshots show the Rust-owned source／destination texture and COM relations, descriptor roles,
copied logical fill, source dirty-region consumption, exact generations, and the device-local
inactive-to-active scene transition. It also carries the two reached
`IDirect3DDevice9::SetSamplerState` completions, immutable sparse sampler-state snapshots, three
explicit post-sampler render-state rejections, and the six reached
`IDirect3DDevice9::SetTextureStageState` completions and the reached stage-0
`IDirect3DDevice9::SetTexture` completion, the reached
`IDirect3DDevice9::SetFVF(0x144)` completion, and the reached
`IDirect3DDevice9::SetStreamSource(0,vertexBuffer,0,28)` completion. The Worker validates the cumulative typed
color／alpha state, the Rust-owned non-owning relation to logical texture 8, and the
typed `xyz-rhw-diffuse-texture1` format plus the Rust-owned non-owning stream-zero
relation to logical vertex buffer 3 before stopping at
`IDirect3DDevice9::DrawPrimitive`; it stores no independent texture content, dirty region,
scene state, sampler state, texture-stage state, texture binding, vertex format, stream binding, COM reference,
or renderer resource.
The Worker observes completions and mirrors checked guest bytes, but owns neither
identity, lock state, protection, resource content, render state, nor a second raster／performance
clock. It does not substitute `performance.now()` or browser scheduling for guest-visible time.
The reached DirectSound path also creates the primary／secondary logical buffers, publishes one
88,200-byte Rust-owned secondary sample backing on full-range Lock, reconciles the caller write,
returns the range to read-only on Unlock, and commits looping secondary Play before stopping at
`SetTimer`. The Worker transports mapping operations and bytes only; it creates no browser audio
resource and owns no DirectSound identity, content generation, or playback state. No texture／surface
pixel, browser renderer, Web Audio node, or audible output is created. A separate repeated-session
benchmark evaluates the versioned latency, heartbeat, heap, backing-storage, linear-memory, and
bridge-crossing thresholds.

A benchmark threshold failure does not invalidate a passing semantic-equivalence run, but it
blocks the next primary checkpoint. Exact failure, host-load evidence, protected thresholds, and
resume condition are recorded in the current manifest; the Worker document does not duplicate
those changing measurements.

Current measurements, artifact identities, exact stop data, and report hashes are recorded only
in [current-checkpoint.json](current-checkpoint.json). Detailed private-run non-claims remain in
[bgi-browser-worker.md](bgi-browser-worker.md), and benchmark methodology remains in
[bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Reproduction

```powershell
npm run check:runtime:browser-worker
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
npm run serve:runtime:browser-worker
npm run check:runtime:browser-worker:stress
```

The first command generates the PE fixture, prepares the pinned adapter, builds
the Rust core for `wasm32-unknown-unknown`, type-checks and bundles the browser
application, then runs success, pending-cancel, and watchdog probes in a clean
Chrome profile. The serve command exposes the operational UI on loopback. The
stress command adds the 30-minute repeated lifecycle run.

## Remaining gates

- full-play peak committed pages and memory-pressure ceiling beyond the bounded 64 MiB guest limit
- general process, thread, DLL, heap, wait, message, and object lifecycle
- long-idle heap-capacity plateau and full-play growth slope
- isolated rerun of any active private-workload performance blocker
- general wait fairness and the next title output path

The Step 8 synthetic matrix, background-tab probe, AudioWorklet deadline probe,
and active steady-memory measurement passed their version 1 thresholds. See
[v86-browser-benchmark.md](v86-browser-benchmark.md).
