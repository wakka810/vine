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
`npm run check:adapter`. Private binaries are never added to Git or required by
CI.

## GitHub Actions

`.github/workflows/ci.yml` runs on Ubuntu 24.04 with Node 22, Rust 1.97.0,
rustfmt, clippy, LLVM 22.1, and stable Chrome. It executes `npm ci`,
`check:fast`, and `check:fixture-e2e`. The workflow has read-only repository
permissions and does not run private-corpus checks.

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
