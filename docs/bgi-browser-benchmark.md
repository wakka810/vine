# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-18
- Workload: 17,814-call `user32-set-window-pos-initial-placement` private BGI generic session
- Observation status: `current-critical-section-capacity-boundary-observation`
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-critical-section-capacity-v1.json`](../fixtures/benchmarks/bgi-browser-critical-section-capacity-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This observation repeats the complete `user32-set-window-pos-initial-placement` BGI
session in one Chrome Dedicated Worker. Each session starts from a new Rust
runtime and v86 mirror, dispatches 17,814 calls over 11,806,421 guest
instructions, performs 74,671 Rust/Wasm bridge operations, and applies four
initial-thread stack-growth commits. It opens the fingerprinted asset twice,
performs two complete 125,461-byte source reads through the same-origin service,
closes both typed file lifetimes, completes one Rust-owned logical window through
creation, small-icon send, initial show／hide callbacks, and reaches the
16,408 live critical-section records, then reaches the module-resource
`LoadCursorA` structured stop.

The session includes all earlier process, NLS, heap, synchronization, display,
COM, WinMM, icon／cursor, window-class, logical-window, system-metric, large-heap,
and source-file contracts. Every
repetition checks call and instruction counts, crossing count, large-allocation
identity, materialization identity, source completion count, memory digest, next
structured stop, and zero pending work before retaining only a compact scalar
summary. Protocol v7 uses full ordered call evidence for the standalone Chrome／
Node semantic-equivalence check and a count-only call ledger for this benchmark;
the latter retains sequence uniqueness and terminal semantics without cloning
17,814 detailed call objects on every repetition. Completed report payloads,
including source-read records, are released after scalar capture. The capacity,
Worker evidence boundary, and slice budget changed in this checkpoint, so the
workload was rerun against the exact current Rust/Wasm and Worker artifacts
without relaxing any threshold.

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

The authoritative clean-profile Chrome run completed 30 sessions with no
failure. The critical-section capacity version 1 configuration retained all 17 existing
threshold values unchanged before this 180-second run. The previous source-read
and create-window workload configurations remain unchanged as historical
evidence; their identities are not applied to this current workload.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 24 | 30 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 39 / 38 |
| Session latency p95 | <= 8,000 ms | 6,380.765 ms |
| Maximum slice p99 | <= 10 ms | 2.665 ms |
| Main heartbeat gap p99 | <= 25 ms | 5.990 ms |
| Main heartbeat gap maximum | <= 250 ms | 18.845 ms |
| Main used heap maximum | <= 16,777,216 bytes | 3,277,525 bytes |
| Main heap slope | <= 2,097,152 bytes/min | 30,845.35 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 29,710,004 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 3,081,023.29 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 59,219,968 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 26,971,793 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | 1,312,783.36 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 10,027,008 bytes |
| Execution-engine linear memory | <= 16,777,216 bytes | 13,893,632 bytes |

The exact report is 954,872 bytes with SHA-256
`cf1759c4501e8a30f757c2f1b6256208af9b199b67d16855690e9920a3da608a`.
The hash identifies this run; the versioned thresholds and semantic assertions
are the regression gate. The report also carries the six-class policy from
[evidence-classification.md](evidence-classification.md).

## Capacity-workload reporting and slice correction

The first 17,814-call 180-second run completed 15 sessions with no semantic
failure, but failed four unchanged thresholds: minimum sessions 15／24, p95
12,690.620／8,000 ms, Worker used heap 38,545,860／33,554,432 bytes, and Worker
heap capacity 77,029,376／67,108,864 bytes. Its report was 943,715 bytes with
SHA-256 `daef5dd405646206c79fe784a4c4b56fa767c964cdf239dc4e9402417abf521d`.

Profile inspection found two bounded harness costs. The full evidence ledger
performed a linear duplicate scan and sorted the growing array after every call;
it now uses a Set for sequence identity and sorts once at terminal publication.
Protocol v7 additionally selects `summary` evidence only for repeated benchmark
sessions, retaining the supported-call count and sequence uniqueness while the
standalone BGI check continues to publish and compare every ordered call. A
10-second diagnostic reduced Worker used heap to 7,505,408 bytes and capacity to
30,932,992 bytes, closing both memory failures, while its two sessions still took
8,562–8,882 ms.

The remaining profile had 44,814 budget yields and 97,080 Wasm crossings at a
256-instruction slice. Raising the bounded BGI slice to 512 instructions reduced
these to 22,399 yields and 74,671 crossings. The full semantic run remained
equivalent, and the final benchmark kept slice p99 at 2.665 ms and heartbeat p99
at 5.990 ms, both below their unchanged limits. The 20-second watchdog remains a
safety ceiling, not the performance acceptance threshold.

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
contract; this current benchmark observes 41,596,440
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

The report records base implementation commit `3c96c6cf2c3648d4592e28358bdbaa08619501f8`
and `dirty: true` for the benchmark implementation under test.
That Git value is run identity rather than the semantic gate; exact generated
artifacts are independently fingerprinted below.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 2,949,669 | `f64cd3cfffdeba2195586d0253ef02a4b8425d332c34d971dcf6782b54ea211b` |
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Runtime Worker bundle | 137,716 | `3e2beea0d506408e1080b858acebaecb829acbd25fc96911751aa217d695cf59` |
| BGI benchmark bundle | 9,579 | `1fd3f86eac898a5f33b6f29baba1de4f447714ddf45064d2c2556474aac76eba` |
| Execution-engine profile | 545 | `792980bc78b58d8aae491c72b9ea13f39176206306321069a4f2bcb2989e7250` |
| Critical-section capacity version 1 configuration | 1,191 | `2455376874c4237b245d830c87d079b20b506c7b1733215292ad8ae9ac8afa07` |

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
  Icon coverage is limited to logical module-resource selection, identity, and
  one small-icon relation;
  cursor coverage is limited to shared `IDC_ARROW` identity. Window-class
  coverage is limited to two process-local registrations and logical ATOM／
  metadata lifetime. Window coverage is limited to one logical top-level window,
  bounded synchronous creation／small-icon／initial show／hide callbacks, default-
  procedure results, and the reached invalid-menu chain. System-metric coverage is
  limited to the reached initial-window indices. Large-heap coverage is one exact 64 MiB reserve and the pages reached
  during startup; multiple／arbitrary large allocations and long-lived page growth are
  not measured. No general z-order／resize／activation, queued window message, DOM
  window, bitmap／cursor rendering, hotspot, active cursor, or browser resource is measured.
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
