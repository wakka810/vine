# BGI `LoadCursorA` checkpoint

## Status

- Status: verified
- Date: 2026-07-18
- Runtime-kernel checkpoint: `user32-load-cursor-a-module-resource`
- Previous capability checkpoint: `kernel32-critical-section-primary-capacity`
- Next structured stop: `d3d9-root-factory-and-identity`

This checkpoint completes the reached shared-cursor band:
`LoadCursorA(NULL, IDC_ARROW)` and `LoadCursorA(main module, 106)`. Rust owns
both logical cursor identities and their process lifetime. TypeScript receives
immutable completion metadata and does not own an HCURSOR or cursor bitmap.

## Native and corpus evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/load-cursor-a-module-v2.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event load-cursor-a-complete `
  --observe load-cursor-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 336,703 | 812 | 370 | `db9b2bbb24723939dfff8f86ce65c8c784a81b108df459c75007a86014321c5a` |
| Confirmation | 338,341 | 815 | 371 | `f7855d7240e4523b77d65571a0926e28cc4ee63f421a6840c71f86f6c5940ae7` |

Both captures have contiguous sequence numbers, balanced call IDs, and zero
hook, vtable, stack, agent, and cleanup diagnostics. The source executable
remained 1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

The guarded x86 oracle establishes two distinct success policies. Repeated
`LoadCursorA(NULL, IDC_ARROW)` returns the same shared system HCURSOR and
preserves seeded LastError; missing predefined ID 65,535 returns NULL／1,814.
Repeated `LoadCursorA(main module, 106)` returns the same shared module HCURSOR
and sets LastError 0; missing module ID 65,535 and wrong-type ID 101 both return
NULL／1,813. Hooked and direct results agree in both captures. Native HCURSOR
values remain opaque artifact identity and are not imported into Vine.

The immutable BGI resource catalog contains one `RT_GROUP_CURSOR(12)` leaf for
ID 106／language 1,041 at RVA `0x0016E9F4` (20 bytes). It selects
`RT_CURSOR(1)` ID 5 at RVA `0x00154F88` (2,220 bytes): 32×32, one plane, 8 bpp,
hotspot (0,0). The group declares the doubled DIB height of 64. The runtime
validates the group, payload prefix, DIB header, language relation, and catalog
RVA／size identity before allocating a logical cursor.

BGI calls `LoadCursorA` four times at return RVAs `0x00067D09`, `0x00067D8C`,
`0x00067FD2`, and `0x00067FE1`. The first three request the shared system arrow.
The fourth requests main-module resource 106, stores the non-NULL result in its
cursor global, and returns success from the initializer. The first arrow is the
`WNDCLASSEXA.hCursor` consumer for both registered classes.

## Policy and ownership

User32 policy version 13 enables the shared system arrow plus bounded integer
module resources with exact 32×32 selection and at most 64 group entries.
String names, other predefined system cursors, missing policy, unmatched size,
PNG payloads, and malformed metadata stop before identity mutation. Invalid
module, missing resource, wrong type, and malformed data return the native
failure codes 6, 1,813, or 1,812 as appropriate.

Private `runtime/user32.rs::RuntimeUser32` owns the monotonic cursor ID,
logical handle namespace `vine.logical-cursor.v1`, source index, handle index,
capacity, and generation. System arrow is ID 1／HCURSOR `0x43010001`; module
resource 106 is ID 2／HCURSOR `0x43010002`. A source is planned without mutation,
then stale-checked and committed. Repeating either source reuses identity.
Module source identity includes the registered module ID／base／source hash and
the selected group／payload metadata; `RuntimeKernel` verifies both catalog
leaves before allowing the User32 commit.

Cursor and icon plans share the User32 generation, so cross-resource stale
plans fail. The process bound is 256 shared cursor identities, with exact-limit
failure atomicity. Process teardown retires icon, cursor, class, and window
identities in one User32 lifecycle commit. Logical HCURSORs are not object-table
handles, COM pointers, native handles, DOM cursor values, or browser resource
IDs.

The public controlled PE fixture is 21,504-byte `icon-resource.exe`, SHA-256
`2ddcca724644e16405cc03d71e3b0d5460ddb72ac60072a1781f128fd5e4da4c`.
Besides its three icon leaves it contains group cursor 106／language 1,041 and a
32×32／32-bpp cursor with hotspot (3,4). Its resource plan has 11 directories,
16 entries, and six leaves.

Representative tests:

```powershell
npm run check:toolchain
cargo test -p vine-core --lib runtime::user32::tests --locked
cargo test -p vine-core --test user32_dispatcher --locked
cargo check -p vine-pe-probe -p vine-runtime-wasm --locked
npm run check:windows-trace
```

Negative coverage includes missing／wrong-type resources, unknown module,
system and module string names, unsupported predefined cursor, malformed group,
unmatched size, invalid hotspot, disabled policy, stale cross-resource plan,
exact capacity overflow, and process teardown without guest-memory, index, or
allocator drift.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-set-window-pos-initial-placement
npm run check:runtime:bgi-browser
```

Node completes 17,815 calls across 75 API contracts. Module `LoadCursorA` is
sequence 17,815／occurrence 4 at instruction 11,806,421, returns
`0x43010002`, sets LastError 0, and emits the immutable module-resource snapshot
without a guest write. Node records 17,862 writeback batches／36,000 dirty pages／
1,806,863 changed bytes, 16,547 host deltas, final generation 34,481, and memory
SHA-256 `a99b92dda8434553d23213a18d4e468ce784d02da969917c47529ece24e7c71f`.
The 207,005,816-byte report has SHA-256
`015528f1469aebec9959f7eba37c7c0593443fbbf3e1daf33da1dac174aa0731`.

Chrome Runtime Worker reproduces all 17,815 ordered completions, cursor
metadata, memory digest, and the D3D9 stop with 40,261 slices／22,399 yields,
40,255 writeback batches／87,857 pages, final generation 56,875, and 74,673 Wasm
crossings. Cancel and watchdog paths leave no pending request or callback. The
24,156,023-byte browser report has SHA-256
`87710b68c70f96b621bb4b8e3bd57dc9a5daa92dd2bd15c0bb9cd83b512b8dc8`;
runtime-core Wasm is 2,967,736 bytes with SHA-256
`54b0c366877379dfd8ef7340ec45c86219e1033c1901a3ab48cc6d6b51ffcba9`.

This semantic leaf does not change the Worker protocol, slice budget, memory
transport, graphics/audio path, or performance threshold. The most recent
three-minute performance authority therefore remains the immediately preceding
critical-section capacity gate: 30 sessions, zero failure, all 17 thresholds
passed.

## Non-claims

- Other predefined cursor kinds, string resource names, cursor destruction,
  active cursor selection, animation, and `SetCursor` are not implemented.
- Logical cursors retain resource identity and validated metadata, not decoded
  XOR／AND pixels or a browser-renderable bitmap.
- No CSS cursor, DOM input behavior, native HCURSOR, or browser renderer
  resource is created.
- D3D9 behavior is not implemented by this checkpoint.

## Next structured stop

`Direct3DCreate9` is a subsystem gateway: it introduces D3D9 frontend policy,
COM-style root／device identity, capability queries, presentation parameters,
logical graphics resources, and a browser renderer boundary. It must be closed
as a minimal vertical slice rather than a factory-return stub.

```text
d3d9.dll!Direct3DCreate9(32)
endpoint ID:       1
endpoint address:  0x7F000000
IAT address:       0x100D14F4
return address:    0x1005B892
instruction count: 11,806,445
fault:             endpoint-contract-missing
```
