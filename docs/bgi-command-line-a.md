# BGI GetCommandLineA

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-command-line-a`
- Previous capability checkpoint: `bgi-set-handle-count`
- First unsupported capability: `kernel32-environment-strings-w`

## Purpose

The cumulative BGI session reached `kernel32.dll!GetCommandLineA()`. This
checkpoint exposes a process-lifetime stable ANSI pointer derived from the
existing guest process-parameter command line.

```text
RTL_USER_PROCESS_PARAMETERS.CommandLine (UTF-16 authority)
  -> bounded ASCII compatibility view in the same process-owned page
  -> zero-argument stdcall returns the stable readable pointer
  -> LastError preserved, no write during the call
  -> GetEnvironmentStringsW() structured stop
```

The runtime does not copy a host pointer or the host launch path into guest
state. Its controlled command line remains `BGI.exe`; the ANSI view is derived
from that same authority.

## Native evidence

Two opt-in `--observe command-line-a` captures agree:

| Capture | Bytes | Lines | SHA-256 | BGI LastError |
|---|---:|---:|---|---|
| Primary | 2,252,782 | 7,070 | `ad7babe19c454f8c2888cc97da47e830f527d8761904c02cd9dd180ce8c7fd84` | `0 -> 0` |
| Confirmation | 251,449 | 812 | `9172c5a03ad171f54d5dc1586267f7e5f578aa0e620e5c295db60747400668ee` | `87 -> 87` |

Both observe BGI caller return RVA `0xA3786`. Native Windows returns the
30-byte NUL-terminated CP932 view of
`"V:\softs\サクラノ詩\BGI.exe"`. Repeated `GetCommandLineA` and
`GetCommandLineW` oracle calls retain one pointer per encoding; the two
pointers are distinct and decode to the same string. Absolute pointer values
vary between processes and are evidence identities only.

The immediate consumer stores the pointer for later process-wide use:

```text
0x004A3780  call dword ptr [0x004D11FC]
0x004A3786  mov  dword ptr [0x00553520], eax
```

The later consumer at `0x004A81D2` reloads the pointer, checks NUL and space,
tracks double quotes, and advances through the executable token. Another
consumer at `0x004A84C6` parses the same retained pointer into argc/argv.
This makes process-lifetime readability and pointer stability semantic; the
native numeric address and full host path are not.

## Runtime contract

`kernel32-command-line-a-v1.json` enables the ANSI command-line capability.
During bootstrap, the existing UTF-16 `BGI.exe\0` remains at `0x7FFDD000`.
An 8-byte ASCII `BGI.exe\0` view is placed at the next 16-byte-aligned offset,
`0x7FFDD010`, in the same sealed command-line page. Rust registers the four
contiguous process-environment pages as one permanent
`ProcessAddressAllocationPurpose::ProcessEnvironment` record and stores the
wide/ANSI pointer relation in `RuntimeProcessState`.

The semantic handler contains no BGI name, caller RVA, host path, or native
pointer. It verifies that the bounded NUL-terminated view remains readable,
then returns its process-owned guest pointer through
`kernel32.get-command-line-a.v1`.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_command_line_a_returns_stable_process_owned_ascii_view_without_mutation `
  -- --exact
```

The test fixes two equal returns, distinct wide/ANSI pointers, allocation
ownership, zero-argument stack cleanup, LastError preservation, readable bytes,
no guest write during either call, and unchanged runtime/memory state.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-command-line-a
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 25 / 62 | 25 / 62 |
| Instruction at next stop | 1,536 | 1,536 |
| CPU writeback batches / dirty pages | 63 / 107 | 178 / 218 |
| Host mapping deltas | 30 | 30 |
| Final generation | 122 | 227 |
| Memory content SHA-256 | `33986edc39e1f5ecb9f050add5e9f534d2e1d81cc31d003ccd5060e1d30806f2` | same |
| Report bytes | 288,041 | 59,264 |
| Report SHA-256 | `efa02444d94353422ff0d9a842045025a89a0adb9cf7f90f6d3a87a616b84668` | `8b7f9de2a1d27313fb0559569872e8b98580dafb184030f13dacc6cc601efc58` |

Chrome used 218 slices, 155 budget yields, 274 Rust/Wasm bridge operations,
and 655 execution-state transitions. Success, cancel, and watchdog all end in
`stopped` with zero pending request records.

## Next structured stop

```text
kernel32.dll!GetEnvironmentStringsW()
static endpoint ID: 98
endpoint address:   0x7F000610
IAT address:        0x100D1230
return address:     0x100A855F
instruction:        1536
fault:              endpoint-contract-missing
```

The next checkpoint must trace the native environment block shape, ownership,
ordering, termination, LastError, paired `FreeEnvironmentStringsW`, and the
immediate UTF-16 consumer before adding any environment variables.

## Non-claims

This checkpoint does not implement non-ASCII guest command-line conversion,
command-line mutation, host-path import, argument injection, environment
expansion, `GetCommandLineW`, or environment-string allocation/free semantics.
