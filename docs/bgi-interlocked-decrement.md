# BGI InterlockedDecrement

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-interlocked-decrement`
- Previous capability checkpoint: `bgi-lc-map-string-w`
- First unsupported capability: `kernel32-get-module-file-name-a`

## Purpose

The cumulative BGI session decrements the same process-owned LONG twice. This
checkpoint connects the reached one-argument ABI to checked guest memory and
continues through existing increment and critical-section contracts.

```text
InterlockedDecrement(0x100F08E0): 1 -> 0, result 0
InterlockedIncrement(0x20000A20): 0 -> 1
EnterCriticalSection(0x100FF708)
InterlockedDecrement(0x100F08E0): 0 -> -1, result -1
InterlockedIncrement(0x20000A20): 1 -> 2
LeaveCriticalSection(0x100FF708)
  -> next stop is GetModuleFileNameA(NULL, 0x100FF520, 260)
```

## Native evidence

The focused `interlocked-decrement` observer retains direct BGI calls and a
guarded self-oracle. It records the pointed LONG before and after each call,
signed result, LastError, alignment, guards, and both immediate consumers.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-interlocked-decrement-20260714-r3.jsonl `
  --seconds 0.2 --kill-after --observe interlocked-decrement
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 51,155 | 70 | 9 | `870cd630131c437bc65ef0818bacb87b4599e01c87bf19ed93d6d9a071c66be9` |
| Confirmation | 51,148 | 70 | 9 | `5bc3f4b68f74964e55def6bf33a31fbf4bd6fc87134c5d3f29a135a6a24d9c25` |

Both captures have contiguous sequence numbers and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. Each contains two BGI
calls, three oracle calls, and four incidental `GetProcAddress` pairs.

The BGI target is RVA `0xF08E0` and is aligned to four bytes. Calls return at
RVA `0xA7201` and `0xA72B8`. Both runs agree on `1 -> 0`／result `0`, followed by
`0 -> -1`／result `-1`; LastError is preserved (`87 -> 87` and `0 -> 0`). The
first consumer tests EAX for zero before releasing or replacing a retained
process pointer. The second tests EAX, later compares it with `-1`, and selects
the release／error path. Result values are therefore consumed rather than
discarded.

The guarded aligned oracle independently fixes:

| Before | After / result | LastError |
|---:|---:|---:|
| 1 | 0 | `0 -> 0` |
| 0 | -1 | `87 -> 87` |
| `INT_MIN` | `INT_MAX` | `0x12345678 -> 0x12345678` |

Leading and trailing 16-byte guards remain unchanged in every case. Native
pointer values and image load addresses vary and are not imported. Windows
requires interlocked variables to be naturally aligned; undefined unaligned
native behavior is not promoted into the runtime contract.

## Runtime contract

`kernel32.interlocked-decrement.v1` uses one required, four-byte-aligned
`InOut LONG` and returns the wrapping decremented LONG bits in EAX. The x86
stdcall frame removes four argument bytes, so ESP advances eight bytes including
the return address. Success preserves selected-thread LastError.

The ABI gateway preflights the full four-byte read and write span before the
handler runs. The shared checked interlocked helper reads exactly one LONG,
applies `i32::wrapping_add(-1)`, and emits exactly one guest write. NULL,
unaligned, unmapped, and read-only targets fault before guest bytes, runtime
state, registers, or LastError change.

No compatibility-policy selector is added. The current LCMapStringW policy is
retained only to enable the preceding NLS calls; target pointer, values, callers,
and order are not semantic branches. The update is indivisible in the current
single cooperative Runtime Worker lane. Cross-Worker shared-memory atomicity is
not claimed.

The public semantic test covers both reached transitions, `INT_MIN` wrapping,
ABI cleanup, LastError preservation, exact writes, and mutation-free NULL,
unaligned, unmapped, and read-only rejection.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-interlocked-decrement
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 38 / 119 | 38 / 119 |
| Instructions at next stop | 15,863 | 15,863 |
| Writeback batches / pages | 120 / 187 | 1,514 / 2,028 |
| Host/mapping deltas | 50 | 50 |
| Final generation | 199 | 1,361 |
| Memory SHA-256 | `83a1a4fd36f1f94efc7c0dcd83f1ff12f35729a00158ef7b48746f51896228f3` | same |
| Report bytes | 580,772 | 176,634 |
| Report SHA-256 | `6c905456197834909eb965cfa950c05a4587a91486b7b9c72f55253340bb9ff4` | `abd04b3cbd301d72e2253881f73ef2f54562537e9247505e2e3a1d59c443ffc9` |

The first decrement is host delta sequence `114`, generation `187 -> 188`,
payload SHA-256
`25d935f63b91d0707a2c69ca70a0d27ad2a336e280099bdf4857bf24227bf37d`.
The second is sequence `117`, generation `193 -> 194`, payload SHA-256
`11f320560d0c6ca418481dffc089df2c07efdb4a539ae4e7dc3cdec6dd39c8f9`.
Each delta writes one data-only image page without cache invalidation.

Chrome uses 2,029 slices, 1,909 budget yields, 1,687 Wasm crossings, and final
generation `1,361`. Success, cancel, and watchdog end in `stopped` with no
pending host call or callback. The runtime-core Wasm is 1,847,201 bytes with
SHA-256 `8ab87eccb369bb7d20fd61360330da322389eb7aac9f16521e284ed65b0c79e1`.

The separate 180-second workload completed 1,004 sessions with zero failure.
All 16 versioned thresholds passed: session p95 `175.615 ms`, slice p99
`0.865 ms`, maximum heartbeat gap `18.935 ms`, main heap slope
`-53,713.69 bytes/min`, Worker heap slope `1,003,838.54 bytes/min`, and backing
storage slope `743,809.75 bytes/min`. The 465,860-byte report has SHA-256
`0cf54a5cbb26e2a872f6616d413f08e8946f72f905b6bd548a6fb49999a5e8df`.

## Next structured stop

```text
KERNEL32.dll!GetModuleFileNameA(NULL, 0x100FF520, 260)
endpoint ID:       34
endpoint address:  0x7F000210
IAT:               0x100D1130
return:            0x100A84C6
instruction:       15863
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next focused observer must measure NULL-module selection, ANSI output,
length/termination, insufficient-capacity behavior, LastError, guards, and the
immediate consumer before adding the reached contract.

## Non-claims

This checkpoint does not implement other interlocked operations, cross-Worker
shared-memory atomicity, general multi-thread scheduling, `GetModuleFileNameA`,
file access, title rendering, audio, input, VFS, COM, or full play.
