# BGI ninth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-ninth-win32-call` version `1`
- Report size／SHA-256: 56,729 bytes／`147a3599cf942af03034c7f410d333b35c0360812936e39a7473c43008d8d5be`

This checkpoint continues [v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md). It declares exact thunk ID `72`, validates the one-argument stdcall frame, indirect IAT call, and bounded guest UTF-16 name in Rust, resolves one controlled runtime module record, returns its typed logical handle, and stops at the next exact owned thunk. It does not resolve an export.

## 2. Native observation

The focused Windows trace was captured with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\bgi-module-handle-20260712.jsonl" `
  --seconds 3 `
  --kill-after `
  --observe module-handle
```

The uncommitted raw trace is 347,850 bytes with SHA-256 `bac37acf086ff691ba436b2959143c76894f20f5e271667813cefebeb5ee3557`. The target call returned to `BGI.exe + 0xA5811`:

| Field | Native observation |
|---|---:|
| API／argument | `GetModuleHandleW(L"KERNEL32.DLL")` |
| Native result | `0x758B0000` |
| LastError before／after | `87`／`87` |

The native result is the host process's ASLR-specific KERNEL32 image base and is not imported into Vine. The immediate BGI-owned consumer is `GetProcAddress(result,"FlsAlloc")` at caller RVA `0xA582F`; the same native handle is then used for `FlsGetValue`, `FlsSetValue`, and `FlsFree` lookups.

## 3. Typed logical loaded-module state

Rust creates exactly one record owned by `checkpoint-loaded-modules`:

| Field | Controlled value |
|---|---:|
| Variant | `getmodulehandlew-kernel32-loaded-runtime-module` |
| Handle namespace | `vine.logical-loaded-module.v1` |
| Type tag／generation／slot | `0x4D`／`1`／`1` |
| Encoded handle | `0x4D010001` |
| Requested／canonical name | `KERNEL32.DLL`／`kernel32.dll` |
| Name match | Unicode, case-insensitive basename |
| Role／lifecycle | `runtime-system-module`／`loaded` |
| Reference count changed | `false` |
| Guest system-DLL image | `not-present` |
| Export resolution | deferred to first reached `GetProcAddress` |

The type tag keeps this handle distinct from the private heap and native host addresses. The encoding and metadata-only system-module representation are version-1 checkpoint choices. Windows normally exposes an image base as `HMODULE`; this checkpoint supports only opaque identity use by the observed dynamic-export lookup and does not permit guest PE-header reads through the logical value.

## 4. Checked UTF-16 stdcall

This mode adds `GetModuleHandleW` as the tenth declared import endpoint, leaving 320 identity-preserving unsupported traps. Reflected bytes and generation remain unchanged. The initial offer is 4,904,305 bytes.

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000470`／`0x100D11C8` |
| Indirect call／return | `0x100A580B`／`0x100A5811` |
| ESP before return | `0x006FFF70` |
| Frame | `[0x100A5811,0x100D24D0]` |
| stdcall argument bytes | `4` |
| ESP after callee return | `0x006FFF78` |
| EAX before／after | `1`／`0x4D010001` |

Rust reads the NUL-terminated name at `0x100D24D0` with a 64-code-unit bound, validates UTF-16, and applies case-insensitive canonical matching. The 26 bytes for `KERNEL32.DLL\0` have SHA-256 `4d40ac1582e46f291dedb30130e36b167bbe19b401a248b1972b1bfa7391d924`. The call must be `FF 15 <0x100D11C8>`, and that IAT slot must still contain owned thunk `0x7F000470`.

## 5. Generation-preserving reconciliation

The runner supplies the current generation `27` stack page at `0x006FF000`, owner `4`. Execution since the heap-create return changed 9 bytes; the page SHA-256 is `2d08da9d16a6782982e4b2b9e4617909c4ec2a65fbc1702479972ad4618f4b2e`.

Rust reconciles the page and validates the frame, IAT provenance, guest pointer, terminator, decoded name, and canonical identity transactionally. Module lookup writes no guest bytes, so generation and acknowledged generation remain `27`, dirty pages remain zero, and no cache effect is requested. The request is 8,721 bytes and receipt is 1,499 bytes, excluding newlines.

## 6. Next runtime stop

After the stdcall return, BGI executes 9 more instructions in 9 execute stops with 8 budget stops. At cumulative instruction count `131`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `150`, `KERNEL32.dll!GetProcAddress` |
| Disposition／address | `unsupported-trap`／`0x7F000950` |
| IAT | `0x100D1300` |
| ESP／return | `0x006FFF68`／`0x100A582F` |
| Arguments | `[0x4D010001,0x100D250C]` -> logical KERNEL32／ASCII `FlsAlloc` |
| Raw stack words | `[0x100A582F,0x4D010001,0x100D250C,0,0,0x100A375B,0xE93F6D87,0]` |

The immutable source fingerprint, reflection SHA-256 `f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`, memory generation, and all earlier report hashes remain unchanged.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-ninth-win32-call
```

Two consecutive runs produced the same 56,729-byte report and SHA-256. `npm run check:private-corpus` runs the first-trap contract and all nine cumulative call-return contracts. Raw traces and generated reports remain under `.work/` and are not committed.

## 8. Non-claims and next checkpoint

This is one exact loaded-module lookup and one metadata-only logical module identity. It is not a general PEB loader list, guest system-DLL image, DLL load／unload lifecycle, path or API-set resolution, reference-count implementation, module enumeration, PE export parser, forwarder resolver, dynamic thunk allocator, or general LastError model.

The next checkpoint should measure and return exact thunk ID `150` for `GetProcAddress(logicalKernel32,"FlsAlloc")`, allocate an identity-preserving callable runtime endpoint for that export, and stop at the next exact owned trap. It must not return the host function address or claim general export／forwarder resolution.
