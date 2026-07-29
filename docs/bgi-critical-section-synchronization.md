# BGI critical-section synchronization

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-critical-section-synchronization`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous generic checkpoint: `bgi-generic-runtime-reentry`
- First unsupported capability: `kernel32-pointer-decoding`

## Purpose

The historical generic BGI session first stopped at
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

Private `runtime/synchronization.rs::RuntimeSynchronization` owns a bounded map
of at most 32,768 critical sections keyed by their nonzero, 4-byte-aligned guest
storage address. Each record owns the spin count, optional controlled thread ID,
and recursion count. `RuntimeKernel`（compatibility alias `RuntimeState`）only
coordinates process／thread／scheduler preflight and the owner-local commit.
Native thread IDs and kernel synchronization handles are not imported.

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
contiguous BGI critical sections. That historical bounded continuation stops
before an enter or leave call; those semantics are fixed by the native evidence
and the public controlled test.

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

## Historical primary idle-cycle reachability

The historical `kernel32-enter-critical-section` profile consumed the existing
synchronization contract inside the reached timer-driven idle cycle:

```powershell
npm run check:runtime:bgi -- --profile kernel32-enter-critical-section
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-enter-critical-section `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
```

The profile completes three balanced acquisitions and releases before the next
`SetWaitableTimer` arm. The pairs are two uses of guest storage `0x1014C8D0`
followed by one use of `0x100FF820`; all run on controlled thread `8193`.
Node and Chrome agree on the ordered semantic digest and final guest memory.
The Rust `RuntimeSynchronization` object remains the only Windows-visible
critical-section authority, and the browser owns no Windows synchronization
state. This is historical reachability evidence for the existing handler, not a
claim for cross-thread blocking, APC delivery, or general critical-section
scheduling. Exact historical and current stop metadata remain in
[current-checkpoint.json](current-checkpoint.json).

The historical scheduler continuation is documented in
[bgi-primary-main-loop-progression.md](bgi-primary-main-loop-progression.md);
the historical main-thread release checkpoint is
[bgi-leave-critical-section.md](bgi-leave-critical-section.md).

## Post-clock balanced roundtrip

The `kernel32-enter-critical-section-post-clock` profile resumes after the
post-heap multimedia-clock sample and consumes the contiguous synchronization
batch through the first different owned endpoint. It records one
`EnterCriticalSection` and the matching `LeaveCriticalSection` on guest storage
`0x20020E08`, both executed by controlled thread `8192`. The calls preserve EAX
and LastError, publish the 24-byte locked and unlocked representations through
the existing guest-memory transaction, and leave the critical section balanced.

The release completes before the scheduler selects thread `8193`; the call
provenance therefore remains the executing thread rather than the thread
selected at the terminal stop. Node and Chrome agree on the ordered call
digest, both guest-state publications, the scheduler handoff, and final guest
memory. `RuntimeSynchronization` remains the only Windows-visible authority,
and TypeScript retains only non-owning execution evidence. The bounded batch
then stops before `WaitForSingleObjectEx(0x57010002, 10, TRUE)`. Exact current
instructions, endpoint identity, report identities, and the follow-on profile
are authoritative in [current-checkpoint.json](current-checkpoint.json).

The follow-on post-critical-section wait profile consumes that existing timer
transaction and reaches the next critical-section acquisition without adding a
second synchronization or scheduler authority. Its stable wait contract is
documented in
[bgi-wait-for-single-object-ex.md](bgi-wait-for-single-object-ex.md).

The `kernel32-enter-critical-section-post-wait` profile then consumes the
contiguous synchronization block through its first non-critical-section owned
endpoint. Controlled thread `8193` completes three balanced acquire/release
roundtrips: two on `0x1014C8D0`, followed by one on `0x100FF820`. Every call
preserves EAX and LastError and publishes the existing 24-byte locked or
unlocked representation through the Rust guest-memory transaction. Node and
Chrome agree on the six ordered completions, both logical identities, final
guest memory, and the terminal `SetWaitableTimer` frame.

This batch adds no new synchronization authority or handler. It is recurrence
evidence for `RuntimeSynchronization`, including owner-thread publication and
balanced release before the next timer arm. Browser code records only
non-owning completion evidence. Exact call positions, generations, terminal
arguments, and report identities remain manifest-owned.

This evidence does not add cross-thread critical-section blocking, fairness,
abandonment, timeout, or APC semantics.

## Historical next structured stop

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
