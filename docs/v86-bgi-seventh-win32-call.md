# BGI seventh supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-seventh-win32-call` version `1`
- Report size／SHA-256: 43,980 bytes／`09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f`

This checkpoint continues [v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md). It declares exact thunk ID `84`, validates the four-argument stdcall frame and indirect IAT call in the Rust session, accepts only the native-observed termination-on-corruption variant, records one controlled process heap-policy selection, preserves LastError by policy, returns BOOL success, and stops at the next exact owned thunk. It does not dispatch `HeapCreate`.

## 2. Native observation

The focused Windows trace was run twice with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\bgi-heap-set-information.jsonl" `
  --seconds 3 `
  --kill-after `
  --observe heap-set-information
```

Both captures observed the target call returning to `BGI.exe + 0xA36FC`:

| Field | Native observation |
|---|---:|
| `HeapHandle` | `NULL` |
| `HeapInformationClass` | `1` (`HeapEnableTerminationOnCorruption`) |
| `HeapInformation` | `NULL` |
| `HeapInformationLength` | `0` |
| BOOL result | `1` |
| LastError before／after | `87`／`87` |

The unchanged `87` is stale thread state, not a success code. The checkpoint therefore returns BOOL success without clearing or replacing LastError. The trace target retained its exact source hash and emitted no hook or call-stack diagnostics.

## 3. Controlled heap-policy variant

The selected variant belongs to owner `checkpoint-process-heap-policy` with policy ID `heap-enable-termination-on-corruption`. Rust accepts only `[NULL,1,NULL,0]`, records `terminationOnCorruptionEnabled=true`, `preserveLastError=true`, and reports BOOL `1`.

This is one process-wide policy selection. It does not create a heap, map a guest heap handle, inspect heap memory, implement corruption detection or termination, support policy disablement, or accept another `HEAP_INFORMATION_CLASS`.

## 4. Checked four-argument stdcall

This mode adds `HeapSetInformation` as the eighth declared import endpoint, leaving 322 identity-preserving unsupported traps. Reflected bytes and thunk addresses remain unchanged. The initial offer is 4,904,371 bytes.

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000530`／`0x100D11F8` |
| Indirect call／return | `0x100A36F6`／`0x100A36FC` |
| ESP before return | `0x006FFF6C` |
| Frame | `[0x100A36FC,NULL,1,NULL,0]` |
| stdcall argument bytes | `16` |
| ESP after callee return | `0x006FFF80` |
| EAX before／after | `0x006FFF90`／`1` |

The call must be `FF 15 <0x100D11F8>`, and the IAT must still contain `0x7F000530`. BOOL success comes from the exact controlled policy, not a generic success stub.

## 5. Generation-preserving reconciliation

Before Rust validates the frame, the runner supplies the current generation `27` stack page at `0x006FF000`, owner `4`. Guest execution after the startup-info acknowledgement changed 12 bytes; the page SHA-256 is `d121a898c549de8cf2eea5d76912375a993cac9a7dfb41d101ef2eb2b68fa3da`.

Rust reconciles the page and validates `[return,NULL,1,NULL,0]`. The service writes no guest memory, so generation and acknowledged generation remain `27`, dirty pages remain zero, and no delta or cache effect is produced. The request is 8,724 bytes and receipt is 1,042 bytes, excluding newlines.

## 6. Next runtime stop

After the stdcall return, BGI executes 21 more instructions in 21 execute stops with 20 budget stops. At cumulative instruction count `108`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `102`, `KERNEL32.dll!HeapCreate` |
| Disposition／address | `unsupported-trap`／`0x7F000650` |
| IAT | `0x100D1240` |
| ESP／return | `0x006FFF6C`／`0x100A8886` |
| Three arguments | `[0,0x1000,0]` |
| Raw stack words | `[0x100A8886,0,0x1000,0,0x100A374A,0xE93F6D87,0,0]` |

The primary source fingerprint and all earlier cumulative report hashes remain unchanged.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-seventh-win32-call
```

`npm run check:private-corpus` runs all eight cumulative BGI contracts. Generated protocol messages, native traces, and reports remain under `.work/` and are not committed.

## 8. Non-claims and next checkpoint

This is one native-observed policy call, one generation-preserving stack reconciliation, and one BOOL return. It is not a heap allocator, process-heap model, corruption detector, exception／termination policy, per-thread LastError implementation, or general `HeapSetInformation` implementation.

The next checkpoint should measure and implement exact thunk ID `102` for the observed `HeapCreate(0, 0x1000, 0)` call, assign one typed logical heap handle and owned heap state, then stop at the next exact owned thunk. It must not fabricate a host pointer or claim general heap allocation before the first reached heap operation is measured.
