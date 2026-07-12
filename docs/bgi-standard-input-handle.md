# BGI standard input handle

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-standard-input-handle`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-private-heap-second-allocation`
- First unsupported capability: `kernel32-file-type`

## Purpose

The cumulative BGI session reached static endpoint ID `94`,
`kernel32.dll!GetStdHandle(STD_INPUT_HANDLE)`. This checkpoint measures the
native call, creates one process-owned logical standard input identity, and
returns it through the existing generic dispatcher.

The implemented boundary is:

```text
checked one-argument stdcall frame
  -> exact STD_INPUT_HANDLE selector gate
  -> process-owned logical stdin registry
  -> stable handle 0x53010001 in vine.logical-standard-handle.v1
  -> no guest write or host MemoryDelta
  -> LastError preserved
  -> immediate GetFileType(logical stdin) trap
```

Native handle values are host-process resources and are discarded. Standard
output, standard error, `SetStdHandle`, close behavior, file type, console
semantics, and file I/O are not part of this checkpoint.

## Native evidence

Two opt-in `--observe standard-handle` captures remain under `.work/`:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 279,633 | 928 | `ececb814fe84a5f1495f3c6fa1fd41801fed12deee8b2699f961db4418f36de9` |
| Confirmation | 246,261 | 820 | `e5979f91d1f77977fc93d9785679190bafefd9afb3b82ec83c27eba9fa42766e` |

Both captures contain the same BGI-owned pair at caller return RVA `0xA87B6`:

| Selector | Native result | LastError |
|---:|---:|---:|
| `0xFFFFFFF6` / `STD_INPUT_HANDLE` | `0x208` | `87 -> 87` |

The native result is recorded only to prove a non-null, non-invalid success.
It is not copied into runtime state. The same helper subsequently requests
standard output and error, but those later invocations remain unsupported.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-standard-handle.jsonl `
  --seconds 2 --kill-after `
  --observe standard-handle
```

## Immediate consumer

Static disassembly at preferred image VA `0x004A87B6` starts with these 17
bytes:

```text
8b f8 83 ff ff 74 42 85 ff 74 3e 57 ff 15 3c 12 4d 00
```

They decode to:

```asm
mov edi, eax
cmp edi, -1
je  failure
test edi, edi
je  failure
push edi
call dword ptr [0x004D123C] ; KERNEL32.dll!GetFileType
```

This proves the immediate consumer rejects `INVALID_HANDLE_VALUE` and NULL,
then passes the unchanged handle to `GetFileType`. The runtime continuation
independently reaches that exact IAT endpoint with logical handle `0x53010001`.

## Runtime contract

`kernel32.get-std-handle-input.v1` uses:

- one `U32` selector argument;
- x86 stdcall with four argument bytes;
- non-null logical-handle return in `vine.logical-standard-handle.v1`;
- LastError preserve policy;
- immediate, callback-free execution.

Only selector `0xFFFFFFF6` is accepted. The runtime lazily creates one record:

| Field | Value |
|---|---|
| owner | `runtime-process-standard-handles` |
| variant | `standard-input-logical-v1` |
| namespace | `vine.logical-standard-handle.v1` |
| handle | `0x53010001` |
| selector | `0xFFFFFFF6` |

Repeated input lookup returns the same identity. Other selectors fail before
state mutation. The registry is omitted from serialized state while empty, so
all earlier reports remain byte-identical.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_std_handle_contract_owns_only_reached_standard_input_identity `
  -- --exact
```

The test fixes identity reuse, one-argument cleanup, LastError preservation,
no guest memory mutation, and mutation-free rejection of standard output.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi-standard-input-handle
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 22 |
| Completed generic calls | 52 |
| `GetStdHandle` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 53 / 90 |
| CPU-changed bytes | 911 |
| Host delta batches | 27 |
| Final synchronized generation | 109 |
| Report bytes | 243,749 |
| Report SHA-256 | `5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e` |

The API itself produces no guest write and no host delta. The additional
writeback is the ordinary stack-page reconciliation accumulated while guest
execution advances to and beyond the call.

All nine earlier runtime-kernel reports remain byte-identical, including the
239,789-byte second-allocation report.

## Next structured stop

```text
kernel32.dll!GetFileType(0x53010001)
static endpoint ID: 101
endpoint address:   0x7F000640
IAT address:        0x100D123C
return address:     0x100A87C8
instruction:        1403
```

The runtime returns `endpoint-contract-missing` in capability cluster
`kernel32-api-contract`. The next checkpoint must measure native
`GetFileType(stdin)` result and LastError, classify only the reached logical
stdin variant, and continue to the next exact trap. It must not add read/write,
console, output/error handle, or broad file behavior at the same time.
