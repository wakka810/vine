# Memory delta and transaction contract

## Status

- Status: Verified foundation checkpoint
- Date: 2026-07-11
- Rust boundary: `vine-core::GuestMemory`
- Incremental transfer: `MemoryDelta`
- v86 evidence: `vine.v86-memory-delta-roundtrip` version `1`

## Transfer model

`GuestMemory` is the semantic source. The first CPU connection uses
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
tracking.

## Transaction model

`GuestMemory::begin_transaction()` takes a private snapshot of owners, regions,
page mappings, physical frames, bytes, protections, seal state, generation,
dirty journal, offered generations, and pending cache effects. Calling
`commit_transaction()` keeps the result. Returning an error or dropping the
transaction restores the snapshot.

This snapshot implementation favors correctness over copy cost at the current
checkpoint. It is used for controlled process environment construction,
runtime return controls, initial stack installation, PE image materialization,
and import commit. Input-derived command-line sizes, fixed ranges, quotas,
section ranges, thunk capacity, and original IAT values are checked before
mutation. Failpoint tests cover every process-environment reserve／commit／zero／
pointer／seal boundary and every import arena／thunk／IAT／seal boundary; each
failure leaves the same observable state and permits retry on the same memory.

## v86 evidence

Reproduction:

```powershell
npm run prepare:engine
npm run check:runtime:memory-delta
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

## Limits

- Delta page payloads are still JSON numeric arrays; binary IPC is deferred.
- Dirty ownership remains Rust-to-v86 in general. Only one explicitly supplied,
  generation／owner-checked full page can currently be reconciled from v86;
  automatic write detection, multi-page writeback, conflicts, and scheduling
  are not implemented.
- The transaction snapshot copies committed frames and is not the final large
  heap or framebuffer transaction strategy.
- The 15-byte execute gate is conservative and can reject a short instruction
  near a page boundary even when its actual bytes do not reach the next page.
  Decode-aware adapter integration remains follow-up work in
  [issue #1](https://github.com/wakka810/vine/issues/1).
- CPU, memory, and persistence snapshot restoration is not implemented.
