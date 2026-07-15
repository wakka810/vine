# BGI `GetCurrentThread` checkpoint

This checkpoint implements the first reached `KERNEL32.dll!GetCurrentThread()`
call without importing a host thread ID or a native handle identity. The Win32
pseudo-handle is stable as a guest-visible value, while its meaning is resolved
from the Rust-owned selected thread and typed thread object on every call.

```text
selected RuntimeScheduler thread
  -> registered RuntimeThreadState
  -> typed RuntimeObjectKey::Thread
  -> Win32 current-thread pseudo-handle 0xFFFFFFFE
```

The resolution is read-only. It does not allocate an object-table entry, open a
new logical handle, retain a host handle, or mutate LastError or guest memory.

## Native evidence

The focused observer uses the target import slot rather than trying to patch the
short x86 `GetCurrentThread` export. It statically records both BGI IAT call
sites and runs a native x86 oracle that sets and captures TEB LastError around
the real API call without returning through JavaScript between those operations.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-current-thread-20260715-r6.jsonl `
  --seconds 0.2 --kill-after --observe get-current-thread
```

The 17,712-byte／60-line capture has SHA-256
`f84cb98c32e1a41fba9eef1fa95dc4d0bd13499a64d6d9c6ad25e21de15c4417`.
It contains four balanced generic API pairs, two static `GetCurrentThread` IAT
call-site records, one independent oracle, and zero hook, call-site, agent,
cleanup, or oracle diagnostics.

The `x86-current-thread-pseudo-handle-v1` oracle establishes:

- all four same-thread calls return unsigned `4294967294` (`0xFFFFFFFE`);
- the invoking native thread is unchanged before and after every call;
- LastError sentinels `0`, `87`, `122`, and `0x12345678` are all preserved;
- the numeric native thread ID is capture identity only and is not a runtime
  semantic input.

The saved checkpoint call is `call dword ptr [IAT RVA 0x000D1148]` at RVA
`0x00069989`, returning to RVA `0x0006998F`. Its immediate continuation is:

```text
mov edi,eax
push 1
push edi
call dword ptr [IAT RVA 0x000D114C]
```

The next import is `SetThreadAffinityMask`, so the first meaningful consumer
requires the pseudo-handle to be accepted as the current-thread handle and does
not require a run-specific numeric identity.

## Rust contract and tests

`RuntimeKernel::resolve_current_thread_pseudo_handle` reads the scheduler's
current selector, validates the corresponding thread record, resolves its
private typed object identity, and returns a resolution containing the constant
pseudo value, selected `RuntimeThreadId`, and `RuntimeObjectId`. This makes the
dynamic relation testable while keeping subsystem collections private.

`kernel32.get-current-thread.v1` is an immediate, zero-argument x86 stdcall
contract with a `U32` return and preserve-LastError policy. The dispatcher test
calls it twice on the initial thread, selects a second registered thread, and
calls it again. All calls return `0xFFFFFFFE`; the resolved typed object changes
with selection; LastError remains thread-local; guest memory, runtime state, and
handle-table cardinality remain unchanged by each dispatch.

## Runtime and Browser Worker evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-current-thread
npm run check:runtime:bgi-browser
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 47 / 285 | 47 / 285 |
| Instructions at next stop | 25,261 | 25,261 |
| Writeback batches / pages | 290 / 444 | 2,482 / 3,176 |
| Host/mapping deltas | 77 | 77 |
| Final generation | 396 | 2,208 |
| Memory SHA-256 | `08afd63f56cf8c18788d80f50a6e8037f005c35d53bd50c55ec3b88975eb698b` | same |

The Node semantic report is 1,254,345 bytes with SHA-256
`50d7ae673691a3036f328284f1e276404d8b7bec07256d16f793236f4bfbe1eb`.
Chrome `150.0.7871.115` reproduces the ordered calls, pseudo-handle result,
LastError, final memory, and exact structured stop. Its 282,255-byte report has
SHA-256
`5ae9a13a397f2c2f2a78833bfe16fb22cebf60d8c444941d3d23c3dbca203d56`;
cancel and watchdog teardown both leave zero pending work.

The commit-bound 180-second Chrome workload gate completed 306 repetitions with
zero failures and passed all 17 versioned thresholds. It retains 2,852 Wasm
bridge operations per session; the 912,820-byte report has SHA-256
`cf288c91072a115db539fafaffa0a21553df7518eb69a1eefaa0901423e9cf8f`.

## Non-claims

This checkpoint does not implement `SetThreadAffinityMask`, native scheduler
affinity, thread access-right checks, pseudo-handle duplication, arbitrary
thread-handle consumers, host thread switching, or parallel guest execution.
The existing typed thread handle remains a distinct logical handle; the pseudo
value is not inserted into the object table.

## Next structured stop

```text
KERNEL32.dll!SetThreadAffinityMask(0xFFFFFFFE, 1)
endpoint ID:       41
endpoint address:  0x7F000280
IAT:               0x100D114C
return:            0x1006999A
instruction:       25261
fault:             endpoint-contract-missing
```
