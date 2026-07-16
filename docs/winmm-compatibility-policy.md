# WinMM compatibility policy

## Status

- Status: implemented timer-capability and resolution-request boundary
- Date: 2026-07-16
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
Rust compatibility policy and logical WinMM state
  virtual capability range; counted resolution-request lifetime
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

The capability query is documented in
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md); native pairing evidence
and the reached begin call are documented in
[bgi-time-begin-period.md](bgi-time-begin-period.md). Active call counts, report
identity, and the next structured stop remain solely in
[`current-checkpoint.json`](current-checkpoint.json).

## Automated contract

`crates/vine-core/tests/winmm_dispatcher.rs` fixes ABI metadata, exact output,
short and oversized lengths, LastError preservation, stdcall cleanup, policy
validation, duplicate begin／matching end, unmatched and out-of-policy
rejection, process teardown, and mutation-free failure paths. Owner model tests
fix capacity and stale-plan behavior. Node and Chrome execute the same semantic
handlers through the generic endpoint catalog; TypeScript stores no parallel
capability or request profile.

## Non-claims

- No periodic callback, wait deadline, sleep, multimedia timer event, audio
  clock, or background-tab behavior is defined here.
- The controlled capability range is not a claim about native or browser timer
  precision, accuracy, latency, or power behavior.
- Periods other than 1 and cross-process aggregation are not enabled by the
  controlled policy.
