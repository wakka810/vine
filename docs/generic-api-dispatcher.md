# Generic API dispatcher and RuntimeState foundation

## Status

- Status: review Step 2／3, Step 4 dispatcher migration, and Step 10 generic BGI re-entry implemented
- Date: 2026-07-13
- Runtime evidence base: `bgi-seventeenth-win32-call`
- Runtime-kernel checkpoint: `bgi-generic-runtime-reentry`
- Scope: frozen 17-call equivalence plus 21 calls through one persistent generic runtime session

## Dispatcher boundary

`vine-core` owns the synchronous API boundary:

- `ApiEndpointIdentity` and `ApiContract`
- `AbiSchema`, argument direction, pointer／string／struct／name-or-ordinal constraints, return schema, and stack cleanup
- `ApiDispatcher`, `ApiCallFrame`, and `ExecutionSession`
- state-driven Kernel32 handlers, typed outcomes, and structured `ApiFault`

The implemented path is:

```text
owned import endpoint
  -> endpoint and bound-IAT lookup
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
| `GetCurrentThreadId` | 1 | returns scheduler current thread |
| `GetTickCount` | 1 | returns process monotonic clock |
| `QueryPerformanceCounter` | 1 | writes process performance counter |
| `GetStartupInfoW` | 1 | writes process startup profile |
| `HeapSetInformation` | 1 | enables the reached process heap policy |
| `HeapCreate` | 1 | allocates deterministic logical heap metadata |
| `GetModuleHandleW` | 1 | bounded UTF-16 loaded-module lookup |
| `GetProcAddress` | 4 | bounded ASCII dynamic-endpoint lookup |
| `TlsAlloc` | 1 | allocates a process TLS index |
| `TlsSetValue` | 1 | stores a current-thread endpoint value |
| `EncodePointer` | 2 | allocates stable process pointer tokens |

All 13 distinct APIs and all 17 existing occurrences now call `ApiDispatcher` and `dispatch_kernel32_api`. The title harness still performs callsite provenance checks, v86 page reconciliation, and evidence serialization around that common semantic path.

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

## RuntimeState authority

One `RuntimeState` is created at cumulative-session start. It owns:

- process ID, address-space control ranges, clock, startup profile, heap policy, logical heaps, loaded modules, and TLS indices;
- current thread ID, per-thread LastError, TLS values, and callback frame stack;
- dynamic endpoints, deterministic encoded pointers, pending host requests, and scheduler events.

The Rust profile preloads the logical runtime KERNEL32 module and installed FLS endpoints. `HeapCreate` and `TlsAlloc` allocate through runtime-owned deterministic allocators. `GetModuleHandleW` and `GetProcAddress` resolve state by canonical module and exact export name. No handler receives a result value from the JSON protocol.

Protocol request values remain only as private evidence assertions. A mismatch fails before return, but cannot determine the API result. Native heap handles, module pointers, export addresses, TLS indices, and process-cookie values are never imported into guest state.

## Thread and scheduler state

LastError is current-thread state and all 13 contracts explicitly preserve it. Callback frames record thread identity and entry LastError, enforce strict LIFO return, and keep callback depth synchronized.

A host request changes `Runnable` or `InCallback` to `WaitingHost`. Completion restores the saved state and enqueues `RuntimeSchedulerEvent::HostCompleted`. This remains a scheduler foundation: no current BGI contract performs a browser asynchronous request.

## Verification

Representative semantic contracts:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
```

The two tests cover all 13 APIs, the five original shapes, output writes, LastError, heap／module／endpoint／TLS state, bounded string lookup, explicit ordinal rejection, callback LIFO, and host completion events.

The cumulative private-corpus command is:

```powershell
npm run check:runtime:bgi-seventeenth-win32-call
```

Before and after migration it reaches the same trap after 238 instructions. The report remains 140,660 bytes with SHA-256 `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`; reflection SHA-256 remains `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`.

The Step 10 semantic run is separate:

```powershell
npm run check:runtime:bgi-generic-runtime-reentry
```

It validates the frozen 17 calls, returns four more `EncodePointer` calls through
the same contract, and stops after 21 completed calls at instruction 266 on
`KERNEL32.dll!InitializeCriticalSectionAndSpinCount(0x100FF630, 4000)`. The run
discovers 38 changed pages／268 changed bytes in 22 automatic writeback batches,
applies 3 host deltas, and advances generation `30 -> 52`. Its 96,244-byte report
has SHA-256 `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` in two
consecutive runs. The frozen report is unchanged, so semantic validation and
historical evidence serialization are now independent.

## Deliberate exclusions

- API-specific request／receipt schemas remain only in the frozen numbered evidence adapters; the Step 10 path does not use them.
- The runtime has controlled process／thread, wait／message, mapped-module, guest-import, and DLL process lifecycle foundations; general CPU-context-backed threads, wait variants, dynamic load／unload, and TLS／thread DLL callbacks remain absent.
- Private heaps have bounded guest-memory backing, but guest `HeapAlloc` dispatch and broad allocator policy remain absent.
- TLS free/thread lifecycle, pointer decode, and arbitrary export pages are absent.
- Host completion is connected to controlled Worker／browser services; general
  VFS, input, graphics, audio, and storage adapters remain absent.
- `GuestMemoryTransaction` uses a changed-state／frame before-image journal;
  Rust byte authority, automatic v86 writeback, and binary transport are now
  fixed by ADR-003 and the memory-writeback checkpoint.
- The generic session has passed the third through sixth `EncodePointer` calls. Its first unsupported cluster is Kernel32 critical-section synchronization; bespoke checkpoint expansion stays paused.

## Next checkpoint

The next proof is the reached `kernel32-critical-section-synchronization`
capability cluster. Native evidence must fix the BOOL return, LastError policy,
x86 critical-section bytes, spin-count behavior, and first reached enter／leave
consumer. One process-owned synchronization contract will then be added through
the same dispatcher and automatic writeback path. Existing import, bootstrap,
DLL lifecycle, bidirectional memory sync, frozen 17-call report, and generic
96,244-byte report remain the regression boundary.
