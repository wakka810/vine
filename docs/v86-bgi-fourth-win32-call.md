# BGI fourth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-fourth-win32-call` version `1`
- Report size／SHA-256: 26,558 bytes／`79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111`

This checkpoint continues [v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md). It declares exact thunk ID `13`, validates its zero-argument stdcall frame and indirect IAT call in the Rust session, returns one controlled 32-bit tick count in EAX, and stops at the next exact owned thunk. It dispatches no performance-counter API.

## 2. Controlled monotonic clock sample

The checkpoint tick count is independent of host uptime and is paired explicitly with the existing controlled wall-clock sample:

| Field | Value |
|---|---|
| Owner | `checkpoint-monotonic-clock` |
| Sample ID | `uptime-ms-3600000` |
| `GetTickCount` value | `3,600,000` ms／`0x0036EE80` |
| Paired wall-clock sample | `utc-2026-07-11T00:00:00.000Z` |

The pair represents one checkpoint instant with a fixed one-hour uptime sample. It does not derive a boot timestamp or define clock advancement, 32-bit wraparound, timer resolution, suspend behavior, wait scheduling, or snapshot restoration.

## 3. Checked zero-argument stdcall

The initial reflection remains 388 pages at generation `24`; the system-time delta advances semantic memory to generation `25`, and the following three EAX-only calls leave it there. This mode adds `GetTickCount` as the fifth declared import endpoint, leaving 325 identity-preserving unsupported traps. Reflected bytes and thunk addresses do not change. The initial offer is 4,904,470 bytes because the four supported Kernel32 thunks omit `unsupported-trap` metadata.

At exact thunk ID `13`, the adapter and Rust protocol validate:

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F0000C0`／`0x100D10DC` |
| Indirect call／return | `0x100A88E8`／`0x100A88EE` |
| ESP before return | `0x006FFFD4` |
| Frame | `[0x100A88EE]` |
| stdcall argument bytes | `0` |
| ESP after callee return | `0x006FFFD8` |
| EAX before／after | `0x00002000`／`0x0036EE80` |

The call must be `FF 15 <0x100D10DC>`, and the IAT must still contain `0x7F0000C0`. The return replaces the preceding controlled thread ID in EAX with the controlled tick count.

## 4. Generation-preserving reconciliation

The guest `CALL` writes its return address to the live stack after the thread-ID frame was reconciled at generation `25`. Before Rust validates the tick-count frame, the runner supplies exactly one current stack page with generation `25`, address `0x006FF000`, and owner `4`. The new return address differs from Rust semantic memory in one byte, and the page SHA-256 is `f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b`.

`GuestMemory::reconcile_reflected_page` checks generation, page size／alignment, owner, mapping, and write permission. No host memory write follows this reconciliation, so generation and acknowledged generation both remain `25`, dirty pages remain zero, and no `MemoryDelta` is emitted. The tick-count request is 8,640 bytes and the Rust receipt is 926 bytes, excluding newlines.

## 5. Next runtime stop

After the stdcall return, BGI executes five more instructions in five execute stops with four budget stops. At cumulative instruction count `34`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `157`, `KERNEL32.dll!QueryPerformanceCounter` |
| Disposition／address | `unsupported-trap`／`0x7F0009C0` |
| IAT | `0x100D131C` |
| ESP／return | `0x006FFFD0`／`0x100A88FA` |
| Output pointer | `0x006FFFE4` |
| Raw stack words | `[0x100A88FA,0x006FFFE4,0,0,0,0,0,929513472]` |

The primary source fingerprint remains unchanged. The earlier first-trap through third-call reports retain their exact 9,592-byte, 14,540-byte, 18,472-byte, and 22,383-byte hashes.

## 6. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-fourth-win32-call
```

`npm run check:private-corpus` runs all five cumulative BGI contracts. Generated protocol messages and reports remain under `.work/` and are not committed.

## 7. Non-claims and next checkpoint

This is one fixed tick-count sample, one page reconciliation, and one zero-argument stdcall return. It is not a general monotonic clock, automatic CPU dirty tracking, generated ABI gateway, timer queue, scheduler, wait model, wraparound policy, or clock snapshot implementation.

The next checkpoint should declare exact thunk ID `157`, choose an explicit controlled performance-counter sample in the same provisional monotonic clock domain, validate its one-argument stdcall frame and writable 64-bit output, return BOOL success, apply any required generation `26` delta, resume BGI, and stop at the next exact owned thunk. Counter frequency and progression remain separate decisions unless runtime evidence reaches them.
