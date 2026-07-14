# BGI private-heap 4-byte allocation

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-private-heap-size-4`
- Previous capability checkpoint: `bgi-private-heap-size-16`
- First unsupported capability: `kernel32-private-heap-free-size-1`

## Purpose

BGI builds an ANSI environment-pointer table after processing the executable
path. The native process has 46 ordinary environment entries, whereas Vine's
controlled process intentionally imports no host environment.

```text
controlled empty environment authority
  -> one pointer-table terminator slot
  -> HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 4)
  -> allocation ID 6 at 0x20000C50
  -> BGI stores the table identity and confirms the NULL terminator
  -> next stop is HeapFree(0x68010001, 0, 0x20000A18)
```

This checkpoint adds one policy value to the existing target-independent heap
handler. It does not import the native host environment or its 188-byte table.

## Native evidence

Two focused `heap-allocation` captures retain only direct BGI `HeapAlloc`／
`HeapFree` pairs, bounded initial-buffer fingerprints, caller frames, and
same-pointer lifetime relations.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-heap-size-4-20260714-r1.jsonl `
  --seconds 0.1 --kill-after --observe heap-allocation
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 361,115 | 168 | 59 | `9ab295457772c0a71f93cafb32f054f45dd0b64a8d07c21fb72f0aff30f15ed6` |
| Confirmation | 360,997 | 168 | 59 | `2836f8c51c84cab7690e0f66a4ec953ddeae9802669710981f0ff2d3f6c12c47` |

Both captures contain 166 contiguous events (`0..165`) and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. Each has the same
ordering:

```text
sequence 64/65  HeapAlloc(nativeHeap, 0, 36), path buffer
sequence 66/67  HeapAlloc(nativeHeap, HEAP_ZERO_MEMORY, 188), environment table
```

The second allocation returns at RVA `0xAC3CC`, succeeds with a run-specific
native pointer, and preserves LastError `0 -> 0`. All 188 requested bytes are
NUL in both runs and have FNV-1a `0x67691275`, fixing `HEAP_ZERO_MEMORY`
initialization without importing the result address.

Static callsite disassembly at RVA `0xA821E` explains the host/runtime size
difference. BGI walks the ANSI environment block, counts entries whose first
byte is not `=`, increments once for the NULL terminator, and calls a checked
`count × 4` allocator:

```text
xor edi, edi
...                         ; walk NUL-terminated entries
cmp al, '='
je  skip_count
inc edi
...
push 4
inc edi                     ; terminator slot
push edi
call allocation-wrapper
```

The lower wrapper at RVA `0xAC383` checks multiplication overflow, multiplies
the count by the four-byte pointer width, and calls `HeapAlloc` with flag `8`.
Native therefore requests `(46 + 1) × 4 = 188` bytes. The controlled double-NUL
environment has zero entries, so the same guest code requests `(0 + 1) × 4 =
4` bytes. Importing 188 would reintroduce host environment state and violate the
process bootstrap authority.

The consumer at RVA `0xA8261` stores the table globally, tests allocation
success, writes one allocated string pointer per retained native entry, advances
by four bytes, and writes a final NULL DWORD. Failure frees the table and clears
the global. Neither accepted 100 ms capture observes a successful-path
`HeapFree` related to this table; its lifetime is proven through the global
store and completed terminator write only.

## Runtime contract

Policy `kernel32-private-heap-size-4-v1` extends the zeroed list from
`[532, 2048]` to `[4, 532, 2048]`. All other policy data and the single
`kernel32.heap-alloc.v2` implementation ID remain unchanged.

The handler validates logical heap identity, the exact policy pair, nonzero
size, alignment, capacity, address-space ownership, and the complete memory
transaction before mutation. It returns allocation ID `6` at `0x20000C50`,
with requested length 4 and reserved length 8. `HEAP_ZERO_MEMORY` selects the
established `zeroed` initialization for the full reserved range.

The heap band is already reserved and committed, so host delta sequence `186`
is dirty-only: generation `267 -> 268`, owner `10`, page `0x20000000`, payload
SHA-256
`1849087a95d674f3e6d3876d75739a49d1faa5ad948249223d3215bc45d4fed5`.
The environment-table consumer is reconciled by writeback sequence `187` at
instruction `18,180`; it changes 16 bytes across stack and image-data pages at
generation `268 -> 269`, with payload SHA-256
`1cd6cab32153e17fba2c7a368e562ff6bec446a637e4f510ec285c7ac265689b`.

The public heap test fixes preceding-policy rejection, allocation ID and
address, requested／reserved lengths, full zero initialization, stdcall cleanup,
LastError preservation, dirty-only delta shape, and unrelated public size
behavior through the same semantic handler.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-4
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 39 / 186 | 39 / 186 |
| Instructions at next stop | 18,180 | 18,180 |
| Writeback batches / pages | 187 / 274 | 1,731 / 2,273 |
| Host/mapping deltas | 53 | 53 |
| Final generation | 269 | 1,542 |
| Memory SHA-256 | `d9abc780ef3d253810695351cb66296b1aeb727e7e8590f73fe20e2c1c73cd2c` | same |
| Report bytes | 833,007 | 211,133 |
| Report SHA-256 | `80f28db63bf8400b7ea7d1db5d7552303dec05dc71addb1ac2ed1ecea29eefa1` | `df0dad34b767393269db89742e40655d7b82044d550bfa46e122f8625a7fb280` |

Chrome uses 2,330 slices, 2,143 budget yields, 1,974 Wasm crossings, and final
generation `1,542`. Success, cancel, and watchdog end in `stopped` with no
pending work. Runtime-core Wasm remains 1,858,027 bytes with SHA-256
`4f2d3c11fd37b5a53ed84696420c922ab680f8ea4c8215abadc3628ed978dab1`.

The separate 180-second workload completed 627 sessions with zero failure. All
16 thresholds passed: session p95 `349.845 ms`, slice p99 `1.950 ms`, maximum
heartbeat gap `50.985 ms`, main heap slope `-99,181.06 bytes/min`, Worker heap
slope `2,802,419.43 bytes/min`, and backing-storage slope
`2,932,591.86 bytes/min`. The 298,928-byte report has SHA-256
`267e0b677f543aeeed5135d2b622d6052866dfea671cf1b084e9e3ea14c8cd6c`.

## Next structured stop

```text
KERNEL32.dll!HeapFree(0x68010001, 0, 0x20000A18)
endpoint ID:       76
endpoint address:  0x7F0004B0
IAT:               0x100D11D8
return:            0x100A48CD
instruction:       18180
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The pointer is allocation ID `3`, the earlier one-byte flags-zero block, not
the new environment table. The next observer must fix native success／failure,
LastError, exact-pointer validation, freed-byte policy, reuse, double-free, and
the caller continuation before exposing the existing internal free operation
through Win32 dispatch.

## Non-claims

This checkpoint does not import native environment entries, expose native heap
pointers, enable arbitrary flags or sizes, implement `HeapFree`／`HeapReAlloc`,
or reach rendering, audio, input, VFS, COM, or full play.
