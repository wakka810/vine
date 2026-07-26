# BGI `EnterCriticalSection` continuation checkpoint

## Status

- Status: semantic checkpoint and performance gate verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-enter-critical-section-continuation`
- Previous checkpoint: [`kernel32-leave-critical-section`](bgi-leave-critical-section.md)
- Next owned endpoint: `KERNEL32.dll!LeaveCriticalSection`

This checkpoint consumes the first main-thread `EnterCriticalSection` reached
after the scheduler round trip and the preceding release. It reuses the
existing Rust `RuntimeSynchronization` handler and adds no second
synchronization authority. The checkpoint contract proves that the same
critical-section address is released and then reacquired by the selected main
thread before execution advances to the following release.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile kernel32-enter-critical-section-continuation
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-enter-critical-section-continuation `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

Node and Chrome complete the same seven bounded timer/wait cycles and the
`8192 -> 8193 -> 8192` scheduler selection before observing the release and
acquisition pair:

| Field | Value |
|---|---|
| Acquisition sequence | `18,736` |
| Acquisition instruction | `11,974,730` |
| Critical-section address | `0x100FD9CC` |
| Acquiring thread | `8192` |
| ABI result / LastError | preserve EAX / `0` |
| Next endpoint | ID `145`, `KERNEL32.dll!LeaveCriticalSection`, `0x7F000900` |

The 24-byte guest state changes from the unlocked representation to
`fffffffffeffffff010000000020000000000000d0070002`. The host delta covers the
single owning page and the terminal report binds the resulting Rust-owned
memory digest. Protocol version 18 transports the typed
`criticalSectionAddress`, `acquisitionThreadId`, and `lastError` completion
metadata. Exact call counts, report identities, generations, semantic digest,
and memory digest remain manifest-owned in
[current-checkpoint.json](current-checkpoint.json).

## Performance gate

The unchanged 180-second Chrome for Testing workload completed 99 sessions
with zero semantic failures, passed all 17 versioned workload thresholds, and
was classified as `valid` by the Windows environment monitor. The accepted
report identity, artifact hashes, sample coverage, and exact distributions are
recorded in [current-checkpoint.json](current-checkpoint.json).

Earlier invalid-environment runs remain historical diagnostics. They recorded
`sustained-external-cpu` after the workload thresholds had passed; no threshold
or source/runtime authority was changed. The valid rerun was performed with
the same fixture, duration, pinned Chrome artifact, and threshold values:

```powershell
npm run check:runtime:bgi-browser-benchmark:prepared
```

An invalid environment is still not promoted to a pass merely because workload
thresholds succeed; future reruns retain the same independent qualification.

## Ownership and invariants

`EnterCriticalSection` validates the process-owned record, current thread,
guest bytes, memory access, and plan generation before commit. The owner
publishes recursion state, guest bytes, host delta, and memory generation as
one targeted transaction. A null, stale, malformed, wrong-owner, or
capacity-failing request mutates none of those authorities.

The Worker verifies the adjacent release/acquisition relation and carries the
typed completion only. It does not own the Windows thread ID, recursion count,
guest pointer, scheduler selection, or synchronization lifetime.

Representative checks:

```powershell
cargo test -p vine-core --test runtime_synchronization --locked
cargo test -p vine-core --lib runtime::synchronization::tests --locked
node tools/test-bgi-checkpoint-command.mjs
node tools/test-runtime-worker-evidence-level.mjs
npm run check:runtime:bgi-browser
```

## Non-claims

- This is one uncontended main-thread reacquisition, not cross-thread
  contention, waiter ordering, deletion, abandonment, or fairness evidence.
- The following release is an owned endpoint identity, not a completed
  continuation checkpoint.
- Earlier workload threshold success in an invalid host environment remains
  diagnostic evidence only.
- This does not establish full message/input flow, rendering, audio,
  save/restart/load, representative flow, or full-play acceptance.
