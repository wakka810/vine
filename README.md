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
| Active runtime | `bgi-create-mutex-a`: 284 calls across 46 API contracts, memory generation 395 |
| Browser | `private-bgi-generic-session` passed; BGI session `semantic-equivalence-passed`; benchmark `threshold-passed` (620 sessions) |
| Engine evidence | v86 `go` from `fixtures/benchmarks/v86-browser-v1.json` |
| Next structured stop | `KERNEL32.dll!GetCurrentThread()` at instruction 25256: `endpoint-contract-missing` |

The active semantic report is 1,250,520 bytes with SHA-256
`0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`. The hash identifies the artifact; semantic checks are the gate.

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
- `KERNEL32.dll!HeapFree`
- `KERNEL32.dll!InterlockedIncrement`
- `KERNEL32.dll!GetStdHandle`
- `KERNEL32.dll!GetFileType`
- `KERNEL32.dll!SetHandleCount`
- `KERNEL32.dll!GetCommandLineA`
- `KERNEL32.dll!GetEnvironmentStringsW`
- `KERNEL32.dll!FreeEnvironmentStringsW`
- `KERNEL32.dll!WideCharToMultiByte`
- `KERNEL32.dll!GetLastError`
- `KERNEL32.dll!TlsGetValue`
- `KERNEL32.dll!FlsGetValue`
- `KERNEL32.dll!SetLastError`
- `KERNEL32.dll!GetACP`
- `KERNEL32.dll!GetCPInfo`
- `KERNEL32.dll!MultiByteToWideChar`
- `KERNEL32.dll!GetStringTypeW`
- `KERNEL32.dll!LCMapStringW`
- `KERNEL32.dll!InterlockedDecrement`
- `KERNEL32.dll!GetModuleFileNameA`
- `KERNEL32.dll!IsProcessorFeaturePresent`
- `KERNEL32.dll!SetUnhandledExceptionFilter`
- `KERNEL32.dll!HeapSize`
- `KERNEL32.dll!InitializeCriticalSection`
- `KERNEL32.dll!SetCurrentDirectoryA`
- `KERNEL32.dll!CreateMutexA`

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
`check:adapter` holds the broader adapter checks. `check:private-target` is the
required local commit-bound BGI gate; it fails when the source is unavailable
and writes a small evidence manifest only after Node／Chrome semantic agreement.
`check:private-corpus` retains the historical prefix checks and treats absence
as an explicit skip. GitHub Actions runs the two public check groups on Linux.
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
`npm run check:runtime:bgi` selects the current manifest profile. Historical
frozen checkpoints use `--checkpoint <id>`; historical runtime capabilities use
`--profile <capability>` (or `--profile baseline`). These are one generic runner
contract rather than permanent npm aliases. Detailed evidence remains in the
linked checkpoint documents.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
