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
| Supported calls | 55 | 55 |
| Guest instructions at next stop | 1,447 | 1,447 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 212 | 113 |
| Host delta batches | 28 | 28 |
| Final memory content SHA-256 | `db2bc5a3d444739b1328defc5e18829d567f3a4ab6f8ff42ee939f1c30656def` | same |
| Next capability cluster | `kernel32-api-contract` | same |
| Next API | `KERNEL32.dll!GetFileType` | same |
| Fault | `file-type-standard-handle-unsupported` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 165
writeback batches; the Node oracle reconciles only at 56 API trap boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 204 CPU slices, 148 budget yields, 252 Rust/Wasm bridge
operations, and a maximum observed slice of 2.61 ms. The main-thread heartbeat
recorded 244 samples with a 23.32 ms maximum gap while the Worker ran. A
single-run `usedJSHeapSize` observation changed from 628,738 to 720,434 bytes;
this remains run identity. The separate versioned BGI workload gate repeats 168
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

- no capability after the current `GetFileType` stop is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
