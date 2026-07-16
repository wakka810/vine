# BGI `timeGetDevCaps` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-time-get-dev-caps`
- Previous capability checkpoint: `bgi-swap-mouse-button`
- First unsupported capability: `winmm-time-begin-period`

This checkpoint adds only the reached, stateless WinMM capability query. It
does not interpret the query as a timer-resolution request and does not import
host or browser timer characteristics.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-time-get-dev-caps-20260716-r4.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event time-get-dev-caps-complete `
  --observe time-get-dev-caps
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r4 | 284,947 | 773 | 351 | `4c14edc2f9609c48dd4a0589e2e608a23364abc4eaab5231a5942c9ba760c6c2` |
| Confirmation r5 | 284,903 | 775 | 352 | `57282a7c58b83082554a3f2dbe2f87653ccd92dfdaf0b26ea70414971ff11c2b` |

Both accepted captures have contiguous agent sequences, balanced enter／leave
IDs, and zero hook, vtable, call-stack, agent-message, or cleanup diagnostics.
The completion observer waits for 250 ms of hook quiescence after the BGI
`timeBeginPeriod` leave before stopping. Earlier short captures were rejected
when unrelated `CreateFile` calls were still open; r3 was balanced but predates
the final `timeBeginPeriod` observation and is not authoritative evidence.

The independent x86 oracle fills a guarded output with `0xA5` and calls
`timeGetDevCaps` with byte lengths `0`, `4`, `7`, `8`, and `12`. Lengths below
8 return `TIMERR_NOCANDO` (`97`) without changing the output or either guard.
Lengths 8 and 12 return `TIMERR_NOERROR` (`0`) and change exactly the first
8 bytes to:

```text
01 00 00 00 40 42 0F 00
```

This is `wPeriodMin = 1` and `wPeriodMax = 1,000,000`. Success is repeated with
LastError sentinels `0`, `87`, and `0x12345678`; the short cases use independent
nonzero sentinels. Every call preserves LastError and matches an unhooked system
call made over an equivalent guarded buffer.

BGI calls `timeGetDevCaps(0x00E0C8E8, 8)` and returns to RVA `0x00091E6D`.
The caller overwrites EAX immediately by reading `wPeriodMin`, compares it with
10, and, because the native value is 1, calls `timeBeginPeriod(1)`. That second
call returns to RVA `0x00091E7E` with MMRESULT 0 and LastError 0. The observed
result is evidence for the next checkpoint, not an implementation claim here.

## Rust policy and API contract

`WinmmCompatibilityPolicy::controlled_v1()` owns the virtual 1..=1,000,000 ms
capability range. The profile is bounded ASCII data validated before execution;
it contains no BGI pointer, RVA, occurrence, executable hash, host timer value,
or browser capability.

`winmm.time-get-dev-caps.v1` is a two-argument x86 stdcall contract returning a
raw `MMRESULT` in EAX. The handler preserves LastError and advances no clock.
For `byteLength < 8` it returns 97 with no memory mutation. For
`byteLength >= 8` the generic dispatcher preflights the complete 8-byte output
and commits one fixed-width write. Missing policy, NULL, truncated, read-only,
or otherwise invalid successful output spans stop before memory mutation.

The Node probe and Runtime Worker use the same WinMM contract catalog and Rust
policy. TypeScript exposes the policy ID only as immutable diagnostic metadata;
it does not keep a second `TIMECAPS` or timer-resolution state.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile winmm-time-get-dev-caps
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 1,334 calls across 55 contracts. `timeGetDevCaps` is sequence
1,334 at instruction 49,731, returns to `0x10091E6D`, writes the exact 8-byte
profile, preserves LastError 0, and produces the host delta ending at generation
1,451. Execution then reaches `timeBeginPeriod(1)` at instruction 49,737.

Node stops with 1,340 writeback batches, 2,531 dirty pages, 8,971 changed bytes,
83 host deltas, and final generation 1,452. Chrome reproduces the same calls,
output, memory, and structured stop with 6,821 slices, 5,481 yields, 4,952
writeback batches／6,696 pages, 83 host deltas, final generation 3,588, and
6,378 Wasm crossings. Both have memory SHA-256
`065650f1edeb4dccdc65d37b972956c38a098acd2e85c910a532853f6bd97966`.

The 5,442,014-byte Node report has SHA-256
`0c2be229553d745cb8ed3c6f2be78ba9d7f9d63f127d5d2103dadcef38a513ab`.
The canonical implementation-run 800,038-byte browser report has SHA-256
`4add3362d9e6f63ab113d23f90fe92a9aeffbcfb390f7421bbe4bdccda28e457`.
Runtime-core Wasm is 2,303,048 bytes with SHA-256
`564960fcac5034339ff10b475bd241b79810fecd539956b8840b342bc4fcc6bd`.

The unchanged 180-second benchmark completed 358 sessions with zero failures
and all 17 thresholds green. The 1,061,505-byte report has SHA-256
`76267a46f787eb38c6e9f8223f2ee85c3829a12da48fa432f48b40603ae7fcad`.

## Non-claims

This checkpoint does not implement `timeBeginPeriod`, `timeEndPeriod`, timer
resolution lifetime, periodic multimedia timers, scheduler deadline changes,
sleep／wait behavior, Web Audio timing, or browser task granularity. The
1..=1,000,000 ms range is a compatibility profile, not measured browser
precision. The native `timeBeginPeriod` success is next-checkpoint evidence
only.

## Next structured stop

```text
WINMM.dll!timeBeginPeriod(1)
endpoint ID:       4
endpoint address:  0x7F000030
IAT:               0x100D14D4
return:            0x10091E7E
instruction:       49737
fault:             endpoint-contract-missing
cluster:           winmm-api-contract
```

The next checkpoint must define timer-resolution request authority, accepted
periods, duplicate／matching lifetime, `timeEndPeriod`, process teardown, and
the explicit non-equivalence to browser scheduling before it commits state.
