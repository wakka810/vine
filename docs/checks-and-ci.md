# Checks and CI

## Canonical commands

```powershell
npm ci
npm run check:fast
npm run check:fixture-e2e
npm run check
```

`check:fast` is the public short gate. It runs documentation validation, public
fixture generation, `cargo fmt --all --check`, workspace tests, clippy for all
targets and features with warnings denied, JavaScript syntax checks, the pinned
small v86 smoke, and the Windows trace-summary contract.

`check:fixture-e2e` prepares the pinned source adapter, then runs the controlled
entry, cdecl dispatch, callback, outer return, entry sentinel, Rust-owned stack,
FS／TEB／PEB, process parameters, MemoryDelta data roundtrip, window loader, and
Chrome Canvas presentation paths. It uses only repository fixtures.

Windows runs execute the two bootstrap fixtures as native self-oracles. Linux
runs cannot execute PE32 directly, so they require the same fixed fixture
SHA-256 and carry `recorded-windows-i386-self-oracle` provenance in the report
while consuming the recorded exit code `0`. This is not reported as a fresh
native execution.

Broader adapter checks are separate:

```powershell
npm run check:adapter
```

Private title checks are opt-in:

```powershell
$env:VINE_BGI_EXE = "V:\softs\サクラノ詩\BGI.exe"
npm run check:private-corpus
```

If no private file exists, the command prints the reason and succeeds as an
explicit skip. If BGI exists but the adapter is absent, it fails and requests
`npm run check:adapter`. When both are present it runs the immutable first-trap
contract and the cumulative first through fifteenth supported Win32 call contracts.
Private binaries are never added to Git or required by CI.

Focused native startup／heap evidence is opt-in with
`py tools/windows/trace.py ... --observe startup-info`,
`--observe heap-set-information`, `--observe heap-lifecycle`,
`--observe module-handle`, or `--observe dynamic-export`. The first records the
68-byte x86 span, the second records the exact policy call, the third hooks
reached heap lifecycle calls, the fourth decodes A/W module names, and the
fifth records enter-time LastError for `GetProcAddress`. Heap, module, and
dynamic-export observations retain the focused LastError evidence. Ordinary
traces do not install these hooks.

## GitHub Actions

`.github/workflows/ci.yml` runs on Ubuntu 24.04 with Node 22, Rust 1.97.0,
rustfmt, clippy, LLVM 22.1.8, and stable Chrome. The PE toolchain is the pinned
82 MB llvm-mingw `20260616` release asset, verified as SHA-256
`534b92e067b22a6b4441f48ae9240a3341b17825d04d577eab0cf85c44b4deda`;
the runner's system clang handles the separate Wasm smoke. It executes
`npm ci`, `check:fast`, and `check:fixture-e2e`. The workflow has read-only
repository permissions and does not run private-corpus checks.

The source adapter has two exact host-build profiles with the same 22 imports,
809 exports, state ABI, and behavioral contract:

| Host profile | Bytes | SHA-256 |
|---|---:|---|
| Windows GNU | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Linux GNU | 2,141,050 | `ac8349cbaccd31d6f5c68a84012fc7bc3a73eda2c06e519331396b43d8c42540` |

Windows report fingerprints remain the canonical evidence values. Linux
reports preserve their actual adapter identity and run all semantic assertions,
but do not pretend to have the same whole-report SHA-256. Browser presentation
uses a clean temporary profile and loopback-only DevTools endpoint to read the
Canvas result. Linux adds Chrome's CI-only sandbox/shared-memory/GPU flags;
Chrome is terminated before the profile and local server are removed.

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
