# BGI pointer decoding

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-pointer-decoding`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-critical-section-synchronization`
- First unsupported capability: `kernel32-fiber-local-storage`

## Purpose

The generic BGI session reached
`DecodePointer(0xEC010003)` after the critical-section initialization cluster.
This checkpoint adds the inverse operation for Vine-owned process tokens and
continues to the first dynamic FLS endpoint. It does not reproduce the Windows
process cookie, accept native encoded values, or implement FLS allocation.

The implemented boundary is:

```text
owned Kernel32 DecodePointer endpoint
  -> checked one-argument stdcall frame
  -> process token registry lookup
  -> token/source index consistency check
  -> original guest source in EAX
  -> immediate owned dynamic endpoint trap
```

## Native evidence

The existing opt-in pointer traces are sufficient for this reached call. They
were captured with `--observe pointer-encoding`; raw JSONL remains under
`.work/windows-traces/` and is not tracked.

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 2,213,858 | 6,979 | `71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3` |
| Confirmation | 312,314 | 1,040 | `4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c` |

At BGI caller RVA `0xA5924`, both captures decode a process-specific value to
the same native `FlsAlloc` export:

| Capture | Encoded input | Native result | LastError |
|---|---:|---:|---:|
| Primary | `0x61C0A638` | `0x758CD300` | `0 -> 0` |
| Confirmation | `0xC7C1EB7E` | `0x758CD300` | `87 -> 87` |

The changing encoded values prove that the native representation is tied to
process state. The stable decoded target and unchanged LastError establish the
contract needed here. Native tokens, addresses, and cookie state are discarded.

## Static consumer

BGI image-base-relative disassembly is:

```text
004A5911  mov edi, dword ptr [0x004D1208]  ; DecodePointer IAT
004A5917  push 0x004A5666                 ; FLS cleanup callback
004A591C  push dword ptr [0x004FEEBC]     ; encoded FlsAlloc
004A5922  call edi
004A5924  call eax
004A5926  mov dword ptr [0x004F0760], eax
```

The generic mapping relocates these values to callback `0x100A5666`, encoded
global `0x100FEEBC`, IAT `0x100D1208`, and post-FLS return `0x100A5926`.
Returning the dynamic endpoint source `0x7F020000` therefore has an immediate,
observable consumer.

## Runtime contract

`RuntimeState::decode_pointer` accepts only a token already registered by the
same process through `encode_pointer`. It validates:

- the configured `vine.logical-encoded-pointer.v1` prefix and nonzero token ID;
- the exact token-to-record identity;
- the record namespace and stored token ID;
- the reverse source-to-token index.

The successful lookup is read-only and returns the original `source` value.
`DecodePointer` is a one-argument stdcall contract with a nullable opaque-pointer return,
no guest writes, and LastError preserve policy. The reached token
`0xEC010003` resolves to dynamic endpoint ID `1` at `0x7F020000`, not to a host
function pointer.

An unallocated but well-formed Vine token fails as
`encoded-pointer-token-unknown`. A value outside the configured namespace fails
as `encoded-pointer-token-invalid`. Registry inconsistency is a runtime defect.
All failures occur without token allocation, guest-memory mutation, or LastError
change.

The representative semantic test covers two distinct encoded sources, repeated
stable decoding, NULL and dynamic-endpoint round trips, LastError preservation,
and unknown/malformed rejection:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

## Generic BGI continuation

The cumulative capability profile is:

```powershell
npm run check:runtime:bgi-pointer-decoding
```

It enables the 13 baseline contracts, the three critical-section contracts,
and `DecodePointer`. The baseline and critical-section profiles continue to
exclude later contracts, so their process-ready metadata and report snapshots
remain unchanged.

| Evidence | Result |
|---|---:|
| Implemented contracts | 17 |
| Completed generic calls | 36 |
| `DecodePointer` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 37 / 66 |
| CPU-changed bytes | 348 |
| Host delta batches | 17 |
| Final synchronized generation | 81 |
| Report bytes | 171,356 |
| Report SHA-256 | `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e` |

The extra dynamic call push adds one 4-byte CPU change and advances generation
`80 -> 81`. Decode itself emits no host delta or guest write. Two consecutive
complete runs produced the same report size and digest.

The compatibility anchors remain byte-identical:

| Profile | Bytes | SHA-256 |
|---|---:|---|
| Frozen 17-call | 140,660 | `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4` |
| Generic re-entry | 96,244 | `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` |
| Critical-section capability | 167,719 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |

## Next structured stop

After returning from DecodePointer, BGI immediately reaches:

```text
kernel32.dll!FlsAlloc(0x100A5666)
dynamic endpoint ID: 1
endpoint address:    0x7F020000
return address:      0x100A5926
instruction:         577
```

The runtime returns `dynamic-export-contract-missing` in capability cluster
`kernel32-fiber-local-storage`. The follow-on
`kernel32-fiber-local-storage-allocation` checkpoint is now verified in
[bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md).
It preserves this report, returns logical index `0`, and advances to static
`HeapAlloc`. `FlsGetValue`, `FlsSetValue`, `FlsFree`, callback invocation,
thread teardown, and general FLS value lifecycle remain unimplemented.
