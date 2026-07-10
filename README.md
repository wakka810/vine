# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

The repository is in Phase 0 discovery. Current evidence includes reproducible
PE32 fixtures, a bounded v86 adapter, native Windows trace attribution, fixed
W1/W2 execution slices, a one-pixel Chrome presentation boundary, a
high-address memory measurement, and a Rust PE32 load-plan ingress for the
fingerprinted BGI image. Guest-memory materialization, DLL lifecycle, process
bootstrap, the Win32 runtime, and the browser application do not exist yet.

The development profile is Windows 11 Pro 25H2, although the current OS API
reports Home; the observed build is recorded separately from that unresolved
edition label. The next task is to connect the owned BGI load plan to the
provisional region/page owner, bounded allocator, and execute gate; see
[docs/pe-loader-ingress.md](docs/pe-loader-ingress.md).

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
fixtures, runs the native Windows fixture oracles, tests the Rust PE32 loader
ingress, verifies the pinned v86 artifacts and instruction smokes, and checks
the Windows trace summarizer contract. Generated reports are written under
`.work/` and are not tracked by Git.

`npm run prepare:engine` reproduces the source adapter and its single CPU state
round-trip contract check when the pinned v86 source build is needed.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
