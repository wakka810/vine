# BGI standard output handle

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-standard-output-handle`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-file-type`
- First unsupported capability: `kernel32-standard-output-file-type`

## Purpose

The cumulative BGI session reached static endpoint ID `94`,
`kernel32.dll!GetStdHandle(STD_OUTPUT_HANDLE)`. This checkpoint extends the
existing one-argument contract with one process-owned logical standard output
identity and returns it through the same generic dispatcher.

The implemented boundary is:

```text
checked one-argument stdcall frame
  -> versioned input/output selector profile
  -> process-owned logical stdout registry
  -> stable handle 0x53010002 in vine.logical-standard-handle.v1
  -> LastError preserved, no guest write or host MemoryDelta
  -> GetFileType(logical stdout) structured stop
```

Standard error, output file type, console writes, redirection, close behavior,
and general file I/O are not part of this checkpoint.

## Native evidence

The two existing opt-in `--observe standard-handle` captures remain under
`.work/`:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 279,633 | 928 | `ececb814fe84a5f1495f3c6fa1fd41801fed12deee8b2699f961db4418f36de9` |
| Confirmation | 246,261 | 820 | `e5979f91d1f77977fc93d9785679190bafefd9afb3b82ec83c27eba9fa42766e` |

Both captures contain the BGI-owned call at caller return RVA `0xA87B6`:

| Selector | Native result | LastError |
|---:|---:|---:|
| `0xFFFFFFF5` / `STD_OUTPUT_HANDLE` | `0x234` | `87 -> 87` |

The result proves non-null, non-invalid success but is not copied into guest
state. The later `file-type` captures observe output handles `0x234` and
`0x23C`, both passed unchanged to `GetFileType` and classified natively as pipe.
That changing host value is additional evidence that the logical runtime
identity must be stable and independent of the native handle number.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-standard-handle.jsonl `
  --seconds 2 --kill-after `
  --observe standard-handle
```

## Immediate consumer

The same helper used for stdin resumes at preferred image VA `0x004A87B6`:

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
instructions after returning logical stdout. Output classification remains the
next unsupported capability rather than being inferred from the native pipe.

## Runtime contract

The current runtime uses one `kernel32.get-std-handle.v2` ABI and handler.
`kernel32-standard-output-v1` allows exactly two selectors:

| Selector | Logical handle | Variant |
|---:|---:|---|
| `0xFFFFFFF6` | `0x53010001` | `standard-input-logical-v1` |
| `0xFFFFFFF5` | `0x53010002` | `standard-output-logical-v1` |

Both records use owner `runtime-process-standard-handles` and namespace
`vine.logical-standard-handle.v1`. The frozen
`kernel32-controlled-v1` remains input-only. Historical report hashes retain
their capture-time implementation IDs as artifact identity; semantic replay
uses the current policy boundary. Standard error still fails before mutation.

The stdin-only `GetFileType` handler now also verifies the resolved record's
selector. A registered stdout handle therefore returns structured
`file-type-standard-handle-unsupported` without changing runtime state, memory,
registers, or LastError.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_std_handle_policy_adds_only_process_owned_standard_output_identity `
  -- --exact
```

The test fixes stdout identity reuse, one-argument cleanup, LastError
preservation, no guest writes, standard-error rejection, and mutation-free
output-file-type rejection.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi-standard-output-handle
```

| Evidence | Result |
|---|---:|
| Implemented API contracts | 23 |
| Completed generic calls | 55 |
| `GetStdHandle` calls | 2 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 56 / 95 |
| CPU-changed bytes | 943 |
| Host delta batches | 28 |
| Final synchronized generation | 113 |
| Report bytes | 257,878 |
| Report SHA-256 | `564c3425f01b35e86717b68335b18f8bcd3d30d996717a1d0848028104cf0a2b` |

The stdout completion itself emits no guest write or host delta. The additional
writeback contains ordinary guest stack activity while the consumer call is
prepared. Historical report hashes remain artifact identities; ordered
completion, memory content, and the structured stop are the semantic gate.

## Next structured stop

```text
kernel32.dll!GetFileType(0x53010002)
static endpoint ID: 101
endpoint address:   0x7F000640
IAT address:        0x100D123C
return address:     0x100A87C8
instruction:        1447
```

The next checkpoint must measure and model only the reached stdout
classification. It must not add standard error, console writes, redirection,
close behavior, or general file I/O at the same time.

That checkpoint is now complete in
[bgi-standard-output-file-type.md](bgi-standard-output-file-type.md). This file
remains the historical stdout-identity checkpoint.
