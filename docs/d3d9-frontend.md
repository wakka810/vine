# Bounded D3D9 frontend contract

## Purpose and current boundary

The D3D9 frontend owns D3D9 ABI policy and uses the shared Rust COM foundation
for guest-visible identity and lifetime. `Direct3DCreate9(32)` creates a bounded
root object with canonical IUnknown identity and a 17-slot `IDirect3D9`
interface. A different SDK version returns NULL before memory, identity,
allocator, or generation mutation. QueryInterface, AddRef, Release, and final
backing retirement use the generic COM plan／commit path.
`GetAdapterIdentifier` at slot 5, `GetAdapterDisplayMode` at slot 8, and
`GetDeviceCaps` at slot 14 delegate through a version-neutral COM gateway to
the D3D9 semantic handler. Other unreached slots return a typed fault with
interface, slot, method, endpoint, and call site.

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

## Bounded adapter display mode

Version 3 of `D3d9CompatibilityPolicy` describes one coherent logical adapter.
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

The same version 3 policy enables adapter `0`, flags `0`, and profile
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

Version 3 also enables adapter `0`, `D3DDEVTYPE_HAL`, and profile
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

## Versioned frontend and shared graphics boundary

D3D8, D3D9, and any future D3D10／11 support remain separate guest-facing
frontends. They do not share COM identity, vtables, version-specific enums, or
device-loss state. A WineD3D-like internal layer is introduced only below those
frontends: the planned `RuntimeGraphics` owner accepts validated logical
device／resource／command contracts and coordinates the browser renderer
adapter. It begins at the D3D9 `CreateDevice` subsystem gateway, where the first
real logical graphics state is required. Root creation and adapter-query leaves
do not allocate placeholder graphics state.

This keeps the translation seam available without building frameworks for
unreached or deferred APIs. D3D10／11 and DXGI remain outside active scope until
explicitly reopened.

## Representative verification

```powershell
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:toolchain
npm run check:runtime:browser-worker
npm run check:runtime:bgi -- --profile d3d9-get-device-caps
```

The controlled PE fixture verifies invalid-SDK NULL, non-NULL factory identity,
the four display-mode fields, a populated adapter-identifier ABI, the required
native or exact Vine device-capability band, AddRef 2, Release 1, and final
Release 0 with retirement. Its Windows self-oracle accepts native nonempty
NUL-terminated identity strings and required caps, while the Vine branch checks
the exact virtual identity and zero-bounded 304-byte caps. Rust tests separately
fix every output byte and the invalid adapter／flags／device type／NULL／unwritable／
policy／display-authority paths. The private Node and Chrome profiles record the
same 304-byte completion and stop next at `IDirect3D9::CheckDeviceType`.

## Non-claims

- Only the bounded current display-mode, virtual adapter-identifier, and
  virtual device-capability queries are implemented. Adapter count, mode
  enumeration, device／format checks, `CreateDevice`, device/resource state,
  draw, Present, Reset, and renderer output remain unsupported.
- The current root object does not create a `RuntimeGraphics` device or browser
  resource.
- D3D8, D3D10／11, DXGI, shader translation, and general D3D9 compatibility are
  not claimed.
