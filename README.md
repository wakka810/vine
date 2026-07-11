# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The repository is in Phase 0 discovery. Current evidence includes reproducible
PE32 fixtures, a bounded v86 adapter, native Windows trace attribution, fixed
W1/W2 execution slices, a one-pixel Chrome presentation boundary, a
high-address memory measurement, and Rust PE32 load-plan, sparse-memory,
materialization, and import-binding ingress for the fingerprinted BGI image.
A production Rust boundary now owns the BGI image range, applies 18,199
`HIGHLOW` relocations at a nonpreferred base, and can bind a supported import
prefix to deterministic runtime-owned `UD2` thunks. General CPU memory/process
integration, DLL/export resolution, process bootstrap, the Win32 runtime, and the
browser application do not exist yet. The sealed controlled image now crosses
a versioned Rust-to-v86 reflection boundary, dispatches fixed cdecl
`fixture_add(19, 23)`, and executes one guest callback to a separately owned
exact return sentinel after real cache invalidation. The host now retires that
callback frame, returns result `43` through the outer cdecl thunk, and runs the
guest entry continuation through result `43`, entry result `0`, and balanced
stack state. A second typed runtime-control slot now classifies that entry
return at an exact production-owned sentinel. The controlled initial stack is
also Rust-owned and reflected instead of being allocated by the adapter. A
separate import-free bootstrap fixture now passes a native Windows oracle for
`FS:[0x18]` TEB self and `FS:[0x30]` PEB access; source-v86 reaches its exact
pre-implementation stop at CR2 `0x18` with PFEC `4` and FS base zero. Rust now
owns the minimum TEB／PEB pages as well; restoring FS base `0x7FFDE000` lets the
same fixture observe both fields and return entry result `0`.
An additional native-oracled fixture fixes `PEB + 0x10` and the i386
`ProcessParameters + 0x40` command-line layout; the current empty guest PEB
returns its explicit unsupported result `2` instead of pretending success.

The development profile is Windows 11 Pro 25H2, although the current OS API
reports Home; the observed build is recorded separately from that unresolved
edition label. The controlled fixture now binds both imports and seals its
image; BGI binds only the native-reachable `Direct3DCreate9` prefix and stops
at `DINPUT8.dll!DirectInput8Create`. The controlled entry now returns result `0`
to exact process-entry sentinel ID `2` with ESP `0x00700000`; arbitrary unmapped
returns still fault. The next bootstrap task installs Rust-owned
process-parameters and command-line bytes; see
[docs/v86-bootstrap-params-first-stop.md](docs/v86-bootstrap-params-first-stop.md).

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

The canonical check validates documentation, rebuilds the LLVM PE/Wasm
fixtures, runs the native Windows fixture oracles, tests the Rust PE32 loader,
sparse guest-memory, image-materialization, and import-binding boundaries,
verifies the pinned v86 artifacts and instruction smokes, and checks the
Windows trace summarizer contract.
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

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
