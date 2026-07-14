# BGI private-heap one-byte allocation free

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-private-heap-free-size-1`
- Previous capability checkpoint: `bgi-private-heap-size-4`
- First unsupported capability: `kernel32-private-heap-size-128`

## Purpose

The cumulative BGI session reaches the first `HeapFree` for the flags-zero
one-byte allocation created while converting the controlled empty environment.
This checkpoint connects the Win32 ABI contract to the existing transactional
private-heap free path without enabling later allocation sizes or broad heap
semantics.

```text
HeapAlloc(privateHeap, flags=0, size=1) -> allocation ID 3 / 0x20000A18
  -> WideCharToMultiByte writes the controlled terminating NUL
  -> allocation remains live across the intervening startup path
  -> HeapFree(privateHeap, flags=0, 0x20000A18) returns TRUE
  -> ten already-supported EncodePointer calls complete
  -> next stop is HeapAlloc(privateHeap, flags=8, size=128)
```

## Native evidence

The focused heap observer in both accepted captures records the source
allocation and its later free in one balanced lifetime:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 361,115 | 168 | 59 | `9ab295457772c0a71f93cafb32f054f45dd0b64a8d07c21fb72f0aff30f15ed6` |
| Confirmation | 360,997 | 168 | 59 | `2836f8c51c84cab7690e0f66a4ec953ddeae9802669710981f0ff2d3f6c12c47` |

Both contain 166 contiguous events `0..165` and zero hook, call-stack, agent,
or cleanup diagnostics. At sequences `60/61`, call ID 7 is
`HeapAlloc(nativeHeap, 0, 3637)` from return RVA `0xA4A33`. The pointer is
non-null and its initial fingerprints differ between runs
(`0x02DDAABF`／`0x0406D700`), so native uninitialized bytes are not imported.

At sequences `160/161`, call ID 57 is
`HeapFree(sameHeap, 0, samePointer)` from return RVA `0xA48CD`. The observer
links it to allocation call ID 7, records the same requested length 3,637, and
returns one while preserving LastError `0 -> 0`. Immediately before free, both
runs have the same bounded buffer fingerprint `0xA8AD592C` and 47 NUL bytes.
Pointer and heap numbers differ between runs and remain opaque identities.

The accepted trace version did not snapshot a free-return continuation. Static
disassembly of the fingerprinted 1,193,472-byte BGI image fills that narrow
gap:

```text
0xA48C7  call [HeapFree IAT]
0xA48CD  test eax, eax
0xA48CF  jne  success
          ... failure path calls GetLastError ...
```

Thus the reached consumer requires nonzero success and only reads LastError on
failure. The native launch's 3,637-byte environment-derived allocation does not
replace Vine's controlled one-byte allocation; the evidence contributes only
the exact heap／pointer lifetime relation and return behavior.

## Runtime contract

Policy `kernel32-private-heap-free-size-1-v1` adds only `HeapFree` flags `0` to
the previous allocation policy. The policy contains no BGI caller RVA, native
pointer, allocation order, or target-specific handler selector.

`kernel32.heap-free.v1` defines a three-argument x86 stdcall contract and BOOL
return. It validates the non-null logical private-heap handle, enabled flags,
eight-byte-aligned non-null pointer, and exact live allocation identity before
mutation. The handler then uses the existing
`plan -> GuestMemory transaction -> targeted runtime commit` free operation.
The reached call:

- retires allocation ID 3 at `0x20000A18`;
- changes live allocations from six to five;
- fills all eight reserved bytes with deterministic freed byte `0xDD`;
- returns `TRUE`, cleans 12 argument bytes plus the return address, and
  preserves LastError `0`;
- emits one dirty-only heap page delta with no mapping or cache operation.

The `0xDD` fill is a Vine use-after-free diagnostic policy, not a claim about
Windows heap bytes. A later first-fit allocation may reuse the address but gets
a new monotonic allocation ID and its requested initialization overwrites the
poisoned bytes.

Public semantic coverage proves mutation-free rejection for a policy-disabled
call, unsupported flags, NULL, an aligned interior pointer, a pointer owned by
another heap, and double free. It also proves exact success, LastError
preservation, dirty-only delta shape, freed-byte policy, and reuse identity:

```powershell
cargo test -p vine-core --test api_dispatcher `
  heap_free_policy_retires_only_an_exact_live_allocation_transactionally `
  -- --exact
cargo test -p vine-core --test runtime_heap_backing --locked
npm run check:win32-policy
```

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-free-size-1
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 40 / 197 | 40 / 197 |
| Instruction at next stop | 18,653 | 18,653 |
| CPU writeback batches / dirty pages | 198 / 300 | 1,772 / 2,329 |
| Host mapping deltas | 54 | 54 |
| Final generation | 281 | 1,582 |
| Memory content SHA-256 | `f248e7dafa44a4a036822bd2eda199959e01c5d39090f67fba5f9f15c0de0186` | same |
| Report bytes | 875,554 | 217,267 |
| Report SHA-256 | `e0b255e0112fb7d0cbcef12afcd91be63877049fed56fc30b62ce5123f1acd96` | `0287b5241596f2aaf08e0ac2fb4599037ee5fe4cd6e1c1b4b54ba1058a69b348` |

The Node free delta is sequence 187, generation `269 -> 270`, owner 10,
page `0x20000000`, and payload SHA-256
`93cb85efb43c4586535cabdedf112e538d7fbe792245ed8954482b9815aa092a`.
Chrome used 2,392 slices, 2,194 budget yields, and 2,027 Rust／Wasm bridge
operations. Success, cancel, and watchdog all ended in `stopped` with no
pending host request or callback record.

The separate 180-second workload gate completed 615 sessions with no failure.
All 16 versioned thresholds passed; session p95 was 356.50 ms, slice p99 was
1.93 ms, and maximum heartbeat gap was 41.18 ms. The 293,660-byte benchmark
report has SHA-256
`cedc7af69177140072a45472c9ce4472fbcf6bebdcda602a509727474eb9a613`.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(0x68010001, 0x00000008, 0x00000080)
static endpoint ID: 75
endpoint address:   0x7F0004A0
IAT address:        0x100D11D4
return address:     0x100AC3CC
instruction:        18653
fault:              heap-alloc-variant-unsupported
```

The API contract and semantic handler already exist, but the active policy does
not enable zeroed size 128. The next checkpoint must measure that allocation's
native size source, consumer, initialization, and lifetime before adding the
single policy pair.

## Non-claims

This checkpoint does not accept arbitrary `HeapFree` flags, NULL, interior,
foreign, or stale pointers; reproduce Windows corruption termination;
implement `HeapReAlloc`, `HeapDestroy` dispatch, zero-size behavior, concurrent
allocation, or LFH; enable size 128; or reach rendering, audio, input, VFS, COM,
or full play.
