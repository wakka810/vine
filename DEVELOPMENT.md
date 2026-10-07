# Development

## Set up a checkout

Use Node.js 22 or newer, npm, Git, Bash, Make, Python 3, tar/xz, and the Rust toolchain pinned in `rust-toolchain.toml`. Put LLVM's `clang`, `lld-link`, `llvm-dlltool`, `llvm-rc`, and `llvm-readobj` on `PATH`; the build needs both WebAssembly and PE32/i386 compiler support. The Linux toolchain versions and verified LLVM archive used in CI are recorded in `.github/workflows/ci.yml`.

From this repository's root:

```sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build:runtime:browser-worker
node tools/check-browser-product.mjs --serve
```

Open the printed loopback URL in Chrome. The normal directory-picker path requires WebGL2 and Web Audio. The first build downloads and prepares the pinned CPU and media dependencies; generated output stays in `.work/` and `target/`. A checkout on another machine can rebuild those files and does not require a workspace-specific environment script or copied game assets. If tools are installed in user-owned directories, add their executable directories to `PATH` before building.

### Prepared or offline rebuilds

The pinned environment uses Rust 1.97.0, Node.js 22, LLVM-MinGW 20260616 for PE fixtures, and Emscripten 4.0.23 for media. JavaScript dependency versions are recorded in `package-lock.json`; do not substitute a fresh lockfile. The Linux CI workflow records the LLVM archive checksum and Chrome fixture version.

For an offline rebuild, first prepare this same revision while online, including `npm ci`, the Rust target and Cargo dependencies, `npm run check:toolchain`, `npm run prepare:engine`, and `npm run prepare:media`. Retain the prepared `.work/vine-v86`, `.work/libav-mpeg-build`, toolchains, `node_modules`, and Cargo cache. Then:

```sh
CARGO_NET_OFFLINE=true npm run build:runtime:browser-worker:prepared
npm run check:engine:prepared
npm run check:media
node tools/check-browser-product.mjs --serve
```

Prepared mode intentionally does not fetch or update CPU/media dependencies. Check that `.work/vine-v86` is clean and its `git rev-parse HEAD` matches `revision` in `tools/prepare-v86-adapter.mjs` before reusing it after a checkout change. Missing tools or caches require online preparation; a fresh clone alone is not an offline bundle. Do not transfer authentication configuration, private source assets, or browser save data with build caches.

## Local loop

1. Check `git status --short --branch` and preserve unrelated changes and immutable game files.
2. `npm run dev:target` builds and serves the **generic** Chrome directory picker, executable browser, and runtime. Select a source directory and PE32 executable; it does not auto-run a title-specific checkpoint.
3. On the first unsupported call, fault, or visible milestone, inspect how the guest uses the return value, LastError/HRESULT, outputs, identity, and lifetime. Read the subsystem implementation, production callers, Wasm/browser consumers, and relevant tests **before** editing.
4. Implement the smallest reusable Windows behavior in its Rust owner and rerun. Do not add title hashes, EIP/occurrence branches, fake successes, or a new policy/schema for one reached variant.
5. Use local focused tests for tricky behavior and trust boundaries. Cross-owner synchronous mutations preflight all fallible work before updating any state; keep pending identity checks when control leaves Rust for Chrome, CPU execution, or a guest callback.

### Optional read-only development source (Linux)

The normal product remains the Chrome directory picker. When a development browser cannot access a directory on the server machine, explicitly select one local source root:

```sh
npm run serve:runtime:browser-worker -- --dev-game-root /absolute/canonical/game-directory
```

With dependencies already prepared, the shorter rebuild/serve loop is:

```sh
npm run build:runtime:browser-worker:prepared
node tools/check-browser-product.mjs --serve --dev-game-root /absolute/canonical/game-directory
```

`--dev-game-root` is accepted only with `--serve`; omitting it exposes no source endpoint and preserves the ordinary picker. `--port N` optionally changes the development port (default 4173; zero chooses a free port). The listener always binds to `127.0.0.1`. The served page gets a development-source marker only while the explicit option is active, without editing generated assets. Do not publish this development server or include private game files in build output.

This transport requires Linux `/proc/self/fd` and `O_NOFOLLOW`; other platforms fail closed. Its root must be an absolute canonical directory without symlinks. Descriptor-relative traversal rejects symlinks in every path component and accepts only regular files/directories. Symlinks and special files are omitted from listings. Enumeration is limited to 4096 entries, hashing is streamed with a bounded metadata cache, and byte reads are at most 2 MiB. File identity, size, and nanosecond change/modification timestamps invalidate changed snapshots, including replacements or writes that restore the original modification time.

Source bytes are read-only and stay in their original directory. Guest saves remain in the existing isolated OPFS tree, with a stable source identity derived from the canonical root. The local endpoint requires a fresh process capability token for reads, exact loopback host/listener-port matching, and same-origin browser requests; it sends no CORS permissions and marks responses `no-store`/`Cross-Origin-Resource-Policy: same-origin`. The explicit forwarded browser route `https://127.0.0.2:<same-port>` is also accepted. The capability is a development convenience, not a service to expose to other users or networks.

If WebGL2 cannot be created, this explicit server-source mode can present the same validated Rust quad snapshots through a bounded Canvas2D software rasterizer. The normal picker still requires WebGL2. Software presentation preserves bilinear repeat sampling, perspective-correct UV/RGB interpolation, XRGB opacity, and viewport clipping; it has a per-frame CPU-work limit and is intended for development, not a production performance guarantee. The launch status identifies software rendering.

## Checks

- `npm run check:fast`: Rust core unit tests, formatting, TypeScript, JS syntax, small browser-adapter tests, and development-source HTTP/containment tests.
- `node tools/test-dev-source-server.mjs` and `node tools/test-dev-source-directory.mjs`: focused opt-in source transport and browser adapter tests. Server tests use temporary fixtures, bounded ranges, cache/snapshot changes, and rejection of invalid paths, symlinks, methods, tokens, origins, and hosts; private game files are not used.
- `node tools/test-software-game-surface.mjs`: explicit fallback gating, shared presentation validation, pixels/fills, bilinear repeat, RGB tint, half-pixel placement, strip/rotation/perspective rendering, CPU bounds, and no partial frame on invalid input.
- `npm run check:rust`: workspace tests and strict Clippy.
- `npm run check:toolchain` and `npm run prepare:engine`: prepare the fixture toolchain and build the pinned Vine-v86 fork in `.work/vine-v86`. Local changes in that CPU checkout must be preserved before preparing another revision.
- `npm run prepare:media`: build the pinned MPEG-1/MP2 Wasm decoder from verified source archives with Emscripten 4.0.23. Generated codec files and source archives stay in `.work/libav-mpeg-build`; the browser build copies the codec and its license into `libav-media/`.
- `npm run check:media`: test the prepared decoder with synthetic MPEG-1 ES/PS and MP2 samples, including pixels, PCM, timestamps, duration, EOF and failed bounded reads.
- `npm run check:runtime:browser-worker:prepared`: release Wasm/browser build plus Chrome PE32 fixture smoke checks using prepared dependencies. `npm run check:runtime:browser-worker` prepares them itself.
- `npm run check:milestone`: broader Rust and Chrome integration at a meaningful user-visible or authority milestone.
- `git diff --check`: catch whitespace errors. Check the actual diff, untracked paths, and source-file immutability before any allowed commit.

Fixture pixels, actual Web Audio PCM/Stop, audio notification waits, browser OPFS primitives, and PE32 save/close/fresh-worker/load show their *own* behavior; none proves full-title play, in-story save/load, or long-run stability. Private title scenarios belong outside this repository. Keep raw traces, generated reports, save data, selected source files, and build output under `.work/` or outside Git. Never modify a user's EXE/DLL/assets to advance a check.
