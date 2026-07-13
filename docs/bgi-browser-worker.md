# BGI Chrome Runtime Worker checkpoint

## Status

- Status: semantic equivalence passed
- Date: 2026-07-13
- Browser: Chrome `150.0.7871.115`
- Runtime Worker protocol: `vine.browser-runtime-worker.v4`
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
| Supported calls | 64 | 64 |
| Guest instructions at next stop | 1,591 | 1,591 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 234 | 124 |
| Host delta batches | 30 | 30 |
| Final memory content SHA-256 | `9b2b6b23255736f59b3c02c75bb3dd5adf811fb99e93ce40bc31a601ba55fddb` | same |
| Next capability cluster | `kernel32-private-heap-allocation` | same |
| Next API | `KERNEL32.dll!HeapAlloc` | same |
| Fault | `heap-alloc-variant-unsupported` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 185
writeback batches; the Node oracle reconciles only at 65 trap boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 226 CPU slices, 161 budget yields, 679 validated Worker
state transitions, 283 Rust/Wasm bridge operations, and a maximum observed slice
of 2.170 ms. The main-thread heartbeat recorded 35 samples with a 16.595 ms
maximum gap while the Worker ran. A single-run `usedJSHeapSize` observation
changed from 628,738 to 714,098 bytes; this remains run identity. The separate
versioned BGI workload gate repeats 2,182
complete sessions and measures main／Worker heap, backing storage, linear memory,
slice distribution, heartbeat, and Wasm crossings.

Separate cancellation and watchdog probes stop after 8 and 4 instructions.
Both retire pending host calls and callbacks, detach the callback-free system
module graph, and leave the Rust process in `exited` state. Cancellation can be
delivered only between complete CPU/writeback transactions.

The explicit state machine ends success, cancellation, and watchdog
in `stopped` after 679, 4, and 3 transitions respectively. All three reports
retain zero host request records. Success and cancellation carry no watchdog
context; the watchdog records thread `8192`, no active request, and the last
`ready -> running` transition. Request records also retain thread identity,
cancel state, and ready／callback completion policy so duplicate, late, and
wrong-thread completion cannot resume the lane.

Every probe also reports the protocol v4 `ExecutionEngine` identity. The
production loop sees structured engine stops, opaque versioned CPU contexts,
mapping operations, changed page batches, and cache effects only. The v86
source revision, Wasm digest, and CPU-context ABI are checked before guest
startup; PTE layout and private offsets remain adapter-local.

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

- no `HeapAlloc(heap, 0, 1)` behavior or capability after the current stop is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
