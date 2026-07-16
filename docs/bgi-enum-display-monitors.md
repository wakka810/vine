# BGI `EnumDisplayMonitors` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-enum-display-monitors`
- Previous capability checkpoint: `bgi-get-system-info`
- First unsupported capability: `ole32-co-initialize`

This checkpoint introduces the first reached User32 display contract. Rust owns
one logical primary monitor and drives the synchronous guest callback without
copying native or browser display topology into guest-visible state.

## Native evidence

The focused x86 observer captures the outer API, every callback argument,
the callback return, LastError, and independent TRUE／FALSE callback oracles:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-enum-display-monitors-20260716-r2.jsonl `
  --seconds 1.2 --kill-after --observe enum-display-monitors
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 208,419 | 643 | 287 | `3b3f673eaf46dcc902483dc2bededa2eeea595ff4149f7743bd3bacf16275f0e` |
| Confirmation r2 | 153,699 | 465 | 200 | `8753e255371e544a10c118f17f60e06ec59d80b75bf546c8c3d92a65f43a172c` |

Both captures have zero diagnostics. BGI calls
`EnumDisplayMonitors(NULL,NULL,0x1006A700,0)` and returns to RVA
`0x0006A781`. Windows invokes the callback once with an opaque native
`HMONITOR`, NULL monitor DC, `RECT(0,0,1920,1080)`, and unchanged LPARAM 0.
The callback returns TRUE, the outer API returns TRUE, and LastError remains 0.

The independent `x86-null-dc-null-clip-callback-enumeration-v1` oracle uses
LPARAM values `0x12345678` and `0x87654321`. A TRUE callback returns outer TRUE;
a FALSE callback returns outer FALSE and stops after the first callback. Both
paths preserve their LastError sentinels (`87` and `0x12345678`), preserve
LPARAM, and receive a NULL monitor DC. Native `HMONITOR=0x00010001` is an opaque
capture identity and is not imported into Vine.

The BGI callback first allocates 24 bytes, copies the four RECT fields into the
new object, links it into process state, and returns TRUE. This establishes that
the outer API must remain suspended while an ordinary nested Win32 trap runs.

## Rust authority and compatibility policy

`RuntimeDisplays` privately owns the configure-once display profile, logical
monitor identity, and geometry. The controlled User32 policy selects profile
`single-monitor-1920x1080-v1`:

```text
monitor namespace: vine.logical-monitor.v1
primary handle:    0x4F010001
rectangle:         (0,0)-(1920,1080)
```

The logical handle namespace is independent of native HMONITOR, Win32 handle
tables, COM pointers, and browser resource IDs. The profile is planned and
validated before commit; duplicate configuration and invalid geometry do not
advance its state.

`user32.enum-display-monitors.v1` is a four-argument stdcall BOOL contract with
required callback, allowed same-thread reentrancy, and LastError preservation.
The current bounded variant accepts only NULL HDC and NULL clip. It validates
the executable callback target and profile consistency before mutation.

The generic callback preparer reserves one contiguous 36-byte stack band:

```text
allocation start  +0x00  RECT scratch (16 bytes)
callback ESP      +0x10  return sentinel 0x7F010000
                  +0x14  logical HMONITOR 0x4F010001
                  +0x18  monitor HDC = 0
                  +0x1C  pointer to RECT scratch
                  +0x20  LPARAM
```

Argument count, scratch size, x86 arithmetic, execute access, and the entire
writable span are preflighted. Only then does Rust push the callback frame and
write the contiguous bytes. Invalid target／scratch／stack, stale frame ID, and
duplicate completion tests preserve memory generation, callback depth, and the
frame allocator. Callback EAX zero becomes outer BOOL FALSE; any nonzero value
becomes TRUE.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-enum-display-monitors
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 50 / 1,293 | 50 / 1,293 |
| Instructions at next stop | 45,685 | 45,685 |
| Writeback batches / pages | 1,299 / 2,459 | 4,524 / 6,222 |
| Host deltas | 80 | 80 |
| Callback entries / maximum depth | 1 / 1 | 1 / 1 |
| Final generation | 1,408 | 3,250 |
| Memory SHA-256 | `8ec8022f983af336f4e1b015d9f1760c6db01c606e678059cd6996cf4b403753` | same |

Call 1,292 enters the callback at instruction 45,614 with allocation
`0x006FFF28`, callback ESP `0x006FFF38`, frame ID 1, and the 16-byte logical
RECT. Nested call 1,293 is the existing
`HeapAlloc(0x68010001,0,24)` at instruction 45,645 and returns
`0x200014E8`. The callback reaches its sentinel at instruction 45,681 and the
outer API resumes with BOOL 1, balanced stdcall ESP, LastError 0, and callback
depth zero.

Node and Worker reserve the outer sequence at callback entry, record nested
calls while it is suspended, and fill the reserved record on callback return.
This preserves call-entry order rather than completion order. The Worker also
applies any callback-completion memory delta before CPU resume.

The 5,279,639-byte Node report has SHA-256
`163bf3db96fd25ab6ffeb881f593bbad468c5edfbc9d2e5693bb330fa67f9033`.
Chrome `150.0.7871.115` reproduces every normalized call, guest write, stack
growth, next stop, and final memory digest in 6,304 slices, 5,005 budget yields,
and 5,906 Wasm bridge calls. The 774,829-byte browser report has SHA-256
`af1be4e3f67aebb6d5cde516cb756f667aee6950eb24c6308945a972c78faea4`;
success, cancellation, and watchdog finish with zero pending callbacks or host
requests.

The runtime-core Wasm is 2,266,327 bytes with SHA-256
`94074a8838fccfb11862f8f33418c5c74f4f1f6364e96163a4e86048e8744010`.
The unchanged 180-second threshold configuration completes 200 sessions with
zero failures and passes all 17 thresholds. Session p95 is `914.315 ms`, slice
p99 `2.435 ms`, heartbeat p99 `10.090 ms`, and maximum heartbeat gap
`32.315 ms`. The 949,816-byte benchmark report has SHA-256
`8062086b75017a1bd9e2c213f0e63e6da14a03c30b7a60c06b30c9d90880656e`.

## Non-claims

This checkpoint does not implement non-NULL HDC or clip rectangles,
multi-monitor enumeration, monitor work areas, DPI, display-device APIs,
display-mode changes, hot-plug, DOM screen discovery, or browser-window
topology. It does not claim arbitrary callback depth, callback SEH, cross-thread
enumeration, or all User32 callback APIs. The fixed 1920x1080 profile is a
versioned compatibility value for the reached consumer, not a native-host
measurement.

## Next structured stop

```text
OLE32.dll!CoInitialize(NULL)
endpoint ID:       301
endpoint address:  0x7F0012C0
IAT:               0x100D1544
return:            0x10087399
instruction:       45685
fault:             endpoint-contract-missing
cluster:           ole32-api-contract
```

This is a COM initialization gateway. The next checkpoint must establish the
thread apartment state, HRESULT／repeat-call semantics, and its relationship to
the existing Rust COM identity foundation before adding the leaf API contract.
