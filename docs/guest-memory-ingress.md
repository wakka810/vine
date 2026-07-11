# Guest memory ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: `P0-ARCH-03` production guest-memory ingress
- Schema: `vine.guest-memory-ingress` version `1`
- Primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

This checkpoint connects the version 1 PE32 load plan to a Rust-owned sparse guest-address boundary before guest execution. It reserves a 4 GiB virtual range model, commits only planned image pages to a bounded physical-page allocator, preserves final page protection metadata, and emits a structured stop before source bytes are materialized. It does not make the image executable or complete ADR-003, the PE loader, or `AC-P1`.

## 2. Implementation boundary

`vine-core` owns the following production-oriented types and operations.

- checked `GuestAddress` and `GuestRange` values over `[0, 2^32)`
- 4 KiB owner-scoped region and page tables without a 4 GiB host allocation
- image, anonymous, and runtime owners, plus image, stack, heap, anonymous, and runtime region kinds
- fixed reserve, commit, protect, decommit, exact-base release, and explicit owner destruction
- one zeroed 4 KiB physical frame per committed page, with quota enforcement and frame reuse
- owner-scoped loader initialization writes followed by an explicit execute seal
- checked read, write, and instruction-fetch access with full-range preflight
- generation-tagged data-TLB, instruction-fetch, and generated-code invalidation requirements

The library remains `#![forbid(unsafe_code)]`. Production source under `vine-core` and `vine-pe-probe` has no `unwrap`, `expect`, panic, `todo!`, or `unimplemented!` path.

`prepare_image_layout` creates one image owner, reserves the complete `SizeOfImage` range, and commits the header and section pages described by the owned W7 plan. Plan ranges must be page-aligned, non-overlapping, and contained by the image reservation. Pages receive the plan's final R/W/X metadata immediately, but initialization writes may populate pages owned by the unsealed image regardless of final write permission. This is the loader-only write path; normal guest writes still enforce page protection.

Default fail-closed limits are:

| Limit | Default |
|---|---:|
| Physical pages | 65,536 pages / 256 MiB |
| Regions | 16,384 |
| Owners | 4,096 |
| One checked access | 16 MiB |

These are current runtime defaults, not a browser memory budget or final compatibility policy.

## 3. Fault and invalidation contract

Every guest read, write, or fetch is split at page boundaries and all pages are preflighted before data is read or mutated. A cross-page write therefore leaves the earlier page unchanged when a later page faults. Fault metadata keeps the first failing guest address, access kind, reason, and x86 page-fault error-code equivalent.

| Access result | PFEC |
|---|---:|
| Unmapped read | `4` |
| Unmapped write | `6` |
| Unmapped fetch | `20` |
| Read denied | `5` |
| Write denied | `7` |
| Execute denied | `21` |
| Executable page owned by an unsealed owner | `21` |

The execute gate is logical Rust metadata. It does not claim that v86 has delivered these faults to a guest exception handler.

Commit, protect, decommit, committed-region release, and image-layout preparation require data-TLB, fetch, and generated-code invalidation. Sealing an owner requires fetch and generated-code invalidation. A normal guest write to an executable page also requires fetch and generated-code invalidation. Pending operations merge their flags into the latest generation, and guest read, write, and fetch stay blocked until that exact generation is acknowledged.

Acknowledgement currently proves the ownership protocol only. No adapter calls v86 `full_clear_tlb`, flushes a decoder or generated-code cache, or binds the acknowledgement to an actual CPU resume. Initialization writes are allowed while the owner is unsealed and do not emit a separate invalidation; sealing supplies the fetch and generated-code barrier before execution can become eligible.

## 4. Representative contract

The single automated contract uses the deterministic high-address fixture.

```powershell
npm run check:toolchain
npm run check:runtime:memory-core
```

| Field | Result |
|---|---|
| Source | `high-memory.exe`, 3,072 bytes, SHA-256 `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7` |
| Image | base `0xF1000000`, size `0x6000`, 6 reserved and committed pages |
| Protection | 6 readable, 1 writable, 1 executable page |
| Physical quota | 7 pages in the contract; 6 image pages plus 1 anonymous probe page |
| Initial delta | generation `2`, reserve／commit journaled, all three cache effects pending |
| Final state | 1 sealed image owner, 6 image pages, generation `13`, acknowledged, zero dirty pages |

The sequence checks pending-delta blocking, owner sealing, executable and non-executable fetches, read-only writes, RW data deltas, a second-page unmapped write with no partial mutation, a protection transition, exact quota failure, decommit and frame reuse, region release, and owner teardown. It also verifies the expected PFEC values `21`, `7`, and `6` at the relevant stops. Incremental transfer and acknowledgement details are in [memory-delta-and-transactions.md](memory-delta-and-transactions.md).

The standalone version 1 layout report is reproduced with an exact six-page image quota:

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7 `
  --prepare-memory-pages 6 `
  --output .work\guest-memory\high-memory-ingress.json `
  .work\toolchain-spike\high-memory.exe
```

Two reports were byte-identical at 1,808 bytes with SHA-256 `8d05d8cecd0bdb7fbecbc00850ca4c25dffbc64a47cb6686c36f6137983399b1`.

## 5. Primary BGI result

The same boundary accepts the fingerprinted BGI load plan with an exact 380-page physical quota.

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 380 `
  --output .work\guest-memory\bgi-layout.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Source | 1,193,472 bytes, expected SHA-256 matched |
| Image | base `0x00400000`, entry `0x004A383E`, size `0x17C000` |
| Layout | 380 reserved, committed, and physical pages |
| Protection | 380 readable, 100 writable, 208 executable pages |
| Ownership | owner `1`, unsealed, 1 image region |
| Initial delta | generation `2`, all three cache effects pending |
| Report | 1,831 bytes, SHA-256 `c46e40fdeb0301de1d852b4532bc0be0c4033bc5fd5f79479ab695663f7a119e` |

A 379-page quota fails with exit code `1`, reports `image-physical-page-quota-exceeded`, and creates no output file. The default W7 probe path remains byte-for-byte unchanged at 1,976,261 bytes and SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be`. The source BGI file retains its original size and SHA-256.

The memory-ingress stop is:

```text
image-materialization / image-bytes-relocations-import-binding-unimplemented
```

## 6. Non-claims and next checkpoint

This checkpoint is partial evidence for `REQ-MEM-01` through `REQ-MEM-03`, `P0-ARCH-03`, `P1-MEM-01` through `P1-MEM-03`, and `AC-P1-01`. None is complete. Stack and heap semantics, guard pages, snapshots and state hashes, mapped files, copy-on-write and sharing, memory-pressure behavior, and Chrome Worker limits remain unimplemented or unverified.

At this checkpoint the image pages contained allocator-initialized zero frames only. It did not copy PE headers or section source bytes, verify PE zero-fill, apply `HIGHLOW` relocations, bind an IAT slot, seal the BGI owner, reflect pages into v86 page tables, acknowledge a real v86/code-cache invalidation, or execute a guest instruction. The report therefore remains layout evidence, not evidence that BGI was mapped or runnable.

The source-copy／zero-fill／`HIGHLOW` continuation is complete in [pe-materialization.md](pe-materialization.md), runtime-owned thunk allocation／IAT binding in [pe-import-binding.md](pe-import-binding.md), controlled page reflection／first-thunk execution in [v86-entry-ingress.md](v86-entry-ingress.md), fixed callback／entry completion in [v86-entry-return-ingress.md](v86-entry-return-ingress.md), and Rust-owned stack reflection in [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md). This memory-ingress report itself remains zero-page layout evidence with the original stop and non-claims.
