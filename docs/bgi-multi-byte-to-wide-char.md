# BGI MultiByteToWideChar

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-multi-byte-to-wide-char`
- Previous capability checkpoint: `bgi-get-cp-info`
- First unsupported capability: `kernel32-get-string-type-w`

## Purpose

The cumulative BGI session uses the code-page table prepared after `GetCPInfo`
as a fixed-length Windows-932 source. This checkpoint performs both the size
query and the immediately reached conversion through one checked semantic
contract.

```text
MultiByteToWideChar(932, 1, 0x006FFDD4, 256, NULL, 0)
  -> 256 UTF-16 units, LastError remains 87
  -> caller allocates a bounded 512-byte stack destination
MultiByteToWideChar(932, 1, 0x006FFDD4, 256, 0x006FF738, 256)
  -> 256 UTF-16 units / 512 bytes, LastError remains 87
  -> next stop is GetStringTypeW(1, 0x006FF738, 256, 0x006FF9D4)
```

## Native evidence

The focused `multi-byte-to-wide-char` observer retains direct BGI calls and a
controlled self-oracle. It treats the API as its documented six-argument x86
stdcall contract; the seventh stack word seen by the preceding unsupported
trap was caller-local data, not an API argument. The observer records bounded
source bytes, UTF-16 destination units, LastError, the first BGI return-site
consumer, and four bounded EBP frames.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-multi-byte-to-wide-char-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe multi-byte-to-wide-char
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 50,395 | 84 | 16 | `99d1e7d520a13f840cc5d21885beb392c3ef018a62036c4af2573a8866192275` |
| Confirmation | 50,419 | 84 | 16 | `15c9640167a33d93146cd0b014eddf2efa78793c53c334d391c8d5a2f721cf49` |

Both traces have contiguous sequence numbers and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. Pointer and thread
values vary and are not imported. The target source content is stable at 256
bytes, FNV-1a `0x80472A65`, with no NUL byte:

| Input byte index | Stored source byte |
|---|---|
| `0` | space (`0x20`) |
| `1..128` | same byte |
| `129..159` | space |
| `160..223` | same byte |
| `224..252` | space |
| `253..255` | same byte |

At BGI RVAs `0xAD691` and `0xAD6FB`, query and conversion both return `256`.
One run enters with LastError `0`, the other with `87`, and each value is
preserved. The 512 output bytes are identical across native runs and the Rust
runtime, with SHA-256
`4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`.
In addition to ASCII/control and half-width katakana mappings, native
Windows-932 maps standalone byte `0xA0` to `U+F8F0` and `0xFD..0xFF` to
`U+F8F1..U+F8F3`. WHATWG Shift_JIS replacement behavior is therefore not a
complete oracle for this API.

The self-oracle fixes these general semantics:

| Call | Result / effect | LastError |
|---|---|---|
| fixed `A 82 A0 00`, query | 3 units | preserves `87` |
| fixed conversion | `[U+0041,U+3042,U+0000]` | preserves `88` |
| NUL-terminated query | 3 units | preserves `89` |
| one-unit destination | returns 0 after writing `U+0041` | `122` |
| zero source count | returns 0, no write | `87` |

The first return continuation checks the query result, bounds `2*result+8`,
uses the small stack-allocation path, and invokes the conversion with the same
source and exact returned capacity. This establishes the consumer relation;
it does not make the caller RVA or the 256-byte reached sample part of Win32
semantics.

## Runtime contract

`kernel32.multi-byte-to-wide-char.v1` has six stdcall arguments and callee
cleanup of 28 bytes including the return address. The source and destination
are dynamic checked spans. Query requires `(destination=NULL, capacity=0)`;
conversion requires a nonnull destination and positive capacity. Success
preserves selected-thread LastError. Zero/invalid shape and overlap return
zero with `ERROR_INVALID_PARAMETER`; an insufficient destination writes the
native-observed complete prefix and returns zero with
`ERROR_INSUFFICIENT_BUFFER`.

Policy `kernel32-multi-byte-to-wide-char-v1` adds only allowed flags `[1]` and
a 65,536-byte source bound to the existing policy-owned Windows-932 code page.
It contains no BGI pointer, 256-byte reached value, caller RVA, call order, or
next-stop identity. Explicit CP `932` is enabled; `CP_ACP`, other code pages,
and other flags stop before mutation. The decoder uses Windows-932 pair rules
and the four measured private-use single-byte mappings rather than importing
the host locale.

The public semantic test fixes the native 256-unit table, query/conversion,
Japanese pair and NUL handling, partial insufficient-buffer write, invalid
count, unsupported selector/flags, LastError, ABI cleanup, and mutation-free
rejection without loading target evidence.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-multi-byte-to-wide-char
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 35 / 94 | 35 / 94 |
| Instructions at next stop | 5,894 | 5,894 |
| Writeback batches / pages | 95 / 150 | 700 / 752 |
| Host/mapping deltas | 37 | 37 |
| Final generation | 161 | 751 |
| Memory SHA-256 | `eb77024dd956d49c9a1a9f89a0e248220cd6ab6850203978134aeb93b8c4e811` | same |
| Report bytes | 423,814 | 91,017 |
| Report SHA-256 | `97655782894eca5f4e97a095d1ee3b64619e4f5ecf076c2be91d9b79ca045718` | `5ea486210af2cb3ce6e4c8e25b3d430ac8b3265bb42991151aacb510121b8774` |

The conversion output is one 512-byte host delta, sequence `94`, generation
`159 -> 160`, payload SHA-256
`be1040f1774da4926f9f0162ff5f0d72360d3af1ccb2afadd0a2e0c30909b73a`.
Chrome uses 772 slices, 677 yields, 835 Wasm crossings, and final generation
`751`; cancel and watchdog leave no pending host call or callback and detach
the process modules.

The separate 180-second workload completed 1,482 sessions with zero failure.
All 16 versioned thresholds passed: session p95 `154.45 ms`, slice p99
`0.675 ms`, maximum heartbeat gap `19.71 ms`, main heap slope
`-125,683.34 bytes/min`, and Worker heap slope `-1,420,395.94 bytes/min`.
The 671,754-byte benchmark report has SHA-256
`6138531c14bfb74bad77a59ef761c0c750a46f5925fb15d8986c2549a4b531c1`.
The runtime-core Wasm is 1,818,865 bytes with SHA-256
`302251acc51cec4f749009ead466877395870b66581c9fbd08a58303be754ba1`.

## Next structured stop

```text
KERNEL32.dll!GetStringTypeW(1, 0x006FF738, 256, 0x006FF9D4)
endpoint ID:       108
endpoint address:  0x7F0006B0
IAT:               0x100D1258
return:            0x100AD70D
instruction:       5894
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must first measure the requested `CT_CTYPE1` fields,
source length behavior, output span, result, LastError, and immediate consumer.

## Non-claims

This checkpoint does not enable `CP_ACP`, other conversion flags/code pages,
`GetStringTypeW`, general locale tables, normalization, host locale import,
later text APIs, title rendering, audio, input, VFS, COM, or full play.
