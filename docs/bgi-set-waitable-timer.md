# BGI `SetWaitableTimer` checkpoint

## Status

- Status: verified
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-set-waitable-timer`
- Previous capability checkpoint: `imm32-default-ime-context`
- First unsupported capability: `kernel32-wait-for-single-object-ex`

This checkpoint implements the reached relative one-shot waitable-timer arm.
`RuntimeTimers` owns the due interval and arm generation. The typed object
table separately owns the timer handle relation and wait state. TypeScript
transports the immutable completion snapshot and does not create a browser
timer or own Windows timer state.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-set-waitable-timer-20260726-r3.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event set-waitable-timer-complete `
  --observe set-waitable-timer
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r2 | 1,615,252 | 5,015 | 2,469 | `aa172689c3033568807fb57635248f9dfb9b3f0a0bd33797e214fad947162174` |
| Confirmation r3 | 1,615,361 | 5,015 | 2,469 | `9342206813c8a06d09a49934d7e2ff33fb1ada00885b0db4436021a76ded5d47` |

Both captures have contiguous agent sequences, balanced call IDs, and zero
diagnostics. Raw traces remain under `.work/` and are not committed. The source
executable remained 1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

BGI passes the timer stored in its object, due time `-5,000` in 100 ns units,
period zero, NULL completion routine and context, and `fResume=FALSE`.
`SetWaitableTimer` returns TRUE, preserves LastError 0, and returns to RVA
`0x00050607`. The immediate consumer calls
`WaitForSingleObjectEx(handle, 10, TRUE)`.

The controlled x86 oracle arms manual-reset and auto-reset timers with relative
due time `-10,000`. After the due time, two zero-time waits return
`WAIT_OBJECT_0, WAIT_OBJECT_0` for the manual timer and
`WAIT_OBJECT_0, WAIT_TIMEOUT` for the auto timer. An invalid handle returns
FALSE with `ERROR_INVALID_HANDLE` 6. Hooked and direct system-call results
match. Native handles and wall-clock scheduling are oracle artifact identity,
not runtime state.

## ABI, policy, and ownership

`kernel32.set-waitable-timer.relative-one-shot.v1` is a six-argument x86
stdcall. It reads one signed `LARGE_INTEGER` from guest memory, returns BOOL,
performs no guest write, and sets LastError 0 on the implemented success path.
Invalid handles return FALSE with LastError 6. Unsupported timing variants stop
before mutation with a concrete fault.

The versioned `kernel32-set-waitable-timer-v1` policy allows relative due-time
magnitudes from 1 through 100,000 in 100 ns units, one-shot period zero, NULL
completion routine and context, and `fResume=FALSE`. Absolute or zero due
times, periodic arms, APC completion, resume requests, and values outside the
bounded range are rejected before timer, object, allocator, or guest-memory
state changes.

`RuntimeTimers` stores timer ID, reset mode, relative due interval, period zero,
and a monotonic arm generation. `RuntimeObjectTable` stores object ID, logical
handle, namespace, open-handle count, and wait state. The reached call arms
timer ID 2 through handle `0x57010002`, publishes generation 1, and resets
object ID 24 to `nonsignaled`.

The kernel preflights the timer arm plan and the exact typed-object wait-state
plan before committing either owner. Stale or duplicate plans, wrong object
kind, unknown or stale handles, unsupported policy variants, unreadable due
time, and injected preflight failures preserve both owners. No subsystem root
is cloned or replaced for rollback.

Representative tests:

```powershell
cargo test -p vine-core --lib runtime::timers::tests --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
cargo test -p vine-core --test api_dispatcher set_waitable_timer --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
node tools/test-kernel32-compatibility-policy.mjs
```

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-waitable-timer
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-set-waitable-timer `
  --bgi-policy fixtures/runtime-policies/kernel32-set-waitable-timer-v1.json
```

Node completes 18,681 calls across 111 contracts. `SetWaitableTimer` is
sequence 18,681 at instruction 11,973,553 and records due magnitude 5,000,
period zero, arm generation 1, and nonsignaled object state. It performs no
guest write. Node records 18,737 writeback batches, 37,708 dirty pages,
2,932,457 changed bytes, 17,341 host deltas, and final generation 36,651.

The 400,534-byte Node report has SHA-256
`ecf7566fc075c042721a2f2780f6ddea7436deffa0005dd51abd1aa41defcb43`.
Ordered-call semantics have SHA-256
`63de9f17f94d715615e80ba13801adb3fb7db47dce4b069bfd22726e8fe15859`;
terminal memory has SHA-256
`ec8092d7bcc78d42a2b8ad6cbbe8571cc1443350f21133cfaaef6ad173ad9bfa`.

Chrome 150.0.7871.187 reproduces the same call, timer/object snapshot, ordered
semantics, terminal memory, and next stop. It records 19,436 slices, 699 budget
yields, 19,434 writeback batches over 39,322 pages, 17,341 mapping deltas,
generation 37,350, and 55,831 Wasm crossings. Cancel and watchdog cleanup leave
no pending host call or callback. The 44,248-byte browser report has SHA-256
`0b848a4375725a6eef66d40798dd2e8df0fc7c705a3353ba31161e28eba1389a`.

This semantic leaf does not add a host crossing, browser clock, expiration
scan, or hot-path work. The unchanged versioned browser performance observation
is therefore retained instead of creating a new performance baseline.

## Late post-critical-section recurrence

The `kernel32-set-waitable-timer-post-critical-section-batch` profile consumes
the late arm reached after the post-wait critical-section batch. It reuses the
same `kernel32.set-waitable-timer.relative-one-shot.v1` transaction and the
same `RuntimeTimers`／`RuntimeObjectTable` authorities. The reached call
rearms manual-reset timer ID 2 through logical handle `0x57010002`, advances
the arm generation, publishes the new logical deadline, and leaves object ID
24 nonsignaled. BOOL, LastError, zero guest write, stack cleanup, typed-object
identity, close policy, and open-handle count are checked as one completion.

Node and Chrome agree on the arm and object snapshots, ordered semantic digest,
terminal memory, and the following
`WaitForSingleObjectEx(0x57010002, 10, TRUE)` frame. The profile stops before
that wait is dispatched. The follow-on
`kernel32-wait-for-single-object-ex-post-timer-arm-batch` profile consumes the
wait through the existing Rust-owned expiration transaction and stops at the
next `EnterCriticalSection` frame. The arm checkpoint itself creates no
browser timer and chooses no wait result; the wait profile uses the browser
only for a versioned pacing acknowledgement. This is recurrence evidence for
the existing semantic leaves, so it does not create a new performance
baseline. Current counts, report identities, and the follow-on checkpoint are
authoritative only in [current-checkpoint.json](current-checkpoint.json).

After that wait completes, the reached balanced critical-section recurrence
now arrives at another `SetWaitableTimer` frame. The critical-section
checkpoint stops before dispatching this call, so it records no additional arm
generation or timer mutation. The
`kernel32-set-waitable-timer-post-timer-wait-batch` profile now consumes that
frame through the same relative one-shot transaction. Rust revalidates timer
ID 2, logical handle `0x57010002`, object ID 24, manual-reset mode, object
lifetime, and the current arm generation before jointly publishing the new
timer payload and `nonsignaled` object state. Node and Chrome agree on BOOL,
LastError, zero guest write, stack cleanup, the new logical deadline, terminal
memory, and the following alertable wait frame.

The Worker records only non-owning completion evidence: it does not create a
browser timer or choose a wait result. The follow-on
`kernel32-wait-for-single-object-ex-post-timer-rearm-batch` profile has now
consumed the rearmed timer through the separate Rust-owned finite alertable
wait transaction and reached the next `EnterCriticalSection` frame. That wait
advances the logical clock and publishes expiration without changing the arm
transaction or giving the browser timer authority. Exact occurrence counts,
generations, deadlines, report identities, and the next critical-section
checkpoint remain authoritative only in
[current-checkpoint.json](current-checkpoint.json).

The post-rearm-wait critical-section recurrence has now consumed its balanced
six-call block and reached another `SetWaitableTimer` frame. The
`kernel32-set-waitable-timer-post-timer-rearm-wait-batch` profile consumes that
frame through the same relative one-shot preflight and targeted-commit
transaction. Rust revalidates timer ID 2, logical handle `0x57010002`, object
ID 24, manual-reset mode, lifetime, and the live generation before jointly
publishing the next logical deadline and `nonsignaled` object state. Node and
Chrome agree on BOOL, LastError, zero guest write, stack cleanup, timer/object
identity, ordered semantics, terminal memory, and the following alertable wait
frame. The Worker records non-owning evidence and creates no browser timer.

Further recurrence is grouped as
`primary-main-loop-recurring-timer-cycle-batch`: one complete alertable wait,
the contiguous three balanced critical-section roundtrips, and the following
rearm are verified as one scenario unit before stopping at the next wait. This
batching changes neither semantic handler nor authority; it prevents repeated
single-call checkpoints from becoming the unit of progress. Exact positions,
generations, deadlines, report identities, and the active stop remain
manifest-owned.

## Non-claims

- This arm transaction does not itself expire or signal the timer, choose a
  wait result, deliver an APC, or resume a waiting thread.
- Periodic rearm, APC completion, cancellation, and resume-from-suspend remain
  outside the active contract.
- Alertable-wait acceptance and the reached 10 ms bounded wait belong to the
  separate wait transaction, not this arm checkpoint.
- The stored relative interval is guest-visible logical state, not a scheduled
  host or browser deadline.
- Auto-reset creation remains outside the active primary policy even though the
  controlled native oracle records its post-expiration behavior.
- Logical timer handles are Vine ABI identities, not native handles or browser
  timer IDs.

## Historical first next structured stop

```text
KERNEL32.dll!WaitForSingleObjectEx(0x57010002, 10, TRUE)
endpoint ID:       122
endpoint address:  0x7F000790
IAT address:       0x100D1290
return address:    0x10050615
instruction count: 11,973,559
fault:             endpoint-contract-missing
```

That first wait stop has been consumed. The active recurrence and its exact
next endpoint are recorded only in the current checkpoint manifest.
