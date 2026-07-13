# BGI Chrome Runtime Worker checkpoint

## Status

- Status: semantic equivalence passed
- Date: 2026-07-13
- Browser: Chrome `150.0.7871.115`
- Runtime authority: Rust/Wasm inside the Runtime Worker
- Node role: semantic oracle only
- Evidence: `.work/browser-runtime/bgi-browser-report.json`
- Performance evidence: [bgi-browser-benchmark.md](bgi-browser-benchmark.md)

The current private BGI generic session now executes in the Chrome Runtime
Worker. The local gate serves the fingerprinted executable directly from its
installed path; it does not copy the binary into `.work`, the browser build, or
Git. Rust/Wasm parses and materializes the image, owns `GuestMemory` and
`RuntimeState`, binds the process import table, installs the process environment
and dynamic exports, and dispatches the same Kernel32 contracts and compatibility
policy used by the Node oracle.

## Equivalence result

| Evidence | Chrome Worker | Node oracle |
|---|---:|---:|
| Supported calls | 60 | 60 |
| Guest instructions at next stop | 1,516 | 1,516 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 224 | 120 |
| Host delta batches | 30 | 30 |
| Final memory content SHA-256 | `5bb58fba2f503a84550a71e0ba35f11049e77ebb9038461db91f04d0dfb3bb61` | same |
| Next capability cluster | `kernel32-api-contract` | same |
| Next API | `KERNEL32.dll!SetHandleCount` | same |
| Fault | `endpoint-contract-missing` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 175
writeback batches; the Node oracle reconciles only at 61 trap boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 214 CPU slices, 153 budget yields, 269 Rust/Wasm bridge
operations, and a maximum observed slice of 2.055 ms. The main-thread heartbeat
recorded 41 samples with a 21.03 ms maximum gap while the Worker ran. A
single-run `usedJSHeapSize` observation changed from 628,738 to 708,542 bytes;
this remains run identity. The separate versioned BGI workload gate repeats 1,807
complete sessions and measures main／Worker heap, backing storage, linear memory,
slice distribution, heartbeat, and Wasm crossings.

Separate cancellation and watchdog probes stop after 8 and 4 instructions.
Both retire pending host calls and callbacks, detach the callback-free system
module graph, and leave the Rust process in `exited` state. Cancellation can be
delivered only between complete CPU/writeback transactions.

The initial mirror uses the same 2 MiB physical pool as the controlled fixture
and skips the v86 VGA MMIO hole at `0xA0000..0xC0000`. The reached heap allocation
applies one reserve and one commit operation in the Worker; later mutations use
dirty page deltas.

## Reproduction

```powershell
npm run check:runtime:bgi-browser
```

The command regenerates the Node oracle, rebuilds the Rust/Wasm and Worker
artifacts, runs clean-profile Chrome success/cancel/watchdog probes, verifies the
source fingerprint before and after, and compares semantic results. The exact
report hash is artifact identity only.

## Non-claims

- no `SetHandleCount` behavior or capability after the current stop is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
