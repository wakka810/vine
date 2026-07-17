# BGI `CreateWaitableTimerA` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-create-waitable-timer-a`
- Previous capability checkpoint: `bgi-get-version-ex-a`
- First unsupported capability: `user32-load-icon-a`

This checkpoint adds the reached unnamed manual-reset waitable-timer creation
contract. Rust owns timer identity and reset mode in `RuntimeTimers`, while the
typed object table owns the logical handle, close policy, and initial wait
state. TypeScript only transports the immutable completion snapshot and does
not own Windows timer state.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-create-waitable-timer-a-20260717-r2.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event create-waitable-timer-a-complete `
  --observe create-waitable-timer-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 256,897 | 779 | 354 | `1debfadd3238adb16d5607779fdce297d6420350f85323f07b15f047230551af` |
| Confirmation r2 | 255,564 | 777 | 353 | `07e769e5e27d7495ea4fbc3f09bebb55cbc6de2c4b6779e37c1e796977eb475c` |

Both captures have contiguous agent sequences, balanced call IDs, zero
diagnostics, and 250 ms of hook quiescence. The source executable remained
1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

The controlled x86 oracle creates one unnamed manual-reset and one unnamed
auto-reset timer. Both calls return a non-NULL handle, leave success LastError
at 0, initially return `WAIT_TIMEOUT` (`258`) from
`WaitForSingleObject(handle, 0)`, and close successfully. Hooked results and
LastError agree with direct system calls. Native handle values are artifact
identity and are not imported into the runtime.

BGI calls `CreateWaitableTimerA(NULL, TRUE, NULL)` and returns to RVA
`0x0005053B`. The immediate consumer stores EAX directly into `[esi + 4]`;
there is no intervening normalization or native-handle arithmetic.

## ABI, policy, and ownership

`kernel32.create-waitable-timer-a.v1` is a three-argument x86 stdcall returning
a non-NULL logical handle in namespace
`vine.runtime-waitable-timer-object.v1`. Handler-owned LastError is 0 on the
implemented success path and the call performs no guest-memory write.

The versioned `kernel32-create-waitable-timer-a-v1` policy enables exactly the
reached variant: NULL security attributes, manual reset, and no name. Security
attributes, named timers, and auto-reset creation stop as explicit unsupported
variants before runtime mutation. The native auto-reset result proves Windows
behavior but does not broaden the active primary profile.

`RuntimeTimers` privately owns monotonic timer IDs and the `manual_reset`
payload. `RuntimeObjectTable` separately owns object ID, handle slot and
generation, namespace, `close-handle` policy, open-handle count, and initial
`nonsignaled` wait state. The reached call creates timer ID 1, object ID 20,
and logical handle `0x57010001`.

Creation preflights timer capacity, timer-plan freshness, and object-table
capacity before either owner commits. Last close preflights payload retirement
and one-handle object retirement together. Process termination builds one
opaque close-handle object plan for mutex and timer keys plus private payload
retirement plans, then commits only after all owners accept the transaction.
Stale, duplicate, exhausted, wrong-kind, and injected object-capacity failures
leave payload maps, identity indexes, handles, allocators, and wait state
unchanged. No subsystem root is cloned or replaced for rollback.

Representative tests:

```powershell
cargo test -p vine-core --lib runtime::timers::tests --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
cargo test -p vine-core --test api_dispatcher create_waitable_timer_a --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
```

## Node, Chrome, and performance evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-create-waitable-timer-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 1,338 calls across 59 contracts. `CreateWaitableTimerA` is
sequence 1,338 at instruction 49,897, returns `0x57010001`, and records the
timer／object snapshot above without a guest write. Execution then reaches
`LoadIconA(0x10000000, 0x65)` at instruction 49,942.

Node records 1,344 writeback batches, 2,541 dirty pages, 9,178 changed bytes,
85 host deltas, final generation 1,458, and memory SHA-256
`de0f2de11612c4792c7f87daa195c3a0b95d519069eefa7f93edc959cd4ce26a`.
The 5,462,881-byte report has SHA-256
`8da8c2f67aa2c7ea83109ad53f29d083735e5cbc98887b2b1cf90e69a9d11ebd`.

Chrome Runtime Worker reproduces the same ordered calls, timer snapshot,
memory digest, and next stop with 6,848 slices, 5,504 yields, 4,975 writeback
batches／6,725 pages, 85 mapping deltas, final generation 3,611, and 6,407
Wasm crossings. Cancel and watchdog cleanup leave no pending host call,
callback, or request record. The 805,891-byte browser report has SHA-256
`e98501b2dac103f6067246e70d15fdc47337c7d6f3cb0c9e934c9714bcaf9c77`.
Runtime-core Wasm is 2,361,392 bytes with SHA-256
`03a9dc99bdd6b592c71e3d9fd344e9ed29402a0faf82a4c02a73b8410eec722b`.

Because this checkpoint expands object lifetime and process teardown, the
three-minute browser workload gate was rerun. It completed 267 sessions with
zero failure and passed all 17 versioned thresholds. Session p95 was 745.900
ms, slice p99 was 2.325 ms, heartbeat p99 was 15.865 ms, and the maximum
heartbeat gap was 25.020 ms. The 969,997-byte report has SHA-256
`77a52398ef6db6618715a42fc3d0354be60d4215737f3984d8071a2f24657e74`.

## Non-claims

- No due time, period, signaling, reset, cancellation, or expiration clock is
  implemented yet; the created object remains initially nonsignaled.
- Auto-reset and named timers are measured but outside the active bounded
  policy. Security descriptors and inheritance are not modeled.
- `WaitForSingleObject`, `SetWaitableTimer`, `CancelWaitableTimer`, and guest
  `CloseHandle` coverage are not added by this checkpoint.
- Logical timer handles are Vine ABI identities, not native handle values or
  browser timer IDs.

## Next structured stop

```text
USER32.dll!LoadIconA(0x10000000, 0x00000065)
endpoint ID:       192
endpoint address:  0x7F000BF0
IAT address:       0x100D13D8
return address:    0x10067CF3
instruction count: 49,942
fault:             endpoint-contract-missing
```
