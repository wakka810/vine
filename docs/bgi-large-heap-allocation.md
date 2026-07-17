# BGI 64 MiB private-heap allocation checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-large-heap-allocation`
- Previous capability checkpoint: `bgi-get-system-metrics`
- Next subsystem gateway: `kernel32-create-file-a-vfs-gateway`

This checkpoint supports the reached
`HeapAlloc(0x68010001, 0, 0x04000000)` without eagerly materializing 64 MiB in
Rust, Wasm, or v86. `RuntimeHeaps` owns one process-wide large-allocation slot;
`GuestMemory` remains the mapping and byte authority; the execution mirror asks
the generic page-fault service to materialize only the faulting 4 KiB page.

## Native lifetime and access evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/large-heap-allocation-v1-20260717-c.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event large-heap-allocation-complete `
  --observe heap-allocation
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 419,374 | 204 | 75 | `5800d1cbeb3bffffed54e7d3abee1cb9e1262dd468f3190e8354dcf70e9610c3` |
| Confirmation | 419,375 | 204 | 75 | `0d2250f012b5795318c5d86bbd47876555720a2d16d78e8e906384dda540914e` |

Both captures have contiguous sequence numbers, balanced call IDs, and zero
hook, vtable, call-stack, agent-message, or cleanup diagnostics. The immutable
executable remains 1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.
Raw traces and summaries stay under ignored `.work/` paths.

In both runs the main-module allocation requests 67,108,864 bytes with flags
zero, returns non-NULL, preserves LastError zero, and is later freed
successfully with flags zero. The guarded access monitor observes this stable
lifetime shape:

| Observation | Value |
|---|---:|
| Requested pages | 16,384 |
| Pages accessed before free | 30 |
| Pages untouched before free | 16,354 |
| Accessed page run | 0 through 29 |
| Accessed extent | 122,880 bytes |
| First-access operations | 30 writes; 0 reads／executes |
| First write source | BGI RVA `0x00061AEF`, allocation offset 0 |
| Last new page source | BGI RVA `0x000A3F37`, allocation offset 122,848 |

Native pointers differ between runs and are opaque artifact identities. The
evidence establishes sparse early use and allocation/free ordering; it does
not assert that Windows uses Vine's reserve layout, fill byte, or page-fault
protocol.

## Policy, ownership, and failure ordering

Kernel32 policy `kernel32-large-heap-allocation-v1` enables flags `0` and
`HEAP_ZERO_MEMORY` for the ordinary inclusive range `1..=16 MiB`, plus the
explicit exact large variant 64 MiB. The 16–64 MiB gap, zero bytes, sizes above
64 MiB, and other flags remain unsupported before runtime or memory mutation.
The policy contains no BGI callsite, occurrence, native pointer, or executable
hash.

The existing 32 ordinary 16 MiB slots remain at
`[0x20000000, 0x40000000)`. A permanent process address-allocation record adds
the one 64 MiB slot `[0x40000000, 0x44000000)`. Allocating the large variant:

1. validates process／thread lifecycle, heap identity, per-heap allocation ID,
   the unoccupied large slot, address record, and GuestMemory capacity;
2. reserves the complete range under a new anonymous heap owner without
   committing a physical page;
3. commits heap metadata only after the memory transaction is ready;
4. retains the permanent address record across `HeapFree` and process teardown
   so the same guest address can be reused with a new allocation ID.

The generic page-fault service first checks the selected thread's stack guard,
then the live large allocation. A non-present fault inside that allocation
plans one aligned page in `RuntimeHeaps`, preflights GuestMemory quota and the
existing reserved region, commits RW／NX mapping and initialization bytes in a
transaction, then commits the materialized-page index. Flags zero use Vine's
deterministic diagnostic byte `0xA5`; `HEAP_ZERO_MEMORY` uses zero. v86 and the
browser adapter receive the normal versioned mapping delta; they do not own a
second heap or page index.

A second live large allocation, duplicate or stale materialization, fault
outside the allocation, present-page protection fault, unsynchronized memory,
and physical-page quota exhaustion all reject before observable mutation.
Free and process teardown release the entire GuestMemory region and owner,
including every materialized page, while retiring the logical allocation and
preserving the permanent address record.

Representative tests:

```powershell
cargo test -p vine-core --lib runtime::heaps::tests --locked
cargo test -p vine-core --test runtime_large_heap_materialization --locked
cargo test -p vine-core --test runtime_heap_backing --locked
cargo test -p vine-core --test runtime_initial_thread_stack --locked
cargo test -p vine-core --test win32_compatibility_policy --locked
npm run check:win32-policy
```

The large-allocation integration test fixes reserve-only physical-page
accounting, `0xA5` and zero page initialization, distinct allocation IDs on
slot reuse, exact release／teardown accounting, permanent address identity, and
mutation-free negative paths.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi
npm run check:runtime:bgi-browser
```

Node completes 1,358 calls across 63 contracts. Sequence 1,350 reaches
`HeapAlloc` at instruction 50,105, returns allocation ID 12 at `0x40000000`,
and reports ten live allocations. The reserve delta advances generation
1,469 to 1,470 with one reserve descriptor, no dirty page, and no physical-page
increase.

The first guest write faults at instruction 50,123 with PFEC 6. Runtime memory
owner 11 and permanent address allocation ID 37 commit page `0x40000000`;
generation advances 1,470 to 1,472 and the 4,096-byte `0xA5` payload has
SHA-256
`f600eca824e84a43f0691b267bd620e462c50da165c5b80e17aecb7a924f1fa8`.
Execution continues to instruction 51,614 with 1,365 writebacks, 2,569 dirty
pages, 9,822 changed bytes, 87 host deltas, final generation 1,481, and memory
SHA-256
`446095a7512d76adfecf4871533c0c7733b6f84e12df323bd0b800bcde4c9225`.
The 5,549,091-byte Node report has SHA-256
`eddac339d56ba6932ebc311fbe6f422b6d161fa8c11080cd3d42ef6b7bd4d80e`.

Chrome reproduces the same call, allocation identity, one-page resolution,
memory digest, and next stop. It executes 7,063 slices／5,698 yields, applies
5,148 writeback batches／6,908 pages and 87 mapping deltas, performs 6,603 Wasm
crossings, and finishes at generation 3,765 with zero pending host calls or
callbacks after cancellation／watchdog cleanup. The 822,167-byte browser
report has SHA-256
`36b93248df38eddfeca6da56b948b5ae325f8114d7166f0b1c9225a70ab889e5`.
Runtime-core Wasm is 2,576,621 bytes with SHA-256
`546842f3d392ad3ac8aba86873bf51b16afda61b6b4bb2874f88dfa573dffe67`.

The changed memory topology and Worker page-fault path required the full
three-minute workload gate. The final clean-profile run completed 326 fresh
sessions with zero failure and passed all 17 unchanged thresholds. Session
P95 was 588.515 ms, slice P99 1.235 ms, heartbeat P99 6.325 ms, Worker heap
maximum 28,348,244 bytes, Worker backing-storage maximum 31,579,723 bytes,
Rust linear memory 10,027,008 bytes, and execution-engine linear memory
11,796,480 bytes. The 1,044,729-byte report has SHA-256
`9c41d51abef831652a48839ea60528831f9bd260f65e10a47330783d36d3cacc`.

## Non-claims

- Only one process-wide exact 64 MiB allocation may be live. General large
  blocks, multiple large blocks, arbitrary VirtualAlloc, and per-heap large
  arenas are not implemented.
- Native pointer reuse, native uninitialized bytes, page commitment timing,
  guard pages, decommit, reallocation, compact／walk, corruption handling, and
  concurrent heap access are not claimed.
- One startup lifetime and a three-minute repeated-startup gate are not a
  full-play memory profile or long-idle soak.
- The first page is materialized because the execution mirror reports a fault;
  TypeScript does not infer demand from API names or maintain heap authority.

## Next structured stop

```text
KERNEL32.dll!CreateFileA(
  filename=0x006FF45C,
  access=0x80000000,
  share=1,
  security=NULL,
  creation=3,
  flags=0x08000020,
  template=NULL)
endpoint ID:       137
endpoint address:  0x7F000880
IAT address:       0x100D12CC
return address:    0x1002BC00
instruction count: 51,614
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

Native evidence identifies the filename as `${TITLE_ROOT}\BGI.gdb`. This is a
filesystem／VFS subsystem gateway, not an isolated API leaf. The next
checkpoint must first define the smallest immutable-source vertical slice and
its handle, path, async host-service, read／seek／close, cancellation, and
failure-atomicity acceptance criteria. Writable save overlay work remains a
separate later slice.
