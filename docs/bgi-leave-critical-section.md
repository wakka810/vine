# BGI `LeaveCriticalSection` checkpoint

## Status

- Status: verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-leave-critical-section`
- Previous capability checkpoint: `primary-main-loop-progression`
- Completed subsystem gateway: `runtime-waitable-timer-pacing`
- Next owned endpoint: `KERNEL32.dll!EnterCriticalSection`
- Follow-on checkpoint:
  [`kernel32-enter-critical-section-continuation`](bgi-enter-critical-section-continuation.md)

This checkpoint closes the scheduler continuation at the first reached
`LeaveCriticalSection` call. It does not add a second synchronization authority:
`RuntimeSynchronization` owns the process critical-section record, and Rust
validates the current thread, address, lifecycle, guest-memory access, and
completion generation before committing the release.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile kernel32-leave-critical-section
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-leave-critical-section `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

Node and Chrome both complete 18,735 generic calls and 11,974,730 guest
instructions. The seven bounded waitable-timer pairs and the
`8192 -> 8193 -> 8192` scheduler selection precede one release completion:

| Field | Value |
|---|---|
| Completion sequence | `18,735` |
| Completion instruction | `11,974,687` |
| Critical-section address | `0x100FD9CC` |
| Releasing thread | `8192` |
| ABI result / LastError | preserve EAX / `0` |
| Next endpoint | ID `144`, `KERNEL32.dll!EnterCriticalSection`, `0x7F0008F0` |

The 24-byte guest state is released to the exact unlocked representation
`ffffffffffffffff000000000000000000000000d0070002`. Node and Chrome agree on
the ordered semantic digest
`77dee747fc8fc285f6c4e32f68e12ba2334118b7b6984ecc68236be8acdac7f4` and final
guest-memory digest
`b326ab911f02986bbbac818825b0b67171299bc946495eb52f5a0be6fb7e8636`.
Current report sizes, generation counters, and artifact hashes remain
manifest-owned in [current-checkpoint.json](current-checkpoint.json).

The historical accepted prepared-Chrome 180-second workload uses this same profile. It
completed 93 sessions with zero failures; session P95 was 2,101.625 ms,
maximum-slice P99 was 5.900 ms, heartbeat P99 was 6.395 ms, and all 17
versioned thresholds passed in a valid environment. The benchmark report and
artifact identities are recorded only in the manifest.

The follow-on continuation checkpoint now consumes the endpoint recorded here,
verifies the same-address main-thread reacquisition, and stops at the next
`LeaveCriticalSection`. Its performance gate later passed in a valid
environment; the earlier structured environment blocker remains historical
diagnostic evidence and does not invalidate this frozen semantic evidence.

## Ownership and invariants

`LeaveCriticalSection` is a Rust-owned semantic handler. Its mutation-free
preflight rejects a null, stale, wrong-owner, or unsupported record before
touching the synchronization collection, allocator, guest bytes, or generation.
The commit publishes the unlocked bytes and the host delta as one completion
boundary. Duplicate, cancelled-late, or wrong-thread completions cannot
partially release the record.

The Worker transports typed completion metadata and opaque CPU context. It does
not own Windows thread IDs, critical-section recursion, guest pointers, or
browser-side synchronization state. The `EnterCriticalSection` endpoint shown
above is intentionally not dispatched within this historical checkpoint.

Representative checks:

```powershell
cargo test -p vine-core --lib runtime::synchronization::tests --locked
node tools/test-bgi-checkpoint-command.mjs
node tools/test-runtime-worker-evidence-level.mjs
node tools/test-current-checkpoint.mjs
npm run check:runtime:bgi-browser:prepared
```

## Non-claims

- This is one reached main-thread release, not general critical-section
  contention, deletion, abandonment, or cross-thread waiter scheduling.
- It does not establish general thread creation, wait-any／wait-all, APC
  delivery, message/input flow, rendering, audio, or save/load behavior.
- Browser delay resources pace the bounded loop but do not own the logical
  clock, synchronization state, or scheduler order.
- The private target remains a checkpoint observation; representative flow and
  full-play acceptance are not claimed.
