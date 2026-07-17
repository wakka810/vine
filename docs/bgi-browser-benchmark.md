# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-18
- Workload: 1,389-call `kernel32-source-file-read` private BGI generic session
- Observation status: `current-immutable-source-read-boundary-observation`
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-source-read-v1.json`](../fixtures/benchmarks/bgi-browser-source-read-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This observation repeats the complete `kernel32-source-file-read` BGI
session in one Chrome Dedicated Worker. Each session starts from a new Rust
runtime and v86 mirror, dispatches 1,389 ordered calls over 11,476,233 guest
instructions, performs 47,668 Rust/Wasm bridge operations, and applies four
initial-thread stack-growth commits. It opens the fingerprinted asset twice,
performs two complete 125,461-byte source reads through the same-origin service,
closes both typed file lifetimes, and reaches the same `CreateWindowExA`
contract-missing stop.

The session includes all earlier process, NLS, heap, synchronization, display,
COM, WinMM, icon／cursor, window-class, system-metric, large-heap, and source-file contracts. Every
repetition checks call and instruction counts, crossing count, large-allocation
identity, materialization identity, source completion count, memory digest, next
structured stop, and zero pending work before retaining only a compact scalar
summary. Completed report payloads, including source-read records, are released
after scalar capture. The storage／Worker boundary changed in this checkpoint, so
the workload was rerun against the exact current Rust/Wasm and Worker artifacts.

This is title-workload evidence. The synthetic CPU／ABI／audio／background gate
remains separately versioned in [v86-browser-benchmark.md](v86-browser-benchmark.md);
neither result substitutes for the other.

## Measurement

The page records main-thread `usedJSHeapSize` and a 5 ms heartbeat. The runner
attaches to the related Dedicated Worker through Chrome DevTools Protocol and
samples V8 used heap, heap capacity, embedder heap, and backing storage every 5
seconds. RuntimeBridge and the v86 adapter report their current linear-memory
sizes at every completed session. No forced garbage collection is used.

Heap growth is ordinary least-squares slope across every available time sample,
not a first／last delta. Session, maximum-slice, and heartbeat-gap distributions
retain p50, p95, p99, and maximum values. Heartbeat uses a P99 responsiveness
gate plus a separate hard freeze ceiling. The report stores all 17 threshold evaluations,
including observed value, comparison, limit, and pass result; an unavailable
required sample is a failure. Report schema version 2 also stores a structured
`repeated-distribution-threshold` decision: `passed`, reason
`all-17-versioned-thresholds-passed`, evaluation count `17`, and no failure IDs.

## Result

The authoritative clean-profile Chrome run completed 35 sessions with no
failure. The source-read version 1 thresholds were fixed before this 180-second
run from the completed semantic single-session measurement. The previous short
large-heap workload configuration remains unchanged as historical evidence; its
throughput and latency thresholds are not applied to this different workload.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 24 | 35 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 38 / 37 |
| Session latency p95 | <= 8,000 ms | 5,452.690 ms |
| Maximum slice p99 | <= 10 ms | 2.765 ms |
| Main heartbeat gap p99 | <= 25 ms | 5.990 ms |
| Main heartbeat gap maximum | <= 250 ms | 37.020 ms |
| Main used heap maximum | <= 16,777,216 bytes | 4,247,223 bytes |
| Main heap slope | <= 2,097,152 bytes/min | 239,735.32 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 29,416,272 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 1,172,205.90 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 59,465,728 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 39,639,066 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | 488,571.14 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 10,027,008 bytes |
| Execution-engine linear memory | <= 16,777,216 bytes | 13,893,632 bytes |

The exact report is 932,580 bytes with SHA-256
`08d17eb8d1eaca0a75fb05bbdfbf8fc26fd8a62bb210cd8dc1fd2e4f02108c5f`.
The hash identifies this run; the versioned thresholds and semantic assertions
are the regression gate. The report also carries the six-class policy from
[evidence-classification.md](evidence-classification.md).

## Completed-report retention correction

The first 1,333-call run completed 348 sessions with zero semantic failure and
passed 16 thresholds. Main heap had one 19,007,386-byte sample at 20 seconds,
above the unchanged 16,777,216-byte ceiling; final heap was 5,795,191 bytes and
the slope 942,509.87 bytes/minute, while every Worker／latency／heartbeat metric
passed. The raw failed report was replaced by the passing rerun; its recorded
identity was 1,061,256 bytes with SHA-256
`ed23f3cd92a5ec50150dfc64743fa18ac49814998e19d83ab7d8e9c63c8593c3`.

Code inspection showed benchmark main retained each full 1,333-call
`SessionReport` across the deliberate 20 ms pacing await after already copying
the compact scalar session sample. The harness now clears completed report
arrays at that ownership boundary. Validation, semantic fields, sampling
interval, forced-GC policy, and all 17 thresholds are unchanged. The passing
historical passing rerun reduced the maximum to 11,661,344 bytes and had a
negative main-heap slope.

## Adapter loader correction

The first protocol v4 candidate verified and compiled the 1.88 MiB adapter for
every runtime session. Semantics and 15 thresholds passed, but Worker backing
storage peaked at 68,054,066 bytes, above the unchanged 67,108,864-byte limit.
The failed report is 911,467 bytes with SHA-256
`9ce6411d98c02f7e7b2d3344ad6d26e59e9ba164810646343d35a74657f791f7`.

Artifact hash, source revision, and ABI are Worker-level invariants, so the
loader now verifies and compiles once, then creates a fresh engine instance for
each session from the verified `WebAssembly.Module`. No CPU context, mapping,
guest memory, or Rust runtime is shared. The passing run reduced peak backing
storage below the unchanged limit. That correction remains part of the loader
contract; this current benchmark observes 39,639,066
bytes and retains bounded p95 latency without changing any threshold.

## Environment

| Field | Value |
|---|---|
| Host | Windows `10.0.26200` x64 |
| CPU | AMD Ryzen 7 8840U with Radeon 780M Graphics |
| Logical CPUs | 16 |
| Host memory | 16,398,209,024 bytes |
| Chrome | `150.0.7871.127` |
| Node.js | `v24.18.0` |

The report records base implementation commit `a31e8576811097bfd22d4694a107d952924d4aef`
and `dirty: true` for the benchmark implementation under test.
That Git value is run identity rather than the semantic gate; exact generated
artifacts are independently fingerprinted below.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 2,759,668 | `8bcfe96589eb0a9922c6c2be6f9ee47205ad31be815b16e39858642e5701194b` |
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Runtime Worker bundle | 134,815 | `2ef1635edf06d770b6166c063a9084a2f44224ab5272960d7194beb182d7f796` |
| BGI benchmark bundle | 9,321 | `b56b745b6be47383a94e549a7d4ed083b467e74d7176cb1bd783a79c0f455d36` |
| Execution-engine profile | 545 | `792980bc78b58d8aae491c72b9ea13f39176206306321069a4f2bcb2989e7250` |
| Source-read version 1 configuration | 1,166 | `660d9dac50429430a8c344576cc9da3f83f9496053765314c35be98d25252111` |

The private executable is never copied into the browser build or Git. The
runner serves it only from its installed path and verifies its 1,193,472-byte
SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
before and after the run.

## Reproduction

```powershell
npm run check:runtime:bgi-browser-benchmark
```

With the toolchain and adapter already prepared:

```powershell
npm run check:runtime:bgi-browser-benchmark:prepared
```

Both commands require the private BGI file and use a clean temporary Chrome
profile plus a loopback-only server and DevTools endpoint.

## Non-claims

- Three minutes of repeated startup is not a full-play or long-idle soak.
- The reached path has no title rendering, audible output, DOM input delivery,
  writable persistence, save／load, or general VFS workload. Source coverage is
  two full reads of one immutable object; it does not cover large-corpus streaming,
  replacement recovery, directory enumeration, writes, restart, or durability.
  COM coverage is limited to per-thread initialization, and
  input coverage is limited to the logical mouse-button mapping transaction.
  Waitable-timer coverage is limited to creation and initial nonsignaled
  lifetime; no due-time, period, signal, or cancellation workload is measured.
  Icon coverage is limited to logical module-resource selection and identity;
  cursor coverage is limited to shared `IDC_ARROW` identity. Window-class
  coverage is limited to two process-local registrations and logical ATOM／
  metadata lifetime. System-metric coverage is limited to five initial-window
  reads. Large-heap coverage is one exact 64 MiB reserve and the pages reached
  during startup; multiple／arbitrary large allocations and long-lived page growth are
  not measured. No window creation, WndProc dispatch, bitmap／cursor
  rendering, hotspot, active cursor, or browser resource is measured.
- A passing slope threshold does not prove that V8 heap capacity has reached a
  permanent plateau; longer scenario-specific gates remain required.
- Additional-thread stack lifecycle, stack overflow, and all title output remain
  outside the completed workload.

## Heartbeat distribution correction

The first 202-call 180-second candidate completed 411 sessions with zero
semantic failure. Session P95 was 527.095 ms and slice P99 was 2.050 ms, but a
single 111.865 ms heartbeat gap exceeded the former 75 ms maximum-only gate;
the other 15 thresholds passed. Power-state changes did not address the
classification problem.

The benchmark now retains every 5 ms heartbeat gap and evaluates P99 <= 25 ms
for sustained responsiveness plus maximum <= 250 ms for an actual freeze. This
follows the architecture rule that performance uses repeated distributions
while still bounding catastrophic stalls. No session, slice, heap, backing, or
linear-memory threshold was relaxed.

## Yield correction

The 57-call candidate initially failed the unchanged gate twice while the
Runtime Worker used `setTimeout(0)` after every 8-instruction slice:

| Attempt | Sessions / minimum | p95 / maximum | Semantic failures |
|---|---:|---:|---:|
| 1 | 63 / 120 | 3,138.965 / 3,000 ms | 0 |
| 2 | 100 / 120 | 3,026.905 / 3,000 ms | 0 |

The second failed report is 64,227 bytes with SHA-256
`f3b5f849c81db5c320b1eca87b252b68c97bbaa522b10a4d8ce430359148e2bb`.
All slice, heartbeat, heap, backing-storage, linear-memory, and failure-count
thresholds passed. The CPU work and Wasm artifacts were unchanged; latency came
from OS-dependent timer clamping across 209 yields. Replacing the timer with a
local `MessageChannel` task retained cancellation at 8 instructions and reduced
the authoritative p95 to 97.010 ms. `scheduler.yield()` was tested and rejected
because its priority continuation starved the external cancellation message.
The threshold configuration was not changed.
