# WinMM compatibility policy

## Status

- Status: implemented timer-capability, resolution-request, and shared-clock boundary
- Date: 2026-07-22
- Semantic authority: Rust `vine-core::winmm`
- Policy schema: `vine.winmm-compatibility-policy` version `1`
- Evidence selection: `docs/current-checkpoint.json`

WinMM ABI semantics, guest-visible timer state, and browser scheduling are
separate responsibilities. `WinmmCompatibilityPolicy` contains bounded virtual
capabilities. Semantic handlers validate and consume that policy, while the
evidence profile records the reached title call and never selects a handler by
caller address, executable hash, or occurrence.

```text
WinMM ABI contract
  stdcall frame, MMRESULT, pointer bounds, LastError policy
                         |
                         v
Rust compatibility policy and Rust-owned state
  virtual capability range; counted resolution requests; shared process tick sample
                         |
                         v
Browser adapters
  scheduler delivery and audio resources, never WinMM state authority
```

## Timer-capability query

The controlled version-1 policy enables one x86 `TIMECAPS` profile with a
minimum period of 1 ms and a maximum period of 1,000,000 ms. These values are
virtual compatibility data. They are not imported from native multimedia timer
hardware, browser task scheduling, `performance.now()`, Web Audio, or the host
power policy.

`winmm.time-get-dev-caps.v1` accepts a two-argument x86 stdcall frame. A byte
length below the 8-byte x86 structure returns `TIMERR_NOCANDO` (`97`) without a
guest write. A length of 8 or more writes exactly two little-endian `u32`
fields and returns `TIMERR_NOERROR` (`0`); bytes beyond the structure are not
touched. LastError is preserved on both paths. Policy absence, an invalid
policy, a NULL output for a successful-size request, or an invalid full output
span stops before memory mutation.

The query is stateless: it does not advance the runtime clock, change scheduler
granularity, create a browser request, or establish a timer-resolution lease.
The policy is internal typed configuration rather than a persisted user format,
so version 1 currently requires no migration path.

## Resolution-request boundary

The controlled policy enables `timer-resolution-period-1-v1`, whose only
accepted period is the reached value 1. This is intentionally narrower than
the native host's broad nonzero acceptance. Policy rejection returns
`TIMERR_NOCANDO` before consulting or mutating runtime state.

`runtime/timers.rs::RuntimeTimers` owns the period-to-request-count map and
generation. Total request count and effective minimum period are derived from
that one map. `timeBeginPeriod` adds one matching request; duplicate calls are
counted independently. `timeEndPeriod` removes exactly one request, and an
unmatched end returns `TIMERR_NOCANDO` without mutation. Begin／end use
mutation-free plans with expected generation and target count, and commit
rejects stale or duplicate plans. The distinct-period table, per-period count,
and generation are bounded; exhaustion is a concrete pre-mutation fault.

Process exit includes an opaque timer teardown plan in the cross-subsystem
preflight and retires all outstanding requests in one targeted commit. A
serialized timer snapshot is immutable diagnostic data, not a second owner.
Browser timeout granularity, Worker yield policy, Web Audio, and host timer
resolution remain unchanged and non-authoritative.

## Shared multimedia tick sample

The controlled policy enables `shared-process-tick-count-increment-1-v1` for
`winmm.time-get-time.v1`. The contract is zero-argument x86 stdcall, returns a
raw DWORD through U32 completion, preserves LastError, and performs no guest
write or browser request. Policy absence or a zero increment stops before
runtime mutation.

`runtime/process.rs::RuntimeProcessState` remains the single owner of
`RuntimeClockState.tick_count`. Both `GetTickCount` and `timeGetTime` call the
same mutation-free sample plan and targeted commit: return the current DWORD,
then advance the stored value with wrapping addition. WinMM does not own a
second epoch or counter. The version-1 increment of 1 is deterministic
compatibility policy, not an observation of elapsed host time.

Timer capabilities and resolution requests remain orthogonal to this datum.
`timeGetDevCaps` does not sample it, and a successful `timeBeginPeriod` does
not alter it. Browser `performance.now()`, host uptime, task scheduling, frame
cadence, and audio clocks are not imported into either API.

Late continuation profiles, including the reached sample after private-heap
reuse, must use the same handler and clock owner. They may expose returned and
next tick values as profile-scoped evidence, but cannot create another clock
datum or import browser timing.

The capability query is documented in
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md); native pairing evidence
and the reached begin call are documented in
[bgi-time-begin-period.md](bgi-time-begin-period.md). Shared-clock native and
caller evidence is documented in
[bgi-time-get-time.md](bgi-time-get-time.md). Active call counts, report
identity, and the next structured stop remain solely in
[`current-checkpoint.json`](current-checkpoint.json).

## Automated contract

`crates/vine-core/tests/winmm_dispatcher.rs` fixes ABI metadata, exact output,
short and oversized lengths, LastError preservation, stdcall cleanup, policy
validation, duplicate begin／matching end, unmatched and out-of-policy
rejection, process teardown, and mutation-free failure paths. Owner model tests
fix capacity, stale-plan, and DWORD-wrap behavior. The `timeGetTime` fixture
also fixes consecutive shared-process samples, zero guest writes, and
disabled／invalid policy failure atomicity. Node and Chrome execute the same semantic
handlers through the generic endpoint catalog; TypeScript stores no parallel
capability or request profile.

The reached post-pointer-release continuation reuses this same contract after
the release callback has retired and an empty `PeekMessageA(PM_REMOVE)` has
completed. It fixes the previous next tick as the returned value, one-step clock
advance, zero guest writes, LastError preservation, callback depth zero, and an
unchanged empty Rust message queue before continuing to the next `timeGetTime`.
This is additional consumer reachability evidence, not a second WinMM clock or
a browser-time synchronization policy.

The following convergence path reaches four contiguous `timeGetTime` calls in
total. The repeat-batch profile applies the same plan/commit independently to
each call, requires the returned tick to equal the prior call's next tick, and
preserves stdcall cleanup, LastError, guest memory, callback depth zero, and the
empty queue throughout. Evidence collection is bounded to 16 completions and
terminates at the first heterogeneous owned endpoint, `GetWindowRect`; neither
the repetition count nor the Worker becomes clock authority.

## Non-claims

- No periodic callback, wait deadline, sleep, multimedia timer event, audio
  clock, or background-tab behavior is defined here.
- The controlled capability range is not a claim about native or browser timer
  precision, accuracy, latency, or power behavior.
- Periods other than 1 and cross-process aggregation are not enabled by the
  controlled policy.
- Deterministic read progression does not claim elapsed-time accuracy or bind
  the sampled clock to timer-resolution requests.
