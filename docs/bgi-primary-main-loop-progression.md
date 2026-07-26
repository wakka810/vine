# BGI primary main-loop progression checkpoint

## Status

- Status: verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `primary-main-loop-progression`
- Previous capability checkpoint: `kernel32-enter-critical-section`
- Completed subsystem gateway: `runtime-waitable-timer-pacing`
- Next owned endpoint: `KERNEL32.dll!LeaveCriticalSection`

This checkpoint verifies the reached main-loop scheduler round trip after the
bounded waitable-timer service. The Rust scheduler remains the authority for
logical thread state, selection, and CPU-context ownership. The Worker only
transports the versioned completion and restores the selected opaque context;
it does not maintain a second Windows thread registry or choose a thread from
browser timing.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile primary-main-loop-progression
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile primary-main-loop-progression `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

The Node and Chrome sessions both complete 18,734 generic calls and
11,974,687 guest instructions. The profile first consumes seven
`SetWaitableTimer`／`WaitForSingleObjectEx` pairs, then records exactly two
API-boundary selections:

| Sequence | Instruction | Previous | Selected | State |
|---:|---:|---:|---:|---|
| 18,663 | 11,973,280 | `8192` | `8193` | runnable |
| 18,734 | 11,974,512 | `8193` | `8192` | runnable |

The terminal endpoint is ID `145`,
`KERNEL32.dll!LeaveCriticalSection` at `0x7F000900`. Node and Chrome agree on
the ordered semantic digest
`0b620924832019b38cc8af0ac9ffa3b1f6300647262bc8a4544ce64408dbf4c8`, final
guest memory digest
`ca7efd6774fe36acfa3ee61621e12b2b6fccbaf4327cc026f323f3aa0859fc2b`, and the
checkpoint completion metadata. Exact artifact identities, writeback counts,
and report hashes remain in [current-checkpoint.json](current-checkpoint.json).

The accepted prepared-Chrome workload repeats this same profile for 180
seconds. It completed 95 sessions with zero failures; session P95 was
2,007.35 ms, maximum-slice P99 was 4.90 ms, heartbeat P99 was 6.37 ms, and
all 17 versioned thresholds passed in a valid environment. The benchmark report
and artifact hashes are manifest-owned.

## Ownership and invariants

`RuntimeScheduler` plans the selection only after the API completion has been
validated. The plan checks the previous owner, selected runnable thread,
registered opaque CPU context, and lifecycle generation before committing.
Guest memory writeback and CPU-context restoration occur at the same
completion boundary; a stale, duplicate, cancelled, or wrong-thread
completion cannot partially advance either owner.

The browser has no authority over Windows thread IDs, scheduler state, guest
memory, or context lifetime. The two selections are an API-boundary
round-robin observation for the reached private workload, not a general
fairness policy.

Representative checks:

```powershell
cargo test -p vine-core --lib runtime::scheduler::tests --locked
cargo test -p vine-core --test runtime_thread_registry --locked
node tools/test-bgi-checkpoint-command.mjs
node tools/test-runtime-worker-evidence-level.mjs
node tools/test-current-checkpoint.mjs
```

## Non-claims

- Timer expiration, message dequeue, paint invalidation, and user input are not
  implemented by this checkpoint.
- This does not establish general thread creation, wait-any／wait-all,
  cross-thread blocking, APC delivery, or scheduler fairness.
- Browser timer latency does not select guest thread order; the Rust completion
  and generation checks do.
- The terminal `LeaveCriticalSection` call is intentionally not dispatched in
  this checkpoint. Its existing semantic handler remains covered by the
  historical `kernel32-enter-critical-section` profile.
