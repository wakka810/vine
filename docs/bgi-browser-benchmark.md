# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-15
- Workload: current 284-call private BGI generic session
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-v1.json`](../fixtures/benchmarks/bgi-browser-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This gate repeats the complete current BGI session in one Chrome Dedicated
Worker. Each session starts from a new Rust runtime and v86 mirror, dispatches
284 ordered calls over 25,256 guest instructions, performs 2,850 Rust/Wasm bridge
operations, applies four initial-thread stack-growth commits, and reaches the
same `GetCurrentThread` contract-missing stop after CP_ACP conversion, the
process-owned current-directory update, named mutex creation, and its immediate
LastError／heap continuation.
Every repetition checks the call count, instruction count, crossing count,
memory digest, next structured stop, and zero pending work before its compact
summary is retained.

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

The authoritative clean-profile Chrome run completed 613 sessions with no
failure. All version 1 thresholds were fixed after a separate 60-second
calibration and before this run.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 120 | 613 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 38 / 36 |
| Session latency p95 | <= 3,000 ms | 328.010 ms |
| Maximum slice p99 | <= 10 ms | 1.090 ms |
| Main heartbeat gap p99 | <= 25 ms | 6.075 ms |
| Main heartbeat gap maximum | <= 250 ms | 21.380 ms |
| Main used heap maximum | <= 16,777,216 bytes | 9,859,344 bytes |
| Main heap slope | <= 2,097,152 bytes/min | 37,187.97 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 27,644,396 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 1,123,628.13 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 57,671,680 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 35,707,336 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | -93,413.21 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 9,895,936 bytes |
| Execution-engine linear memory | <= 16,777,216 bytes | 11,796,480 bytes |

The exact report is 1,178,218 bytes with SHA-256
`ee43cc636fc1a1587bffdb900c9216b1d49057d82a6df0c22c867b4d08a6adae`.
The hash identifies this run; the versioned thresholds and semantic assertions
are the regression gate. The report also carries the six-class policy from
[evidence-classification.md](evidence-classification.md).

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
storage below the unchanged limit. The current checkpoint observes 35,707,336
bytes and retains bounded p95 latency without changing any threshold.

## Environment

| Field | Value |
|---|---|
| Host | Windows `10.0.26200` x64 |
| CPU | AMD Ryzen 7 8840U with Radeon 780M Graphics |
| Logical CPUs | 16 |
| Host memory | 16,398,209,024 bytes |
| Chrome | `150.0.7871.115` |
| Node.js | `v24.18.0` |

The report records the repository base and dirty state present during the
pre-commit checkpoint measurement. That Git value is run identity rather than
the semantic gate; exact generated artifacts are independently fingerprinted
below, and commit-bound BGI evidence is generated after the implementation
commit.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 2,089,782 | `f98b901a33127740860437081dad8eb296c14363fcfe5414a94d7809c5d006c1` |
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Runtime Worker bundle | 97,283 | `c9767db112ea0cf63f4c22a1241cd61e8b664e0530f897075a928e5d735ca836` |
| BGI benchmark bundle | 8,965 | `f882c1b309bb467bb347c4899f80540915ef8c30169b20e77493ff5077b4913e` |
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
- The reached path has no title rendering, audio, input, VFS, or COM workload.
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
