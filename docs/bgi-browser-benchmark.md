# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-13
- Workload: current 57-call private BGI generic session
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-v1.json`](../fixtures/benchmarks/bgi-browser-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This gate repeats the complete current BGI session in one Chrome Dedicated
Worker. Each session starts from a new Rust runtime and v86 mirror, dispatches
57 ordered calls over 1,484 guest instructions, performs 260 Rust/Wasm bridge
operations, and reaches the same `GetStdHandle(0xfffffff4)` structured stop.
Every repetition checks the call count, instruction count, crossing count,
memory digest, next API and fault, and zero pending work before its compact
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
not a first／last delta. Session and maximum-slice distributions retain p50,
p95, p99, and maximum values. The report stores all 16 threshold evaluations,
including observed value, comparison, limit, and pass result; an unavailable
required sample is a failure.

## Result

The authoritative clean-profile Chrome run completed 1,750 sessions with no
failure. All version 1 thresholds were fixed after a separate 60-second
calibration and before this run.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 120 | 1,750 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 38 / 36 |
| Session latency p95 | <= 3,000 ms | 97.010 ms |
| Maximum slice p99 | <= 10 ms | 0.810 ms |
| Main heartbeat gap | <= 75 ms | 20.985 ms |
| Main used heap maximum | <= 16,777,216 bytes | 6,626,295 bytes |
| Main heap slope | <= 2,097,152 bytes/min | -295,486.19 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 26,665,220 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 1,981,278.25 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 57,409,536 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 51,094,197 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | 2,506,238.75 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 9,764,864 bytes |
| v86 linear memory | <= 16,777,216 bytes | 11,796,480 bytes |

The exact report is 778,494 bytes with SHA-256
`223fe48163e9665a2584bce9fbc869d438402ad150446977bdd506f03695a26e`.
The hash identifies this run; the versioned thresholds and semantic assertions
are the regression gate.

## Environment

| Field | Value |
|---|---|
| Host | Windows `10.0.26200` x64 |
| CPU | AMD Ryzen 7 8840U with Radeon 780M Graphics |
| Logical CPUs | 16 |
| Host memory | 16,398,209,024 bytes |
| Chrome | `150.0.7871.115` |
| Node.js | `v24.18.0` |

The report records repository commit `2d65e1cd231b94b21b02b067d6f048e1b8362e7b`
with a dirty tree because the evidence was captured before the Item 9 checkpoint
commit. Exact generated artifacts are independently fingerprinted below.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 1,516,032 | `83c8a1d31c632c6d063c62864ef22a114491e7d26611d837444403e00144a870` |
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Runtime Worker bundle | 64,685 | `862dad699640312fa2fb8db1573e9ccd6e314029cd29465e5d3ed3f68e789bd0` |
| BGI benchmark bundle | 7,103 | `7902ebcea4cf91c1873f58911c76f99c05d05517fd0609027463c5f5c8c04d07` |
| Version 1 configuration | 915 | `44d985f746ce06ed6e6d7f76ac35deb75e937e9bfbfa31419366855c27a2f922` |

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
- Standard error identity and all title output remain outside this workload.

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
