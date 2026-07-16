# WinMM compatibility policy

## Status

- Status: implemented timer-capability query boundary
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
  virtual capability range; future resolution-request lifetime
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
length below the 8-byte x86 structure returns `TIMERR_STRUCT` (`97`) without a
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

`timeBeginPeriod` and `timeEndPeriod` are not implied by a successful capability
query. When reached, their request count, accepted period range, matching end,
process teardown, and failure ordering must be owned by a dedicated Rust WinMM
timer-resolution state boundary. Browser timeout granularity and AudioContext
state may consume an explicit scheduler／audio policy decision, but cannot act
as the guest-visible request table or refcount.

The first reached caller and its native size／guard oracle are documented in
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md). Active call counts, report
identity, and the next structured stop remain solely in
[`current-checkpoint.json`](current-checkpoint.json).

## Automated contract

`crates/vine-core/tests/winmm_dispatcher.rs` fixes ABI metadata, exact output,
short and oversized lengths, LastError preservation, stdcall cleanup, policy
validation, and mutation-free disabled-policy／NULL／truncated／read-only output
faults. Node and Chrome execute the same semantic handler through the generic
endpoint catalog; TypeScript stores no parallel capability profile.

## Non-claims

- No timer-resolution request, refcount, matching end, or teardown behavior is
  implemented by this policy checkpoint.
- No periodic callback, wait deadline, sleep, multimedia timer event, audio
  clock, or background-tab behavior is defined here.
- The controlled capability range is not a claim about native or browser timer
  precision, accuracy, latency, or power behavior.
