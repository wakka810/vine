# BGI `timeGetTime` checkpoint

## Status

- Status: verified
- Date: 2026-07-22
- Runtime-kernel checkpoint: `winmm-time-get-time`
- Previous capability checkpoint: `d3d9-surface-get-dc`
- First unsupported capability: `user32-message-box-a`

This checkpoint implements the reached zero-argument WinMM multimedia tick
sample. `timeGetTime` and Kernel32 `GetTickCount` use the same Rust-owned
process tick counter. The WinMM timer-resolution request table remains a
separate authority and cannot change the sampled clock. No host uptime,
browser clock, scheduler granularity, or audio clock is imported.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-time-get-time-primary-20260721.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event time-get-time-complete `
  --observe time-get-time

node tools/summarize-windows-trace.mjs `
  --input .work/windows-traces/bgi-time-get-time-primary-20260721.jsonl `
  --output .work/windows-traces/bgi-time-get-time-primary-20260721-summary.json
```

| Capture | Raw bytes / lines / pairs | Raw SHA-256 | Summary bytes / SHA-256 |
|---|---:|---|---|
| Primary | 304,054 / 854 / 390 | `8560ea8389627cfc2c85b4a98f30f835ab53ff3ec13038ac55b25ad736790a29` | 51,478 / `5fa32c69a605e1cd173eed042522669f253534a1d993d943ee239b12d16fc777` |
| Confirmation | 303,051 / 851 / 388 | `8f3a8847793282b4eadb4f4e45c6a142610b9f3ea1f3e4437e0b8349b314cc5e` | 51,652 / `83d7a4ca65a6b109379b796838cc1d97718083a28657bc70d395d636ef9604c3` |

Both captures have contiguous agent sequences, balanced call IDs, one BGI
call, and zero diagnostics. The `x86-time-get-time-shared-tick-epoch-v1`
oracle invokes `timeGetTime` with LastError sentinels `0`, `87`, and
`0x12345678`. For every call, the hook and `SystemFunction` results agree,
LastError is preserved, and the result lies within unsigned wrapping
`GetTickCount` samples taken immediately before and after. The concrete host
tick values are artifact identity, not runtime policy.

BGI returns from the call at RVA `0x00091EBF` with LastError 0. Its immediate
consumer follows the `wrap-aware-eax-delta-v1` flow: EAX remains the sampled
DWORD through a direct continuation and is used by the frame rooted at RVA
`0x00083144` as an unsigned wrapping elapsed-time operand. The handler does not
branch on either RVA, executable hash, or occurrence.

## Rust authority and ABI

`winmm.time-get-time.v1` is a zero-argument x86 stdcall contract with a U32
return, callee cleanup, preserved LastError, no callback, and no guest write.
The controlled policy profile
`shared-process-tick-count-increment-1-v1` supplies only a nonzero increment of
1. `runtime/process.rs::RuntimeProcessState` plans a sample from its existing
`RuntimeClockState.tick_count`, returns the current DWORD, and commits the next
value with wrapping addition. This is the same plan／commit path already used
by `GetTickCount`.

The policy ID includes the clock profile, but the clock datum is not copied
into WinMM state. `RuntimeTimers` continues to own only resolution-request
counts and generation. A successful `timeBeginPeriod(1)` therefore does not
change `timeGetTime`, Worker yielding, browser timers, or audio timing.
Disabled or invalid clock policy fails before runtime or memory mutation.

`crates/vine-core/tests/winmm_dispatcher.rs` fixes the ABI, two consecutive
shared-clock samples, LastError preservation, zero write, stdcall cleanup, and
mutation-free disabled／invalid policy failures. Existing process model tests
cover stale plans and DWORD wrap.

## Node, Chrome, and benchmark evidence

```powershell
npm run check:runtime:bgi -- --profile winmm-time-get-time
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 17,858 calls across 85 contracts. `timeGetTime` is sequence
17,858 at instruction 11,807,928, returns `3,601,004`, advances the shared next
sample to `3,601,005`, preserves LastError 0, and writes no guest bytes. It
reaches `USER32.dll!MessageBoxA` at instruction 11,807,974. The 49,985-byte
summary report has SHA-256
`dff9c0799ff4b6e3d583a52f3d2bbef1db02ebe132ffc8191dcd693f2f556a79`.

Chrome reproduces the complete semantic-call SHA-256
`8bc885cebc50634d583f8ee8cfc7f22995fa7c177b2f4b94e83cea55695b563b`,
terminal memory SHA-256
`a540914cb267680fa2b9d6a07ab7d3c02853703073b4969d1fb448bb0291d3b0`,
and structured stop. It records 29,089 writeback batches／61,972 pages, 16,568
mapping deltas, final generation 45,989, and 63,598 Wasm crossings; cancellation
and watchdog cleanup retain no pending work. The 115,932-byte browser report
has SHA-256
`41bcd53626661df496cab3a8bf1b1f9bf41eb135ee091561d05c87886f2a6f0a`.

The unchanged 17-threshold, 180-second workload completes 63 sessions with no
semantic failure. Session p95 is 2,952.075 ms, slice p99 is 2.665 ms, and all
thresholds pass. The 944,587-byte report has SHA-256
`cb8af93eb085ac01e74ec0bbf76aba7d019933a5ce4679fd9a79a0ff93de85bc`.
The private executable remains 1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

## Post-heap continuation

The later `winmm-time-get-time-post-heap-reuse` profile reuses this exact ABI
and semantic handler after the private-heap reuse checkpoint. It records one
reached sample together with the Rust-owned returned tick, next tick, and
policy increment, then stops before the first different owned endpoint,
`KERNEL32.dll!EnterCriticalSection`. Node and Chrome must agree on ordered call
semantics, terminal guest memory, the critical-section argument, IAT identity,
and stack state.

This continuation adds no WinMM state and needs no new native oracle: the
consumer and shared-clock contract are already fixed above. TypeScript only
transports the Rust completion evidence and does not sample a browser clock.
Current occurrence counts, instruction counts, report identities, and the next
structured stop are maintained only in
[`current-checkpoint.json`](current-checkpoint.json).

## Post-Present continuation

The later `winmm-time-get-time-post-present` profile reuses the same semantic
handler after the fourth discard Present. Under the scheduled clock, the read
samples the process-owned projection without advancing it, preserves LastError,
writes no guest bytes, and then continues to the next owned
`IDirect3DDevice9::TestCooperativeLevel` boundary. Node and Chrome must agree on
the exact call, clock／timer projection, ordered semantics, terminal memory, and
next COM identity; TypeScript also preserves the already-published frame receipt
without becoming a clock or Windows graphics authority. See
[bgi-winmm-time-get-time-post-present.md](bgi-winmm-time-get-time-post-present.md)
for the bounded acceptance and non-claims.

## Post-callback continuation

The later `winmm-time-get-time-post-test-cooperative-level-timer-callback`
profile reuses the same semantic handler after the reached TIMERPROC frame,
outer `DispatchMessageA`, and empty current-thread `PeekMessageA` have completed.
It samples the existing scheduled process clock without advancing it, imports no
host or browser time, and continues to the following `USER32.dll!GetWindowRect`
boundary. Node and Chrome must agree on the exact clock projection, zero-write
contract, retired callback state, ordered semantics, terminal memory, and next
endpoint. See
[the post-callback timeGetTime profile](bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback.md)
for bounded acceptance and non-claims.

## Non-claims

- One deterministic increment per API read is not elapsed host time and does
  not claim native or browser millisecond precision.
- `timeBeginPeriod` does not alter the process clock, browser scheduling,
  wait deadlines, frame cadence, or audio sample delivery.
- No sleep, wait, `timeSetEvent`, multimedia callback, WinMM audio, MCI, or
  background-tab catch-up behavior is implemented here.
- A single reached caller does not establish every `timeGetTime` consumer or
  long-run timing-sensitive title behavior.

## Next structured stop

```text
USER32.dll!MessageBoxA(0x48010001, 0x100DCBA0, 0x100DCBE0, 0x00001010)
endpoint ID:       221
endpoint address:  0x7F000DC0
IAT:               0x100D144C
return:            0x10066B0D
instruction:       11807974
fault:             endpoint-contract-missing
cluster:           user32-api-contract
```

This is an ordinary unsupported API endpoint, not a feasibility blocker. Its
checkpoint must establish text pointer／encoding bounds, owner-window and
modal-state behavior, result consumption, LastError, and browser interaction
before choosing the smallest vertical slice.
