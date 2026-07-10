# v86 source adapter spike

## Status

- Source revision: `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`
- Rust: `1.97.0` (`2d8144b7880597b6e6d3dfd63a9a9efae3f533d3`)
- LLVM／Clang: `22.1.8`
- Target: `wasm32-unknown-unknown`
- Result: bounded interpreter、release exception stop、versioned CPU state round trip verified

This is a disposable Phase 0 adapter experiment, not a vendored v86 fork or an accepted production engine.

## Patch

The preparation command applies:

- [`patches/v86-bounded-execution.patch`](patches/v86-bounded-execution.patch): bounded interpreter and structured stop reasons
- [`patches/v86-state-module.patch`](patches/v86-state-module.patch) and [`v86/vine.rs`](v86/vine.rs): versioned 640-byte CPU state buffer

The adapter adds these Wasm exports without adding host imports:

```text
vine_execute_budget(max_instructions) -> stop_reason
vine_get_stop_error_code() -> i32
vine_cpu_state_{version,size,ptr,save,restore}
```

Stop reasons used by the spike are:

| Value | Meaning |
|---:|---|
| `1` | caller instruction budget reached |
| `2` | CPU halted |
| `0x100 + n` | CPU exception vector `n` |

The adapter disables and clears generated-code JIT state before its first interpreter run. The normal v86 execution path is unchanged when `vine_execute_budget` is not active.

## Reproduction

One command clones the fixed source revision, installs the workspace-local Rust toolchain when needed, applies both overlays, builds fallback Wasm, and runs the contract check. On Windows, `make` and `sh` from MSYS2 must be on `PATH`; the preparation script uses the workspace-local `rust-lld.exe` directly because the upstream Python linker wrapper is not a Windows executable.

```sh
npm run prepare:engine
```

The check writes `.work/v86-adapter-spike/report.json` and verifies:

- a zero-instruction budget returns without executing
- a same-page infinite loop stops after exactly 7 instructions and resumes for exactly 5
- CPL3 divide error, invalid opcode, general protection, and page fault return structured exception reasons
- CPL0 `hlt` returns the halt reason
- GPR、x87、XMM state survives one save／reset／restore／resume sequence

## Observed artifact

The 2026-07-10 local build produced the following evidence. Compiler output hashes are observations, not a cross-platform reproducibility promise.

| Property | Value |
|---|---|
| Size | 1,882,836 bytes |
| SHA-256 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Imports | 22 |
| Exports | 809 |
| Source build time | approximately 24 seconds total (Rust compile: 21 seconds) |

## Remaining work

The state layout is explicit, but guest physical memory remains contiguous and initial CPU/RAM setup still uses private offsets pinned to this artifact. The external W1 harness now verifies exact owned `UD2` thunks, one cdecl guest callback, and callback/entry return sentinels for the fixed `abi-smoke.exe`. General thunk metadata, broader calling conventions, nested callbacks, sparse memory, and Chromium Worker integration remain unimplemented.
