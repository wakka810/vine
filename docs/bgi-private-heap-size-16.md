# BGI private-heap 16-byte allocation

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-private-heap-size-16`
- Previous capability checkpoint: `bgi-get-module-file-name-a`
- First unsupported capability: `kernel32-private-heap-size-4`

## Purpose

BGI derives a temporary path-processing buffer size after
`GetModuleFileNameA`. The controlled virtual path is ASCII, so the reached
private-heap request is 16 bytes.

```text
GetModuleFileNameA(NULL, 0x100FF520, 260) -> "C:\Vine\BGI.exe\0"
  -> HeapAlloc(0x68010001, 0, 16)
  -> allocation ID 5 at 0x20000C40
  -> BGI consumes the bounded path buffer
  -> next stop is HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 4)
```

This checkpoint adds one policy value to the existing target-independent heap
handler. It does not import the different native installation-path size.

## Native evidence

Two combined observers retain `GetModuleFileNameA`, direct BGI `HeapAlloc`／
`HeapFree`, the path-capacity oracle, allocation fingerprints, caller frames,
and their exact temporal relation.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-heap-size-16-20260714-r2.jsonl `
  --seconds 0.1 --kill-after `
  --observe get-module-file-name-a --observe heap-allocation
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 464,225 | 190 | 69 | `6f0ed9c6a2b26576f1d2ba4d291ae6b7b5b862fa71e67a2f2780734bebe786ce` |
| Confirmation | 464,304 | 190 | 69 | `551e58fabb2e2ad97cb8b977c76945eef7bf636b7b84dfee3180ab8a77aa8cc1` |

Both captures contain 188 contiguous events (`0..187`) and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. Each has the same
ordering:

```text
sequence 83  GetModuleFileNameA leave, result 27
sequence 84  HeapAlloc enter
sequence 85  HeapAlloc leave, non-NULL result
```

The native private installation path contains five Japanese characters and the
reached native allocation is therefore `HeapAlloc(nativeHeap, 0, 36)`, not 16.
The call returns at RVA `0xA4A33` and preserves LastError `0 -> 0`. Native heap
handles and result pointers vary and are discarded.

The path consumer computes a checked size from two path-shape components before
calling the shared allocator wrapper:

```text
mov edi, eax
shl edi, 2
lea eax, [edi + ecx]
cmp eax, ecx
jb failure
push eax
call allocator-wrapper
```

The allocation return at RVA `0xA8517` is retained in ESI, tested against NULL,
and passed as the lower bound of the following path conversion together with
its computed upper bound. Thus allocation success and immediate consumption are
observed, while the host-specific size is only evidence of the native path.
The controlled all-ASCII path takes the same guest branch with size 16.

The native 36-byte buffers contain 30 NUL bytes in both runs, but their FNV-1a
fingerprints differ (`0x1B0CEF1D`／`0x5128B3FD`). Native initialization is not
stable and is not copied into guest semantics. Neither 100 ms capture observes
a related `HeapFree` for this allocation; it is proven live through the observed
consumer only.

## Runtime contract

Policy `kernel32-private-heap-size-16-v1` extends the flags-zero list from
`[1, 544]` to `[1, 16, 544]`. All other policy data and the single
`kernel32.heap-alloc.v2` implementation ID remain unchanged.

The handler validates logical heap identity, the exact policy pair, nonzero
size, alignment, capacity, address-space ownership, and the complete memory
transaction before mutation. It returns allocation ID `5` at `0x20000C40`,
with requested and reserved lengths both 16. Flags zero selects the established
`deterministic-uninitialized` initialization (`0xA5`). This is a replay-stable
runtime choice, not a claim about native heap bytes.

The heap band is already reserved and committed, so host delta sequence `153`
is dirty-only: generation `233 -> 234`, owner `10`, page `0x20000000`, payload
SHA-256
`e542f0cb74547f59cf7579ea2dba8ad7f69b7bd174332b3ec7163768ebd2df03`.
The first consumer is reconciled by writeback sequence `154` at instruction
`17,061`; it changes 55 bytes across stack and heap pages at generation
`234 -> 235`, with payload SHA-256
`b02aaa44a8d92c0a63162c813c3eba46e7fec72782c3636e28a88d0ef2a6e9b3`.

The public heap test fixes preceding-policy rejection, allocation ID and
address, requested／reserved lengths, deterministic initialization, stdcall
cleanup, LastError preservation, dirty-only delta shape, and unrelated public
size behavior through the same semantic handler.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-16
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 39 / 185 | 39 / 185 |
| Instructions at next stop | 18,143 | 18,143 |
| Writeback batches / pages | 186 / 272 | 1,728 / 2,269 |
| Host/mapping deltas | 52 | 52 |
| Final generation | 267 | 1,538 |
| Memory SHA-256 | `b648580e9ea1840e7c2703115d144e3b7014040f74a6670a14136808ee2782ff` | same |
| Report bytes | 828,414 | 210,684 |
| Report SHA-256 | `887a74c11a1466cfbbfdeebf126ecc6138588c4b1f9e89c39cfdbb290a2ee8e6` | `39efd3ceaa9d0e3c7cb9bc92b4ef00f342558677e7c1731bdca804166a63c29a` |

Chrome uses 2,325 slices, 2,139 budget yields, 1,969 Wasm crossings, and final
generation `1,538`. Success, cancel, and watchdog end in `stopped` with no
pending work. Runtime-core Wasm remains 1,858,027 bytes with SHA-256
`4f2d3c11fd37b5a53ed84696420c922ab680f8ea4c8215abadc3628ed978dab1`.

The separate 180-second workload completed 918 sessions with zero failure. All
16 thresholds passed: session p95 `197.300 ms`, slice p99 `0.990 ms`, maximum
heartbeat gap `18.320 ms`, main heap slope `-119,695.85 bytes/min`, Worker heap
slope `1,707,349.22 bytes/min`, and backing-storage slope
`1,442,354.00 bytes/min`. The 428,100-byte report has SHA-256
`91739bf0b40acabed9f14463faff014069a8161adec4b6b6d00bd7554f364ec9`.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 4)
endpoint ID:       75
endpoint address:  0x7F0004A0
IAT:               0x100D11D4
return:            0x100AC3CC
instruction:       18143
fault:             heap-alloc-variant-unsupported
cluster:           kernel32-private-heap-allocation
```

The next observer must fix the native size source, zero initialization,
allocation consumer, LastError, and lifetime before policy admits the reached
`(flags=8, size=4)` pair.

## Non-claims

This checkpoint does not reproduce native uninitialized bytes, import native
path-derived size 36, expose native heap pointers, enable arbitrary flags or
sizes, implement `HeapFree`／`HeapReAlloc`, or reach rendering, audio, input,
VFS, COM, or full play.
