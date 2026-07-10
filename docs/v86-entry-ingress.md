# v86 production-memory entry ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: production memory reflection and controlled entry to owned thunk
- Report schema: `vine.v86-entry-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint connects the sealed controlled image and production import-thunk owner to the provisional v86 paging adapter. It keeps the same Rust `GuestMemory` alive while Node installs semantic pages into adapter-owned page tables, clears v86 translation and generated-code state, and acknowledges pending generation `4`. It then starts the relocated entry at `0x10001000` on a bounded harness stack and stops at exact production thunk ID `1`／`0x7F000000` before any host service behavior.

## 2. Reflection boundary

`GuestMemory::pending_reflection` emits committed pages in ascending guest-address order while normal guest access remains synchronization-blocked. Each page contains only:

- guest virtual address
- logical owner ID and seal state
- read／write／execute metadata
- dirty state
- copied 4 KiB content

The reflection does not expose Rust physical-frame indices, host offsets, v86 physical addresses, page-table bytes, or private v86 state offsets. Executable pages are rejected if their owner is unsealed. The content digest covers each page address, owner, protection／seal／dirty metadata, and bytes in order.

The probe holds the live memory object across this bounded protocol:

```text
Rust probe                         Node v86 adapter
    |                                     |
    | vine.v86-memory-reflection v1       |
    |------------------------------------>|
    |                                     | install PDE/PTE/backing
    |                                     | restore CPU state ABI
    |                                     | full_clear_tlb
    |                                     | jit_clear_cache_js
    | reflection-ack v1, generation 4     |
    |<------------------------------------|
    | acknowledge_invalidation(4)         |
    | normal read/fetch verification      |
    | reflection-receipt v1               |
    |------------------------------------>|
```

An acknowledgement is rejected if its schema／generation is wrong or any required data-TLB, instruction-fetch, or generated-code class is incomplete. The version 1 session rejects physical quotas above 16 pages, the Node runner rejects protocol lines above 512 KiB, and the probe reads at most 4 KiB of acknowledgement input. After acceptance it verifies the bound IAT and fetches both entry and thunk bytes through normal Rust guest access; `pendingInvalidation` must then be absent.

## 3. Representative contract

Reproduction requires the fixed fixture and source-built adapter:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:entry
```

The Rust reflection contains six production-owned pages:

| Guest page | Owner | Protection | Role |
|---:|---:|---|---|
| `0x10000000` | 1 | R | relocated PE headers |
| `0x10001000` | 1 | RX | `.text`／entry |
| `0x10002000` | 1 | R | `.rdata`／bound IAT |
| `0x10003000` | 1 | RW | `.data` |
| `0x10004000` | 1 | R | `.reloc` |
| `0x7F000000` | 2 | RX | committed thunk arena page |

Both owners are sealed and all six pages are dirty after loader initialization. The reflection content SHA-256 is `3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e`. The thunk page digest remains `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`.

The adapter uses the pinned 1,882,836-byte v86 Wasm artifact with SHA-256 `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26`. A 2 MiB contiguous adapter-local physical pool backs:

- one page directory
- three page tables for directory indices `64`, `508`, and `1`
- six reflected production pages
- one harness-only stack page `[0x006FF000, 0x00700000)`

This is 11 allocated physical pages／44 KiB. Rust owns guest virtual semantics and bytes; the Node adapter owns provisional physical placement and PDE／PTE encoding. Only the adapter-local memory-size setup still reads pinned private v86 offset `812`; CPU registers, segments, and control state are configured through the 640-byte version 1 state ABI.

## 4. Entry result

After the reflection receipt, the runner writes harness-only entry sentinel `0xDEADC0DE` at initial ESP `0x006FFFFC`, restores flat CPL3／paging state, and checks logical execute permission before each budget slice. With instruction budget `3`:

| Field | Result |
|---|---|
| Execute stops | 2 |
| Budget stops | 1 |
| Final stop | structured `#UD`, reason `0x106`, error `-1` |
| EIP／previous EIP | `0x7F000000`／`0x7F000000` |
| Exact owner | thunk ID `1`, `vine_fixture.dll!fixture_add` |
| ESP | `0x006FFFF0` |
| Return address | `0x1000100A` |
| Arguments | `[19, 23]` |
| Bound IAT | `0x10002034` -> `0x7F000000` |

The deterministic report is 4,831 bytes with SHA-256 `9b32ed50a4fa53d60a2743267230d43acf754bb6bc490851dd754955e70cbcb8`.

## 5. Non-claims and next checkpoint

This is partial evidence for `REQ-MEM-01` through `REQ-MEM-03`, `REQ-CPU-01`, `REQ-CPU-03`, `REQ-ABI-01`, `REQ-ABI-02`, `REQ-DIAG-03`, `P0-ARCH-03`, `P1-MEM-01`, `P1-MEM-02`, `P1-CPU-01`, `P1-ABI-01`, `P1-DIAG-01`, `AC-P1-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

The one-page stack, flat segment state, sentinel, 2 MiB pool, physical allocation order, and non-PAE PTE encoding are checkpoint scaffolding, not the production process bootstrap or final memory layout. The execute gate checks the EIP at each bounded slice; v86 still lacks native PE execute permission and instruction-fetch PFEC semantics. The reflection is not a snapshot／restore format. It does not implement PEB／TEB、guard pages、stack growth、DLL lifecycle、SEH、thread scheduling、host service dispatch、guest callback、Chrome Worker integration, or BGI execution.

The fixed cdecl continuation is complete in [v86-cdecl-ingress.md](v86-cdecl-ingress.md), the one-level callback return in [v86-callback-return-ingress.md](v86-callback-return-ingress.md), and the frame retirement／outer return／entry continuation in [v86-outer-return-ingress.md](v86-outer-return-ingress.md). Together they classify thunk IDs `1`／`2`, return `42`, invoke target `0x10001030`, return result `43` through the outer service, and execute the guest entry path to its measured return boundary.
