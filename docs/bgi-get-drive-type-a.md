# BGI `GetDriveTypeA` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-get-drive-type-a`
- Previous capability checkpoint: `bgi-get-system-default-lang-id`
- First unsupported capability: `user32-swap-mouse-button`

This checkpoint implements the reached A-through-Z drive scan as a semantic
leaf in the existing process-directory owner. Vine exposes only the virtual
process-image drive as fixed. It does not import native or browser drive
topology and does not create a VFS, mount table, file namespace, or storage
backend.

## Native evidence

The focused observer records the root-path bytes, BGI consumer, a four-variant
oracle, and a balanced completion event after the 26th BGI call:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-drive-type-20260716-r11.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event drive-type-scan-complete `
  --observe drive-type
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r10 | 68,100 | 128 | 37 | `d42ff6b0032bf52bf134c712886abd6a56c8b833156a6dbd5d92794e7a4d4919` |
| Confirmation r11 | 68,042 | 128 | 37 | `08fa728b429fb36ae6fc6a063048ba0a227fdc8235d2f69f7be8d77e6b14dd7a` |

Both accepted captures have contiguous event sequences, balanced call IDs, and
zero hook, vtable, call-stack, agent-message, or cleanup diagnostics. The
event-selected stop blocks the observed guest thread after the completed
`Z:\` leave, so termination cannot strand a later graphics or window API.
Runs r8／r9 were balanced but predated the correction that passes the original
`GetCurrentDirectoryA` Windows-932 buffer directly to the non-root oracle.

BGI calls `GetDriveTypeA` 26 times at return RVA `0x000602B2`, using one
four-byte stack buffer for `A:\0` through `Z:\0`. On the evidence host,
`C:\` and the native source drive `V:\` return `DRIVE_FIXED` (`3`); every
other root returns `DRIVE_NO_ROOT_DIR` (`1`). Those host drive letters are
artifact identity, not runtime policy.

The immediate consumer is:

```asm
xor ecx, ecx
cmp eax, 3
setne cl
mov dword ptr [ebx*4 + 0xa94358], eax
inc ebx
mov dword ptr [ebx*4 + 0xa943bc], ecx
cmp ebx, 0x1a
jl 0x9f02a0
```

It stores each result and a non-fixed flag, then continues until all 26 drive
letters have been classified.

The independent same-process oracle fixes the bounded general behavior used by
the handler. Current root and NULL current-drive queries return `3`; an
existing non-root current-directory path and relative `.` return `1`. The four
calls preserve LastError sentinels `0`, `87`, `122`, and `0x12345678`.

## Rust authority and API contract

`runtime/process.rs::RuntimeProcessDirectories` remains the sole authority for
process-image and current-directory identity. Its drive query derives a letter
from those already validated Windows-932 paths; it adds no stored drive list,
identity allocator, or mutable state. A NULL API argument selects the current
directory's drive. An exact alphabetic `X:\` or `X:/` root is fixed only when
it matches the process-image drive; all other bounded strings return no root.

`Kernel32CompatibilityPolicy.get_drive_type_a` enables only this
process-image-drive subset and supplies the maximum ANSI scan length. It does
not store the active drive letter or type. The active controlled bootstrap uses
`C:\Vine`, so occurrence 3 (`C:\`) is the sole fixed result in Node and
Chrome even though native evidence was captured from `V:`.

`kernel32.get-drive-type-a.v1` is a one-argument x86 stdcall returning U32. It
preserves LastError, writes no guest bytes, and mutates neither runtime nor
memory. Unterminated input reaches a resource-limit fault at the policy bound;
unmapped input reaches a memory fault; a disabled policy reaches
`get-drive-type-a-policy-disabled`. Each failure occurs before state mutation.
Public tests cover NULL, upper/lower-case and slash aliases, another drive,
non-root, relative, empty, unterminated, policy-disabled, ABI cleanup, and
owner-level current-drive derivation.

## Node, Chrome, and benchmark evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-drive-type-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 53 / 1,321 | 53 / 1,321 |
| Instructions at next stop | 46,215 | 46,215 |
| Writeback batches / pages | 1,327 / 2,513 | 4,592 / 6,290 |
| Host / mapping deltas | 80 | 80 |
| Final generation | 1,436 | 3,316 |
| Memory SHA-256 | `070ccf6100b1a0537a43fa250942b23f9e293cad712d78b1d2a3c9d9a3ad0433` | same |

Calls 1,296 through 1,321 occur every 14 instructions from 45,840 through
46,190. All use argument `0x006FFC4C`, return to `0x100602B2`, preserve
LastError 0, and write no guest bytes. Occurrence 3 returns `3`; the other 25
return `1`. The 5,388,302-byte Node report has SHA-256
`f639cd0b48a19ee1b62b1966ce8473dcf8cfc2cc8d4b60f3ed5fa6b1f43580a9`.

Chrome `150.0.7871.115` uses 6,378 slices, 5,051 budget yields, and 6,002
Wasm bridge calls. The 789,280-byte browser report has SHA-256
`19a92b027d37eea6add217655bb67d17a70d0ffc2b963bbd62f965888f183ea7`;
success, cancellation, and watchdog all retire pending callbacks and host work.
The runtime-core Wasm is 2,287,046 bytes with SHA-256
`48a26d0fceb0a7d7e44343a79ffd8451593305a37bab787627970500261b455c`.

The unchanged 180-second configuration completes 193 sessions with zero
failures and passes all 17 thresholds. Session p95 is `1,032.995 ms`, slice
p99 `2.800 ms`, heartbeat p99 `9.730 ms`, and maximum heartbeat gap
`34.075 ms`. The 957,901-byte benchmark report has SHA-256
`5f6216d80b05b2f1f5b25e2fa74bce990e1f3d95c05bd4b5bf484ac811596ba3`.

## Non-claims

This checkpoint does not implement removable, remote, optical, RAM-disk, or
unknown drive types; UNC paths; volume labels; per-drive current directories;
general DOS normalization; file or directory existence; case-insensitive
lookup; mounts; source/overlay resolution; browser storage; or either storage
adapter defined by ADR-010 and the ADR-006 oracle. Returning `DRIVE_NO_ROOT_DIR` for non-root and
unregistered paths is the measured bounded query behavior, not proof that a
general VFS would resolve those paths the same way.

## Next structured stop

```text
USER32.dll!SwapMouseButton(0)
endpoint ID:       166
endpoint address:  0x7F000A50
IAT:               0x100D1370
return:            0x10088A3B
instruction:       46215
fault:             endpoint-contract-missing
cluster:           user32-api-contract
```

The next checkpoint must capture return, LastError, caller consumption, and
whether this is a stateless query/mutation leaf inside the existing User32
policy or a new input/window responsibility boundary.
