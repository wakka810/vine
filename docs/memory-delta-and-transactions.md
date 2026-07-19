# Memory delta and transaction contract

## Status

- Status: Verified foundation checkpoint
- Date: 2026-07-12
- Rust boundary: `vine-core::GuestMemory`
- Incremental transfer: `MemoryDelta`
- CPU writeback: `CpuWritebackBatch`
- v86 evidence: `vine.v86-memory-delta-roundtrip`／`vine.v86-memory-writeback` version `1`

## Transfer model

`GuestMemory` is the byte and metadata authority. The first CPU connection uses
`pending_reflection()` to transfer every committed page. After that generation
is acknowledged, `pending_reflection()` is rejected and callers use
`pending_delta()`.

A delta contains:

- the last acknowledged and offered generations;
- ordered reserve／commit／protect／decommit／release operations;
- only pages whose bytes changed since the acknowledged generation;
- independent TLB, instruction-fetch, and generated-code effects;
- transferred page-byte count and deterministic content digest.

An ordinary write to an RW, non-executable page advances the generation and
emits one dirty page with all cache effects false. A write touching executable
memory additionally requests instruction-fetch and generated-code invalidation.
Mapping changes carry their semantic operation and request only the cache
effects required by that transition. Multiple writes to one page collapse to
one latest page payload without losing generation ordering; a cross-page write
emits every changed page.

`acknowledge_delta(generation)` accepts only a generation previously returned
by a full reflection or delta. It removes journal entries and page dirty state
at or below that generation while retaining later mutations. Future, stale,
duplicate, and never-offered acknowledgements are structured errors. A delta
may be obtained, followed by another mutation, and then acknowledged safely:
the newer page generation remains dirty for the next delta.

`reconcile_reflected_page(generation, owner, address, bytes)` is the first
bounded CPU-to-Rust direction. It accepts exactly one aligned 4 KiB writable
page only when the initial reflection is acknowledged, no delta is pending,
and the generation and owner match. The bytes already active in the CPU mirror
replace the Rust frame without advancing generation or making it dirty. A
following host write can then emit a normal full-page delta without erasing
guest stack changes. This is a synchronous one-page handoff, not general dirty
tracking; it remains for older checkpoint replay.

The production direction is `reconcile_cpu_writeback[_binary]`. At every
bounded CPU stop, the execution mirror scans all writable PTEs, collects pages
whose x86 Dirty bit is set, clears those bits, flushes the TLB, and submits one
generation-tagged batch. Rust preflights the whole batch for synchronized base
generation, unique aligned addresses, owner, writable mapping, exact page size,
and configured limits. Any failure leaves all bytes and generations unchanged.
A valid multi-page batch copies only pages whose bytes actually differ in one
transaction, advances generation once, and acknowledges that generation
without emitting a Rust-to-v86 delta because the mirror already has those
bytes. RWX pages are detected and counted in the receipt.

Initial reflection (`VINERF01`), host delta (`VINEDL01`), and CPU writeback
(`VINEWB01`) use fixed-width binary control descriptors and a separate
contiguous page payload. `VINEFR01` bounds the two buffers on the session
transport. JSON page-byte arrays are no longer part of the production memory
path; JSON remains an evidence format.

## Transaction model

`GuestMemory::begin_transaction()` now creates an empty changed-state journal;
it does not clone `GuestMemory`, the page table, or committed frame bytes.
Owner, region, and page metadata store their prior value only on the first
transactional mutation. An existing frame stores one 4 KiB before-image on its
first byte write. Allocate and release operations keep ordered inverse allocator
records; pages first allocated by the active transaction need no byte
before-image because rollback discards their frame slot.

The journal also records the starting owner ID, generation, pending-change
length, and generation offers introduced by the transaction. Calling
`commit_transaction()` discards the journal and keeps the result. Returning an
error or dropping the transaction replays frame inverses, restores changed
metadata and scalars, removes new offers, and truncates pending changes.
Acknowledging a delta before commit is rejected because the external CPU mirror
cannot be rolled back by a Rust-only transaction. Transactions are deliberately
non-nestable; composition uses the transaction-aware internal mapping path
instead of starting a second journal. Immediate API completion follows the same
rule: when CPU-page reconciliation already owns the journal, validated guest
writes and LastError completion join that journal. Without an outer journal the
dispatcher creates and commits its own. A second explicit `begin_transaction()`
remains a runtime defect rather than silently creating a partial savepoint.

The same journal covers controlled process environment construction, runtime
return controls, initial stack installation, PE image materialization, import
commit, and v86 page reconciliation. Failpoint tests cover every
process-environment reserve／commit／zero／pointer／seal boundary and every import
arena／thunk／IAT／seal boundary; each failure leaves the same observable state and
permits retry on the same memory. A destructive rollback contract additionally
reconciles a CPU page, releases its two-page region, destroys its owner, offers a
delta, and restores all bytes, mappings, allocator state, generation, and offers
when dropped. The API dispatcher regression also commits and rolls back a required
eight-byte guest output while an outer memory transaction is active.

`MemoryTransactionStats` exposes deterministic rollback payload accounting.
The representative scaling contract starts with 1,024 committed pages
(4,194,304 bytes): transaction start journals zero frame bytes, the first write
to one page peaks at 4,096 bytes, a repeated write to that page adds nothing,
and a write to a second page peaks at 8,192 bytes. This measures exact frame
before-image payload, not process RSS or allocator bookkeeping.

## v86 evidence

Reproduction:

```powershell
npm run prepare:engine
npm run check:runtime:memory-delta
npm run check:runtime:memory-writeback
```

The public session reflects code, data, and a two-page execute-gate fixture.
After initial acknowledgement, Rust writes little-endian bytes for
`0x8877665544332211` to `0x20000000`. Generation `11` transfers one 4 KiB dirty
page, no mapping operations, and no cache invalidation. The live v86 mapping is
updated in place; guest instructions load EAX `0x44332211` and EDX `0x88776655`
before the exact `UD2` stop. Rust acknowledgement leaves zero dirty pages.

The same session places a five-byte instruction at `0x30000FFE`: its first two
bytes are on an executable page and the remainder is on a non-executable page.
The provisional adapter gate checks the conservative 15-byte x86 fetch window
and returns `instruction-fetch / cross-page-execute-permission-unimplemented`
without executing the instruction.

The private BGI continuation uses the reconciliation boundary because the
first `GetSystemTimeAsFileTime` output points into a v86-mutated stack page.
Generation `24`／owner `4` reconciliation preserves 11 changed bytes; the
eight-byte host output then emits generation `25`, one 4 KiB dirty page, no
mapping operations, and no cache effects. The full transferred page is checked
against the current v86 page plus only the `FILETIME` bytes before stdcall
return. See [v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md).

The following `GetCurrentProcessId` call uses the same boundary at acknowledged
generation `25` to reconcile its four-byte return frame. Because the API only
returns EAX and performs no host memory write, generation remains `25`, no page
is dirty, and no delta is emitted. See
[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md).

The following `GetCurrentThreadId` call repeats the generation `25` boundary
for its zero-argument frame. Its return address changes one stack-page byte;
reconciliation imports that byte, while the EAX-only return again leaves
generation `25`, dirty pages zero, and no delta. See
[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md).

The following `GetTickCount` call reconciles one changed return-address byte at
generation `25`. Its controlled result is returned only in EAX, so semantic
generation and dirty-page state remain unchanged and no delta is emitted. See
[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md).

`QueryPerformanceCounter` then reconciles five guest-changed stack bytes at
generation `25`, writes one controlled 64-bit output, and emits generation `26`
as one full-page data-only delta. The adapter applies and acknowledges it before
BOOL success is returned. See
[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md).

The sixth BGI call reconciles 31 additional stack-byte changes at generation
`26`, overwrites one full 68-byte x86 `STARTUPINFOW`, and transfers generation
`27` as another one-page data-only delta before the VOID return. See
[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md).

The automatic writeback session reflects nine pages through binary buffers and
executes two consecutive slices. The first discovers seven stack／heap／surface／
RWX pages and reconciles 49 changed bytes at generation `18 -> 19`; an
intervening one-page host delta advances `19 -> 20`; the second discovers four
different writable pages and reconciles 13 changed bytes at `20 -> 21`.
Untouched writable pages remain clean, every Dirty clear is followed by a TLB
flush, and Rust ends each receipt with no dirty page or pending invalidation.
See [memory-writeback.md](memory-writeback.md).

## Limits

- PTE Dirty discovery is O(writable mappings) per bounded slice and currently
  clears the full TLB. Title-workload cost is deferred to the v86 benchmark.
- The Browser Worker applies each mapping／dirty batch synchronously after a
  full preflight, so cancellation cannot leave a half-applied mapping. An
  ambiguous acknowledgement rebuilds the paging mirror from a current Rust
  reflection while preserving CPU context. An idempotent queued transport for
  future cross-worker or persistent delivery is not implemented.
- Transaction journal metadata grows with changed owners, regions, pages, and
  allocator operations. Exact frame payload is measured, but host allocator
  overhead and process RSS are not yet benchmarked.
- The 15-byte execute gate is conservative and can reject a short instruction
  near a page boundary even when its actual bytes do not reach the next page.
  Decode-aware adapter integration remains follow-up work in
  [issue #1](https://github.com/wakka810/vine/issues/1).
- CPU, memory, and persistence snapshot restoration is not implemented.
