# BGI `LeaveCriticalSection` continuation checkpoint

## Status

- Status: semantic checkpoint and performance gate verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-leave-critical-section-continuation`
- Previous checkpoint:
  [`kernel32-enter-critical-section-continuation`](bgi-enter-critical-section-continuation.md)
- Next owned endpoint: `KERNEL32.dll!EnterCriticalSection`

This checkpoint closes one post-scheduler main-thread critical-section cycle.
It consumes the release immediately after the verified reacquisition and
reuses the existing Rust `RuntimeSynchronization` handler. The evidence
contract anchors the sequence at the scheduler-selected main-thread release,
then requires adjacent same-address reacquisition and release operations. It
does not create profile-specific synchronization semantics or a TypeScript
state authority.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile kernel32-leave-critical-section-continuation
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-leave-critical-section-continuation `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

Node and Chrome complete the same seven bounded timer/wait cycles and the
`8192 -> 8193 -> 8192` scheduler selection before observing the adjacent
release/acquire/release sequence:

| Field | Value |
|---|---|
| Release sequence | `18,737` |
| Release instruction | `11,974,905` |
| Critical-section address | `0x100FD9CC` |
| Releasing thread | `8192` |
| ABI result / LastError | preserve EAX / `0` |
| Next endpoint | ID `144`, `KERNEL32.dll!EnterCriticalSection`, `0x7F0008F0` |

The terminal release writes the exact 24-byte unlocked representation
`ffffffffffffffff000000000000000000000000d0070002`. Node and Chrome agree on
the ordered semantic digest
`cedf137a3e4d410bd8a409c6405fd02f53f2f1efb6f4c6511e2ecad67bb45631`
and Rust-owned memory digest
`9473c7af4f1c00a6bf8f2f5db611cb7a5253d04387297b9e0244d81f032315bc`.
Protocol version 19 transports the typed `criticalSectionAddress`,
`releaseThreadId`, and `lastError` completion. Exact report identities,
generation counters, and browser reconciliation totals remain manifest-owned
in [current-checkpoint.json](current-checkpoint.json).

## Performance gate

The versioned 180-second Chrome for Testing workload completed 107 sessions
with zero semantic failures, passed all 17 thresholds, and was classified as
`valid` by the independent Windows environment monitor. Session P95 was
1,842.600 ms, maximum-slice P99 was 2.250 ms, and heartbeat P99 was 6.070 ms.
The fixture, artifact identities, full distributions, environment samples, and
accepted report identity are recorded in the current manifest.

```powershell
npm run check:runtime:bgi-browser-benchmark:prepared
```

## Ownership and invariants

`LeaveCriticalSection` validates the process-owned record, current thread,
guest bytes, memory access, and plan generation before commit. The owner
publishes recursion state, guest bytes, host delta, and memory generation as
one targeted transaction. Null, stale, malformed, wrong-owner, wrong-thread,
and capacity failures mutate none of those authorities.

The Node and Worker profile observers only identify the reached sequence. They
anchor it to the second scheduler selection and require adjacent operations on
the same critical-section address and selected main thread. Rust still owns
the section identity, recursion, thread selection, LastError, guest bytes, and
lifecycle.

Representative checks:

```powershell
cargo test -p vine-core --test runtime_synchronization --locked
cargo test -p vine-core --lib runtime::synchronization::tests --locked
node tools/test-bgi-checkpoint-command.mjs
node tools/test-runtime-worker-evidence-level.mjs
npm run check:runtime:bgi-browser
```

## Non-claims

- This is one uncontended main-thread cycle, not cross-thread contention,
  waiter ordering, deletion, abandonment, or fairness evidence.
- The next `EnterCriticalSection` is an owned endpoint identity, not another
  completed cycle.
- Repeated idle cycles do not establish message/input consumption.
- This does not establish rendering, audio, save/restart/load,
  representative-flow, or full-play acceptance.
