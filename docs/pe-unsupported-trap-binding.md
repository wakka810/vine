# PE32 explicit unsupported trap binding

## 1. Status

- Status: Verified controlled contract and sealed BGI image
- Date: 2026-07-11
- Schema: `vine.pe32-import-binding` version `1`, optional unsupported-trap fields
- Primary source: `V:\softs\サクラノ詩\BGI.exe`
- Primary size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Primary report size／SHA-256: 126,633 bytes／`7f9d96c502ae1b5cacd27da7a71cad87ada56a9fcf31ea9bc68b73bdf6ff54dc`

This checkpoint continues [pe-import-binding.md](pe-import-binding.md). The declared-only policy reports every catalog-missing import without mutation. An explicit optional policy instead plans each missing identity as an owned `UD2` trap so one atomic commit can seal the image without inventing an API result.

## 2. Binding contract

`RuntimeExportCatalog` remains the set of declared runtime endpoints. The new policy does not add unknown exports to that catalog. Instead, each planned import receives one of two dispositions:

| Disposition | Meaning | Serialized metadata |
|---|---|---|
| Declared runtime export | The catalog contains the exact module／name or module／ordinal identity | omitted to preserve the previous version 1 JSON |
| Unsupported trap | The catalog lacks the identity; dispatch must stop before any return value or stack cleanup | `"disposition": "unsupported-trap"` |

Both dispositions use the existing 16-byte owned slot containing `UD2` followed by `HLT`. The bytes alone do not determine service behavior; exact thunk metadata supplies requester owner, module, name／ordinal, IAT RVA／address, and original IAT value. `thunk_at` accepts only an exact owned slot start.

When `--trap-unsupported-imports` is absent, incomplete planning returns the full unresolved list, creates no arena, rewrites no IAT, and leaves generation unchanged. Adding endpoints and retrying the same image is supported. Both policies share the same read-only plan and rollback commit machinery.

## 3. Controlled contract

The representative test declares only `vine_fixture.dll!fixture_add`. It binds `fixture_call_callback` as the one unsupported trap and still seals both image and runtime owners.

| Field | Result |
|---|---|
| Total／bound／remaining | `2`／`2`／`0` |
| Declared endpoint | ID `1`, `fixture_add` |
| Unsupported trap | ID `2`, `fixture_call_callback`, IAT `0x10002038` |
| Arena | 1 page, existing SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc` |
| Owners／delta | both sealed, generation `6` before initial reflection acknowledgement |
| Stop | `thunk-dispatch`／`unsupported-import-trap-dispatch-pending` |

After acknowledgement, the test has zero dirty pages, reads both IAT pointers, and fetches the unsupported `UD2` slot. It does not execute the trap or synthesize a return. Failpoint coverage verifies rollback after arena creation, page fill, each thunk／IAT write, verification, and either owner seal.

Two controlled reports were byte-identical at 4,487 bytes with SHA-256 `298797b19f15d4a1a95c048e24820b1af9fd1aaedd7833f0fa1560a46cc429bf`.

The automated command is:

```powershell
npm run check:toolchain
npm run check:runtime:imports
```

## 4. Primary BGI result

The primary replay keeps the native-reachable `d3d9.dll!Direct3DCreate9` endpoint declared and applies the explicit policy to every other static import:

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 382 `
  --materialize-image-base 0x10000000 `
  --bind-name d3d9.dll Direct3DCreate9 `
  --trap-unsupported-imports `
  --output .work\import-binding\bgi-traps.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Total／bound／remaining imports | `330`／`330`／`0` |
| Declared／unsupported dispositions | `1`／`329` |
| First thunk | ID `1`, `d3d9.dll!Direct3DCreate9`, `0x7F000000`, IAT `0x100D14F4` |
| First unsupported | ID `2`, `DINPUT8.dll!DirectInput8Create`, `0x7F000010`, IAT `0x100D1050` |
| Last thunk | ID `330`, `DSOUND.dll` ordinal `11`, `0x7F001490`, IAT `0x100D1058` |
| Arena | 2 committed pages／330 slots, SHA-256 `88f1b31a1ed2f724903f9ddaf16c789240b2010077537b9203faa44a63c20dde` |
| Memory | 382 committed／physical／dirty pages, 396 reserved pages |
| Owners／delta | image `1` and runtime `2` sealed, generation `6` |
| Stop | `thunk-dispatch`／`unsupported-import-trap-dispatch-pending` |

The 329 unsupported identities cover 328 named imports and `DSOUND.dll` ordinal `11` across 13 modules; the one declared D3D9 name is the fourteenth module. Two independent reports were byte-identical. The BGI source retained its original size and SHA-256 before and after both runs.

## 5. Non-claims and next checkpoint

Binding an unsupported trap is not export implementation, DLL loading, API-set resolution, a successful `LoadLibrary`／`GetProcAddress`, calling-convention knowledge, stack cleanup, LastError behavior, or a stubbed API return. No BGI instruction executes in this checkpoint, so the order of static imports does not predict the first runtime-reachable service.

The fixed arena address, plan-order IDs, and 382-page quota remain version 1 checkpoint choices. The report is not a snapshot format, and the binder still does not model forwarded exports, module objects, DLL lifecycle, TLS, or delayed imports. Transaction rollback currently copies the committed frame snapshot; it is a correctness boundary rather than the final large-image optimization.

That continuation is complete in [v86-bgi-first-trap.md](v86-bgi-first-trap.md). The bounded 388-page state reaches exact thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime` after 19 instructions, confirming that static ID `1` is not the first runtime call. This binding report and its non-execution claim remain unchanged.
