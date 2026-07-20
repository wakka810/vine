# GDI frontend and logical surface contract

## Purpose and current boundary

The GDI frontend owns guest-visible GDI identity, lifetime, and logical drawing
state in Rust. The current bounded slice implements `USER32!GetDC`,
`GDI32!GetStockObject` for `BLACK_BRUSH`, `USER32!FillRect`, and
`USER32!ReleaseDC` for both a logical window and the reached D3D9 system-memory
surface. It establishes the responsibility boundary needed for later GDI work
without claiming a pixel renderer, general DC behavior, or a shared Direct3D
device.

`RuntimeGdi` is a private subsystem owned by `RuntimeKernel`. It owns the HDC,
GDI-object, and window-surface collections; their indexes and identity allocators;
bounded window fill history; and the subsystem generation. `RuntimeUser32` remains the
authority for windows and client geometry. The kernel passes GDI an opaque,
non-owning window relation only after revalidating the live HWND, owner thread,
and client geometry. GDI never owns or mutates the window record.

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

Rust model tests cover distinct acquisition identity, shared surface and stock
object lifetime, wrong relation, stale commit, capacity and generation failure,
empty and invalid rectangles, graphics-surface HDC uniqueness, COM-retirement
guarding, cross-owner fill preflight, and process teardown. The dispatcher
integration tests cover x86 ABI, LastError, guest pointer preflight, and failure
atomicity.
The private Node and Chrome runners compare the same immutable GDI completion
snapshots and stop at the next owned endpoint. Browser TypeScript observes the
snapshots but owns no Windows or GDI state.

Representative commands are:

```powershell
cargo test -p vine-core --lib runtime::gdi::tests --locked
cargo test -p vine-core --lib runtime::graphics::tests --locked
cargo test -p vine-core --test user32_dispatcher window_dc --locked
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:runtime:bgi-browser
```

## Non-claims

- Desktop/null-window DCs, child or multiple windows, cross-thread DC use,
  class/private/cached DC policy, SaveDC/RestoreDC, selection, clipping, mapping
  modes, DIBs, palettes, text, BitBlt, StretchBlt, and general GDI objects are
  not implemented.
- Neither logical fill history is a raster buffer; the reached D3D9 fill does
  not allocate pixel storage or produce a browser-visible frame.
- Native numeric HDC values and their reuse are artifact identity, not a Vine
  compatibility promise.
- No Direct3D device, general WineD3D implementation, D3D8, D3D10/11, DXGI, or
  general GDI compatibility is claimed.
