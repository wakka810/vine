# BGI private-heap 128-byte allocation

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-private-heap-size-128`
- Previous capability checkpoint: `bgi-private-heap-free-size-1`
- First unsupported capability: `kernel32-is-processor-feature-present`

## Purpose

BGI allocates a fixed 32-element table immediately after the startup
EncodePointer run. This checkpoint enables only that reached zeroed allocation
through the existing policy-driven heap handler.

```text
HeapFree(privateHeap, 0, allocation ID 3) -> five live allocations
  -> ten EncodePointer calls
  -> HeapAlloc(privateHeap, HEAP_ZERO_MEMORY, 32 * 4)
  -> allocation ID 7 at 0x20000C58, 128 requested/reserved zero bytes
  -> EncodePointer(allocation) and store the encoded identity twice
  -> clear the first DWORD through the retained allocation pointer
  -> next stop is IsProcessorFeaturePresent(10)
```

The address is selected from Vine's current heap state. It is not copied from
a native Windows heap address or from native allocator reuse behavior.

## Native evidence

The accepted focused heap captures are the same balanced traces that establish
the preceding allocation/free lifetime:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 361,115 | 168 | 59 | `9ab295457772c0a71f93cafb32f054f45dd0b64a8d07c21fb72f0aff30f15ed6` |
| Confirmation | 360,997 | 168 | 59 | `2836f8c51c84cab7690e0f66a4ec953ddeae9802669710981f0ff2d3f6c12c47` |

Both have 166 contiguous events `0..165` and zero hook, call-stack, agent,
cleanup, or oracle diagnostics. At sequences `160/161`, call ID 57 frees the
3,637-byte block allocated by native call ID 7. At sequences `162/163`, call
ID 58 is `HeapAlloc(sameHeap, 8, 128)` from return RVA `0xAC3CC`. It succeeds,
preserves LastError `0 -> 0`, and returns the just-freed native address in both
runs. All 128 requested bytes are NUL, with FNV-1a `0xA7B537C5`.

The pointer reuse is evidence about the native allocator instance, not a
portable Win32 guarantee. Vine had freed an eight-byte reserved block at this
point, which cannot satisfy 128 bytes. The runtime therefore skips that hole
and creates a new monotonic allocation identity at its next fitting address.

Static disassembly of the fingerprinted BGI image fixes the size source rather
than treating the observed 128 as a general heap variant:

```text
RVA 0xA1DC3
  push 4
  push 0x20
  call 0xA4EAC

RVA 0xA4EAC
  pass element-count=32, element-bytes=4, flags=0
  to checked multiply/allocation helper RVA 0xAC383
```

The checked helper multiplies `32 * 4` and reaches `HeapAlloc` with flag 8.
The caller continuation retains and consumes the pointer:

```text
RVA 0xA1DCF
  pop ecx
  pop ecx
  mov esi, eax
  push esi
  call [EncodePointer]
  mov [global1], eax
  mov [global2], eax
  test esi, esi
  jne success
  ...
success:
  and dword ptr [esi], 0
```

Neither accepted capture observes a later free related to this allocation
before the capture ends. The proof is intentionally limited to its fixed size
source, zero initialization, successful identity flow, and immediate consumer.

## Runtime contract

Policy `kernel32-private-heap-size-128-v1` extends the flag-8 allocation list
from `[4, 532, 2048]` to `[4, 128, 532, 2048]`. It retains flags-zero sizes
`[1, 16, 544]` and HeapFree flag `0`; no caller RVA, sequence number, native
pointer, or target-specific implementation branch enters the policy.

The existing `kernel32.heap-alloc.v2` handler validates the logical heap, exact
policy pair, nonzero size, alignment, capacity, address-space ownership, and
complete GuestMemory transaction before mutation. The reached call:

- returns allocation ID 7 at `0x20000C58`;
- records 128 requested and 128 reserved bytes;
- zeroes the complete reserved range because `HEAP_ZERO_MEMORY` is set;
- changes live allocations from five to six;
- preserves LastError `0` and performs x86 stdcall cleanup;
- emits a dirty-only owner-10 heap delta without mapping or cache effects.

Host delta sequence 198 advances generation `281 -> 282` on page
`0x20000000`; its payload SHA-256 is
`93cb85efb43c4586535cabdedf112e538d7fbe792245ed8954482b9815aa092a`.
Supported call 199 encodes the new pointer. Consumer writeback sequence 200 at
instruction 18,693 advances `283 -> 284`, changes 14 bytes across stack,
image-data, and heap pages, and has payload SHA-256
`181672ed0948ea489ac06efa8f4c8440ba46bd17c631f9729e7fb92c1f20e913`.

The public heap test fixes previous-policy rejection, exact allocation identity
and address, requested/reserved lengths, full zero initialization, LastError,
stdcall cleanup, dirty-only delta shape, and continued operation of an
unrelated public policy size after the new allocation.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-128
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 40 / 199 | 40 / 199 |
| Instruction at next stop | 18,693 | 18,693 |
| CPU writeback batches / dirty pages | 200 / 304 | 1,775 / 2,333 |
| Host mapping deltas | 55 | 55 |
| Final generation | 284 | 1,586 |
| Memory content SHA-256 | `5262246193dbe482a09781cac162f4f23f4eabaa4981c5f50ec2c9460ce96a89` | same |
| Report bytes | 883,995 | 218,302 |
| Report SHA-256 | `51382513962b57159e178966b16aac2010513a3c58cccaf9b9d6abbfc857d073` | `ff611b35fe525989527e2bf7eca52537d6b68caf2d04421749a17f334d53c6eb` |

Chrome used 2,398 slices, 2,198 budget yields, and 2,033 Rust/Wasm bridge
operations. Success, cancellation, and watchdog all ended in `stopped` with no
pending host request or callback record.

The separate 180-second workload gate completed 591 sessions with zero
failure. All 16 versioned thresholds passed: session p95 was `354.955 ms`,
slice p99 was `1.765 ms`, maximum heartbeat gap was `54.130 ms`, main-heap
slope was `-56,889.96 bytes/min`, Worker-heap slope was
`2,509,579.05 bytes/min`, and backing-storage slope was
`2,090,206.60 bytes/min`. The 283,162-byte report has SHA-256
`e652e459c8e62740c1ae5bc8605c496dbe26e747c7a8db1a80a918e84e7c17e9`.

## Next structured stop

```text
KERNEL32.dll!IsProcessorFeaturePresent(0x0000000A)
endpoint ID:       74
endpoint address:  0x7F000490
IAT:               0x100D11D0
return:            0x100A4D87
instruction:       18693
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next trace-driven checkpoint must determine which processor feature index
10 represents on the fixed native environment, the native return and
LastError behavior, the immediate branch consumer, and the controlled runtime
policy. It must not import arbitrary host CPU capabilities.

## Non-claims

This checkpoint does not enable arbitrary heap sizes or flags; promise native
address reuse; implement HeapReAlloc, HeapDestroy, or concurrent/LFH behavior;
implement IsProcessorFeaturePresent; import host CPU feature discovery; or
reach rendering, audio, input, VFS, COM, or full play.
