# BGI SetHandleCount

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-set-handle-count`
- Previous capability checkpoint: `bgi-standard-error-file-type`
- First unsupported capability: `kernel32-command-line-a`

## Purpose

The cumulative BGI session reached `kernel32.dll!SetHandleCount(32)`. This
checkpoint returns only the policy-enabled count through the synchronous
`kernel32.set-handle-count.v1` contract.

```text
checked one-argument stdcall frame
  -> versioned count policy
  -> return 32, LastError preserved
  -> no runtime or guest-memory mutation
  -> GetCommandLineA() structured stop
```

General handle-table sizing, CRT descriptor state, console behavior, and file
I/O are not implemented.

## Native evidence

Two opt-in `--observe set-handle-count` captures agree:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 233,779 | 779 | `ce0ced493ed1af598d5f4b3e6b3aad3ab811b84991cc69fbfe4bf5d98b649a71` |
| Confirmation | 234,327 | 779 | `8f09c81e20837926d02d8584954641db726932eb7d7f1aa5eebb6ab9e152f4e8` |

Both observe BGI caller return RVA `0xA881F`, argument `32`, result `32`, and
LastError `87 -> 87`. The source fingerprint remains
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

The caller discards the API result immediately:

```text
0x004A8813  push dword ptr [0x005533E8]
0x004A8819  call dword ptr [0x004D1234]
0x004A881F  xor eax, eax
0x004A8821  pop edi
0x004A8822  pop ebx
0x004A8823  pop esi
0x004A8824  leave
0x004A8825  ret
```

The implementation does not import native handle-table state or infer behavior
from the host result.

## Runtime contract

[`kernel32-set-handle-count-v1.json`](../fixtures/runtime-policies/kernel32-set-handle-count-v1.json)
adds `setHandleCounts: [32]` to the preceding standard-handle and heap policy.
Historical policies omit the field and remain fail-closed. The semantic handler
contains no BGI caller, RVA, or exact-count branch: it validates the frame,
requires policy authorization, and returns the authorized count without state
mutation.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  set_handle_count_policy_returns_reached_count_without_mutation `
  -- --exact
```

The test fixes return `32`, one-argument stdcall cleanup, LastError preservation,
no guest write, no runtime／memory mutation, and default-policy rejection.

## Generic continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-handle-count
```

| Evidence | Result |
|---|---:|
| Implemented API contracts | 24 |
| Completed generic calls | 61 |
| `SetHandleCount` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| CPU writeback batches / dirty pages | 62 / 105 |
| CPU-changed bytes | 992 |
| Host delta batches | 30 |
| Final synchronized generation | 121 |
| Memory content SHA-256 | `3e0e3dc6fc0420af4cf1bc4233d8fc267311f3926abc18b91ce839ce2ff083b3` |
| Report bytes | 284,215 |
| Report SHA-256 | `91be62c168dd5296fa549a8bf553975853dfef506bcb3ce7670a7b8843f82f3b` |

The completion is sequence 61 at instruction 1,516 and emits no guest write or
host memory delta.

## Browser and performance

Chrome reproduces all 61 ordered completions and the same memory digest at
instruction 1,526. It uses 216 slices, 154 budget yields, 176 writeback batches
over 215 dirty pages, 271 Rust/Wasm bridge operations, and final generation 225.
The protocol v3 execution state machine records 649／4／3 transitions for
success／cancel／watchdog, with all paths stopped and zero pending request
records. Cancel remains at 8 instructions and the watchdog at 4 instructions.
The 57,409-byte Browser report has SHA-256
`9bf2e353cfe43ae400d7bdff37de5fb831c024faf9a8cc2319e08fc8ec277988`.

The unchanged 180-second version 1 gate completed 2,090 sessions with no
failure: p95 session latency 79.545 ms, p99 maximum slice 0.545 ms, main heap
slope 124,705 bytes/min, Worker heap slope 546,371 bytes/min, and Worker
backing slope 989,237 bytes/min. All 16 thresholds passed. The 927,780-byte
report has SHA-256
`f6e07c36cd189edf9bcd5bf6104b7499232609b58e044cb4c1c1d141d64d4558`.
Detailed evidence is in [bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Next structured stop

```text
kernel32.dll!GetCommandLineA()
static endpoint ID: 85
endpoint address:   0x7F000540
IAT address:        0x100D11FC
return address:     0x100A3786
instruction:        1526
fault:              endpoint-contract-missing
```

The next checkpoint must measure the native pointer relation, returned byte
string, LastError, and immediate consumer before connecting the existing
process-parameter authority to an ANSI command-line contract.
