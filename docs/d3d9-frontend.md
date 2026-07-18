# Bounded D3D9 frontend contract

## Purpose and current boundary

The D3D9 frontend owns D3D9 ABI policy and uses the shared Rust COM foundation
for guest-visible identity and lifetime. `Direct3DCreate9(32)` creates a bounded
root object with canonical IUnknown identity and a 17-slot `IDirect3D9`
interface. A different SDK version returns NULL before memory, identity,
allocator, or generation mutation. QueryInterface, AddRef, Release, and final
backing retirement use the generic COM plan／commit path; unsupported slots
return a typed fault with interface, slot, method, endpoint, and call site.

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

## Versioned frontend and shared graphics boundary

D3D8, D3D9, and any future D3D10／11 support remain separate guest-facing
frontends. They do not share COM identity, vtables, version-specific enums, or
device-loss state. A WineD3D-like internal layer is introduced only below those
frontends: the planned `RuntimeGraphics` owner accepts validated logical
device／resource／command contracts and coordinates the browser renderer
adapter. It begins at the D3D9 `CreateDevice` subsystem gateway, where the first
real logical graphics state is required. Root creation and display-query leaves
do not allocate placeholder graphics state.

This keeps the translation seam available without building frameworks for
unreached or deferred APIs. D3D10／11 and DXGI remain outside active scope until
explicitly reopened.

## Representative verification

```powershell
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:toolchain
npm run check:runtime:browser-worker
npm run check:runtime:bgi -- --profile d3d9-root-factory-and-identity
```

The controlled PE fixture verifies invalid-SDK NULL, non-NULL factory identity,
AddRef 2, Release 1, and final Release 0 with retirement. The private profile
also verifies that the first reached unsupported root method stops without
mutating the COM object or guest memory.

## Non-claims

- Display queries, adapter enumeration, `CreateDevice`, device/resource state,
  draw, Present, Reset, and renderer output are not implemented by the root
  checkpoint.
- The current root object does not create a `RuntimeGraphics` device or browser
  resource.
- D3D8, D3D10／11, DXGI, shader translation, and general D3D9 compatibility are
  not claimed.
