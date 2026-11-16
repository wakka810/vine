# Independent PE32 execution backend

The independent AOT/JIT backend is preserved as an experimental alternative.
The product defaults to the separately pinned Vine-v86 CPU. Neither backend
has established general title compatibility or sustained 60 FPS.

## Ownership and source map

| Component | Responsibility |
| --- | --- |
| `crates/vine-recompiler` | Decode/lift, IR, code discovery, Wasm generation, CPU state, execution mappings and code-cache lifetime |
| `crates/vine-x87` | Extended-precision arithmetic and numerical kernels; bundled SoftFloat sources and license |
| `crates/vine-cpu-profile` | Guest CPUID, timestamp frequency and existing Windows processor answers |
| `crates/vine-core/src/image_code.rs` | Runtime-validated executable roots from sealed PE images |
| `browser/runtime-worker/recompiler-mirror.ts` | CPU adapter, generated-module installation and checked Runtime memory transport |

The compiler uses iced-x86 decoding and its own instruction semantics. It does
not import v86's JIT, memory helpers, flags or dispatcher. Rust `Runtime` remains
the owner of guest bytes, mappings, Windows objects, threads, callbacks and
scheduler decisions. TypeScript owns browser resources and CPU transport, with
no duplicate Windows state. Source executables, DLLs and assets are immutable.

## Preparation and execution

AOT and demand compilation use the same conversion path. Before the independent
backend executes guest code or DllMain, Runtime supplies checked EXE/DLL entries,
executable exports and relocated code pointers as optional discovery roots.
They are validated against live image identity, ownership and permissions;
there is no title detector or arbitrary-word scan. Unknown or invalidated code
continues through demand compilation.

Ordinary lowering discovers bounded instruction regions, places decoded bodies
once and retains arbitrary admitted entries. Static cyclic components have
separate Wasm loops; known forward edges use direct branches. Indirect transfers
check the complete guest PC. Bounded known-return specialization checks the
actual guest stack target before taking a direct continuation. GPRs and flags
remain local until an exit, with precise recovery for faults, budgets and SMC.

Prepared code and demand code have separate residency limits. Preparation is
bounded at 4096 units and 128 MiB of generated Wasm; demand code defaults to
64 units and 16 MiB. Expanded demand capacity is explicit. Both classes use the
same ownership, mapping and code-content checks. Pending preparation is
revalidated after browser handoffs, and exhausted capacity is reported explicitly.

## Architectural state and memory

`state.rs` owns complete CPU interchange state: GPRs, EIP/EFLAGS, segment state,
raw x87 registers/environment, XMM registers, MXCSR and the exit record. MMX
aliases physical x87 storage. Thread switching and memory rebuilds retain that
state, including private resumable instruction continuations where required.

The execution mirror is independent of Windows memory authority. Guest pages
map to checked backing allocations with permission, owner and dirty metadata;
guest addresses cannot directly name Wasm pointers. Generated accesses preserve
read/write permission, segment bounds, page-crossing behavior and ordered faults.
Browser views must be reacquired after Wasm growth.

Cached modules depend on the code pages they fetched. Mapping, protection,
content changes and dirty aliases retire affected code. Admission validates
current identity and code stamps before execution; stale asynchronous compilation
cannot publish a replacement for a retired instance. Snapshot, writeback and
Runtime reconciliation remain checked operations.

Numerical kernels own no guest pointers or mappings. The exact x87 component
checks pending exceptions and prepares state/output before committing checked
stores. Unsupported instruction forms, execution modes or exceptional results
exit explicitly. Its supported arithmetic and accuracy contracts are described
in [crates/vine-x87/README.md](crates/vine-x87/README.md).

## Selection and numerical modes

`ExecutionEngineOptions.backend = "recompiler"` explicitly selects this backend.
Its Worker integration uses the separate Runtime execution architecture
(`executionMode=strict`); the product Fast module contains v86. The independent
Runtime test runner selects the required path itself.

The browser adapter defaults to ordinary lowering with native unit chaining,
immutable instance bindings, FP IR, typed numerical imports and balanced
binary64 arithmetic. Strict arithmetic and approximate binary32 fast arithmetic
remain selectable through `recompilerNumericMode`. Low-level semantic runners
retain strict defaults. Approximate modes keep load/store/compare semantics and
memory authority checks; they must be evaluated with explicit numerical bounds.

Alternative integer/control IR, direct-link directory, loop-memory proofs and
expanded-cache settings remain opt-in experiments. Their selection and mutual
exclusions live in `execution-engine-contract.ts` and `recompiler-mirror.ts`.
Changing a mode retires incompatible code and rejects incompatible pending
continuations. Passing semantic checks does not establish a performance benefit.

## Checks and remaining limits

See [DEVELOPMENT.md](DEVELOPMENT.md) for toolchain setup. Useful prepared checks:

```sh
cargo test --workspace --locked
cargo build -p vine-recompiler -p vine-runtime-wasm --target wasm32-unknown-unknown --release --locked
node tools/test-recompiler-semantics.mjs --chrome
node tools/test-recompiler-mirror.mjs
node tools/test-independent-runtime.mjs
```

The suites cover generated instruction behavior, precise fault/budget recovery,
SMC and cache lifetime, complete CPU snapshots, thread and callback boundaries,
and real Chrome Worker/Runtime execution. Chrome fixtures also exercise graphics,
input, audio and OPFS save/close/fresh-runtime/load. Fixed private VM, pixel and
audio captures are checked outside Git against complete CPU/memory results.

There is no automatic v86 fallback. A future cross-backend handoff would need
complete architectural state conversion and checked memory reconciliation at an
explicit boundary, while retaining the existing Rust Windows owners.

AOT preparation time, generated bytes, warm execution, demand compilation,
Runtime synchronization and browser work are separate costs. Capture speedups
are not additive title speedups, and Present counts do not establish display
FPS. Real observations reach title/opening progression, but full-title behavior,
SEH coverage, audio quality, steady frame deadlines and general compatibility
remain unqualified. Detailed measurements and rejected prototypes stay external.
