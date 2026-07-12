# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The current checkpoint is `bgi-seventeenth-win32-call`. Rust owns and materializes
the fingerprinted BGI image, applies 18,199 `HIGHLOW` relocations, binds all
330 imports to deterministic runtime-owned `UD2` thunks, installs four ordered
dynamic-export endpoints, and reflects 389 sealed pages into source-built v86.
BGI executes 19 instructions from entry
`0x100A383E` and reaches exact thunk ID `104`,
`KERNEL32.dll!GetSystemTimeAsFileTime`. In this cumulative mode, a controlled
clock sample is written through a generation `28` stack-page `MemoryDelta`;
the checked stdcall return
then executes four more instructions and stops at thunk ID `103`,
`KERNEL32.dll!GetCurrentProcessId`. A second checked stdcall returns controlled
process ID `4096`; three more instructions stop at thunk ID `149`,
`KERNEL32.dll!GetCurrentThreadId`. A third checked stdcall returns controlled
thread ID `8192`; three more instructions stop at thunk ID `13`,
`KERNEL32.dll!GetTickCount`. A fourth checked stdcall returns controlled tick
count `3,600,000`; five more instructions stop at thunk ID `157`,
`KERNEL32.dll!QueryPerformanceCounter`. A fifth checked stdcall writes counter
`12,345,678,901` through generation `29`, returns BOOL success, and runs 44
more instructions to thunk ID `83`, `KERNEL32.dll!GetStartupInfoW`. A sixth
checked VOID stdcall writes controlled 68-byte x86 `STARTUPINFOW` profile
`startupinfow-v68-zero-optional-fields` through generation `30`, preserves
EAX, and runs nine more instructions to thunk ID `84`,
`KERNEL32.dll!HeapSetInformation`. A seventh checked stdcall accepts only
`HeapSetInformation(NULL, 1, NULL, 0)`, records the controlled process heap
policy, returns BOOL success without changing generation `30`, and runs 21
more instructions to thunk ID `102`, `KERNEL32.dll!HeapCreate`. An eighth
checked stdcall validates
`HeapCreate(0, 0x1000, 0)`, creates Rust-owned metadata for logical private
heap handle `0x68010001` without copying the native host pointer, and runs 14
more instructions to thunk ID `72`, `KERNEL32.dll!GetModuleHandleW`. A ninth
checked stdcall reads bounded
UTF-16 `KERNEL32.DLL`, returns Rust-owned logical loaded-module handle
`0x4D010001` without copying the native HMODULE, and runs nine more
instructions to thunk ID `150`, `KERNEL32.dll!GetProcAddress`. A tenth checked
stdcall validates logical KERNEL32
handle `0x4D010001`, bounded ASCII `FlsAlloc`, and the register-indirect IAT
call sequence; it returns identity-preserving guest endpoint `0x7F020000`
without copying the host function address. Five more instructions reach thunk
ID `150` again for `FlsGetValue`. An eleventh checked stdcall preserves the
first endpoint at BGI slot `0x100FEEBC`, validates persistent ESI call
provenance and bounded ASCII `FlsGetValue`, and returns distinct endpoint
`0x7F020010`. BGI stores it at `0x100FEEC0`; five instructions later the same
thunk is reached for `FlsSetValue`. A twelfth checked stdcall preserves both
prior endpoint stores, validates bounded ASCII `FlsSetValue`, and returns
endpoint `0x7F020020`. BGI stores it at `0x100FEEC4`; five instructions later
the same thunk is reached for `FlsFree`. A thirteenth checked stdcall preserves
all three prior stores and returns endpoint `0x7F020030`. BGI stores it at
`0x100FEEC8`; twelve instructions later static thunk ID `70` is reached for
`KERNEL32.dll!TlsAlloc`. A fourteenth checked zero-argument stdcall validates
the bound IAT and all four endpoint stores, discards native TLS index `0x1C`,
and returns Vine-owned logical TLS index `0`. Seven instructions later BGI
reaches thunk ID `68`, `KERNEL32.dll!TlsSetValue`, with arguments
`[0, 0x7F020010]`. A fifteenth checked stdcall reconciles the stack, endpoint
store page, and BGI's TLS-index global, then associates endpoint ID `2` with
controlled thread ID `8192` and returns BOOL `1`. Nine instructions later BGI
reaches thunk ID `89`, `KERNEL32.dll!EncodePointer`, with argument `NULL`.
A sixteenth checked stdcall validates the exact direct-IAT call and retained TLS
state, discards two observed process-cookie-dependent native results, and
returns deterministic encoded-NULL token `0xEC010001`. Fifty-five instructions
later BGI reaches thunk ID `89` again with non-NULL argument `0x100A9608`.
A seventeenth checked stdcall validates that executable process-initializer
function, returns distinct token `0xEC010002`, and observes BGI store it at
`0x100FF62C`. Nine instructions later the same thunk is reached for controlled
dynamic endpoint ID `1`, `FlsAlloc` at `0x7F020000`.
Fourteen static endpoints are declared; the other 316 retain exact identity as
explicit traps. Memory generation remains `30`.

The runtime kernel has advanced beyond that frozen compatibility anchor through
the separate `bgi-generic-runtime-reentry` semantic run. BGI is registered with
14 process-owned runtime dependencies and all 330 runtime export endpoints, then
executes through one persistent Rust runtime session with automatic PTE Dirty
writeback. It validates the frozen 17 calls, completes four additional
`EncodePointer` calls, and reaches 21 completed calls before stopping at
instruction 266 on
`KERNEL32.dll!InitializeCriticalSectionAndSpinCount(0x100FF630, 4000)`. The
generic run advances memory generation `30 -> 52` through 22 writeback batches
and 3 host deltas without an API-specific reconciliation list. Its separate
96,244-byte report has SHA-256
`a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa`; the
frozen 140,660-byte report remains byte-identical.

The critical-section capability profile adds process-owned synchronization.
Native evidence fixes the 24-byte x86 layout, BOOL／LastError policy, recursive
same-thread enter／leave, and BGI wrapper RVAs. The generic session initializes 14
contiguous objects and reaches 35 completed calls before stopping at instruction
575 on `DecodePointer(0xEC010003)`. It uses 36 writeback batches, 65 dirty pages,
344 CPU-changed bytes, and 17 host deltas, finishing at generation `80`. Its
167,719-byte report has SHA-256
`cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69`.

The cumulative pointer-decoding profile accepts only registered Vine tokens,
returns `0x7F020000` for `0xEC010003`, and immediately reaches dynamic
`FlsAlloc(0x100A5666)` at instruction 577. It completes 36 calls through 37
writeback batches／66 dirty pages／348 changed bytes and generation `81`. Its
171,356-byte report has SHA-256
`5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e`.

The cumulative FLS-allocation profile dispatches dynamic `FlsAlloc` through the
same generic path, validates executable callback `0x100A5666`, preserves
LastError, and returns process-owned logical index `0` instead of native index
`9`. It completes 37 calls, then reaches
`HeapAlloc(0x68010001, 8, 0x214)` at instruction 618. Its 38 writeback batches
cover 68 dirty pages／399 changed bytes and finish at generation `82`; the
175,123-byte report has SHA-256
`26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c`.

The cumulative private-heap profile dispatches that reached allocation through
the existing transaction, returns zeroed guest address `0x20000000`, and maps
its first 4 KiB page into the v86 mirror. It completes 39 calls, including a
second `DecodePointer`, then stops at dynamic
`FlsSetValue(0, 0x20000000)` at instruction 644. Its 40 writeback batches cover
70 dirty pages／415 changed bytes; 18 host deltas finish at generation `87`.
The 183,102-byte report has SHA-256
`d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4`.

The cumulative FLS-value profile stores `0x20000000` in logical slot `0` for
controlled thread `8192`, returns BOOL `1`, and preserves LastError. Existing
contracts then pass a second module lookup and one critical-section entry before
stopping at `InterlockedIncrement(0x100F08E0)` at instruction 707. It completes
42 calls through 43 writeback batches／75 dirty pages／497 changed bytes and 19
host deltas, finishing at generation `91`. The 195,821-byte report has SHA-256
`f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f`.

Twenty Kernel32 contracts now use the generic Rust dispatcher. This includes
clock／identity, startup, heap, module／export, TLS, pointer encoding／decoding,
critical-section, dynamic FLS allocation／value storage, and reached private-heap allocation.
The dispatcher owns endpoint lookup, checked frame decoding,
pointer and name／ordinal validation, guest writes, return cleanup, LastError
policy, and identity-preserving faults.

Those calls now share one authoritative Rust `RuntimeState`. It owns the
controlled process／logical thread registry, per-thread LastError, clock and startup
profile, heap／module／endpoint registries, per-thread TLS／FLS values, deterministic encoded
pointers with strict inverse lookup, guest-address-keyed critical sections, and
bounded process-owned FLS slots with retained cleanup callbacks. Nested callback frames are LIFO and asynchronous host completion is a
scheduler event that restores the prior runnable／callback state. Private JSON
request values remain evidence assertions, not API result sources. A controlled
one-object wait queue now moves a logical lane from runnable to waiting and back
when another thread's waitable object is signaled. Per-thread bounded raw message
FIFOs add empty-queue blocking and first-message wake without claiming User32 APIs.

The foundation below that checkpoint uses Rust `GuestMemory` as byte authority
and v86 paging as a bounded execution mirror. Initial reflection, host delta,
and CPU writeback use binary control buffers plus contiguous page payloads.
After every bounded CPU slice, the host scans writable PTE Dirty bits without
an API-specific page list and atomically reconciles changed pages by owner and
generation. A public two-slice fixture writes across stack, heap, surface, and
RWX pages, discovers exactly 7 then 4 dirty pages, and advances Rust generation
`18 -> 21` around one intervening host delta. The earlier fixture still writes
eight bytes to a non-executable data page, transfers one 4 KiB delta to v86,
and has guest x86 read back `0x8877665544332211`. A conservative 15-byte execute
window also stops cross-page instruction fetch when the next page is
non-executable.
Multi-owner bootstrap, PE materialization, and import commit paths use a
changed-state／frame before-image rollback journal. Transaction start copies no
committed page bytes; a 1,024-page scaling contract records 4 KiB only after the
first changed page. Import planning is read-only and reports every unresolved import;
IAT and thunk changes are all-or-nothing, so extending a catalog and retrying on
the same image is supported before the first successful bind.

A controlled PE32 fixture now runs inside a Chrome Runtime Worker through the
same Rust memory and generic dispatcher boundaries. The browser path completes
46 guest instructions in 10 bounded slices with three periodic yields, two
deferred host services, a depth-2 nested callback, seven automatic writeback
batches, Canvas RGBA readback, an 880 Hz Web Audio pulse, IndexedDB byte `0x5a`,
and exact clean exit. Pending-request cancellation and a four-instruction
watchdog both leave no suspended host request or callback. A 30-minute repeated
lifecycle stress completed 11,326 sessions with zero failures, a measured JS
heap slope of 136 bytes/minute, and a worst main-thread heartbeat gap of
16.28 ms.

The development profile is Windows 11 Pro 25H2, although the current OS API
reports Home; the observed build is recorded separately from that unresolved
edition label. General DLL/export lifecycle, CPU-context-backed multi-thread scheduling,
general wait/message semantics, BGI
browser execution, production graphics/audio/storage semantics, and full title
play remain unimplemented.
The 17 checkpoint expectations now live in a versioned manifest shared by the
Node and Rust runners. Stable npm checkpoint commands select one generic session
by ID; API-specific dispatch flags are no longer used. A clean stable-Chrome
benchmark now records a `go` decision for the exchangeable v86 baseline:
134.1M integer instructions/s, 11,279 production API roundtrips/s, 14.58 MB
active steady memory, a 0.985 hidden-tab throughput ratio, and a 51.315 ms
maximum AudioWorklet delivery gap. A generation-tagged typed object／handle
table now separates process／thread object lifetime from closable handles,
rejects stale generations and type aliases, preserves wait signals across
reopen, and validates the existing heap／module handles. A process-owned module
graph now adds canonical basename identity, main／guest／runtime classes,
dependency and reference ownership, and deterministic dependency-first attach／
reverse detach metadata. Process／initial-thread lifecycle now separates
bootstrap, running, exit request, and waitable completion, with atomic cleanup
and thread-before-process terminal ordering. A controlled second-thread registry
adds unique TIDs, typed handles, isolated LastError／TLS／callback lanes, explicit
selection, thread-only exit, and deterministic all-lane process termination.
A controlled wait queue adds monotonic registration identity, object-indexed
FIFO ownership, exact signal/wake events, already-signaled completion, and exit
cancellation. A controlled raw message queue adds global sequence identity,
owner-thread FIFO dequeue, reblock／rewake, and exit cleanup. Private heaps now
have bounded guest-memory backing; mapped images, bounded PE32 export resolution,
guest／runtime import binding, and DLL process attach failure unwind／retry／detach
are also process-owned foundations. The next runtime-kernel checkpoint measures
and implements `InterlockedIncrement(0x100F08E0)` against checked writable guest
memory. Bespoke numbered BGI checkpoint expansion remains paused.

## Requirements

- Node.js 22 or newer
- Rust 1.97.0; the root toolchain file pins the production workspace
- LLVM/Clang with `wasm32` and `i686-pc-windows-msvc` targets
- `lld-link`, `wasm-ld`, and `llvm-readobj`
- MSYS2 `make` and `sh` on `PATH` for `npm run prepare:engine`
- npm dependencies from the committed lockfile

## Checks

```sh
npm ci
npm run check
```

`check:fast` validates docs and the runtime checkpoint manifest, rebuilds public
fixtures, runs Rust format/tests/clippy with warnings denied, checks JavaScript
syntax, runs the small v86 smoke,
and checks the Windows trace summarizer. `check:fixture-e2e` prepares the pinned
source adapter and runs the public entry, ABI callback/return, stack, TEB/PEB,
process-parameters, MemoryDelta, automatic binary CPU writeback, window, and
browser-presentation paths, including the guest CPU Runtime Worker vertical
slice.
`check:adapter` holds the broader adapter checks. `check:private-corpus` runs BGI
only when `VINE_BGI_EXE` or the documented local path exists; absence is an
explicit skip. GitHub Actions runs the two public check groups on Linux.
Linux bootstrap checks consume fixture-hash-bound recorded Windows oracle
evidence and label that provenance; they do not claim to execute PE32 natively.
Generated reports are written under `.work/` and are not tracked by Git.

`npm run prepare:engine` reproduces the source adapter and its single CPU state
round-trip contract check when the pinned v86 source build is needed.
`npm run check:runtime:entry` then verifies the production-memory reflection
and controlled entry-to-thunk boundary. `npm run check:runtime:cdecl` verifies
the fixed cdecl return and callback-service ingress without invoking the
callback. `npm run check:runtime:callback` verifies the separate Rust-owned
return sentinel, checked frame, guest callback, and exact result `43` stop.
`npm run check:runtime:outer-return` retires that frame, returns through the
outer cdecl thunk, and verifies the following result write, entry result, stack
balance, and structured unmapped-return fault.
`npm run check:runtime:entry-return` replaces that harness return value with a
second production-owned control sentinel and verifies exact successful entry
completion without accepting unrelated faults.
`npm run check:runtime:stack-bootstrap` reflects a Rust-owned initialized stack
page and verifies the same completion with no adapter-local stack allocation.
`npm run check:runtime:bootstrap-fs-first-stop` reruns the native FS oracle and
verifies the exact zero-FS-base source-v86 page fault before TEB／PEB setup.
`npm run check:runtime:bootstrap-fs` adds Rust-owned TEB／PEB pages, restores FS
base through the CPU state ABI, and verifies exact entry completion.
`npm run check:runtime:bootstrap-params-first-stop` compares the native
command-line oracle with the explicit missing-pointer guest result.
`npm run check:runtime:bootstrap-params` installs the measured Rust-owned
parameters and UTF-16 bytes and verifies exact guest completion.
`npm run check:runtime:memory-delta` applies one dirty data page to live v86
mapping and verifies the updated eight-byte value from guest code.
`npm run check:runtime:memory-writeback` runs two bounded guest slices, scans
v86 PTE Dirty bits, and atomically returns stack／heap／surface／RWX pages to the
Rust byte authority through binary control and payload buffers.
`npm run check:runtime:browser-worker` builds the zero-import Rust/Wasm core and
TypeScript application, then runs the controlled guest CPU, async completion,
nested callback, pixel／audio／storage, cancellation, and watchdog paths in a
clean Chrome profile. `npm run check:runtime:browser-worker:stress` adds the
30-minute repeated lifecycle measurement.
`npm run check:runtime:bgi-first-win32-call` reconciles one live stack page,
writes a controlled `FILETIME` through `MemoryDelta`, returns from exact thunk
ID `104`, and stops at exact thunk ID `103` before a second dispatch.
`npm run check:runtime:bgi-second-win32-call` reconciles the next live call
frame at generation `25`, returns controlled process ID `4096` from exact thunk
ID `103`, and stops at exact thunk ID `149` before a thread-ID dispatch.
`npm run check:runtime:bgi-third-win32-call` reconciles the next live call frame,
returns controlled thread ID `8192` from exact thunk ID `149`, and stops at
exact thunk ID `13` before a tick-count dispatch.
`npm run check:runtime:bgi-fourth-win32-call` returns controlled tick count
`3,600,000` from exact thunk ID `13` and stops at exact thunk ID `157` before a
performance-counter dispatch.
`npm run check:runtime:bgi-fifth-win32-call` writes controlled counter
`12,345,678,901` through a generation `26` delta, returns from exact thunk ID
`157`, and stops at exact thunk ID `83` before a startup-info dispatch.
`npm run check:runtime:bgi-sixth-win32-call` writes the controlled 68-byte x86
startup profile through generation `27`, preserves EAX for the VOID return from
exact thunk ID `83`, and stops at exact thunk ID `84` before a heap-information
dispatch.
`npm run check:runtime:bgi-seventh-win32-call` validates the exact termination-
on-corruption variant, preserves generation `27`, returns BOOL success from ID
`84`, and stops at exact thunk ID `102` before a heap-creation dispatch.
`npm run check:runtime:bgi-eighth-win32-call` validates the exact private-heap
creation variant, returns typed logical handle `0x68010001` from ID `102`
without a guest write, and stops at exact thunk ID `72` before module lookup.
`npm run check:runtime:bgi-ninth-win32-call` validates bounded UTF-16 module
lookup, returns typed logical KERNEL32 handle `0x4D010001` from ID `72`, and
stops at exact thunk ID `150` before dynamic export lookup.
`npm run check:runtime:bgi-tenth-win32-call` validates the register-indirect
`GetProcAddress` IAT sequence and bounded ASCII `FlsAlloc`, returns sealed
dynamic endpoint `0x7F020000`, and stops at ID `150` for `FlsGetValue`.
`npm run check:runtime:bgi-eleventh-win32-call` reconciles BGI's first endpoint
store, validates the persistent ESI call and bounded ASCII `FlsGetValue`,
returns endpoint `0x7F020010`, and stops at ID `150` for `FlsSetValue`.
`npm run check:runtime:bgi-twelfth-win32-call` reconciles both prior endpoint
stores, validates the next persistent ESI call and bounded ASCII `FlsSetValue`,
returns endpoint `0x7F020020`, and stops at ID `150` for `FlsFree`.
`npm run check:runtime:bgi-thirteenth-win32-call` verifies all three prior
stores, returns `FlsFree` endpoint `0x7F020030`, observes its guest store, and
stops at unsupported static thunk ID `70` for `TlsAlloc`.
`npm run check:runtime:bgi-fourteenth-win32-call` validates zero-argument
`TlsAlloc`, returns typed logical index `0` without importing the native index,
preserves all four endpoint stores, and stops at unsupported thunk ID `68` for
`TlsSetValue(0, 0x7F020010)`.
`npm run check:runtime:bgi-fifteenth-win32-call` records that endpoint as the
initial controlled thread's logical TLS value, returns BOOL `1`, and stops at
unsupported thunk ID `89` for `EncodePointer(NULL)`.
`npm run check:runtime:bgi-sixteenth-win32-call` validates that direct-IAT
NULL call, returns deterministic token `0xEC010001` without importing the
native process cookie, retains the TLS value, and stops at the next non-NULL
`EncodePointer(0x100A9608)` call.
`npm run check:runtime:bgi-seventeenth-win32-call` validates that executable
guest function, returns token `0xEC010002`, observes its guest global store,
and stops at `EncodePointer(0x7F020000)` for the `FlsAlloc` endpoint.
`npm run check:runtime:bgi-generic-runtime-reentry` independently validates all
17 frozen semantics, continues through 21 generic calls with automatic binary
writeback, and stops fail-closed at the first missing critical-section contract.
`npm run check:runtime:bgi-critical-section-synchronization` validates the 14
process-owned initializations, preserves both earlier reports, and stops
fail-closed at `DecodePointer(0xEC010003)` after 35 completed calls.
`npm run check:runtime:bgi-pointer-decoding` resolves only the registered token,
preserves all three earlier reports, and stops fail-closed at dynamic
`FlsAlloc(0x100A5666)` after 36 completed calls.
`npm run check:runtime:bgi-fiber-local-storage-allocation` allocates logical FLS
index `0`, preserves all four earlier reports, and stops fail-closed at
`HeapAlloc(0x68010001, 8, 0x214)` after 37 completed calls.
`npm run check:runtime:bgi-private-heap-allocation` returns zeroed guest pointer
`0x20000000`, preserves all five earlier reports, and stops fail-closed at
dynamic `FlsSetValue(0, 0x20000000)` after 39 completed calls.
`npm run check:runtime:bgi-fiber-local-storage-value` stores that pointer for
controlled thread `8192`, preserves all six earlier reports, and stops
fail-closed at `InterlockedIncrement(0x100F08E0)` after 42 completed calls.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
