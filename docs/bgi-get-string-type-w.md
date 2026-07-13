# BGI GetStringTypeW

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-get-string-type-w`
- Previous capability checkpoint: `bgi-multi-byte-to-wide-char`
- First unsupported capability: `kernel32-lc-map-string-w`

## Purpose

The cumulative BGI session classifies the 256 UTF-16 units produced by the
reached Windows-932 conversion. This checkpoint implements only the observed
`CT_CTYPE1` domain through a checked four-argument contract.

```text
GetStringTypeW(1, 0x006FF738, 256, 0x006FF9D4)
  -> BOOL TRUE, LastError remains 87
  -> write 256 CTYPE1 WORD values / 512 bytes
  -> caller stores and tests BOOL, then continues through existing helpers
  -> next stop is LCMapStringW(1041, 256, 0x006FF708, 256, NULL, 0)
```

## Native evidence

The focused `get-string-type-w` observer retains direct BGI calls and a
controlled self-oracle. It records bounded UTF-16 input, WORD output, result,
LastError, leading/trailing guards, the first return-site consumer, and bounded
x86 EBP frames.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-string-type-w-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe get-string-type-w
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 54,930 | 76 | 12 | `579835aa3610b6ac7550d98598eeb465930ae4c71633fdcdd84401dee0b494de` |
| Confirmation | 54,934 | 76 | 12 | `ffca1c677d9c6a1b38af85c36e2bc6f98f922ec57a53935ac77bf613a1a14e2` |

Both captures have contiguous sequence numbers and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. At BGI return RVA
`0xAD70D`, both calls use information type `1`, an explicit length of `256`,
return TRUE, and preserve the incoming LastError (`0 -> 0` and `87 -> 87`).
Pointers and thread identities vary and are not imported.

The source has FNV-1a `0x0F08DD60` and SHA-256
`4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`.
It contains 188 ASCII and 68 non-ASCII BMP units, with no NUL or surrogate.
The 512-byte native output has FNV-1a `0x1AB401A9` and SHA-256
`8eea87edb52710ebeee687bb46f65962ffaf7ad306d9d0e98bc81929b1af6d84`.

The self-oracle fixes representative CTYPE1 words independently of BGI:

```text
[544,616,552,552,584,528,528,528,528,644,644,897,769,528,898,770,
 544,544,584,528,584,768,768,528,768,528,644,897,898,528,768,768,
 768,512]
```

These cover ASCII case/digit/hex, control and whitespace, punctuation,
Japanese/CJK, fullwidth and halfwidth forms, and the Windows-932 private-use
units. Successful calls preserve LastError sentinels `0`, `87`, and
`0x12345678`. Zero count returns FALSE with `87` without changing output.
Invalid information type returns FALSE with `1004`, also without output
mutation, and both guards remain unchanged.

Native Windows also accepts negative counts as NUL-terminated input. That shape
was measured to distinguish it from failure, but the target did not reach it;
it remains deliberately disabled rather than being inferred into this
checkpoint.

The immediate consumer stores BOOL and tests it around RVA `0xA71ED`, branching
to the success continuation. This proves that success is consumed; the caller
RVA, source address, and exact 256-unit sample are evidence, not Win32 semantic
branches.

## Runtime contract

`kernel32.get-string-type-w.v1` has four stdcall arguments and callee cleanup of
20 bytes including the return address. The checked source and output spans are
both `2 * sourceCount` bytes. Policy `kernel32-get-string-type-w-v1` enables
information type `[1]` and bounds explicit positive source lengths at 65,536
code units. It contains no BGI pointer, reached length, caller RVA, call order,
or output table.

The handler classifies the observed `CT_CTYPE1` code-unit domain without host
locale input and preserves selected-thread LastError on success. Zero count
models the measured FALSE／`ERROR_INVALID_PARAMETER` result. Negative count,
other information types, overlapping spans, over-limit input, and unimplemented
code units stop before mutation with an identity-bearing structured fault.

The public semantic test fixes representative native words, success and zero
count, ABI cleanup, LastError, disabled policy, unsupported type/negative
count/code unit/overlap, and mutation-free rejection without loading target
evidence.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-string-type-w
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 36 / 101 | 36 / 101 |
| Instructions at next stop | 7,675 | 7,675 |
| Writeback batches / pages | 102 / 159 | 726 / 780 |
| Host/mapping deltas | 39 | 39 |
| Final generation | 170 | 778 |
| Memory SHA-256 | `3c26a9fa98427249ef0f37546f25592d18d18e9ade8c07c4ebbd10e2ce6b9aa2` | same |
| Report bytes | 468,010 | 116,522 |
| Report SHA-256 | `00facfca5960a6e0e233deab536aa5d01e190d6e7f72e6be7caffb2e8997c425` | `3891d75e2041f12234bc769515f4933f4eff4301daabee7eaf6ed8800bca9a87` |

The classification output is host delta sequence `95`, generation `161 ->
162`, with payload SHA-256
`b4a11394007a1cbd18154b31d864421d2c9ec4062e83b88dafa18421e238dab6`.
The consumer writeback at sequence `96` and instruction `5967` changes 74 bytes
across stack and heap pages at generation `162 -> 163`; its payload SHA-256 is
`1a343abf4d9e2b61cf0924a9eb44ac80c77131a231ddeebbba1eb93e2a816809`.

Chrome uses 998 slices, 896 budget yields, 870 Wasm crossings, and final
generation `778`. Success, cancel, and watchdog end in `stopped` with no pending
host call or callback. The runtime-core Wasm is 1,831,323 bytes with SHA-256
`20e8a284d7a42bdea9474dfadac87d76827e14bc54a5d865f2a9e3a943e2e8c3`.

The separate 180-second workload completed 1,507 sessions with zero failure.
All 16 versioned thresholds passed: session p95 `114.345 ms`, slice p99
`0.630 ms`, maximum heartbeat gap `18.375 ms`, main heap slope
`20,640.90 bytes/min`, and Worker heap slope `2,006,291.21 bytes/min`. The
682,359-byte report has SHA-256
`9bc0a71f9970f2cb4621bed92dd7a56eb67cf77e71485f0c4080e3e0f02020e6`.

## Next structured stop

```text
KERNEL32.dll!LCMapStringW(1041, 256, 0x006FF708, 256, 0, 0)
endpoint ID:       96
endpoint address:  0x7F0005F0
IAT:               0x100D1228
return:            0x100A7F81
instruction:       7675
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The unsupported stack snapshot contains one additional following word
`0x0000CCCC`. `LCMapStringW` has six documented arguments, so that word is not
promoted to an API argument. The next focused observer must measure locale,
flags, source/output length modes, output/result, LastError, guards, and the
immediate consumer before any contract is added.

## Non-claims

This checkpoint does not enable negative-length `GetStringTypeW`, arbitrary
Unicode classification, `CT_CTYPE2`／`CT_CTYPE3`, host locale tables,
`LCMapStringW`, later text APIs, title rendering, audio, input, VFS, COM, or
full play.
