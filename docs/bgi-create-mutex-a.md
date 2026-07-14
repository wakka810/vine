# BGI CreateMutexA

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-create-mutex-a`
- Previous capability checkpoint: `bgi-set-current-directory-a`
- First unsupported capability: `kernel32-get-current-thread`

## Purpose

The reached call creates BGI's process-wide single-instance mutex before the
startup path continues through `GetLastError`:

```text
CreateMutexA(0, 1, 0x100E1CAC)
  -> name `Buriko General Interpreter for Sakuranouta is executing.\0`
  -> logical handle 0x58010001
  -> new object LastError 0
  -> initial thread 8192 owns mutex ID 1 with recursion 1
```

This checkpoint connects one named synchronization object to the existing typed
object／handle and wait boundaries. It does not implement cross-thread blocking,
the guest `WaitForSingleObject`／`ReleaseMutex`／`CloseHandle` contracts, or a
general Windows object-manager namespace.

## Native evidence

The focused observer records `CreateMutexA`, its bounded caller frames, and an
independent create／wait／release／close oracle:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-create-mutex-a-20260714-r2.jsonl `
  --seconds 0.2 --kill-after --observe create-mutex-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| New BGI instance | 58,888 | 94 | 21 | `56931882497c18b6126a3dd93d535df07c4e717f2f3c800e78045bf56cb24904` |
| Existing BGI instance | 58,776 | 94 | 21 | `d83862ad0d0effaafd273e3e9624363a6cbd87e1cf59f1c6d49afbd51b6a77cd` |

Both captures contain 92 contiguous events (`0..91`) and zero hook, vtable,
call-stack, agent-message, cleanup, or oracle diagnostics. The BGI call returns
to RVA `0x00087300`.

The ANSI name occupies 57 bytes including NUL. Its byte SHA-256 is
`3f423adb8974d8f8cb0748e49b073d2f8f23d97d45fbe7d88b6d0f91cad196e5`
and FNV-1a is `0x0C6AC3D2`. A new native instance returns a non-NULL opaque
handle with LastError `0`; while the first BGI process retains the name, a
second process receives a distinct non-NULL handle with LastError `183`
(`ERROR_ALREADY_EXISTS`). Numeric native handles are run identity only.

The immediate continuation stores EAX in process-global `0x100E1FD0`, calls
`GetLastError`, and compares EAX with `183`. The branch therefore consumes both
handle lifetime and already-existing status; it does not depend on an exact
handle value.

The independent `x86-named-copy-owner-handle-lifetime-v1` oracle establishes:

- the name is copied before the caller mutates its input buffer;
- two creates of the same name return distinct handles to one object;
- the first `initialOwner=TRUE` grants ownership and recursion `1`;
- `initialOwner` is ignored when the named object already exists;
- same-thread zero-time waits acquire recursively and preserve LastError;
- `ReleaseMutex` decrements recursion, and an unowned release fails with
  `ERROR_NOT_OWNER` (`288`);
- closing one handle leaves the object reachable through the other;
- closing the last handle retires the name, so a later create is a new object.

## Runtime contract

`RuntimeSynchronization` privately owns mutex records and the UTF-16 named
index. `RuntimeMutexNameA` owns both the original ANSI bytes including NUL and
the decoded Windows-932 UTF-16 identity; no guest pointer survives dispatch.
A mutex record owns a monotonic mutex ID, its paired typed object ID, optional
owner thread, and recursion count.

Creation is a cross-subsystem transaction. The aggregate coordinator clones the
generic object table and synchronization state, plans name lookup／creation,
allocates or opens the `RuntimeObjectKind::Mutex` object, allocates a type-tag
`0x58` handle, and commits both subsystems only after lifecycle and capacity
preflight succeeds. Existing-name creates allocate another handle to the same
object without changing ownership. The last handle close retires the object,
mutex record, and named index together; process teardown retires every remaining
mutex handle and record.

The existing one-object wait boundary now has mutex-specific terminal results:

- unowned mutex: acquire for the current thread, recursion `1`;
- same owner: recursive acquire with checked increment;
- other owner: `mutex-contended-unsupported` before any mutation.

Release similarly plans and commits one owned record. Abandonment, waiter
registration, scheduling, timeout, and cross-thread wake are intentionally not
invented before BGI reaches contention.

`kernel32.create-mutex-a.v1` supplies one three-argument stdcall semantic
handler. The active policy enables null security attributes, named mutexes, and
a 32,768-byte ANSI scan; unnamed mutexes are disabled. Success returns the
logical handle and sets LastError to `0` for a new object or `183` for an
existing object. Unsupported security attributes, unnamed／empty variants,
policy-disabled operation, and unterminated names stop before runtime or memory
mutation.

Public tests use the independent name `Vine public mutex`, mutate its guest
buffer after return, reopen it through a second address, verify distinct handles
and shared identity, exercise recursive wait／release／last-close recreation and
process teardown, and cover policy／ABI／mutation-free rejection without loading
private evidence.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-create-mutex-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 46 / 284 | 46 / 284 |
| Instructions at next stop | 25,256 | 25,256 |
| Writeback batches / pages | 289 / 443 | 2,481 / 3,175 |
| Host/mapping deltas | 77 | 77 |
| Final generation | 395 | 2,207 |
| Memory SHA-256 | `338da6511938aedf1ec46c5838499f159da7fcbf86218459821e385cc6cd3add` | same |
| Report bytes | 1,250,520 | 281,761 |
| Report SHA-256 | `0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164` | `d47056b546b35caac768035928e980caad799957345291cd4d5cf3fe33586a84` |

Call 281 returns logical handle `0x58010001`, mutex ID `1`, typed object ID
`19`, owner thread `8192`, recursion `1`, and LastError `0`. Calls 282–284 then
complete `GetLastError`, `HeapAlloc(...,16)`, and `HeapFree`. Chrome used 3,246
slices, 2,957 budget yields, 9,739 validated state transitions, and 2,850
Rust/Wasm bridge operations. Both paths retain identical current-directory and
mutex metadata, four stack growths, final memory, next fault, cancel／watchdog
cleanup, process exit, and DLL detach.

Runtime-core Wasm is 2,087,633 bytes with SHA-256
`cce4e68d59599631729fbf211c16aa94338bf3845f770a94532baf2365633f3c`.
The separate 180-second workload completed 624 sessions with zero failures and
passed all 17 versioned thresholds. Session p95 is `319.125 ms`, slice p99
`1.280 ms`, heartbeat p99 `6.080 ms`, maximum heartbeat gap `18.245 ms`, main
heap slope `-123,360.29 bytes/min`, Worker heap slope `3,052,138.61 bytes/min`,
and backing-storage slope `3,850,303.45 bytes/min`. The 1,180,715-byte report has
SHA-256
`297afb5f8ba3073b347619529d868b5dd9645262fbdbde4a654c5f44cf41ae15`.

## Next structured stop

```text
KERNEL32.dll!GetCurrentThread()
endpoint ID:       40
endpoint address:  0x7F000270
IAT:               0x100D1148
return:            0x1006998F
instruction:       25256
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must measure the native pseudo-handle value only as opaque
evidence, identify its immediate equality／consumer relations, and connect the
result to the existing Rust-owned selected-thread identity without creating a
second thread authority.

## Non-claims

This checkpoint does not implement general object namespaces, access masks or
security descriptors, inheritance, unnamed mutexes, `OpenMutex`, abandonment,
cross-process runtime state, cross-thread blocking／wake, timeout, wait-any／
wait-all, guest wait／release／close API contracts, additional-thread CPU
contexts, title rendering, audio, VFS, COM, or full play.
