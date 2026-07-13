# Automatic guest-memory writeback

## Status

- Status: Verified foundation checkpoint
- Date: 2026-07-12
- Byte authority: `vine-core::GuestMemory`
- Execution mirror: source-built v86 paging
- Evidence schema: `vine.v86-memory-writeback` version `1`
- Report: `.work/v86-memory-writeback/report.json`

## Contract

Rust owns committed guest bytes, page ownership, protection, and generation. v86 receives a binary full reflection, executes against its bounded physical mirror, and exposes x86 PTE Dirty bits at each bounded slice boundary. The host scans every writable mapping without an API-specific page list, copies only dirty 4 KiB pages into one binary writeback batch, clears those Dirty bits, and flushes the TLB before any next slice.

`GuestMemory::reconcile_cpu_writeback_binary` decodes a bounded `VINEWB01` control buffer plus a contiguous page payload. It rejects stale base generations, pending host deltas, duplicate or unaligned pages, wrong owners, non-writable mappings, malformed lengths, and batches over the configured page limit before changing authority. A valid batch replaces only pages whose bytes differ, in one changed-state transaction, advances generation once for the whole batch, and marks that generation acknowledged because the CPU mirror already contains the same bytes. A batch containing only same-value dirty pages leaves generation unchanged and does not journal their frame bytes.

Initial `VINERF01`, host `VINEDL01`, and CPU `VINEWB01` transports separate fixed-width control descriptors from contiguous page bytes. The session uses bounded `VINEFR01` frames. JSON is produced only after execution as an evidence artifact; no execution-path page is serialized as a JSON numeric array.

Executable writable pages participate in the same discovery. The receipt counts them explicitly. v86 owns cache handling for the guest write that already occurred; later Rust-originated writes to executable pages continue to request instruction-fetch and generated-code invalidation through `MemoryDelta`.

## Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:memory-writeback
```

The fixture reflects nine pages, eight of them writable: two stack pages, two heap pages, two surface pages, one RWX target, one scratch page, and one RX program page. The first slice performs cross-page stack and surface writes, a `REP STOSD` heap write, and an RWX byte write. PTE discovery returns exactly seven pages; the untouched scratch page remains clean. Rust reconciles 49 changed bytes and advances generation `18 -> 19`.

Rust then writes four bytes to scratch and sends one binary host delta, advancing `19 -> 20`. The second slice writes one different stack, heap, surface, and RWX page. Discovery returns exactly those four pages, reconciles 13 changed bytes, and advances `20 -> 21`. Both writebacks end with Rust dirty pages `0`, no pending invalidation, cleared PTE Dirty bits, and a TLB flush after discovery.

| Evidence | Result |
|---|---:|
| Reflected pages／writable pages | 9／8 |
| First slice dirty pages／changed bytes | 7／49 |
| Host delta pages | 1 |
| Second slice dirty pages／changed bytes | 4／13 |
| Final generation | 21 |
| Report bytes | 4,578 |
| Report SHA-256 | `5eab2884b70f8b1a48d98dff8021b29a1690296c03dcae0183b5c9fdc10368b8` |

Rust semantic tests additionally fix binary reflection and delta descriptors, all five mapping-operation encodings, multi-owner atomic reconciliation, stale generation, wrong-owner, and truncated-payload rejection.

## Failure Policy And Limits

After Dirty bits are cleared, the host retains the batch until Rust returns its receipt. A rejection or cancellation keeps the guest paused. A definitely uncommitted batch may be retried byte-for-byte; ambiguous commit or lost receipt destroys the execution mirror and rebuilds it from Rust authority. Continuing one side independently is forbidden.

The current v86 adapter discovery pass is O(writable mappings) and performs `full_clear_tlb()` after each scan. The generic Runtime Worker receives only owner／address／bytes changed-page records. The fixture proves correctness and bounded transfer. Controlled Chrome Worker cancellation, the repeated-session 30-minute fixture slope, and the three-minute／2,210-session BGI workload threshold now use this path. Production physical backing, incremental dirty indexing, snapshot, guard pages, general stack／heap allocation, and full-play throughput remain later gates.
