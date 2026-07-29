# BGI `WaitForSingleObjectEx` checkpoint

## Status

- Status: verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-wait-for-single-object-ex`
- Previous capability checkpoint: `kernel32-set-waitable-timer`
- Completed subsystem gateway: `runtime-waitable-timer-pacing`
- Next capability checkpoint: `primary-main-loop-progression`

This checkpoint implements the reached finite alertable wait over a relative
one-shot waitable timer. Rust owns the logical timer clock, timer expiration,
typed-object signal state, wait registration, and scheduler lane transition.
For a positive logical delay, TypeScript owns only a cancellable browser timer
resource. It does not own Windows wait state, decide the wait result, or import
browser elapsed time into the Rust clock.

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

`RuntimeScheduler` and `RuntimeWaits` plan a bounded registration with thread
ID, object ID, logical handle, timeout deadline, and alertable flag. Positive
delay also reserves a host request and emits `runtime.timer-delay.v1` payload
version 1 with full-width request, registration, timer, clock, and arm
generations. The browser completion carries only ack `1`.

Completion revalidates request, thread, lane, registration, object, timer,
clock, and arm identity before committing clock advance, expiration, object
signal, registration retirement, and exact owner-lane resume. Stale clock,
stale arm, duplicate completion, wrong ack, rearm, close, cancellation, wrong
object, existing cross-thread waiters, and injected preflight failures preserve
all owners and allocators.

The Worker freezes a pending delay on page suspension, preserves its remaining
duration, and rearms it on resume. Lifecycle and request generations reject
late callbacks, cancellation retires the browser resource, and catch-up is
bounded to one completion per task. During this service only, guest CPU
execution is held until the delay completes and the Rust-authorized owner lane
is consumed once. This prevents wall timing from choosing a different guest
instruction order. Other host services retain ordinary multi-lane scheduling.

Representative tests:

```powershell
cargo test -p vine-core --lib runtime::timers::tests --locked
cargo test -p vine-core --lib runtime::scheduler::tests --locked
cargo test -p vine-core --lib runtime::waits::tests --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
cargo test -p vine-core --test api_dispatcher wait_for_single_object_ex --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
node tools/test-kernel32-compatibility-policy.mjs
node tools/test-runtime-worker-timer-pacing.mjs
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
bytes, 17,353 host deltas, and final generation 36,680. Its three pacing
requests use request IDs `3/4/5`, registration IDs `1/2/3`, one-millisecond
delays, clock generations `0/1/2`, and arm generations `1/2/3`. Node completes
them with deterministic acknowledgements and creates no timer.
Ordered-call semantics have SHA-256
`31b7d2d7d3d14452f93d45fa9a00c962dfaaf8b9516186855d6f4e4208080332`;
checkpoint memory has SHA-256
`975002521bc7cc363fbc870a484d935b1cba9dc700b5afb8113b189c7929b828`.

Chrome 150.0.7871.187 reproduces the same calls, clock and expiration
snapshots, ordered semantics, memory, and terminal endpoint. It records 19,453
slices, 19,451 writeback batches over 39,346 pages, 17,353 mapping deltas,
generation 37,379, and 55,880 Wasm crossings. It creates and completes exactly
three pacing resources with zero pending, cancellation, or stale callback in
the success run. Cancel and watchdog cleanup leave no pending host call or
callback. Current report sizes, hashes, Wasm identity, and browser version are
recorded only in [current-checkpoint.json](current-checkpoint.json).

Because this gateway changes browser scheduling and host crossings, the
versioned repeated-session browser performance observation was rerun after the
semantic contract completed. Both full runs passed all 17 workload thresholds
with zero semantic failures, and the copy-free final memory summary closed the
initial observer-only Rust linear-memory excess without changing guest
semantics. Both observations were independently invalidated by the same
policy-defined sustained external-CPU condition, so neither is accepted
performance evidence. Exact reports, protected invariants, and the isolated-host
resume condition are recorded only in the current manifest.

The historical follow-on `kernel32-enter-critical-section` profile consumed
three balanced `EnterCriticalSection`／`LeaveCriticalSection` pairs in this
same idle cycle. The active
[`kernel32-leave-critical-section`](bgi-leave-critical-section.md) profile
continues through the same seven timer/wait pairs, records the Rust-owned
`8192 -> 8193 -> 8192` scheduler round trip, releases the main-thread
critical section, and stops at `KERNEL32.dll!EnterCriticalSection`. The wait
policy remains a dependency policy rather than a second state authority. Its
180-second repeated-session observation passed all versioned thresholds in a
valid performance environment; exact current metrics remain manifest-owned.

## Post-critical-section recurrence

The `kernel32-wait-for-single-object-ex-post-critical-section` profile resumes
after the post-clock balanced critical-section roundtrip and consumes the
already-armed manual-reset timer wait through the next owned endpoint. It uses
the existing finite alertable `WaitForSingleObjectEx` transaction: Rust
revalidates the handle, timer, object, arm, registration, logical clock,
waiting thread, and scheduler lane before atomically expiring the timer,
signaling the object, retiring the registration, and returning
`WAIT_OBJECT_0` without a guest write.

Node and Chrome agree on the completion snapshot, ordered semantic digest,
terminal memory, and next `EnterCriticalSection` call. The Worker contributes
one browser pacing acknowledgement and finishes with no pending or stale
resource; it does not own the Windows timer, wait registration, result, or
thread selection. This is reachability evidence for the existing transaction,
not a new timeout, APC, cross-thread waiter, or browser-clock semantic claim.
Exact report identities and the follow-on checkpoint remain authoritative only
in [current-checkpoint.json](current-checkpoint.json).

The follow-on critical-section profile consumes the complete contiguous
balanced batch and stops before the next `SetWaitableTimer` arm. That endpoint
is a recurrence of the same Rust-owned timer state described here; reaching it
does not arm the timer in the critical-section checkpoint or create a browser
timer authority. The balanced batch contract is documented in
[bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md).

The late `kernel32-set-waitable-timer-post-critical-section-batch` profile
consumes that arm and stops on the next wait frame. The follow-on
`kernel32-wait-for-single-object-ex-post-timer-arm-batch` profile now consumes
that wait through the same Rust-owned transaction. It revalidates timer ID 2,
logical handle `0x57010002`, object ID 24, arm generation, logical deadline,
wait registration, owner thread, and scheduler lane before publishing the
logical expiration and `WAIT_OBJECT_0` result. Node and Chrome agree on the
signaled object snapshot, advanced timer clock, zero guest write, preserved
LastError, ordered semantic digest, terminal memory, and following
`EnterCriticalSection(0x1014c8d0)` frame.

The Worker acknowledges exactly one additional `runtime.timer-delay.v1`
request and retains no pending or stale browser callback. It remains a pacing
adapter: Rust owns the timer, registration, logical clock, result, and thread
resume. This is another reached recurrence of the existing semantic leaf, so
it does not broaden APC, general timeout, cross-thread waiter, or performance
claims. Exact counts and artifact identities remain manifest-owned.

The following post-timer-wait critical-section profile consumes the complete
balanced six-call batch and reaches the next timer-arm frame. That continuation
does not alter the wait result or create another timer, wait, scheduler, or
browser authority; its stable synchronization contract is documented in
[bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md).

The `kernel32-set-waitable-timer-post-timer-wait-batch` profile now consumes
that reached arm through the existing Rust timer transaction and stops before
the next `WaitForSingleObjectEx(0x57010002, 10, TRUE)` dispatch. Its completion
leaves the manual-reset object nonsignaled with a new live arm; it does not
advance the clock, expire the timer, choose a wait result, or create a browser
pacing resource.

The follow-on
`kernel32-wait-for-single-object-ex-post-timer-rearm-batch` profile now
consumes that live arm through the same finite alertable wait transaction.
Rust revalidates the timer, typed object, arm generation, registration,
logical clock, owner thread, and scheduler lane before jointly publishing the
logical expiration, signaled object, retired registration, and
`WAIT_OBJECT_0`. Node and Chrome agree on the zero-write completion, preserved
LastError, ordered semantics, terminal memory, and following
`EnterCriticalSection(0x1014c8d0)` frame. The Worker acknowledges one more
versioned pacing request and retains no pending, cancelled, or stale callback;
it remains non-owning evidence rather than a Windows wait authority. The next
critical-section recurrence and all exact occurrence, generation, deadline,
and artifact values remain manifest-owned.

## Non-claims

- Alertable mode is accepted, but APC delivery and `WAIT_IO_COMPLETION` are not
  implemented.
- General process, thread, mutex, event, semaphore, wait-any, wait-all, and
  cross-thread waiter interleaving are outside this contract.
- `INFINITE`, general host-elapsed timeout behavior, multi-waiter fairness, and
  timer behavior outside the bounded 0..10 ms policy are not implemented.
- Browser delay paces the reached loop, but it is not a Windows clock
  authority and does not determine guest thread order or the wait result.
- Lifecycle freeze/resume and bounded catch-up have deterministic service-model
  coverage; this is not a claim for every browser throttling mode or long-idle
  timer workload.
- Auto-reset expiration is covered by Rust model tests but remains outside the
  active primary creation policy.

## Historical next checkpoint

The pacing gateway's original next endpoint was ID 144,
`KERNEL32.dll!EnterCriticalSection` at `0x7F0008F0`. That historical stop has
been consumed; the active structured stop is the primary main-loop progression
boundary recorded in the current checkpoint manifest.
