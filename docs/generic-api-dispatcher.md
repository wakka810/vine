# Generic API dispatcher and RuntimeState foundation

## Status

- Status: review Step 2／3, Step 4／10, critical-section, pointer-decoding, FLS-allocation, private-heap-allocation, FLS-value, InterlockedIncrement, second private-heap allocation, standard-handle/file-type, and SetHandleCount capabilities implemented
- Date: 2026-07-13
- Runtime evidence base: `bgi-seventeenth-win32-call`
- Runtime-kernel checkpoint: `bgi-set-handle-count`
- Scope: frozen 17-call equivalence plus 61 calls through one persistent generic runtime session

## Dispatcher boundary

`vine-core` owns the synchronous API boundary:

- `ApiEndpointIdentity` and `ApiContract`
- `AbiSchema`, argument direction, pointer／string／struct／name-or-ordinal constraints, return schema, and stack cleanup
- `ApiDispatcher`, `ApiCallFrame`, and `ExecutionSession`
- state-driven Kernel32 handlers, typed outcomes, and structured `ApiFault`

The implemented path is:

```text
owned import or dynamic endpoint
  -> binding-scoped endpoint lookup and optional bound-IAT validation
  -> contract lookup
  -> checked x86 frame decode
  -> pointer / name-or-ordinal access validation
  -> RuntimeState handler
  -> contract-owned guest writes
  -> EAX / EDX / EFLAGS / ESP / EIP / LastError transition
```

An owned endpoint without a contract returns `endpoint-contract-missing` with exact endpoint identity. Contract-invalid output writes are rejected before memory mutation. `GetProcAddress` models its second argument as `NameOrOrdinal`: name pointers are checked and bounded, while ordinals reach an explicit `get-proc-address-ordinal-unsupported` fault rather than an accidental memory fault.

## Migrated contracts

| API | Occurrences | RuntimeState behavior |
|---|---:|---|
| `GetSystemTimeAsFileTime` | 1 | writes process wall clock |
| `GetCurrentProcessId` | 1 | returns process identity |
| `GetCurrentThreadId` | 2 | returns scheduler current thread |
| `GetTickCount` | 1 | returns process monotonic clock |
| `QueryPerformanceCounter` | 1 | writes process performance counter |
| `GetStartupInfoW` | 2 | writes process startup profile |
| `HeapSetInformation` | 1 | enables the reached process heap policy |
| `HeapCreate` | 1 | allocates deterministic logical heap metadata |
| `GetModuleHandleW` | 2 | bounded UTF-16 loaded-module lookup |
| `GetProcAddress` | 4 | bounded ASCII dynamic-endpoint lookup |
| `TlsAlloc` | 1 | allocates a process TLS index |
| `TlsSetValue` | 1 | stores a current-thread endpoint value |
| `EncodePointer` | 6 | allocates stable process pointer tokens |
| `DecodePointer` | 2 | resolves only registered process tokens to their original source |
| `InitializeCriticalSectionAndSpinCount` | 14 | creates process-owned synchronization state and exact x86 bytes |
| `EnterCriticalSection` | 2 | acquires or recursively enters for the current thread |
| `LeaveCriticalSection` | 2 | decrements recursion and releases at zero |
| `FlsAlloc` | 1 | allocates a bounded process-owned logical FLS slot and retains its callback |
| `FlsSetValue` | 1 | stores an opaque value in the selected thread's allocated FLS slot |
| `HeapAlloc` | 2 | allocates the first two reached zeroed blocks through transactional guest-memory backing |
| `InterlockedIncrement` | 3 | performs a checked wrapping LONG update in guest memory |
| `GetStdHandle` | 3 | returns versioned process-owned logical input／output／error identities |
| `GetFileType` | 2 | classifies registered logical stdin as character and stdout as pipe |

All 23 distinct APIs use `ApiDispatcher` and `dispatch_kernel32_api`. The frozen
17 occurrences remain the compatibility anchor; the capability profile completes
58 BGI calls, including six `EncodePointer` occurrences, 16 critical-section
initializations, two `DecodePointer` calls, dynamic `FlsAlloc`／`FlsSetValue`,
two `HeapAlloc`, three `InterlockedIncrement` calls, and later existing
critical-section／thread／startup occurrences plus three `GetStdHandle` and two
`GetFileType` calls.
Leave semantics remain fixed by native evidence and the controlled public test.
The title harness still performs callsite provenance checks, automatic
v86 dirty-page synchronization, and evidence serialization around the common
semantic path.

## Checkpoint manifest and runner

`fixtures/checkpoints/bgi-startup-win32.json` is the version 1 data source for the
initial trap and all 17 cumulative call checkpoints. Each entry extends the prior
entry and records dispatch identity／occurrence, exact arguments, result, guest
writes, next stop, instruction count, reflection digest, and report digest.

Both the Node evidence runner and Rust probe load the same manifest and select a
checkpoint by ID. The Node runner builds declared imports from distinct manifest
dispatches, executes an ordered handler registry in one generic loop, and selects
an evidence serializer from the same call count. Rust replaces the former 17
boolean session fields with one validated ordered selection. The `package.json`
commands retain their stable names but all use `--checkpoint ID`; API-specific
dispatch flags no longer exist.

Those numbered commands are compatibility adapters for the immutable historical
reports. Step 10 adds a separate `--generic-runtime-reentry` mode. It registers
the BGI image and 14 runtime-system dependencies in the process module graph,
binds all 330 runtime exports, and constructs one persistent `RuntimeState`,
`ApiDispatcher`, and `ExecutionSession`. At each trap it scans all writable PTE
Dirty bits, sends binary `VINEWB01` writeback, dispatches by owned endpoint
identity, applies binary `VINEDL01` host deltas, and resumes from a generic
register completion. It has no API-owned reconciliation list, request type, or
receipt type.

The manifest validator is public and does not require BGI:

```powershell
npm run check:checkpoint-manifest
```

Each cumulative capability is enabled by a capability-named profile, not a
numbered or API-specific request／receipt mode:

```powershell
npm run check:runtime:bgi -- --profile kernel32-critical-section-synchronization
npm run check:runtime:bgi -- --profile kernel32-pointer-decoding
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-allocation
npm run check:runtime:bgi -- --profile kernel32-private-heap-allocation
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-value
npm run check:runtime:bgi -- --profile kernel32-interlocked-increment
npm run check:runtime:bgi -- --profile kernel32-private-heap-second-allocation
npm run check:runtime:bgi -- --profile kernel32-standard-input-handle
npm run check:runtime:bgi -- --profile kernel32-file-type
npm run check:runtime:bgi -- --profile kernel32-standard-output-handle
```

## RuntimeState authority

One `RuntimeState` is created at cumulative-session start. It owns:

- process ID, address-space control ranges, clock, startup profile, heap policy, logical heaps, loaded modules, and TLS indices;
- current thread ID, per-thread LastError, TLS／FLS values, and callback frame stack;
- dynamic endpoints, deterministic encoded pointers, pending host requests, and scheduler events.
- process-owned critical sections keyed by guest address, with spin count,
  controlled thread ownership, and recursion state.
- bounded process-owned FLS slots keyed by logical index, with retained optional
  guest cleanup callbacks.
- bounded private-heap backing and reached guest allocation metadata, with
  `GuestMemory` as byte and mapping authority.
- the reached process standard input／output identities, without importing host handles.

The Rust profile preloads the logical runtime KERNEL32 module and installed FLS endpoints. `HeapCreate` and `TlsAlloc` allocate through runtime-owned deterministic allocators. `GetModuleHandleW` and `GetProcAddress` resolve state by canonical module and exact export name. No handler receives a result value from the JSON protocol.

Protocol request values remain only as private evidence assertions. A mismatch fails before return, but cannot determine the API result. Native heap handles, module pointers, export addresses, TLS indices, and process-cookie values are never imported into guest state.

## Thread and scheduler state

LastError is current-thread state and all 23 contracts explicitly preserve it. Callback frames record thread identity and entry LastError, enforce strict LIFO return, and keep callback depth synchronized.

A host request changes `Runnable` or `InCallback` to `WaitingHost`. Completion restores the saved state and enqueues `RuntimeSchedulerEvent::HostCompleted`. Critical-section recursion is synchronous for the current thread; cross-thread contention returns structured `critical-section-contention-unimplemented` before mutation because scheduler-backed blocking is not implemented. This remains a scheduler foundation: no current BGI contract performs a browser asynchronous request.

## Verification

Representative semantic contracts:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

The twelve tests cover all 23 APIs, the five original shapes, output writes,
LastError, heap／module／endpoint／TLS state, bounded string lookup, explicit
ordinal rejection, callback LIFO, host completion events, exact critical-section
bytes, same-thread recursion, release, structured contention, dynamic/static ID
collision, executable FLS callbacks, bounded FLS allocation, selected-thread
FLS value isolation, the reached transactional private-heap allocation, and
wrapping checked LONG updates, and versioned two-block heap allocation with
mutation-free later-variant rejection, plus the process-owned standard input
identity and unreached-selector rejection.
The file-type test additionally fixes registered-stdin classification and
unknown-logical-handle rejection.
The stdout test fixes the versioned two-selector profile, logical identity
reuse, standard-error rejection, and mutation-free output-file-type rejection.

The cumulative private-corpus command is:

```powershell
npm run check:runtime:bgi -- --checkpoint bgi-seventeenth-win32-call
```

Before and after migration it reaches the same trap after 238 instructions. The report remains 140,660 bytes with SHA-256 `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`; reflection SHA-256 remains `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`.

The Step 10 semantic run is separate:

```powershell
npm run check:runtime:bgi -- --profile baseline
```

It validates the frozen 17 calls, returns four more `EncodePointer` calls through
the same contract, and stops after 21 completed calls at instruction 266 on
`KERNEL32.dll!InitializeCriticalSectionAndSpinCount(0x100FF630, 4000)`. The run
discovers 38 changed pages／268 changed bytes in 22 automatic writeback batches,
applies 3 host deltas, and advances generation `30 -> 52`. Its 96,244-byte report
has SHA-256 `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` in two
consecutive runs. The frozen report is unchanged, so semantic validation and
historical evidence serialization are now independent.

The critical-section capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-critical-section-synchronization
```

It completes 35 calls, including 14 contiguous
`InitializeCriticalSectionAndSpinCount(..., 4000)` calls, through 36 automatic
writeback batches／65 dirty pages／344 changed bytes and 17 host deltas. Final
generation is `80`. The 167,719-byte report has SHA-256
`cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69`
and stops at instruction 575 on `DecodePointer(0xEC010003)`. Full native and
runtime evidence is in [bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md).

The pointer-decoding capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-pointer-decoding
```

It resolves controlled token `0xEC010003` to its registered source
`0x7F020000`, preserves LastError, and performs no guest write. The immediate
dynamic call adds one 4-byte CPU writeback, so the profile completes 36 calls,
uses 37 writeback batches／66 dirty pages／348 changed bytes, and ends at
generation `81`. Its 171,356-byte report has SHA-256
`5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e`
and stops at instruction 577 on dynamic endpoint ID `1`／`FlsAlloc`. Full
evidence is in [bgi-pointer-decoding.md](bgi-pointer-decoding.md).

The FLS-allocation capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-allocation
```

It returns logical index `0` for callback `0x100A5666`, preserves LastError,
and emits no API guest write or host delta. The profile completes 37 calls,
uses 38 writeback batches／68 dirty pages／399 changed bytes, and ends at
generation `82`. Its 175,123-byte report has SHA-256
`26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c`
and stops at instruction 618 on static endpoint ID `75`／
`HeapAlloc(0x68010001,8,0x214)`. Full evidence is in
[bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md).

The private-heap-allocation capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-allocation
```

It accepts only `HeapAlloc(0x68010001,HEAP_ZERO_MEMORY,0x214)`, returns guest
pointer `0x20000000`, and applies the existing allocator's reserve/commit/zero
delta to the v86 mirror. The profile completes 39 calls, including a second
`DecodePointer`, uses 40 writeback batches／70 dirty pages／415 changed bytes and
18 host deltas, and ends at generation `87`. Its 183,102-byte report has
SHA-256 `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4`
and stops at instruction 644 on dynamic endpoint ID `3`／
`FlsSetValue(0,0x20000000)`. Full evidence is in
[bgi-private-heap-allocation.md](bgi-private-heap-allocation.md).

The FLS-value capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-value
```

It stores `0x20000000` in logical slot `0` for controlled thread `8192`, returns
BOOL `1`, preserves LastError, and emits no guest write. Existing contracts then
complete a second `GetModuleHandleW` and the first reached
`EnterCriticalSection`. The profile completes 42 calls, uses 43 writeback
batches／75 dirty pages／497 changed bytes and 19 host deltas, and ends at
generation `91`. Its 195,821-byte report has SHA-256
`f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f`
and stops at instruction 707 on static endpoint ID `124`／
`InterlockedIncrement(0x100F08E0)`. Full evidence is in
[bgi-fiber-local-storage-value.md](bgi-fiber-local-storage-value.md).

The InterlockedIncrement capability profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-interlocked-increment
```

It returns three reached wrapping increments with exact before/after evidence:
`0 -> 1` at `0x100F08E0`, `1 -> 2` at `0x100F0F70`, and `0 -> 1` at
`0x100F0EBC`. Each required four-byte guest write is applied through
`GuestMemory` and reflected to v86 before resuming. Existing contracts continue
through 50 calls, 51 writeback batches／86 dirty pages／667 changed bytes and 26
host deltas, ending at generation `106`. Its 234,775-byte report has SHA-256
`4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643`
and stops at instruction 934 on the already measured
`HeapAlloc(0x68010001,HEAP_ZERO_MEMORY,0x800)` variant. Full evidence is in
[bgi-interlocked-increment.md](bgi-interlocked-increment.md).

The second private-heap-allocation profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-second-allocation
```

It replaces only the profile's HeapAlloc implementation ID with the versioned
first-two-reached contract, preserving the earlier v1 report. Allocation ID `2`
is zeroed at `0x20000218` while allocation ID `1` remains live. The existing
unsealed heap owner emits a dirty-only generation `106 -> 107` delta without a
new mapping. The profile completes 51 calls, uses 52 writeback batches／89 dirty
pages／906 changed bytes and 27 host deltas, and ends at generation `108`. Its
239,789-byte report has SHA-256
`fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8`
and stops at instruction 1395 on `GetStdHandle(STD_INPUT_HANDLE)`. Full evidence
is in [bgi-private-heap-second-allocation.md](bgi-private-heap-second-allocation.md).

The standard-input-handle profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-input-handle
```

It accepts only `STD_INPUT_HANDLE`, returns logical handle `0x53010001`,
preserves LastError, and emits no API guest write or host delta. The profile
completes 52 calls, uses 53 writeback batches／90 dirty pages／911 changed bytes
and 27 host deltas, and ends at generation `109`. Its 243,749-byte report has
SHA-256 `5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e`
and stops at instruction 1403 on `GetFileType(0x53010001)`. Full evidence is in
[bgi-standard-input-handle.md](bgi-standard-input-handle.md).

The file-type profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-file-type
```

It classifies only logical stdin `0x53010001` as `FILE_TYPE_CHAR (2)`, then
returns the existing 15th critical-section initialization. The profile completes
54 calls, uses 55 writeback batches／94 dirty pages／938 changed bytes and 28 host
deltas, and ends at generation `112`. Its 252,994-byte report has SHA-256
`dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf`
and stops at instruction 1439 on `GetStdHandle(STD_OUTPUT_HANDLE)`. Full evidence
is in [bgi-file-type.md](bgi-file-type.md).

The standard-output-handle profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-output-handle
```

It uses the same `kernel32.get-std-handle.v2` semantic contract with the
versioned `kernel32-standard-output-v1` policy. Logical stdout is `0x53010002`;
standard error and output file type remain unsupported. The
profile completes 55 calls, uses 56 writeback batches／95 dirty pages／943 changed
bytes and 28 host deltas, and ends at generation `113`. Its 257,878-byte report
has SHA-256
`564c3425f01b35e86717b68335b18f8bcd3d30d996717a1d0848028104cf0a2b`
and stops at instruction 1447 on `GetFileType(0x53010002)`. Full evidence is in
[bgi-standard-output-handle.md](bgi-standard-output-handle.md).

The standard-output-file-type profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-output-file-type
```

It uses the same `kernel32.get-file-type.v2` handler with the versioned
`kernel32-standard-output-file-type-v1` policy. Registered logical stdout alone
returns `FILE_TYPE_PIPE (3)` with LastError preserved and no guest write. The
profile completes 57 calls, uses 58 writeback batches／99 dirty pages／966 changed
bytes and 29 host deltas, and ends at generation `116`. Its 267,223-byte report
has SHA-256
`0bc3770b1da0af519e4065021c11cc423427b9855d9ceedac6d9b00053837301`
and stops at instruction 1484 on `GetStdHandle(STD_ERROR_HANDLE)`. Full evidence
is in [bgi-standard-output-file-type.md](bgi-standard-output-file-type.md).

The standard-error-handle profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-error-handle
```

It uses the same `kernel32.get-std-handle.v2` handler with the versioned
`kernel32-standard-error-v1` policy. Logical stderr is `0x53010003`; its file
type remains unsupported. The profile completes 58 calls, uses 59 writeback
batches／100 dirty pages／971 changed bytes and 29 host deltas, and ends at
generation `117`. Its 271,184-byte report has SHA-256
`bfa1da3badbbd96ed5c282bd21c3bb0041fa06834dbad38fe0af32e116432c2f`
and stops at instruction 1492 on `GetFileType(0x53010003)`. Full evidence is in
[bgi-standard-error-handle.md](bgi-standard-error-handle.md).

The standard-error-file-type profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-error-file-type
```

It reuses `kernel32.get-file-type.v2` with the versioned
`kernel32-standard-error-file-type-v1` policy. Registered logical stderr alone
returns `FILE_TYPE_PIPE (3)` with LastError preserved and no guest write. The
profile completes 60 calls, uses 61 writeback batches／104 dirty pages／991
changed bytes and 30 host deltas, and ends at generation `120`. Its 280,475-byte
report has SHA-256
`ce9d4271db746b1b7d377bb3e42a861f9c611367e5fcfefba0f19b35fca5aee1`
and stops at instruction 1516 on `SetHandleCount(32)`. Full evidence is in
[bgi-standard-error-file-type.md](bgi-standard-error-file-type.md).

The SetHandleCount profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-handle-count
```

It uses `kernel32.set-handle-count.v1` with the versioned
`kernel32-set-handle-count-v1` policy. Count `32` returns unchanged with
LastError preserved and no runtime／memory mutation. The profile completes 61
calls, uses 62 writeback batches／105 dirty pages／992 changed bytes and 30 host
deltas, and ends at generation `121`. Its 284,215-byte report has SHA-256
`91be62c168dd5296fa549a8bf553975853dfef506bcb3ce7670a7b8843f82f3b`
and stops at instruction 1526 on `GetCommandLineA()`. Full evidence is in
[bgi-set-handle-count.md](bgi-set-handle-count.md).

## Deliberate exclusions

- API-specific request／receipt schemas remain only in the frozen numbered evidence adapters; the Step 10 path does not use them.
- The runtime has controlled process／thread, wait／message, mapped-module, guest-import, and DLL process lifecycle foundations; general CPU-context-backed threads, wait variants, dynamic load／unload, and TLS／thread DLL callbacks remain absent.
- Private heaps have bounded guest-memory backing and the first two reached
  zeroed `HeapAlloc` variants enabled by policy; other sizes/flags,
  free/destroy/realloc dispatch, and broad allocator policy remain absent.
- FLS get/free/callback teardown and TLS free/thread lifecycle, critical-section
  deletion／contention blocking, and arbitrary export pages are absent. Only the
  reached FLS value store is implemented.
- Host completion is connected to controlled Worker／browser services; general
  VFS, input, graphics, audio, and storage adapters remain absent.
- `GuestMemoryTransaction` uses a changed-state／frame before-image journal;
  Rust byte authority, automatic v86 writeback, and binary transport are now
  fixed by ADR-003 and the memory-writeback checkpoint.
- `InterlockedIncrement` is implemented only for the checked 32-bit LONG shape
  in the single cooperative Runtime Worker. Adjacent interlocked APIs and
  cross-worker shared-memory atomicity are absent.
- The generic session owns only the reached standard input／output／error
  identities, stdin character classification, and stdout／stderr pipe
  classifications plus the policy-enabled `SetHandleCount(32)` no-op contract.
  General handle-table sizing, `SetStdHandle`, close behavior, broader file type,
  console semantics, and file I/O remain absent. Its first unsupported
  operation is `GetCommandLineA()`; bespoke
  checkpoint expansion stays paused.

## Next checkpoint

The next proof is `kernel32-command-line-a` at `GetCommandLineA()`. It must first
fix the native pointer relation, returned bytes, LastError, and immediate
consumer, then connect the existing process-parameter authority without
importing a host pointer or broad environment semantics.
Existing import, bootstrap, DLL lifecycle,
bidirectional memory sync, frozen 17-call report, generic 96,244-byte report,
critical-section 167,719-byte report, pointer-decoding 171,356-byte report,
FLS-allocation 175,123-byte report, private-heap 183,102-byte report, FLS-value
195,821-byte report, InterlockedIncrement 234,775-byte report, second-heap
239,789-byte report, standard-input 243,749-byte report, and file-type
252,994-byte report remain historical artifact identities. Current semantic
regression uses field assertions; the replayed policy-bearing standard-output
report is 257,878 bytes. The stdout-file-type and stderr-handle reports are
267,223 and 271,184 bytes respectively; the current stderr-file-type report is
280,475 bytes and the current SetHandleCount report is 284,215 bytes.
