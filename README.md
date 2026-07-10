# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The repository is in Phase 0 discovery. The current evidence includes
reproducible PE32 fixtures, a freestanding Wasm module, and a source-patched
v86 adapter with bounded execution, structured faults, and a versioned CPU
state ABI. A PE/Win32 runtime and browser application do not exist yet.

Development uses Windows 11 Pro 25H2. The first task is to capture a focused
native startup trace from the target `BGI.exe`; see
[docs/windows-development.md](docs/windows-development.md).

## Requirements

- Node.js 22 or newer
- LLVM/Clang with `wasm32` and `i686-pc-windows-msvc` targets
- `lld-link`, `wasm-ld`, and `llvm-readobj`
- MSYS2 `make` and `sh` on `PATH` for `npm run prepare:engine`
- npm dependencies from the committed lockfile

## Checks

```sh
npm ci
npm run check
```

The canonical check validates the documentation, rebuilds the LLVM PE/Wasm
fixtures, verifies the pinned v86 Wasm artifacts, and runs controlled i386,
x87, SSE, and CPL3 trap programs. Generated reports are written under `.work/`
and are not tracked by Git.

`npm run prepare:engine` reproduces the source adapter and its single CPU state
round-trip contract check when the pinned v86 source build is needed.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
