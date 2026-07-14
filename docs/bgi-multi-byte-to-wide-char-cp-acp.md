# BGI MultiByteToWideChar CP_ACP

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-multi-byte-to-wide-char-cp-acp`
- Previous capability checkpoint: `bgi-initial-thread-stack-growth`
- First unsupported capability: `kernel32-set-current-directory-a`

## Purpose

The reached call converts the process image path after initial-thread stack
growth. This checkpoint enables the active-code-page selector, flags zero, and
NUL-terminated source form through the existing six-argument semantic handler.

```text
MultiByteToWideChar(CP_ACP, 0, 0x006FE0A4, -1, 0x006FD150, 780)
  -> 16 UTF-16 units for controlled `C:\Vine\BGI.exe\0`
  -> the returned count and 32 output bytes include the trailing NUL
  -> LastError remains 0
```

The native install path has a different length. Its value is evidence identity,
not a runtime semantic input.

## Native evidence

The focused observer records each distinct BGI callsite once, including its
immediate instructions and four bounded caller frames. The version 2 oracle adds
a `CP_ACP`／flags-zero NUL-terminated query and conversion while retaining the
explicit code-page 932／flags-one cases.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-multi-byte-to-wide-char-cp-acp-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe multi-byte-to-wide-char
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 80,561 | 88 | 18 | `a9f7c88d9ee5aa91c5a046bc9e84feea67682c3927611f6c6e1ea5903e0ca600` |
| Confirmation | 80,592 | 88 | 18 | `b66f55eddf91e399a0333fbfb43df64b6f5025f818a9bc0ef9f0d202b1891fb1` |

Both traces have contiguous sequence numbers and zero hook, vtable, call-stack,
agent-message, cleanup, or oracle diagnostics. Pointer and thread identities vary.
The item-30 call is pair `84/85`, return RVA `0x0002BB13`, in both captures.

The native source is 28 Windows-932 bytes including NUL, FNV-1a
`0x0A2D1C1D`, SHA-256
`72472c80161f0c142cee81058bcdfa1a43c25576788b35e0cb9fd0991a041b3c`.
It converts to 23 UTF-16 units including NUL, FNV-1a `0xF3B09AC8`, SHA-256
`04c17e909e2f4211898fc55f7671636c434e365faf07eee8dc7557af4be44e93`.
LastError is `0` before and after the BGI call.

The controlled oracle fixes the general form independently of the title path:

| Call | Result / effect | LastError |
|---|---|---|
| `CP_ACP, flags=0, A 82 A0 00, -1`, query | 3 units, including NUL | preserves `92` |
| same conversion with capacity 3 | `[U+0041,U+3042,U+0000]` | preserves `93` |
| explicit 932／flags 1 fixed query and conversion | 3 units | preserves `87`／`88` |

The immediate native continuation does not consume EAX. It prepares five path
component buffers, tests nonnull component pointers, and converts present UTF-16
components back to ANSI. In the controlled runtime, the first two reached
consumers are `WideCharToMultiByte` calls returning `C:\0` and `\Vine\\0` at
sequences 270 and 271. This confirms output consumption without making caller
RVA, native path, buffer capacity 780, or component count part of semantics.

## Runtime contract

`kernel32.multi-byte-to-wide-char.v1` remains the single semantic handler for
query and conversion. Policy
`kernel32-multi-byte-to-wide-char-cp-acp-v1` adds two general capabilities to the
previous cumulative policy:

- `allowActiveCodePageSelector: true`, resolving `CP_ACP` to policy-owned 932;
- allowed flags `[0, 1]` under the existing 65,536-byte source bound.

Historical policies omit the selector flag and therefore remain explicit-code-page
only. Source `-1` is scanned through checked guest memory until NUL within the
policy bound. The decoder includes that NUL in its UTF-16 result and returned
length. Query performs no write; conversion validates the complete destination
span before commit. Unsupported selectors／flags and malformed shapes stop before
mutation. No BGI pointer, path, caller, count, or checkpoint identity is present
in the semantic handler or policy.

The public dispatcher test covers explicit 932／flags-one regression, `CP_ACP`／
flags-zero query and conversion, NUL-inclusive length/output, LastError, partial
buffer behavior, invalid shapes, unsupported selector／flags, ABI cleanup, and
mutation-free rejection without loading target evidence.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-multi-byte-to-wide-char-cp-acp
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 44 / 279 | 44 / 279 |
| Instructions at next stop | 24,294 | 24,294 |
| Writeback batches / pages | 284 / 431 | 2,385 / 3,075 |
| Host/mapping deltas | 75 | 75 |
| Final generation | 388 | 2,126 |
| Memory SHA-256 | `dfb9d4fae57c56d3c518b522669c129377a2de98e64cd7de9e5dfdad42e6113d` | same |
| Report bytes | 1,225,836 | 273,463 |
| Report SHA-256 | `80073d04b9881acff731109960be7be52b19074c7775dd3c5ef93f14d178deea` | `e17b7a135ce7e5d4108e3729932b5de2b718f7f27a35be43c0d14a2b80001691` |

Chrome used 3,123 slices, 2,839 budget yields, and 2,747 Rust/Wasm bridge
operations. Four stack growths, cancel, watchdog, zero pending work, process exit,
and DLL detach remain intact. Runtime-core Wasm is 1,928,420 bytes with SHA-256
`2d7a53fb42439df06d21637f23dd16e4f76fd72919d63eb9a19e41955cec85b3`.

The separate 180-second workload completed 607 sessions with zero failures and
passed all 17 versioned thresholds. Session p95 is `326.935 ms`, slice p99
`1.230 ms`, heartbeat p99 `8.530 ms`, main heap slope `-428,816.67 bytes/min`,
Worker heap slope `836,675.33 bytes/min`, and backing-storage slope
`1,099,018.63 bytes/min`. The 1,161,211-byte report has SHA-256
`02de19d3a07641b79ea053e95312ea62b8ffeac450605d27a041a7a164b125d1`.

## Next structured stop

```text
KERNEL32.dll!SetCurrentDirectoryA(0x10104048)
endpoint ID:       25
endpoint address:  0x7F000180
IAT:               0x100D110C
return:            0x1006084A
instruction:       24294
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must measure the ANSI path bytes and lifetime, result,
LastError, process-current-directory state, relative-path consumers, and failure
behavior before choosing browser/VFS authority.

## Non-claims

This checkpoint does not implement `SetCurrentDirectoryA`, a current-directory
object, relative-path resolution, file I/O, other code pages, arbitrary NLS flags,
normalization, additional-thread execution, title rendering, audio, VFS, COM, or
full play.
