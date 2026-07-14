# BGI SetCurrentDirectoryA

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-set-current-directory-a`
- Previous capability checkpoint: `bgi-multi-byte-to-wide-char-cp-acp`
- First unsupported capability: `kernel32-create-mutex-a`

## Purpose

The reached call establishes the process current directory before BGI copies the
same path into process-global storage and starts opening title resources.

```text
SetCurrentDirectoryA(0x10104048)
  -> controlled path `C:\Vine\0`
  -> BOOL 1
  -> LastError remains 0
  -> Rust-owned current-directory generation 0 -> 1
```

This checkpoint adds process-owned path identity and transactional mutation. It
does not add a VFS or make the native install path part of Win32 semantics.

## Native evidence

The focused observer records `SetCurrentDirectoryA`, bounded caller frames,
process current-directory state, and an independent oracle:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-set-current-directory-a-20260714-r4.jsonl `
  --seconds 0.1 --kill-after --observe set-current-directory-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r2 | 42,686 | 74 | 11 | `b6ea50c32a22c4beb28ce71fabc3329ec64553a74a1f56e96dc343003a94eee1` |
| Confirmation r4 | 42,764 | 74 | 11 | `9a60c2e7562f5d3889c392e267bca332caf4031afdc0fe34404e7f0b54d873ed` |

Both focused captures have contiguous event sequence `0..71` and zero hook,
vtable, call-stack, agent-message, cleanup, or oracle diagnostics. The BGI call
returns to RVA `0x0006084A`.

The native path is `V:\softs\サクラノ詩\` and occupies 21 Windows-932 bytes
including NUL. Its byte SHA-256 is
`8c0d0e2de4dc6bacd126c540e2c5ade2c95d2f60aed46130cee309b1dc8dade4`
and FNV-1a is `0x6BEEB8DB`. The call returns `1`, preserves LastError `0`,
leaves the caller bytes unchanged, and changes neither the normalized process
directory nor its identity because the process was already rooted there. Windows
reports the normalized directory without the input trailing separator.

The immediate continuation discards EAX with `xor eax,eax` and copies the source
path from its input global to another process-global buffer. A longer 302,028-byte,
922-line capture with SHA-256
`b0a3a972029cc86e21fd7738d4331ea5d51a7fae7723100883715347cef4f43d`
shows later `CreateFileA` calls using absolute paths rooted in that copied
directory, including `...\BGI.gdb`. This is evidence of the consumer lifetime;
it does not require retaining the guest pointer after return.

The independent oracle fixes the general Windows behavior used by the bounded
runtime subset:

| Input | Result | LastError / state |
|---|---:|---|
| `..` | `1` | preserves `87` |
| `.` | `1` | preserves `88` |
| caller buffer mutated after return | n/a | process directory remains unchanged |
| existing file used as a directory | `0` | `267` (`ERROR_DIRECTORY`) |
| missing directory | `0` | `2` (`ERROR_FILE_NOT_FOUND`) |
| empty string | `0` | `123` (`ERROR_INVALID_NAME`) |
| restore original directory | `1` | preserves `92` |

## Runtime contract

`RuntimeProcessDirectories` is private process state in
`runtime/process.rs`. `RuntimeCurrentDirectoryA` owns one normalized,
trailing-NUL Windows-932 byte vector and a monotonic generation. Mutation uses
`plan -> process-lifecycle preflight -> commit`; a stale plan or lifecycle change
cannot partially change the directory.

`kernel32.set-current-directory-a.v1` implements the Win32 ABI independently of
BGI evidence:

- success returns BOOL `1`, preserves LastError, copies the normalized identity
  into Rust-owned process state, and increments its generation;
- null returns BOOL `0` with `ERROR_INVALID_PARAMETER` (`87`);
- empty input returns BOOL `0` with `ERROR_INVALID_NAME` (`123`);
- an unterminated input beyond the policy bound returns a resource-limit fault
  before mutation;
- a valid but unsupported absolute or relative variant returns a structured
  unsupported fault before mutation.

Policy `kernel32-set-current-directory-a-v1` enables only the registered process
image directory identity and bounds the ANSI scan to 32,768 bytes:

```json
"setCurrentDirectoryA": {
  "processImageDirectoryOnly": true,
  "maximumPathBytes": 32768
}
```

The path normalizer is Windows-932 aware, so a DBCS trail byte `0x5C` is not
mistaken for a separator. DOS drive letter, separator, trailing slash, and ASCII
case differences normalize to one identity. The policy is not hard-coded to
`C:\Vine`; Node and Browser bootstrap explicitly supply that controlled virtual
identity for this fixed target.

Public tests cover normalized identity, generation, caller-buffer ownership,
null／empty／unterminated input, unsupported relative and other absolute paths,
stale-plan and lifecycle atomicity, DBCS separator handling, policy parsing, and
ABI cleanup without loading private evidence.

## Runtime and browser evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-current-directory-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 45 / 280 | 45 / 280 |
| Instructions at next stop | 24,496 | 24,496 |
| Writeback batches / pages | 285 / 435 | 2,408 / 3,099 |
| Host/mapping deltas | 75 | 75 |
| Final generation | 389 | 2,136 |
| Memory SHA-256 | `7b95978468db1f5664799d9621b990ace2c06332b4e251bf6b377fa6c4e15bb2` | same |
| Report bytes | 1,230,591 | 275,732 |
| Report SHA-256 | `a90733ca1dc9b8017263c5c5439e55570403927420f1a535ff61e2774278fd68` | `59d96785bf569942c3390a7c9c9e4ad76cbe0ad22b858eef34e65e263cea3d18` |

Chrome used 3,149 slices, 2,864 budget yields, and 2,771 Rust/Wasm bridge
operations. The initial directory and generation, the post-call process-image
directory and generation `1`, all four stack growths, cancel, watchdog, zero
pending work, process exit, and DLL detach match the Node semantic oracle.
Runtime-core Wasm is 1,961,871 bytes with SHA-256
`737e8e38546a04d5dabd254a0c5785570d052c857bfdef0f4eb24599e6b9c174`.

The separate 180-second workload completed 566 sessions with zero failures and
passed all 17 versioned thresholds. Session p95 is `351.465 ms`, slice p99
`1.460 ms`, heartbeat p99 `16.135 ms`, maximum heartbeat gap `20.905 ms`,
main heap slope `-441,630.72 bytes/min`, Worker heap slope
`2,101,721.01 bytes/min`, and backing-storage slope
`1,180,012.31 bytes/min`. The 1,033,655-byte report has SHA-256
`7e9d56df7b81333ff46415b390af504872c5af98cb9a03a34e6dc590b1a3478a`.

## Next structured stop

```text
KERNEL32.dll!CreateMutexA(0, 1, 0x100E1CAC)
endpoint ID:       59
endpoint address:  0x7F0003A0
IAT:               0x100D1194
return:            0x10087300
instruction:       24496
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must measure mutex name bytes and lifetime, initial-owner
semantics, handle identity, LastError including already-existing behavior, close
and wait consumers, and process teardown before choosing the synchronization
object contract.

## Non-claims

This checkpoint does not implement `CreateMutexA`, general relative-path
resolution, a directory/file namespace, file open semantics, host filesystem
access, VFS overlay/persistence, arbitrary Windows path normalization, title
rendering, audio, COM, or full play.
