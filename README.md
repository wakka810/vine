# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The current checkpoint is `bgi-fourteenth-win32-call`. Rust owns and materializes
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
`[0, 0x7F020010]`. Twelve static endpoints are declared; the other 318 retain
exact identity as explicit traps. Memory generation remains `30` after the
allocation.

The foundation below that checkpoint now has an incremental `MemoryDelta`
protocol. The first transfer is a full snapshot; later generations carry only
mapping operations and dirty pages, with data transfer separated from TLB,
instruction-fetch, and generated-code effects. A public fixture writes eight
bytes to a non-executable data page, transfers one 4 KiB delta to v86, and has
guest x86 read back `0x8877665544332211`. A conservative 15-byte execute window
also stops cross-page instruction fetch when the next page is non-executable.
Multi-owner bootstrap, PE materialization, and import commit paths use rollback
transactions. Import planning is read-only and reports every unresolved import;
IAT and thunk changes are all-or-nothing, so extending a catalog and retrying on
the same image is supported before the first successful bind.

The development profile is Windows 11 Pro 25H2, although the current OS API
reports Home; the observed build is recorded separately from that unresolved
edition label. General DLL/export lifecycle, Win32 services, scheduler, browser
runtime, graphics, audio, storage, and full title play remain unimplemented.
The next API checkpoint is thunk ID `68` for `KERNEL32.dll!TlsSetValue`, limited
to the observed logical index `0` and `FlsGetValue` endpoint value.

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

`check:fast` validates docs, rebuilds public fixtures, runs Rust format/tests/
clippy with warnings denied, checks JavaScript syntax, runs the small v86 smoke,
and checks the Windows trace summarizer. `check:fixture-e2e` prepares the pinned
source adapter and runs the public entry, ABI callback/return, stack, TEB/PEB,
process-parameters, MemoryDelta, window, and browser-presentation paths.
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

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
