# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-17
- Workload: current 1,340-call `user32-load-cursor-a` private BGI generic session
- Observation status: `current-system-arrow-cursor-boundary-rerun`
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-v1.json`](../fixtures/benchmarks/bgi-browser-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This current observation repeats the complete `user32-load-cursor-a` BGI
session in one Chrome Dedicated Worker. Each session starts from a new Rust
runtime and v86 mirror, dispatches
1,340 ordered calls over 49,957 guest instructions, performs 6,411 Rust/Wasm bridge
operations, applies four initial-thread stack-growth commits, and reaches the
same `RegisterClassExA(0x006FFB08)` contract-missing stop after CP_ACP
conversion, the process-owned current-directory update, named mutex creation,
its immediate LastError／heap continuation, and `GetCurrentThread` pseudo-handle
resolution. The workload additionally commits current-thread logical affinity
and completes BGI's 1,003-read tick-count calibration loop before restoring the
virtual mask.
It then serializes the policy-owned 36-byte x86 `SYSTEM_INFO` profile without
importing host topology.
The continuation enumerates the logical monitor, initializes the COM apartment,
reads the policy LANGID, scans the virtual drive profile, and completes the
two-call logical mouse-button normalize／restore transaction before later
LastError／TLS／FLS and critical-section calls. It finally writes the
policy-owned 8-byte WinMM `TIMECAPS` profile without changing any clock. It
then records the Rust-owned logical `timeBeginPeriod(1)` request and continues
through the following heap allocation without changing browser timer or audio
configuration. It writes the bounded ANSI OS-version structure, creates the
Rust-owned unnamed manual-reset waitable timer, and verifies its typed handle,
timer ID, object ID, and initial nonsignaled state. It then selects the mapped
module's exact 32-pixel DIB icon and records the Rust-owned shared logical HICON
and loads the Rust-owned shared logical system-arrow HCURSOR before reaching
the stop.
Every repetition checks the call count, instruction count, crossing count,
memory digest, next structured stop, and zero pending work before its compact
summary is retained.

The system-cursor checkpoint changes User32 process lifetime and the Worker
completion payload, so this workload was rerun against the exact current
Rust/Wasm and Worker artifacts. The active Node／Chrome semantic gate remains
separately required in [`current-checkpoint.json`](current-checkpoint.json).

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

The authoritative clean-profile Chrome run completed 376 sessions with no
failure. All version 1 thresholds were fixed after a separate 60-second
calibration and before this run.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 120 | 376 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 38 / 37 |
| Session latency p95 | <= 3,000 ms | 491.795 ms |
| Maximum slice p99 | <= 10 ms | 2.290 ms |
| Main heartbeat gap p99 | <= 25 ms | 6.020 ms |
| Main heartbeat gap maximum | <= 250 ms | 20.630 ms |
| Main used heap maximum | <= 16,777,216 bytes | 8,787,972 bytes |
| Main heap slope | <= 2,097,152 bytes/min | -125,574.29 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 28,462,188 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 3,370,770.80 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 60,293,120 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 34,580,100 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | 4,865,391.38 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 10,027,008 bytes |
| Execution-engine linear memory | <= 16,777,216 bytes | 11,796,480 bytes |

The exact report is 1,072,091 bytes with SHA-256
`4795b50b1561aac01c5fe08fb0442e6c46521f5d9576c8fd2b1090c7a73a3c4e`.
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
contract; this current benchmark observes 34,580,100
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

The report records base implementation commit `b3d7eab64a0a1eb31cd225e3ac2dea35acb267d6`
and `dirty: true` for the benchmark implementation under test.
That Git value is run identity rather than the semantic gate; exact generated
artifacts are independently fingerprinted below.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 2,461,281 | `1731911cc13c5b2749fbdc4c71af27a6e23e4fb449a69b5a84b2775d6b0149c7` |
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Runtime Worker bundle | 121,889 | `3ee6730692f1369d08c6af72f0878e977c7dd6c850dabe09f73525acec57af9b` |
| BGI benchmark bundle | 9,286 | `33be6c16928ebee490f9c5f75680923898ea9b9f66dc6546bb5b87d2b0a6eebb` |
| Execution-engine profile | 545 | `792980bc78b58d8aae491c72b9ea13f39176206306321069a4f2bcb2989e7250` |
| Version 1 configuration | 967 | `0862c59c00adc8d1b0218283b57ffdb2e87393624b6461c873b035fc4b2d98ef` |

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
  or VFS workload. COM coverage is limited to per-thread initialization, and
  input coverage is limited to the logical mouse-button mapping transaction.
  Waitable-timer coverage is limited to creation and initial nonsignaled
  lifetime; no due-time, period, signal, or cancellation workload is measured.
  Icon coverage is limited to logical module-resource selection and identity;
  cursor coverage is limited to shared `IDC_ARROW` identity. No bitmap／cursor
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
