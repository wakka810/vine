# Chrome Runtime Worker vertical slice

## Status

- Status: Verified controlled fixture and private BGI semantic checkpoint
- Date: 2026-07-13
- Browser: Chrome `150.0.7871.115`
- Protocol: `vine.browser-runtime-worker` version `4`
- Runtime core: Rust/Wasm, zero imports, 1,519,513 bytes
- CPU mirror: pinned source-v86 adapter in a dedicated Runtime Worker
- Controlled PE32 fixture: `browser-vertical.exe`
- Evidence: `.work/browser-runtime/browser-report.json`
- BGI evidence: `.work/browser-runtime/bgi-browser-report.json`
- BGI benchmark evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`
- Stress evidence: `.work/browser-runtime/browser-stress-report.json`

This checkpoint runs guest CPU execution inside Chrome rather than presenting a
Node-generated event. It crosses PE32 load, Rust-owned memory, v86 execution,
generic API dispatch, asynchronous host completion, nested guest callbacks,
Canvas, Web Audio, IndexedDB, cancellation, watchdog termination, and exact
process exit in one controlled browser application.

It does not claim a general Win32 process, title rendering, a production
renderer or mixer, or full-play steady performance. The fixture APIs are
checkpoint services and are not counted as implemented Windows APIs.

## Runtime path

```text
Chrome main thread
  operational UI / Canvas / AudioContext / IndexedDB / heartbeat
                         |
                         | versioned messages and host completions
                         v
Runtime Worker
  browser event delivery / watchdog / cancellation
                         |
             +-----------+-----------+
             |                       |
             v                       v
Rust/Wasm runtime core          ExecutionEngine adapter
  PE loader                       source-v86 Wasm
  GuestMemory authority           paging / dirty discovery
  RuntimeState                    CPU context / cache effects
  ApiDispatcher
```

`crates/vine-runtime-wasm` owns the immutable PE snapshot, load plan,
materialized bytes, import endpoints, initial stack, `RuntimeState`, decoded API
frames, suspended calls, request identity, scheduler policy, and callback stack.
Its raw buffer ABI keeps JSON metadata separate from `VINERF01`, `VINEDL01`,
and `VINEWB01` page payloads and has no Wasm imports. TypeScript does not become
guest byte, pointer, handle, LastError, wait, message, module, or scheduler
authority.

The Worker passes the initial reflection to `ExecutionEngine`, acknowledges
cache effects, and restores a versioned CPU context. Every CPU run has an
explicit caller budget and returns structured `budget-yield`／`trap`／`fault`／
`halt` state. At each slice boundary the engine returns address／owner／bytes for
changed pages, which the Worker atomically reconciles into Rust before
classifying or resuming the guest. The upper loop does not know how changes are
discovered.

Before guest entry, the same Worker applies seven ordered `VINEDL01` batches.
The first real private-heap allocation lazily reserves its 16 MiB backing,
commits one page, and transfers that page; the next batch is dirty-only. The
remaining batches protect read-only／read-write, decommit, recommit, and release
the same backing. Mapping operations and dirty pages are preflighted against a
cloned engine registry before any mapping or byte mutation. Protect invalidation
is applied through generic data-TLB, instruction-fetch, and generated-code
effects.
Decommit and release both produce a code-bearing unmapped-access stop on a stale
probe. A current full reflection rebuild preserves the CPU context and proves
the receipt-loss recovery path without changing Rust acknowledgement state.

## Execution engine contract

Protocol version 4 binds the production loop to `ExecutionEngine`, not to v86
exports or numeric stop reasons. The interface owns budgeted execution,
structured stops, versioned opaque CPU-context save／restore, guest mapping
operations, owner-tagged changed-page batches, CPU resume／callback entry, and
data-TLB／instruction-fetch／generated-code effects. `runtime-worker.ts` contains
no PTE address, v86 private offset, `full_clear_tlb`, or v86 reason constant.

The current implementation remains the source-v86 adapter. Its non-PAE tables,
PTE Dirty scan, private instruction-counter／memory-size offsets, TLB flush, and
generated-code hook are confined to `v86-mirror.ts`. The pinned profile is
[`fixtures/execution-engines/v86-browser-v1.json`](../fixtures/execution-engines/v86-browser-v1.json):
source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f` and CPU-context ABI version `1`／
640 bytes are common. Its artifact allowlist contains exact host builds: Windows
GNU is 1,882,836 bytes／SHA-256
`f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26`;
Linux GNU is 2,141,050 bytes／SHA-256
`ac8349cbaccd31d6f5c68a84012fc7bc3a73eda2c06e519331396b43d8c42540`.
Build preparation matches the actual Wasm to one allowlist entry and records
that selected identity, rather than the whole allowlist, in the build manifest.
Worker startup checks artifact bytes／SHA-256 before compilation and exported
context version／size before guest initialization.

The verified adapter module is cached once per Worker; each runtime session
still creates a fresh Wasm instance, CPU context, mapping registry, and guest
memory. This placement removed repeated 1.88 MiB fetch／hash／compile backing
storage without sharing guest state. `npm run check:execution-engine-contract:prepared`
uses the actual Wasm to fix CPU context, structured yield／trap, all five mapping
operations, changed-page owner/address, cache effects, and revision／ABI／hash
mismatch rejection.

## Execution state contract

Protocol version 3 introduced, and version 4 preserves, the active scheduler
lane through an explicit state
machine: `ready`, `running`, `budget-yield`, `api-trap`, `waiting-host`,
`host-completion`, `guest-callback`, `callback-return`, `stopping`, and
`stopped`. Prepared Rust metadata supplies the owning process and thread IDs.
TypeScript combines that lane identity with Worker lifecycle state; it does not
replace the Rust scheduler authority.

Every transition is checked against a closed adjacency table before state,
sequence, or last-transition metadata is changed. API traps remember whether a
synchronous completion resumes `ready` or a nested `guest-callback`. Permitted
async starts enter a table whose records contain request ID, thread ID, service,
cancel state, and `resume-ready`／`resume-callback` policy. Completion must match
both identities. Wrong-thread, duplicate, cancelled-late, unknown, and reused
request completions are rejected without completing another record.

Cancellation retires every active delivery record before final reporting.
Success, cancellation, structured stop, watchdog, and fault all pass through
`stopping -> stopped`; terminal reports require zero pending request records.
Only a watchdog report may contain diagnostic context, which records the cause
thread, current request when one exists, and the last transition observed
before termination. The current Worker executes one CPU lane, but request
ownership is no longer represented by a global single pending slot.

## Fixture contract

The reproducible PE32/i386 fixture imports three symbols from
`vine_browser_fixture.dll`:

1. `fixture_deferred_add(19, 23)` suspends the guest, sends one coarse host
   request, and resumes with `42`.
2. `fixture_call_callback` enters an outer guest callback. That callback calls
   the same API for an inner callback, producing strict depth `2` and result
   `44` before both frames unwind.
3. `fixture_commit_outputs(0xff0000ff, 880, 0x5a)` suspends again. The main
   thread presents an RGBA8 test pixel, schedules an 880 Hz Web Audio pulse,
   writes byte `0x5a` to IndexedDB, reads it back, and returns success.

The fixture returns through the exact Rust-owned process-entry sentinel with
exit code `0`. The native Windows self-oracle also exits `0`.

Host operations never expose a Promise to the guest stack. `RuntimeState`
enters `WaitingHost`, records a request ID and resume state, and returns to the
Worker. Protocol version 4 transports the lossless decimal request ID, owning
thread ID, bounded copied scalar payload, and matching Rust `waiting-host`
authorization. TypeScript retains only a browser-delivery Promise. A typed
completion restores the Rust scheduler state and completes the saved generic
dispatcher frame before CPU registers are resumed. The fixture retains no guest
pointer across the asynchronous boundary.

## Browser result

The clean-profile Chrome check recorded:

| Evidence | Result |
|---|---:|
| Guest instructions | 46 |
| CPU slices / periodic budget yields | 10 / 3 |
| Deferred host requests | 2 |
| Rust-authorized wait / resume transitions | 2 |
| Worker transitions, success / cancel / watchdog | 31 / 5 / 5 |
| Final Worker state / pending request records | `stopped` / 0 |
| Guest callback entries / maximum depth | 2 / 2 |
| Automatic writeback batches / pages | 7 / 9 |
| Mapping deltas / dirty pages | 7 / 3 |
| Reserve / commit / protect / decommit / release | 1 / 2 / 2 / 1 / 1 |
| Atomic rejection / stale-access stops / rebuilds | 1 / 2 / 1 |
| Mapping TLB / fetch / generated-code effects | 6 / 6 / 6 |
| Final Rust memory generation | 30 |
| Canvas readback at `(1,1)` | `[255,0,0,255]` |
| Audio pulse | 880 Hz |
| IndexedDB readback | `0x5a` |
| Guest exit code | 0 |

The successful run's main-thread heartbeat continued while the guest executed
in the Worker. Desktop `1280x800` and mobile `390x844` screenshots were captured
after the same result, and both retained non-overlapping controls, surface, and
telemetry. Generated screenshots remain under `.work/`.

The cancellation probe stops while the first deferred host request is pending.
It reaches one host request after four guest instructions, then Rust cancellation
retires the request and leaves pending host calls and callbacks at zero. A
The wall-clock watchdog expires after four instructions while the first host
request is pending and also leaves both counts at zero. Neither path fabricates
a guest return. It
identifies thread `8192`, request `1`, and the last
`api-trap -> waiting-host` transition before retiring that request. Non-watchdog
reports contain no watchdog context.
Mapping application contains no event-loop yield: cancellation can occur before
or after one preflighted batch, never between its mapping and dirty-page commits.

## Private BGI result

`npm run check:runtime:bgi-browser` now executes the current 185-call generic BGI
session in this same Runtime Worker. Rust/Wasm owns PE load, process/module/heap
state, Kernel32 dispatch, and guest bytes; Node supplies only an independently
generated semantic oracle. Chrome reaches the same unsupported
`HeapAlloc(0x68010001, 8, 4)` stop after 18,143
instructions, with equal ordered API results and final memory content SHA-256.
The bounded Worker loop uses 2,325 slices, 2,139 periodic yields, and 6,976
checked transitions while the main-thread heartbeat remains active.

The Chrome memory generation ends at 1,538 rather than the Node oracle's 267
because Chrome reconciles dirty pages at every 8-instruction budget boundary.
This difference is synchronization identity, not guest-visible state. Separate
cancel and watchdog probes stop after 8 and 4 instructions and confirm Rust
process exit, module detach, and zero pending host/callback work. Detailed
evidence and non-claims are in [bgi-browser-worker.md](bgi-browser-worker.md).

The separate 180-second BGI workload gate repeats this complete session 918
times in the same Worker. It records p95 session latency 197.300 ms, p99
maximum slice 0.990 ms, main heap slope -119,696 bytes/min, Worker heap slope
1,707,349 bytes/min, backing-storage slope 1,442,354 bytes/min, stable Rust/engine
linear-memory sizes, and 1,969 bridge operations per session. All version 1
thresholds pass; details are in
[bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## 30-minute stress

`npm run check:runtime:browser-worker:stress` repeated the complete successful
lifecycle in one Chrome page and Worker for 1,800,074 ms. Each repetition creates
a new Rust runtime and v86 mirror, executes both asynchronous services and the
nested callback, verifies all three browser outputs, and exits cleanly.

| Evidence | Result |
|---|---:|
| Successful sessions | 11,326 |
| Failed sessions | 0 |
| One-minute memory samples | 31 |
| First / last `usedJSHeapSize` | 960,969 / 994,282 bytes |
| Linear heap slope | 136.21 bytes/minute |
| Worst main-thread heartbeat gap | 16.28 ms |
| Report bytes | 7,039 |
| Exact-run report SHA-256 | `13a7b579bb13841aa073de4dcf5ed7db0743485810e12b3159272205fbb43bb3` |

The slope uses linear regression across all samples, not only the first and
last values. Timing, heap samples, and the full report hash describe this exact
run and are not deterministic semantic gates. The automated gate separately
asserts instruction, callback, output, and cleanup semantics.

This is a repeated controlled-session lifecycle soak. It is not a 30-minute
single-process title or gameplay soak, and it does not establish background-tab
or audio-deadline behavior. Those measurements belong to the next v86
go/no-go benchmark.

## Reproduction

```powershell
npm run check:runtime:browser-worker
npm run check:runtime:bgi-browser
npm run serve:runtime:browser-worker
npm run check:runtime:browser-worker:stress
```

The first command generates the PE fixture, prepares the pinned adapter, builds
the Rust core for `wasm32-unknown-unknown`, type-checks and bundles the browser
application, then runs success, pending-cancel, and watchdog probes in a clean
Chrome profile. The serve command exposes the operational UI on loopback. The
stress command adds the 30-minute repeated lifecycle run.

## Remaining gates

- full-play Worker physical backing and memory ceiling beyond the three-minute workload gate
- general process, thread, DLL, heap, wait, message, and object lifecycle
- long-idle heap-capacity plateau and full-play growth slope
- the next unsupported BGI capability and title output path

The Step 8 synthetic matrix, background-tab probe, AudioWorklet deadline probe,
and active steady-memory measurement passed their version 1 thresholds. See
[v86-browser-benchmark.md](v86-browser-benchmark.md).
