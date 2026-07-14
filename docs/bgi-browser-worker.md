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
| Supported calls | 210 | 210 |
| Guest instructions at next stop | 19,185 | 19,185 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 1,640 | 299 |
| Host delta batches | 58 | 58 |
| Final memory content SHA-256 | `90c245267e933a01c1e54fa0305bec1d54a16180a3e7465b6a3b6b3c8d3dd0b8` | same |
| Next capability cluster | `kernel32-critical-section-synchronization` | same |
| Next API | `KERNEL32.dll!InitializeCriticalSection(0x100FD9CC)` | same |
| Fault | `endpoint-contract-missing` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 1,828
writeback batches; the Node oracle reconciles at 211 writeback boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 2,465 CPU slices, 2,254 budget yields, 7,396 validated
Worker state transitions, 2,100 Rust/Wasm bridge operations, and a maximum
observed slice of 2.880 ms. The main-thread heartbeat recorded 95 samples with a 27.345 ms
maximum gap while the Worker ran. A single-run `usedJSHeapSize` observation
changed from 628,738 to 840,218 bytes; this remains run identity. The separate
versioned BGI workload gate repeats 438
complete sessions and measures main／Worker heap, backing storage, linear memory,
slice distribution, heartbeat, and Wasm crossings.

Separate cancellation and watchdog probes stop after 8 and 4 instructions.
Both retire pending host calls and callbacks, detach the callback-free system
module graph, and leave the Rust process in `exited` state. Cancellation can be
delivered only between complete CPU/writeback transactions.

The explicit state machine ends success, cancellation, and watchdog
in `stopped` after 7,396, 4, and 3 transitions respectively. All three reports
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
stack output delta. `GetStringTypeW` then adds a checked 512-byte CTYPE1 output
delta. Its consumer and later existing helper calls reach four checked
`LCMapStringW` lower／upper query-conversion calls. Two checked
`InterlockedDecrement` calls then update the same aligned LONG around existing
increment and critical-section calls before both paths stop at
`GetModuleFileNameA`. The Rust-owned virtual path `C:\Vine\BGI.exe\0` is then
copied through the checked 16-byte output write. The following flags-zero
16-byte heap block and its path consumer are then applied through a dirty-only
host delta and ordinary CPU writeback. The controlled empty environment then
allocates one zeroed four-byte pointer-table terminator and stores its global
identity. The reached flags-zero one-byte block is then freed through the same
Rust-owned transactional heap path; Node and Chrome apply one dirty-only free
delta and complete ten existing `EncodePointer` calls. The following zeroed
128-byte allocation is applied as a dirty-only heap delta, passed through one
more `EncodePointer`, and consumed through ordinary stack／image／heap writeback.
Both `IsProcessorFeaturePresent(10)` calls then return the policy-owned BOOL 1
and store it in separate image globals. A later already-enabled 2,048-byte
zeroed allocation crosses a heap-page boundary, applies the second commit, and
returns allocation ID 8 before both paths converge at
`SetUnhandledExceptionFilter(0x100A816F)`. The checked process-owned filter
replacement returns NULL, preserves LastError, and continues through one
existing critical-section entry and two existing pointer decodes. The following
`HeapSize(0x68010001,0,0x20000C58)` returns the Rust-owned live allocation's
requested size 128 without guest writes or LastError mutation. Two existing
`EncodePointer` calls and one `LeaveCriticalSection` then complete before both
paths converge at `InitializeCriticalSection(0x100FD9CC)`.

## Reproduction

```powershell
npm run check:runtime:bgi-browser
```

The command regenerates the Node oracle, rebuilds the Rust/Wasm and Worker
artifacts, runs clean-profile Chrome success/cancel/watchdog probes, verifies the
source fingerprint before and after, and compares semantic results. The exact
report hash is artifact identity only.

## Non-claims

- no negative-length／broad `GetStringTypeW`, general `LCMapStringW`, explicit-module `GetModuleFileNameA`, SEH dispatch／filter invocation, broad `HeapSize`, `HeapReAlloc`, or broad heap variant dispatch is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
