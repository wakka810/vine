# Production callback return ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: production-owned callback return sentinel and one guest callback return
- Report schema: `vine.v86-callback-return-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint continues the fixed cdecl service at exact thunk ID `2`. An optional Rust core path allocates and seals a distinct callback-control region, emits its sentinel metadata with the existing semantic reflection, and acknowledges the resulting generation only after actual adapter invalidation. The host creates callback frame ID `1`, writes `[return sentinel, 42]` to checked harness stack space, executes guest target `0x10001030`, and stops at the exact sentinel with result `43`. It does not retire the frame or return through outer thunk ID `2`.

## 2. Runtime-owned sentinel

The callback mode adds one region without changing the existing import-thunk arena:

| Field | Value |
|---|---:|
| Sentinel ID | `1` |
| Address | `0x7F010000` |
| Owner | `3`, runtime |
| Reserved range | `[0x7F010000, 0x7F020000)` |
| Committed range | `[0x7F010000, 0x7F011000)` |
| Protection | RX |
| Slot size | `16` bytes |
| Slot bytes | `UD2`, then `HLT` padding |
| Page SHA-256 | `4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059` |

The import arena remains `[0x7F000000, 0x7F010000)`, owner `2`, with thunk IDs `1` and `2`. The separate control region prevents a callback return trap from being classified as an import service. Reserving, committing, and sealing owner `3` advances the pending generation from `6` to `9`.

Callback mode reflects seven production pages with content SHA-256 `45d980f7ab5aa3c98942777693438a9c780f5d6a4f0e86f2f8db63211a3af406`. The adapter uses the same 2 MiB physical pool with one page directory, three page tables, seven reflected pages, and one harness stack: 12 pages／48 KiB total. After state restore, `full_clear_tlb`, and `jit_clear_cache_js`, Rust acknowledges generation `9`, clears initial dirty state, and fetch-verifies the sentinel bytes through normal guest access.

The callback-specific mode is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:callback
```

`check:runtime:entry` and `check:runtime:cdecl` omit the optional control owner and retain their previous 4,831-byte and 5,501-byte report hashes.

## 3. Frame and execution result

Thunk ID `2` is reached with outer ESP `0x006FFFF0`, outer return `0x10001019`, callback target `0x10001030`, and value `42`. The callback frame is:

```text
frame ID       = 1
parent thunk   = 2
outer ESP      = 0x006FFFF0
frame ESP      = 0x006FFFE8
[frame ESP]    = 0x7F010000  callback return sentinel
[frame ESP+4]  = 42          cdecl argument
```

The target page is sealed and executable. Adapter stack writes require a mapped writable page and stay within `[0x006FF000, 0x00700000)`. After `add_one` executes `MOV`／`INC`／`RET`, exact sentinel classification yields:

| Field | Result |
|---|---:|
| Execute stops | `6` |
| Budget stops | `3` |
| Stop reason／error | structured `#UD` `0x106`／`-1` |
| EIP／previous EIP | `0x7F010000`／`0x7F010000` |
| EAX／result | `43` |
| ESP | `0x006FFFEC` |
| Callee-saved registers | unchanged |
| Wasm host imports | `microtick` once |

The four-byte ESP increase is the guest callback's `RET` consuming only the callback return sentinel. Its cdecl argument remains at the current ESP. Frame ID `1` and parent thunk ID `2` remain active at the stop.

The deterministic report is 7,028 bytes with SHA-256 `29ede77d61369e560197dedb35311e662e0d3c56a3c6ce5dadc9743c02c10771`.

## 4. Non-claims and next checkpoint

This is partial evidence for `REQ-CPU-01`, `REQ-CPU-03`, `REQ-CPU-04`, `REQ-ABI-02`, `REQ-DIAG-03`, `P1-CPU-01`, `P1-ABI-02`, `P1-DIAG-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

The callback-control range, owner number, ID, and slot encoding are version 1 checkpoint choices, not a final process-layout or generated-metadata decision. The one-page stack is harness-only. Callback frame writes mutate only adapter-reflected bytes; bidirectional dirty-page ownership and Rust synchronization remain absent. There is no general frame stack, nesting, reentrant service dispatch, cancellation, exception／SEH path, stack growth, or production bootstrap. The outer callback service is still active and its arguments remain on the guest stack.

The outer continuation is complete in [v86-outer-return-ingress.md](v86-outer-return-ingress.md). It retires frame ID `1`, returns result `43` through outer thunk ID `2` with EAX `43`, ESP `0x006FFFF4`, and EIP／previous EIP `0x10001019`, then executes the guest result／entry path. Exact process-entry completion is separately verified in [v86-entry-return-ingress.md](v86-entry-return-ingress.md); this callback-only report remains stopped at slot `1` with its original page hash.
