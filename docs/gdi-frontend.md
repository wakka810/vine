# GDI frontend and logical surface contract

## Purpose and current boundary

The GDI frontend owns guest-visible GDI identity, lifetime, and logical drawing
state in Rust. The current bounded slice implements `USER32!GetDC` and
`USER32!ReleaseDC` for a logical window, the NULL-window primary screen
acquisition, one policy-owned `GDI32!EnumFontFamiliesExA` callback record, and
one exact `GDI32!CreateFontA` logical object variant. It also implements the
NULL-source `GDI32!CreateCompatibleDC` memory-DC lifecycle and `GDI32!DeleteDC`,
plus the reached D3D9 system-memory surface. `GDI32!GetStockObject`
for `BLACK_BRUSH` and `USER32!FillRect` remain bounded to the reached window and
graphics-surface paths. This establishes the responsibility boundary needed for
later GDI work without claiming a pixel renderer, screen drawing, general DC
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
introspection, and deletion are separate later lifetime transitions and are not
implied by successful creation.

Each successful reached `CreateCompatibleDC(NULL)` call creates a distinct
same-thread memory HDC. The first call also publishes the process-shared
`SYSTEM_FONT` index 13 and 1 x 1 monochrome `DEFAULT_BITMAP` index 21 through
the same GDI-object allocator; later calls reuse those exact object identities.
The memory DC stores their selected object IDs and a zero selection generation.
It has a non-owning relation to the current primary monitor rectangle, but no
pixel storage, native HDC, Canvas context, or browser renderer resource.

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

The current surface geometry is fixed at creation. A later window resize is a
separate cross-subsystem design problem and currently produces an explicit
geometry conflict instead of silently resizing or replacing the surface.

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

A memory HDC also has no drawable surface until a selected DIB exists. Passing
it to the current `FillRect` path produces the typed
`memory-dc-drawing-unsupported` failure before mutation.

## Bounded font catalog, creation, and memory DC policy

`Gdi32CompatibilityPolicy` owns the reached catalog, not `RuntimeGdi`, the
browser, or a host font query. Version 3 accepts only flags zero and the exact
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
the creation profile accepts the exact reached request: height 64, width 32,
zero escapement and orientation, weight 100, no italic／underline／strikeout,
charset 128, output precision 4, clip precision 0, quality 2, pitch-family 0,
and the same NUL-bounded Windows-932 face. The handler reads at most 32 bytes,
requires the terminating NUL, preflights the full request and owner capacity,
then commits one `RuntimeGdi` font object. Success returns the logical handle,
writes no guest bytes, and preserves LastError.

NULL, unterminated, cross-page, embedded-NUL policy mismatch, unsupported field
values, exhausted identity, and stale or duplicate plans fail without consuming
an object ID or handle and without changing GDI or guest-memory state. Creation
allocates no glyph outline, raster surface, browser `FontFace`, Canvas text
state, or renderer resource. Object selection, `GetObjectA`, deletion, text
measurement, glyph rasterization, and font substitution remain subsequent
gateways.

The compatible-DC profile independently admits only source HDC zero. The
aggregate resolves and revalidates the Rust-owned primary display before the
GDI owner plans one HDC plus any missing default objects. Capacity for the DC
and both initial objects is preflighted before one targeted commit. A stale or
duplicate plan, invalid display relation, non-NULL source, identity exhaustion,
or object-index conflict changes neither collection nor allocator. The policy
does not authorize a host-compatible DC, DIB, bitmap bytes, or renderer state.

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
cross-owner fill preflight, and process teardown. The dispatcher integration
tests cover nullable HWND ABI, LastError, guest pointer preflight, repeated
screen release, policy/display authority, and failure atomicity. GDI32
dispatcher tests additionally cover the exact 284-byte callback payload,
callback-result propagation, stale／cross-thread HDCs, unsupported filters and
flags, callback preparation failure, and allocator／memory preservation. The
same suite covers the exact 14-word `CreateFontA` ABI, distinct logical object
identity, exact LOGFONTA state, bounded string reads, policy-disabled and
malformed variants, stale plans, and allocator／memory preservation. Compatible
DC tests cover the one-word create／delete ABIs, distinct HDC identity, shared
default-object identity, exact serialized memory-DC state, two-object capacity
preflight, stale plans, policy-disabled／non-NULL source, same-thread deletion,
repeat／unknown／wrong-target deletion, LastError preservation, and zero guest
writes.
The private Node and Chrome runners compare the same immutable GDI completion
snapshots and stop at the next owned endpoint. Browser TypeScript observes the
snapshots but owns no Windows or GDI state.

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
  policy, compatible DCs from non-NULL sources, SaveDC/RestoreDC, object
  selection／introspection／deletion, clipping, mapping modes, DIBs, palettes,
  text rendering, BitBlt, StretchBlt, and general GDI objects are not implemented.
- The single enumerated record is not general system-font discovery and does
  not claim that a browser or host has the corresponding font installed.
- Neither logical fill history is a raster buffer; the reached D3D9 fill does
  not allocate pixel storage or produce a browser-visible frame.
- Native numeric HDC values and their reuse are artifact identity, not a Vine
  compatibility promise.
- No Direct3D device, general WineD3D implementation, D3D8, D3D10/11, DXGI, or
  general GDI compatibility is claimed.
