# BGI critical-section synchronization

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-critical-section-synchronization`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous generic checkpoint: `bgi-generic-runtime-reentry`
- First unsupported capability: `kernel32-pointer-decoding`

## Purpose

The generic BGI session first stopped at
`InitializeCriticalSectionAndSpinCount(0x100FF630, 4000)`. This checkpoint
measures the reached x86 contract on native Windows, adds process-owned critical
section state to the Rust runtime, and continues through the same generic
dispatcher and automatic memory synchronization path. It does not add a numbered
BGI call mode or an API-specific reconciliation protocol.

The implemented boundary is deliberately narrow:

```text
owned Kernel32 endpoint
  -> checked stdcall frame and 24-byte guest storage
  -> process-owned RuntimeCriticalSection
  -> same-thread acquire / recursion / release
  -> exact x86-compatible guest bytes
  -> VINEDL01 host delta and generic register completion
```

## Native evidence

The opt-in trace was reproduced with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work\windows-traces\bgi-critical-section-20260713-r3.jsonl `
  --seconds 2 `
  --kill-after `
  --observe critical-section
```

The raw trace is not tracked in Git. Its provenance is:

| Field | Value |
|---|---:|
| Bytes | 8,094,273 |
| Lines | 14,542 |
| SHA-256 | `c668e8e6bd831442d4f0f5e3e2937395b7cd2bc003aa96a67b8872417cc53567` |
| Frida | `17.15.4` |
| Python | `3.14.6` |
| Windows build | `26200` |
| Diagnostics／oracle errors | `0`／`0` |

BGI calls `InitializeCriticalSectionAndSpinCount` 14 times from caller RVA
`0xAB5E7`. The native image is based at `0x00240000`, so the observed storage
addresses are `0x0033F630` through `0x0033F768`; their image RVAs are
`0xFF630` through `0xFF768`, the stride is exactly 24 bytes, and the generic
mapping therefore uses `0x100FF630` through `0x100FF768`. Every call passes spin
count 4000 and returns BOOL `1`.

The initialized x86 structure is:

| Offset | Field | Value |
|---:|---|---:|
| `+0x00` | `DebugInfo` | `0xFFFFFFFF` |
| `+0x04` | `LockCount` | `-1` |
| `+0x08` | `RecursionCount` | `0` |
| `+0x0C` | `OwningThread` | `0` |
| `+0x10` | `LockSemaphore` | `0` |
| `+0x14` | `SpinCount` | `4000` |

The exact 24 bytes are:

```text
FF FF FF FF  FF FF FF FF  00 00 00 00  00 00 00 00
00 00 00 00  A0 0F 00 00
```

BGI immediately consumes three of those objects through wrapper RVAs
`0xAB763`／`EnterCriticalSection` and `0xAB66F`／`LeaveCriticalSection`. During
the two-second capture, storage RVAs `0xFF6C0`, `0xFF6F0`, and `0xFF708` had
5, 16, and 18 balanced enter／leave pairs respectively. The enter observation
starts from the unlocked bytes; the paired leave starts with `LockCount = -2`,
`RecursionCount = 1`, and the native TID.

## Controlled oracle

The trace profile also runs one Frida-owned x86 oracle with sentinel LastError
values. It starts from 24 bytes of `0xA5`, then observes:

| Step | `LockCount` | Recursion | Owner | LastError |
|---|---:|---:|---:|---:|
| Initialize | `-1` | `0` | `0` | `87` |
| First enter | `-2` | `1` | native TID | `88` |
| Recursive enter | `-2` | `2` | native TID | `89` |
| Recursive leave | `-2` | `1` | native TID | `90` |
| Final leave | `-1` | `0` | `0` | `91` |
| Delete | `0` | `0` | `0` | `92` |

Initialization returns BOOL `1`; enter, leave, and delete are VOID. Every
sentinel survives the call, so all observed operations preserve LastError. The
oracle globals are reset in unconditional cleanup even if a native call or hook
validation throws.

## Runtime contract

`RuntimeState` owns a bounded map of at most 4,096 critical sections keyed by
their nonzero, 4-byte-aligned guest storage address. Each record owns the spin
count, optional controlled thread ID, and recursion count. Native thread IDs and
kernel synchronization handles are not imported.

Three Kernel32 contracts are registered:

| API | Return | Behavior |
|---|---|---|
| `InitializeCriticalSectionAndSpinCount` | BOOL `1` | creates unlocked process state and writes all 24 bytes |
| `EnterCriticalSection` | VOID | validates guest bytes, acquires or recursively enters for the current thread |
| `LeaveCriticalSection` | VOID | validates ownership, decrements recursion, and releases at zero |

All three use stdcall cleanup and preserve LastError. Enter and leave require the
guest bytes to match the authoritative process state before mutation. The guest
owner field uses Vine's controlled TID, `8192`, rather than a native Windows TID.

Cross-thread contention is not silently accepted. The dispatcher returns
structured unsupported fault `critical-section-contention-unimplemented` before
changing the process object or guest bytes because scheduler-backed blocking and
wake-up are not implemented. Uninitialized, inconsistent, and wrong-owner calls
also fail with typed runtime faults.

`DeleteCriticalSection` is observed by the oracle but not registered in this
checkpoint because the current BGI continuation does not reach it. The same is
true for try-enter, spin-count mutation, and scheduler-visible contention.

The representative public semantic test covers initialization, exact bytes,
same-thread recursion, partial and final release, LastError preservation, and a
second controlled thread's contention fault:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

## Generic BGI continuation

The capability profile reuses the existing generic runtime mode:

```powershell
npm run check:runtime:bgi -- --profile kernel32-critical-section-synchronization
```

It registers 16 Kernel32 contracts, validates the frozen 17-call semantics,
returns the four later `EncodePointer` occurrences, and initializes all 14
contiguous BGI critical sections. No enter or leave call is reached in this
bounded continuation; those semantics are fixed by the native evidence and the
public controlled test.

| Evidence | Result |
|---|---:|
| Initial reflection | 389 pages, report generation 27 |
| Frozen compatibility generation | `30` |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| Completed generic calls | 35 |
| Critical-section initializations | 14 |
| CPU writeback batches／dirty pages | 36／65 |
| CPU-changed bytes | 344 |
| Host delta batches | 17 |
| Final synchronized generation | 80 |
| Report bytes | 167,719 |
| Report SHA-256 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |

The frozen 140,660-byte report remains byte-identical with SHA-256
`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`.
The separate Step 10 generic report also remains 96,244 bytes with SHA-256
`a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa`.

## Next structured stop

After the fourteenth initialization returns, BGI reaches instruction 575 at:

```text
KERNEL32.dll!DecodePointer(0xEC010003)
endpoint ID:      88
endpoint address: 0x7F000570
IAT address:      0x100D1208
return address:   0x100A5924
```

Rust returns `endpoint-contract-missing` in capability cluster
`kernel32-pointer-encoding`; it does not decode the controlled token or invoke
the associated FLS endpoint.

The follow-on `kernel32-pointer-decoding` checkpoint is now verified in
[bgi-pointer-decoding.md](bgi-pointer-decoding.md). It preserves this report,
resolves only the registered token, and reaches dynamic `FlsAlloc(0x100A5666)`.
General FLS calls, arbitrary synchronization, CPU-backed multithreading, and BGI
browser execution remain outside this checkpoint.
