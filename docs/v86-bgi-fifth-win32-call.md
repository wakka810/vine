# BGI fifth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-fifth-win32-call` version `1`
- Report size／SHA-256: 31,427 bytes／`eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1`

This checkpoint continues [v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md). It declares exact thunk ID `157`, validates its one-argument stdcall frame and indirect IAT call in the Rust session, writes one controlled 64-bit performance-counter sample, acknowledges the resulting memory delta, returns BOOL success in EAX, and stops at the next exact owned thunk. It dispatches no startup-info API.

## 2. Controlled performance-counter sample

The sample belongs to the same provisional monotonic-clock owner as the controlled tick count, but its origin and frequency remain unspecified:

| Field | Value |
|---|---|
| Owner | `checkpoint-monotonic-clock` |
| Sample ID | `qpc-ticks-12345678901` |
| Counter | `12,345,678,901`／`0x00000002DFDC1C35` |
| Low／high DWORD | `3,755,744,309`／`2` |
| Little-endian bytes | `[35,1C,DC,DF,02,00,00,00]` |
| Paired tick sample | `uptime-ms-3600000` |

Pairing records that both values describe one checkpoint instant. It does not assert a conversion ratio, counter origin, frequency, precision, progression, cross-core behavior, suspend behavior, or snapshot restoration policy.

## 3. Checked one-argument stdcall

This mode adds `QueryPerformanceCounter` as the sixth declared import endpoint, leaving 324 identity-preserving unsupported traps. Reflected bytes and thunk addresses do not change. The initial offer is 4,904,437 bytes because the five supported Kernel32 thunks omit `unsupported-trap` metadata.

At exact thunk ID `157`, the adapter and Rust protocol validate:

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F0009C0`／`0x100D131C` |
| Indirect call／return | `0x100A88F4`／`0x100A88FA` |
| ESP before return | `0x006FFFD0` |
| Frame | `[0x100A88FA,0x006FFFE4]` |
| Writable 64-bit output | `0x006FFFE4`／owner `4`／RW／NX |
| stdcall argument bytes | `4` |
| ESP after callee return | `0x006FFFD8` |
| EAX before／after | `0x006FFFE4`／`1` |

The call must be `FF 15 <0x100D131C>`, and the IAT must still contain `0x7F0009C0`. BOOL success is returned only after the output write is acknowledged.

## 4. Reconciliation and generation 26 delta

Before Rust validates the frame, the runner supplies exactly one current stack page with generation `25`, address `0x006FF000`, and owner `4`. Guest execution since the tick-count reconciliation changed five bytes; the page SHA-256 is `a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc`. The output’s eight bytes are initially zero.

Rust reconciles that page, validates `[return, output*]`, and writes only the controlled counter bytes. The write produces generation `26`, one 4 KiB dirty stack page, no mapping operations, and no cache effects. The delta content SHA-256 is `e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951`. After the adapter applies the full page and acknowledges generation `26`, Rust reports the exact eight output bytes and zero dirty pages.

The request is 8,681 bytes, delta is 9,583 bytes, and receipt is 366 bytes, excluding newlines.

## 5. Next runtime stop

After the stdcall return, BGI executes 44 more instructions in 44 execute stops with 43 budget stops. At cumulative instruction count `78`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `83`, `KERNEL32.dll!GetStartupInfoW` |
| Disposition／address | `unsupported-trap`／`0x7F000520` |
| IAT | `0x100D11F4` |
| ESP／return | `0x006FFF78`／`0x100A36E7` |
| Output pointer | `0x006FFF90` |
| Raw stack words | `[0x100A36E7,0x006FFF90,0xE93F6D87,0,0,0,0,0]` |

The primary source fingerprint remains unchanged. All earlier cumulative report hashes remain exact.

## 6. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-fifth-win32-call
```

`npm run check:private-corpus` runs all six cumulative BGI contracts. Generated protocol messages and reports remain under `.work/` and are not committed.

## 7. Non-claims and next checkpoint

This is one fixed counter sample, one checked 64-bit output write, and one BOOL return. It is not a general performance-counter clock, frequency contract, automatic CPU dirty tracking, generated ABI gateway, timer queue, scheduler, wait model, or clock snapshot implementation.

The next checkpoint should declare exact thunk ID `83`, inspect and validate the writable x86 `STARTUPINFOW` span at `0x006FFF90`, define only the controlled process-startup fields required at this call, return from the VOID one-argument stdcall, resume BGI, and stop at the next exact owned thunk. It must not silently claim general process creation, standard-handle inheritance, desktop, title, show-window, or reserved-field behavior.
