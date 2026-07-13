# BGI GetModuleFileNameA

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-get-module-file-name-a`
- Previous capability checkpoint: `bgi-interlocked-decrement`
- First unsupported capability: `kernel32-private-heap-size-16`

## Purpose

The cumulative BGI session asks for the current executable path and retains the
returned ANSI bytes for later startup processing.

```text
GetModuleFileNameA(NULL, 0x100FF520, 260)
  -> writes "C:\Vine\BGI.exe\0"
  -> returns 15 and sets LastError to 0
  -> next stop is HeapAlloc(0x68010001, 0, 16)
```

The runtime path is deliberately virtual. Native installation directories are
evidence, not semantic input; Rust process bootstrap owns the bounded process
image path used by the Win32 handler.

## Native evidence

The focused `get-module-file-name-a` observer retains both reached BGI calls and
guarded self-oracles for capacity, module selection, LastError, termination,
and mutation.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-module-file-name-a-20260714-r4.jsonl `
  --seconds 0.2 --kill-after --observe get-module-file-name-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 116,515 | 80 | 14 | `dbea36944098cc0ade8c4d76e916447aa7563c7b97d53026c3601cb91edd5cae` |
| Confirmation | 116,457 | 80 | 14 | `5e52ed288cb692914ecfc542b904b7b0efc8d638968934a027d80cbaca8cee88` |

Both captures have contiguous sequence numbers `0..77`, eight oracle pairs,
two direct BGI pairs, four incidental `GetProcAddress` pairs, and zero hook,
vtable, call-stack, agent-message, cleanup, or oracle diagnostics.

The reached calls return at RVAs `0xA84C6` and `0x60219`. Both pass a NULL
module handle; capacities are 260 and 780. They return 27, preserve success
LastError `0`, and write the same Windows-932 path:

```text
V:\softs\サクラノ詩\BGI.exe\0
[86,58,92,115,111,102,116,115,92,131,84,131,78,131,137,131,109,142,141,92,66,71,73,46,101,120,101,0]
SHA-256 72472c80161f0c142cee81058bcdfa1a43c25576788b35e0cb9fd0991a041b3c
```

The first destination is image RVA `0xFF520`; the second is a thread-stack
buffer. Both consumers retain the path for subsequent processing rather than
branching immediately on EAX.

The guarded x86 oracle fixes the following behavior:

| Variant | Result | LastError | Output |
|---|---:|---:|---|
| generous or exact success capacity | 27 | 0 | full path plus NUL |
| capacity 27 | 27 | 122 | byte 26 replaced by NUL |
| capacity 1 | 1 | 122 | one NUL byte |
| capacity 0, valid or NULL output | 0 | 122 | unchanged |
| explicit KERNEL32 module | 32 | 0 | `C:\WINDOWS\System32\KERNEL32.DLL` |
| invalid module handle | 0 | 126 | unchanged |

All leading and trailing guards remain unchanged. Host handles, image bases,
and the private installation path are not imported into the runtime contract.

## Runtime contract

`kernel32.get-module-file-name-a.v1` uses a three-argument stdcall frame and a
dynamic checked output span. Policy `kernel32-get-module-file-name-a-v1`
enables only a NULL module handle and caps capacity at 32 KiB.

Rust process bootstrap owns `RuntimeProcessImagePathA` with a nonempty source
authority, `windows-932-v1` encoding, one trailing NUL, no embedded NUL, and a
32 KiB maximum. The BGI profile installs `C:\Vine\BGI.exe\0`; public fixtures
install `C:\Vine\fixture.exe\0`. This keeps host paths and native installation
details outside semantic dispatch.

For positive capacity the full capacity span is writable-preflighted before any
guest mutation. Capacity greater than the path length writes the complete path
and NUL, returns the length excluding NUL, and sets LastError to `0`. Capacity
at or below the path length writes exactly the capacity, forces its final byte
to NUL, returns the capacity, and sets `ERROR_INSUFFICIENT_BUFFER` (`122`).
Zero capacity returns `0`／`122` without dereferencing the output. NULL output
with positive capacity returns `0`／`ERROR_INVALID_PARAMETER` (`87`).

Explicit module handles, missing policy, capacity above 32 KiB, and invalid
guest spans stop before guest bytes, runtime state, registers, or LastError are
mutated. Although the native oracle records explicit KERNEL32 behavior, it is
not reached by BGI and is intentionally outside this checkpoint.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-module-file-name-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 39 / 152 | 39 / 152 |
| Instructions at next stop | 16,965 | 16,965 |
| Writeback batches / pages | 153 / 229 | 1,619 / 2,142 |
| Host/mapping deltas | 51 | 51 |
| Final generation | 233 | 1,445 |
| Memory SHA-256 | `b233490f03ec30f568aa6ddf9868da4f41804150f4bbccfff8a20f2e9d01e943` | same |
| Report bytes | 704,797 | 193,912 |
| Report SHA-256 | `3bbeda2ffff7720afc73c7729b52f7168b2989758baf6c7cc7e6a45b870e15e7` | `6bcd72b1693354cfc88928bb0a5952e6b2fc187a993a52e465ac4620d0a30e07` |

The reached call is sequence `120` at instruction `15,863`, returning to
`0x100A84C6`. It writes 16 bytes to `0x100FF520`, sets EAX to `15`, and changes
generation `199 -> 200`. Host delta sequence `120` touches the data-only image
page `0x100FF000` owned by image owner `1`; its payload SHA-256 is
`ba1dab4296a6af9944e54edfbca89515d034ce0207960b404e65a3dfa252b498`.

Chrome uses 2,174 slices, 2,021 budget yields, 1,826 Wasm crossings, and final
generation `1,445`. Success, cancel, and watchdog end in `stopped` with no
pending host call or callback. The runtime-core Wasm is 1,858,027 bytes with
SHA-256 `4f2d3c11fd37b5a53ed84696420c922ab680f8ea4c8215abadc3628ed978dab1`.

The separate 180-second workload completed 961 sessions with zero failure. All
16 versioned thresholds passed: session p95 `185.185 ms`, slice p99 `0.880 ms`,
maximum heartbeat gap `18.240 ms`, main heap slope `-44,410.26 bytes/min`,
Worker heap slope `1,060,582.92 bytes/min`, and backing-storage slope
`-84,444.67 bytes/min`. The 447,265-byte report has SHA-256
`09d5a5a8de1e6b8fb69eb3d7ee70714a1530a970477ff2890151f817a6096567`.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(0x68010001, 0, 16)
endpoint ID:       75
endpoint address:  0x7F0004A0
IAT:               0x100D11D4
return:            0x100A4A33
instruction:       16965
fault:             heap-alloc-variant-unsupported
cluster:           kernel32-private-heap-allocation
```

The next focused observer must measure allocation identity, initialization,
alignment, failure, LastError, lifetime, and the immediate consumer before the
policy admits the reached `(flags=0, size=16)` variant.

## Non-claims

This checkpoint does not implement explicit module handles, arbitrary ANSI
encodings, host-path import, broad `HeapAlloc`, file access, title rendering,
audio, input, VFS, COM, or full play.
