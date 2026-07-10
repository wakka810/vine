# PE32 image materialization

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: `W7` production PE32 image materialization
- Schema: `vine.pe32-materialization` version `1`
- Primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

This checkpoint converts the owned W7 PE32 plan into guest image bytes before import binding or execution. It verifies the source identity, maps the image at a caller-selected 64 KiB-aligned base, copies headers and sections, explicitly zero-fills every mapped tail, applies 4-byte `HIGHLOW` relocations, verifies the original IAT values, and hashes the final mapped ranges. It stops at the first unbound IAT slot with the image owner still unsealed.

## 2. Materialization contract

`materialize_pe32_image` accepts only the immutable `ModuleSnapshot`, its matching `vine.pe32-loader-ingress` version 1 report, an empty `GuestMemory`, and a selected load base. It does not parse PE structures again. Before allocating guest pages it checks:

- snapshot integrity and exact source identity match
- loader schema version, preferred image end, preferred entry address, and selected 4 GiB range
- 64 KiB load-base alignment
- header／section source spans, page-sized mapped spans, and zero-fill coverage
- relocation counts, 4-byte `highlow` operations, and mapped targets
- nonpreferred images have usable relocations
- every planned IAT source DWORD is file-backed and still equals `originalIatValue`

The write sequence is:

1. Reserve one image region and commit only planned header／section pages with final R／W／X metadata.
2. Copy source ranges in chunks no larger than `MemoryLimits.maxAccessBytes` through owner-scoped initialization writes.
3. Explicitly zero every header／section mapped tail and read it back to reject any nonzero byte.
4. For a nonpreferred base, read each planned `HIGHLOW` DWORD and write `original.wrapping_add(loadBase - preferredBase)`.
5. Read every IAT slot and verify it still contains the planned original RVA／ordinal thunk value.
6. SHA-256 hash all mapped ranges in ascending plan order after relocation and before IAT binding.

Mapping creates generation `1` data-TLB／fetch／generated-code invalidation. Initialization writes do not acknowledge it. The owner remains unsealed because IAT writes are still pending; normal guest read, write, and fetch remain blocked.

## 3. Representative fixture

The single automated contract relocates the fixed `abi-smoke.exe` from `0x00400000` to `0x10000000`.

```powershell
npm run check:toolchain
npm run check:runtime:materialization
```

| Field | Result |
|---|---|
| Source | 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| Image | 5 pages／20,480 mapped bytes, exact 5-page quota |
| Copy | 4 ranges／2,560 bytes |
| Zero-fill | 5 ranges／17,920 bytes, all read back as zero |
| Relocation | delta `0x0FC00000`, 5 `HIGHLOW` operations |
| Relocated entry | `0x10001000` |
| IAT | 2 original slots verified; first `vine_fixture.dll!fixture_add` at `0x10002034` |
| Mapped content | SHA-256 `1cfa1ba4ef560433b38e2598ba291ae6cbd0f8139408274c2192c30e4a185110` |
| Stop | `import-binding`／`iat-binding-unimplemented` |

The test independently applies the relocation delta to a source-derived expected section buffer and checks all five fixed target values: `0x10002034`, `0x10001030`, `0x10002038`, and two references to `0x10003000`. It also verifies header／section copy, all zero tails, both unchanged IAT values, source immutability, all five dirty pages, pending generation `1`, deterministic result JSON, and an unsealed owner.

The standalone report is reproduced with:

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7 `
  --prepare-memory-pages 5 `
  --materialize-image-base 0x10000000 `
  --output .work\materialization\abi.json `
  .work\toolchain-spike\abi-smoke.exe
```

Two reports were byte-identical at 2,403 bytes with SHA-256 `f66b1dcf4d6a008c672b807c1e6acaf35348eaf1d37e3ad22d8c62011774407c`.

## 4. Primary BGI result

The same path relocates the fingerprinted BGI image from `0x00400000` to `0x10000000` under an exact 380-page quota.

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 380 `
  --materialize-image-base 0x10000000 `
  --output .work\materialization\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Guest image | `[0x10000000, 0x1017C000)`, entry `0x100A383E` |
| Page metadata | 380 readable, 100 writable, 208 executable, 380 dirty |
| Copy | 6 ranges／1,193,472 bytes |
| Zero-fill | 5 ranges／363,008 bytes, all read back as zero |
| Relocation | delta `0x0FC00000`, 18,199 `HIGHLOW` operations |
| IAT | 330 original slots verified |
| First unbound import | `d3d9.dll!Direct3DCreate9`, IAT `0x100D14F4` |
| Mapped content | 6 ranges／1,556,480 bytes, SHA-256 `d9dc33d121ea2da6ba5e459a9da13b70bf8bfc1c668593a80c6b3689d841199b` |
| Stop | `import-binding`／`iat-binding-unimplemented` |

Two local reports were byte-identical at 2,448 bytes with SHA-256 `e6ce8d5599922c7ea11470475313907e6732c1c9e4a2ca0ecd4457cd0d502019`. The original BGI file remained 1,193,472 bytes with its primary SHA-256. Reports, mapped bytes, game data, and absolute local paths remain under `.work/` or outside Git.

## 5. Non-claims and next checkpoint

This checkpoint is partial evidence for `REQ-PE-02`, `REQ-PE-03`, `REQ-PE-06`, `REQ-MEM-01` through `REQ-MEM-03`, `P1-MEM-01`／`P1-MEM-02`, `P2-PE-02`／`P2-PE-03`／`P2-PE-10`, and `AC-P2-01`／`AC-P2-05`. None is complete.

No DLL or synthetic system module is resolved. No runtime-owned thunk address is allocated or written to an IAT slot. The image owner is not sealed, invalidation is not connected to v86, discardable sections remain mapped, and PEB／TEB／process parameters／stack／TLS／DLL lifecycle／entry execution are not attempted. The mapped-content digest proves this deterministic loader boundary, not Windows-equivalent execution or BGI startup.

The next primary checkpoint defines production runtime-owned thunk addresses and import ownership. It binds both controlled fixture imports, then attempts the first BGI import selected by the existing plan and native reachability evidence. Unsupported symbols stop with module／symbol／IAT context; they do not receive silent-success stubs.
