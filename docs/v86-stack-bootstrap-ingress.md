# Production stack bootstrap ingress

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: Rust-owned controlled initial stack and bootstrap metadata
- Report schema: `vine.v86-stack-bootstrap-ingress` version `1`
- Primary fixture: `abi-smoke.exe`, 2,560 bytes, SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7`

This checkpoint moves the controlled initial stack used by [v86-entry-return-ingress.md](v86-entry-return-ingress.md) from adapter-local allocation into Rust `GuestMemory`. Rust reserves and initializes the stack, includes it in semantic reflection, and emits bootstrap metadata. The adapter maps the reflected page without creating or writing a separate harness stack, then preserves the same fixed service／callback／entry completion result.

## 2. Rust stack ownership

The version 1 controlled stack is:

| Field | Value |
|---|---:|
| Owner | `4`, anonymous |
| Region kind | stack |
| Reserved／committed range | `[0x006FF000, 0x00700000)` |
| Reserved／committed pages | `1`／`1` |
| Protection | RW, non-executable |
| Stack top | `0x00700000` |
| Initial ESP | `0x006FFFFC` |
| Initial DWORD | process-entry sentinel ID `2` address `0x7F010010` |
| Page SHA-256 | `3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079` |
| Owner state | sealed |
| Final invalidation generation | `8` |

`install_controlled_initial_stack` uses `MemoryRegionKind::Stack`, commits one RW page, writes the entry return address through checked owner initialization, hashes the full initialized page, and seals the owner. The stack owner is separate from image owner `1`, import owner `2`, and control owner `3`.

The process-entry sentinel is installed and sealed first at generation `6`. Stack commit advances generation to `7`, and stack seal advances it to `8`. The resulting memory stats are four owners／regions, four sealed owners, 38 reserved pages, eight committed／physical／dirty pages, and one pending generation `8` invalidation before acknowledgement.

## 3. Reflection and adapter mapping

The reflection contains eight production pages with content SHA-256 `d7471bcc744e2f769861e1a40cc5520f5c0d452f2ea8539233a8573315b6f1cd`. The stack is the first page in ascending guest-address order and carries owner `4`, RW protection, seal state, dirty state, and the full 4 KiB bytes.

The adapter still allocates 12 physical pages／48 KiB from its 2 MiB pool:

```text
page directory
page tables for directory indices 1, 64, and 508
eight reflected production pages
```

There is no `harness-stack` allocation. Directory index `1` is allocated first, and the reflected owner `4` page receives the physical page that follows it. Physical addresses remain adapter-local and are not part of Rust bootstrap metadata.

Before execution, the adapter verifies the reflected mapping is RW and non-executable, reads `0x7F010010` from initial ESP, and does not call its virtual-memory write helper for stack setup. After state restore／TLB／generated-code invalidation, Rust acknowledges generation `8` and normal-reads the top 16 stack bytes:

```text
00 00 00 00 00 00 00 00 00 00 00 00 10 00 01 7F
```

## 4. Execution result

The execution contract is unchanged:

| Field | Result |
|---|---:|
| Execute stops | `9` |
| Budget stops | `5` |
| Fixture result | `0x10003000 = 43` |
| Entry result | EAX `0` |
| Final ESP | `0x00700000` |
| Completion trap | process-entry sentinel ID `2`／`0x7F010010` |
| Stop reason／error | structured `#UD` `0x106`／`-1` |
| Callback frame ID `1` | retired |
| Callee-saved registers | unchanged |
| Wasm host imports | `microtick` once |

The deterministic report is 9,427 bytes with SHA-256 `a1a66c7d98835289da2604217b72edfdbdd576eaad29dd442faea4097507b7de`. The entry, cdecl, callback-return, outer-return, and adapter-stack entry-return reports remain byte-identical at 4,831, 5,501, 7,028, 8,124, and 8,674 bytes.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:stack-bootstrap
```

## 5. Non-claims and next checkpoint

This is partial evidence for `REQ-MEM-01`, `REQ-MEM-02`, `REQ-MEM-03`, `REQ-CPU-01`, `REQ-CPU-03`, `REQ-CPU-04`, `REQ-ABI-01`, `REQ-ABI-02`, `REQ-DIAG-03`, `P0-ARCH-03`, `P1-MEM-01`, `P1-MEM-02`, `P1-CPU-01`, `P1-ABI-01`, `P1-ABI-02`, `P1-DIAG-01`, `AC-P1-01`, `AC-P1-02`, `AC-P1-03`, and `AC-P1-10`. None is complete.

The fixed address, one-page reserve／commit, owner ID, and sealed initialization are checkpoint choices. This does not honor arbitrary PE stack reserve／commit values, implement guard pages, growth, overflow handling, multi-thread stacks, stack teardown, or Windows loader frames. Guest stack and result mutations still occur only in adapter-reflected bytes and are not synchronized back into Rust `GuestMemory`. Flat CPL3 segment state, process／thread lifecycle, PEB／TEB, process parameters, DLL lifecycle, exception／SEH delivery, and BGI execution remain absent.

The next checkpoint adds a separate controlled bootstrap fixture that reads the minimum i386 thread/process environment contract. It should first establish a Rust-owned TEB self pointer and PEB pointer, set the guest FS base through the versioned CPU state, and verify one exact field access before process parameters or broader loader state are added.
