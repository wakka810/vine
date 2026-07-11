# Bootstrap FS first-stop ingress

## 1. Status

- Status: Verified controlled first stop
- Date: 2026-07-11
- Fixture: `.work/toolchain-spike/bootstrap-fs.exe`
- Fixture size: 2,560 bytes
- Fixture SHA-256: `5712377ff8a8bd2be292f8678902d19d3b4b2f1c1919922cce9d373f0696637c`
- Report schema: `vine.v86-bootstrap-fs-first-stop` version `1`
- Report size: 5,982 bytes
- Report SHA-256: `9bca668c33208fa1bc6a4e8253dafb247bd9520a106761a24fd06cf46364f7f9`

This checkpoint fixes the minimum i386 thread/process environment access before Vine supplies that environment. It proves the fixture against native Windows, reflects its import-free image through production Rust memory, and records the exact source-built v86 failure with FS base zero.

## 2. Fixture and native oracle

`fixtures/pe32/bootstrap-fs/fixture-bootstrap.c` uses the compiler intrinsic for these two direct accesses:

```text
FS:[0x18] -> TEB self pointer
FS:[0x30] -> PEB pointer
```

The fixture rejects a null TEB or PEB, then verifies `*(TEB + 0x18) == TEB` and `*(TEB + 0x30) == PEB`. It is PE32／i386, has no imports, exports the entry and four observed values, and returns native exit code `0` on the recorded Windows host. Its first 14 entry bytes are:

```text
64 8B 15 18 00 00 00    mov edx, dword ptr fs:[0x18]
64 8B 0D 30 00 00 00    mov ecx, dword ptr fs:[0x30]
```

The native result is a self-oracle for these offsets, not a claim that Vine adopts every internal field of the host Windows PEB／TEB.

## 3. Production reflection

The reflection session now permits a fully bound import-free image without inventing an import thunk. Existing imported-image offers keep their previous JSON shape and all six earlier entry／cdecl／callback／return report hashes remain unchanged.

The fixture is relocated from preferred base `0x00400000` to `0x10000000`. Rust owns five image pages as owner `1`, the combined callback／process-entry control page as owner `2`, and the initialized one-page stack as owner `3`. There is no import arena. Generation `6` contains seven reflected pages with content SHA-256 `bb476b06336e93654df562ce78054e0927d5b2527ac12b54241d84a1d50b0696`.

The adapter uses 11 physical pages／44 KiB:

```text
page directory
page tables for directory indices 1, 64, and 508
seven reflected production pages
```

Rust reports three owners／regions, 22 reserved pages, and seven committed／physical／dirty pages. The initial ESP is `0x006FFFFC`, containing process-entry sentinel `0x7F010010`.

## 4. Exact first stop

The version 1 CPU state starts in flat CPL3 state with FS selector `0`, FS base `0`, EIP `0x10001000`, and instruction budget `3`. The first source-v86 execution stop is:

| Field | Result |
|---|---:|
| Stop reason | structured `#PF` `0x10E` |
| PFEC | `4` (CPL3 non-present read) |
| CR2 | `0x00000018` |
| Current EIP | `0x10001007` |
| Previous／faulting EIP | `0x10001000` |
| ESP | `0x006FFFFC` |
| EDX | `0` |
| FS selector／base | `0`／`0` |
| Execute／budget stops | `1`／`0` |

v86 advances its current EIP past the decoded seven-byte instruction while preserving the faulting instruction in previous EIP. The report retains both values. CR2 `0x18` proves that the flat zero FS base, rather than a fabricated TEB pointer, caused the stop.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:bootstrap-fs-first-stop
```

## 5. Non-claims and next checkpoint

No Rust-owned TEB or PEB page is installed in this checkpoint. It does not set a nonzero FS base, execute the second FS read, complete the fixture, provide process parameters／loader lists／TLS／LastError, or define general process and thread lifecycle. The one-page stack and fixed control addresses remain controlled checkpoint choices.

The next checkpoint adds one Rust-owned TEB page and one Rust-owned PEB page, initializes only TEB offsets `0x18` and `0x30`, reflects both pages, and sets the versioned CPU state's FS base to the TEB address. The same fixture must then observe both exact fields and return `0` to the existing process-entry sentinel.
