# BGI standard input file type

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-file-type`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-standard-input-handle`
- First unsupported capability: `kernel32-standard-output-handle`

## Purpose

The cumulative BGI session reached static endpoint ID `101`,
`kernel32.dll!GetFileType(0x53010001)`. This checkpoint measures the native
standard input classification and returns the corresponding value only for the
process-owned logical stdin identity.

The implemented boundary is:

```text
checked one-argument stdcall frame
  -> logical standard-handle namespace
  -> authoritative process registry lookup
  -> FILE_TYPE_CHAR (2)
  -> LastError preserved, no guest write
  -> existing critical-section initialization
  -> STD_OUTPUT_HANDLE structured stop
```

Standard output/error classification, console behavior, read/write, close,
redirection, and broad file handles are not part of this checkpoint.

## Native evidence

Two opt-in `--observe file-type` captures remain under `.work/`:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 246,422 | 819 | `480e2e9e7901535f88cd6dd87994bc7da97c47cd90ee0eb77dbef1149bfb7a9f` |
| Confirmation | 261,838 | 872 | `9986a349350e6875c779f35582347dbc485618492f7a68afa3716064ba1f3f02` |

Both captures contain the same BGI-owned call at caller return RVA `0xA87C8`:

| Capture | Native stdin handle | Result | LastError |
|---|---:|---:|---:|
| Primary | `0x208` | `2` / `FILE_TYPE_CHAR` | `87 -> 87` |
| Confirmation | `0x210` | `2` / `FILE_TYPE_CHAR` | `87 -> 87` |

The changing native handle proves that its numeric value is not guest state.
For context only, the later native output/error handles return `3` / pipe; those
variants remain outside the implemented profile.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-file-type.jsonl `
  --seconds 2 --kill-after `
  --observe file-type
```

## Immediate consumer

The caller at preferred image VA `0x004A87C8` checks for zero, masks the low
byte, stores the unchanged handle, and distinguishes character (`2`) from pipe
(`3`):

```asm
test eax, eax
je   failure
and  eax, 0xff
mov  [esi], edi
cmp  eax, 2
jne  check_pipe
or   byte ptr [esi + 4], 0x40
```

It then calls existing `InitializeCriticalSectionAndSpinCount(esi + 0x0C,
4000)`. The runtime continuation independently reaches that 15th occurrence at
guest address `0x20000224` before looping to `STD_OUTPUT_HANDLE`.

## Runtime contract

`kernel32.get-file-type-standard-input.v1` uses:

- one non-null logical handle in `vine.logical-standard-handle.v1`;
- x86 stdcall with four argument bytes;
- `U32` return value `2`;
- LastError preserve policy;
- immediate, callback-free execution.

The handler resolves the argument through `RuntimeState::standard_handle_by_value`.
Unknown logical handles fail before state, memory, registers, or LastError
change. No native handle number or host file operation enters the handler.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_file_type_contract_classifies_only_process_owned_standard_input `
  -- --exact
```

The test fixes `FILE_TYPE_CHAR`, stdcall cleanup, LastError preservation, no
guest writes, and mutation-free rejection of an unregistered logical handle.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-file-type
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 23 |
| Completed generic calls | 54 |
| `GetFileType` calls | 1 |
| Critical-section initializations | 15 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 55 / 94 |
| CPU-changed bytes | 938 |
| Host delta batches | 28 |
| Final synchronized generation | 112 |
| Report bytes | 252,994 |
| Report SHA-256 | `dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf` |

The `GetFileType` completion itself emits no guest write or host delta. Guest
code stores the logical handle and flags into the second heap block; the
existing critical-section contract writes 24 checked bytes at `0x20000224`.

All ten earlier reports remain byte-identical, including the 243,749-byte
standard-input-handle report.

## Next structured stop

```text
kernel32.dll!GetStdHandle(STD_OUTPUT_HANDLE / 0xFFFFFFF5)
static endpoint ID: 94
endpoint address:   0x7F0005D0
IAT address:        0x100D1220
return address:     0x100A87B6
instruction:        1439
```

The existing input-only contract returns
`standard-handle-selector-unsupported` without mutation. The next checkpoint
must measure and model only the reached standard output identity. It must not
add standard error, output file type, console writes, redirection, or general
file I/O at the same time.
