# BGI third supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-third-win32-call` version `1`
- Report size／SHA-256: 22,383 bytes／`d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa`

This checkpoint continues [v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md). It declares exact thunk ID `149`, validates its zero-argument stdcall frame and indirect IAT call in the Rust session, returns one controlled thread ID in EAX, and stops at the next exact owned thunk. It dispatches no tick-count API.

## 2. Controlled thread identity

The checkpoint thread identity is independent of the host Node／Windows thread:

| Field | Value |
|---|---|
| Owner | `checkpoint-thread-identity` |
| Thread ID | `8192`／`0x00002000` |
| Owning checkpoint process ID | `4096`／`0x00001000` |
| Source | fixed runtime sample, not a host thread ID |

This value is a deterministic process/thread-bootstrap sample. It does not populate the TEB `CLIENT_ID` fields or define the eventual guest-thread object, scheduler, TLS, wait, suspend, exit, or snapshot model.

## 3. Checked zero-argument stdcall

The initial reflection remains 388 pages at generation `24`; the system-time delta advances semantic memory to generation `25`, and both identity calls leave it there. This mode adds `GetCurrentThreadId` as the fourth declared import endpoint, leaving 326 identity-preserving unsupported traps. Reflected bytes and thunk addresses do not change. The initial offer is 4,904,503 bytes because the three supported Kernel32 thunks omit `unsupported-trap` metadata.

At exact thunk ID `149`, the adapter and Rust protocol validate:

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000940`／`0x100D12FC` |
| Indirect call／return | `0x100A88E0`／`0x100A88E6` |
| ESP before return | `0x006FFFD4` |
| Frame | `[0x100A88E6]` |
| stdcall argument bytes | `0` |
| ESP after callee return | `0x006FFFD8` |
| EAX before／after | `0x00001000`／`0x00002000` |

The call must be `FF 15 <0x100D12FC>`, and the IAT must still contain `0x7F000940`. The return replaces the preceding controlled process ID in EAX with the controlled thread ID.

## 4. Generation-preserving reconciliation

The guest `CALL` writes its return address to the live stack after the process-ID frame was reconciled at generation `25`. Before Rust validates the thread-ID frame, the runner supplies exactly one current stack page with generation `25`, address `0x006FF000`, and owner `4`. The new return address differs from Rust semantic memory in one byte, and the page SHA-256 is `b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617`.

`GuestMemory::reconcile_reflected_page` checks generation, page size／alignment, owner, mapping, and write permission. No host memory write follows this reconciliation, so generation and acknowledged generation both remain `25`, dirty pages remain zero, and no `MemoryDelta` is emitted. The thread-ID request is 8,548 bytes and the Rust receipt is 840 bytes, excluding newlines.

## 5. Next runtime stop

After the stdcall return, BGI executes three more instructions in three execute stops with two budget stops. At cumulative instruction count `29`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `13`, `KERNEL32.dll!GetTickCount` |
| Disposition／address | `unsupported-trap`／`0x7F0000C0` |
| IAT | `0x100D10DC` |
| ESP／return | `0x006FFFD4`／`0x100A88EE` |
| Raw stack words | `[0x100A88EE,0,0,0,0,0,929513472,31264968]` |

The primary source fingerprint remains unchanged. The earlier first-trap, first-call, and second-call reports retain their exact 9,592-byte, 14,540-byte, and 18,472-byte hashes.

## 6. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-third-win32-call
```

`npm run check:private-corpus` runs all four cumulative BGI contracts. Generated protocol messages and reports remain under `.work/` and are not committed.

## 7. Non-claims and next checkpoint

This is one fixed thread identity, one page reconciliation, and one zero-argument stdcall return. It is not a general process／thread object model, automatic CPU dirty tracking, generated ABI gateway, thread-ID allocator, TEB client-ID implementation, scheduler, TLS, or thread lifecycle.

The next checkpoint should declare exact thunk ID `13`, choose an explicit controlled tick-count sample and clock ownership consistent with the existing fixed wall-clock sample, validate its zero-argument stdcall frame and call provenance, return the tick count in EAX, resume BGI, and stop at the next exact owned thunk.
