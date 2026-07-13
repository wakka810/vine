# BGI Chrome Runtime Worker checkpoint

## Status

- Status: semantic equivalence passed
- Date: 2026-07-14
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
| Supported calls | 94 | 94 |
| Guest instructions at next stop | 5,894 | 5,894 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 751 | 161 |
| Host delta batches | 37 | 37 |
| Final memory content SHA-256 | `eb77024dd956d49c9a1a9f89a0e248220cd6ab6850203978134aeb93b8c4e811` | same |
| Next capability cluster | `kernel32-api-contract` | same |
| Next API | `KERNEL32.dll!GetStringTypeW(1,0x006FF738,256,0x006FF9D4)` | same |
| Fault | `endpoint-contract-missing` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 700
writeback batches; the Node oracle reconciles at 95 writeback boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 772 CPU slices, 677 budget yields, 2,317 validated Worker
state transitions, 835 Rust/Wasm bridge operations, and a maximum observed slice
of 1.595 ms. The main-thread heartbeat recorded 41 samples with a 20.305 ms
maximum gap while the Worker ran. A single-run `usedJSHeapSize` observation
changed from 628,738 to 740,542 bytes; this remains run identity. The separate
versioned BGI workload gate repeats 1,482
complete sessions and measures main／Worker heap, backing storage, linear memory,
slice distribution, heartbeat, and Wasm crossings.

Separate cancellation and watchdog probes stop after 8 and 4 instructions.
Both retire pending host calls and callbacks, detach the callback-free system
module graph, and leave the Rust process in `exited` state. Cancellation can be
delivered only between complete CPU/writeback transactions.

The explicit state machine ends success, cancellation, and watchdog
in `stopped` after 2,317, 4, and 3 transitions respectively. All three reports
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
and skips the v86 VGA MMIO hole at `0xA0000..0xC0000`. The session's first heap
allocation applies one reserve and one commit operation in the Worker. The
flags-zero size-1 and size-544 allocations reuse the committed band as
dirty-only deltas; the 544-byte BGI copy consumer is detected by the same generic
PTE-dirty reconciliation path. The checked 18-byte `GetCPInfo` output is applied
as a one-page host delta before the same mirror continues through its lead-byte
consumer. The reached `MultiByteToWideChar` conversion adds one checked 512-byte
stack output delta and reaches `GetStringTypeW` with identical Node/Chrome bytes.

## Reproduction

```powershell
npm run check:runtime:bgi-browser
```

The command regenerates the Node oracle, rebuilds the Rust/Wasm and Worker
artifacts, runs clean-profile Chrome success/cancel/watchdog probes, verifies the
source fingerprint before and after, and compares semantic results. The exact
report hash is artifact identity only.

## Non-claims

- no `GetStringTypeW`, broad locale classification, later `HeapFree`, or broad heap variant dispatch is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
