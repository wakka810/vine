# Production outer return ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: callback frame retirement, fixed outer cdecl return, and measured guest entry continuation
- Report schema: `vine.v86-outer-return-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint continues the exact callback-return stop recorded in [v86-callback-return-ingress.md](v86-callback-return-ingress.md). The host retires callback frame ID `1`, returns result `43` through outer cdecl thunk ID `2`, and resumes the guest at `0x10001019`. The guest cleans its two cdecl arguments, stores result `43`, computes entry result `0`, and returns through the harness-only stack value `0xDEADC0DE`. Because that address is deliberately unmapped, the measured boundary is a structured user-mode instruction-fetch page fault. A production-owned process-entry return sentinel is not installed in this checkpoint.

## 2. Outer frame retirement

The callback return state remains the prior contract:

| Field | Value |
|---|---:|
| Callback frame ID | `1` |
| Parent thunk ID | `2` |
| Callback sentinel | `0x7F010000` |
| Callback EAX／result | `43` |
| Callback ESP | `0x006FFFEC` |
| Outer ESP | `0x006FFFF0` |
| Outer return address | `0x10001019` |

The fixed cdecl service return consumes only the outer return address. The host restores:

```text
EAX          = 0x0000002B
ESP          = 0x006FFFF4
EIP          = 0x10001019
PREVIOUS_EIP = 0x10001019
frame ID 1   = retired
```

The two outer arguments remain above ESP for the guest caller to clean. This is the same cdecl division of responsibility used for thunk ID `1`; it is not a general signature or frame-stack implementation.

## 3. Measured entry continuation

The fixed guest continuation is:

```text
0x10001019  ADD ESP, 8
0x1000101C  MOV [0x10003000], EAX
0x10001021  MOV ECX, [0x10003000]
0x10001027  XOR EAX, EAX
0x10001029  CMP ECX, 43
0x1000102C  SETNE AL
0x1000102F  RET
```

The entry was initially called with stack value `0xDEADC0DE` at `0x006FFFFC`. Its final `RET` consumes that value and advances ESP to `0x00700000`. The next instruction fetch produces:

| Field | Result |
|---|---:|
| Total execute stops | `9` |
| Total budget stops | `5` |
| Stop reason | structured `#PF` `0x10E` |
| Page-fault error code | `4`, user read／fetch from non-present page |
| CR2／fault address | `0xDEADC0DE` |
| EIP／previous EIP | `0xDEADC0DE`／`0xDEADC0DE` |
| Final EAX／entry result | `0` |
| Final ESP | `0x00700000` |
| Guest result address／value | `0x10003000`／`43` |
| Callee-saved registers | unchanged |
| Wasm host imports | `microtick` once |

The continuation adds three execute stops and two budget stops to the callback-return checkpoint. The page fault is evidence that the guest completed its result path and consumed the harness return value; it is not being treated as a process exit.

The deterministic report is 8,124 bytes with SHA-256 `de6ce28c1aa6f990fe211abd81c71415a0c908fb7a47ba6651ad0aa355cdd841`. The entry-only, cdecl-only, and callback-return reports remain byte-identical at 4,831, 5,501, and 7,028 bytes respectively.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:outer-return
```

## 4. Non-claims and next checkpoint

This is partial evidence for `REQ-CPU-01`, `REQ-CPU-03`, `REQ-CPU-04`, `REQ-ABI-01`, `REQ-ABI-02`, `REQ-DIAG-03`, `P1-CPU-01`, `P1-ABI-01`, `P1-ABI-02`, `P1-DIAG-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

The one-page stack, flat CPL3 state, fixed cdecl service, and `0xDEADC0DE` return value remain harness-only. The adapter stack and guest result mutation are not synchronized back into Rust `GuestMemory`. There is no general callback-frame stack, nested／reentrant callback policy, callback fault unwind, LastError, exception／SEH delivery, PEB／TEB, production initial stack, process exit lifecycle, or BGI execution.

The production-owned process-entry sentinel continuation is complete in [v86-entry-return-ingress.md](v86-entry-return-ingress.md). It places typed sentinel ID `2`／`0x7F010010` on the initial stack and classifies exact completion while preserving result `43`, entry EAX `0`, and ESP `0x00700000`. This outer-only report still verifies that the unmapped `0xDEADC0DE` path remains a structured fault.
