# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The current checkpoint is `bgi-first-win32-call`. Rust owns and materializes
the fingerprinted BGI image, applies 18,199 `HIGHLOW` relocations, binds all
330 imports to deterministic runtime-owned `UD2` thunks, and reflects 388
sealed pages into source-built v86. BGI executes 19 instructions from entry
`0x100A383E` and reaches exact thunk ID `104`,
`KERNEL32.dll!GetSystemTimeAsFileTime`. A controlled clock sample is written
through a generation `25` stack-page `MemoryDelta`; the checked stdcall return
then executes four more instructions and stops at thunk ID `103`,
`KERNEL32.dll!GetCurrentProcessId`. Two endpoints are declared; the other 328
retain exact identity as explicit unsupported traps.

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
The next endpoint checkpoint is a checked zero-argument stdcall return from
thunk ID `103` using one explicit controlled process ID.

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

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
