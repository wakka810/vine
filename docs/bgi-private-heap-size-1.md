# BGI private-heap one-byte allocation

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-private-heap-size-1`
- Previous capability checkpoint: `bgi-wide-char-to-multi-byte`
- First unsupported capability: `kernel32-get-last-error`

## Purpose

The cumulative BGI session uses the result of a `WideCharToMultiByte` size
query as the byte length of a nonzero, non-`HEAP_ZERO_MEMORY` private-heap
allocation. The controlled runtime environment contains only its terminating
NUL, so the reached length is one byte. This checkpoint enables only that
policy pair and reuses the existing target-independent `kernel32.heap-alloc.v2`
semantics.

```text
controlled UTF-16 environment copy
  -> WideCharToMultiByte size query returns 1
  -> HeapAlloc(privateHeap, flags=0, size=1) returns allocation ID 3
  -> WideCharToMultiByte writes the terminating byte to that allocation
  -> FreeEnvironmentStringsW retires the source copy
  -> next stop is GetLastError
```

## Native evidence

Two complete combined traces retain the heap, conversion, environment-copy,
and later-free relation:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 4,390,685 | 13,830 | 6,882 | `6d0c372dc3fde306798ddf2e5dc98c656aecf96e72db0424c465abb9976be693` |
| Confirmation | 4,211,371 | 13,262 | 6,598 | `4825636bcbfc9c923caaac28c96980b60d60fc4856acb9a2b4258b461bb9fe96` |

Both summaries contain zero hook, vtable, call-stack, agent, or cleanup
diagnostics. Two interrupted captures are excluded because timeout occurred
inside a `HeapFree`／`HeapAlloc` pair; the summarizer rejects that unbalanced
evidence instead of inferring a result.

The native launch environment produces a 3,637-byte query result. At BGI
return RVA `0xA4A33`, `HeapAlloc(nativeHeap, 0, 3637)` returns non-null. The
returned allocation pointer is exactly the destination passed to the following
conversion at return RVA `0xA85BC`; query RVA `0xA8599` and conversion both
return 3,637. `FreeEnvironmentStringsW` succeeds for the source copy, and the
same allocation pointer is later passed to `HeapFree` at return RVA `0xA48CD`,
which returns one. The sampled allocation preserves LastError `0 -> 0`.

Pointer numbers are opaque run identity. Initial bytes differ between the two
captures, so native evidence fixes allocation shape, non-null result, consumer,
and lifetime relations only. It does not define exact uninitialized content or
promote the native launch's size 3,637 into general Win32 semantics.

## Runtime contract

Policy `kernel32-private-heap-size-1-v1` adds exactly `(flags=0,
byteLength=1)` beside the previously enabled zeroed `0x214` and `0x800`
allocations. The policy remains data: it does not select a BGI-specific handler
branch or contain caller RVAs, native pointers, call order, or a native result.

`kernel32.heap-alloc.v2` validates the logical heap, policy pair, nonzero size,
alignment, capacity, and address-space allocation before mutation. The reached
call returns guest pointer `0x20000A18`, allocation ID `3`, requested length
`1`, and reserved length `8`. Flags zero selects the runtime's
`deterministic-uninitialized` initialization, currently byte `0xA5`, rather
than accidentally inheriting zero-fill. This is a deterministic compatibility
choice for replay and testing, not a claim about native initial bytes.

The allocation reuses the already reserved and committed heap band. Its host
delta is dirty-only, and the following conversion changes the requested byte to
NUL through the generic dispatcher's bounded write contract. Unsupported flag
or size pairs stop before heap metadata or guest memory changes.

Representative public tests:

```powershell
cargo test -p vine-core --test api_dispatcher `
  heap_alloc_policy_enables_bounded_sizes_without_changing_semantics `
  -- --exact
npm run check:win32-policy
```

The Rust test fixes allocation identity, requested／reserved lengths,
initialization class and bytes, dirty-only delta shape, monotonic allocation,
and mutation-free policy rejection. Policy tests validate the JSON without an
evidence manifest and exercise another valid public-fixture size through the
same semantic implementation ID.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-1
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 28 / 67 | 28 / 67 |
| Instruction at next stop | 1,687 | 1,687 |
| CPU writeback batches / dirty pages | 68 / 114 | 196 / 237 |
| Host mapping deltas | 32 | 32 |
| Final generation | 129 | 247 |
| Memory content SHA-256 | `db234b2791244b2ca04dc20c957d3636a04b8a75ab506194d08ee03065bed7f2` | same |
| Report bytes | 308,984 | 62,288 |
| Report SHA-256 | `bfecc42fc7b32d21fc86580f81174548e6a26ff2b96f9dee05de51a9e7b4e26e` | `047a4877c916742e26b9904548b962d809f965693e9670f1a06b85d38224b6f2` |

Chrome used 240 slices, 172 budget yields, 299 Rust/Wasm bridge operations,
and 721 execution-state transitions. Success, cancel, and watchdog all end in
`stopped` with zero pending request records.

The separate 180-second workload gate completed 2,162 sessions with no failure.
All 16 versioned thresholds passed. The 970,774-byte report has SHA-256
`4c4e665e36a01e0e07acb47349edd6d09f6494916bfd8b0c76cac27397824cc2`.

## Next structured stop

```text
KERNEL32.dll!GetLastError()
static endpoint ID: 129
endpoint address:   0x7F000800
IAT address:        0x100D12AC
return address:     0x100A55DD
instruction:        1687
fault:              endpoint-contract-missing
```

The import is bound to an identity-preserving runtime endpoint, but the generic
dispatcher has no ABI contract for it. The next checkpoint must trace the
reached return and LastError relation, add a target-independent zero-argument
contract, and prove per-thread ownership without broadening adjacent error APIs.

## Non-claims

This checkpoint does not reproduce native uninitialized bytes, expose a host
heap pointer, enable arbitrary flags or sizes, dispatch `HeapFree`, implement
`GetLastError`, broaden heap capacity／failure／reallocation behavior, import the
host environment, or reach title rendering, audio, input, VFS, COM, or full play.
