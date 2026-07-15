# BGI `SetThreadAffinityMask` and process-clock checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-set-thread-affinity-mask`
- Previous capability checkpoint: `bgi-get-current-thread`
- First unsupported capability: `kernel32-get-system-info`

This checkpoint implements the reached current-thread affinity operation without
importing a host CPU topology or native affinity mask. Its immediate continuation
also establishes the first progressing process clock required by BGI: the title
pins the current thread, spins on `GetTickCount` for approximately one second,
then restores the mask returned by the first affinity call.

```text
GetCurrentThread() -> 0xFFFFFFFE
SetThreadAffinityMask(0xFFFFFFFE, 1) -> previous logical mask
GetTickCount() until unsigned elapsed >= 1000
SetThreadAffinityMask(0xFFFFFFFE, saved mask) -> previous logical mask
```

## Native evidence

The focused observer records BGI's two affinity calls, their continuations, an
independent current-thread affinity oracle, and an aggregate of the calibration
loop without emitting every `GetTickCount` call:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-set-thread-affinity-mask-20260716-r6.jsonl `
  --seconds 1.4 --kill-after --observe set-thread-affinity-mask
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r6 | 147,512 | 435 | 185 | `f55699275c2e3f7edcf8b7d414d7fe1e8bbabb8e701b04fa45acf097b4de0d5a` |
| Confirmation r7 | 141,205 | 416 | 176 | `c3807adb646982fe1dccf82c80fd385d1402bd745d094a36c6dc9998e135e563` |

Both captures have zero hook, call-site, agent, cleanup, calibration, or oracle
diagnostics. On the measured host the native process and system mask is
`0x0000FFFF`. The first BGI call returns that previous mask after selecting bit
`1`; the second requests the saved `0x0000FFFF` and returns `1`. Both successes
preserve LastError. The return RVAs are `0x0006999A` and `0x000699F0`.

The independent `x86-current-thread-logical-affinity-v1` oracle additionally
establishes:

- success returns the previous mask and supports restoring it;
- a zero mask returns zero with `ERROR_INVALID_PARAMETER` (`87`);
- a null thread handle returns zero with `ERROR_INVALID_HANDLE` (`6`);
- success preserves LastError sentinels;
- process／system masks and native handle values are capture identity, not guest
  runtime inputs.

Two longer calibration captures aggregate the loop:

| Capture | Bytes / lines | Tick reads | Value changes | Regressions | Elapsed | SHA-256 |
|---|---:|---:|---:|---:|---:|---|
| r8 | 133,327 / 383 | 154,477 | 64 | 0 | 1,016 ms | `d485f0aa38f178ab12de96604f373a97e7d5ae8811a138a858dc84019d45898b` |
| r9 | 149,701 / 436 | 149,406 | 64 | 0 | 1,016 ms | `90e6649bd8405d58724c3ca7b6e8b0e4c3f18a9b00b03b62ecdd27830f22f077` |

The consumer therefore depends on monotonic progress, not on a particular host
tick origin or the native read frequency. A fixed `GetTickCount` sample cannot
continue this reached path.

## Rust contract and policy

`RuntimeThreads` privately owns each thread's optional logical affinity mask.
An unset mask resolves to the policy-owned virtual process mask. A mutation-free
plan validates a running thread, a nonzero requested subset, and the previous
effective mask; commit rejects a stale or duplicate plan through the existing
thread-record comparison. No host scheduler state or browser resource is
created.

`kernel32.set-thread-affinity-mask.v1` is a two-DWORD stdcall contract. The
active compatibility policy declares virtual process mask `1`. It accepts the
reached current-thread pseudo-handle, returns the previous logical mask, writes
no guest bytes, and preserves LastError on success. Null handle and invalid mask
have the Win32 failure results above. Other handle forms remain a mutation-free
structured unsupported result until reached evidence justifies access and
lifetime semantics.

`RuntimeProcessState` owns the current tick count. The active policy declares a
deterministic `+1 ms` per `GetTickCount` read. Sampling creates a plan containing
the returned and next DWORD values, requires a running process, rejects stale
commit, and uses DWORD wrapping addition. Historical policies omit the option
and retain their fixed-sample reproduction behavior. The increment is a virtual
clock policy, not elapsed host time.

Public tests cover two logical threads, selection isolation, success and
restore, LastError preservation, null／zero／outside-mask failures, arbitrary
handle rejection, policy disablement, stale plan rejection, and DWORD wrap.

## Runtime and Browser Worker evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-thread-affinity-mask
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 48 / 1,290 | 48 / 1,290 |
| Instructions at next stop | 45,420 | 45,420 |
| Writeback batches / pages | 1,295 / 2,453 | 4,497 / 6,194 |
| Host/mapping deltas | 77 | 77 |
| Final generation | 1,401 | 3,223 |
| Memory SHA-256 | `ca8193a48aa4293f31a9cd95bf516ff777e50256e3d1c53c440ae02b0d6b4bb5` | same |

Calls `286` and `1290` invoke `SetThreadAffinityMask(0xFFFFFFFE,1)`, each
return `1`, preserve LastError `0`, and write no guest bytes. Calls `287..1289`
perform 1,003 calibration reads: the first result is `3,600,001`, the last is
`3,601,003`, and the next process tick is `3,601,004`. Including the earlier
startup read, the session contains 1,004 `GetTickCount` calls.

The Node report is 5,260,396 bytes with SHA-256
`ffaf389e5ed377d07aefe71ad8ad3380209fcb603fa70b27b99c9ddf8f6ff2db`.
Chrome `150.0.7871.115` reproduces every ordered call, logical affinity result,
tick result, memory digest, and structured stop. It uses 6,269 slices, 4,974
budget yields, and 5,872 Wasm bridge operations. The 772,080-byte browser report
has SHA-256
`71dc8db1d7d933bf2a08f2b51825ace88c0ce68c1470463bb6b425a3102ef79c`;
cancel and watchdog teardown leave zero pending work.

The runtime-core Wasm is 2,229,296 bytes with SHA-256
`377cc05c4a006bd3654be23ce2e5e88357c3f6bfcdc6f0a4fe114370ca876911`.
The unchanged 180-second gate completes 421 sessions with zero failures and
passes all 17 versioned thresholds. Session p95 is `430.865 ms`, slice p99
`2.190 ms`, heartbeat p99 `6.055 ms`, and maximum heartbeat gap `25.555 ms`.
The 1,092,780-byte benchmark report has SHA-256
`7d633dcf1f50d67bc53be6732fceb90d7f05f2685bc312b58170bb5a702caf72`.

## Non-claims

This checkpoint does not control host CPU affinity, expose host topology,
support arbitrary thread handles／access rights, implement process-affinity
APIs, schedule guest lanes in parallel, or define a real-time clock rate. The
deterministic read increment is sufficient for the reached calibration loop but
does not yet implement `Sleep`, timers, wait deadlines, performance-counter
frequency, or browser audio-clock synchronization.

## Next structured stop

```text
KERNEL32.dll!GetSystemInfo(0x006FFED8)
endpoint ID:       120
endpoint address:  0x7F000770
IAT:               0x100D1288
return:            0x10069F0A
instruction:       45420
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The output pointer lies in the live initial-thread stack. The next checkpoint
must establish the x86 `SYSTEM_INFO` layout and caller-consumed fields while
keeping processor topology a virtual compatibility value rather than importing
the current browser host.
