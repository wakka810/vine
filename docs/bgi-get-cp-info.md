# BGI GetCPInfo

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-get-cp-info`
- Previous capability checkpoint: `bgi-private-heap-size-544`
- First unsupported capability: `kernel32-multi-byte-to-wide-char`

## Purpose

The cumulative BGI session reaches `GetCPInfo(932, output)` after creating and
consuming its 544-byte private-heap block. This checkpoint writes only the
measured active-code-page fields from versioned compatibility policy data.

```text
GetCPInfo(932, 0x006FF9C0)
  -> BOOL TRUE, LastError remains 87
  -> write 18 measured CPINFO field bytes
  -> BGI expands the two lead-byte ranges into its byte-class table
  -> existing LastError/TLS/FLS helpers continue
  -> next stop is MultiByteToWideChar(932, 1, ..., 256, NULL, NULL, 256)
```

## Native evidence

The focused `get-cp-info` observer retains only direct BGI calls and its
controlled self-oracle. It records 32 bounded output bytes before and after,
BOOL and LastError, the immediate return-site instructions, and four bounded
x86 EBP frames. The oracle places the output inside a 64-byte `0xA5` guard so
the written span is measured rather than inferred from a header declaration.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-cp-info-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe get-cp-info
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 29,775 | 68 | 8 | `5689d985a0d780efed7d449144347b73421125aec337e8570f7c8463dc795f46` |
| Confirmation | 29,849 | 68 | 8 | `b930cdafb0443759a3a6db3cbaf6b549ba327de2d85725cf35456c49ea4fc0ef` |

Both traces have contiguous sequence numbers, one BGI pair, three successful
oracle pairs, and zero hook, vtable, call-stack, agent-message, cleanup, or
oracle diagnostics. Native stack pointers differ (`0x00B4F578` and
`0x012FF70C`) and are not imported.

At BGI return RVA `0xA6D13`, both runs pass code page `932`, return `1`, and
preserve LastError `87 -> 87`. The BGI output storage begins as 32 zero bytes.
The returned prefix is:

```text
02 00 00 00  3F 00  81 9F E0 FC 00 00 00 00 00 00 00 00
```

The sentinel oracle proves that only those first 18 bytes are written. Bytes
18 through 31 and both surrounding guards remain `0xA5`. Across LastError
sentinels `0`, `87`, and `0x12345678`, every call returns `1`, the hook and
`SystemFunction` results agree, and LastError is preserved.

The measured fields decode as:

| Field | Value |
|---|---|
| `MaxCharSize` | `2` |
| `DefaultChar[2]` | `[0x3F, 0x00]` |
| `LeadByte[12]` | `[0x81,0x9F,0xE0,0xFC,0,0,0,0,0,0,0,0]` |

The return continuation tests BOOL, clears a 256-byte classification table,
then reads the first lead-byte pair and expands each inclusive range. The first
outer-frame return is RVA `0xA7115`. This establishes the consumer relation;
it does not make the caller RVA or table algorithm part of Win32 semantics.

## Runtime contract

`kernel32.get-cp-info.v1` has stdcall ABI with a `u32` code page and a required,
4-byte-aligned output pointer. Its checked output contract covers the 18 field
bytes observed above, returns `Bool32(TRUE)`, performs callee cleanup of 12
bytes including the return address, and preserves LastError.

Policy `kernel32-get-cp-info-v1` adds `activeCodePageInfo` beside the existing
Windows-932 ACP. It contains `maxCharSize`, two default bytes, and twelve lead
bytes, but no BGI pointer, caller, call order, or next-stop identity. Policy
validation requires an enabled ACP, bounded character size, ordered lead-byte
ranges, and trailing zero pairs.

The handler accepts only the explicit reached page equal to the policy ACP.
Other pages stop with `get-cp-info-code-page-unsupported`; a policy without
metadata stops with `get-cp-info-policy-disabled`. The generic ABI gateway
preflights the full output span. Non-932, disabled-policy, and cross-region
output tests all preserve runtime and guest memory before returning a structured
fault.

The successful output is host delta sequence `88`, generation `152 -> 153`, on
the stack page owned by owner `4`. Its payload SHA-256 is
`6581defd12dd1676cc0b42a7597d4a0c7d3e2ba5daf74e2167f8a083e662df4d`.
The consumer reaches the next helper at instruction `5703`; writeback sequence
`89` changes 307 stack bytes at generation `153 -> 154`, with payload SHA-256
`d71dced9e69c9bf404b67af6fef4ee760f2b5975b86c8ee00e02bb2a9c6dc545`.

Representative tests:

```powershell
cargo test -p vine-core --test api_dispatcher get_cp_info --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
npm run check:win32-policy
npm run check:windows-trace
```

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-cp-info
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 34 / 92 | 34 / 92 |
| Instruction at next stop | 5,793 | 5,793 |
| CPU writeback batches / dirty pages | 93 / 148 | 690 / 742 |
| Host mapping deltas | 36 | 36 |
| Final generation | 158 | 741 |
| Memory content SHA-256 | `1d0a6694d155aed6f9b12989c7d78ba0f6017c556749f218b9b131ac199eb389` | same |
| Report bytes | 407,009 | 79,411 |
| Report SHA-256 | `da95600680cdb922a2952a7045ac5ace1b6f2510f5f553773343854e2f2310a1` | `1e9d26be08af2bde80df8805a5287ac795a43bd310d2acfa37073e1c2e040b0c` |

Chrome used 759 slices, 666 budget yields, 822 Rust/Wasm bridge operations,
and 2,278 successful execution-state transitions. Success, cancellation, and
watchdog all end in `stopped` with no pending host request records.

The separate 180-second workload completed 1,587 sessions with zero failures.
All 16 versioned thresholds passed; session p95 was
`109.62999999523163 ms`, slice p99 was `0.4699999690055847 ms`, and maximum
heartbeat gap was `19.34500002861023 ms`. The 717,684-byte report has SHA-256
`ca23564a060d7963866b8232163fc89bc532a66e35d04838247435b780591a52`.

## Next structured stop

```text
KERNEL32.dll!MultiByteToWideChar(932, 1, 0x006FFDD4, 256, 0, 0, 256)
static endpoint ID: 130
endpoint address:   0x7F000810
IAT address:        0x100D12B0
return address:     0x100AD691
instruction:        5793
fault:              endpoint-contract-missing
cluster:            kernel32-api-contract
```

The next checkpoint must trace source content and length semantics, flags,
destination behavior, result, LastError, and immediate consumer before adding
only this reached Windows-932 conversion variant.

## Non-claims

This checkpoint does not enable `CP_ACP` selector zero, arbitrary code pages,
ACP mutation, host locale import, `GetCPInfoEx`, `MultiByteToWideChar`, later
heap teardown, title rendering, audio, input, VFS, COM, or full play.
