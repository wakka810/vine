# GDI frontend and logical surface contract

## Purpose and current boundary

The GDI frontend owns guest-visible GDI identity, lifetime, and logical drawing
state in Rust. The current bounded slice implements `USER32!GetDC` and
`USER32!ReleaseDC` for a logical window, the NULL-window primary screen
acquisition, one policy-owned `GDI32!EnumFontFamiliesExA` callback record, and
two exact `GDI32!CreateFontA` logical object variants. It also implements the
NULL-source `GDI32!CreateCompatibleDC` memory-DC lifecycle and `GDI32!DeleteDC`,
the reached 8-bpp `GDI32!CreateDIBSection` object and backing lifecycle, exact
font／bitmap `GetObjectA`, memory-DC `SelectObject`, and bounded `DeleteObject`.
The selected created font also supports bounded `GDI32!GetOutlineTextMetricsA`
and one exact `GDI32!TextOutA` CP932 glyph raster into the selected 8-bpp DIB.
It also supports the reached D3D9 system-memory surface. `GDI32!GetStockObject`
for `BLACK_BRUSH` and `USER32!FillRect` remain bounded to the reached window and
graphics-surface paths. This establishes the responsibility boundary needed for
later GDI work without claiming a general pixel renderer, screen drawing, general DC
behavior, or a shared Direct3D device.

`RuntimeGdi` is a private subsystem owned by `RuntimeKernel`. It owns the HDC,
GDI-object, and window-surface collections; their indexes and identity allocators;
bounded window fill history; and the subsystem generation. `RuntimeUser32` remains the
authority for windows and client geometry. The kernel passes GDI an opaque,
non-owning window relation only after revalidating the live HWND, owner thread,
and client geometry. GDI never owns or mutates the window record.

For `GetDC(NULL)`, `RuntimeDisplays` remains the authority for the configured
primary monitor identity and rectangle. The kernel resolves that monitor and
the current guest thread, passes a typed non-owning relation to `RuntimeGdi`,
and revalidates the display relation before commit and release. GDI owns only
the acquisition HDC and its typed target. It does not allocate a window
surface, screen pixel storage, or browser renderer resource.

For a D3D9 surface HDC, `RuntimeGraphics` remains the authority for the logical
surface descriptor, association, fill command history, and content generation.
The kernel passes `RuntimeGdi` a typed non-owning relation only after resolving
the live COM-to-graphics identity. GDI owns the acquisition HDC and its relation,
not the graphics surface or COM lifetime.

## Identity and lifetime

Each successful `GetDC(HWND)` creates a distinct monotonic acquisition identity
in namespace `vine.logical-gdi-dc.v1`. Concurrent acquisitions therefore have
different HDCs even when they refer to the same window surface. Native numeric
HDC reuse after `ReleaseDC` varies between runs and is not imported as a
semantic contract.

Each successful `GetDC(NULL)` likewise creates a distinct monotonic HDC. The
record contains the acquiring guest thread and the non-owning logical primary
monitor handle and rectangle. It creates no logical surface. Bounded
`ReleaseDC(NULL, hdc)` accepts only the live screen acquisition from the same
guest thread and retires that HDC; an unknown, repeated, wrong-target, or
wrong-thread release returns FALSE without changing LastError or another
acquisition. Native HDC values are opaque artifact identity.

The first acquisition for a window creates one logical client surface. Later
acquisitions share that surface identity while retaining distinct HDC
identities. `ReleaseDC` requires the exact HWND, live HDC, and acquiring guest
thread relation and retires only that HDC. It does not retire the surface or a
shared stock object. Process teardown preflights all referenced subsystems,
retires GDI before Graphics and User32, and then destroys those authorities, so
no GDI record retains a dangling window or graphics-surface relation.

At most one HDC may be outstanding for a logical graphics surface. That HDC uses
the same monotonic namespace but records the surface ID, surface COM object,
guest thread, geometry, and color format as an explicitly typed target. It does
not AddRef the surface. A live HDC instead blocks the corresponding COM
retirement plan until exact `ReleaseDC(surface, hdc)` succeeds. Wrong-surface,
wrong-thread, unknown, and repeated releases do not retire another acquisition.

`BLACK_BRUSH` is a process-shared object in namespace
`vine.logical-gdi-object.v1`. Repeated `GetStockObject(4)` returns the same
logical object. Stock objects are not owned by an HDC and remain live until
process teardown.

Each successful reached `CreateFontA` call creates a distinct, nonshared object
in the same typed GDI-object namespace. `RuntimeGdi` owns the monotonic object
ID and handle allocators and the exact 60-byte `LOGFONTA` semantic state. It
does not resolve that request to a native or browser font. Object selection,
introspection, and deletion use the same GDI owner: `GetObjectA` returns the
exact 60-byte state, `SelectObject` changes only a live same-thread memory DC,
and deletion succeeds only after the object has been restored out of every DC.

Each successful reached `CreateCompatibleDC(NULL)` call creates a distinct
same-thread memory HDC. The first call also publishes the process-shared
`SYSTEM_FONT` index 13 and 1 x 1 monochrome `DEFAULT_BITMAP` index 21 through
the same GDI-object allocator; later calls reuse those exact object identities.
The memory DC stores their selected object IDs and a zero selection generation.
It has a non-owning relation to the current primary monitor rectangle. Creating
the DC itself allocates no pixel storage, native HDC, Canvas context, or browser
renderer resource; pixel bytes appear only when a DIB object is created.

The reached `CreateDIBSection` variant creates one distinct nonshared GDI object
for a top-down 128 x 96, 8-bpp `BI_RGB` bitmap. `RuntimeGdi` owns its descriptor,
256-entry reverse-grayscale RGBQUAD palette, object identity, and backing
relation. `GuestMemory` owns the 12,288 zero-initialized pixel bytes, and
`ProcessAddressSpaceAllocator` owns the explicit-release allocation in the
graphics resource arena. The observed address is evidence, not a policy value
or stable ABI promise. `RuntimeGdi` also owns the DIB content generation; it
does not duplicate the pixel bytes.

Creation preflights the HDC relation, policy descriptor, GDI object identity,
address allocation, memory owner and mapping, zero-filled backing, and writable
bits-output pointer before publishing any of them. `GetObjectA(dib, 24)` returns
the exact Win32 `BITMAP` shape and the same guest bits pointer. `SelectObject`
returns the previous logical handle and advances the memory-DC selection
generation. The primary sequence selects the DIB and created font, then restores
the shared default bitmap and font before deletion.

`DeleteObject` retires only a live nonshared font or DIB that is not selected in
any DC. DIB retirement releases the GDI identity, guest-memory region, memory
owner, and address allocation as one preflighted transaction. Native Windows
accepts deletion of a selected bitmap with deferred lifetime, but Vine
deliberately rejects that variant without mutation; the primary caller restores
the object first. Stock objects, HDC values, wrong-type handles, stale objects,
and repeated deletion cannot retire another object.

`DeleteDC` retires only a live same-thread memory HDC. Unknown, repeated,
wrong-thread, and non-memory handles return FALSE while preserving LastError
and cannot retire another DC or either shared default object. `ReleaseDC` does
not delete memory DCs. Process teardown retires the remaining HDCs and stock
objects through the existing GDI owner boundary.

## Surface mutation and failure ordering

The reached surface is an 800 by 600 logical surface. `FillRect` currently
accepts a live acquisition HDC, an in-bounds RECT, and the logical
`BLACK_BRUSH`. A nonempty fill appends one bounded logical operation containing
the clipped-independent RECT, brush identity, and COLORREF. This operation is
authoritative semantic state, not a browser pixel buffer.

All capacity, handle, owner-thread, window relation, rectangle, fill-sequence,
owner-generation, and surface-generation checks occur while building an
opaque plan. The next surface generation is computed before commit. Commit
first rejects stale state and then performs only infallible targeted mutation;
invalid, stale, exhausted, or unmapped inputs cannot append a fill, consume an
identity, retire an HDC, or partially change guest memory.

The reached window resize is a bounded cross-subsystem operation. After the
matching `WM_NCCALCSIZE` default procedure completes, `RuntimeKernel`
preflights the pending `RuntimeUser32` geometry operation and a
`RuntimeGdi` window-surface resize plan before committing either owner. An
absent surface is a mutation-free no-op. An existing 800 by 600 surface keeps
its logical identity, becomes 1280 by 720, clears its bounded fill history for
the reached `NOCOPYBITS` behavior, and advances its surface and GDI
generations. A live acquisition HDC, stale plan, wrong source geometry, or
identity mismatch rejects both the USER32 and GDI changes before mutation.
General resize, content preservation, and browser renderer resizing are not
implied.

For the reached D3D9 surface, `FillRect` validates the live HDC and stock brush
in `RuntimeGdi`, then validates the live COM association, `1024x1024` bounds,
16,384-command capacity, sequence, and generations in `RuntimeGraphics`.
Both plans are preflighted before the graphics owner appends `[0, 0, 800, 600]`
with COLORREF `0`. GDI state is not a second copy of that fill. Failure cannot
append a command, advance either owner, consume an identity, or change guest
memory.

A screen HDC has no drawing surface in this checkpoint. Passing it to the
current `FillRect` path produces the typed `screen-dc-drawing-unsupported`
failure before mutation.

A DIB backing is byte storage with one bounded raster contract, not a general
GDI surface. The reached `TextOutA(hdc, 0, 0, "a", 1)` path revalidates the
same-thread memory HDC, selected FontA and DIB identities, policy profile,
content generation, and the writable backing range. The kernel then preflights
the expected old bytes before committing one 4,384-byte span to `GuestMemory`
and advancing the DIB content generation from zero to one. The exact mask
changes 476 bytes from palette index 0 to 255. Stale selection, stale content,
unwritable or changed backing, and unsupported text fail without changing GDI
state or guest bytes. Passing a memory HDC to `FillRect` still produces the
typed `memory-dc-drawing-unsupported` failure before mutation.

## Bounded font, memory DC, and DIB policy

`Gdi32CompatibilityPolicy` owns the reached catalog, metrics, and exact raster
mask, not `RuntimeGdi`, the browser, or a host font query. Version 7 accepts
only flags zero and the exact
Windows-932 `MS Gothic` face with `SHIFTJIS_CHARSET` on a live same-thread
screen HDC. Other `LOGFONTA` fields are deliberately ignored because the native
caller leaves them as unstable stack bytes. Unsupported face or charset,
nonzero flags, a stale or wrong-thread HDC, and a non-executable callback fail
before callback-frame or guest-memory mutation.

The policy materializes one zero-filled canonical 188-byte `ENUMLOGFONTEXA`
and 96-byte `NEWTEXTMETRICEXA` record in runtime callback scratch. The x86
stdcall callback receives those two pointers, `TRUETYPE_FONTTYPE`, and the
caller's unchanged LPARAM. Its 32-bit result becomes the outer enumeration
result; the reached callback returns zero, so enumeration stops after the first
record and preserves LastError. Native bytes after the first NUL in fixed ANSI
fields are uninitialized and are not imported. Only the NUL-bounded face, full
name, style, script, numeric LOGFONT fields, text metrics, and font signature
are compatibility evidence.

The enumeration record is logical catalog and metric data only. Separately,
the creation catalog accepts two exact reached requests. Both use zero
escapement and orientation, weight 100, no italic／underline／strikeout,
charset 128, output precision 4, clip precision 0, quality 2, pitch-family 0,
and the same NUL-bounded Windows-932 face; their height／width pairs are 64／32
and 224／112. The handler reads at most 32 bytes, requires the terminating NUL,
preflights the full request and owner capacity, then commits one `RuntimeGdi`
font object. Success returns the logical handle, writes no guest bytes, and
preserves LastError. The 224 x 112 object remains logical state only; it does
not extend the 64 x 32 outline-metric or text-raster contracts.

NULL, unterminated, cross-page, embedded-NUL policy mismatch, unsupported field
values, exhausted identity, and stale or duplicate plans fail without consuming
an object ID or handle and without changing GDI or guest-memory state. Creation
allocates no glyph outline, raster surface, browser `FontFace`, Canvas text
state, or renderer resource. Object selection, `GetObjectA`, deletion, outline
metrics, and the exact raster are independent bounded contracts. General text
measurement, glyph rasterization, and font substitution are not implied.

The compatible-DC profile independently admits only source HDC zero. The
aggregate resolves and revalidates the Rust-owned primary display before the
GDI owner plans one HDC plus any missing default objects. Capacity for the DC
and both initial objects is preflighted before one targeted commit. A stale or
duplicate plan, invalid display relation, non-NULL source, identity exhaustion,
or object-index conflict changes neither collection nor allocator. The policy
does not authorize a host-compatible DC or renderer state.

The independent DIB profile admits only the reached 40-byte `BITMAPINFOHEADER`:
width 128, height -96, one plane, 8 bits per pixel, `BI_RGB`, 256 colors, usage
`DIB_RGB_COLORS`, NULL section, and zero offset. The palette must be the exact
reverse grayscale table. The runtime derives stride and backing length with
checked arithmetic. Other dimensions, bit depths, compression, palette shapes,
file-mapping sections, deleted HDCs, and non-memory HDCs stop before allocation
or output mutation.

The text profile admits only origin `(0, 0)`, one Windows-932 byte `0x61`, the
selected 64 x 32 created font, and the selected 128 x 96 top-down 8-bpp DIB.
It stores 35 rows of a 32-pixel monochrome mask, beginning at logical row 20,
with expected whole-plane FNV-1a transition `0x52965dc5 -> 0xe6fc8461`.
Zero-length text is a mutation-free success. No host font lookup, native HDC,
browser font rasterization, Canvas text state, or browser bitmap resource is
consulted or created.

## Direct3D and renderer separation

GDI, D3D8, D3D9, and any future D3D10/11 support remain separate guest-facing
frontends. They do not share handles, COM identities, version-specific state,
device-loss rules, or resource lifetime. The WineD3D-like seam is below those
frontends: validated logical resources and commands may later target a shared
renderer adapter, but frontend authority is not merged to obtain that reuse.

`RuntimeGraphics` is introduced at the reached Direct3D `CreateDevice` subsystem
gateway and owns the later texture, surface, and surface-fill records. The GDI
frontend neither constructs a placeholder Direct3D device nor duplicates those
records. It also creates no Canvas/WebGL/WebGPU resource or browser renderer
identity. If GDI later presents pixels through a shared browser command
protocol, that protocol remains a non-owning adapter to Rust logical state and
uses a browser-resource namespace distinct from HWND, HDC, GDI object, COM, and
Direct3D resource identities.

## Evidence and verification

Two independent native captures fix the successful call order, caller
consumption, LastError preservation, client RECT, and negative errors. A
separate acquisition oracle establishes distinct concurrent HDCs and rejects
numeric post-release reuse as a contract. Artifact identities and the detailed
oracle are recorded in
[windows-native-evidence.md](windows-native-evidence.md); raw traces remain
ignored under `.work/`.

Rust model tests cover distinct window and screen acquisition identity, typed
screen serialization, same-thread and monitor relations, shared surface and
stock object lifetime, stale commit, capacity and generation failure, empty and
invalid rectangles, graphics-surface HDC uniqueness, COM-retirement guarding,
cross-owner fill preflight, bounded window-surface resize identity and content
reset, live-HDC rejection, and process teardown. The dispatcher integration
tests cover nullable HWND ABI, LastError, guest pointer preflight, repeated
screen release, policy/display authority, and failure atomicity. GDI32
dispatcher tests additionally cover the exact 284-byte callback payload,
callback-result propagation, stale／cross-thread HDCs, unsupported filters and
flags, callback preparation failure, and allocator／memory preservation. The
same suite covers both exact 14-word `CreateFontA` variants, distinct logical object
identity, exact LOGFONTA state, bounded string reads, policy-disabled and
malformed variants, stale plans, and allocator／memory preservation. Compatible
DC tests cover the one-word create／delete ABIs, distinct HDC identity, shared
default-object identity, exact serialized memory-DC state, two-object capacity
preflight, stale plans, policy-disabled／non-NULL source, same-thread deletion,
repeat／unknown／wrong-target deletion, LastError preservation, and zero guest
writes. DIB tests cover the six-word create ABI, exact descriptor and palette,
allocation and guest-output atomicity, zero backing, exact 24-byte `BITMAP`,
font／bitmap select and restore order, selected-object rejection, release of all
backing identities, stale output preservation, malformed and unreached
variants, and wrong／deleted HDC rejection.
Outline-metrics tests cover size queries, partial and complete output, selected
font validation, native padding canonicalization, and write failure atomicity.
Text-raster tests cover the nullable counted Windows-932 ABI, exact native mask,
zero-length no-op, stale selection and content generations, changed or
unwritable backing, and the combined memory／GDI transaction.
The private Node and Chrome runners compare the same immutable GDI completion
snapshots and stop at the next owned endpoint. Browser TypeScript observes the
snapshots but owns no Windows or GDI state.

The USER32 integration suite additionally fixes the reached resize sequence
`WM_WINDOWPOSCHANGING -> WM_NCCALCSIZE -> WM_SIZE ->
WM_WINDOWPOSCHANGED`, verifies that geometry and surface state remain old
through NCCALCSIZE dispatch and commit together before WM_SIZE, and proves that
a live window HDC leaves both owners unchanged.

Representative commands are:

```powershell
cargo test -p vine-core --lib runtime::gdi::tests --locked
cargo test -p vine-core --lib runtime::graphics::tests --locked
cargo test -p vine-core --test user32_dispatcher window_dc --locked
cargo test -p vine-core --test user32_dispatcher screen_dc --locked
cargo test -p vine-core --test gdi32_dispatcher --locked
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:runtime:bgi-browser
```

## Non-claims

- Drawing through the primary screen HDC, non-primary or multiple monitors,
  child or multiple windows, cross-thread DC use, class/private/cached DC
  policy, compatible DCs from non-NULL sources, SaveDC/RestoreDC, object types
  outside the bounded font／DIB lifecycle, clipping, mapping modes, other DIB
  dimensions／formats／palettes, file-mapped DIBs, other text, glyphs, fonts,
  positions, colors, background modes, alignment, clipping or raster formats,
  BitBlt, StretchBlt, and general GDI objects are not implemented.
- Resize variants other than the reached 800 x 600 to 1280 x 720 client
  transition, copy/preserve/stretch behavior, resizing with a live HDC, and a
  browser renderer resource are not implemented.
- The single enumerated record is not general system-font discovery and does
  not claim that a browser or host has the corresponding font installed.
- Neither logical fill history is a raster buffer; the reached D3D9 fill does
  not allocate pixel storage or produce a browser-visible frame.
- Native numeric HDC values and their reuse are artifact identity, not a Vine
  compatibility promise.
- No Direct3D device, general WineD3D implementation, D3D8, D3D10/11, DXGI, or
  general GDI compatibility is claimed.
