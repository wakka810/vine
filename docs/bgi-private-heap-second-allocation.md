# BGI private-heap second allocation

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-private-heap-second-allocation`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-interlocked-increment`
- First unsupported capability: `kernel32-standard-handles`

## Purpose

The cumulative BGI session reached the second static endpoint ID `75` call,
`kernel32.dll!HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 0x800)`. This checkpoint
extends the versioned HeapAlloc contract to the first two measured sizes,
retains the first block, and allocates the second block through the existing
bounded Rust heap transaction.

The implemented boundary is:

```text
existing allocation ID 1 at 0x20000000 (0x214 requested / 0x218 reserved)
  -> checked second HeapAlloc stdcall frame
  -> exact HEAP_ZERO_MEMORY + 0x800 variant gate
  -> existing first-fit private-heap transaction
  -> zeroed allocation ID 2 at 0x20000218
  -> dirty-only delta for the existing unsealed heap owner
  -> guest pointer return with LastError preserved
  -> next owned GetStdHandle trap
```

Later sizes, other flags, zero-size behavior, allocation failure returns,
`HeapFree`, `HeapDestroy`, and `HeapReAlloc` are not part of this checkpoint.

## Native evidence

The existing opt-in heap-lifecycle trace remains under `.work/`:

| Evidence | Value |
|---|---:|
| Bytes / lines | 6,870,828 / 21,822 |
| SHA-256 | `9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682` |

The first two paired BGI events at caller return RVA `0xAC3CC` are:

| Call ID | Flags | Size | Native result | LastError |
|---:|---:|---:|---:|---:|
| 28 | `0x8` | `0x214` | `0x04A405C0` | `0 -> 0` |
| 29 | `0x8` | `0x800` | `0x04A407E0` | `0 -> 0` |

The next same-caller allocation is size `0xBC`, followed by further distinct
sizes. Those later variants remain unsupported. Native heap and result addresses
are process-specific and are discarded.

## Versioned runtime contract

The original `kernel32.heap-alloc.v1` contract remains unchanged and accepts
only `0x214`. This preserves the earlier private-heap, FLS-value, and
InterlockedIncrement reports byte-for-byte. The new cumulative profile selects
`kernel32.heap-alloc-first-two-reached.v1`, which uses the same ABI and handler
but admits `0x214` and `0x800` only.

Both variants use:

- logical heap handle, flags, and byte length arguments;
- x86 stdcall with 12 argument bytes;
- non-null opaque guest-pointer return;
- LastError preserve policy;
- `RuntimeHeapInitialization::Zeroed`.

The allocator plans metadata before mutation and applies mapping／byte changes in
one `GuestMemoryTransaction`. The second block has allocation ID `2`, address
`0x20000218`, requested／reserved length `0x800`, and shares the already committed
page. No reserve or commit operation is required.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  heap_alloc_contract_versions_preserve_first_profile_and_add_second_block `
  -- --exact
```

The test fixes v1 mutation-free rejection, v2 first-block retention, the exact
second identity／address／zeroed bytes, dirty-only delta, stdcall cleanup,
LastError preservation, and mutation-free rejection of the next observed
`0xBC` size.

## Execution-mirror correction

The second allocation exposed an evidence-adapter assumption that dirty-only
host deltas always belong to sealed owners. Runtime heap owners remain unsealed
so backing can grow and mutate. The v86 mirror now retains the owner-sealed bit
with each mapping and requires later deltas to match it. It still rejects owner,
protection, or sealed-state identity changes.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi-private-heap-second-allocation
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 21 |
| Completed generic calls | 51 |
| `HeapAlloc` calls | 2 |
| Live allocations after second call | 2 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 52 / 89 |
| CPU-changed bytes | 906 |
| Host delta batches | 27 |
| Final synchronized generation | 108 |
| Report bytes | 239,789 |
| Report SHA-256 | `fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8` |

The second allocation host delta is:

```text
sequence:           51
generation:         106 -> 107
mapping operations: 0
heap owner:         10 (unsealed, identity preserved)
dirty page:         0x20000000
payload:            4,096 bytes
payload SHA-256:    bd49b343e4864d3d12162b7f0d9a7ce77782b4aa1c931f72121ddcb0f2018532
cache effects:      none
```

All eight earlier reports remain byte-identical, including the 234,775-byte
InterlockedIncrement report.

## Next structured stop

```text
kernel32.dll!GetStdHandle(STD_INPUT_HANDLE / 0xFFFFFFF6)
static endpoint ID: 94
endpoint address:   0x7F0005D0
IAT address:        0x100D1220
return address:     0x100A87B6
instruction:        1395
```

The runtime returns `endpoint-contract-missing` in capability cluster
`kernel32-api-contract`. The next checkpoint must measure the native standard
handle result, LastError, and immediate consumer, then define a process-owned
logical standard-handle contract. It must not expose host OS handles or add file
I/O behavior before that evidence exists.
