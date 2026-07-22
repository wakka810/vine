# Bounded D3D9 frontend contract

## Purpose and current boundary

The D3D9 frontend owns D3D9 ABI policy and uses the shared Rust COM foundation
for guest-visible identity and lifetime. `Direct3DCreate9(32)` creates a bounded
root object with canonical IUnknown identity and a 17-slot `IDirect3D9`
interface. A different SDK version returns NULL before memory, identity,
allocator, or generation mutation. QueryInterface, AddRef, Release, and final
backing retirement use the generic COM plan／commit path.
`GetAdapterCount` at slot 4, `GetAdapterIdentifier` at slot 5,
`GetAdapterDisplayMode` at slot 8,
`CheckDeviceType` at slot 9, `CheckDeviceFormat` at slot 10, `GetDeviceCaps`
at slot 14, `GetAdapterMonitor` at slot 15, and `CreateDevice` at slot 16
delegate through a version-neutral COM gateway to the D3D9 semantic handler.
`CreateDevice` publishes a complete 119-slot `IDirect3DDevice9` interface.
Its IUnknown methods, slot 23 `CreateTexture`, and slot 26
`CreateVertexBuffer` delegate to implemented handlers. `CreateTexture`
publishes a complete 22-slot `IDirect3DTexture9`
interface whose IUnknown methods and slot 18 `GetSurfaceLevel` delegate to
implemented handlers. `GetSurfaceLevel(0)` publishes a complete 17-slot
`IDirect3DSurface9` interface whose IUnknown methods plus slot 15 `GetDC` and
slot 16 `ReleaseDC` delegate to implemented handlers. `CreateVertexBuffer`
publishes a complete 14-slot `IDirect3DVertexBuffer9` interface whose IUnknown
methods plus slot 11 `Lock` and slot 12 `Unlock` delegate to implemented
handlers. Other
unreached slots return a typed fault with interface, slot, method, endpoint, and
call site.
The catalog fixes each x86 method name, explicit stack-word count, and return
class. In particular, slots 11, 21, and 22 are `void`, while slot 80 returns a
32-bit floating value through x87 `ST(0)`; those methods remain unsupported and
therefore do not yet require a completion transport for those return classes.

The two-page COM backing is Rust-owned guest memory. Its data page finishes
read-only and its endpoint page finishes read／execute. The endpoint bytes are
an execution marker for the replaceable CPU adapter; they are not a title
callsite key or a second interface registry. Rust resolves marker addresses
against the live COM endpoint index before dispatch.

The Runtime Worker queries Rust when the CPU reaches an otherwise unknown
`#UD`. Only a live Rust-owned endpoint may enter COM dispatch. TypeScript then
applies the versioned memory delta and CPU resume; it does not retain COM
objects, pointers, vtables, reference counts, or Windows device state. The Node
semantic runner uses the same endpoint-resolution, delta, and completion／fault
relations for the private workload.

## Bounded logical adapter count

Version 14 of `D3d9CompatibilityPolicy` retains profile
`single-logical-adapter-v1`. `IDirect3D9::GetAdapterCount` returns unsigned
value `1` after validating that `RuntimeDisplays` still exposes the coherent
`single-monitor-1920x1080-v1` profile. The count is D3D9 policy authority; the
display owner is consulted only to reject missing or mismatched topology before
completion. The query preserves LastError, COM identity and reference count,
guest memory, runtime generation, `RuntimeGraphics`, and browser resources.

The existing selected-vtable native trace contains six direct BGI calls from
RVA `0x59D36`; all return `1` with observed LastError `0`. Immutable caller
inspection shows the result stored in EDI and used as the exclusive upper bound
for adapter indices. Each iteration calls slot 15 `GetAdapterMonitor(index)`
and compares its HMONITOR with the earlier `MonitorFromWindow` result. Vine
therefore commits to one logical adapter, not the host GPU count. Multi-adapter
enumeration and host or browser GPU topology remain outside this profile.

## Bounded adapter-to-monitor mapping

The version 14 `primary-adapter-monitor-v1` profile maps adapter `0` to the
logical primary HMONITOR owned by `RuntimeDisplays`. The D3D9 policy owns only
the adapter-to-display-profile mapping; it neither allocates nor copies the
monitor identity. `GetAdapterMonitor(0)` therefore returns `0x4f010001`, the
same logical identity returned by reached `MonitorFromWindow`, while adapter
`1` or `UINT_MAX` returns NULL as a normal completion.

Success and invalid-adapter completion preserve LastError, guest memory, COM
reference count, object lifetime, runtime generation, and browser resources.
A disabled or malformed policy, missing primary monitor, or display-profile
mismatch produces a typed fault before mutation. The contract does not import
the native opaque HMONITOR value, host monitor topology, or browser display
topology.

The selected native trace contains six direct BGI calls from RVA `0x59D5E`,
all with adapter `0`, a non-NULL stable result, and observed LastError `0`. A
separate COFF-i386 oracle fixes the boundary cases: adapter `0` returns the
same primary identity, adapters `1` and `UINT_MAX` return NULL, and sentinel
LastError `0x5a5a` is preserved for all three. Raw trace and oracle binaries
remain ignored under `.work/`; only their identities and semantic relations
are recorded in the current manifest.

## Bounded adapter display mode

Version 14 of `D3d9CompatibilityPolicy` describes one coherent logical adapter.
Its display-mode profile is `primary-adapter-1920x1080-60-x8r8g8b8-v1`.
Width and height are derived from the primary monitor owned by
`RuntimeDisplays`; refresh rate `60` and format `22` (`D3DFMT_X8R8G8B8`)
remain D3D9 compatibility policy. The handler writes one packed 16-byte
`D3DDISPLAYMODE` value:

```text
Width        1920
Height       1080
RefreshRate  60
Format       22
bytes        80070000380400003c00000016000000
```

An invalid adapter or NULL output returns `D3DERR_INVALIDCALL` without mutation.
Disabled／malformed policy, missing or mismatched logical display authority, and
unwritable output produce typed faults before guest bytes or runtime state are
changed. Success preserves LastError and COM reference count. Output preflight
precedes one guest-memory transaction, so a partial structure is never
published.

Two independent Windows trace v2 captures each observed four successful calls
with adapter `0` and the same mode. The first call changed a zeroed 16-byte
structure completely; direct BGI callers were RVA `0x5B8D6` once and
`0x59DA5` three times. Caller inspection confirms the HRESULT sign is consumed
and subsequent code reads all four fields. Raw captures remain ignored under
`.work/`; their hashes and semantic summary are recorded in
[windows-native-evidence.md](windows-native-evidence.md) and the current
checkpoint manifest.

## Bounded virtual adapter identity

The same version 14 policy enables adapter `0`, flags `0`, and profile
`primary-virtual-adapter-identifier-v1`. It serializes one zero-filled,
1,100-byte `D3DADAPTER_IDENTIFIER9` from policy-owned virtual identity:

```text
Driver            vine-d3d9.dll
Description       Vine Virtual Direct3D9 Adapter
DeviceName        \\.\DISPLAY1
DriverVersion     0x0001000000000000
Vendor/Device/... 0
DeviceIdentifier  56494e45-4433-4439-8000-000000000001
WHQLLevel         0
```

This is a stable guest compatibility identity, not the host browser GPU. It is
linked to the same `RuntimeDisplays` primary profile as the display mode, while
all identity fields remain owned by `D3d9CompatibilityPolicy`. Invalid adapter,
unknown flags, or NULL output returns `D3DERR_INVALIDCALL` without mutation.
`D3DENUM_WHQL_LEVEL` is a typed unsupported variant because satisfying it may
require host certificate discovery. Disabled／malformed policy, mismatched
display authority, and unwritable output fault before guest bytes, reference
count, or runtime state change. Success preserves LastError and publishes the
entire structure through one guest-memory transaction.

Two independent native captures each observed one successful call from RVA
`0x5B91C`, with adapter `0`, flags `0`, and a zeroed input structure. Native
host identity changed 169 bytes, but the caller checked only the HRESULT sign
and continued to `GetDeviceCaps`; no identifier field was consumed. Therefore
the native host GPU values are artifact evidence, not a compatibility golden.
The stable Vine identity above is fixed by model tests and the public PE ABI
fixture. Raw native captures remain ignored under `.work/`.

## Bounded virtual device capabilities

Version 14 also enables adapter `0`, `D3DDEVTYPE_HAL`, and profile
`primary-hal-readscanline-dynamic-textures-ps3-v1`. The handler zero-fills one
304-byte, 4-byte-aligned x86 `D3DCAPS9` and publishes only these nonzero words:

```text
DeviceType                1          D3DDEVTYPE_HAL
Caps                      0x00020000 D3DCAPS_READ_SCANLINE
Caps2                     0x20000000 D3DCAPS2_DYNAMICTEXTURES
PixelShaderVersion        0xffff0300 D3DPS_VERSION(3, 0)
NumberOfAdaptersInGroup   1
```

`AdapterOrdinal` and every other field are zero. This is a versioned virtual
capability commitment for the reached primary path, not a copy of host GPU or
browser caps. It selects the minimum device／translation band that the later
`CreateDevice` gateway must either implement or reject explicitly; it does not
make shader translation or renderer state exist at this root query.

Two independent native captures each observed one successful call from RVA
`0x5B949`, with adapter `0`, device type `1`, and the same 304-byte output hash.
Clang record layout fixed the structure size at 304 bytes and every field
offset. The output pointer equals the caller ESP after return. The bounded
caller consumes the HRESULT sign, `Caps` at offset 8, `Caps2` at offset 12, and
`PixelShaderVersion` at offset 204. Native values for those fields were
`0x00020000`, `0xe0010000`, and `0xffff0300`; Vine retains only the reached
`READ_SCANLINE` and `DYNAMICTEXTURES` bits instead of importing the remaining
host flags.

Invalid adapter, device type, or NULL output returns `D3DERR_INVALIDCALL`
without mutation. Disabled／malformed policy, mismatched display authority,
and unwritable output fault before guest bytes, reference count, or runtime
state change. Success preserves LastError and commits the full structure in one
guest-memory transaction.

## Bounded device-type predicate

Version 14 retains profile `primary-hal-x8r8g8b8-windowed-fullscreen-v1` for
`IDirect3D9::CheckDeviceType`. It accepts adapter `0`, `D3DDEVTYPE_HAL`,
`D3DFMT_X8R8G8B8` adapter／back-buffer formats, and both windowed and
full-screen mode. The x86 `BOOL` follows the Windows convention: zero means
full-screen and any nonzero value means windowed. This predicate is owned by
the Rust compatibility policy and checked against the same `RuntimeDisplays`
profile as the display mode, identifier, and caps queries.

The primary caller performs two direct calls. RVA `0x5B9D6` passes Windowed
`1`; RVA `0x5BA1D` passes `0`. Two independent native traces report `S_OK` for
both, followed by a sign test of each HRESULT. The same traces contain three
additional successful windowed calls from inside `d3d9.dll`; the manifest
distinguishes those parented implementation calls from the two BGI calls.

The Microsoft D3D9 contract distinguishes invalid adapter／nonexistent device
identity from an unavailable format band. Vine therefore returns
`D3DERR_INVALIDCALL` (`0x8876086c`) for another adapter or device type and
`D3DERR_NOTAVAILABLE` (`0x8876086a`) for other formats. Disabled／malformed
policy or mismatched display authority produces a typed fault before
completion. Every path preserves LastError, COM reference count, guest memory,
`RuntimeGraphics`, and browser renderer resources.

## Bounded device-format predicate

Version 14 retains profile `primary-hal-x8r8g8b8-texture-basic-dynamic-v1` for
`IDirect3D9::CheckDeviceFormat`. It accepts adapter `0`, `D3DDEVTYPE_HAL`,
adapter／checked format `D3DFMT_X8R8G8B8`, resource type `D3DRTYPE_TEXTURE`,
and either Usage `0` or `D3DUSAGE_DYNAMIC` (`0x200`). The predicate is
policy-owned and checked against the same `RuntimeDisplays` profile as the
other root queries.

Two independent native traces each contain 13 calls. Six basic-usage format
`22` calls and four dynamic-usage format `22` calls return `S_OK`; the latter
include the one BGI direct call from RVA `0x5BA69`. Three `CreateDevice`-
parented basic-usage checks for format `118` return `D3DERR_NOTAVAILABLE`.
The remaining nine non-parented internal calls come from `d3d9.dll`. The BGI
caller tests the HRESULT sign immediately and, on success, proceeds to
`LoadLibraryA("d3dx9_43.dll")`.

The Microsoft contract returns `D3DERR_INVALIDCALL` for an out-of-range adapter
or unsupported device type and `D3DERR_NOTAVAILABLE` when the format is not
acceptable for the requested usage. Vine preserves that distinction: another
adapter／device type is invalid, while any format, usage, or resource-type
combination outside the bounded profile is unavailable. All paths preserve
LastError, COM reference count, guest memory, `RuntimeGraphics`, and browser
renderer resources.

## Bounded logical device creation

Version 14 retains `primary-windowed-hal-device-v1` for the reached
`IDirect3D9::CreateDevice` call. It accepts adapter `0`, `D3DDEVTYPE_HAL`,
behavior flags `D3DCREATE_MULTITHREADED | D3DCREATE_HARDWARE_VERTEXPROCESSING`
(`0x44`), one live focus／device HWND, and one 56-byte x86
`D3DPRESENT_PARAMETERS` with this bounded shape:

```text
BackBufferWidth/Height/Format  0 / 0 / D3DFMT_UNKNOWN
BackBufferCount               1
MultiSampleType/Quality       NONE / 0
SwapEffect                    DISCARD
Windowed                      TRUE
EnableAutoDepthStencil        FALSE
AutoDepthStencilFormat        D3DFMT_D16
Flags/refresh/interval        0 / 0 / 0
```

The zero width and height resolve from the live `RuntimeUser32` client geometry;
the zero format resolves from policy to `D3DFMT_X8R8G8B8`. For the primary
window this atomically publishes the 12-byte prefix `800, 600, 22`, the new
device interface pointer, the child COM identity, and one `RuntimeGraphics`-
owned logical device. Preflight covers both guest write spans, window／display
authority, graphics capacity, COM backing, and address allocation before any
publication. Invalid identity or overlapping／NULL pointers return
`D3DERR_INVALIDCALL`; a well-formed request outside the bounded policy returns
`D3DERR_NOTAVAILABLE`; memory or resource exhaustion faults before mutation.

The logical graphics device owns the normalized presentation, window relation,
and COM-object association. It has its own monotonic ID namespace and no browser
renderer association yet. The child COM object starts at reference count `1`
and retains one reference to the creating root interface. Its final `Release`
retires the logical graphics association and device backing, then releases the
retained root reference; if that was the root's last reference, root retirement
cascades in the same planned operation.

Two independent native traces each contain three successful device creations.
All begin with zero width／height／format and preserve the remaining presentation
fields; the returned native sizes are `800x600` once and `1280x720` twice, with
format `22`. The direct BGI call is RVA `0x5ACA5`, returns `S_OK`, and its first
reached device operation is slot 23 `CreateTexture`; a nested device `AddRef`
returns `2`. Native pointers and host geometry are artifact evidence, not Vine
guest identities.

## Bounded logical texture creation

Version 14 retains the three-variant texture policy with profile
`primary-initial-x8r8g8b8-textures-v1`. Every accepted variant has width and
height `1024`, one mip level, `D3DFMT_X8R8G8B8`, and a NULL shared-handle
pointer. The bounded allow-list is:

| Usage | Pool | Logical usage／pool |
|---:|---:|---|
| `0` | `D3DPOOL_SYSTEMMEM` (`2`) | static／system memory |
| `0` | `D3DPOOL_DEFAULT` (`0`) | static／default |
| `D3DUSAGE_DYNAMIC` (`0x200`) | `D3DPOOL_DEFAULT` (`0`) | dynamic／default |

Width／height zero, NULL output, or a non-NULL shared handle returns
`D3DERR_INVALIDCALL`; a well-formed shape outside this exact allow-list returns
`D3DERR_NOTAVAILABLE`. The list is compatibility policy, not a hidden call-order
state machine. Reached order is checked by scenario evidence. An unwritable
output or capacity failure faults before guest bytes, COM identity, reference
counts, graphics indexes, allocators, or generations change. LastError is
preserved on every path.

Each success publishes one distinct interface pointer, child COM object, and
`RuntimeGraphics` logical texture descriptor only after guest memory, the
graphics plan, COM backing, parent identity, and process address allocation all
preflight. Texture IDs use one monotonic logical namespace with a 4,096-object
capacity. Each descriptor records the creating logical device relation,
dimensions, level count, usage class, color format, and pool. `DEFAULT` is a
logical D3D9 pool classification at this boundary; it does not allocate texel
bytes, browser GPU memory, or a renderer resource.

The 22-slot texture COM object starts at reference count `1` and retains one
reference to the creating device interface. Texture final `Release` retires its
logical graphics association and COM backing, then releases that device
reference. If the device and root have no other references, their COM backing
and logical device association retire in the same preflighted cascading
operation. `RuntimeGraphics` owns the non-owning device-to-texture index;
`RuntimeCom` remains the sole reference-count authority.

Two independent focused native traces each contain the same three successful
BGI calls from RVAs `0x5A715`, `0x5A7B7`, and `0x5A7DC`, matching the table
above. All return `S_OK`, use the same device and NULL shared handles, and
publish three non-NULL distinct native texture pointers. The first caller
consumes the HRESULT sign and immediately calls texture slot 18
`GetSurfaceLevel(0, output)`; after that surface DC chain, the next two callers
consume their HRESULT signs and proceed to device slot 26 `CreateVertexBuffer`.
Native pointers remain opaque artifact identity. Vine claims the exact shapes,
ordering, distinctness, and caller use, not broader resource behavior.

## Bounded initial vertex buffer

Version 14 adds profile `primary-initial-xyzw-diffuse-tex1-quad-v1` for device
slot 26. It accepts exactly a 112-byte, `D3DUSAGE_WRITEONLY`,
`D3DFVF_XYZRHW|D3DFVF_DIFFUSE|D3DFVF_TEX1`, `D3DPOOL_MANAGED` vertex buffer
with a NULL shared-handle pointer. Zero length, NULL output, or a non-NULL
shared handle returns `D3DERR_INVALIDCALL`; a well-formed descriptor outside
that band returns `D3DERR_NOTAVAILABLE`. LastError is preserved.

Creation preflights the guest output, COM child and parent reference, logical
graphics capacity, graphics-resource address reservation, and guest-memory
backing before mutation. Commit publishes one 14-slot
`IDirect3DVertexBuffer9`, one `RuntimeGraphics` descriptor and backing relation,
one process allocation record, and a zero-filled read-only guest range as a
single transaction. The vertex buffer starts at reference count `1`, retains
one device-interface reference, and uses its own 4,096-object logical namespace.
Final `Release` retires the COM backing, graphics-resource backing and owner,
address records, and logical association together before releasing the retained
device reference; device and root retirement may then cascade.

The reached slot 11 `Lock(0, 0, output, 0)` means a full-range lock. It
atomically publishes the backing address and changes that exact guest range to
read／write only after output writability, live identity, lock state, allocation
identity, owner, and protection all preflight. A duplicate lock or any other
offset, size, or flags returns `D3DERR_INVALIDCALL` without mutation. Rust
`GuestMemory` remains byte authority while the guest writes the 112-byte
payload; the browser mirror is execution state, not a second resource store.
Reached slot 12 `Unlock()` requires that active lock, returns the range to
read-only, clears the lock, and increments content generation in one
cross-owner commit. Unlock without a lock is failure-atomic.

Two focused native traces reproduce `CreateVertexBuffer`, `Lock`, and `Unlock`
from caller RVAs `0x5A890`, `0x5AA96`, and `0x5AAB2`. Creation and locking expose
112 zero bytes before the caller copies its 112-byte quad payload; all three
calls return `S_OK`, preserve observed LastError `0`, and lead directly to
device slot 57 `SetRenderState(22, 1)`. Native resource and data pointers remain
opaque artifact identity.

## Bounded initial render states

Version 14 adds profile `primary-initial-fixed-function-render-states-v1` for
device slot 57. The reached band is exactly the ordered startup cluster
`D3DRS_CULLMODE=D3DCULL_NONE`, `D3DRS_ZENABLE=FALSE`,
`D3DRS_LIGHTING=FALSE`, and `D3DRS_ALPHABLENDENABLE=TRUE`. A well-formed setting
outside that band returns `D3DERR_NOTAVAILABLE`; disabled policy returns a typed
unsupported fault. Both paths preserve LastError and all logical graphics state.

`RuntimeGraphicsDevice` owns the typed values. It does not manufacture
unreached Windows defaults: each field remains absent until the guest sets it.
The owner builds a mutation-free plan, preflights device and global generations,
and commits one targeted update. Stale plans, unknown devices, and generation
exhaustion fail before mutation. Reapplying the current value succeeds without
changing either generation. TypeScript owns no duplicate render-state table and
creates no renderer resource at this boundary.

Two focused native traces reproduce four contiguous slot-57 calls on one device
and thread from caller RVAs `0x5AD13`, `0x5AD26`, `0x5AD3C`, and `0x5AD50`.
Their state/value pairs are `22/1`, `7/0`, `137/0`, and `27/1`; every result is
`S_OK` with observed LastError `0`. Device and thread values are opaque artifact
identity. The private Node and Chrome sessions reproduce sequences
17,862–17,865 and stop next at slot 19 `GetRasterStatus` without guest writes or
browser renderer allocation.

## Bounded level-zero surface retrieval

Version 14 retains `primary-texture-level-zero-surface-v1` for the first reached
`IDirect3DTexture9::GetSurfaceLevel` call. It accepts only level `0` on the
one-level logical texture created from source variant index `0`, the static
system-memory descriptor, and a non-NULL writable output pointer. The two
default-pool variants stop with typed
`get-surface-level-texture-variant-unsupported` before output validation or
mutation. A missing output or any other level returns `D3DERR_INVALIDCALL`; an
unwritable output or resource exhaustion faults before guest bytes, COM
identity, reference counts, graphics indexes, allocators, or generations
change. LastError is preserved on every Vine path.

The first successful lookup publishes one surface interface pointer, one child
COM object, and one `RuntimeGraphics` logical surface descriptor after the guest
write, texture association, surface capacity, COM backing, parent identity, and
process address allocation all preflight. The descriptor records level `0`,
dimensions `1024x1024`, and format `D3DFMT_X8R8G8B8`; it does not allocate pixel
bytes, pitch, lock staging, a DC, or a browser renderer resource. Surface IDs
use a monotonic namespace with a 4,096-object capacity, and the owner-local
texture-to-surface index prevents duplicate logical children.

The 17-slot surface COM object starts at reference count `1` and retains one
reference to the texture interface. Repeating `GetSurfaceLevel(0)` returns the
same surface and canonical IUnknown identity while adding one reference; it
does not allocate another COM or graphics object. Surface final `Release`
retires its logical graphics association and COM backing, then releases the
retained texture reference. If no other references remain, texture, device, and
root retirement may cascade in the same preflighted operation. `RuntimeGraphics`
owns the non-owning texture-to-surface index; `RuntimeCom` remains the sole
reference-count authority.

Two independent focused native traces each contain one successful call from BGI
RVA `0x5A731` at level `0`. The caller consumes the HRESULT sign and immediately
calls surface slot 15 `GetDC`. A separate controlled i386 oracle fixes repeat
pointer and IUnknown identity, distinct texture／surface IUnknown identity, and
`GetContainer` returning the original texture. Native D3D9 zeroes a non-NULL
output for an invalid level and raises access violation for a NULL output; Vine
intentionally keeps the stronger repository-wide failure-atomicity contract and
leaves the output unchanged while returning `D3DERR_INVALIDCALL`.

## Bounded surface DC and GDI fill bridge

Version 14 retains `primary-surface-gdi-dc-v1` for surface slots 15 and 16.
`GetDC` accepts one non-NULL writable HDC output on a live logical surface.
Guest-memory access, the live COM-to-graphics association, the current surface
descriptor, GDI capacity, identity allocation, and the one-outstanding-HDC rule
are preflighted before publication. The four-byte guest write is held in a
memory transaction until `RuntimeGdi` commits the HDC; a stale cross-subsystem
plan rolls the write back. A NULL output or repeated acquisition returns
`D3DERR_INVALIDCALL` without changing the output or allocator. Other invalid
authority or memory conditions fault before mutation.

The HDC uses the monotonic `vine.logical-gdi-dc.v1` namespace and records a
non-owning relation to the exact logical graphics surface, COM object, current
guest thread, geometry, and color format. It does not retain a COM reference.
Instead, `RuntimeGdi` guards COM retirement while that HDC is live. `ReleaseDC`
requires the same surface object, HDC, and guest thread; an unknown, mismatched,
or already released HDC returns `D3DERR_INVALIDCALL`. Successful release retires
only the HDC and allows a later reacquisition. All completion paths preserve
LastError.

Reached `FillRect` remains the User32 frontend call, but dispatch now branches on
the typed HDC target. For a graphics-surface HDC the kernel revalidates the GDI
HDC and stock `BLACK_BRUSH`, then preflights the corresponding
`RuntimeGraphics` surface, rectangle, fill capacity, sequence, and generations.
Only after both owners pass does `RuntimeGraphics` append the bounded logical
fill command and advance surface and content generations. The history is capped
at 16,384 commands. The primary slice records `[0, 0, 800, 600]` with COLORREF
`0` on the `1024x1024` B8G8R8X8 surface. This command is authoritative semantic
state; no texel array, pitch, lock staging, upload, or renderer resource is
created.

Two focused native traces reproduce the exact ordered chain `GetDC`,
`GetStockObject(4)`, `FillRect`, `ReleaseDC`, and surface `Release`, with caller
RVAs `0x5A746`, `0x5A76C`, `0x5A77D`, `0x5A78E`, and `0x5A79A`. A controlled
COFF-i386 oracle fixes repeated acquisition, wrong and repeated release,
reacquisition, unchanged surface reference count, and the native
`LockRect`-while-DC rejection. Native NULL or read-only output raises access
violation; Vine retains the repository-wide checked-memory fault boundary.

## Versioned frontend and shared graphics boundary

D3D8, D3D9, and any future D3D10／11 support remain separate guest-facing
frontends. They do not share COM identity, vtables, version-specific enums, or
device-loss state. A WineD3D-like internal layer is introduced only below those
frontends: `RuntimeGraphics` accepts validated logical
device／resource／command contracts and coordinates the browser renderer
adapter. The D3D9 `CreateDevice` gateway now creates the first logical device;
the `CreateTexture` gateway creates the first device-related logical resource;
the `GetSurfaceLevel` gateway creates its first logical subresource identity;
and `CreateVertexBuffer` creates the first guest-backed logical geometry
resource.
The active initial-resource slice leaves three logical textures live on one
device: one system-memory static texture and two default-pool textures, the last
of which is dynamic. The surface `GetDC` gateway adds a typed GDI acquisition
relation, and reached
`FillRect` appends the first `RuntimeGraphics`-owned surface command.
The vertex-buffer `Lock`／`Unlock` pair exposes only its checked Rust-owned guest
backing and records lock and content generations; it does not allocate browser
GPU memory.
Root creation and adapter-query leaves still do not allocate placeholder
graphics state, and none of these gateways creates a renderer resource.

This keeps the translation seam available without building frameworks for
unreached or deferred APIs. D3D10／11 and DXGI remain outside active scope until
explicitly reopened.

## Representative verification

```powershell
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:toolchain
npm run check:runtime:browser-worker
npm run check:runtime:bgi -- --profile d3d9-initial-render-states
```

The controlled PE fixture verifies invalid-SDK NULL, non-NULL factory identity,
the four display-mode fields, a populated adapter-identifier ABI, the required
native or exact Vine device-capability band, AddRef 2, Release 1, and final
Release 0 with retirement. Its Windows self-oracle accepts native nonempty
NUL-terminated identity strings and required caps, while the Vine branch checks
the exact virtual identity and zero-bounded 304-byte caps. It also verifies the
windowed／full-screen device-type predicates and the reached dynamic texture
format predicate. Rust tests separately fix every output byte and the invalid
adapter／flags／device type／format／usage／resource type／NULL／unwritable／policy／
display-authority paths. They also fix the complete device vtable catalog,
presentation normalization, device／root and texture／device reference relations,
surface／texture reference and repeat-identity relations, device／texture／surface
capacity, vertex-buffer／device reference and backing relations, lock uniqueness,
address and memory-owner retirement, surface-HDC uniqueness and retirement guard, GDI／graphics fill
capacity, stale-plan rejection, cross-owner publication, and final cascading
retirement.
The fixture also verifies that adapter `0` has a
non-NULL monitor and the first past-end adapter has none. The private Node and
Chrome profiles record identical `CreateDevice`, all three `CreateTexture`,
`GetSurfaceLevel`, surface `GetDC`／`ReleaseDC`, stock-brush `FillRect`, and final
surface `Release`, `CreateVertexBuffer`, `Lock`, and `Unlock` completions; guest
writes; COM／HDC identities; logical device／texture／surface／vertex-buffer
descriptors, fill command, backing protection, and final guest memory.
The active profile additionally fixes four typed device render-state updates,
their idempotent same-value behavior, zero guest writes, and the following
`GetRasterStatus` stop without creating a browser renderer resource.

Historical D3D9 runners expose only the frontend methods enabled by their bounded
evidence policy. Thus `d3d9-get-adapter-count` stops at slot 15 and
`d3d9-get-adapter-monitor` stops at slot 16 through the common
`com-method-unsupported` path. The historical `d3d9-create-device` profile
delegates slot 16 and stops at slot 23, while the historical
`d3d9-create-texture` profile also delegates device slot 23. The historical
`d3d9-surface-get-dc` and `winmm-time-get-time` profiles additionally delegate
texture slot 18 and surface slots 15／16 while restricting texture policy to the
system-memory variant. The historical `d3d9-initial-textures` profile uses the
three-variant policy and stops at device slot 26. The historical
`d3d9-initial-vertex-buffer` profile delegates device slot 26 and vertex-buffer
slots 11／12 before slot 57. The active `d3d9-initial-render-states` profile uses
version 14, delegates the exact four slot-57 settings, and stops at device slot
19 `GetRasterStatus`. Profile selection does not add callsite branches to the
semantic handler or move Windows state into the browser runtime.

## Non-claims

- Only the bounded single logical adapter count, adapter-to-primary-monitor
  mapping, current display-mode, virtual adapter-identifier, virtual
  device-capability, exact device-type predicate, and exact texture-format
  predicate queries plus the exact windowed HAL `CreateDevice`, the three exact
  initial `CreateTexture` variants, level-zero `GetSurfaceLevel`, and one exact
  surface `GetDC`／black `FillRect`／`ReleaseDC` profile plus the exact initial
  `CreateVertexBuffer`／full-range `Lock`／`Unlock` sequence and the exact four
  initial `SetRenderState` settings are implemented. Mode
  enumeration, general device／format／usage checks, other levels,
  surface／texture storage, vertex declarations, other resource locks, other
  device/resource state, draw, Present, Reset, and renderer output remain
  unsupported.
- `CreateDevice`, `CreateTexture`, `GetSurfaceLevel`, the reached surface DC
  fill, and the vertex-buffer gateway create Rust logical state only. They do
  not create a browser renderer resource,
  Canvas／WebGL／WebGPU context, swap chain, or visible frame.
- D3D8, D3D10／11, DXGI, shader translation, and general D3D9 compatibility are
  not claimed.
