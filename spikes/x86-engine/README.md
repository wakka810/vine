# v86 source adapter spike

## Status

- Source revision: `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`
- Rust: `1.97.0` (`2d8144b7880597b6e6d3dfd63a9a9efae3f533d3`)
- LLVM／Clang: `22.1.8`
- Target: `wasm32-unknown-unknown`
- Result: bounded interpreter、release exception stop、versioned CPU state round trip、high-address paging／R／W fault measurement verified

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

After `npm run check:toolchain`, `npm run check:runtime:memory` writes `.work/v86-memory-spike/report.json`. It maps a fixed PE32 image at `0xF1000000` through low physical pages and checks present／R／W faults、cross-page no-partial-write、protect／remap invalidation. It also records that this v86 revision executes a PE non-executable section and does not emit the instruction-fetch PFEC bit. [ADR-003](../../docs/adr-003-guest-memory.md) keeps the resulting paging candidate provisional.

`npm run check:runtime:entry` holds a production Rust `GuestMemory` in a bounded probe session, reflects six sealed semantic pages without exposing Rust physical-frame indices, and acknowledges generation `4` only after v86 state restore、`full_clear_tlb`、and `jit_clear_cache_js`. It then executes the relocated controlled entry on one harness stack page and stops at production thunk ID `1`. The versioned result and non-claims are recorded in [v86-entry-ingress.md](../../docs/v86-entry-ingress.md).

`npm run check:runtime:cdecl` uses the same session and ordered production thunk metadata to return fixed `fixture_add(19, 23) = 42` through the versioned CPU state ABI. Guest code performs cdecl argument cleanup and reaches exact thunk ID `2` with callback target `0x10001030`／value `42`. It does not construct or invoke the callback; the result is recorded in [v86-cdecl-ingress.md](../../docs/v86-cdecl-ingress.md).

`npm run check:runtime:callback` optionally adds a separate Rust-owned RX callback-control page, acknowledges generation `6`, creates frame ID `1` on checked harness stack space, and executes target `0x10001030` to exact sentinel `0x7F010000` with result `43`. It stops before outer service return; the result is recorded in [v86-callback-return-ingress.md](../../docs/v86-callback-return-ingress.md).

`npm run check:runtime:outer-return` continues that exact stop, retires frame ID `1`, returns result `43` through outer cdecl thunk ID `2`, and executes the guest result／entry continuation. It verifies result `43`, entry EAX `0`, ESP `0x00700000`, and the structured instruction-fetch page fault at the harness-only unmapped return `0xDEADC0DE`. The result is recorded in [v86-outer-return-ingress.md](../../docs/v86-outer-return-ingress.md).

`npm run check:runtime:entry-return` adds process-entry sentinel ID `2` at control slot `0x7F010010`, places that exact address on the initial stack, and classifies the final `#UD` as successful entry completion with EAX `0`, ESP `0x00700000`, and fixture result `43`. The outer-only unmapped fault remains a separate regression contract. The result is recorded in [v86-entry-return-ingress.md](../../docs/v86-entry-return-ingress.md).

`npm run check:runtime:stack-bootstrap` moves the one-page controlled stack into Rust owner `4`, reflects generation `8`, and verifies the same completion without an adapter-local stack allocation or setup write. The result is recorded in [v86-stack-bootstrap-ingress.md](../../docs/v86-stack-bootstrap-ingress.md).

`npm run check:runtime:bootstrap-fs-first-stop` runs the import-free native TEB／PEB offset oracle, reflects its sealed image／control／stack pages, and fixes the source-adapter stop at CR2 `0x18`／PFEC `4` with FS base zero. The result is recorded in [v86-bootstrap-fs-first-stop.md](../../docs/v86-bootstrap-fs-first-stop.md).

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

The state layout is explicit, but adapter physical memory remains contiguous and memory-size setup still uses one private offset pinned to this artifact. W1／W2 harnesses verify fixed thunk／callback／window flow, the memory harness verifies high VA plus R／W behavior, and the production reflection harness returns one fixed cdecl service plus one guest callback to an exact sentinel. Outer frame retirement／service return、general region mutation／dirty-page synchronization、production stack／bootstrap、snapshot、general ABI、Chromium Worker remain unimplemented.
