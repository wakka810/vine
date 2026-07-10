# Production entry return ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: production-owned process-entry return sentinel and exact successful entry completion
- Report schema: `vine.v86-entry-return-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint replaces the harness-only unmapped return measured in [v86-outer-return-ingress.md](v86-outer-return-ingress.md) with a typed process-entry return sentinel. The sentinel is owned and reflected by the Rust runtime, placed on the adapter's initial stack, and classified by exact kind, ID, and address after the guest finishes. The fixture result remains `43`, the entry result is `0`, and ESP is balanced at `0x00700000`.

## 2. Combined runtime control page

Entry-return mode installs two typed sentinel slots before sealing the existing runtime control owner:

| Field | Callback return | Process-entry return |
|---|---:|---:|
| Sentinel ID | `1` | `2` |
| Address | `0x7F010000` | `0x7F010010` |
| Owner | `3`, runtime | `3`, runtime |
| Slot bytes | `UD2`, then `HLT` padding | `UD2`, then `HLT` padding |
| Slot size | `16` bytes | `16` bytes |

Both slots are in the first RX page of reserved range `[0x7F010000, 0x7F020000)`. The combined page SHA-256 is `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`. Owner seal still produces generation `6`; adding the second initialization write does not add a commit or seal transition.

The session reflects seven production pages with content SHA-256 `b184b076d28abbf59401e35113aaeb30ca1f29b8cee590ff1d71897313bfb9db`. The adapter keeps the same page directory, three page tables, seven reflected pages, and one harness stack page: 12 physical pages／48 KiB total. After state restore, `full_clear_tlb`, and `jit_clear_cache_js`, Rust acknowledges generation `6` and fetch-verifies both exact slots.

Callback-only and outer-return modes continue to install only slot `1`. Their one-sentinel page SHA-256 `4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059` and previous reports remain unchanged.

## 3. Entry completion

The adapter writes process-entry sentinel address `0x7F010010` at initial ESP `0x006FFFFC`. The existing fixed execution sequence remains:

```text
entry -> thunk ID 1 -> fixed cdecl result 42
      -> thunk ID 2 -> callback frame ID 1
      -> callback sentinel ID 1 with result 43
      -> retire frame ID 1 and outer cdecl return
      -> fixture result 43 and entry result 0
      -> process-entry sentinel ID 2
```

The final measured state is:

| Field | Result |
|---|---:|
| Execute stops | `9` |
| Budget stops | `5` |
| Stop reason／error | structured `#UD` `0x106`／`-1` |
| Exact classification | process-entry return sentinel ID `2` |
| EIP／previous EIP | `0x7F010010`／`0x7F010010` |
| EAX／entry result | `0` |
| ESP | `0x00700000` |
| Guest result address／value | `0x10003000`／`43` |
| Callback frame ID `1` | retired |
| Callee-saved registers | unchanged |
| Wasm host imports | `microtick` once |

Only the exact process-entry metadata can classify this `#UD` as successful completion. The outer-return mode still places unmapped `0xDEADC0DE` on the same initial stack and verifies reason `0x10E`, PFEC `4`, and CR2／EIP `0xDEADC0DE`; arbitrary unmapped returns are not accepted as exits.

The deterministic report is 8,674 bytes with SHA-256 `2619711263b918c419b61483499da1bcdc7d02ab9fe261ba57546a356b39756c`. Earlier entry, cdecl, callback-return, and outer-return reports remain byte-identical at 4,831, 5,501, 7,028, and 8,124 bytes.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:entry-return
```

## 4. Non-claims and next checkpoint

This is partial evidence for `REQ-CPU-01`, `REQ-CPU-03`, `REQ-CPU-04`, `REQ-ABI-01`, `REQ-ABI-02`, `REQ-DIAG-03`, `P1-CPU-01`, `P1-ABI-01`, `P1-ABI-02`, `P1-DIAG-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

The control arena, owner number, slot IDs, and addresses are version 1 checkpoint choices. The initial stack page, flat CPL3 state, and CPU bootstrap remain adapter-owned harness state. The production sentinel does not turn its return into a Windows process exit, thread exit, or loader lifecycle. Adapter stack／result mutations are not synchronized back into Rust `GuestMemory`. There is no general callback stack, nested／reentrant policy, callback fault unwind, LastError, exception／SEH delivery, PEB／TEB, process parameters, production stack growth／guard, DLL lifecycle, or BGI execution.

The next checkpoint moves the controlled initial thread stack and its bootstrap record into Rust-owned guest memory. It must reflect the stack page through the same ownership protocol, remove the adapter-local stack allocation for that mode, place the exact entry sentinel through checked initialization, and preserve the completed execution state before adding PEB／TEB or process parameters.
