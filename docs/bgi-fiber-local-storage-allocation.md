# BGI fiber-local-storage allocation

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-fiber-local-storage-allocation`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-pointer-decoding`
- First unsupported capability: `kernel32-private-heap-allocation`

## Purpose

The cumulative BGI session reached dynamic endpoint ID `1`,
`kernel32.dll!FlsAlloc(0x100A5666)`, after decoding a Vine-owned process token.
This checkpoint measures that reached call, adds bounded process-owned FLS slot
allocation, dispatches the dynamic endpoint through the common API dispatcher,
and continues to the next owned boundary.

The implemented boundary is:

```text
owned dynamic FlsAlloc endpoint
  -> checked one-argument stdcall frame
  -> nullable executable guest callback validation
  -> bounded process-owned logical slot allocation
  -> logical index in EAX with LastError preserved
  -> BGI index store and failure-sentinel check
  -> next owned HeapAlloc trap
```

`FlsGetValue`, `FlsSetValue`, `FlsFree`, per-thread values, teardown callback
invocation, and allocation-exhaustion return semantics are not part of this
checkpoint.

## Native evidence

Two opt-in traces were captured with `--observe fls-lifecycle`. Raw JSONL and
generated summaries remain under `.work/windows-traces/` and are not tracked.

| Capture | Bytes | Lines | API pairs | Max depth | Diagnostics | SHA-256 |
|---|---:|---:|---:|---:|---:|---|
| Primary | 441,487 | 1,459 | 694 | 3 | 0 | `2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281` |
| Confirmation | 453,151 | 1,498 | 713 | 3 | 0 | `d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186` |

Both summaries have contiguous sequence numbers, balanced call-ID pairs, and
empty hook, vtable, call-stack, agent-message, and cleanup diagnostics.

The exact BGI-owned call is the same in both captures:

| Capture | Callback RVA | Native result | LastError |
|---|---:|---:|---:|
| Primary | `0xA5666` | index `9` | `87 -> 87` |
| Confirmation | `0xA5666` | index `9` | `0 -> 0` |

The hook records caller return RVA `0xA5926`; the call instruction is at RVA
`0xA5924`. Native index `9` depends on allocations already made in that Windows
process and is discarded. The unchanged nonzero and zero LastError samples
establish preserve policy without treating LastError as the success signal.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-fls-lifecycle.jsonl `
  --seconds 2 --kill-after `
  --observe fls-lifecycle

node tools/summarize-windows-trace.mjs `
  --input .work/windows-traces/bgi-fls-lifecycle.jsonl `
  --output .work/windows-traces/bgi-fls-lifecycle.summary.json
```

## Static consumer

The measured call immediately stores and checks the returned index:

```text
004A5917  push 0x004A5666
004A591C  push dword ptr [0x004FEEBC]  ; encoded FlsAlloc
004A5922  call edi                     ; DecodePointer
004A5924  call eax                     ; FlsAlloc
004A5926  mov dword ptr [0x004F0760], eax
004A592B  cmp eax, -1                  ; FLS_OUT_OF_INDEXES
004A592E  je failure
```

The runtime mapping relocates the callback to `0x100A5666`, the index global
to `0x100F0760`, and the return to `0x100A5926`. Successful continuation stores
logical index `0`, passes the `-1` check, and later reaches the existing private
heap's first allocation consumer.

## Runtime contract

`RuntimeState::allocate_fls_slot` owns a monotonic process-local table with:

- namespace `vine.logical-fls-index.v1`;
- logical indices `0..127`;
- the optional guest cleanup callback for each slot;
- process-running, current-thread-running, and runnable/callback scheduler gates.

The reached non-NULL callback must identify executable guest memory before the
table changes. NULL is accepted because Windows permits a missing callback.
Capacity exhaustion is a structured `fls-index-space-exhausted` resource limit
without state, guest-memory, or LastError mutation. Although
`FLS_OUT_OF_INDEXES == 0xFFFFFFFF` is recorded, returning it has not been
implemented because native exhaustion behavior was not measured.

`FlsAlloc` is a one-argument stdcall contract with a `U32` return and LastError
preserve policy. It emits no guest write or host delta. The representative Rust
test covers unmapped callback rejection, executable and NULL callbacks,
indices `0..127`, fail-closed capacity, LastError preservation, and unchanged
guest memory:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

Dynamic exports now use the same `ApiDispatcher` as IAT imports. Endpoint
identity is keyed by `(binding kind, endpoint ID)`, so static import ID `1` and
dynamic export ID `1` coexist while addresses remain globally unique. Dynamic
contracts omit IAT/disposition metadata and still use the common checked ABI,
return, LastError, and fault path. The representative test includes that exact
ID collision.

## Generic BGI continuation

The cumulative capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-allocation
```

It enables the 13 baseline contracts, three critical-section contracts,
`DecodePointer`, and dynamic `FlsAlloc`. Earlier profiles do not register the
dynamic contract, preserving their exact process-ready metadata and report
serialization.

| Evidence | Result |
|---|---:|
| Implemented contracts | 18 |
| Completed generic calls | 37 |
| `FlsAlloc` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 38 / 68 |
| CPU-changed bytes | 399 |
| Host delta batches | 17 |
| Final synchronized generation | 82 |
| Report bytes | 175,123 |
| Report SHA-256 | `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c` |

The final post-return writeback reconciles the stack and BGI index-global pages
as one generation `81 -> 82` batch with 51 changed bytes. The FLS dispatch
itself has no guest write or host delta. Two complete runs produced the same
report size and digest.

The compatibility anchors remain byte-identical:

| Profile | Bytes | SHA-256 |
|---|---:|---|
| Frozen 17-call | 140,660 | `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4` |
| Generic re-entry | 96,244 | `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` |
| Critical-section capability | 167,719 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |
| Pointer-decoding capability | 171,356 | `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e` |

## Next structured stop

After returning logical FLS index `0`, BGI reaches:

```text
kernel32.dll!HeapAlloc(0x68010001, 8, 0x214)
static endpoint ID: 75
endpoint address:   0x7F0004A0
IAT address:        0x100D11D4
return address:     0x100AC3CC
instruction:        618
```

The runtime returns `endpoint-contract-missing` in capability cluster
`kernel32-private-heap-allocation`. The logical heap already has bounded Rust
guest-memory backing, but the reached `HeapAlloc` Win32 contract and BGI
continuation are not yet connected. That is the next checkpoint. It must reuse
the existing allocator transaction rather than introduce a second heap model.
