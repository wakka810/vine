# Chrome Runtime Worker vertical slice

## Status

- Status: Verified controlled vertical checkpoint
- Date: 2026-07-12
- Browser: Chrome `150.0.7871.115`
- Protocol: `vine.browser-runtime-worker` version `2`
- Runtime core: Rust/Wasm, zero imports, 811,512 bytes
- CPU mirror: pinned source-v86 adapter in a dedicated Runtime Worker
- Controlled PE32 fixture: `browser-vertical.exe`
- Evidence: `.work/browser-runtime/browser-report.json`
- Stress evidence: `.work/browser-runtime/browser-stress-report.json`

This checkpoint runs guest CPU execution inside Chrome rather than presenting a
Node-generated event. It crosses PE32 load, Rust-owned memory, v86 execution,
generic API dispatch, asynchronous host completion, nested guest callbacks,
Canvas, Web Audio, IndexedDB, cancellation, watchdog termination, and exact
process exit in one controlled browser application.

It does not claim a general Win32 process, BGI browser execution, a production
renderer or mixer, or title-workload performance. The fixture APIs are
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
Rust/Wasm runtime core          source-v86 Wasm
  PE loader                       x86 execution
  GuestMemory authority           paging mirror
  RuntimeState                    PTE Dirty discovery
  ApiDispatcher
```

`crates/vine-runtime-wasm` owns the immutable PE snapshot, load plan,
materialized bytes, import endpoints, initial stack, `RuntimeState`, decoded API
frames, suspended calls, request identity, scheduler policy, and callback stack.
Its raw buffer ABI keeps JSON metadata separate from `VINERF01`, `VINEDL01`,
and `VINEWB01` page payloads and has no Wasm imports. TypeScript does not become
guest byte, pointer, handle, LastError, wait, message, module, or scheduler
authority.

The Worker applies the initial reflection to a bounded 2 MiB v86 physical pool,
acknowledges cache effects, and restores CPL3 CPU state through the version 1
640-byte adapter ABI. Every CPU run has an explicit instruction budget. At each
slice boundary, including traps and budget yields, it scans all writable PTEs,
clears Dirty bits, flushes the TLB, and atomically reconciles the resulting
binary batch into Rust before classifying or resuming the guest.

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
Worker. Protocol version 2 transports the lossless decimal request ID, owning
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
| Guest callback entries / maximum depth | 2 / 2 |
| Automatic writeback batches / pages | 7 / 9 |
| Final Rust memory generation | 21 |
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
separate instruction-ceiling probe triggers the watchdog after four instructions
and also leaves both counts at zero. Neither path fabricates a guest return.

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
npm run serve:runtime:browser-worker
npm run check:runtime:browser-worker:stress
```

The first command generates the PE fixture, prepares the pinned adapter, builds
the Rust core for `wasm32-unknown-unknown`, type-checks and bundles the browser
application, then runs success, pending-cancel, and watchdog probes in a clean
Chrome profile. The serve command exposes the operational UI on loopback. The
stress command adds the 30-minute repeated lifecycle run.

## Remaining gates

- title-workload Worker physical backing and memory ceiling beyond the controlled session
- general process, thread, DLL, heap, wait, message, and object lifecycle
- BGI execution through this browser runtime path

The Step 8 synthetic matrix, background-tab probe, AudioWorklet deadline probe,
and active steady-memory measurement passed their version 1 thresholds. See
[v86-browser-benchmark.md](v86-browser-benchmark.md).
