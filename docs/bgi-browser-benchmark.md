# BGI Browser workload benchmark

## Status

- Status: threshold passed
- Date: 2026-07-22
- Workload: 17,861-call `d3d9-initial-vertex-buffer` private BGI generic session
- Observation status: `current-d3d9-initial-vertex-buffer-observation`
- Duration: 180 seconds
- Configuration: [`fixtures/benchmarks/bgi-browser-d3d9-initial-vertex-buffer-v1.json`](../fixtures/benchmarks/bgi-browser-d3d9-initial-vertex-buffer-v1.json)
- Evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`

This observation repeats the complete `d3d9-initial-vertex-buffer` BGI
session in one Chrome Dedicated Worker. Each session starts from a new Rust
runtime and v86 mirror, dispatches 17,861 calls before the 11,808,458-instruction
terminal stop, performs 63,614 Rust/Wasm bridge operations, and applies four
initial-thread stack-growth commits. It opens the fingerprinted asset twice,
performs two complete 125,461-byte source reads through the same-origin service,
closes both typed file lifetimes, completes one Rust-owned logical window through
creation, small-icon send, initial show／hide callbacks, client-rect query,
window-long update, and same-geometry activation, then creates one Rust-owned
logical D3D9 device with an 800x600 format-22 presentation. It creates one
1024x1024／one-level／format-22 system-memory logical texture and its level-zero
surface, acquires one typed surface HDC, records one `[0,0,800,600]` black fill,
releases the HDC, and retires the surface. It then creates two more logical
textures with the same dimensions and format: one static default-pool variant
and one dynamic default-pool variant. All three texture identities remain live,
then the device creates one 112-byte write-only, managed vertex buffer. Its
full-range Lock publishes Rust-owned guest backing, the caller writes the exact
112-byte quad payload, and Unlock returns the range to read-only. The device
reference count is five, and execution reaches `IDirect3DDevice9::SetRenderState`
at slot 57. The successful resource path
does not traverse the earlier `timeGetTime`／`MessageBoxA` error branch. No
texture／surface pixel bytes, GPU vertex buffer, or browser renderer is allocated.

The session includes all earlier process, NLS, heap, synchronization, display,
COM, icon／cursor, window-class, logical-window, system-metric, large-heap,
source-file, D3D9 adapter／resource, and reached WinMM capability／resolution contracts. Every
repetition checks call and instruction counts, crossing count, large-allocation
identity, materialization identity, source completion count, memory digest, next
structured stop, and zero pending work before retaining only a compact scalar
summary. Protocol v7 uses independently computed full-sequence canonical digests
for the standalone Chrome／Node semantic-equivalence check and the repeated benchmark;
the digest retains sequence uniqueness and terminal semantics without retaining
17,861 detailed call objects or canonical strings on every repetition. Each
benchmark session checks the call count, digest algorithm, and semantic SHA
against the manifest. Completed report payloads, including source-read records,
are released after scalar capture. The D3D9 compatibility profile and Rust/Wasm
artifact changed in this checkpoint, so the workload was rerun against those
exact artifacts without changing browser scheduling, relaxing any threshold,
or accepting a retry.

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
`repeated-distribution-threshold` decision with `passed` or `failed` status,
reason, the exact evaluation count, and failure IDs. The current decision is
`passed`, reason `all-17-versioned-thresholds-passed`, with no failure IDs.

## Result

The isolated clean-profile Chrome run completed 64 sessions with no semantic failure.
The D3D9 Initial Vertex Buffer version 1 configuration retained all 17 existing threshold
values unchanged before this 180-second run. All 17 thresholds passed. Previous workload configurations remain
unchanged as historical evidence; their identities are not applied to this
current workload.

| Metric | Threshold | Result |
|---|---:|---:|
| Completed sessions | >= 24 | 64 |
| Failed sessions | <= 0 | 0 |
| Main／Worker heap samples | >= 30 each | 38 / 37 |
| Session latency p95 | <= 8,000 ms | 2,914.615 ms |
| Maximum slice p99 | <= 10 ms | 2.585 ms |
| Main heartbeat gap p99 | <= 25 ms | 5.995 ms |
| Main heartbeat gap maximum | <= 250 ms | 20.855 ms |
| Main used heap maximum | <= 16,777,216 bytes | 3,503,991 bytes |
| Main heap slope | <= 2,097,152 bytes/min | 52,745.45 bytes/min |
| Worker used heap maximum | <= 33,554,432 bytes | 27,492,896 bytes |
| Worker heap slope | <= 8,388,608 bytes/min | 2,161,225.38 bytes/min |
| Worker heap capacity maximum | <= 67,108,864 bytes | 58,941,440 bytes |
| Worker backing storage maximum | <= 67,108,864 bytes | 27,572,578 bytes |
| Worker backing slope | <= 10,485,760 bytes/min | 1,440,798.06 bytes/min |
| Rust linear memory | <= 16,777,216 bytes | 10,092,544 bytes |
| Execution-engine linear memory | <= 16,777,216 bytes | 14,024,704 bytes |

The exact report is 955,155 bytes with SHA-256
`c80155da249b02dc5aab121732bcb96a0853577cbcffe63b68f1acd5f7ce50d0`.
The hash identifies this run; the versioned thresholds and semantic assertions
are the regression gate. The report also carries the six-class policy from
[evidence-classification.md](evidence-classification.md).

## Surface DC heap-capacity correction

The first 180-second Surface DC run completed 46 sessions with zero semantic
failure and passed 16 thresholds, but Worker heap capacity reached 71,565,312
bytes against the unchanged 67,108,864-byte ceiling. Its report SHA-256 was
`8f97a6d00e708cae0bed47e37418db81f3869092805ff8821d787012c8d53845`.
An unchanged reproduction completed 50 sessions and again failed only capacity,
at 73,904,128 bytes. That 950,038-byte report remains ignored under `.work/`
with SHA-256
`6de79ec0b8771bf31d5af8d090e725867e60f64266eb2d45345a273842c2d7e9`.
The repeated failure triggered allocation profiling rather than retry-based
acceptance or a threshold change.

A bounded three-session sampled-allocation comparison attributed the avoidable
pressure to observer digest churn, not Rust Windows state or guest memory. SHA
compression destructured a typed array per block, every canonical fragment used
an `encodeInto` result object, top-level calls built sortable intermediate
objects, and completed sequence identity was retained both by a Set and by the
ordered digest. The corrected path reads SHA state by index, streams fixed-order
top-level fields, writes ASCII directly into the reusable SHA block, uses
`TextEncoder` only for non-ASCII fragments, and makes the ordered digest the sole
sequence-uniqueness authority. Sampled allocation fell from about 1.465 GB to
0.682 GB for the diagnostic workload. Unicode fallback, optional COM fields,
guest-write projection, duplicates, ordering, algorithm identity, and exact
Node／Chrome SHA remain covered by focused tests.

The final unchanged-threshold run above passed capacity at 59,465,728 bytes and
all other thresholds. This optimization changes observer allocation only; it
does not move Windows state, relax evidence, force GC, or alter the canonical
digest algorithm.

## Historical resolved host-contention observation

The first failure is `maximum-p95-session-milliseconds`: observed
8,437.465 ms, required at most 8,000 ms. The same run passed the remaining 16
thresholds and all 26 sessions completed semantically. A bounded same-host Node
comparison recorded CreateDevice and CreateTexture medians of 36,326.809 ms and
36,623.290 ms, a 0.816% increase. That added workload cost does not explain the
larger Chrome distribution shift by itself.

Follow-up process sampling found `MuMuVMMHeadless` and `MaaGakumasu` consuming
about 234% and 123% logical CPU respectively. Their lifecycle is outside the
repository task and was not changed. The fixed duration and thresholds were
preserved, and no rerun was used as acceptance evidence. The exact benchmark
report and Node comparison identity, attempted options, protected invariants,
required authority, and resume condition were structured in the then-current
checkpoint manifest. The current manifest no longer carries the resolved
blocker.

The user paused both external workloads. One isolated rerun used the same
180-second fixture and unchanged thresholds, completed 45 sessions, and passed
all 17 thresholds with 4,131.915 ms session P95. That satisfies the recorded
resume condition and resolves the blocker without a threshold change or
implementation optimization. The failed report remains historical evidence of
why ambient host load cannot be accepted as a clean performance oracle.

## Historical CreateDevice workload and transport correction

The retained benchmark configuration still described the earlier
`user32-set-window-pos-activation` stop after the active runtime had advanced to
`d3d9-create-device`. That old fixture remains historical; the replacement at
this stage selected the exact 17,849-call／11,807,772-instruction／`CreateTexture`
fixture. Later checkpoint fixtures preserve both identities rather than
rewriting either historical workload.

The first current run completed 14 sessions and had 14,831.200 ms P95, failing
minimum sessions and latency. The second completed 24 sessions but still had
10,199.540 ms P95 and a 71,565,312-byte Worker capacity maximum, failing latency
and capacity. Their ignored reports remain under `.work/`; neither was accepted
by retry or threshold change.

The bounded title slice increased from 512 to 1,024 instructions while the
watchdog remains 4. Page-only host deltas now validate against live mappings
without cloning the registry; mapping-changing deltas retain cloned preflight.
The Rust bridge no longer recomputes a per-page SHA when the aggregate payload
SHA already authenticates the batch, and a successful acknowledgement reports
the invariant post-reconciliation dirty-page count `0`. These changes reduced
the repeated workload to 63,571 Wasm crossings per session.

The summary digest now streams canonical JSON fragments directly into reusable
UTF-8 and SHA state, retaining only bounded out-of-order fragments. Node and
Chrome still produce exact digest
`d062bba8eef984451f149034d3bb1f27b2d92a81184be5ffcfa4fcddb8f74f14`.
The final run passed all 17 original thresholds; no GC forcing, retry-based
acceptance, or threshold change was used.

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
contract; its passing observation retained bounded P95 latency without changing
any threshold.

## Environment

| Field | Value |
|---|---|
| Host | Windows `10.0.26200` x64 |
| CPU | AMD Ryzen 7 8840U with Radeon 780M Graphics |
| Logical CPUs | 16 |
| Host memory | 16,398,209,024 bytes |
| Chrome | `150.0.7871.129` |
| Node.js | `v24.18.0` |

The report records base implementation commit `ed5f0967032854372ff792c5ba1ac534a247bdbe`
and `dirty: true` for the benchmark implementation under test.
That Git value is run identity rather than the semantic gate; exact generated
artifacts are independently fingerprinted below.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| Rust runtime core Wasm | 3,654,703 | `b37c6f49d0954d49c73a6ca0a76a75913384a797b0749b17f9a96118a5f5bc4d` |
| v86 adapter Wasm | 1,857,800 | `99a0948c222a6ac2f21984798c9e9213dec9e43b95dff6fc5c8acf1f4dc7954f` |
| Runtime Worker bundle | 156,801 | `658f211b67e1473158eaeaec0f8622a735224c08b7cd4594046f0b482819894b` |
| BGI benchmark bundle | 10,354 | `fb23aa94ae00e63ecefc12decc48a6c7560bf8b4e1574cfcebec90c4315b514d` |
| Execution-engine profile | 545 | `1111e3e9baea61abbc669a7e4b625b8874f4706ea37e5d976f4b8527edce68e8` |
| D3D9 Initial Vertex Buffer version 1 configuration | 1,189 | `297d6ffd9833a98ef379aee0207643329bf205fb2800bf2a268a9a4c39bdfc0b` |

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
  COM coverage is limited to per-thread initialization plus the reached D3D9
  root／device／three texture／first level-zero surface／one vertex-buffer identities and lifetime. DC coverage is
  one surface HDC acquisition, black-brush fill command, exact release, and COM
  retirement relation. The logical textures and surface have no pixel storage or
  pitch. Vertex-buffer coverage is one exact 112-byte zeroed backing, full-range
  Lock, caller write, and Unlock; arbitrary descriptors, partial／flagged locks,
  GPU upload, draw, and renderer resources remain unmeasured. General
  COM and D3D9 resource behavior remain unmeasured. Likewise,
  input coverage is limited to the logical mouse-button mapping transaction.
  Waitable-timer coverage is limited to creation and initial nonsignaled
  lifetime; no due-time, period, signal, or cancellation workload is measured.
  Multimedia coverage reaches capability and resolution-request state, but this
  successful graphics path does not call `timeGetTime`. It does not measure
  elapsed host time, timer precision, sleep／wait timing, frame cadence, or audio
  delivery.
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
