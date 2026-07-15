# BGI `GetSystemInfo` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-get-system-info`
- Previous capability checkpoint: `bgi-set-thread-affinity-mask`
- First unsupported capability: `user32-enum-display-monitors`

This checkpoint returns a coherent x86 virtual machine profile without
importing the native or browser host's CPU topology. It closes the reached
one-output-pointer `GetSystemInfo` call and advances BGI into display-monitor
enumeration.

## Native evidence

The focused observer records the exact x86 layout, guards around an independent
oracle buffer, LastError sentinels, and the first BGI consumer:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-system-info-20260716-r2.jsonl `
  --seconds 1.4 --kill-after --observe get-system-info
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 256,120 | 784 | 357 | `a64745f92c00a1523133e9b9ae38b434073008c7fe3f7a0d01f32208af5ed2c8` |
| Confirmation r2 | 221,171 | 667 | 299 | `95f2df7426f70a9d91e6d94b1b29bceb697de6730fdcc3178162046ea82fc022` |

Both captures have zero diagnostics. The independent
`x86-system-info-output-v1` oracle proves that exactly 36 bytes change, the
16-byte prefix and 12-byte suffix guards remain unchanged, all three outputs
are stable, and LastError sentinels `0`, `87`, and `0x12345678` are preserved.
The measured host output is:

| x86 field | Native value |
|---|---:|
| `wProcessorArchitecture` / `wReserved` | `0` / `0` |
| `dwPageSize` | `4096` |
| minimum / maximum application address | `0x00010000` / `0x7FFEFFFF` |
| active processor mask / processor count | `0x0000FFFF` / `16` |
| processor type / level / revision | `586` / `25` / `0x7502` |
| allocation granularity | `65536` |

Those topology and processor-version values identify the capture host; they are
not runtime inputs. The BGI call returns to RVA `0x00069F0A`. Its immediate
continuation reads the first output DWORD, copies it through a caller-owned
pointer, overwrites EAX with `1`, and returns. Thus the reached consumer uses
the architecture／reserved DWORD, not the host processor count or mask.

## Rust contract and policy

`kernel32.get-system-info.v1` is a VOID stdcall contract with one non-NULL,
four-byte-aligned 36-byte OUT structure. The generic dispatcher preflights the
entire span, preserves EAX and LastError, and applies one atomic guest write.

Policy `kernel32-get-system-info-v1` supplies the bounded PE32 profile:

```text
architecture=0, pageSize=4096
applicationAddress=0x00010000..0x7FFEFFFF
processorType=586, processorLevel=6, processorRevision=0
allocationGranularity=65536
```

It does not repeat topology. `dwActiveProcessorMask` comes from the existing
virtual thread-affinity process mask (`1`), and `dwNumberOfProcessors` is that
mask's popcount (`1`). `wReserved` is zero. The resulting 36 bytes are:

```text
000000000010000000000100fffffe7f01000000010000004a0200000000010006000000
```

Schema-v1 policies and the preceding schema-v2 policy omit `systemInfo` and
retain their prior shape. Validation requires the schema-v2 bounded PE32
address profile and an existing nonzero affinity mask. Public dispatcher tests
use synthetic mask `0b1011`, level `5`, and revision `0x1234` to prove the
handler is policy-driven rather than host-shaped. They also cover exact ABI
cleanup, EAX／LastError preservation, trailing guards, policy disablement, NULL,
read-only, and mapped-prefix／unmapped-suffix failure atomicity.

## Runtime and Browser Worker evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-system-info
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 49 / 1,291 | 49 / 1,291 |
| Instructions at next stop | 45,614 | 45,614 |
| Writeback batches / pages | 1,296 / 2,455 | 4,517 / 6,214 |
| Host deltas | 78 | 78 |
| Final generation | 1,403 | 3,241 |
| Memory SHA-256 | `651b3336011adb55b98e8bb13329e4bb368993a73d59ab878c65294636df92f5` | same |

Call 1,291 writes the 36 policy bytes at `0x006FFED8`, preserves LastError `0`,
and produces generation `1402` from `1401`. Its page delta SHA-256 is
`17edf8b735d23805592033c604371e1cf6a00f3ed5b152c0e0ec25bdd1562999`.
The caller continuation reaches the next stop with generation `1403`.

The 5,266,052-byte Node report has SHA-256
`ddcc44d2f9f21c11f6fe919a6c7a51fb018db01cc74e3b177f919811939c3c1c`.
Chrome `150.0.7871.115` reproduces every ordered call, output byte, memory
digest, and structured stop using 6,294 slices, 4,998 budget yields, and 5,894
Wasm bridge operations. The 773,580-byte browser report has SHA-256
`d4b4684807a37c5913f788a890673dec2453940ecd037f6780aa86ef61389444`;
cancel and watchdog teardown leave zero pending work.

The runtime-core Wasm is 2,240,075 bytes with SHA-256
`9e1496378aa63342267f1240c44f45f52565ba93ad7269d5d956b4f72b360af9`.
The 180-second gate completes 415 sessions with zero failures and passes all 17
versioned thresholds. Session p95 is `440.220 ms`, slice p99 `1.935 ms`,
heartbeat p99 `6.045 ms`, and maximum heartbeat gap `18.535 ms`. The
1,091,712-byte benchmark report has SHA-256
`64a7e8785555897bcd17be704d8d141cfa5063e3ac1ec58657be31c50c6dd84d`.

## Non-claims

This checkpoint does not expose host topology, control host scheduling, model
processor groups／NUMA／hot-plug, implement x64 `SYSTEM_INFO`, add process
affinity APIs, or establish physical-memory and OS-version profiles. The
single logical processor is a compatibility value consistent with the existing
virtual affinity authority.

## Next structured stop

```text
USER32.dll!EnumDisplayMonitors(0, 0, 0x1006A700, 0)
endpoint ID:       189
endpoint address:  0x7F000BC0
IAT:               0x100D13CC
return:            0x1006A781
instruction:       45614
fault:             endpoint-contract-missing
cluster:           user32-api-contract
```

This is a window／display subsystem gateway candidate: it introduces callback
enumeration and logical monitor geometry. Its next checkpoint must establish a
minimal vertical slice and callback/lifetime acceptance conditions before
adding an isolated API stub.
