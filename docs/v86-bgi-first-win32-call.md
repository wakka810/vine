# BGI first supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-first-win32-call` version `1`
- Report size／SHA-256: 14,540 bytes／`7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16`

This checkpoint continues [v86-bgi-first-trap.md](v86-bgi-first-trap.md). It declares exact thunk ID `104` as the one supported endpoint, validates its one-argument stdcall frame and indirect IAT call, reconciles the current v86 stack page into Rust, writes one controlled `FILETIME` through `MemoryDelta`, returns to BGI, and stops at the next exact owned thunk. It dispatches no second API.

## 2. Controlled clock sample

The checkpoint host clock owns one fixed sample. It does not read the current wall clock.

| Field | Value |
|---|---|
| Owner | `checkpoint-host-clock` |
| Sample ID／UTC | `utc-2026-07-11T00:00:00.000Z`／`2026-07-11T00:00:00.000Z` |
| Windows 100 ns ticks since 1601-01-01 | `134282016000000000` |
| `dwLowDateTime`／`dwHighDateTime` | `929513472`／`31264968` |
| Little-endian bytes | `[00,40,67,37,C8,10,DD,01]` |

This is a deterministic test sample, not a scheduler clock policy. A browser runtime must later define wall-clock, monotonic-clock, replay, pause, and snapshot ownership separately.

## 3. Checked stdcall frame

The initial 388-page reflection remains generation `24` with content SHA-256 `f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`. Adding `GetSystemTimeAsFileTime` to the runtime export catalog changes only thunk metadata: 2 imports are declared and 328 retain `unsupported-trap` disposition. The thunk bytes and reflected page content remain unchanged.

At exact thunk ID `104`, the adapter and Rust protocol validate:

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000670`／`0x100D1248` |
| Indirect call／return | `0x100A88CC`／`0x100A88D2` |
| ESP before return | `0x006FFFD0` |
| Frame | `[0x100A88D2, 0x006FFFEC]` |
| `FILETIME *` output | `0x006FFFEC`, 8 bytes, owner `4`, RW／NX |
| stdcall argument bytes | `4` |
| ESP after callee return | `0x006FFFD8` |

The six-byte call must be `FF 15 <0x100D1248>`, and the IAT must still contain `0x7F000670`. Rust re-reads the reconciled frame before writing. `GetSystemTimeAsFileTime` has a `VOID` result, so this checkpoint preserves EAX `0x006FFFEC`; it does not claim that Windows specifies an EAX value for this API.

## 4. Stack reconciliation and delta

The output points into the stack page that v86 has modified while running the first 19 instructions. Applying a full Rust page from generation `24` without reconciliation would overwrite the live call frame. The adapter therefore sends exactly one current 4 KiB page with acknowledged generation `24`, page address `0x006FF000`, and owner `4`. `GuestMemory::reconcile_reflected_page` checks generation, alignment, exact page size, owner, mapping, and write permission before replacing the semantic page without advancing generation or marking it dirty.

The current stack page differs from the initial Rust page in 11 bytes and has SHA-256 `46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785`. The following checked 8-byte host write advances generation to `25` and emits:

| Delta field | Result |
|---|---|
| From／to generation | `24`／`25` |
| Mapping operations | `0` |
| Dirty pages／bytes | `1`／`4096` |
| Dirty address／owner | `0x006FF000`／`4` |
| Cache effects | TLB `false`, instruction fetch `false`, generated code `false` |
| Delta content SHA-256 | `dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840` |

The runner verifies the entire dirty page equals the reconciled v86 page with only the eight output bytes replaced. After applying it to live v86, Rust acknowledges generation `25`, reports zero dirty pages, and reads the same output bytes back.

## 5. Next runtime stop

After the stdcall return, BGI executes four more instructions in four execute stops with three budget stops. At cumulative instruction count `23`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `103`, `KERNEL32.dll!GetCurrentProcessId` |
| Disposition／address | `unsupported-trap`／`0x7F000660` |
| IAT | `0x100D1244` |
| ESP／return | `0x006FFFD4`／`0x100A88DE` |
| Raw stack words | `[0x100A88DE,0,0,0,0,0,929513472,31264968]` |

The source fingerprint is checked before and after the run and is unchanged. The protocol line sizes are 4,904,569 bytes for the initial offer, 1,494 bytes for its receipt, 8,670 bytes for the write request, 9,584 bytes for the delta offer, and 361 bytes for the delta receipt.

## 6. Reproduction

The source adapter and primary title must already be present:

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-first-win32-call
```

`npm run check:runtime:bgi -- --checkpoint bgi-first-trap` remains the prior stop-before-dispatch contract and retains its 9,592-byte report hash. Generated data remains under `.work/` and is not committed.

## 7. Non-claims and next checkpoint

This is one fixed API call, one host-owned clock sample, one page reconciliation, and one stdcall return. It is not a general clock／timer subsystem, general v86 dirty-page tracker, generated ABI gateway, process identity model, LastError implementation, scheduler, DLL lifecycle, heap, snapshot, Worker integration, or broad Kernel32 support.

The next checkpoint should declare exact thunk ID `103`, choose an explicit controlled process ID, validate its zero-argument stdcall frame and call provenance, return the process ID in EAX without changing memory, resume BGI, and stop at the next exact owned thunk.
