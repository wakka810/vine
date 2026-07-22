# Checks and CI

## Canonical commands

```powershell
npm ci
npm run check:fast
npm run check:fixture-e2e
npm run check
```

`check:fast` remains the public local short gate. It composes three independently
runnable groups: `check:static` for generated documentation, semantic／evidence／CI
contracts, JavaScript syntax, and Windows trace-summary validation;
`check:toolchain-engine` for reproducible public PE fixtures and the pinned small v86
smoke; and `check:rust` for format, workspace tests, and clippy with warnings denied.
This grouping preserves the canonical command while allowing CI failures to identify
their owner without replaying unrelated work.

The focused evidence contract is:

```powershell
npm run check:evidence-classification
```

It fixes the six evidence classes, accepts valid scheduler partial-order
interleavings, rejects forbidden order, keeps whole-report mismatch non-semantic,
and verifies structured benchmark pass／fail reasons.

The focused COM foundation contract is:

```powershell
npm run check:com-foundation
```

It runs the Rust IUnknown ABI／identity／lifetime／rollback fixture, the reached
`CoInitialize` ABI fixture, per-thread apartment plan／overflow／exit tests,
TypeScript Runtime Worker typecheck, and the Browser-resource
identity-separation contract.

`check:fixture-e2e` prepares the pinned source adapter, fixes its generic
execution-engine contract against the actual Wasm, then runs the controlled
entry, cdecl dispatch, callback, outer return, entry sentinel, Rust-owned stack,
FS／TEB／PEB, process parameters, MemoryDelta data roundtrip, automatic binary
CPU writeback, window loader, Chrome Canvas presentation, and the full Chrome
Runtime Worker vertical path. The Worker check runs guest CPU, deferred host
completion, depth-2 callbacks, Canvas, Web Audio, IndexedDB, immutable source
Open／Size／Read／Seek／Close, pending-request cancellation, and watchdog cleanup.
It also rejects a cross-origin source service before fetch and uses only repository fixtures.

The focused immutable-source contracts are:

```powershell
cargo test -p vine-core --test source_file_gateway --locked
cargo test -p vine-core --lib runtime::files::tests --locked
npm run check:runtime:browser-worker
```

The Rust tests fix manifest／path／share／typed-handle／cursor／completion／teardown
atomicity. The browser command builds the public self-checking PE32 source fixture,
verifies its native exit, and exercises same-origin success, pending cancel, and
watchdog without requiring the private title.

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

The BGI benchmark command is a separate three-minute private workload gate. Its
current capacity workload is selected by the checkpoint manifest and pinned by
[`bgi-browser-d3d9-initial-render-states-v1.json`](../fixtures/benchmarks/bgi-browser-d3d9-initial-render-states-v1.json).
It repeats the complete current session through the first texture's surface
`GetDC`, stock-brush `FillRect`, `ReleaseDC`, surface retirement, then the
static-default and dynamic-default texture creations, device slot 26
`CreateVertexBuffer`, vertex-buffer slot 11／12 `Lock`／`Unlock`, and the exact
four device slot 57 `SetRenderState` calls before the slot 19 `GetRasterStatus`
stop. Every summary-evidence
repetition checks call count, digest algorithm／SHA, terminal semantics, and
teardown while the standalone Chrome check retains ordered-call equivalence, and
applies versioned latency, heartbeat, heap／backing-storage slope, linear-memory,
and Wasm-crossing thresholds. Its result and non-claims are recorded in
[bgi-browser-benchmark.md](bgi-browser-benchmark.md). It is not part of public CI
because the private executable is never distributed.

`threshold-blocked` is a required failure outcome, not a soft pass. The current
manifest must retain the failing threshold ID, full distribution count, exact
report and Node A/B identities, attempted isolation evidence, protected
threshold／retry invariants, and resume condition.
No downstream primary checkpoint or private-evidence commit proceeds until the
same 180-second fixture passes unchanged on an isolated host, or a repeated
isolated failure leads to a profiled bounded optimization.

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
[v86-browser-benchmark.md](v86-browser-benchmark.md). Its production API probe
uses a bounded 16-instruction setup outside the timed region and requires the
exact runtime-owned `UD2`; a budget stop is reported with reason, error, count,
and EIP rather than misclassified as an API trap.

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
`--observe multi-byte-to-wide-char`. Reached shared-clock evidence additionally uses
`--observe time-get-time`. Startup records the 68-byte x86 span;
heap-set records the exact policy call; heap-lifecycle hooks reached lifecycle
calls; and heap-allocation retains only main-executable `HeapAlloc`／`HeapFree`
with bounded buffer／caller／lifetime metadata. Module-handle decodes A/W names,
while dynamic-export records enter-time LastError for `GetProcAddress`.
Heap, module, and dynamic-export observations retain the focused LastError
evidence. GetCPInfo retains direct BGI calls and a guarded raw-output／LastError
self-oracle with bounded x86 consumer frames. The multibyte option records the
six-argument query／conversion pair, bounded bytes／UTF-16 output, consumer
frames, and a guarded Windows-932 oracle. The multimedia-clock option records
hook／system result agreement, LastError sentinels, wrapping `GetTickCount`
brackets, and the reached consumer without treating host tick values as policy. Ordinary
traces do not install these hooks.

## GitHub Actions

`.github/workflows/ci.yml` uses separate `static-semantic`, `rust-tests`,
`fixture-build`, `fixture-core-e2e`, `browser-acceptance`,
`performance-observation`, `chrome-stable-observation`, `windows-native-oracle`,
and `fuzz-smoke` jobs. The fixture build is the only producer of the public PE32 and
source-adapter inputs; Rust, core, and browser consumers download that immutable
artifact. Rust tests consume the reproducibly generated PE fixtures, so the explicit
artifact dependency is part of their test contract rather than an optional cache.
The browser build is likewise passed to performance and stable observation without
rebuilding it. The workflow contract test parses both workflow files as YAML and
rejects missing jobs, unknown npm scripts, fixture dependency loss, target loss, or
browser-role drift. Browser servers bind port `0` and publish the OS-selected
loopback URL; acceptance and benchmark code do not depend on a fixed port or on the
browser Fetch unsafe-port list.

Required browser acceptance and performance use Chrome for Testing
`151.0.7922.34`. A separate non-blocking job reruns the Worker vertical on the moving
Chrome stable channel, so a browser release is observed without changing the
acceptance oracle. Ubuntu uses Node 22, Rust 1.97.0, and the
`wasm32-unknown-unknown` target. Its 82 MB llvm-mingw `20260616` release asset is
verified as SHA-256
`534b92e067b22a6b4441f48ae9240a3341b17825d04d577eab0cf85c44b4deda`;
the runner's system clang handles the separate Wasm smoke.

The Windows 2022 job downloads the matching UCRT x86_64 archive, verifies SHA-256
`b9b68a4d276e16fa25802aaba458e4638f64b3884c290aaccdc2d87083b6ca35`,
captures and verifies the runner LLVM's `wasm32` backend before llvm-mingw is added
to `PATH`, and passes that absolute compiler path as `VINE_WASM_CLANG`. The pinned
llvm-mingw compiler remains the PE32 owner; the runner compiler is used only for the
separate Wasm smoke. The job then runs `check:toolchain` plus the trace-summary
contract. On Windows,
`check:toolchain` executes nine public PE32 self-oracles natively, including ABI,
DLL lifecycle, source read, window／paint, FS／process parameters, TLS, and FLS. It
does not contain or execute the private title. All jobs retain read-only repository
permissions; browser, benchmark, fuzz failure, and Windows reports are uploaded from
their owning jobs.

The browser-vertical D3D9 fixture keeps host and guest display authority separate:
native Windows accepts a bounded nonzero `D3DDISPLAYMODE` shape because the runner's
desktop mode is not deterministic, while the Vine adapter path requires the exact
versioned 1920x1080／60 Hz／format-22 profile. A native host mode is never imported into
the browser runtime.

The source adapter has two exact host-build profiles with the same 22 imports,
814 exports, adapter ABI version `1`, state ABI, and behavioral contract:

| Host profile | Bytes | SHA-256 |
|---|---:|---|
| Windows GNU | 1,857,800 | `99a0948c222a6ac2f21984798c9e9213dec9e43b95dff6fc5c8acf1f4dc7954f` |
| Linux GNU | 2,116,017 | `21b0fb4f63e9604a66e6522839c9744a4ae17f9ead1c2e201ff2e46fccff20c4` |

Windows report fingerprints remain the canonical evidence values. Linux
reports preserve their actual adapter identity and run all semantic assertions,
but do not pretend to have the same whole-report SHA-256. Browser checks use a
clean temporary profile and loopback-only DevTools endpoint. The Worker check
additionally captures Canvas readback, browser-service state, desktop/mobile
screenshots, and structured cleanup reports. Linux adds Chrome's CI-only
sandbox/shared-memory/GPU flags; Chrome is terminated before the profile and
local server are removed. Windows preparation forces the fallback target to
rebuild so MSYS `make` cannot silently retain an artifact from an older overlay.

## Fuzz target

The cargo-fuzz package is outside the production workspace and pins three targets:

- `pe32_load_plan`: arbitrary PE snapshot and load-plan ingress;
- `cpu_writeback_decode`: arbitrary split binary CPU writeback control／payload;
- `source_read_response_decode`: arbitrary split host read response, including the
  1 MiB pre-copy bound.

Linux reproduction uses the pinned nightly and cargo-fuzz release:

```bash
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly-2026-07-10 fuzz run --fuzz-dir fuzz cpu_writeback_decode -- \
  -max_total_time=15 -max_len=1048611 -timeout=5 -rss_limit_mb=2048
```

Pull requests and pushes run each target for 15 seconds in the independent
`fuzz-smoke` job. `.github/workflows/fuzz.yml` runs a 30-minute-per-target scheduled
matrix and uploads corpus／failure artifacts. Valid values and structured errors are
accepted; panic, abort, out-of-bounds, and unbounded allocation are failures. The
separate `fuzz/Cargo.lock` pins harness dependencies. The Windows GNU host cannot
link the required sanitizer runtime; local harness type-checking uses MSYS2 UCRT
`clang++ -fms-extensions`, while actual campaigns run on the documented Linux
environment. The production workspace remains pinned to stable Rust 1.97.0.
