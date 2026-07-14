# BGI `HeapSize` checkpoint

## Status

- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-heap-size`
- Frozen historical prefix: `bgi-seventeenth-win32-call`
- Implemented contract: `kernel32.heap-size.v1`
- Compatibility policy: `kernel32-heap-size-v1`
- First unsupported capability: `kernel32-initialize-critical-section`

This checkpoint queries the requested size of one exact live allocation from
the Rust process-owned private-heap metadata. It does not inspect host heap
state or infer size from mapped capacity. Existing pointer encoding and
critical-section contracts then continue to `InitializeCriticalSection`.

## Native evidence

The final focused captures are uncommitted private artifacts:

| Capture | Bytes / lines | SHA-256 | Balanced API pairs |
|---|---:|---|---:|
| r1 | 2,248,521 / 7,038 | `705979af74515682f36f6b5149394a393c9c7099ec211524075efd6052f40952` | 3,482 |
| r2 | 2,247,585 / 7,035 | `c74e88eb1a51c8e2e1e05d99cdb49510077be0960f0de4eef9ea930fb68d65fa` | 3,480 |

Both traces have contiguous sequence metadata, balanced calls, and zero
diagnostics. Each 12-second capture observes five BGI calls from return RVA
`0x000A4F77`. Every call uses flags 0, points to the same exact live allocation
whose earlier requested size is 128, returns 128, and preserves LastError
`0 -> 0`. The caller at RVA `0x000A1D4D` stores EAX in EBX, compares it with the
current capacity, and computes growth with a minimum of 2,048 bytes.

The `x86-private-heap-size-query-v1` oracle creates a separate private heap and
allocates 128 bytes. Flags 0 and `HEAP_NO_SERIALIZE` both return 128 and preserve
LastError sentinels `0`, `87`, and `0x12345678`. An interior pointer and the
exact pointer after free both return `0xFFFFFFFF` and set LastError to
`ERROR_INVALID_PARAMETER` (`87`). Native heap and pointer numbers are evidence
identity only.

## Runtime contract

`RuntimeState::private_heap_allocation` reads the exact live allocation record
owned by the selected logical private heap. `HeapSize`:

- accepts policy-enabled flags 0 and `HEAP_NO_SERIALIZE`;
- returns `RuntimeHeapAllocation.byte_length`, not aligned reserved bytes or
  committed backing capacity;
- returns `0xFFFFFFFF` and LastError 87 for NULL, interior, foreign-heap, or
  freed pointers;
- rejects an unknown logical heap or unsupported flags before mutation;
- uses three-argument stdcall cleanup (`ESP += 16`) and writes no guest bytes;
- preserves LastError on success and changes no heap or memory state.

The public fixture deliberately queries a 129-byte allocation reserved as 136
bytes, covers both enabled flags, all invalid pointer relations, unknown heap,
and post-free behavior. The BGI pointer, caller, result, and call position do
not select the handler branch.

## Node continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-heap-size
```

Call 207 queries `HeapSize(0x68010001,0,0x20000C58)`, returns 128, and preserves
LastError 0. Calls 208／209 encode `0x100A8851` and `0x20000C5C`; call 210 leaves
the existing critical section at `0x100FF6C0`.

| Evidence | Value |
|---|---:|
| Supported calls / contracts | 210 / 43 |
| CPU writebacks / dirty pages | 211 / 324 |
| Changed bytes | 4,757 |
| Host deltas | 58 |
| Final generation | 299 |
| Memory SHA-256 | `90c245267e933a01c1e54fa0305bec1d54a16180a3e7465b6a3b6b3c8d3dd0b8` |
| Report bytes | 929,421 |
| Report SHA-256 | `058ca051ad9b4118ce8854333eafd35aab117dc70da860b28188a2e1afff8fac` |

## Chrome equivalence and workload gate

`npm run check:runtime:bgi-browser` reproduces all 210 ordered completions, the
memory digest, and the `InitializeCriticalSection` stop in Chrome. The Worker
uses 2,465 slices, 2,254 budget yields, 1,828 writeback batches over 2,392
pages, 58 mapping deltas, and 2,100 Rust／Wasm crossings. It ends at generation
1,640; success, cancellation, and watchdog all stop with no pending work.

The canonical 225,486-byte browser report has SHA-256
`df0fbb0bfc3c36ef472fe10fa5dcb1e728c5146af99e188b53ca3e09e5786736`.
The Rust runtime core is 1,880,515 bytes with SHA-256
`3b700a98b3f9e09edfb5e1aabc4dcc6f0276bd1fa59cd8d173058d92ed9792ad`.

The 180-second workload completed 438 sessions with zero failure and passed all
17 thresholds. Session P95 was 536.710 ms, slice P99 was 2.730 ms, and
heartbeat P99／maximum were 9.935／37.930 ms. Main and Worker heap slopes were
-31,199.69 and 1,019,353.38 bytes/min; backing-storage slope was 512,752.49
bytes/min. The 1,065,785-byte report has SHA-256
`a0b6ca623df6f8857861f14ff5c1d1b2506886a44f98a8c6e12bfed13b8c0d54`.

## Next structured stop

```text
KERNEL32.dll!InitializeCriticalSection(0x100FD9CC)
endpoint ID:       147
endpoint address:  0x7F000920
IAT:               0x100D12F4
return:            0x1009B2C3
instruction:       19185
fault:             endpoint-contract-missing
cluster:           kernel32-critical-section-synchronization
```

This historical stop is closed by
[bgi-initialize-critical-section.md](bgi-initialize-critical-section.md), which
measures the one-argument initial bytes／ABI and advances to the initial-thread
stack-growth fault without generalizing every synchronization primitive.

## Non-claims

This checkpoint does not query host heaps, process heaps, interior usable size,
unknown heap handles, arbitrary flags, `HeapReAlloc`, heap validation or walking,
or cross-thread heap serialization. It also does not implement general critical
section initialization, SEH dispatch, title rendering, audio, input, VFS, COM,
or full play.
