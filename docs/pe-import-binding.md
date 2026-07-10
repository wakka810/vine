# PE32 import thunk binding

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: production runtime-owned import thunk and IAT binding
- Schema: `vine.pe32-import-binding` version `1`
- Primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

This checkpoint gives materialized PE32 imports stable runtime ownership. It creates a bounded executable thunk arena, assigns deterministic IDs in load-plan order, writes owned thunk addresses into IAT slots, and seals a controlled image only when every import has a declared trap endpoint. An unsupported import binds only the supported prefix and returns module／symbol／IAT context without sealing the image or pretending the service succeeded.

## 2. Production boundary

`RuntimeExportCatalog` is an explicit development-time endpoint catalog. DLL names match ASCII case-insensitively, while export names and ordinals match exactly. Duplicate or invalid catalog entries fail before binding state changes. Declaring an endpoint means only that the loader may bind a runtime-owned trap address; it does not claim Win32／DirectX service behavior.

Version 1 thunk ownership is:

| Property | Value |
|---|---:|
| Reserved arena | `[0x7F000000, 0x7F010000)` |
| Reservation | 64 KiB／16 pages |
| Slot stride | 16 bytes |
| Capacity | 4,096 slots |
| ID | 1-based, plan-order stable |
| Committed backing | Only pages required by the supported prefix |
| Page protection | logical RX |
| Default committed byte | `HLT` (`0xF4`) |
| Owned slot prefix | `UD2` (`0x0F 0x0B`) |

The binder first verifies the loader schema, image source, recorded load base／range, unsealed owner report, materialization stop, pending generation, and every candidate IAT DWORD before mutation. Owner-scoped memory access also requires the referenced owner and region to remain live. It computes the supported prefix and physical quota before creating the arena. For each prefix import it writes `UD2` at the owned slot and the slot address into the read-only image IAT through the owner-scoped initialization path. It verifies all writes, hashes committed arena bytes, and seals the runtime owner.

If all imports bind, the image owner is also sealed. If one is unsupported, later imports are untouched and the image remains unsealed. Mapping, runtime-owner seal, and optional image seal merge into the existing pending invalidation generation. Guest access remains blocked until an actual adapter acknowledges that generation.

## 3. Controlled fixture result

The representative contract binds both `abi-smoke.exe` imports after relocating the image to `0x10000000`.

```powershell
npm run check:toolchain
npm run check:runtime:imports
```

| Field | Result |
|---|---|
| Physical quota | 6 pages: 5 image + 1 thunk page |
| Runtime exports | `vine_fixture.dll!fixture_add`, `fixture_call_callback` |
| Thunks | ID `1` at `0x7F000000`, ID `2` at `0x7F000010` |
| IAT | `0x10002034`／`0x10002038` rewritten and read back |
| Arena digest | SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc` |
| Owners | image `1`, runtime `2`, both sealed |
| Invalidation | generation `4`, data TLB／fetch／generated code |
| Stop | `process-bootstrap`／`peb-teb-process-parameters-unimplemented` |

After acknowledgement, the test reads both IAT pointers through normal guest access, fetches each `UD2`／`HLT` slot, verifies an unowned committed slot is all `HLT`, and confirms the sealed read-only IAT rejects guest writes with PFEC `7`. Address lookup maps only exact owned slot starts back to their IDs and symbols.

The deterministic probe command is:

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7 `
  --prepare-memory-pages 6 `
  --materialize-image-base 0x10000000 `
  --bind-name vine_fixture.dll fixture_add `
  --bind-name vine_fixture.dll fixture_call_callback `
  --output .work\import-binding\abi.json `
  .work\toolchain-spike\abi-smoke.exe
```

Two reports were byte-identical at 4,421 bytes with SHA-256 `04d6f001e157c0e0fc3f1878516fb913a131e5cfe187162cf265b375e949472d`.

## 4. Primary BGI result

Native trace already proves that BGI reaches `Direct3DCreate9`, so the probe declares only that trap endpoint and does not bulk-bind unobserved APIs.

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 381 `
  --materialize-image-base 0x10000000 `
  --bind-name d3d9.dll Direct3DCreate9 `
  --output .work\import-binding\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Total imports | 330 |
| Bound prefix | 1: `d3d9.dll!Direct3DCreate9` |
| Bound IAT／thunk | `0x100D14F4` -> ID `1`／`0x7F000000` |
| Runtime arena | 1 committed page, sealed, SHA-256 `4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059` |
| Remaining imports | 329 |
| First unresolved | `DINPUT8.dll!DirectInput8Create`, IAT `0x100D1050` |
| Image owner | unsealed |
| Invalidation | generation `3` |
| Stop | `import-resolution`／`import-symbol-unimplemented` |

Two reports were byte-identical at 4,258 bytes with SHA-256 `64217ed90cd86098396d27652ed41828ffeb6354f29f96bb7cc981daaab34fc0`. The original BGI file remained 1,193,472 bytes with its primary SHA-256.

## 5. Non-claims and next checkpoint

This is partial evidence for `REQ-PE-03`, `REQ-PE-06`, `REQ-ABI-01`, `REQ-DIAG-03`, `P1-ABI-01`, `P2-PE-04`, `P2-DIAG-01`, `P2-COR-01`, `AC-P2-01`, and `AC-P2-06`. None is complete.

No thunk has been dispatched by the production CPU path. The D3D9 endpoint does not implement `Direct3DCreate9`; reaching it must still stop as an unimplemented service until a typed frontend exists. The `0x7F000000` arena is a version 1 checkpoint allocation, not a final process-layout or ASLR decision. General DLL／export／forwarder resolution, API metadata generation, calling conventions, LastError, callbacks, SEH, v86 page reflection, PEB／TEB／stack, and guest execution remain unimplemented.

The next primary checkpoint reflects the sealed controlled image, thunk arena, and a bounded stack into the provisional v86 paging adapter. It acknowledges generation `4` only after real TLB／fetch invalidation, starts the controlled entry point, and classifies the first `UD2` as exact owned thunk ID `1`. Host service execution and callback handling remain the following checkpoint.
