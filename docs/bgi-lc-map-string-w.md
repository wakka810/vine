# BGI LCMapStringW

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-lc-map-string-w`
- Previous capability checkpoint: `bgi-get-string-type-w`
- First unsupported capability: `kernel32-interlocked-decrement`

## Purpose

The cumulative BGI session maps the same 256-unit Windows-932-derived table to
lowercase and uppercase forms. This checkpoint implements the four directly
observed Japanese-locale query/conversion calls through one checked six-argument
contract.

```text
LCMapStringW(1041, LCMAP_LOWERCASE, 0x006FF708, 256, NULL, 0) -> 256
LCMapStringW(1041, LCMAP_LOWERCASE, 0x006FF708, 256, 0x006FF4F8, 256) -> 256
LCMapStringW(1041, LCMAP_UPPERCASE, 0x006FF728, 256, NULL, 0) -> 256
LCMapStringW(1041, LCMAP_UPPERCASE, 0x006FF728, 256, 0x006FF518, 256) -> 256
```

Each conversion feeds the existing `WideCharToMultiByte` path. The continued
session next stops at `InterlockedDecrement(0x100F08E0)`.

## Native evidence

The focused `lc-map-string-w` observer retains direct BGI calls and a guarded
self-oracle. It records all six arguments, bounded UTF-16 input/output, result,
LastError, two immediate consumers, and bounded x86 EBP frames.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-lc-map-string-w-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe lc-map-string-w
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 80,889 | 84 | 16 | `f6688a93a53991e866782e6cd9ae4cb8f6cf086a8b5ba42325eeea4376203d02` |
| Confirmation | 80,851 | 84 | 16 | `eb6507ae0d35080711a2ecd95472138b224d882c8b6d45c35aa5686f4bd3f61c` |

Both captures have contiguous sequence numbers and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. Each contains four
BGI calls, eight oracle calls, and four incidental `GetProcAddress` pairs.
Pointers vary and are not imported.

The BGI query return RVA is `0xA7F81`; conversion returns at `0xA8020`. All four
calls use locale `1041` (`ja-JP`), explicit length `256`, return `256`, and
preserve LastError `87`. Both source copies have FNV-1a `0x0F08DD60` and
SHA-256 `4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`.
They contain 188 ASCII and 68 non-ASCII BMP units with no NUL or surrogate.

| Mapping | Output FNV-1a | Output SHA-256 |
|---|---|---|
| lowercase (`0x100`) | `0x16715DE0` | `2dbfb1ab3f867c8f7cc233c63a3749f303dbf5b7471e5ddf888bb074b043242c` |
| uppercase (`0x200`) | `0x4B0046E0` | `34c6ffd61beff429b3655fa26536beb9f81f0baaca883c24ea5dfb4443c8f7d9` |

The guarded oracle uses input
`[A,Z,a,U+3042,U+30A2,U+FF21,U+FF41]`. Query and conversion return `7`; the
lowercase output is
`[a,z,a,U+3042,U+30A2,U+FF41,U+FF41]`. Explicit success preserves sentinels
`0` and `87`. NUL-terminated input returns `8` including the NUL and preserves
`88`; native Windows also treats count `-2` as NUL-terminated, returning `2`
and preserving `91`.

Failure evidence is mutation-free:

| Shape | Result | LastError |
|---|---:|---:|
| two-unit destination for seven units | 0 | 122 |
| zero source count | 0 | 87 |
| flags `0` | 0 | 1004 |
| NULL destination with capacity `1` | 0 | 87 |

All source snapshots and leading/trailing guards remain unchanged. Negative
source counts are measured but not enabled because the target reaches only
explicit positive lengths.

The query consumer stores the result, rejects zero, checks it against the
caller capacity, and either uses the provided destination or allocates a
bounded stack buffer before the conversion call. The conversion consumer tests
the result and passes the mapped buffer to `WideCharToMultiByte`. These relations
do not make the caller RVA, pointer, or 256-unit sample part of Win32 semantics.

## Runtime contract

`kernel32.lc-map-string-w.v1` has six stdcall arguments and callee cleanup of 28
bytes including the return address. Policy `kernel32-lc-map-string-w-v1`
enables locale `[1041]`, exact flags `[0x100,0x200]`, and a 65,536-unit source
bound. It contains no BGI pointer, reached length, caller RVA, call order, or
output table.

The handler accepts explicit positive lengths. Query is exactly
`(destination=NULL, capacity=0)`; conversion requires a nonnull destination with
capacity at least the source count. It performs host-locale-independent ASCII
case mapping, the measured fullwidth lowercase mapping, and identity mapping
for the observed control, whitespace, Japanese/CJK, halfwidth, and Windows-932
private-use domains. Success preserves selected-thread LastError. Insufficient
capacity returns zero／`ERROR_INSUFFICIENT_BUFFER`; zero count and NULL-with-
capacity return zero／`ERROR_INVALID_PARAMETER`, all before output mutation.

Negative count, other locale/flags, overlap, over-limit source, and unimplemented
code units stop before mutation with identity-bearing structured faults. The
public semantic test independently covers lowercase/uppercase query and
conversion, native representative units, capacity/zero/NULL failures, ABI
cleanup, LastError, disabled policy, and mutation-free unsupported paths.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-lc-map-string-w
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 37 / 113 | 37 / 113 |
| Instructions at next stop | 12,137 | 12,137 |
| Writeback batches / pages | 114 / 174 | 1,046 / 1,101 |
| Host/mapping deltas | 44 | 44 |
| Final generation | 187 | 887 |
| Memory SHA-256 | `00d38b3de81df69fb459a3ff8ad7f0b53e1e50317f1fc6b546efabab19f20b55` | same |
| Report bytes | 550,988 | 168,513 |
| Report SHA-256 | `d56849eac369c5ad5b946c06b27acba844b779e548795efcc464a67bc2c7a889` | `5cb490e3b70e28ed2df3abf5f1706ccb3dc19922176c1223b31cacbde5e20975` |

The lowercase output is host delta sequence `103`, generation `171 -> 172`,
payload SHA-256
`387148ebbbfe78efb69a41e9c0cde9408f237880a748e69f0765f6d8d2ad4b27`.
The uppercase output is sequence `112`, generation `183 -> 184`, payload
SHA-256
`1f5252fecc213757fa3e867aa9886d993f6ca748454b847df7ad211c55dfe83c`.

Chrome uses 1,560 slices, 1,446 budget yields, 1,207 Wasm crossings, and final
generation `887`. Success, cancel, and watchdog end in `stopped` with no pending
host call or callback. The runtime-core Wasm is 1,846,405 bytes with SHA-256
`45ac72d9ef51044820f8bf43edaadf4ebc07b97d22d2e33ba68234ef748347c8`.

The separate 180-second workload completed 1,200 sessions with zero failure.
All 16 versioned thresholds passed: session p95 `185.410 ms`, slice p99
`0.670 ms`, maximum heartbeat gap `18.285 ms`, main heap slope
`-346,425.58 bytes/min`, and Worker heap slope `1,135,716.73 bytes/min`. The
551,776-byte report has SHA-256
`56eaace78498bb5448123633d478e48d60109dd5dbc8ef708b577b9a8cf212c4`.

## Next structured stop

```text
KERNEL32.dll!InterlockedDecrement(0x100F08E0)
endpoint ID:       123
endpoint address:  0x7F0007A0
IAT:               0x100D1294
return:            0x100A7201
instruction:       12137
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

Only the first stack word after the return address is the documented LONG
pointer. The next focused observer must measure the initial/final LONG, atomic
result, wrapping behavior, LastError, alignment, and immediate consumer before
adding the reached decrement contract.

## Non-claims

This checkpoint does not enable arbitrary locales, combined mapping flags,
negative-length mapping, general Unicode casing, sort keys, normalization,
`InterlockedDecrement`, title rendering, audio, input, VFS, COM, or full play.
