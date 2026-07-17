# BGI `LoadIconA` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-load-icon-a`
- Previous capability checkpoint: `bgi-create-waitable-timer-a`
- First unsupported capability: `user32-load-cursor-a`

This checkpoint adds bounded PE32 resource catalogs and the reached module
integer-resource `LoadIconA` contract. Rust owns the parsed resource identity
and the process-shared logical HICON. TypeScript receives an immutable
completion snapshot for equivalence checking and does not own Win32 icon
identity or lifetime.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/load-icon-a-v1.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event load-icon-a-complete `
  --observe load-icon-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 285,147 | 779 | 354 | `668a89b250c796eadce88ea8eec767220c8671b4a63b541327ad6969bdc8c49d` |
| Confirmation | 2,257,599 | 6,952 | 3,438 | `7b6fdc36e757566b7bb71cbfa21960e3b7297c5b2ee413ff7abfa25f5252a098` |

Both captures have balanced call IDs and zero hook, vtable, stack, agent, and
cleanup diagnostics. The source executable remained 1,193,472 bytes with
SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

The guarded x86 oracle calls `LoadIconA` twice with main-module resource ID
101. Both calls return the same non-NULL HICON and success LastError 0.
`GetIconInfo` identifies a 32×32 color bitmap with one plane and 32 bits per
pixel; the observed large and small system metrics are 32×32 and 16×16.
Resource ID 65,535 returns NULL with LastError 1,813. Hooked and direct system
results agree.

BGI reaches the same resource at return RVAs `0x00067CF3` and `0x00067D7C`.
Both results become `WNDCLASSEXA.hIcon`; `hIconSm` remains NULL. Static caller
inspection shows `LoadCursorA(NULL, IDC_ARROW)` and `RegisterClassExA` after
the first call. The BGI resource tree contains `RT_GROUP_ICON` 101, language
1,041, with 256／48／32／16-pixel entries. Windows selects 32-pixel `RT_ICON`
3, whose DIB is 4,264 bytes. Native HICON and bitmap handle values are artifact
identity and are not imported into Vine.

## PE resource, policy, and ownership

`Pe32LoadPlan::resources` is an optional bounded catalog of type／name／language
identities and leaf RVA／size／code page. Integer and UTF-16 named entries are
supported. Directory, entry, leaf, and name limits are 4,096, 65,536, 65,536,
and 1,024 UTF-16 code units. Offsets, depth, directory bands, duplicate
identities, file backing, and data ranges are checked before a plan is
published.

`RuntimeModules` retains that immutable catalog beside the sealed mapped image
and indexes images by load base. `LoadIconA` validates the supplied module
against this registry, selects language-matched group and icon leaves, and
reads only their registered guest ranges. A leaf referring outside the
registered resource catalog cannot be smuggled in by guest bytes.

User32 policy version 3 enables exactly the reached slice: mapped module,
integer group resource, exact 32×32 selection, and at most 64 group entries.
Among exact-size candidates, the highest bit depth wins. The selected image
must be a bounded DIB whose header, doubled icon height, planes, bit depth, and
declared byte count match the group entry. Success sets LastError 0. Missing
resource, malformed resource data, and invalid module return errors 1,813,
1,812, and 6 without partial state.

Private `runtime/user32.rs::RuntimeUser32` owns monotonic icon ID, logical
handle namespace `vine.logical-icon.v1`, resource index, handle index, and
generation. The first icon is ID 1／HICON `0x49010001`. Load is a mutation-free
plan followed by a stale-checked targeted commit; a repeated resource returns
the existing identity without advancing allocators. Process teardown retires
all icon identities in the same lifecycle transaction. Logical HICONs are not
Win32 object-table handles or browser resource IDs.

The reproducible public fixture is 16,896-byte `icon-resource.exe`, SHA-256
`52587db69f1ea9f39f8604bf7ccb8c50ee783f7378228cf5f20229ac54803dac`.
It contains group 101, language 1,041, and 48／32／16-pixel DIB leaves. Its plan
has seven directories, ten entries, and four leaves.

Representative tests:

```powershell
npm run check:toolchain
cargo test -p vine-core --test pe32_resources --locked
cargo test -p vine-core --test user32_dispatcher --locked
npm run typecheck:runtime:browser-worker
```

The negative contracts cover malformed tree offsets and bands, duplicate
identity, over-limit counts and names, missing group／icon, malformed DIB,
invalid module, unsupported system／string／PNG variants, repeated identity,
capacity, stale plan, and process teardown without allocator or index drift.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-load-icon-a
npm run check:runtime:bgi-browser
```

Node completes 1,339 calls across 60 contracts. `LoadIconA` is sequence 1,339
at instruction 49,942, returns `0x49010001`, and emits the immutable resource
snapshot above with no guest write. Node records 1,345 writeback batches,
2,542 dirty pages, 9,189 changed bytes, 85 host deltas, final generation
1,459, and memory SHA-256
`b342681d4f87eaf94a93d2f5b86fd66e5fad3168f364354f42149d6b6d0136c2`.
The 5,467,360-byte report has SHA-256
`283fa974605ad58aaac8b01a493654f18209db4781929769b444ad7f66c7a0ee`.

Chrome Runtime Worker reproduces every ordered completion, the icon snapshot,
memory digest, and next stop with 6,849 slices, 5,504 yields, 4,976 writeback
batches／6,726 pages, 85 mapping deltas, final generation 3,612, and 6,409
Wasm crossings. Cancel and watchdog paths finish with no pending request or
callback. The 807,154-byte browser report has SHA-256
`c29ffd39a7bb260fb6f4a029a543d309efd8821b0f9bd38cd5d4dbaa1b4f2b1f`.
Runtime-core Wasm is 2,438,709 bytes with SHA-256
`146daeb99867ba30908e8b5d234b2d39211d24434853ed4dc79972145f35d8c5`.

Because this checkpoint changes startup resource parsing, runtime-core size,
and a Worker completion payload, the three-minute workload gate was rerun. It
completed 254 sessions with zero failure and passed all 17 versioned
thresholds. Session p95 was 734.170 ms, slice p99 was 4.135 ms, heartbeat p99
was 10.370 ms, and the maximum heartbeat gap was 38.570 ms. The 969,455-byte
benchmark report has SHA-256
`e75cac32068f78a1d00beec9b6c623d6cab18b8f0416662b2dcf88d204c765e4`.

## Non-claims

- System icons, string resource names, PNG-compressed icon images, alternate
  size selection, cursors, icon destruction, icon drawing, and class
  registration are not implemented by this checkpoint.
- The logical HICON does not expose a native HICON, GDI bitmap, DOM image, or
  browser renderer resource.
- Resource editing, lookup across fallback languages, MUI modules, and general
  `FindResource`／`LoadResource` APIs remain outside this slice.

## Next structured stop

```text
USER32.dll!LoadCursorA(0x00000000, 0x00007F00)
endpoint ID:       244
endpoint address:  0x7F000F30
IAT address:       0x100D14A8
return address:    0x10067D09
instruction count: 49,949
fault:             endpoint-contract-missing
```
