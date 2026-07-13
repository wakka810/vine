# BGI standard output file type

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-standard-output-file-type`
- Previous capability checkpoint: `bgi-standard-output-handle`
- First unsupported capability: `kernel32-standard-error-handle`

## Purpose

The cumulative BGI session reached
`kernel32.dll!GetFileType(0x53010002)`. This checkpoint classifies only the
process-owned logical stdout identity as `FILE_TYPE_PIPE (3)` through the
existing generic `kernel32.get-file-type.v2` handler and a new versioned
compatibility policy.

```text
checked one-argument stdcall frame
  -> process-owned logical stdout lookup
  -> versioned stdout file-type policy
  -> FILE_TYPE_PIPE (3), LastError preserved
  -> 16th critical-section initialization
  -> GetStdHandle(STD_ERROR_HANDLE) structured stop
```

Standard error identity／classification, console writes, redirection, close,
and general file I/O are not implemented.

## Native evidence

The two existing opt-in `--observe file-type` captures remain the source:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 246,422 | 819 | `480e2e9e7901535f88cd6dd87994bc7da97c47cd90ee0eb77dbef1149bfb7a9f` |
| Confirmation | 261,838 | 872 | `9986a349350e6875c779f35582347dbc485618492f7a68afa3716064ba1f3f02` |

At BGI caller return RVA `0xA87C8`, native stdout handles differ while the
classification remains stable:

| Capture | Native handle | Result | LastError |
|---|---:|---:|---:|
| Primary | `0x234` | `3` / `FILE_TYPE_PIPE` | `87 -> 87` |
| Confirmation | `0x23C` | `3` / `FILE_TYPE_PIPE` | `87 -> 87` |

The native values are discarded. The caller masks the low byte, stores the
handle, distinguishes character `2` from pipe `3`, and initializes the next
critical section. The same helper then requests `STD_ERROR_HANDLE`.

## Runtime contract

[`kernel32-standard-output-file-type-v1.json`](../fixtures/runtime-policies/kernel32-standard-output-file-type-v1.json)
enables stdin／stdout selectors, stdin type `2`, stdout type `3`, and the two
previously reached heap allocations. Historical policies remain unchanged.
The handler resolves `vine.logical-standard-handle.v1` through authoritative
`RuntimeState`; the policy selects a value only after that lookup. Unknown
handles or a policy without stdout classification fail before mutation.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_file_type_policy_classifies_process_owned_standard_output_as_pipe `
  -- --exact
```

The test fixes return `3`, one-argument stdcall cleanup, LastError preservation,
no guest write, no runtime／memory mutation, and default-policy rejection.

## Generic continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-output-file-type
```

| Evidence | Result |
|---|---:|
| Implemented API contracts | 23 |
| Completed generic calls | 57 |
| `GetFileType` calls | 2 |
| Initial reflection | 389 pages, generation 27 |
| CPU writeback batches / dirty pages | 58 / 99 |
| CPU-changed bytes | 966 |
| Host delta batches | 29 |
| Final synchronized generation | 116 |
| Memory content SHA-256 | `909cca3e66d023f7bc2a6b9c8ab9f2435b17338fa850a14823983f513b1c1b68` |
| Report bytes | 267,223 |
| Report SHA-256 | `0bc3770b1da0af519e4065021c11cc423427b9855d9ceedac6d9b00053837301` |

The stdout completion is sequence 56 at instruction 1,447 and emits no guest
write. Existing guest code prepares the 16th critical section at `0x20000264`;
its checked 24-byte initialization is sequence 57 at instruction 1,461.

## Browser and performance

Chrome reproduces all 57 ordered completions and the same memory digest at
instruction 1,484. It uses 209 slices, 151 budget yields, 260 Rust/Wasm bridge
operations, and final generation 218. Cancel remains at 8 instructions and the
watchdog at 4 instructions with zero pending work.

The Runtime Worker previously used `setTimeout(0)` at every slice boundary.
Windows timer quantum changes made the same CPU work vary from about 1 to 3
seconds per session. A local `MessageChannel` now posts the next Worker task.
This keeps one task boundary per slice for cancellation without timer-clamp
latency. `scheduler.yield()` was rejected because its continuation priority
allowed the session to finish before the external cancel message.

The unchanged 180-second version 1 gate completed 1,750 sessions with no
failure: p95 session latency 97.010 ms, p99 maximum slice 0.810 ms, main heap
slope -295,486 bytes/min, Worker heap slope 1,981,278 bytes/min, and Worker
backing slope 2,506,239 bytes/min. All 16 thresholds passed. Detailed evidence
is in [bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Next structured stop

```text
kernel32.dll!GetStdHandle(STD_ERROR_HANDLE / 0xFFFFFFF4)
static endpoint ID: 94
endpoint address:   0x7F0005D0
IAT address:        0x100D1220
return address:     0x100A87B6
instruction:        1484
fault:              standard-handle-selector-unsupported
```

The next checkpoint must model only the reached standard-error identity from
native evidence. It must not infer console writes, redirection, close behavior,
or broad file semantics.

That checkpoint is now complete in
[bgi-standard-error-handle.md](bgi-standard-error-handle.md). This file remains
the historical stdout-file-type checkpoint.
