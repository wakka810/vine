# BGI standard error handle

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-standard-error-handle`
- Previous capability checkpoint: `bgi-standard-output-file-type`
- First unsupported capability: `kernel32-standard-error-file-type`

## Purpose

The cumulative BGI session reached
`kernel32.dll!GetStdHandle(STD_ERROR_HANDLE)`. This checkpoint adds only the
process-owned logical stderr identity through the existing
`kernel32.get-std-handle.v2` handler and a versioned compatibility policy.

```text
checked one-argument stdcall frame
  -> versioned STD_ERROR_HANDLE selector policy
  -> process-owned logical stderr registration / reuse
  -> 0x53010003, LastError preserved
  -> GetFileType(0x53010003) structured stop
```

Stderr file classification, console writes, redirection, close behavior, and
general file I/O are not implemented.

## Native evidence

The two existing opt-in `--observe standard-handle` captures remain the source:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 279,633 | 928 | `ececb814fe84a5f1495f3c6fa1fd41801fed12deee8b2699f961db4418f36de9` |
| Confirmation | 246,261 | 820 | `e5979f91d1f77977fc93d9785679190bafefd9afb3b82ec83c27eba9fa42766e` |

Both captures contain the BGI-owned call at caller return RVA `0xA87B6`:

| Selector | Native result | LastError |
|---:|---:|---:|
| `0xFFFFFFF4` / `STD_ERROR_HANDLE` | `0x238` | `87 -> 87` |

The host value proves non-null, non-invalid success but is discarded. Separate
file-type captures observe stderr handles `0x238` and `0x240`; both return
`FILE_TYPE_PIPE (3)` with LastError `87 -> 87`. That classification remains the
next checkpoint rather than being inferred here.

The same helper used for stdin and stdout resumes with:

```asm
mov  edi, eax
cmp  edi, -1
je   failure
test edi, edi
je   failure
push edi
call dword ptr [0x004D123C] ; KERNEL32.dll!GetFileType
```

The runtime independently reaches that exact IAT endpoint eight guest
instructions after returning logical stderr.

## Runtime contract

[`kernel32-standard-error-v1.json`](../fixtures/runtime-policies/kernel32-standard-error-v1.json)
enables stdin, stdout, and stderr selectors while retaining only the reached
stdin／stdout file classifications and the two reached heap allocations.
Historical policies remain unchanged.

`RuntimeState` owns logical stderr `0x53010003` under owner
`runtime-process-standard-handles`, namespace
`vine.logical-standard-handle.v1`, and variant
`standard-error-logical-v1`. Repeated lookup returns the same identity. An
unlisted selector fails before runtime or memory mutation.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_std_handle_policy_adds_only_process_owned_standard_error_identity `
  -- --exact
```

The test fixes one-argument cleanup, LastError preservation, identity reuse, no
guest write, no memory mutation, absent stderr classification, and mutation-free
unsupported-selector rejection.

## Generic continuation

```powershell
npm run check:runtime:bgi-standard-error-handle
```

| Evidence | Result |
|---|---:|
| Implemented API contracts | 23 |
| Completed generic calls | 58 |
| `GetStdHandle` calls | 3 |
| Initial reflection | 389 pages, generation 27 |
| CPU writeback batches / dirty pages | 59 / 100 |
| CPU-changed bytes | 971 |
| Host delta batches | 29 |
| Final synchronized generation | 117 |
| Memory content SHA-256 | `a97f019fb34b6ab90d9cbf76f35b194674296de83088cb1736cbdbad090b5b12` |
| Report bytes | 271,184 |
| Report SHA-256 | `bfa1da3badbbd96ed5c282bd21c3bb0041fa06834dbad38fe0af32e116432c2f` |

The stderr completion is sequence 58 at instruction 1,484 and emits no guest
write or host memory delta.

## Browser and performance

Chrome reproduces all 58 ordered completions and the same memory digest at
instruction 1,492. It uses 210 slices, 151 budget yields, 171 writeback batches
over 208 dirty pages, 262 Rust/Wasm bridge operations, and final generation 219.
Cancel remains at 8 instructions and the watchdog at 4 instructions with zero
pending work. The 53,865-byte Browser report has SHA-256
`b1a1c326681abbdc24c5e8af8c420f3b99f87f8bcfdff84e106453fde40f694b`.

The unchanged 180-second version 1 gate completed 1,771 sessions with no
failure: p95 session latency 96.485 ms, p99 maximum slice 0.905 ms, main heap
slope -326,902 bytes/min, Worker heap slope 2,510,664 bytes/min, and Worker
backing slope 1,402,825 bytes/min. All 16 thresholds passed. The 787,612-byte
report has SHA-256
`0ec70344a6880213e5bb504139f0e98c54c6b127a69cca501eaf3337a01c45a3`.
Detailed evidence is in [bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Next structured stop

```text
kernel32.dll!GetFileType(0x53010003)
static endpoint ID: 101
endpoint address:   0x7F000640
IAT address:        0x100D123C
return address:     0x100A87C8
instruction:        1492
fault:              file-type-standard-handle-unsupported
```

The next checkpoint must classify only the registered logical stderr identity
from the existing native evidence. It must not add console writes, redirection,
close behavior, or broad file semantics.

That checkpoint is now complete in
[bgi-standard-error-file-type.md](bgi-standard-error-file-type.md). This file
remains the historical stderr-handle checkpoint.
