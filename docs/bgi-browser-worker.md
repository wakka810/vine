# BGI Chrome Runtime Worker checkpoint

## Status

- Status: semantic equivalence passed
- Date: 2026-07-15
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
| Supported calls | 285 | 285 |
| Guest instructions at next stop | 25,261 | 25,261 |
| Initial memory generation | 27 | 27 |
| Final memory generation | 2,208 | 396 |
| Host delta batches | 77 | 77 |
| Initial-stack growths | 4 | 4 |
| Final memory content SHA-256 | `08afd63f56cf8c18788d80f50a6e8037f005c35d53bd50c55ec3b88975eb698b` | same |
| Next capability cluster | `kernel32-api-contract` | same |
| Endpoint | `kernel32.dll!SetThreadAffinityMask` | same |
| Structured fault | `endpoint-contract-missing` | same |

Every completion is compared by ordered module/symbol occurrence, decoded
arguments, EAX policy, implementation ID, return and stack pointers, LastError,
and guest writes. The final full-memory digest and structured fault must also
match. Node does not retain a production scheduler, Windows state, or memory
authority for the browser session.

Generation is intentionally not equal at the end. Chrome reconciles dirty CPU
pages after each bounded 8-instruction slice and API trap, producing 2,482
writeback batches; the Node oracle reconciles at 290 writeback boundaries.
Both paths begin at generation 27 and converge on identical bytes. Generation is
therefore synchronization identity, not guest-visible equivalence.

## Worker lifecycle

The successful run used 3,247 CPU slices, 2,957 budget yields, 9,742 validated
Worker state transitions, 2,852 Rust/Wasm bridge operations, and a maximum
observed slice of 2.780 ms. The main-thread heartbeat recorded 106 samples with a 28.010 ms
maximum gap while the Worker ran. A single-run `usedJSHeapSize` observation
changed from 628,738 to 899,918 bytes; this remains run identity. The separate
versioned BGI workload gate repeats
complete sessions and measures main／Worker heap, backing storage, linear memory,
slice distribution, heartbeat, and Wasm crossings.

Separate cancellation and watchdog probes stop after 8 and 4 instructions.
Both retire pending host calls and callbacks, detach the callback-free system
module graph, and leave the Rust process in `exited` state. Cancellation can be
delivered only between complete CPU/writeback transactions.

The explicit state machine ends success, cancellation, and watchdog
in `stopped` after 9,742, 4, and 3 transitions respectively. All three reports
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
paths initialize `0x100FD9CC` through the policy-backed one-argument VOID
contract. Another 56 existing calls complete before both paths apply four
Rust-owned initial-thread stack-growth commits. The second
`GetModuleFileNameA` occurrence then completes. Policy-owned CP_ACP resolution
converts the NUL-terminated process path to 16 UTF-16 units. Two existing
`WideCharToMultiByte` component consumers and eight thread-local helper calls
then complete before both paths call `SetCurrentDirectoryA(0x10104048)`. Rust
copies the normalized process-image-directory identity, commits directory
generation `0 -> 1`, returns BOOL 1, and preserves LastError without a guest
write. Browser and Node compare both initial and post-call directory metadata.
The caller's path copy is reconciled as ordinary CPU writeback before both paths
call `CreateMutexA(0,1,0x100E1CAC)`. Rust copies the 57-byte name into the
private synchronization subsystem, transactionally creates mutex ID `1`／typed
object ID `19`／logical handle `0x58010001`, grants initial ownership to thread
`8192`, returns LastError `0`, and retains no guest pointer. `GetLastError`, one
flags-zero 16-byte `HeapAlloc`, and its `HeapFree` then complete. `GetCurrentThread`
then resolves the selected typed thread object without allocating a handle,
returns pseudo-handle `0xFFFFFFFE`, and preserves LastError. Both paths pass the
pseudo value and affinity mask `1` to the next import and stop on
`SetThreadAffinityMask` at instruction 25,261.

## Reproduction

```powershell
npm run check:runtime:bgi-browser
```

The command regenerates the Node oracle, rebuilds the Rust/Wasm and Worker
artifacts, runs clean-profile Chrome success/cancel/watchdog probes, verifies the
source fingerprint before and after, and compares semantic results. The exact
report hash is artifact identity only.

## Non-claims

- no `SetThreadAffinityMask`, additional-thread stack lifecycle／stack overflow, broad `GetStringTypeW`, general `LCMapStringW`, explicit-module `GetModuleFileNameA`, general relative-path／VFS state, cross-thread mutex blocking／abandonment／guest wait-release-close coverage, SEH dispatch／filter invocation, broad `HeapSize`, `HeapReAlloc`, or broad heap variant dispatch is implemented
- no title rendering, audio, input, VFS, COM, or full-play path is claimed
- the three-minute workload gate is not a full-play or long-idle soak
- Node remains available for differential evidence but is not a production path
