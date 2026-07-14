# Checks and CI

## Canonical commands

```powershell
npm ci
npm run check:fast
npm run check:fixture-e2e
npm run check
```

`check:fast` is the public short gate. It runs documentation and runtime
checkpoint-manifest validation, evidence-classification contracts, public fixture generation, `cargo fmt --all
--check`, workspace tests, clippy for all targets and features with warnings
denied, JavaScript syntax checks, the pinned small v86 smoke, and the Windows
trace-summary contract.

The focused evidence contract is:

```powershell
npm run check:evidence-classification
```

It fixes the six evidence classes, accepts valid scheduler partial-order
interleavings, rejects forbidden order, keeps whole-report mismatch non-semantic,
and verifies structured benchmark pass／fail reasons.

`check:fixture-e2e` prepares the pinned source adapter, fixes its generic
execution-engine contract against the actual Wasm, then runs the controlled
entry, cdecl dispatch, callback, outer return, entry sentinel, Rust-owned stack,
FS／TEB／PEB, process parameters, MemoryDelta data roundtrip, automatic binary
CPU writeback, window loader, Chrome Canvas presentation, and the full Chrome
Runtime Worker vertical path. The Worker check runs guest CPU, deferred host
completion, depth-2 callbacks, Canvas, Web Audio, IndexedDB, pending-request
cancellation, and watchdog cleanup. It uses only repository fixtures.

The engine contract rejects source-revision, CPU-context ABI, or artifact
identity mismatch before guest startup and checks budgeted stops, context
save／restore, all five mapping operations, owner-tagged changed pages, and cache
effects. It also guards the production Worker source against v86 PTE／private
offset／numeric-stop dependencies.

The focused Worker commands are:

```powershell
npm run check:runtime:browser-worker
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
npm run serve:runtime:browser-worker
npm run check:runtime:browser-worker:stress
```

The single Node BGI checkpoint command selects the active manifest profile by
default. Historical evidence uses arguments instead of npm aliases:

```powershell
npm run check:runtime:bgi
npm run check:runtime:bgi -- --checkpoint bgi-seventeenth-win32-call
npm run check:runtime:bgi -- --profile kernel32-standard-error-file-type
```

`--profile baseline` selects the generic runtime re-entry before the first
capability profile. `--checkpoint` and `--profile` are mutually exclusive. The
Browser runner derives the Node oracle report directory from the same active
profile.

The last command adds a 30-minute repeated lifecycle soak and is not part of the
ordinary short gate. Its exact evidence and non-claims are recorded in
[browser-runtime-worker.md](browser-runtime-worker.md).

The BGI benchmark command is a separate three-minute private workload gate. It
repeats the complete current session, checks semantics on every repetition, and
applies versioned latency, heartbeat, heap／backing-storage slope, linear-memory,
and Wasm-crossing thresholds. Its result and non-claims are recorded in
[bgi-browser-benchmark.md](bgi-browser-benchmark.md). It is not part of public CI
because the private executable is never distributed.

The canonical commit-bound private target gate is:

```powershell
$env:VINE_BGI_EXE = "V:\softs\サクラノ詩\BGI.exe"
npm run check:private-target
```

It requires the private source, runs the current Node oracle and Chrome Runtime
Worker path, and writes the small versioned
[`private-target-evidence.json`](private-target-evidence.json). The result binds
the semantic outcome and artifact identities to the exact implementation
commit and tree. Protected runtime inputs must be clean. Explicitly admitted
unrelated dirty paths remain visible in the result. The three-minute benchmark
is recorded as a separate observation and is not rerun by this semantic gate.
The two-commit local workflow and failure policy are defined in
[private-target-gate.md](private-target-gate.md).

The versioned v86 performance gate is separate from the ordinary local short
gate because it takes about one minute and depends on a stable Chrome process:

```powershell
npm run check:runtime:v86-benchmark
```

It measures CPU／ABI／memory／Worker／background／AudioWorklet behavior with the
tracked `v86-browser-baseline-v1` thresholds. Only `go` exits successfully;
`conditional-go` and `no-go` both fail the regression command. The reference
Windows result, threshold semantics, and non-claims are recorded in
[v86-browser-benchmark.md](v86-browser-benchmark.md).

Public CI uses the explicit prepared observation mode:

```powershell
npm run check:runtime:v86-benchmark:ci:prepared
```

It still runs the same versioned suite and uploads every threshold failure.
`go` and non-severe `conditional-go` exit successfully in this mode; `no-go`
remains a required failure. This keeps a single hosted-runner scheduling outlier
from overriding the deterministic semantic signals while retaining the strict
local decision command above.

Windows runs execute the two bootstrap fixtures as native self-oracles. Linux
runs cannot execute PE32 directly, so they require the same fixed fixture
SHA-256 and carry `recorded-windows-i386-self-oracle` provenance in the report
while consuming the recorded exit code `0`. This is not reported as a fresh
native execution.

Broader adapter checks are separate:

```powershell
npm run check:adapter
```

Historical private prefix checks are opt-in:

```powershell
$env:VINE_BGI_EXE = "V:\softs\サクラノ詩\BGI.exe"
npm run check:private-corpus
```

`check:private-corpus` is not the checkpoint commit gate. If no private file
exists, the command prints the reason and succeeds as an
explicit skip. If BGI exists but the adapter is absent, it fails and requests
`npm run check:adapter`. When both are present it loads the versioned checkpoint
manifest and runs the immutable first-trap contract plus the cumulative first
through seventeenth supported Win32 call contracts through one generic runner.
Private binaries are never added to Git or required by CI.

Focused native startup／heap evidence is opt-in with
`py tools/windows/trace.py ... --observe startup-info`,
`--observe heap-set-information`, `--observe heap-lifecycle`, `--observe heap-allocation`,
`--observe module-handle`, `--observe dynamic-export`, `--observe get-cp-info`, or
`--observe multi-byte-to-wide-char`. Startup records the 68-byte x86 span;
heap-set records the exact policy call; heap-lifecycle hooks reached lifecycle
calls; and heap-allocation retains only main-executable `HeapAlloc`／`HeapFree`
with bounded buffer／caller／lifetime metadata. Module-handle decodes A/W names,
while dynamic-export records enter-time LastError for `GetProcAddress`.
Heap, module, and dynamic-export observations retain the focused LastError
evidence. GetCPInfo retains direct BGI calls and a guarded raw-output／LastError
self-oracle with bounded x86 consumer frames. The multibyte option records the
six-argument query／conversion pair, bounded bytes／UTF-16 output, consumer
frames, and a guarded Windows-932 oracle. Ordinary
traces do not install these hooks.

## GitHub Actions

`.github/workflows/ci.yml` runs on Ubuntu 24.04 with Node 22, Rust 1.97.0,
the `wasm32-unknown-unknown` target, rustfmt, clippy, LLVM 22.1.8, and stable
Chrome. The PE toolchain is the pinned
82 MB llvm-mingw `20260616` release asset, verified as SHA-256
`534b92e067b22a6b4441f48ae9240a3341b17825d04d577eab0cf85c44b4deda`;
the runner's system clang handles the separate Wasm smoke. It executes
`npm ci`, `check:fast`, `check:fixture-e2e`, and the prepared v86 browser
observation mode. The workflow has read-only repository permissions and does not run
private-corpus checks. It uploads the benchmark JSON report／screenshot,
toolchain report, Worker semantic report, and browser build manifest even when
the benchmark step fails.

The source adapter has two exact host-build profiles with the same 22 imports,
809 exports, state ABI, and behavioral contract:

| Host profile | Bytes | SHA-256 |
|---|---:|---|
| Windows GNU | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Linux GNU | 2,141,050 | `ac8349cbaccd31d6f5c68a84012fc7bc3a73eda2c06e519331396b43d8c42540` |

Windows report fingerprints remain the canonical evidence values. Linux
reports preserve their actual adapter identity and run all semantic assertions,
but do not pretend to have the same whole-report SHA-256. Browser checks use a
clean temporary profile and loopback-only DevTools endpoint. The Worker check
additionally captures Canvas readback, browser-service state, desktop/mobile
screenshots, and structured cleanup reports. Linux adds Chrome's CI-only
sandbox/shared-memory/GPU flags; Chrome is terminated before the profile and
local server are removed.

## Fuzz target

The cargo-fuzz package is outside the production workspace:

```powershell
cargo install cargo-fuzz
cargo +nightly fuzz check --fuzz-dir fuzz pe32_load_plan
cargo +nightly fuzz run --fuzz-dir fuzz pe32_load_plan
```

The target feeds bounded arbitrary bytes through `ModuleSnapshot` and
`build_pe32_load_plan`. Valid plans and structured errors are accepted; panic,
abort, and unbounded allocation are failures. The separate `fuzz/Cargo.lock`
pins the harness dependencies. On the current Windows GNU host, type-checking
uses LLVM `clang++` with MS extensions; a libFuzzer campaign requires a host
whose sanitizer-coverage runtime links correctly, such as the documented Linux
environment. The production workspace remains pinned to stable Rust 1.97.0.
