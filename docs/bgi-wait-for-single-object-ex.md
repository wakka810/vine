# BGI `WaitForSingleObjectEx` checkpoint

## Status

- Status: verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-wait-for-single-object-ex`
- Previous capability checkpoint: `kernel32-set-waitable-timer`
- Next subsystem gateway: `runtime-waitable-timer-pacing`

This checkpoint implements the reached finite alertable wait over a relative
one-shot waitable timer. Rust owns the logical timer clock, timer expiration,
typed-object signal state, wait registration, and scheduler lane transition.
TypeScript transports immutable completion metadata and does not create a
browser timer or own Windows wait state.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-wait-for-single-object-ex-20260726-r1.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event wait-for-single-object-ex-complete `
  --observe wait-for-single-object-ex
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 1,615,447 | 5,018 | 2,471 | `1ad73149838b453939c1e5c638775f645ddd42d032358bbd6ff0aceee2a0a1eb` |
| Confirmation r2 | 1,610,933 | 5,019 | 2,471 | `e851ed15088768643823fbbb14d3d0a87372b19ee770ff936c41bf0a4fa7c0c0` |

Both captures have contiguous agent sequences, balanced call IDs, and zero
diagnostics. Raw traces remain under `.work/` and are not committed. The source
executable remained 1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

BGI calls `WaitForSingleObjectEx(handle, 10, TRUE)` after arming the timer for
relative due time `-5,000` in 100 ns units. It receives `WAIT_OBJECT_0`, keeps
LastError 0, and returns through RVA `0x00050615` with the result in EAX.

The controlled `x86-wait-for-single-object-ex-timer-v1` oracle fixes the
following relations:

- an armed manual-reset timer returns `WAIT_OBJECT_0`;
- a repeated zero-time wait on that signal returns `WAIT_OBJECT_0`;
- a zero-time wait on a nonsignaled timer returns `WAIT_TIMEOUT`;
- invalid handle `0x1234` returns `WAIT_FAILED` with
  `ERROR_INVALID_HANDLE` 6;
- success and timeout preserve sentinel LastError values;
- hooked and direct system-call results and LastError values match.

Native handles, measured durations, and wall-clock scheduling are oracle
artifact identity, not Vine runtime state.

## ABI, policy, and ownership

`kernel32.wait-for-single-object-ex.waitable-timer.v1` is a three-argument x86
stdcall returning U32. The versioned
`kernel32-wait-for-single-object-ex-v1` policy accepts only logical waitable
timer handles, finite timeouts from 0 through 10 ms, and `alertable=TRUE`.
Nonalertable waits, larger finite values, `INFINITE`, and other object kinds are
rejected before mutation. Invalid or stale handles return `WAIT_FAILED` and
LastError 6. `WAIT_OBJECT_0` and `WAIT_TIMEOUT` preserve the caller's
LastError.

`RuntimeTimers` owns a monotonic logical clock in 100 ns units. An arm records
`armedAtTime100ns`, `deadlineTime100ns`, period zero, and a monotonic arm
generation. A finite wait advances that clock to the earlier of the timer
deadline and timeout deadline. Expiration consumes only the exact live arm
generation and records the last expiration. Manual-reset expiration leaves the
typed object signaled; a later `SetWaitableTimer` resets it to nonsignaled.

`RuntimeScheduler` and `RuntimeWaits` plan an immediate bounded registration
with thread ID, object ID, logical handle, timeout deadline, and alertable flag.
The current lane round-trips from `Runnable` or `InCallback` through
`WaitingObject` back to its exact prior state. Registration, clock advance,
timer expiration, and object signal updates are all preflighted before targeted
commit. Stale clock, stale arm, duplicate registration, wrong lane, wrong
object, existing cross-thread waiters, and injected preflight failures preserve
all owners and allocators.

Representative tests:

```powershell
cargo test -p vine-core --lib runtime::timers::tests --locked
cargo test -p vine-core --lib runtime::scheduler::tests --locked
cargo test -p vine-core --lib runtime::waits::tests --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
cargo test -p vine-core --test api_dispatcher wait_for_single_object_ex --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
node tools/test-kernel32-compatibility-policy.mjs
```

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-wait-for-single-object-ex
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-wait-for-single-object-ex `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

Node completes 18,698 calls across 112 contracts. The recurring arm sequences
are 18,681, 18,689, and 18,697; wait sequences are 18,682, 18,690, and 18,698.
The three waits complete at instructions 11,973,559, 11,973,706, and
11,973,853. Arm generations and logical clock values progress as
`1/5,000`, `2/10,000`, and `3/15,000`.

The verification stops normally at instruction 11,973,878 before dispatching
owned endpoint ID 144, `KERNEL32.dll!EnterCriticalSection`, at `0x7F0008F0`.
This is a capability checkpoint, not an unsupported API fault.

Node records 18,754 writeback batches, 37,732 dirty pages, 2,932,699 changed
bytes, 17,353 host deltas, and final generation 36,680. The 408,570-byte Node
report has SHA-256
`ee88f67fb63e49921804022133c0a1f72c974e021be28673890a9d616bc55273`.
Ordered-call semantics have SHA-256
`31b7d2d7d3d14452f93d45fa9a00c962dfaaf8b9516186855d6f4e4208080332`;
checkpoint memory has SHA-256
`975002521bc7cc363fbc870a484d935b1cba9dc700b5afb8113b189c7929b828`.

Chrome 150.0.7871.187 reproduces the same calls, clock and expiration
snapshots, ordered semantics, memory, and terminal endpoint. It records 19,453
slices, 19,451 writeback batches over 39,346 pages, 17,353 mapping deltas,
generation 37,379, and 55,877 Wasm crossings. Cancel and watchdog cleanup leave
no pending host call or callback. The 44,043-byte browser report has SHA-256
`4ab890821402ef2584a9d1d8df9fb004f71ae57c2dc566e817bfe4a7c187372c`.

The existing versioned browser performance observation is retained. This
bounded semantic checkpoint adds no host crossing and deliberately does not
claim real-time pacing of the recurring loop.

## Non-claims

- Alertable mode is accepted, but APC delivery and `WAIT_IO_COMPLETION` are not
  implemented.
- General process, thread, mutex, event, semaphore, wait-any, wait-all, and
  cross-thread waiter interleaving are outside this contract.
- `INFINITE`, host-elapsed timeout behavior, cancellation during a pending
  wait, background throttling, freeze/resume, and bounded catch-up are not
  implemented.
- The current handler advances deterministic logical time and completes the
  bounded wait synchronously. It is not browser wall-clock pacing.
- Auto-reset expiration is covered by Rust model tests but remains outside the
  active primary creation policy.

## Next subsystem gateway

`runtime-waitable-timer-pacing` must decide how the existing Rust-owned timer,
wait, scheduler, request, and generation identities cross an asynchronous
Worker/browser delay without importing browser time as Windows authority.
Acceptance must cover cancellation, stale completion, page lifecycle, and
bounded catch-up before the recurring loop is allowed to run continuously.
