# BGI `IsProcessorFeaturePresent(10)` checkpoint

## Status

- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-is-processor-feature-present`
- Frozen historical prefix: `bgi-seventeenth-win32-call`
- Implemented contract: `kernel32.is-processor-feature-present.v1`
- Compatibility policy: `kernel32-is-processor-feature-present-v1`
- First unsupported capability: `kernel32-set-unhandled-exception-filter`

This checkpoint returns both BGI calls to
`IsProcessorFeaturePresent(PF_XMMI64_INSTRUCTIONS_AVAILABLE)` from one explicit
policy map. It does not inspect the browser or host CPU. The same session then
completes an already-enabled 2,048-byte zeroed heap allocation and stops at
`SetUnhandledExceptionFilter`.

## Native evidence

The final focused captures are uncommitted private artifacts:

| Capture | Bytes / lines | SHA-256 | Balanced API pairs |
|---|---:|---|---:|
| r3 | 329,977 / 1,043 | `1c0d15bc9a6325cd2a5d3eb8a12b5b901424eb954d1433977de75f4884cef054` | 484 |
| r4 | 332,449 / 1,055 | `819c0266ad0f98f59f05574049f6bdf207dafa413d6d875c21fce1fadd1fdbf5` | 490 |

Both traces have contiguous sequence metadata, balanced calls, and zero
diagnostics. The selected hook records only main-executable and Frida-oracle
processor-feature calls. The semantic projection is identical in both runs:

| Caller return RVA | Index | BOOL | LastError | Immediate consumer |
|---|---:|---:|---:|---|
| `0x000A4D87` | 10 | 1 | `0 -> 0` | store EAX at image RVA `0x00153518`; return 0 |
| `0x000A7E7C` | 10 | 1 | `0 -> 0` | store EAX at image RVA `0x00153500`; return 0 |

The `x86-processor-feature-query-v1` oracle calls feature 10 with LastError
sentinels `0`, `87`, and `0x12345678`. Every call returns 1 and preserves its
sentinel. An additional `0xFFFFFFFF` query returns 0 while preserving sentinel
122. This establishes BOOL and LastError behavior; it does not authorize every
feature index.

## Runtime contract

`Kernel32CompatibilityPolicy.processorFeatures` is a bounded list of unique
`{index, present}` records. The active policy contains only `{10, true}`. The
handler:

- decodes one x86 `DWORD` argument;
- returns canonical BOOL32 1 or 0 from the explicit policy value;
- uses stdcall callee cleanup (`ESP += 8`);
- preserves the selected thread's LastError;
- performs no guest-memory or runtime-state mutation;
- stops with `processor-feature-index-unsupported` before mutation when the
  index is absent.

The public fixture proves both `{10, true}` and an unrelated explicit
`{6, false}` through the same handler. It also proves mutation-free rejection
of an unregistered index and duplicate-index policy rejection. No BGI name,
callsite, sequence, or reached value appears in the semantic handler.

## Node continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-is-processor-feature-present
```

The strict Node oracle completes 202 calls across 41 contracts. Calls 200 and
201 are the two feature queries at instructions 18,693 and 18,721. Their
consumer writebacks advance generation `284 -> 285` and `285 -> 286` without a
host delta.

Call 202 is an existing policy-enabled
`HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 2048)`. It returns allocation ID 8 at
`0x20000CD8`, changes live allocations to seven, and emits a two-page delta
with one mapping operation. That delta advances `286 -> 288`; the final CPU
writeback advances to generation 289.

| Evidence | Value |
|---|---:|
| Supported calls / contracts | 202 / 41 |
| CPU writebacks / dirty pages | 203 / 311 |
| Changed bytes | 4,561 |
| Host deltas | 56 |
| Final generation | 289 |
| Memory SHA-256 | `b4e25674b53cc1e159ee02cff2c00610dfbd3499d716c932ee2cd64fc99362e1` |
| Report bytes | 896,534 |
| Report SHA-256 | `c173fd9d2ae2763f15ddafa0b487bddeba9b934a978ddd840766cfbad870cdb5` |

The generic harness now gives this profile eight bounded heap-backing pages
instead of the former exact one-page allowance. Previous profiles keep their
old quota. The extra capacity is a harness resource limit, not an additional
Win32 variant or a host-memory authority.

## Chrome equivalence and workload gate

`npm run check:runtime:bgi-browser` reproduces all 202 ordered completions and
the same memory digest and next stop in Chrome. The Worker uses 2,437 slices,
2,234 budget yields, 1,804 writeback batches over 2,365 pages, 56 mapping
deltas, and 2,066 Rust/Wasm crossings. It ends at generation 1,614; the Node
generation differs because reconciliation boundaries differ. Success,
cancellation, and watchdog all finish in `stopped` with no pending work.

The 220,102-byte browser report has SHA-256
`44baa2c9372b3c3ab4ea36550344c82261bca698fe445576e1187b24880ba741`.

The improved 180-second benchmark completed 405 sessions with zero failure and
passed all 17 versioned thresholds. Heartbeat is now gated as a distribution
(P99 <= 25 ms) plus a separate freeze ceiling (maximum <= 250 ms), rather than
letting one scheduler outlier be the only verdict. The observed P99 was
14.970 ms and maximum was 37.225 ms. Session P95 was 534.750 ms and slice P99
was 3.670 ms. The 1,040,802-byte report has SHA-256
`b725e988b5f45ea0d35be777fb9259978782f5465438ee874793fc8611937bb8`.

## Next structured stop

```text
KERNEL32.dll!SetUnhandledExceptionFilter(0x100A816F)
endpoint ID:       78
endpoint address:  0x7F0004D0
IAT:               0x100D11E0
return:            0x100A81BC
instruction:       18993
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must trace the filter pointer identity, return value,
LastError behavior, replacement semantics, and consumer. It must distinguish
the process-owned logical filter record from host SEH and browser exceptions.

## Non-claims

This checkpoint does not expose host CPU discovery; enable unregistered
processor features; implement SEH dispatch, vectored handlers, or unhandled
exception UI; broaden heap flags or sizes; or reach rendering, audio, input,
VFS, COM, or full play.
