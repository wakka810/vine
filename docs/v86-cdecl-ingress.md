# Production cdecl service ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: first production-owned cdecl thunk dispatch and guest resume
- Report schema: `vine.v86-cdecl-service-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint continues the production-memory entry contract from exact thunk ID `1`. It classifies the trap through the ordered Rust-owned thunk metadata, checked-reads the fixed cdecl stack frame from reflected guest memory, computes `fixture_add(19, 23) = 42`, updates only the versioned CPU state ABI fields required to simulate the callee return, and resumes guest code. It stops at exact thunk ID `2` with callback target／value context before constructing or invoking a guest callback.

## 2. Metadata and mode boundary

The `vine.v86-memory-reflection` version 1 offer retains its existing `firstThunk` field and now also includes all owned thunks in stable plan order. This is an additive schema change. The controlled image reports:

| ID | Address | IAT | Symbol |
|---:|---:|---:|---|
| 1 | `0x7F000000` | `0x10002034` | `vine_fixture.dll!fixture_add` |
| 2 | `0x7F000010` | `0x10002038` | `vine_fixture.dll!fixture_call_callback` |

`npm run check:runtime:entry` remains the previous entry-only mode and retains its 4,831-byte report hash. The new mode is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:cdecl
```

Both modes use the same Rust reflection／acknowledgement session, six production pages, 11 adapter physical pages, generation `4`, one harness stack page, state ABI, instruction budget, and slice-boundary execute gate.

## 3. Fixed cdecl return

At thunk ID `1`, the structured `#UD` state is:

| Field | Value |
|---|---:|
| EIP／previous EIP | `0x7F000000` |
| ESP | `0x006FFFF0` |
| `[ESP]` return | `0x1000100A` |
| `[ESP+4]` left | `19` |
| `[ESP+8]` right | `23` |

The fixed service returns through the 640-byte CPU state ABI with:

```text
EAX = 42
ESP = 0x006FFFF4
EIP = PREVIOUS_EIP = 0x1000100A
```

Advancing ESP by 4 models the callee's `RET`: it consumes only the guest return address. The cdecl caller remains responsible for removing its two arguments. Guest code performs that cleanup, validates result `42`, and issues the next indirect call through IAT `0x10002038`.

## 4. Callback-service ingress

The continuation takes four execute stops and two budget stops in total. The second structured `#UD` is classified only because EIP exactly matches owned thunk ID `2`:

| Field | Result |
|---|---|
| EIP／previous EIP | `0x7F000010`／`0x7F000010` |
| EAX | `42` |
| ESP | `0x006FFFF0` |
| Return address | `0x10001019` |
| Callback target | `0x10001030` |
| Callback value | `42` |
| Service | `vine_fixture.dll!fixture_call_callback` |

The callback target lies in the sealed executable image page. The runner does not infer a service from an address range or title-specific EIP; both traps must match exact `ImportThunk` records. Callee-saved EBX／EBP／ESI／EDI sentinels remain unchanged, and the only Wasm host import called is one `microtick`.

The deterministic report is 5,501 bytes with SHA-256 `5ee8871673b667a097c2b25be4ca2f810555785ad4e4a563b9c43681c968d953`.

## 5. Non-claims and next checkpoint

This is partial evidence for `REQ-CPU-01`, `REQ-CPU-03`, `REQ-CPU-04`, `REQ-ABI-01`, `REQ-ABI-02`, `REQ-DIAG-03`, `P1-CPU-01`, `P1-ABI-01`, `P1-ABI-02`, `P1-DIAG-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

This checkpoint implements one fixed two-argument cdecl development service. It is not a generated ABI gateway and does not cover stdcall、thiscall、fastcall、64-bit values、floating-point／structure returns、varargs、LastError、SEH, cancellation, or reentrancy. The service reads adapter-reflected bytes; bidirectional runtime dirty-page ownership is still absent. The callback target is validated but no callback frame, callback return sentinel, outer host frame, or nested unwind is created. The harness-only stack and process state remain outside production bootstrap.

The callback return continuation is complete in [v86-callback-return-ingress.md](v86-callback-return-ingress.md): it allocates a separate production-owned sentinel, pushes `[return sentinel, value]` in frame ID `1`, enters target `0x10001030`, and stops at the exact sentinel with result `43`. The outer continuation is complete in [v86-outer-return-ingress.md](v86-outer-return-ingress.md): it retires that frame, returns result `43` through thunk ID `2`, and executes the guest result／entry path to its measured return boundary.
