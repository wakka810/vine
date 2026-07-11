# PE32 import thunk binding

## 1. Document information

- Status: Verified checkpoint
- Date: 2026-07-11
- Checkpoint: production runtime-owned import thunk and IAT binding
- Schema: `vine.pe32-import-binding` version `1`
- Primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

This checkpoint gives materialized PE32 imports stable runtime ownership. Planning is read-only across every import and assigns deterministic IDs／addresses in load-plan order. Commit creates the bounded executable thunk arena, rewrites every IAT slot, verifies it, and seals both owners inside one rollback transaction. If any import lacks an allowed endpoint, all unresolved identities are returned and guest memory is unchanged.

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
| Committed backing | Only pages required by the complete plan |
| Page protection | logical RX |
| Default committed byte | `HLT` (`0xF4`) |
| Owned slot prefix | `UD2` (`0x0F 0x0B`) |

Phase 1 verifies the loader schema, image source, load base／range, live unsealed owner, materialization stop, pending generation, thunk capacity, fixed arena availability, physical quota, and every original IAT DWORD without mutation. Each `PlannedImport` records module, name／ordinal, requester owner, IAT RVA／address／original value, optional catalog endpoint, disposition, stable ID, and thunk address. `RequireDeclaredExport` reports every unresolved import across every module. `BindUnsupportedTrap` changes those dispositions to identity-preserving returnless traps.

Phase 2 revalidates the memory generation and every IAT value, then creates the arena, commits pages, writes thunk bytes and all IAT values, verifies IATs, hashes the arena, and seals runtime／image owners in one `GuestMemory` transaction. Any error restores owners, regions, frames, bytes, protection, seal state, generation, and pending delta. A successful bind is explicitly non-repeatable because the image owner is sealed; an incomplete plan can be retried on the same image after catalog extension.

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
| Memory generation | `6`, data TLB／fetch／generated code pending before initial reflection |
| Stop | `process-bootstrap`／`peb-teb-process-parameters-unimplemented` |

After initial reflection acknowledgement, dirty state is zero. The test reads both IAT pointers through normal guest access, fetches each `UD2`／`HLT` slot, verifies an unowned committed slot is all `HLT`, and confirms the sealed read-only IAT rejects guest writes with PFEC `7`. Address lookup maps only exact owned slot starts back to their IDs and symbols.

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

Two reports were byte-identical at 4,483 bytes with SHA-256 `bdcc35c73320d5c628cd917b6ab0b0c9a2d2c3aefad7673ca7dc03e169a9b661`.

## 4. Primary BGI result

Native trace proves that BGI reaches `Direct3DCreate9`, but a catalog containing only that endpoint is now a read-only incomplete plan. No prefix is committed.

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 380 `
  --materialize-image-base 0x10000000 `
  --bind-name d3d9.dll Direct3DCreate9 `
  --output .work\import-binding\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Total imports | 330 |
| Bound imports | 0; planning found an incomplete catalog |
| Planned declared endpoint | ID `1`: `d3d9.dll!Direct3DCreate9` |
| Runtime arena | not created |
| Unresolved imports | all 329 identities across the remaining modules |
| First unresolved | `DINPUT8.dll!DirectInput8Create`, IAT `0x100D1050` |
| Image owner | unsealed |
| Image owner／IAT | unsealed and byte-identical to pre-plan state |
| Memory generation | unchanged materialization generation `2` |
| Stop | `import-resolution`／`import-symbols-unimplemented` |

Two reports were byte-identical at 86,073 bytes with SHA-256 `6887944898fc8cecb132d63774573e6823e4ade25aba8aee6de98b31d4b6a69f`. The larger report preserves every unresolved identity rather than only the first stop. The original BGI file remained 1,193,472 bytes with its primary SHA-256.

## 5. Non-claims and next checkpoint

This is partial evidence for `REQ-PE-03`, `REQ-PE-06`, `REQ-ABI-01`, `REQ-DIAG-03`, `P1-ABI-01`, `P2-PE-04`, `P2-DIAG-01`, `P2-COR-01`, `AC-P2-01`, and `AC-P2-06`. None is complete.

Within this import-binding checkpoint no thunk is dispatched. A declared D3D9 endpoint does not implement `Direct3DCreate9`; reaching it must still stop until a typed frontend exists. The `0x7F000000` arena is a version 1 checkpoint allocation, not a final process-layout or ASLR decision. General DLL／export／forwarder resolution, API metadata generation, calling conventions, LastError, callbacks, and SEH remain unimplemented.

The subsequent [v86 entry checkpoint](v86-entry-ingress.md) reflects the sealed controlled image and thunk arena into provisional v86 paging and reaches exact thunk ID `1`. Later continuations return one fixed service／callback, classify exact entry completion in [v86-entry-return-ingress.md](v86-entry-return-ingress.md), and reflect a Rust-owned stack in [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md). General service／callback handling remains later work. This import-binding report and its stops remain unchanged.

The optional continuation in [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md) binds catalog-missing identities to explicit returnless traps. It seals the BGI image only after all 330 IAT slots have owned metadata; it does not claim those APIs are implemented.
