# BGI second supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-second-win32-call` version `1`
- Report size／SHA-256: 18,472 bytes／`bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67`

This checkpoint continues [v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md). It declares exact thunk ID `103`, validates its zero-argument stdcall frame and indirect IAT call in the Rust session, returns one controlled process ID in EAX, and stops at the next exact owned thunk. It dispatches no thread API.

## 2. Controlled process identity

The checkpoint process identity is independent of the host Node／Windows process:

| Field | Value |
|---|---|
| Owner | `checkpoint-process-identity` |
| Process ID | `4096`／`0x00001000` |
| Source | fixed runtime sample, not `process.pid` or `GetCurrentProcessId` on the host |

This value is a deterministic process-bootstrap sample. It does not define the eventual process object, pseudo-handle, child process, wait, exit, debugger, or snapshot model.

## 3. Checked zero-argument stdcall

The initial reflection remains 388 pages at generation `24`. The first `GetSystemTimeAsFileTime` call produces the same generation `25` delta and report evidence as the preceding checkpoint. This mode adds `GetCurrentProcessId` as the third declared import endpoint, leaving 327 identity-preserving unsupported traps. Reflected bytes and thunk addresses do not change; the initial offer is 4,904,536 bytes because both supported Kernel32 thunks omit `unsupported-trap` metadata.

At exact thunk ID `103`, the adapter and Rust protocol validate:

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000660`／`0x100D1244` |
| Indirect call／return | `0x100A88D8`／`0x100A88DE` |
| ESP before return | `0x006FFFD4` |
| Frame | `[0x100A88DE]` |
| stdcall argument bytes | `0` |
| ESP after callee return | `0x006FFFD8` |
| EAX before／after | `0x006FFFEC`／`0x00001000` |

The call must be `FF 15 <0x100D1244>`, and the IAT must still contain `0x7F000660`. Unlike the preceding `VOID` API, this call explicitly replaces EAX with the controlled process ID.

## 4. Generation-preserving reconciliation

The guest `CALL` writes its return address to the live stack after generation `25` was acknowledged. Before Rust validates the frame, the runner supplies exactly one current stack page with generation `25`, address `0x006FF000`, and owner `4`. The page differs from Rust semantic memory in four bytes, and its SHA-256 is `5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f`.

`GuestMemory::reconcile_reflected_page` checks generation, page size／alignment, owner, mapping, and write permission. No host memory write follows this reconciliation, so generation and acknowledged generation both remain `25`, dirty pages remain zero, and no `MemoryDelta` is emitted. The process-ID request is 8,552 bytes and the Rust receipt is 845 bytes, excluding newlines.

## 5. Next runtime stop

After the stdcall return, BGI executes three more instructions in three execute stops with two budget stops. At cumulative instruction count `26`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `149`, `KERNEL32.dll!GetCurrentThreadId` |
| Disposition／address | `unsupported-trap`／`0x7F000940` |
| IAT | `0x100D12FC` |
| ESP／return | `0x006FFFD4`／`0x100A88E6` |
| Raw stack words | `[0x100A88E6,0,0,0,0,0,929513472,31264968]` |

The primary source fingerprint remains unchanged. The earlier stop-before-dispatch and first-call reports retain their exact 9,592-byte and 14,540-byte hashes.

## 6. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-second-win32-call
```

`npm run check:private-corpus` runs all three cumulative BGI contracts. Generated protocol messages and reports remain under `.work/` and are not committed.

## 7. Non-claims and next checkpoint

This is one fixed process identity, one page reconciliation, and one zero-argument stdcall return. It is not a general process／handle object model, automatic CPU dirty tracking, generated ABI gateway, PID allocator, thread identity model, TLS/client-ID TEB implementation, scheduler, or process lifecycle.

The next checkpoint should declare exact thunk ID `149`, choose an explicit controlled thread ID owned by the same process/thread bootstrap model, validate its zero-argument stdcall frame and call provenance, return the thread ID in EAX, resume BGI, and stop at the next exact owned thunk.
