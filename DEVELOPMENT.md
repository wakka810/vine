# Development

## Set up a checkout

Use Node.js 22 or newer, npm, Git, Bash, Make, Python 3, tar/xz, and the Rust toolchain pinned in `rust-toolchain.toml`. Put LLVM's `clang`, `llvm-ar`, `lld-link`, `llvm-dlltool`, `llvm-rc`, and `llvm-readobj` on `PATH`; the build needs both WebAssembly and PE32/i386 compiler support. The independent backend's exact x87 component builds its bundled SoftFloat sources with `clang` and `llvm-ar` (overridable through `VINE_X87_CC` and `VINE_X87_AR`). The Linux toolchain versions and verified LLVM archive used in CI are recorded in `.github/workflows/ci.yml`.

From this repository's root:

```sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build:runtime:browser-worker
node tools/check-browser-product.mjs --serve
```

Open the printed loopback URL in Chrome. The normal directory-picker path requires WebGL2 and Web Audio. The first build downloads and prepares the pinned CPU and media dependencies; generated output stays in `.work/` and `target/`. A checkout on another machine can rebuild those files and does not require a workspace-specific environment script or copied game assets. If tools are installed in user-owned directories, add their executable directories to `PATH` before building.

## Local loop

`CARGO_TARGET_DIR` can put Rust build output on another drive; the browser
packager uses that same directory for both Runtime and independent compiler Wasm.
Independent launches prepare known EXE/DLL entry paths before guest execution.
Preparation time and a bounded-capacity stop are logged separately; unknown or
invalidated targets still compile on demand. The mirror and Runtime checks cover
this path, including cooperative yields when many small regions fill a host turn.
The independent browser adapter also defaults to immutable instance IDs outside
generated code bytes, so Chrome can reuse native modules after logical code
retirement. `recompilerBindingGlobal: false` retains the diagnostic inline-ID
path. Linking and expanded cache capacity remain explicit options.

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

If WebGL2 cannot be created, this explicit server-source mode can present the same validated Rust quad snapshots through a bounded Canvas2D software rasterizer. The normal picker still requires WebGL2. Software presentation preserves point/linear sampling with wrap/clamp addressing, perspective-correct UV/RGB interpolation, XRGB opacity, and viewport clipping; it has a per-frame CPU-work limit and is intended for development, not a production performance guarantee. The launch status identifies software rendering.

## Checks

### Strict and experimental Fast execution

The launch selector defaults to Fast when the selected CPU checkout supports the
common Runtime arena. Strict remains selectable as the separate CPU/Runtime path;
a build without the common-memory CPU offers Strict only.
It uses `runtime-cpu.wasm`; Strict uses `runtime-core.wasm` and `v86-adapter.wasm`.
The choice can also be made with `?executionMode=fast` or `strict`. These are
execution architectures, not x87 numerical settings; preserving a current
baseline also preserves its existing numerical policy.

For a local comparison, set `VINE_V86_FAST_SOURCE` to the development fork and
`VINE_V86_STRICT_WASM` to a frozen baseline CPU. Alternatively, put those paths
in ignored `.work/v86-execution-modes.json` as `fastSource` and `strictWasm`.
Environment values override that local file. `prepare:engine` also uses the local
`fastSource` when no source override is set. The normal prepared build links
Fast from the selected Rust CPU sources and records artifact hashes in
`.work/browser-runtime/execution-modes.json`. `VINE_SHARED_TARGET` selects the
unified module's build cache. The pinned fork supports both execution paths;
a clean checkout builds Fast without a development source or frozen binary.

`npm run check:engine:fast:prepared` runs actual Wasm ownership/rollback tests and
PE32 JIT, SMC, fault, growth and direct-API integration. The full Chrome product
runner follows the built artifact's default mode. Set `VINE_EXECUTION_MODE=strict`
to check the comparison path explicitly. Guest threads share one execution Worker
and the Rust scheduler. Fast retains the physical page quota and currently
supports only guest virtual addresses below 2 GiB.
Wasm reserves a little over 2 GiB of virtual linear space; this is not an eager
2 GiB physical-page allocation. Full-title timing and private data stay outside Git.

- `npm run check:fast`: Rust core unit tests, formatting, TypeScript, JS syntax, small browser-adapter tests, and development-source HTTP/containment tests.
- `node tools/test-dev-source-server.mjs` and `node tools/test-dev-source-directory.mjs`: focused opt-in source transport and browser adapter tests. Server tests use temporary fixtures, bounded ranges, cache/snapshot changes, and rejection of invalid paths, symlinks, methods, tokens, origins, and hosts; private game files are not used.
- `node tools/test-software-game-surface.mjs`: explicit fallback gating, shared presentation validation, pixels/fills, point/linear wrap/clamp sampling, RGB tint, half-pixel placement, strip/rotation/perspective rendering, CPU bounds, and no partial frame on invalid input.
- `npm run check:rust`: workspace tests and strict Clippy.
- Independent backend: `cargo build -p vine-recompiler --target wasm32-unknown-unknown --release --locked`, then `node tools/test-recompiler-semantics.mjs --chrome` for generated-code semantics. `tools/test-independent-backend.mjs --input <external-manifest> --compiler <compiler.wasm> --output <external-report> --region-instructions 512` runs private captured code through the compiler and generated modules in Chrome. Keep private inputs/reports external; record compiler/input hashes, cold conversion, warm execution and mirror bookkeeping separately. This runner does not measure Runtime reconciliation, full-title display or OPFS.
- The unqualified numeric predicate IR is selected explicitly with `--predicate-ir` in semantic/mirror/Runtime checks or `--predicate-ir on` in the capture runner. It is mutually exclusive with the other experimental integer/control modes; normal launch does not enable it. A mode change rejects pending timestamp or REP comparison continuations before invalidating their code identities.
- Whole-unit SSA is selected with `--flow-ir` in those checks or `--flow-ir on` in the capture runner. It retains original decoded raw entries, merges instruction-level static continuations, fuses direct chains and owns local fault recovery; it is mutually exclusive with the other integer/control experiments. Capture results expose `warmCacheStable` and include any recompilation cost; versioned code must not be made to look resident by enlarging the comparison cache. It is disabled by default; the same continuation guards apply. Neither experimental mode is a 60 FPS qualification.
- Numerical policies are selected by `ExecutionEngineOptions.recompilerNumericMode`: `balanced` (browser adapter default), `strict`, or `fast`. FP IR and typed numerical imports are enabled by default; disabling FP IR requires explicit strict mode. Rust validates selection and retires code when it changes. Typed compilation binds approximate arithmetic directly and keeps load/store/compare imports strict. Low-level compiler defaults and existing exact-state regression runners remain strict. The semantic suite with `--fp-ir --typed-numeric` includes all three modes with separate error bounds, write faults and snapshot/restart; the mirror suite also executes unconfigured balanced mode and handle rebuild. Approximate capture comparisons must measure numerical deltas explicitly, not turn off strict checks and call that equivalence. Startup AOT delay is allowed; demand-compilation stalls and retained-cache execution still count.
- Independent Runtime attachment: `node tools/test-independent-runtime.mjs` selects the experimental backend and explicit strict numerical mode through the actual Worker/factory and Rust Runtime with immutable PE32 fixtures. It does not mock Windows API or scheduler results. `node tools/test-recompiler-mirror.mjs` checks the actual Wasm adapter's cache reuse, code invalidation, thread context and failed/successful memory rebuild. Add `--linked-regions` to these or the semantic suite, or `--linked-regions on` to the capture runner, to exercise the experimental Wasm region ABI. The corresponding `--fp-ir` / capture `--fp-ir on` options exercise explicit x87 state lowering with unchanged exact arithmetic. Adding `--typed-numeric` / capture `--typed-numeric on` uses fixed-operation numeric exports; it requires FP IR and also survives adapter rebuild. The `--expanded-cache` / capture `--expanded-cache on` option increases retained regions from 64 to 512 under the same 16 MiB byte cap. Runtime and compiler artifacts must both match the complete CPU snapshot ABI. `fixtures/pe32/fp-context.S` checks x87/XMM state across actual guest thread creation, Sleep and join when built as an immutable PE32 fixture. For the optional structured-loop experiment, use `--structured-loops` or capture `--structured-loops on`. These regression-runner options remain disabled by default; browser FP IR and typed numerical defaults are described above. The prepared browser build includes both CPU artifacts.
- `npm run check:toolchain` and `npm run prepare:engine`: prepare the fixture toolchain and build the pinned Vine-v86 fork in `.work/vine-v86`. Local changes in that CPU checkout must be preserved before preparing another revision.
- For fork development, set `VINE_V86_SOURCE` to an existing separate Vine-v86 checkout before `prepare:engine` and the browser build/check commands. This explicitly builds its current source, including local changes, without checking out or resetting a revision. The default remains the published pin. `ExecutionEngineOptions.v86NumericMode` selects `strict` or `fast`; capable CPUs default to fast, and an explicit fast request rejects an older CPU. Fast mode uses binary64 arithmetic and JIT-local x87 values with raw80 state at exits. It relaxes intermediate precision and numerical exception flags; directed rounding, unmasked exceptions and exceptional values retain the existing path. Guest memory permissions, writeback and thread snapshots remain checked. The selection survives physical-pool replacement.
- `npm run prepare:media`: build the pinned MPEG-1/MP2 Wasm decoder from verified source archives with Emscripten 4.0.23. Generated codec files and source archives stay in `.work/libav-mpeg-build`; the browser build copies the codec and its license into `libav-media/`.
- `npm run check:media`: test the prepared decoder with synthetic MPEG-1 ES/PS and MP2 samples, including pixels, PCM, timestamps, duration, EOF and failed bounded reads.
- `npm run check:runtime:browser-worker:prepared`: release Wasm/browser build plus Chrome PE32 fixture smoke checks using prepared dependencies. `npm run check:runtime:browser-worker` prepares them itself. `node tools/check-browser-product.mjs --assets <directory>` runs the same checks against an already-built, fixed asset directory, including an explicitly selected independent backend; it does not rebuild or change the supplied artifacts.
- `npm run check:milestone`: broader Rust and Chrome integration at a meaningful user-visible or authority milestone.
- `git diff --check`: catch whitespace errors. Check the actual diff, untracked paths, and source-file immutability before any allowed commit.

Fixture pixels, actual Web Audio PCM/Stop, audio notification waits, browser OPFS primitives, and PE32 save/close/fresh-worker/load show their *own* behavior; none proves full-title play, in-story save/load, or long-run stability. Private title scenarios belong outside this repository. Keep raw traces, generated reports, save data, selected source files, and build output under `.work/` or outside Git. Never modify a user's EXE/DLL/assets to advance a check.
