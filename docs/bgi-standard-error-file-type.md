# BGI standard error file type

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-standard-error-file-type`
- Previous capability checkpoint: `bgi-standard-error-handle`
- First unsupported capability: `kernel32-set-handle-count`

## Purpose

The cumulative BGI session reached
`kernel32.dll!GetFileType(0x53010003)`. This checkpoint classifies only the
process-owned logical stderr identity as `FILE_TYPE_PIPE (3)` through the
existing `kernel32.get-file-type.v2` handler and a versioned compatibility
policy.

```text
checked one-argument stdcall frame
  -> process-owned logical stderr lookup
  -> versioned stderr file-type policy
  -> FILE_TYPE_PIPE (3), LastError preserved
  -> 17th critical-section initialization
  -> SetHandleCount(32) structured stop
```

`SetHandleCount`, console writes, redirection, close behavior, and general file
I/O are not implemented.

## Native evidence

The two existing opt-in `--observe file-type` captures remain the source:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 246,422 | 819 | `480e2e9e7901535f88cd6dd87994bc7da97c47cd90ee0eb77dbef1149bfb7a9f` |
| Confirmation | 261,838 | 872 | `9986a349350e6875c779f35582347dbc485618492f7a68afa3716064ba1f3f02` |

At BGI caller return RVA `0xA87C8`, native stderr handles differ while the
classification remains stable:

| Capture | Native handle | Result | LastError |
|---|---:|---:|---:|
| Primary | `0x238` | `3` / `FILE_TYPE_PIPE` | `87 -> 87` |
| Confirmation | `0x240` | `3` / `FILE_TYPE_PIPE` | `87 -> 87` |

The host values are discarded. The caller checks for zero, masks the low byte,
stores the unchanged handle, distinguishes character `2` from pipe `3`, and
initializes the stream's critical section. The runtime continuation then reaches
`SetHandleCount(32)`.

## Runtime contract

[`kernel32-standard-error-file-type-v1.json`](../fixtures/runtime-policies/kernel32-standard-error-file-type-v1.json)
enables stdin, stdout, and stderr selectors; stdin type `2`; stdout／stderr type
`3`; and the two reached heap allocations. Historical policies remain
unchanged. The handler resolves `vine.logical-standard-handle.v1` through
authoritative `RuntimeState`; the policy selects a value only after that lookup.
Unknown handles or a policy without stderr classification fail before mutation.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_file_type_policy_classifies_process_owned_standard_error_as_pipe `
  -- --exact
```

The test fixes return `3`, one-argument stdcall cleanup, LastError preservation,
no guest write, no runtime／memory mutation, and default-policy rejection.

## Generic continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-standard-error-file-type
```

| Evidence | Result |
|---|---:|
| Implemented API contracts | 23 |
| Completed generic calls | 60 |
| `GetFileType` calls | 3 |
| Initial reflection | 389 pages, generation 27 |
| CPU writeback batches / dirty pages | 61 / 104 |
| CPU-changed bytes | 991 |
| Host delta batches | 30 |
| Final synchronized generation | 120 |
| Memory content SHA-256 | `5bb58fba2f503a84550a71e0ba35f11049e77ebb9038461db91f04d0dfb3bb61` |
| Report bytes | 280,475 |
| Report SHA-256 | `ce9d4271db746b1b7d377bb3e42a861f9c611367e5fcfefba0f19b35fca5aee1` |

The stderr completion is sequence 59 at instruction 1,492 and emits no guest
write. Existing guest code prepares the 17th critical section at `0x200002A4`;
its checked initialization is sequence 60 at instruction 1,506.

## Browser and performance

Chrome reproduces all 60 ordered completions and the same memory digest at
instruction 1,516. It uses 214 slices, 153 budget yields, 175 writeback batches
over 214 dirty pages, 269 Rust/Wasm bridge operations, and final generation 224.
Cancel remains at 8 instructions and the watchdog at 4 instructions with zero
pending work. The 55,478-byte Browser report has SHA-256
`e880a1634ade8238295fc563a424542b23f2152a06325b8995c050c237c47c93`.

The unchanged 180-second version 1 gate completed 1,807 sessions with no
failure: p95 session latency 95.215 ms, p99 maximum slice 0.860 ms, main heap
slope -374,792 bytes/min, Worker heap slope 2,988,378 bytes/min, and Worker
backing slope 7,557,701 bytes/min. All 16 thresholds passed. The 803,761-byte
report has SHA-256
`1a4d5d4ce456c91cf8db2d26c5a534df640cfa932134c5cc0f0d789ed0a0d152`.
Detailed evidence is in [bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Next structured stop

```text
kernel32.dll!SetHandleCount(32)
static endpoint ID: 99
endpoint address:   0x7F000620
IAT address:        0x100D1234
return address:     0x100A881F
instruction:        1516
fault:              endpoint-contract-missing
```

The next checkpoint must measure the reached native result, LastError, and
consumer before adding only this one API contract. It must not infer general
handle-table sizing, file I/O, or console behavior.

That checkpoint is now complete in
[bgi-set-handle-count.md](bgi-set-handle-count.md). This file remains the
historical stderr-file-type checkpoint.
