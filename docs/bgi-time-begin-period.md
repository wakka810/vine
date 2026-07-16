# BGI `timeBeginPeriod` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-time-begin-period`
- Previous capability checkpoint: `bgi-time-get-dev-caps`
- First unsupported capability: `kernel32-get-version-ex-a`

This checkpoint adds a Rust-owned logical WinMM timer-resolution request
lifecycle. It implements the reached `timeBeginPeriod(1)` call together with
the matching `timeEndPeriod` contract, duplicate pairing, failure atomicity,
and process-exit teardown. It does not change browser scheduling, Web Audio,
wait deadlines, or the runtime clock.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-time-period-20260716-r2.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event time-get-dev-caps-complete `
  --observe time-get-dev-caps
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r2 | 310,319 | 830 | 379 | `25e7c69f714c70d1918db6fea0b033419afa2308f89076a81ecb039cb69ba210` |
| Confirmation r3 | 310,070 | 829 | 379 | `85cd6dd52528f1268773ae3031d7d18e22b571cec8ec3dc0d2e99514cb7c84ce` |

Both accepted captures have contiguous agent sequences, balanced enter／leave
IDs, zero diagnostics, and 250 ms of hook quiescence after BGI's period call.
The 2,337,931-byte r1 capture is rejected: its observer referenced
`isMainModuleCaller` from `onLeave`, produced 14 agent messages, and therefore
cannot establish timer semantics.

The guarded x86 oracle makes 17 calls. It establishes these relations:

- unmatched `timeEndPeriod(1)`, `timeBeginPeriod(0)`, and
  `timeEndPeriod(0)` return `TIMERR_NOCANDO` (`97`);
- begin／end pairs for 2, 1,000,001, and `UINT_MAX` return
  `TIMERR_NOERROR` (`0`) on this native host;
- two successful `timeBeginPeriod(1)` calls require two successful matching
  ends;
- `timeEndPeriod(2)` while only period 1 is active and an extra
  `timeEndPeriod(1)` return 97;
- all 17 calls preserve independent LastError sentinels and the hooked and
  unhooked results agree.

The broad native acceptance of nonzero values is evidence, not the active
compatibility policy. The reached title variant is period 1, so version 1
permits only 1 until another consumer requires a broader contract.

BGI returns from `timeBeginPeriod(1)` to RVA `0x00091E7E` with result 0 and
LastError 0, then unconditionally stores its logical timer-active flag. The
handler does not branch on that RVA, title hash, or occurrence.

## Rust authority and failure ordering

`runtime/timers.rs::RuntimeTimers` privately owns period-specific request
counts and a generation. Its immutable snapshot derives total request count
and the minimum requested period; neither value is duplicated in the kernel or
TypeScript. Begin and end first create mutation-free plans containing expected
generation and count. Commit rejects stale or duplicate plans, changes only the
target period, and increments the generation once.

The table is bounded to 64 distinct periods. Zero, capacity exhaustion, count
overflow, and generation overflow fail before mutation. An unmatched or
out-of-policy end returns 97 without changing runtime or guest memory. Process
exit plans timer teardown alongside process and thread lifecycle, preflights
the timer generation before any timer mutation, then retires all outstanding
requests in one targeted commit.

`winmm.time-begin-period.v1` and `winmm.time-end-period.v1` are one-argument
x86 stdcall contracts returning raw `MMRESULT` and preserving LastError. The
controlled policy profile `timer-resolution-period-1-v1` accepts only period
1. The contracts perform no guest write and make no browser request.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile winmm-time-begin-period
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 1,336 calls across 57 contracts. `timeBeginPeriod(1)` is
sequence 1,335 at instruction 49,737 and records one request, total count 1,
effective period 1, and timer generation 1. Sequence 1,336 is the following
8-byte `HeapAlloc`. Execution reaches `GetVersionExA(0x006FFE9C)` at
instruction 49,890.

Node records 1,342 writeback batches, 2,536 dirty pages, 9,099 changed bytes,
84 host deltas, and final memory generation 1,455. Chrome reproduces the same
calls, result, timer snapshot, final memory, and structured stop with 6,841
slices, 5,499 yields, 4,968 writeback batches／6,715 pages, 84 mapping deltas,
final generation 3,603, and 6,397 Wasm crossings. Both have memory SHA-256
`b38b7030fc29769c220a4095891aa1f2bd2d71ea71584f5c3349097d7b084f6f`.

The 5,450,899-byte Node report has SHA-256
`832c6ce3b1ad84d5fd63053f58e9b6d8ec0ece89fd746cfbdd2e04b800d36efb`.
The canonical 801,197-byte browser report has SHA-256
`6b9a5aaf70a5751929fc4a948b9e25d29e2fb936665df732b2c7f49a727fbbd6`.
Runtime-core Wasm is 2,314,433 bytes with SHA-256
`5e98c0118da4067a21ef1cb12448797134c48360bd672b1511a5b984b1d782dd`.

The unchanged 180-second benchmark completed 367 sessions with zero semantic
failure and all 17 thresholds green. Session p95 was 503.055 ms, slice p99
2.610 ms, and heartbeat p99 6.040 ms. Its 1,069,902-byte report has SHA-256
`1774d6c01bc0f0840960bec18051e07df926ac009bf32a9677099bb08d3a362d`.

## Non-claims

- Native acceptance of arbitrary nonzero periods is not generalized.
- A logical request does not promise 1 ms browser precision, latency, or power
  behavior and does not reconfigure `setTimeout`, Worker yielding, or audio.
- No `timeSetEvent`, periodic callback, sleep, wait, WinMM audio, or MCI
  behavior is implemented here.
- The title has not yet reached `timeEndPeriod`; its contract is included only
  because exact begin lifetime, negative pairing, and process teardown require
  it.

## Next structured stop

```text
KERNEL32.dll!GetVersionExA(0x006FFE9C)
endpoint ID:       117
endpoint address:  0x7F000740
IAT:               0x100D127C
return:            0x10050526
instruction:       49890
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

This is an ordinary unsupported API endpoint, not a feasibility blocker. The
next checkpoint must first establish the reached structure size, caller
consumption, version compatibility policy, LastError behavior, and failure
ordering.
