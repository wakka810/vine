# Generic API dispatcher and RuntimeState foundation

## Status

- Status: review Step 2／3, Step 4／10, critical-section, pointer-decoding, FLS-allocation, private-heap allocation/free, FLS-value, InterlockedIncrement／Decrement, second private-heap allocation, standard-handle/file-type, SetHandleCount, GetCommandLineA, GetModuleFileNameA, wide-environment copy-lifetime, Windows-932 wide/multibyte conversion, one-byte, 16-byte, and 544-byte uninitialized private heaps, four-byte environment-pointer table, 128-byte zeroed private heap, GetLastError, TlsGetValue, FlsGetValue, SetLastError, GetACP, GetCPInfo, GetStringTypeW, LCMapStringW, IsProcessorFeaturePresent, and SetUnhandledExceptionFilter capabilities implemented
- Date: 2026-07-14
- Runtime evidence base: `bgi-seventeenth-win32-call`
- Runtime-kernel checkpoint: `bgi-set-unhandled-exception-filter`
- Scope: frozen 17-call equivalence plus 206 calls through one persistent generic runtime session

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
| `InitializeCriticalSectionAndSpinCount` | 17 | creates process-owned synchronization state and exact x86 bytes |
| `EnterCriticalSection` | 3 | acquires or recursively enters for the current thread |
| `LeaveCriticalSection` | 3 | decrements recursion and releases at zero |
| `FlsAlloc` | 1 | allocates a bounded process-owned logical FLS slot and retains its callback |
| `FlsSetValue` | 1 | stores an opaque value in the selected thread's allocated FLS slot |
| `HeapAlloc` | 6 | allocates three zeroed and three flags-zero deterministic-uninitialized reached blocks through transactional guest-memory backing |
| `InterlockedIncrement` | 3 | performs a checked wrapping LONG update in guest memory |
| `GetStdHandle` | 3 | returns versioned process-owned logical input／output／error identities |
| `GetFileType` | 3 | classifies registered logical stdin as character and stdout／stderr as pipes |
| `SetHandleCount` | 1 | returns the reached compatibility-policy count without resizing host state |
| `GetCommandLineA` | 1 | returns the stable process-owned ANSI view derived at bootstrap |
| `GetEnvironmentStringsW` | 1 | acquires one process-owned copy lease for the controlled UTF-16 environment block |
| `FreeEnvironmentStringsW` | 1 | retires only the exact active copy lease and returns BOOL success |
| `WideCharToMultiByte` | 4 | performs host-locale-independent Windows-932 query/conversion shapes |
| `GetLastError` | 7 | reads selected-thread LastError without mutation |
| `TlsGetValue` | 7 | reads the selected thread's allocated TLS value and applies Win32 LastError semantics |
| `FlsGetValue` | 7 | reads initialized selected-thread FLS storage and applies Win32 LastError semantics |
| `SetLastError` | 7 | writes the full selected-thread LastError value without guest-memory mutation |
| `GetACP` | 1 | returns the policy-owned active ANSI code page without mutation |
| `GetCPInfo` | 1 | writes the policy-owned 18-byte active code-page fields |
| `MultiByteToWideChar` | 6 | queries and writes checked Windows-932 UTF-16 output without host-locale input |
| `GetStringTypeW` | 1 | writes checked policy-enabled CTYPE1 WORD classifications without host-locale input |
| `LCMapStringW` | 4 | queries and writes checked ja-JP lowercase／uppercase output without host-locale input |
| `InterlockedDecrement` | 2 | performs a checked aligned wrapping LONG decrement in guest memory |
| `GetModuleFileNameA` | 1 | copies the bootstrap-owned main-module ANSI path to checked output |

All 39 distinct APIs use `ApiDispatcher` and `dispatch_kernel32_api`. The frozen
17 occurrences remain the compatibility anchor; the capability profile completes
119 BGI calls, including six `EncodePointer` occurrences, 17 critical-section
initializations, two `DecodePointer` calls, dynamic `FlsAlloc`／`FlsSetValue`,
four `HeapAlloc`, three `InterlockedIncrement` calls, and later existing
critical-section／thread／startup occurrences plus three `GetStdHandle`, three
`GetFileType`, `SetHandleCount`, `GetCommandLineA`, and
`GetEnvironmentStringsW`, four `WideCharToMultiByte`, seven each of
`GetLastError`, `TlsGetValue`, `FlsGetValue`, and `SetLastError`, one `GetACP`,
one `GetCPInfo`, six `MultiByteToWideChar`, one `GetStringTypeW`, four
`LCMapStringW`, and two `InterlockedDecrement` calls.
Leave semantics remain fixed by native evidence and the controlled public test.
The source-v86 Node oracle harness still performs callsite provenance checks and
adapter-specific dirty-page synchronization around the common semantic path.
The production Browser Worker receives only generic changed-page batches from
`ExecutionEngine`.

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
- a process-owned ANSI command-line pointer derived from the UTF-16 process-parameter authority.
- one controlled UTF-16 environment authority and one bounded copy lease; host environment
  names and values are never imported.

The Rust profile preloads the logical runtime KERNEL32 module and installed FLS endpoints. `HeapCreate` and `TlsAlloc` allocate through runtime-owned deterministic allocators. `GetModuleHandleW` and `GetProcAddress` resolve state by canonical module and exact export name. No handler receives a result value from the JSON protocol.

Protocol request values remain only as private evidence assertions. A mismatch fails before return, but cannot determine the API result. Native heap handles, module pointers, export addresses, TLS indices, and process-cookie values are never imported into guest state.

## Thread and scheduler state

LastError is current-thread state and all 33 contracts declare an explicit preserve-or-handler policy. Callback frames record thread identity and entry LastError, enforce strict LIFO return, and keep callback depth synchronized.

A host request changes `Runnable` or `InCallback` to `WaitingHost`. Completion restores the saved state and enqueues `RuntimeSchedulerEvent::HostCompleted`. Critical-section recursion is synchronous for the current thread; cross-thread contention returns structured `critical-section-contention-unimplemented` before mutation because scheduler-backed blocking is not implemented. This remains a scheduler foundation: no current BGI contract performs a browser asynchronous request.

## Verification

Representative semantic contracts:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

The semantic tests cover all 33 APIs, the five original shapes, fixed and
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

## Deliberate exclusions

- API-specific request／receipt schemas remain only in the frozen numbered evidence adapters; the Step 10 path does not use them.
- The runtime has controlled process／thread, wait／message, mapped-module, guest-import, and DLL process lifecycle foundations; general CPU-context-backed threads, wait variants, dynamic load／unload, and TLS／thread DLL callbacks remain absent.
- Private heaps have bounded guest-memory backing, the first four reached zeroed
  allocations, and flags-zero deterministic-uninitialized size-1, size-16, and size-544
  allocations enabled by policy; exact flags-zero free is enabled for reached
  live allocations. Other sizes/flags, destroy/realloc dispatch, Windows
  corruption termination, and broad allocator policy remain absent.
- FLS free／callback teardown／fiber switching and TLS free／teardown／guest-thread
  lifecycle, critical-section deletion／contention blocking, and arbitrary export
  pages are absent. TLS and FLS reads are limited to process allocation and
  selected-thread state already owned by the runtime.
- Host completion is connected to controlled Worker／browser services; general
  VFS, input, graphics, audio, and storage adapters remain absent.
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
  pointers, `CP_ACP` multibyte selection, other conversion flags, negative
  `GetStringTypeW` lengths, other information types, arbitrary `LCMapStringW`
  locale/flag/length variants, and broad locale tables
  remain absent. Explicit module file-name lookup and host image-path import are
  also absent. Processor feature presence is explicit policy data and does not
  import host CPU discovery. One process-owned top-level exception-filter
  pointer is stored, replaced, and retired without bridging to host exception
  machinery; SEH dispatch and filter invocation remain absent. Its first
  unsupported operation is `HeapSize(0x68010001,0,0x20000C58)`; bespoke
  checkpoint expansion stays paused.

## Next checkpoint

The next proof is `kernel32-heap-size` at
`HeapSize(0x68010001,0,0x20000C58)`. It must fix live-allocation identity,
return／failure sentinel, flags, LastError, and the consumer without broadening
the reached pointer into unrestricted heap introspection.
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
    is 704,797 bytes, the size-16 heap report is 828,414 bytes, and the current
    size-4 heap report is 833,007 bytes, the one-byte-free report is 875,554
    bytes, the size-128 heap report is 883,995 bytes, the processor-feature
    report is 896,534 bytes, and the current process-filter report is 912,806
    bytes.
