# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json).
Edit the manifest and run `npm run docs:generate`; do not edit this block by hand.

| Area | Current state |
|---|---|
| Historical baseline | `bgi-seventeenth-win32-call`: 17 calls, 238 instructions |
| Active runtime | `bgi-set-handle-count`: 61 calls across 24 API contracts, memory generation 121 |
| Browser | `private-bgi-generic-session` passed; BGI session `semantic-equivalence-passed`; benchmark `threshold-passed` (1579 sessions) |
| Engine evidence | v86 `go` from `fixtures/benchmarks/v86-browser-v1.json` |
| Next structured stop | `KERNEL32.dll!GetCommandLineA()` at instruction 1526: `endpoint-contract-missing` |

The active semantic report is 284,215 bytes with SHA-256
`91be62c168dd5296fa549a8bf553975853dfef506bcb3ce7670a7b8843f82f3b`. The hash identifies the artifact; semantic checks are the gate.

### Supported API Contracts

- `KERNEL32.dll!GetSystemTimeAsFileTime`
- `KERNEL32.dll!GetCurrentProcessId`
- `KERNEL32.dll!GetCurrentThreadId`
- `KERNEL32.dll!GetTickCount`
- `KERNEL32.dll!QueryPerformanceCounter`
- `KERNEL32.dll!GetStartupInfoW`
- `KERNEL32.dll!HeapSetInformation`
- `KERNEL32.dll!HeapCreate`
- `KERNEL32.dll!GetModuleHandleW`
- `KERNEL32.dll!GetProcAddress`
- `KERNEL32.dll!TlsAlloc`
- `KERNEL32.dll!TlsSetValue`
- `KERNEL32.dll!EncodePointer`
- `KERNEL32.dll!DecodePointer`
- `KERNEL32.dll!InitializeCriticalSectionAndSpinCount`
- `KERNEL32.dll!EnterCriticalSection`
- `KERNEL32.dll!LeaveCriticalSection`
- `KERNEL32.dll!FlsAlloc`
- `KERNEL32.dll!FlsSetValue`
- `KERNEL32.dll!HeapAlloc`
- `KERNEL32.dll!InterlockedIncrement`
- `KERNEL32.dll!GetStdHandle`
- `KERNEL32.dll!GetFileType`
- `KERNEL32.dll!SetHandleCount`

### Reproduce Current Checkpoint

```sh
npm run check:runtime:bgi-browser
```
<!-- END GENERATED CURRENT STATUS -->

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
`npm run check:runtime:bgi-interlocked-increment` returns three checked LONG
updates, preserves all seven earlier reports, and stops fail-closed at the
measured second `HeapAlloc(0x68010001, 8, 0x800)` after 50 completed calls.
`npm run check:runtime:bgi-private-heap-second-allocation` returns zeroed
allocation ID `2`, preserves all eight earlier reports, and stops fail-closed at
`GetStdHandle(STD_INPUT_HANDLE)` after 51 completed calls.
`npm run check:runtime:bgi-standard-input-handle` returns logical stdin handle
`0x53010001`, preserves all nine earlier reports, and stops fail-closed at
`GetFileType(0x53010001)` after 52 completed calls.
`npm run check:runtime:bgi-file-type` returns `FILE_TYPE_CHAR`, preserves all ten
earlier reports, and stops fail-closed at `GetStdHandle(STD_OUTPUT_HANDLE)` after
54 completed calls.
`npm run check:runtime:bgi-standard-output-handle` returns logical stdout handle
`0x53010002`, preserves all eleven earlier reports, and stops fail-closed at
`GetFileType(0x53010002)` after 55 completed calls.
`npm run check:runtime:bgi-standard-output-file-type` classifies that logical
stdout as `FILE_TYPE_PIPE`, preserves the prior policies and reports, and stops
fail-closed at `GetStdHandle(STD_ERROR_HANDLE)` after 57 completed calls.
`npm run check:runtime:bgi-standard-error-handle` returns logical stderr handle
`0x53010003`, preserves the prior policies and reports, and stops fail-closed at
`GetFileType(0x53010003)` after 58 completed calls.
`npm run check:runtime:bgi-standard-error-file-type` classifies that logical
stderr as `FILE_TYPE_PIPE`, preserves the prior policies and reports, and stops
fail-closed at `SetHandleCount(32)` after 60 completed calls.
`npm run check:runtime:bgi-set-handle-count` returns the policy-enabled count
`32`, preserves state and LastError, and stops fail-closed at `GetCommandLineA()`
after 61 completed calls.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
