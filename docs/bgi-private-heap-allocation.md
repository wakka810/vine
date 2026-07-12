# BGI private-heap allocation

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-private-heap-allocation`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-fiber-local-storage-allocation`
- First unsupported capability: `kernel32-fiber-local-storage-value`

## Purpose

The cumulative BGI session reached static endpoint ID `75`,
`kernel32.dll!HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 0x214)`. This checkpoint
connects that reached three-argument stdcall contract to the existing bounded
Rust private-heap transaction and continues with a guest pointer owned by
`GuestMemory`.

The implemented boundary is:

```text
logical private-heap handle 0x68010001
  -> checked HeapAlloc stdcall frame
  -> exact flags and byte-length gate
  -> existing transactional RuntimeHeap allocation
  -> lazy 16 MiB reservation and one-page RW/NX commit
  -> zeroed guest block 0x20000000..0x20000218
  -> guest pointer in EAX with LastError preserved
  -> DecodePointer(FlsSetValue token)
  -> next owned dynamic FlsSetValue trap
```

`HeapFree`, `HeapDestroy`, `HeapReAlloc`, zero-size behavior, nonzero
initialization, allocation failure return policy, and the later `0x800`
allocation are not part of this checkpoint.

## Native evidence

The existing opt-in heap-lifecycle capture remains under `.work/`:

| Evidence | Value |
|---|---:|
| Raw trace | `.work/windows-traces/bgi-heap-lifecycle-20260712.jsonl` |
| Bytes / lines | 6,870,828 / 21,822 |
| SHA-256 | `9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682` |

The exact paired events at caller return RVA `0xAC3CC` are:

```text
HeapAlloc(nativeHeap, 0x8, 0x214) -> 0x04A405C0, LastError 0 -> 0
HeapAlloc(nativeHeap, 0x8, 0x800) -> 0x04A407E0, LastError 0 -> 0
```

Native heap handles and result addresses are process-specific and are not
imported. The first call establishes `HEAP_ZERO_MEMORY`, byte length `0x214`, a
nonnull result, and LastError preserve behavior. The second call bounds the
next allocator variant but remains unsupported. Timeout termination left an
unrelated `DirectSoundCreate8` pair open, so the whole raw capture is not used
as a clean summary oracle; the exact HeapAlloc enter/leave pairs are balanced.

## Runtime contract

`HeapAlloc` uses the common `ApiDispatcher` with:

- arguments: logical private-heap handle, `U32` flags, `U32` byte length;
- return: nonnull opaque guest pointer;
- calling convention: x86 stdcall with 12 argument bytes;
- LastError policy: preserve;
- supported variant: flags `0x8`, byte length `0x214` only.

The handler validates flags and size before mutation, then calls:

```rust
ExecutionSession::allocate_private_heap_block(
    handle,
    0x214,
    RuntimeHeapInitialization::Zeroed,
)
```

This reuses the existing heap plan and `GuestMemoryTransaction`. The first
allocation has ID `1`, address `0x20000000`, requested length `0x214`, aligned
reservation `0x218`, and one committed 4 KiB page. The returned value is a
guest address, never the native result.

Other flags or sizes return structured
`heap-alloc-variant-unsupported` before heap metadata or guest memory changes.
Allocator resource failures remain structured because native NULL/LastError
failure behavior has not been measured.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  heap_alloc_contract_uses_transactional_guest_backing_and_rejects_other_variants `
  -- --exact
```

It fixes the returned pointer, zeroed bytes, reserve/commit delta, LastError,
stdcall cleanup, and mutation-free rejection of the observed `0x800` variant.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi-private-heap-allocation
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 19 |
| Completed generic calls | 39 |
| `HeapAlloc` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 40 / 70 |
| CPU-changed bytes | 415 |
| Host delta batches | 18 |
| Final synchronized generation | 87 |
| Report bytes | 183,102 |
| Report SHA-256 | `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4` |

The allocation emits the only mapping-bearing host delta in this profile:

```text
sequence:           38
generation:         82 -> 85
mapping operations: reserve + commit
heap owner:         10
dirty page:         0x20000000
payload:            4,096 bytes
payload SHA-256:    ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7
```

The v86 mirror creates the missing page table and maps the committed RW/NX
page before acknowledging generation `85`. BGI then reaches the second
`DecodePointer` occurrence at instruction `642`, resolving token `0xEC010005`
to dynamic `FlsSetValue` endpoint `0x7F020020`.

The compatibility anchors remain byte-identical:

| Profile | Bytes | SHA-256 |
|---|---:|---|
| Frozen 17-call | 140,660 | `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4` |
| Generic re-entry | 96,244 | `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` |
| Critical-section capability | 167,719 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |
| Pointer-decoding capability | 171,356 | `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e` |
| FLS-allocation capability | 175,123 | `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c` |

## Next structured stop

```text
kernel32.dll!FlsSetValue(0, 0x20000000)
dynamic endpoint ID: 3
endpoint address:     0x7F020020
return address:       0x100A5955
instruction:          644
```

The runtime returns `dynamic-export-contract-missing` in capability cluster
`kernel32-fiber-local-storage`. The next checkpoint must measure and implement
the reached per-thread FLS value store through the same dynamic dispatcher. It
must not skip directly to the later `HeapAlloc(..., 0x800)` call.
