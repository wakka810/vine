# BGI sixth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-sixth-win32-call` version `1`
- Report size／SHA-256: 39,031 bytes／`208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c`

This checkpoint continues [v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md). It declares exact thunk ID `83`, validates its one-argument stdcall frame and indirect IAT call in the Rust session, overwrites one x86 `STARTUPINFOW` with a controlled process-startup profile, acknowledges the resulting memory delta, preserves EAX for the VOID return, and stops at the next exact owned thunk. It does not dispatch `HeapSetInformation`.

## 2. Native x86 observation

The focused Windows trace was run twice with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\bgi-startup-info.jsonl" `
  --seconds 3 `
  --kill-after `
  --observe startup-info
```

The target call is `BGI.exe + 0xA36E1`, returning to RVA `0xA36E7`. The 68-byte input was uninitialized stack data. Native Windows overwrote all fields and returned this semantic profile:

| Field | Native observation |
|---|---|
| `cb` | `68` |
| `lpReserved`／text | nonzero／empty |
| `lpDesktop`／text | nonzero／empty |
| `lpTitle`／text | nonzero／absolute BGI image path |
| geometry／character counts／fill | all zero |
| `dwFlags` | `0x100` (`STARTF_USESTDHANDLES`) |
| show-window／reserved2 | all zero |
| standard handles | three nonzero inherited host handles |

The pointer and handle values changed with process launch and are not compatibility constants. The trace target retained its exact source hash, emitted no hook or call-stack diagnostics, and was terminated after collection. The opt-in observation records both the initial and final 68-byte spans; ordinary startup traces do not add this hook.

## 3. Controlled process-startup profile

Vine selects profile `startupinfow-v68-zero-optional-fields`, owned by `checkpoint-process-startup`. Rust constructs exactly 68 bytes:

| Field | Controlled value |
|---|---:|
| `cb` | `68` |
| all pointer fields | `0` |
| geometry／character counts／fill | `0` |
| flags／show-window | `0` |
| reserved2 fields | `0` |
| standard handles | `0` |

The bytes are `[44,00,00,00]` followed by 64 zero bytes, SHA-256 `9a577f8dadf13d9197f2f83feebcf47567472ab68625099b4e7bcde0fef18653`. Empty native reserved／desktop strings are represented by null pointers. The launcher-owned absolute title path and inherited handles are intentionally absent because the current controlled process bootstrap does not own image-path, desktop, console, or standard-handle lifecycle. `dwFlags == 0` follows the same immediate show-window branch as native `0x100`, because neither contains `STARTF_USESHOWWINDOW`.

This is a checkpoint profile, not a claim that every process should receive zero optional fields.

## 4. Checked one-argument VOID stdcall

This mode adds `GetStartupInfoW` as the seventh declared import endpoint, leaving 323 identity-preserving unsupported traps. Reflected bytes and thunk addresses remain unchanged. The initial offer is 4,904,404 bytes.

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000520`／`0x100D11F4` |
| Indirect call／return | `0x100A36E1`／`0x100A36E7` |
| ESP before return | `0x006FFF78` |
| Frame | `[0x100A36E7,0x006FFF90]` |
| Writable output | `0x006FFF90`／68 bytes／owner `4`／RW／NX |
| stdcall argument bytes | `4` |
| ESP after callee return | `0x006FFF80` |
| EAX before／after | `0x006FFF90`／`0x006FFF90` |

The call must be `FF 15 <0x100D11F4>`, and the IAT must still contain `0x7F000520`. Because the Win32 return type is VOID, the checkpoint preserves EAX instead of fabricating a success value.

## 5. Reconciliation and generation 27 delta

Before Rust validates the frame, the runner supplies the current generation `26` stack page at `0x006FF000`, owner `4`. Guest execution after the counter acknowledgement changed 31 bytes; the page SHA-256 is `75e2211e3e9a79f92313319d9897bb1e868d74bb15f0dc513cc9cb393c9bb843`. The 68-byte output span is zero except for its final old stack DWORD `0x100A88FA`; its SHA-256 is `5fa35fdf26ef33a7361c44c3dabdb80cee7c2a468bf39e1ff0d2188e53b48aed`.

Rust reconciles the page, validates `[return, output*]`, and writes the full structure. The write produces generation `27`, one 4 KiB dirty stack page, no mapping operations, and no cache effects. The delta content SHA-256 is `a2c82c133e41916b0b9af10314376ece9a281a8503db4b811031e5ce52919de8`. After application and acknowledgement, Rust reads back the exact 68 controlled bytes and reports zero dirty pages.

The request is 8,663 bytes, delta is 9,936 bytes, and receipt is 474 bytes, excluding newlines.

## 6. Next runtime stop

After the VOID stdcall return, BGI executes nine more instructions in nine execute stops with eight budget stops. At cumulative instruction count `87`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `84`, `KERNEL32.dll!HeapSetInformation` |
| Disposition／address | `unsupported-trap`／`0x7F000530` |
| IAT | `0x100D11F8` |
| ESP／return | `0x006FFF6C`／`0x100A36FC` |
| Four arguments | `[NULL,1,NULL,0]` |
| Raw stack words | `[0x100A36FC,0,1,0,0,0xE93F6D87,0,0]` |

The primary source fingerprint and all earlier cumulative report hashes remain unchanged.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-sixth-win32-call
```

`npm run check:private-corpus` runs all seven cumulative BGI contracts. Generated protocol messages, native traces, and reports remain under `.work/` and are not committed.

## 8. Non-claims and next checkpoint

This is one native-observed layout, one controlled structure write, one stack-page reconciliation, and one VOID return. It is not general process creation, image-path ownership, desktop selection, console or standard-handle inheritance, show-window behavior, automatic CPU dirty tracking, or a generated ABI gateway.

The next checkpoint should measure and return exact thunk ID `84` for the observed `HeapSetInformation(NULL, HeapEnableTerminationOnCorruption, NULL, 0)` call, then stop at the next exact owned thunk. It must not claim general heap allocation, process heap ownership, mitigation policy, or arbitrary `HEAP_INFORMATION_CLASS` support.
