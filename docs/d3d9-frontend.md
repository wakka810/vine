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
Its IUnknown methods, slot 23 `CreateTexture`, slot 26 `CreateVertexBuffer`,
slot 31 `UpdateTexture`, slot 41 `BeginScene`, slot 43 `Clear`, and slot 69
`SetSamplerState` delegate to implemented handlers. `CreateTexture`
publishes a complete 22-slot `IDirect3DTexture9`
interface whose IUnknown methods, slot 18 `GetSurfaceLevel`, and slot 21
`AddDirtyRect` delegate to implemented handlers. `GetSurfaceLevel(0)` publishes a complete 17-slot
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

Version 21 of `D3d9CompatibilityPolicy` retains profile
`single-logical-adapter-v1`. `IDirect3D9::GetAdapterCount` returns unsigned
value `1` after validating that `RuntimeDisplays` still exposes the coherent
`single-monitor-1920x1080-work-1920x1032-v2` profile. The count is D3D9 policy authority; the
display owner is consulted only to reject missing or mismatched topology before
completion. The query preserves LastError, COM identity and reference count,
guest memory, runtime generation, `RuntimeGraphics`, and browser resources.
The profile's work rectangle is part of coherent display identity but does not
alter D3D9 display-mode geometry.

The existing selected-vtable native trace contains six direct BGI calls from
RVA `0x59D36`; all return `1` with observed LastError `0`. Immutable caller
inspection shows the result stored in EDI and used as the exclusive upper bound
for adapter indices. Each iteration calls slot 15 `GetAdapterMonitor(index)`
and compares its HMONITOR with the earlier `MonitorFromWindow` result. Vine
therefore commits to one logical adapter, not the host GPU count. Multi-adapter
enumeration and host or browser GPU topology remain outside this profile.

## Bounded adapter-to-monitor mapping

The version 21 `primary-adapter-monitor-v1` profile maps adapter `0` to the
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

Version 21 of `D3d9CompatibilityPolicy` describes one coherent logical adapter.
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

The same version 21 policy enables adapter `0`, flags `0`, and profile
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

Version 19 also enables adapter `0`, `D3DDEVTYPE_HAL`, and profile
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

Version 19 retains profile `primary-hal-x8r8g8b8-windowed-fullscreen-v1` for
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

Version 19 retains profile `primary-hal-x8r8g8b8-texture-basic-dynamic-v1` for
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

Version 19 retains `primary-windowed-hal-device-v1` for the reached
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

Version 19 retains the texture profile introduced in version 17,
`primary-initial-and-recreated-x8r8g8b8-textures-v2`. Every accepted variant
has height `1024`, one mip level, `D3DFMT_X8R8G8B8`, and a NULL shared-handle
pointer. Width is either the initial `1024` or the reached recreated `2048`;
each width admits the same bounded usage／pool band:

| Width × height | Usage | Pool | Logical usage／pool |
|---|---:|---:|---|
| `1024x1024`, `2048x1024` | `0` | `D3DPOOL_SYSTEMMEM` (`2`) | static／system memory |
| `1024x1024`, `2048x1024` | `0` | `D3DPOOL_DEFAULT` (`0`) | static／default |
| `1024x1024`, `2048x1024` | `D3DUSAGE_DYNAMIC` (`0x200`) | `D3DPOOL_DEFAULT` (`0`) | dynamic／default |

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

The initial focused evidence contains three successful `1024x1024` calls from
RVAs `0x5A715`, `0x5A7B7`, and `0x5A7DC`. A second pair of focused traces proves
the same caller sequence recreates the resources at `2048x1024`: all three calls
return `S_OK`, preserve observed LastError `0`, write four output bytes, and
publish non-NULL distinct native texture pointers. Each immediate consumer tests
the HRESULT sign. The first system-memory texture continues through
`GetSurfaceLevel -> GetDC -> GetStockObject -> FillRect -> ReleaseDC -> Release`;
the two default-pool calls then complete. Native pointers remain opaque artifact
identity. Vine claims the exact six descriptors, ordering, distinctness, and
caller use, not broader resource behavior.

## Bounded initial vertex buffer

Version 19 retains profile `primary-initial-xyzw-diffuse-tex1-quad-v1` for device
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

Version 19 retains profile `primary-initial-fixed-function-render-states-v1` for
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

## Bounded raster status

Version 19 retains `primary-stable-nonblank-scanline-v1` for device slot 19
`GetRasterStatus(0, output)`. The policy returns one fully initialized
`D3DRASTER_STATUS`: normalized `InVBlank=FALSE` and `ScanLine=1`. These values
are a deterministic compatibility sample, not imported host or browser scanout
state. Rust first resolves the live device COM object to its
`RuntimeGraphicsDevice`, verifies the primary display profile and swap-chain
index, preflights the complete eight-byte output span, and publishes one atomic
guest write. The call preserves LastError, reference counts, graphics state,
and all generations.

NULL output or a swap-chain index outside the device's bounded back-buffer
count returns `D3DERR_INVALIDCALL` without mutation. An unwritable output,
unknown／retired COM-to-graphics relation, inconsistent display authority, or
disabled policy faults before any guest byte changes. TypeScript owns no raster
counter or Windows status object.

Two focused native runs each capture the first slot-19 call from RVA `0x5AEDD`.
One observes nonblank scanline `1200`; the other observes VBlank with scanline
`0`. The immediate consumer checks HRESULT, branches on `InVBlank`, and consumes
the scanline; its outer caller accepts either success or the VBlank sentinel,
repeats sampling, and yields with `Sleep(0)`. Thus the changing native values
are host artifact observations, while the consumer contract admits Vine's
stable nonblank sample. Node and Chrome both complete call 17,866 at instruction
11,808,586 with output `0000000001000000`, then stop 40 instructions later at
`kernel32.dll!QueryPerformanceFrequency`.

## Bounded level-zero surface retrieval

Version 19 retains `primary-system-memory-texture-level-zero-surfaces-v2` for the
reached `IDirect3DTexture9::GetSurfaceLevel` calls. It accepts only level `0` on
one-level logical textures created from source variant indices `0` and `3`, the
`1024x1024` and `2048x1024` static system-memory descriptors, with a non-NULL
writable output pointer. The default-pool variants stop with typed
`get-surface-level-texture-variant-unsupported` before output validation or
mutation. A missing output or any other level returns `D3DERR_INVALIDCALL`; an
unwritable output or resource exhaustion faults before guest bytes, COM
identity, reference counts, graphics indexes, allocators, or generations
change. LastError is preserved on every Vine path.

The first successful lookup publishes one surface interface pointer, one child
COM object, and one `RuntimeGraphics` logical surface descriptor after the guest
write, texture association, surface capacity, COM backing, parent identity, and
process address allocation all preflight. The descriptor records level `0`,
source-texture dimensions, and format `D3DFMT_X8R8G8B8`; it does not allocate pixel
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

Version 19 retains `primary-surface-gdi-dc-v1` for surface slots 15 and 16.
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
created. The recreated slice applies the same bridge to a `2048x1024` surface
and records `[0, 0, 1280, 720]` before releasing its HDC and surface identity.

Two focused native traces reproduce the exact ordered chain `GetDC`,
`GetStockObject(4)`, `FillRect`, `ReleaseDC`, and surface `Release`, with caller
RVAs `0x5A746`, `0x5A76C`, `0x5A77D`, `0x5A78E`, and `0x5A79A`. A controlled
COFF-i386 oracle fixes repeated acquisition, wrong and repeated release,
reacquisition, unchanged surface reference count, and the native
`LockRect`-while-DC rejection. Native NULL or read-only output raises access
violation; Vine retains the repository-wide checked-memory fault boundary.

## Bounded full-target Clear

Version 19 adds `primary-full-render-target-black-v1` for device slot 43. The
accepted call is exactly `Clear(0, NULL, D3DCLEAR_TARGET, 0xFF000000, 1.0f, 0)`.
It clears the complete presentation back buffer of the live logical device to
opaque black. Count or rectangle-pointer changes, another flag or color, or
different depth／stencil arguments return `D3DERR_INVALIDCALL` without changing
COM, graphics, guest-memory, allocator, sequence, or generation state. A
disabled or malformed policy produces a typed fault before mutation.

`RuntimeGraphicsDevice` owns the authoritative back-buffer content. Device
creation begins with a pristine logical buffer; the reached Clear replaces it
with one uniform-clear command containing an owner-local sequence, full
presentation rectangle, ARGB color, and content generation. The owner first
validates the live COM-to-device relation and builds a mutation-free plan, then
preflights global, device, content, and sequence generations before a targeted
commit. Stale and duplicate plans are rejected. The aggregate coordinates the
COM relation only; TypeScript owns no second back-buffer state and creates no
Canvas／WebGL／WebGPU resource.

Two independent Windows trace-v2 captures contain 402 and 404 calls from BGI
return RVA `0x5AF9E`. Every call has the exact six explicit arguments above,
returns `S_OK`, and observes LastError `0`; both captures are balanced and have
zero diagnostics. The native device pointer is opaque artifact identity. The
canonical Node and Chrome sessions reach sequence 127,355 at instruction
28,761,128 on the third logical device. That device has presentation
`1280x720`, COM object `21`, reference count `5`, and a live
`vine.logical-graphics-device.v1` identity. Both sessions record back-buffer
content generation `1`, clear sequence `1`, rectangle `[0,0,1280,720]`, color
`0xFF000000`, and device generation `44`. The following User32 `SetRect` constructs the
full-presentation `RECT [0,0,1280,720]` in guest memory without creating graphics or browser
state. Node and Chrome then stop eight instructions later at
`IDirect3DTexture9::AddDirtyRect` slot 21 with identical semantic and terminal-memory digests.

This slice does not clear depth or stencil, accept partial rectangles, allocate
pixel storage, upload texture content, draw, Present, model device loss, or
produce a visible browser frame.

## Bounded system-memory texture dirty region

Version 21 retains
`primary-recreated-system-memory-texture-dirty-rect-v1` for texture slot 21.
It admits only recreated source-texture variant 3: a live `2048x1024`, one-level,
static X8R8G8B8 SYSTEMMEM texture. The primary call passes
`RECT [0,0,1280,720]`; a NULL pointer marks the whole texture. Strictly ordered,
nonempty, nonnegative in-bounds rectangles return `D3D_OK`, while empty,
inverted, negative, or out-of-bounds rectangles return `D3DERR_INVALIDCALL`.
LastError, guest bytes, COM reference count, and browser resources remain
unchanged.

`RuntimeGraphicsTexture` is the sole dirty-region authority. Texture creation
begins dirty over the whole level-zero image, matching D3D9 managed update
semantics. A mutation-free owner plan validates the live COM-to-texture
relation, descriptor, rectangle, and generations before targeted commit.
Success merges partial rectangles by bounding union; NULL or a full rectangle
canonicalizes to `whole-texture`. Repeating a mark already covered by the
current region succeeds without consuming an identity or incrementing the
texture generation. State-changing stale／duplicate plans, capacity exhaustion,
policy mismatch, wrong-owner COM objects, and malformed rectangles fail before
any observable state changes; a repeated already-covered mark remains an
explicit mutation-free success.

Two focused Windows trace-v2 runs each contain the primary call plus seven
controlled variants. The primary return RVA is `0x5B01D`; all full, interior,
and NULL calls return `D3D_OK`, while all four invalid geometry classes return
`D3DERR_INVALIDCALL`. Sentinel LastError values are preserved. Both native
runs immediately consume the source texture through
`IDirect3DDevice9::UpdateTexture` slot 31. Canonical Node and Chrome sessions
complete AddDirtyRect at sequence 127,357 and instruction 28,761,165. They
agree on logical texture 7／COM object 22, device 3, whole-texture dirty state,
texture generation 41, content generation 1, one logical fill, zero guest writes,
and no renderer resource, then stop at that
UpdateTexture call.

This slice does not consume the dirty region, copy bytes to the default-pool
destination, allocate texture pixel storage, upload to a browser renderer,
draw, or Present.

## Bounded whole-texture logical content transfer

Version 20 adds
`primary-recreated-system-memory-to-static-default-texture-update-v1` for
device slot 31. The active variant requires the two reached recreated textures:
source variant 3 is static `2048x1024` X8R8G8B8 SYSTEMMEM, destination variant
4 has the identical shape in the DEFAULT pool, and both belong to the calling
device. NULL, identical, unknown, wrong-device, reversed-pool, mismatched-shape,
or otherwise wrong-role inputs return `D3DERR_INVALIDCALL` before mutation.

Level-zero logical content is owned by `RuntimeGraphicsTexture`, not by the
temporary surface view. Consequently the GDI fill survives `ReleaseDC` and final
surface `Release`. `UpdateTexture` creates a mutation-free two-texture plan,
preflights both live identities and generations, clones the source fill command,
then commits the destination content and source dirty-region consumption as one
targeted operation. The source becomes clean, destination records update sequence
1 and the consumed whole-texture region, each texture generation advances once,
and the global graphics generation advances once. A repeated call with a clean
source is a mutation-free `D3D_OK`; stale or duplicate state-changing plans and
capacity failure leave both textures, sequence allocation, and generations
unchanged.

Two focused native runs agree on the primary caller return RVA `0x5B038`, static
SYSTEMMEM-to-static-DEFAULT descriptors, `D3D_OK`, and preserved LastError. A
non-NULL controlled oracle also fixes repeat and dynamic-DEFAULT success plus
reversed, DEFAULT-source, and same-resource `D3DERR_INVALIDCALL`. The active
policy intentionally admits only the static destination observed at the primary
call; dynamic destination support and NULL semantics are not claimed by this
slice. Canonical Node and Chrome complete the call at sequence 127,358 and
instruction 28,761,175. They agree on source texture 7／COM 22 at generation 42,
destination texture 8／COM 24 at generation 45, content generation 1, fill
sequence 3 over `[0,0,1280,720]`, consumed source dirty state, zero guest writes,
and no renderer resource. Six instructions later both stop at device slot 41
`BeginScene`, endpoint `0x010112C0`, return address `0x1005B048`.

This gateway transfers only authoritative logical commands. It does not allocate
texel backing, perform a browser upload, implement partial-dirty transfer or the
reached dynamic-destination native variant, begin a scene, draw, Present, or
produce visible pixels.

## Bounded device scene lifecycle

Version 21 adds `primary-first-frame-scene-begin-v1` for device slot 41.
`RuntimeGraphicsDevice` is the sole active-scene authority. `BeginScene` accepts
only a live device whose scene is inactive, creates a mutation-free plan with the
device generation and next scene sequence, preflights sequence／generation
capacity, and commits the inactive-to-active transition once. NULL, unknown,
wrong-class, retired, or already-active devices return `D3DERR_INVALIDCALL`
without consuming the sequence or changing device／graphics generations. Stale
and duplicate state-changing plans are likewise rejected before mutation.

Two focused native runs stop on the first primary `BeginScene` at caller RVA
`0x5B048`; both return `D3D_OK`, preserve LastError 0, and immediately execute
`test eax,eax; js`. A controlled PE32/i386 lifecycle oracle independently fixes
`EndScene` outside a scene, nested `BeginScene`, and repeated `EndScene` as
`D3DERR_INVALIDCALL`, while valid begin／end cycles return `D3D_OK`. All seven
calls preserve LastError sentinel `0x5A5A`, two identical runs exit 0, and the
device reference count remains 1. This establishes the scene-state contract
without using a primary callsite or native COM pointer as semantic policy.

Canonical Node and Chrome sessions complete `BeginScene` at sequence 127,359
and instruction 28,761,181. They agree on device 3／COM object 21, reference
count 5, `1280x720` presentation, existing black-clear content generation 1,
active scene sequence 1, logical-device generation 45, zero guest writes, and
no renderer resource. Fourteen instructions later both stop at device slot 69
`SetSamplerState(0,6,2)`, endpoint `0x01011480`, return address `0x1005B18A`.

This slice does not implement `EndScene`, sampler state, draw calls, Present,
device loss, texel backing, renderer upload, or visible pixels. Beginning a
logical scene alone is not a first-present milestone completion.

## Bounded initial sampler state

Version 22 adds `primary-first-frame-linear-sampler-states-v1` for device slot
69. `RuntimeGraphicsDevice` owns a sparse typed sampler-state map; neither the
COM frontend nor TypeScript keeps a second copy. The bounded profile accepts
only sampler 0 `D3DSAMP_MINFILTER = D3DTEXF_LINEAR`, followed by sampler 0
`D3DSAMP_MAGFILTER = D3DTEXF_LINEAR`. Each state change validates the live
device and exact typed enum／value, creates a mutation-free device-generation
plan, preflights generation capacity, and performs one targeted commit. A
same-value repeat succeeds without changing generations. NULL, unknown,
wrong-class, retired, stale, unsupported sampler／state／value, and generation
exhaustion paths leave device, sampler map, and graphics generations unchanged.

Focused primary traces agree on caller RVAs `0x5B18A` and `0x5B1A0`, the exact
MIN-then-MAG order, `D3D_OK`, preserved LastError 0, and ignored HRESULTs. A
controlled PE32/i386 readback oracle fixes the initial MIN point filter, both
linear transitions, repeat success, LastError sentinel preservation, and stable
device reference count. That native driver also returns `D3D_OK` for the tested
invalid sampler, state, and filter values; the runtime intentionally narrows
that permissiveness to the reached primary band and returns
`D3DERR_NOTAVAILABLE` before mutation for other variants.

Canonical Node and Chrome complete the two calls at sequences 127,360／127,361
and instructions 28,761,195／28,761,204. They agree on device 3／COM object 21,
reference count 5, MIN then MIN+MAG linear snapshots, device generations 46／47,
zero guest writes, and no renderer resource. The caller then ignores three
explicit `D3DERR_NOTAVAILABLE` results for unprofiled `SetRenderState(171,1)`,
`SetRenderState(20,6)`, and `SetRenderState(19,5)` without changing logical
state. Both engines stop at instruction 28,761,237 on device slot 67
`SetTextureStageState(0,1,4)`, endpoint `0x01011460`, return address
`0x1005B1F5`.

This slice does not implement other samplers, sampler states or filter values,
the three rejected render states, texture-stage state, `EndScene`, draw calls,
Present, device loss, texel backing, renderer upload, or visible pixels.

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
The initial-resource slice leaves three logical textures live on one
device: one system-memory static texture and two default-pool textures, the last
of which is dynamic. The version 22 policy inherits the same three descriptors at
`2048x1024` when the guest recreates the resources; it does not infer a general
dimension or pool policy. The surface `GetDC` gateway adds a typed GDI acquisition
relation. The surface is a non-owning level-zero view, while reached `FillRect`
appends the first `RuntimeGraphics`-owned texture-content command.
Reached slot 43 `Clear` similarly appends authoritative uniform content to the
logical device back buffer without crossing into a renderer adapter.
The following `SetRect` leaf materializes the caller-owned guest structure, and
slot 21 `AddDirtyRect` updates only the source texture's Rust-owned dirty-region
    state. Slot 31 `UpdateTexture` is the first two-resource consumer boundary and
    copies that logical texture content while consuming the source dirty region.
Slot 41 `BeginScene` then owns the first device-local active-scene sequence and
lifecycle transition without creating browser graphics state. Slot 69
`SetSamplerState` adds the first sparse typed sampler map to that same logical
device and preserves the plan／preflight／targeted-commit boundary.
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
npm run check:runtime:bgi -- --profile d3d9-initial-sampler-states
npm run check:runtime:bgi-browser
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
The cumulative runtime additionally fixes four typed device render-state
updates, their idempotent same-value behavior, and the bounded
`GetRasterStatus` output. It subsequently records the exact full-target Clear,
logical device／COM relation, back-buffer generation, and the exact full-presentation
`SetRect` guest bytes. It then records the exact slot-21 `AddDirtyRect`, logical
texture／COM relation, authoritative whole-texture dirty state, idempotent mark,
texture content, and zero writes. It then records the exact slot-31
`UpdateTexture`, both logical texture／COM relations, copied fill command, dirty
consumption, generations, and zero writes. It then records the exact slot-41
`BeginScene`, Rust-owned inactive-to-active device transition, scene sequence,
lifecycle policy, and zero writes. Finally it records the exact two slot-69
`SetSamplerState` transitions, sparse sampler-state snapshots, device
generations, and three explicit post-sampler render-state rejections before
stopping at device slot 67 `SetTextureStageState`, without creating a browser
renderer resource or a second graphics authority.

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
slots 11／12 before slot 57. The historical `d3d9-initial-render-states` profile
delegates the exact four slot-57 settings and stops at device slot 19. The historical
`d3d9-raster-status` profile additionally delegates the bounded slot-19 query
and stops at `kernel32.dll!QueryPerformanceFrequency`. Profile selection does
not add callsite branches to the semantic handler or move Windows state into
the browser runtime. The historical `user32-update-window` profile stops at
slot 43, while `d3d9-clear` enables only the exact Clear profile and stops at
`USER32.dll!SetRect`. The historical `user32-set-rect` profile additionally
enables that exact User32 contract and stops at `IDirect3DTexture9::AddDirtyRect`
slot 21. The historical `d3d9-add-dirty-rect` profile enables only the bounded
source-texture dirty-region contract and stops at `IDirect3DDevice9::UpdateTexture`
slot 31. The historical `d3d9-update-texture` profile additionally enables the exact
static SYSTEMMEM-to-DEFAULT logical-content transfer and stops at
`IDirect3DDevice9::BeginScene` slot 41. The historical `d3d9-begin-scene` profile
additionally enables the exact inactive-to-active scene transition and stops at
`IDirect3DDevice9::SetSamplerState` slot 69. The active
`d3d9-initial-sampler-states` profile enables only the exact sampler-0 MIN／MAG
linear transitions, explicitly rejects the following three unprofiled render
states, and stops at `IDirect3DDevice9::SetTextureStageState` slot 67.

## Non-claims

- Only the bounded single logical adapter count, adapter-to-primary-monitor
  mapping, current display-mode, virtual adapter-identifier, virtual
  device-capability, exact device-type predicate, and exact texture-format
  predicate queries plus the exact windowed HAL `CreateDevice`, the three exact
  initial `CreateTexture` variants, level-zero `GetSurfaceLevel`, and one exact
  surface `GetDC`／black `FillRect`／`ReleaseDC` profile plus the exact initial
  `CreateVertexBuffer`／full-range `Lock`／`Unlock` sequence, the exact four
  initial `SetRenderState` settings, one bounded `GetRasterStatus` query,
  one exact full-target black `Clear`, one bounded source-texture
  `AddDirtyRect`, one exact static SYSTEMMEM-to-DEFAULT `UpdateTexture`, one
  exact inactive-to-active `BeginScene`, and the exact two sampler-0 MIN／MAG
  linear `SetSamplerState` calls are
  implemented. Mode
  enumeration, general device／format／usage checks, other levels,
  surface／texture storage, vertex declarations, other resource locks, other
  device/resource state, partial dirty-region transfer, dynamic-destination
  `UpdateTexture`, partial／depth／stencil Clear, `EndScene`, other sampler／state／
  filter variants, texture-stage state, draw, Present, Reset,
  and renderer output remain unsupported.
- `CreateDevice`, `CreateTexture`, `GetSurfaceLevel`, the reached surface DC
  fill, vertex-buffer gateway, full-target Clear, bounded AddDirtyRect,
  bounded UpdateTexture, bounded BeginScene, and bounded sampler state create Rust logical state
  only. They do
  not create a browser renderer resource,
  Canvas／WebGL／WebGPU context, swap chain, or visible frame.
- D3D8, D3D10／11, DXGI, shader translation, and general D3D9 compatibility are
  not claimed.
