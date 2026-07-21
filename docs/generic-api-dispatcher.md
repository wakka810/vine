# Generic API dispatcher and RuntimeState foundation

<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [`current-checkpoint.json`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
<!-- END CURRENT CHECKPOINT REFERENCE -->

## Status

本書はdispatcher境界とRust runtime authorityを記述する。現行profile、contract一覧、
call数、next stopは上のmanifest参照を正本とし、ここへ複製しない。

## Dispatcher boundary

`vine-core` owns the synchronous API boundary:

- `ApiEndpointIdentity` and `ApiContract`
- `AbiSchema`, argument direction, fixed／bounded-dynamic pointer, string／struct／name-or-ordinal constraints, return schema, and stack cleanup
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

## Frozen contract-inventory snapshot

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->

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
| `EncodePointer` | 19 | allocates stable process pointer tokens |
| `DecodePointer` | 4 | resolves only registered process tokens to their original source |
| `InitializeCriticalSectionAndSpinCount` | 17 | creates process-owned synchronization state and exact x86 bytes |
| `EnterCriticalSection` | 5 | acquires or recursively enters for the current thread |
| `LeaveCriticalSection` | 5 | decrements recursion and releases at zero |
| `FlsAlloc` | 1 | allocates a bounded process-owned logical FLS slot and retains its callback |
| `FlsSetValue` | 1 | stores an opaque value in the selected thread's allocated FLS slot |
| `HeapAlloc` | 8 | allocates five zeroed and three flags-zero deterministic-uninitialized reached blocks through transactional guest-memory backing |
| `HeapFree` | 1 | retires one exact live allocation through transactional guest-memory backing |
| `InterlockedIncrement` | 5 | performs a checked wrapping LONG update in guest memory |
| `GetStdHandle` | 3 | returns versioned process-owned logical input／output／error identities |
| `GetFileType` | 3 | classifies registered logical stdin as character and stdout／stderr as pipes |
| `SetHandleCount` | 1 | returns the reached compatibility-policy count without resizing host state |
| `GetCommandLineA` | 1 | returns the stable process-owned ANSI view derived at bootstrap |
| `GetEnvironmentStringsW` | 1 | acquires one process-owned copy lease for the controlled UTF-16 environment block |
| `FreeEnvironmentStringsW` | 1 | retires only the exact active copy lease and returns BOOL success |
| `WideCharToMultiByte` | 4 | performs host-locale-independent Windows-932 query/conversion shapes |
| `GetLastError` | 23 | reads selected-thread LastError without mutation |
| `TlsGetValue` | 23 | reads the selected thread's allocated TLS value and applies Win32 LastError semantics |
| `FlsGetValue` | 23 | reads initialized selected-thread FLS storage and applies Win32 LastError semantics |
| `SetLastError` | 23 | writes the full selected-thread LastError value without guest-memory mutation |
| `GetACP` | 1 | returns the policy-owned active ANSI code page without mutation |
| `GetCPInfo` | 1 | writes the policy-owned 18-byte active code-page fields |
| `MultiByteToWideChar` | 6 | queries and writes checked Windows-932 UTF-16 output without host-locale input |
| `GetStringTypeW` | 1 | writes checked policy-enabled CTYPE1 WORD classifications without host-locale input |
| `LCMapStringW` | 4 | queries and writes checked ja-JP lowercase／uppercase output without host-locale input |
| `InterlockedDecrement` | 2 | performs a checked aligned wrapping LONG decrement in guest memory |
| `GetModuleFileNameA` | 1 | copies the bootstrap-owned main-module ANSI path to checked output |
| `IsProcessorFeaturePresent` | 2 | returns explicit policy-owned processor-feature presence without host discovery |
| `SetUnhandledExceptionFilter` | 1 | replaces the process-owned nullable top-level exception-filter pointer |
| `HeapSize` | 1 | returns exact requested size from Rust-owned live-allocation metadata |

All 43 distinct APIs use `ApiDispatcher` and `dispatch_kernel32_api`. The frozen
17 occurrences remain the compatibility anchor; the capability profile completes
210 BGI calls. Current occurrence counts are listed above rather than encoded as
handler branches; caller RVA, argument sequence, and next stop remain evidence
data. The final four calls add exact `HeapSize`, two existing `EncodePointer`
occurrences, and one existing `LeaveCriticalSection` continuation.
Leave semantics remain fixed by native evidence and the controlled public test.
The source-v86 Node oracle harness still performs callsite provenance checks and
adapter-specific dirty-page synchronization around the common semantic path.
The production Browser Worker receives only generic changed-page batches from
`ExecutionEngine`.

<!-- END FROZEN HISTORICAL EVIDENCE -->

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

Historical report bytes and hashes remain capture-time artifact identities. A
replay reports whether they match but gates on the exact call sequence, semantic
results, writes, instruction count, and next stop instead of making JSON layout a
production ABI. See [evidence-classification.md](evidence-classification.md).

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

## Runtime kernel authority

One `RuntimeKernel` is created at cumulative-session start; `RuntimeState` is
its compatibility type alias. The kernel is the aggregate authority, but it does
not keep subsystem collections as flat fields or mutate their maps directly:

- private `RuntimeProcessState` owns process ID, lifecycle, clock, startup
  profile, heap policy, standard handles, image／directory policy, command line,
  environment authority, and bounded environment-copy lifetime;
- private `RuntimeThreads` owns thread records, per-thread LastError, TLS／FLS
  values, callback frames, process-wide TLS／FLS registries, and their allocators;
- private scheduler／wait／message owners hold current selection, lanes, events,
  host requests, wait registrations, and per-thread message FIFOs;
- private object, heap, module, synchronization, and registry owners hold their
  respective identities, maps, allocators, and lifecycle plans;
- `ProcessAddressSpaceAllocator` owns address-range identity, while
  `GuestMemory` remains byte and mapping authority.

The kernel exposes the existing API surface and coordinates owner-local
`plan -> preflight -> commit` transactions. Existing diagnostic JSON is emitted
through borrowed compatibility views; it is not a second mutable authority.

The Rust profile preloads the logical runtime KERNEL32 module and installed FLS endpoints. `HeapCreate` and `TlsAlloc` allocate through runtime-owned deterministic allocators. `GetModuleHandleW` and `GetProcAddress` resolve state by canonical module and exact export name. No handler receives a result value from the JSON protocol.

Protocol request values remain only as private evidence assertions. A mismatch fails before return, but cannot determine the API result. Native heap handles, module pointers, export addresses, TLS indices, and process-cookie values are never imported into guest state.

## Thread and scheduler state

LastError is current-thread state and all 33 contracts declare an explicit preserve-or-handler policy. Callback frames record thread identity and entry LastError, enforce strict LIFO return, and keep callback depth synchronized.

A host request changes `Runnable` or `InCallback` to `WaitingHost`. Completion restores the saved state and enqueues `RuntimeSchedulerEvent::HostCompleted`. Private `runtime/scheduler.rs::RuntimeScheduler` owns the lane、event queue、pending request table、and monotonic request ID. Start／completion plan lane and identity before targeted commit; thread exit uses a thread-scoped cancellation plan. The root `registries.hostRequests`／`nextHostRequestId` report fields are a borrowed compatibility view, not duplicate state. Critical-section recursion is synchronous for the current thread; cross-thread contention returns structured `critical-section-contention-unimplemented` before mutation because scheduler-backed blocking is not implemented. This remains a scheduler foundation: no current BGI contract performs a browser asynchronous request.

Private `runtime/registries.rs::RuntimeRegistries` owns dynamic endpoint records、encoded-pointer token/source
indexes、pointer policy and token allocator. Endpoint registration and pointer allocation are mutation-free plans
followed by targeted commits; root pointer-policy／source／counter report fields are borrowed compatibility views.
The neighboring `registries.hostRequests` report field remains scheduler-owned and is not registry state.

## Frozen checkpoint verification

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->

Representative semantic contracts:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

The semantic tests cover all registered APIs, the five original shapes, fixed and
bounded-dynamic output writes,
LastError, heap／module／endpoint／TLS state, bounded string lookup, explicit
ordinal rejection, callback LIFO, host completion events, exact critical-section
bytes, same-thread recursion, release, structured contention, dynamic/static ID
collision, executable FLS callbacks, bounded FLS allocation, selected-thread
FLS value isolation, the reached transactional private-heap allocation, and
wrapping checked LONG updates, and versioned two-block heap allocation with
mutation-free later-variant rejection, plus the process-owned standard input
identity and unreached-selector rejection, stable ANSI command-line identity,
UTF-16 double-NUL environment bytes, one-active-copy lifetime, exact paired
free, and mutation-free concurrent／foreign／duplicate rejection.
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

The GetCommandLineA profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-command-line-a
```

It derives one process-owned ASCII view from the existing UTF-16
process-parameter authority. The stable pointer is distinct from the wide
pointer, preserves LastError, and writes no guest bytes during the call. The
profile completes 62 calls, ends at generation `122`, and stops at instruction
1536 on `GetEnvironmentStringsW()`. Full evidence is in
[bgi-command-line-a.md](bgi-command-line-a.md).

The current wide-environment profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-environment-strings-w
```

It installs an empty UTF-16 double-NUL authority without importing the host
environment. `GetEnvironmentStringsW` activates one distinct copy lease;
`FreeEnvironmentStringsW` validates and retires that exact pointer. Concurrent
acquisition, foreign pointer, and duplicate free stop before mutation. The BGI
profile completes 63 calls, uses 64 writeback batches／108 dirty pages／1,015
changed bytes and 30 host deltas, and ends at generation `123`. Its 291,793-byte
report has SHA-256
`96cb98f09a1d2733442d38ce2736c15c38fd87f3f4fa082e020d623c3fdb6013`
and stops at instruction 1558 on `WideCharToMultiByte`. Full evidence is in
[bgi-environment-strings-w.md](bgi-environment-strings-w.md).

The current Windows-932 conversion profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-wide-char-to-multi-byte
```

It fixes `CP_ACP` to Windows-932 without importing the host locale and supports
bounded positive／`-1` UTF-16 input, query and conversion, complete-unit partial
output, system-default `?` substitution, optional used-default BOOL, and exact
LastError behavior. `ApiDynamicPointerConstraint` keeps variable-length output
inside dispatcher validation and atomic guest writes. The BGI profile completes
64 calls, uses 65 writeback batches／109 dirty pages／1,057 changed bytes and 30
host deltas, and ends at generation `124`. Its 295,762-byte report has SHA-256
`ea0eb0f6a14869ca4e75643f01bd42c8c2e7360584c893e7578f6c21423b56cf`
and stops at instruction 1591 on `HeapAlloc(0x68010001, 0, 1)`. Full evidence is
in [bgi-wide-char-to-multi-byte.md](bgi-wide-char-to-multi-byte.md).

The preceding one-byte private-heap profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-1
```

It reuses `kernel32.heap-alloc.v2` and adds only policy pair `(flags=0,
byteLength=1)`. The allocation is ID `3` at `0x20000A18`, reserves 8 bytes with
deterministic-uninitialized fill, and reaches the following conversion and
environment-copy release. The profile completes 67 calls, uses 68 writeback
batches／114 dirty pages／1,166 changed bytes and 32 host deltas, and ends at
generation `129`. Its 308,984-byte report has SHA-256
`bfecc42fc7b32d21fc86580f81174548e6a26ff2b96f9dee05de51a9e7b4e26e`
and stops at instruction 1687 on the missing `GetLastError` contract. Full
evidence is in [bgi-private-heap-size-1.md](bgi-private-heap-size-1.md).

The current thread LastError profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-last-error
```

`kernel32.get-last-error.v1` reads the selected Rust runtime thread's LastError,
returns the full 32-bit value, preserves it, and performs no runtime or memory
mutation. It needs no compatibility-policy option. The profile completes 68
calls, uses 69 writeback batches／115 dirty pages／1,179 changed bytes and 32 host
deltas, and ends at generation `130`. Its 312,644-byte report has SHA-256
`29b3517baa9bc71d3b34b523f0a57c0cb17a0ff09e1dba9a842a0fc1339516b6`
and stops at instruction 1695 on the missing `TlsGetValue(0)` contract. Full
evidence is in [bgi-get-last-error.md](bgi-get-last-error.md).

The current selected-thread TLS read profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-tls-get-value
```

`kernel32.tls-get-value.v1` validates the process-owned allocated-index registry
and reads only the selected Rust runtime thread's TLS map. Set values return the
stored opaque pointer, allocated-unset values return NULL, and both set
LastError `0`; an invalid index returns NULL with LastError `87`. The handler
does not mutate TLS or guest memory and needs no compatibility-policy option.
The profile completes 69 calls, uses 70 writeback batches／116 dirty pages／1,180
changed bytes and 32 host deltas, and ends at generation `131`. Its 316,360-byte
report has SHA-256
`f14401c55cac1c5247050aad297cc19e35cdd2b4f772ccb842568e0de21ee2d9`
and stops at instruction 1703 on the missing dynamic `FlsGetValue(0)` contract.
Full evidence is in [bgi-tls-get-value.md](bgi-tls-get-value.md).

The current selected-thread FLS read profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-fls-get-value
```

`kernel32.fls-get-value.v1` validates the process-owned slot and selected
thread's FLS-storage lifecycle. Initialized set／unset lanes return the stored
value／NULL with LastError `0`; an invalid index or a new thread without FLS
storage returns NULL with LastError `87`. `FlsAlloc` and `FlsSetValue` initialize
the relevant thread lane. The read changes neither FLS state nor guest memory
and needs no compatibility-policy option. The profile completes 70 calls, uses
71 writeback batches／117 dirty pages／1,183 changed bytes and 32 host deltas,
and ends at generation `132`. Its 320,070-byte report has SHA-256
`f7672697a5f95f709fc242ce1c31b8f95039152a0d06f516aa042c8dafcacbcf`
and stops at instruction 1709 on missing `SetLastError(87)`. Full evidence is in
[bgi-fls-get-value.md](bgi-fls-get-value.md).

The current selected-thread LastError write profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-last-error
```

`kernel32.set-last-error.v1` accepts one full 32-bit value, returns VOID, and
uses `LastErrorPolicy::FromHandler` to update only the selected Rust runtime
thread. It writes no guest byte and needs no compatibility-policy option. The
profile reaches the setter three times while reusing existing TLS/FLS and
critical-section contracts, completes 81 calls, uses 82 writeback batches／132
dirty pages／1,370 changed bytes and 34 host deltas, and ends at generation
`145`. Its 363,609-byte report has SHA-256
`a915d7b91d65e6b1d4805bc4f0ab62d6444573693402bed9fd473518f56f9657`
and stops at instruction 1936 on missing `GetACP()`. Full evidence is in
[bgi-set-last-error.md](bgi-set-last-error.md).

The current active-code-page read profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-acp
```

`kernel32.get-acp.v1` reads only the active ANSI code page exposed by the
versioned compatibility policy. It returns `932`, preserves LastError `87`,
writes no guest byte, and rejects a policy without an active code page before
mutation. The profile completes 82 calls, uses 83 writeback batches／134 dirty
pages／1,430 changed bytes and 34 host deltas, and ends at generation `146`. Its
367,384-byte report has SHA-256
`b897324ff6b85383b2b51be1505e84868e53d9fa5488e5d8f0f4da0ed36b476d`
and stops at instruction 1978 on unsupported
`HeapAlloc(0x68010001,0,0x220)`. Full evidence is in
[bgi-get-acp.md](bgi-get-acp.md).

The current 544-byte allocation profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-544
```

`kernel32-private-heap-size-544-v1` adds only `(flags=0, byteLength=544)`
to the existing `kernel32.heap-alloc.v2` semantics. Allocation ID `4` is
returned at `0x20000A20` with deterministic-uninitialized bytes; the following
BGI consumer overwrites all 544 requested bytes. The profile completes 87 calls,
uses 88 writeback batches／142 dirty pages／2,300 changed bytes and 35 host
deltas, and ends at generation `152`. Its 386,921-byte report has SHA-256
`355c6c2dfc089c8d6f1ca30ea6cbe3b697f4e7afdfa6ed1ec4f2b0ea15593781`
and stops at instruction 4523 on missing `GetCPInfo(932,0x006FF9C0)`. Full
evidence is in [bgi-private-heap-size-544.md](bgi-private-heap-size-544.md).

The current active-code-page information profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-cp-info
```

`kernel32.get-cp-info.v1` accepts only the explicit page equal to the
policy-owned ACP and writes the 18 measured CPINFO field bytes through a checked
output contract. It returns TRUE and preserves LastError `87`; disabled
metadata, other pages, and invalid output ranges stop before mutation. The
profile completes 92 calls, uses 93 writeback batches／148 dirty pages／2,690
changed bytes and 36 host deltas, and ends at generation `158`. Its 407,009-byte
report has SHA-256
`da95600680cdb922a2952a7045ac5ace1b6f2510f5f553773343854e2f2310a1`
and stops at instruction 5793 on missing `MultiByteToWideChar`. Full evidence
is in [bgi-get-cp-info.md](bgi-get-cp-info.md).

The preceding Windows-932 multibyte profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-multi-byte-to-wide-char
```

`kernel32.multi-byte-to-wide-char.v1` implements the six-argument query and
conversion shapes through checked dynamic source/destination spans. It uses
Windows-932 pair decoding plus native private-use mappings for standalone
`0xA0` and `0xFD..0xFF`, preserves LastError on success, and models invalid
parameters and insufficient-buffer prefix writes. The profile completes 94
calls, uses 95 writeback batches／150 dirty pages／2,741 changed bytes and 37
host deltas, and ends at generation `161`. Its 423,814-byte report has SHA-256
`97655782894eca5f4e97a095d1ee3b64619e4f5ecf076c2be91d9b79ca045718`
and stops at instruction 5894 on missing `GetStringTypeW`. Full evidence is in
[bgi-multi-byte-to-wide-char.md](bgi-multi-byte-to-wide-char.md).

The preceding CTYPE1 classification profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-string-type-w
```

`kernel32.get-string-type-w.v1` implements the four-argument explicit-positive-
length `CT_CTYPE1` shape through checked dynamic source/output spans. It
classifies the native-observed ASCII, control, whitespace, Japanese/CJK,
fullwidth/halfwidth, and Windows-932 private-use units without host locale
input. Success preserves LastError; zero length models the measured invalid
parameter result. Negative length, other information types, overlap, and
unimplemented units stop before mutation. The profile completes 101 calls,
uses 102 writeback batches／159 dirty pages／2,958 changed bytes and 39 host
deltas, and ends at generation `170`. Its 468,010-byte report has SHA-256
`00facfca5960a6e0e233deab536aa5d01e190d6e7f72e6be7caffb2e8997c425`
and stops at instruction 7675 on missing `LCMapStringW`. Full evidence is in
[bgi-get-string-type-w.md](bgi-get-string-type-w.md).

The current ja-JP case-mapping profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-lc-map-string-w
```

`kernel32.lc-map-string-w.v1` implements the six-argument explicit-positive-
length query/conversion shapes for locale `1041` and exact lower／upper flags.
It performs bounded host-locale-independent case mapping, preserves LastError on
success, and models insufficient destination and invalid zero/NULL shapes
without mutation. Negative length, other locale/flags, overlap, and
unimplemented units stop before mutation. The profile completes 113 calls,
uses 114 writeback batches／174 dirty pages／3,350 changed bytes and 44 host
deltas, and ends at generation `187`. Its 550,988-byte report has SHA-256
`d56849eac369c5ad5b946c06b27acba844b779e548795efcc464a67bc2c7a889`
and stops at instruction 12137 on missing `InterlockedDecrement`. Full evidence
is in [bgi-lc-map-string-w.md](bgi-lc-map-string-w.md).

The current checked decrement profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-interlocked-decrement
```

`kernel32.interlocked-decrement.v1` implements the one-argument aligned
`InOut LONG` shape without a compatibility-policy selector. It shares the
checked LONG helper with `InterlockedIncrement`, applies wrapping subtraction,
and preserves LastError. NULL, unaligned, unmapped, and read-only targets stop
before mutation. The profile completes 119 calls, uses 120 writeback batches／
187 dirty pages／3,640 changed bytes and 50 host deltas, and ends at generation
`199`. Its 580,772-byte report has SHA-256
`6c905456197834909eb965cfa950c05a4587a91486b7b9c72f55253340bb9ff4`
and stops at instruction 15863 on missing `GetModuleFileNameA`. Full evidence is
in [bgi-interlocked-decrement.md](bgi-interlocked-decrement.md).

The current process-image-path profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-module-file-name-a
```

`kernel32.get-module-file-name-a.v1` copies the Rust-owned bounded
`windows-932-v1` process image path through a three-argument stdcall contract.
Policy admits only the current process image and caps output capacity at 32
KiB. Success and modern truncation results, NUL termination, LastError, and
zero/NULL shapes are explicit; explicit module handles and invalid spans stop
before mutation. The BGI profile writes virtual path `C:\Vine\BGI.exe\0`, not
the private host installation path. It completes 152 calls, uses 153 writeback
batches／229 dirty pages／3,920 changed bytes and 51 host deltas, and ends at
generation `233`. Its 704,797-byte report has SHA-256
`3bbeda2ffff7720afc73c7729b52f7168b2989758baf6c7cc7e6a45b870e15e7`
and stops at instruction 16965 on unsupported `HeapAlloc(0x68010001,0,16)`.
Full evidence is in
[bgi-get-module-file-name-a.md](bgi-get-module-file-name-a.md).

The current path-buffer allocation profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-16
```

Policy `kernel32-private-heap-size-16-v1` adds only `(flags=0,size=16)`
to the existing `kernel32.heap-alloc.v2` handler. It returns allocation ID `5`
at `0x20000C40`, uses deterministic-uninitialized bytes, and applies a dirty-only
host delta because the heap band is already committed. The following path
consumer is reconciled through ordinary PTE-dirty writeback. The profile
completes 185 calls, uses 186 writebacks／272 dirty pages／4,178 changed bytes and
52 host deltas, and ends at generation `267`. Its 828,414-byte report has
SHA-256 `887a74c11a1466cfbbfdeebf126ecc6138588c4b1f9e89c39cfdbb290a2ee8e6`
and stops at instruction 18143 on unsupported
`HeapAlloc(0x68010001,8,4)`. Full evidence is in
[bgi-private-heap-size-16.md](bgi-private-heap-size-16.md).

The current environment-pointer-table allocation profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-4
```

Policy `kernel32-private-heap-size-4-v1` adds only `(flags=8,size=4)` to the
existing `kernel32.heap-alloc.v2` handler. It returns allocation ID `6` at
`0x20000C50`, zeroes the full reserved eight bytes, and applies a dirty-only
host delta because the heap band is already committed. The empty environment
consumer stores the pointer-table identity and NULL terminator through ordinary
PTE-dirty writeback. The profile completes 186 calls, uses 187 writebacks／274
dirty pages／4,194 changed bytes and 53 host deltas, and ends at generation
`269`. Its 833,007-byte report has SHA-256
`80f28db63bf8400b7ea7d1db5d7552303dec05dc71addb1ac2ed1ecea29eefa1`
and stops at instruction 18180 on missing
`HeapFree(0x68010001,0,0x20000A18)`. Full evidence is in
[bgi-private-heap-size-4.md](bgi-private-heap-size-4.md).

The current reached-free profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-free-size-1
```

Policy `kernel32-private-heap-free-size-1-v1` adds only flags-zero `HeapFree`.
`kernel32.heap-free.v1` routes the exact live allocation through the existing
transactional heap subsystem, returns BOOL `TRUE`, preserves LastError, poisons
the eight reserved bytes, and emits one dirty-only delta. The profile completes
197 calls, including ten subsequent existing `EncodePointer` calls, uses 198
writebacks／300 dirty pages／4,423 changed bytes and 54 host deltas, and ends at
generation `281`. Its 875,554-byte report has SHA-256
`e0b255e0112fb7d0cbcef12afcd91be63877049fed56fc30b62ce5123f1acd96`
and stops at instruction 18,653 on policy-disabled
`HeapAlloc(0x68010001,8,128)`. Full evidence is in
[bgi-private-heap-free-size-1.md](bgi-private-heap-free-size-1.md).

The current fixed-table allocation profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-128
```

Policy `kernel32-private-heap-size-128-v1` adds only
`(flags=8,size=128)` to the existing `kernel32.heap-alloc.v2` handler. It
returns allocation ID `7` at `0x20000C58`, zeroes all 128 requested/reserved
bytes, preserves LastError, and emits one dirty-only heap delta. One existing
`EncodePointer` call retains the result, then ordinary CPU writeback reconciles
the consumer's two global stores and first-DWORD zero. The profile completes
199 calls, uses 200 writebacks／304 dirty pages／4,444 changed bytes and 55 host
deltas, and ends at generation `284`. Its 883,995-byte report has SHA-256
`51382513962b57159e178966b16aac2010513a3c58cccaf9b9d6abbfc857d073`
and stops at instruction 18,693 on missing
`IsProcessorFeaturePresent(10)`. Full evidence is in
[bgi-private-heap-size-128.md](bgi-private-heap-size-128.md).

The current processor-feature profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-is-processor-feature-present
```

Policy `kernel32-is-processor-feature-present-v1` adds the explicit mapping
`{index:10,present:true}`. The semantic handler returns BOOL32, preserves
LastError, performs stdcall cleanup, and rejects an unregistered index before
mutation. Calls 200 and 201 return 1 at both native-observed callsites. Existing
call 202 then completes the already-enabled 2,048-byte zeroed allocation and a
second heap-page commit. The profile uses 203 writebacks／311 dirty pages／4,561
changed bytes and 56 host deltas, ends at generation `289`, and stops at
instruction 18,993 on missing
`SetUnhandledExceptionFilter(0x100A816F)`. The 896,534-byte report has SHA-256
`c173fd9d2ae2763f15ddafa0b487bddeba9b934a978ddd840766cfbad870cdb5`.
Full evidence is in
[bgi-is-processor-feature-present.md](bgi-is-processor-feature-present.md).

The current process-filter profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-unhandled-exception-filter
```

`kernel32.set-unhandled-exception-filter.v1` checks a non-NULL argument for
executable guest access, replaces one process-owned nullable filter, returns the
previous pointer, preserves LastError, and writes no guest bytes. Call 203
registers `0x100A816F` and returns NULL. Existing `EnterCriticalSection` and two
`DecodePointer` calls then continue through call 206. The profile uses 207
writebacks／316 dirty pages／4,690 changed bytes and 57 host deltas, ends at
generation `294`, and stops at instruction 19,096 on missing
`HeapSize(0x68010001,0,0x20000C58)`. The 912,806-byte report has SHA-256
`92caa9ef24a63fc1abbac0bfc063361b2c7b9998cd14d2d2ee02e2d5c999defa`.
Full evidence is in
[bgi-set-unhandled-exception-filter.md](bgi-set-unhandled-exception-filter.md).

The preceding private-heap size profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-heap-size
```

`kernel32.heap-size.v1` accepts policy-enabled flags `0` and
`HEAP_NO_SERIALIZE`, identifies an exact live allocation owned by the selected
logical private heap, and returns its requested byte length rather than reserved
or committed capacity. Invalid, interior, foreign-heap, and freed pointers return
`0xFFFFFFFF` with LastError 87; unknown heap and unsupported flags fault before
mutation. Call 207 returns 128 and preserves LastError. Two existing
`EncodePointer` calls and one `LeaveCriticalSection` continue through call 210.
The profile uses 211 writebacks／324 dirty pages／4,757 changed bytes and 58 host
deltas, ends at generation `299`, and stops at instruction 19,185 on missing
`InitializeCriticalSection(0x100FD9CC)`. The 929,421-byte report has SHA-256
`058ca051ad9b4118ce8854333eafd35aab117dc70da860b28188a2e1afff8fac`.
Full evidence is in [bgi-heap-size.md](bgi-heap-size.md).

The current one-argument critical-section profile is:

```powershell
npm run check:runtime:bgi -- --profile kernel32-initialize-critical-section
```

`kernel32.initialize-critical-section.v1` validates one checked 24-byte output,
uses the policy-owned raw spin-state field, and creates state through the same
process-owned critical-section subsystem as the existing spin-count contract.
The VOID stdcall preserves EAX and LastError. Missing policy and duplicate
initialization stop before guest-memory mutation. Call 211 writes the measured
`FF FF FF FF FF FF FF FF ... D0 07 00 02` state to `0x100FD9CC` through a
generation `299 -> 300` host delta.

The session continues through 267 calls with 268 writebacks／406 dirty pages／
5,855 changed bytes and 67 host deltas, ending at generation `365`. At
instruction 22,754 it records an endpoint-null page fault at `0x006FE000`, the
page immediately below the initial-thread stack mapping. Final memory SHA-256 is
`5a7b35e013f2ac108f5d7394dc908ba91a76aaa77dc4e5288d3ee827460c146c`.
The 1,154,575-byte report SHA-256 is
`c396de1128554bad61efabc52e1e10f29896a14e923a75d349751cde2eda7686`.
Full evidence is in
[bgi-initialize-critical-section.md](bgi-initialize-critical-section.md).

The initial-stack checkpoint preserves those 267 results, installs the PE
1 MiB reserve／4 KiB commit layout, and registers its thread-lifetime process
allocation. Four guard-page faults emit one zeroed RW commit each and move the
guard from `0x006FE000` to `0x006FA000`. The session then completes the second
`GetModuleFileNameA` occurrence as call 268. It records 273 writebacks／412 dirty
pages／5,929 changed bytes, 72 host deltas, and Node generation `374`. The next
stop is the CP_ACP／flags-zero `MultiByteToWideChar` variant at instruction
22,843. Final memory SHA-256 is
`ca1d082833b4349640451886843761aafecc0a5e63e5771694e09e5c1906c0aa`.
Full evidence is in
[bgi-initial-thread-stack-growth.md](bgi-initial-thread-stack-growth.md).

The active-code-page checkpoint retains the same implementation ID and makes
`CP_ACP` an explicit policy selector for the policy-owned Windows-932 page. Source
count `-1` performs a bounded checked scan and includes its NUL in the decoded
output and result. Call 269 converts controlled `C:\Vine\BGI.exe\0` into 16
UTF-16 units, after which two existing `WideCharToMultiByte` consumers and eight
thread-local helper calls return through call 279. The session records 284
writebacks／431 dirty pages／6,270 changed bytes, 75 host deltas, and Node
generation `388`. At instruction 24,294 it stops on missing
`SetCurrentDirectoryA(0x10104048)`. Final memory SHA-256 is
`dfb9d4fae57c56d3c518b522669c129377a2de98e64cd7de9e5dfdad42e6113d`.
Full evidence is in
[bgi-multi-byte-to-wide-char-cp-acp.md](bgi-multi-byte-to-wide-char-cp-acp.md).

The current-directory checkpoint keeps guest-visible process state in Rust.
Call 280 reads `0x10104048`, normalizes the controlled `C:\Vine\0` identity,
returns BOOL 1, preserves LastError 0, and commits generation `0 -> 1` without
retaining the guest pointer or writing guest memory. The following guest copy is
ordinary CPU writeback. The session records 285 writebacks／435 dirty pages／
6,307 changed bytes, 75 host deltas, and Node generation `389`. At instruction
24,496 it stops on missing `CreateMutexA(0,1,0x100E1CAC)`. Final memory SHA-256
is `7b95978468db1f5664799d9621b990ace2c06332b4e251bf6b377fa6c4e15bb2`.
Full evidence is in
[bgi-set-current-directory-a.md](bgi-set-current-directory-a.md).

The named-mutex checkpoint adds `kernel32.create-mutex-a.v1` without embedding
the reached BGI name or callsite. Call 281 copies the 57-byte Windows-932 name,
transactionally creates mutex ID `1`, typed object ID `19`, and logical handle
`0x58010001`, grants owner thread `8192` recursion `1`, returns the handle, and
sets LastError `0`. The private synchronization subsystem owns name／owner／
recursion while the generic object table owns handle slots and object lifetime;
neither exposes its mutable collection to the other. Calls 282–284 complete
`GetLastError`, one existing flags-zero 16-byte allocation, and its free. The
session records 289 writebacks／443 dirty pages／6,633 changed bytes, 77 host
deltas, and Node generation `395`. At instruction 25,256 it stops on missing
`GetCurrentThread()`. Final memory SHA-256 is
`338da6511938aedf1ec46c5838499f159da7fcbf86218459821e385cc6cd3add`.
Full evidence is in [bgi-create-mutex-a.md](bgi-create-mutex-a.md).

<!-- END FROZEN HISTORICAL EVIDENCE -->

## Logical affinity and process-clock contract

`kernel32.set-thread-affinity-mask.v1` uses the same catalog／dispatcher path as
the other Kernel32 APIs. It accepts two DWORD arguments and returns the previous
logical mask. The semantic handler resolves the current-thread pseudo-handle,
then asks the private thread subsystem to plan and commit a nonzero subset of
the policy-owned virtual process mask. Success preserves LastError and writes no
guest memory; null handle and invalid mask return their Win32 failures, while
unreached handle variants stop before mutation.

The reached consumer exposed an existing clock invariant: `GetTickCount` must
progress while the guest polls it. The same `kernel32.get-tick-count.v1`
implementation now asks `RuntimeProcessState` for a policy-driven sample plan.
The active policy advances after each read; policies without that selector keep
the historical fixed sample. Returned and next DWORD values are committed by
the process owner with stale-plan rejection and wrapping arithmetic. Handler
selection remains independent of callsite, occurrence, and evidence identity.

Detailed native, Node, Chrome, and performance evidence is in
[bgi-set-thread-affinity-mask.md](bgi-set-thread-affinity-mask.md). Changing host
affinity, importing host topology, arbitrary thread-handle access, and a general
timer／wait clock remain outside this contract.

## x86 system-information contract

`kernel32.get-system-info.v1` is a one-output-pointer VOID stdcall contract.
The generic dispatcher preflights the full 36-byte `SYSTEM_INFO` span, preserves
EAX and LastError, and commits one guest write only after the policy-backed
handler has produced the complete structure. NULL, read-only, unmapped, and
cross-page-invalid targets fail without a prefix write.

The schema-v2 policy owns the bounded PE32 address and processor fields. The
existing virtual process affinity mask is the only active-mask authority and
the processor count is its popcount; neither value is copied into a second
policy field. No semantic branch reads the browser or native host topology.
Detailed native consumer, Node, Chrome, and benchmark evidence is in
[bgi-get-system-info.md](bgi-get-system-info.md).

## Size-versioned ANSI OS-information contract

`kernel32.get-version-ex-a.v1` uses a dynamic in/out pointer whose catalog
constraint preflights the four-byte declared size and bounds any completion at
156 bytes. The handler accepts exactly the 148-byte base and 156-byte extended
layouts. It reads the full accepted span, merges policy fields into the caller's
before-image, and returns one checked write, preserving bytes Windows leaves
untouched. Invalid sizes produce BOOL FALSE and LastError 122 without a write.

This is not an API-specific memory transport: generic completion still
validates the write against the pointer contract and `GuestMemory` commits it
atomically. NULL, read-only, unmapped, and cross-page-invalid targets fail
before any prefix update. Success preserves the selected Rust thread's
LastError. Policy owns the virtual NT version; native and browser host identity
never enter the handler. Detailed evidence is in
[bgi-get-version-ex-a.md](bgi-get-version-ex-a.md).

## Waitable-timer creation contract

`kernel32.create-waitable-timer-a.v1` is a three-argument x86 stdcall whose
catalog return is a non-NULL logical handle in the waitable-timer object
namespace. The bounded policy accepts only NULL security attributes, manual
reset, and a NULL name. Auto-reset, named, security-attribute, and disabled
policy variants return explicit unsupported faults before runtime mutation.
Success sets LastError 0 and writes no guest byte.

The handler delegates identity to two existing owners instead of creating an
API-local registry. `RuntimeTimers` plans the monotonic timer ID and reset-mode
payload; `RuntimeObjectTable` plans object ID, typed handle, close policy, and
initial nonsignaled wait state. The aggregate preflights both plans before
targeted commits. Last close and process teardown use the generalized opaque
`RuntimeCloseHandleObjectMutationPlan` together with private timer-retirement
plans, preserving both allocators and indexes on failure. Worker metadata is an
immutable snapshot of the Rust result, not a browser timer authority. Detailed
evidence is in
[bgi-create-waitable-timer-a.md](bgi-create-waitable-timer-a.md).

## Typed guest-callback scratch frames

`GuestCallbackRequest` separates scalar arguments from scratch-relative
pointers and carries an explicit completion policy. `ExecutionSession` computes
scratch padding, sentinel／argument bytes, and the full x86 allocation range
with checked arithmetic; it validates callback and sentinel execute access plus
the entire writable stack band before pushing a Rust callback frame or writing
guest bytes. The contiguous write makes scratch pointers and argument bytes one
failure domain. Invalid offsets, unaligned pointers, capacity overflow, and
unmapped stacks leave memory generation and callback-frame allocation
unchanged.

Callback completion policy is checked against the outer ABI return schema.
Current policies cover raw U32 and BOOL32 normalization. Runtime adapters retire
only the exact top frame, apply any callback-completion memory delta, and then
resume the suspended outer call. Call evidence reserves the outer sequence at
entry, so reentrant API traps are ordered by call entry rather than completion.

The first production use is `user32.enum-display-monitors.v1`: one 16-byte RECT
scratch object and four stdcall arguments in a 36-byte frame. Its Rust display
authority and bounded variants are documented in
[bgi-enum-display-monitors.md](bgi-enum-display-monitors.md). This does not
generalize arbitrary callback nesting, SEH, cross-thread callbacks, or all
User32 callback APIs.

## Typed narrow returns, HRESULT, and Ole32 dispatch

`ApiReturnSchema::U16` and `ApiReturnValue::U16` preserve a WORD-sized API
result as a distinct ABI kind and zero-extend it into EAX at generic completion.
This avoids treating unspecified native upper EAX bits as guest semantics.
The first production use is `kernel32.get-system-default-lang-id.v1`; its
native WORD evidence and policy boundary are documented in
[bgi-get-system-default-lang-id.md](bgi-get-system-default-lang-id.md).

`kernel32.get-drive-type-a.v1` is a policy-enabled U32 query over the existing
Rust process-directory identity. The handler scans one bounded nullable ANSI
argument, preserves LastError, and returns fixed only for the derived virtual
process-image drive. It stores no drive table and imports no host or browser
topology. The A–Z caller, NULL/non-root oracle, failure cases, and Node／Chrome
equivalence are documented in
[bgi-get-drive-type-a.md](bgi-get-drive-type-a.md).

`ApiReturnSchema::HResult` and `ApiReturnValue::HResult` preserve the signed
COM result domain while committing its exact 32-bit representation to EAX.
They are distinct from raw U32 and BOOL normalization, so a caller can apply
standard failed-HRESULT sign-bit semantics without an API-specific register
path. ABI validation rejects a handler returning a mismatched kind before
completion.

The generic endpoint catalog now checks Kernel32, User32, Ole32, then WinMM
semantic contract providers. `ole32.co-initialize.v1` is the first Ole32 contract and
uses the same decoded frame, LastError policy, completion, Worker transport,
and structured-stop path as the existing APIs. Apartment state stays in the
Rust thread owner; the Worker forwards an immutable diagnostic snapshot only
for that completion. Details and bounded non-claims are in
[bgi-co-initialize.md](bgi-co-initialize.md).

The User32 catalog also exposes `user32.swap-mouse-button.v1`. Its synchronous
BOOL transition is dispatched through the common frame／completion path, while
`runtime/input.rs::RuntimeInput` owns profile identity, previous／current mapping,
and generation. Neither the dispatcher nor the Worker keeps a parallel input
mapping. The reached two-call normalize／restore sequence and atomic negative
cases are documented in [bgi-swap-mouse-button.md](bgi-swap-mouse-button.md).

The WinMM catalog first exposes `winmm.time-get-dev-caps.v1`. Its two-argument
stdcall handler returns a policy-owned 8-byte x86 `TIMECAPS` through the common
checked-write completion path. Short sizes return `TIMERR_NOCANDO` without a
write; successful sizes preserve LastError and never advance the runtime clock
or call a browser service. The Worker stores no capability mirror. Policy,
native guard oracle, and caller consumption are documented in
[winmm-compatibility-policy.md](winmm-compatibility-policy.md) and
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md).

The same catalog exposes `winmm.time-begin-period.v1` and
`winmm.time-end-period.v1` as one-argument stdcall `MMRESULT` contracts. The
handlers preserve LastError and delegate counted request lifetime to the
private `runtime/timers.rs::RuntimeTimers` owner. Policy rejection and
unmatched end return `TIMERR_NOCANDO` without mutation; owner plans reject
stale commits, and process exit retires outstanding requests. Completion may
carry an immutable timer snapshot for diagnostics, but the Worker does not own
or apply the request table. Details are in
[bgi-time-begin-period.md](bgi-time-begin-period.md).

`winmm.time-get-time.v1` is a zero-argument stdcall U32 contract. It preserves
LastError, writes no guest bytes, and delegates to the same
`RuntimeProcessState` tick-sample plan and targeted commit as Kernel32
`GetTickCount`. The WinMM policy selects only the bounded increment; it owns no
second counter, and `RuntimeTimers` resolution requests cannot alter the
sample. Neither the handler nor the Worker imports a host or browser clock.
Native consumer and Node／Chrome evidence are documented in
[bgi-time-get-time.md](bgi-time-get-time.md).

## Deliberate exclusions

- API-specific request／receipt schemas remain only in the frozen numbered evidence adapters; the Step 10 path does not use them.
- The runtime has controlled process／thread, wait／message, mapped-module, guest-import, and DLL process lifecycle foundations; general CPU-context-backed threads, wait variants, dynamic load／unload, and TLS／thread DLL callbacks remain absent.
- Private heaps have bounded guest-memory backing, the first four reached zeroed
  allocations, and flags-zero deterministic-uninitialized size-1, size-16, and size-544
  allocations enabled by policy; exact flags-zero free is enabled for reached
  live allocations. Exact live-allocation `HeapSize` is enabled for flags 0 and
  `HEAP_NO_SERIALIZE`. Other sizes/flags, destroy/realloc dispatch, Windows
  corruption termination, and broad allocator policy remain absent.
- FLS free／callback teardown／fiber switching and TLS free／teardown／guest-thread
  lifecycle, critical-section deletion／contention blocking, general raw
  initialization policy, and arbitrary export
  pages are absent. TLS and FLS reads are limited to process allocation and
  selected-thread state already owned by the runtime.
- Host completion is connected to controlled Worker／browser services. Logical
  mouse-button mapping, the virtual WinMM capability profile, and logical
  timer-resolution request lifetime have Rust owners. The request does not
  reconfigure browser scheduling or audio; DOM input delivery, general VFS,
  graphics, audio, and storage adapters remain absent.
- `GuestMemoryTransaction` uses a changed-state／frame before-image journal;
  Rust byte authority, automatic v86 writeback, and binary transport are now
  fixed by ADR-003 and the memory-writeback checkpoint.
- `InterlockedIncrement` and `InterlockedDecrement` are implemented only for the
  checked aligned 32-bit LONG shape in the single cooperative Runtime Worker. Other interlocked APIs and
  cross-worker shared-memory atomicity are absent.
- The generic session owns only the reached standard input／output／error
  identities, stdin character classification, and stdout／stderr pipe
  classifications, the policy-enabled `SetHandleCount(32)` no-op contract, and
  the process-owned ASCII command-line view, one controlled wide-environment
  authority／copy lease, a policy-owned Windows-932 active code page, and its
  measured 18-byte code-page information fields, checked Windows-932
  multibyte query/conversion, and the reached positive-length `CT_CTYPE1`
  classification, checked wrapping decrement of one aligned process LONG, and
  a bounded process-owned Windows-932 executable path copied for NULL-module
  `GetModuleFileNameA`.
  General handle-table sizing, `SetStdHandle`, close behavior, broader file type,
  console semantics, file I/O, host-environment import, ANSI environment strings,
  environment mutation, non-Windows-932 code pages, explicit default-character
  pointers, conversion flags other than `0`／`1`, negative
  `GetStringTypeW` lengths, other information types, arbitrary `LCMapStringW`
  locale/flag/length variants, and broad locale tables
  remain absent. Explicit module file-name lookup and host image-path import are
  also absent. Processor feature presence is explicit policy data and does not
  import host CPU discovery. One process-owned top-level exception-filter
  pointer is stored, replaced, and retired without bridging to host exception
  machinery; SEH dispatch and filter invocation remain absent. Initial-thread
  stack growth is Rust-owned; additional-thread stacks, overflow, and guard
  exception delivery remain absent. Process current-directory identity and the
  reached process-image-directory update are Rust-owned; general ANSI relative-path
  resolution and VFS effects remain absent. Named mutex identity, initial owner,
  recursive immediate wait／release, distinct-handle lifetime, and teardown are
  Rust-owned; security attributes, unnamed objects, abandonment, cross-thread
  blocking／wake, and guest wait／release／close API coverage remain absent.
  Reached unnamed manual-reset waitable-timer identity, typed handle, initial
  nonsignaled state, last-close retirement, and process teardown are Rust-owned;
  due time, period, signaling, cancellation, auto-reset, named variants, and
  guest timer／wait／close APIs remain absent.
  Current-thread logical affinity and deterministic tick progression are
  Rust-owned, but host affinity／topology, arbitrary thread handles, process
  affinity APIs, and general sleep／timer／wait-deadline semantics remain absent.
  The x86 virtual `SYSTEM_INFO` profile is policy-owned and fixed to the bounded
  PE32 address model; x64, processor groups, NUMA, dynamic host topology, and
  broader system-information APIs remain absent.
  Reached `CoInitialize(NULL)` owns a counted STA apartment on the selected
  Rust thread. Guest `CoInitializeEx`／`CoUninitialize`, class activation,
  marshaling, and automation remain absent.
  Active structured-stop identity is intentionally not repeated in this document.

## Frozen next-checkpoint notes

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->

The next proof is the reached zero-argument `GetCurrentThread()` call. It must
measure native pseudo-handle identity and the immediate consumer relation, then
return the selected Rust-owned thread identity without importing a host TID or
creating TypeScript Windows state. Named mutex lifetime, current-directory
identity, CP_ACP conversion, and the four stack-growth deltas must remain green.
Existing import, bootstrap, DLL lifecycle,
bidirectional memory sync, frozen 17-call report, generic 96,244-byte report,
critical-section 167,719-byte report, pointer-decoding 171,356-byte report,
FLS-allocation 175,123-byte report, private-heap 183,102-byte report, FLS-value
195,821-byte report, InterlockedIncrement 234,775-byte report, second-heap
239,789-byte report, standard-input 243,749-byte report, and file-type
252,994-byte report remain historical artifact identities. Current semantic
regression uses field assertions; the replayed policy-bearing standard-output
report is 257,878 bytes. The stdout-file-type and stderr-handle reports are
267,223 and 271,184 bytes respectively; the stderr-file-type report is 280,475
bytes, the SetHandleCount report is 284,215 bytes, the GetCommandLineA report is
288,041 bytes, the environment-strings report is 291,793 bytes, the Windows-932
report is 295,762 bytes, the one-byte heap report is 308,984 bytes, the
GetLastError report is 312,644 bytes, the TlsGetValue report is 316,360 bytes,
    the FlsGetValue report is 320,070 bytes, the SetLastError report is 363,609
    bytes, the GetACP report is 367,384 bytes, the 544-byte heap report is
    386,921 bytes, the GetCPInfo report is 407,009 bytes, the
    MultiByteToWideChar report is 423,814 bytes, the GetStringTypeW report is
    468,010 bytes, the LCMapStringW report is 550,988 bytes, the
    InterlockedDecrement report is 580,772 bytes, the GetModuleFileNameA report
    is 704,797 bytes, the size-16 heap report is 828,414 bytes, the size-4 heap
    report is 833,007 bytes, the one-byte-free report is 875,554 bytes, the
    size-128 heap report is 883,995 bytes, the processor-feature report is
    896,534 bytes, the process-filter report is 912,806 bytes, the heap-size
    report is 929,421 bytes, the InitializeCriticalSection report is 1,154,575
    bytes, and the initial-stack-growth report is 1,179,850 bytes. The current
    CP_ACP report is 1,225,836 bytes, and the current-directory report is
    1,230,591 bytes. The current CreateMutexA report is 1,250,520 bytes.
<!-- END FROZEN HISTORICAL EVIDENCE -->

The active next stop and reproduction command come only from the manifest reference above.
