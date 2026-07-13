# BGI GetEnvironmentStringsW / FreeEnvironmentStringsW

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-environment-strings-w`
- Previous capability checkpoint: `bgi-command-line-a`
- First unsupported capability: `kernel32-wide-char-to-multi-byte`

## Purpose

The cumulative BGI session reached `kernel32.dll!GetEnvironmentStringsW()`.
This checkpoint adds a bounded process-owned UTF-16 environment authority and
the paired copy/free lifetime without importing the host environment.

```text
RTL_USER_PROCESS_PARAMETERS.Environment (controlled empty authority)
  -> GetEnvironmentStringsW acquires one distinct logical copy
  -> BGI scans through the double NUL
  -> BGI begins a two-pass WideCharToMultiByte conversion
  -> FreeEnvironmentStringsW remains available for the retained copy
```

## Native evidence

The final `--observe environment-strings-w` observer records block shape and
lifetime relations, not environment names or values. Two captures agree:

| Capture | Bytes | Lines | SHA-256 | BGI LastError |
|---|---:|---:|---|---|
| Primary | 263,415 | 819 | `c091345340239bb1bb450bac172d79c47a0d1f0123e2ba8e62ceac38d1f411a9` | `0 -> 0` |
| Confirmation | 264,022 | 820 | `4c94f4efe5fee18a752dbafd4577b36d88b408530ca383b7d076cf135a69cba9` | `87 -> 87` |

The observed host block had 46 entries and occupied 7,274 bytes including the
UTF-16 double NUL. Those values describe the launch environment only and are
not runtime requirements. The final observer's two oracle acquisitions returned
distinct readable copies with equal shape/content comparison hashes. Neither
pointer equaled `RTL_USER_PROCESS_PARAMETERS.Environment`. LastError sentinels
`0` and `87` were preserved. Freeing each exact pointer returned BOOL `1` and
preserved sentinels `88` and `89`.

BGI receives the block at return RVA `0xA855F`, scans to the double NUL, and
calls `WideCharToMultiByte` twice over the entire block. It then passes the
original pointer to `FreeEnvironmentStringsW` at return RVA `0xA85D3`; native
Free returns `1` and preserves LastError. The copy is used synchronously and is
not retained after Free.

## Runtime contract

Policy `kernel32-environment-strings-w-v1` layers this capability over the
existing ANSI command-line profile. The controlled environment has zero
entries. Its exact byte representation is four zero bytes: two UTF-16 NUL code
units.

The existing process-environment allocation remains the only address-space
record. No new fixed arena is introduced:

| View | Guest address | Role |
|---|---:|---|
| UTF-16 command line | `0x7FFDD000` | existing process-parameter authority |
| ANSI command line | `0x7FFDD010` | existing `GetCommandLineA` view |
| UTF-16 environment | `0x7FFDD020` | internal process-parameter authority |
| UTF-16 copy | `0x7FFDD030` | one active Get/Free lease |

`RTL_USER_PROCESS_PARAMETERS.Environment` at offset `0x48` points to the
internal authority. `GetEnvironmentStringsW` verifies that authority and copy
still have the exact double-NUL shape, activates one bounded copy lease, and
returns the distinct copy pointer. It writes no guest bytes and preserves
LastError. A concurrent second acquisition stops with
`wide-environment-copy-limit-reached` before mutation.

`FreeEnvironmentStringsW` accepts only the active copy pointer, retires the
lease, returns BOOL `1`, writes no guest bytes, and preserves LastError. Foreign
or duplicate frees stop before state mutation. The bytes remain in the sealed
process allocation; logical lifetime, equality, and accepted pointer identity
are the guest-visible contract.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_and_free_environment_strings_w_manage_one_process_owned_copy `
  -- --exact
```

The test fixes the process-parameter pointer, distinct authority/copy
addresses, double-NUL bytes, zero-argument and one-argument stdcall cleanup,
LastError preservation, one-active-copy limit, exact free pointer, duplicate
and foreign free rejection, no guest writes, and unchanged memory statistics.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-environment-strings-w
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 27 / 63 | 27 / 63 |
| Instruction at next stop | 1,558 | 1,558 |
| CPU writeback batches / dirty pages | 64 / 108 | 181 / 221 |
| Host mapping deltas | 30 | 30 |
| Final generation | 123 | 230 |
| Memory content SHA-256 | `20109e02acad2ff74cc74520f10b4bba715479f87a1fec2ae000be3f3e179481` | same |
| Report bytes | 291,793 | 59,752 |
| Report SHA-256 | `96cb98f09a1d2733442d38ce2736c15c38fd87f3f4fa082e020d623c3fdb6013` | `fae715d2bd580ae505f98b1ef8c3ce6102a42bcc5b200ba32fa8ca9e257cad91` |

Chrome used 221 slices, 157 budget yields, 278 Rust/Wasm bridge operations,
and 664 execution-state transitions. Success, cancel, and watchdog all end in
`stopped` with zero pending request records.

The separate 180-second workload gate completed 2,205 sessions with no failure.
All 16 versioned thresholds passed. The 989,269-byte report has SHA-256
`eca0b65a2b4300abc8ce69cd193d9dcc4af5d8d7c3f87c18a7feb4a4433025be`.

## Next structured stop

```text
kernel32.dll!WideCharToMultiByte(
  CodePage=CP_ACP, flags=0,
  source=0x7FFDD030, sourceUnits=1,
  destination=NULL, destinationBytes=0,
  defaultChar=NULL, usedDefaultChar=NULL)
static endpoint ID: 132
endpoint address:   0x7F000830
IAT address:        0x100D12B8
return address:     0x100A8599
instruction:        1558
fault:              endpoint-contract-missing
```

The bounded trap frame records seven argument words after the return address;
disassembly confirms the eighth `usedDefaultChar=NULL` push. The next checkpoint
must trace both query and conversion calls, CP_ACP behavior, invalid input,
default-character reporting, LastError, output sizing, and exact consumer before
adding a conversion contract.

## Non-claims

This checkpoint does not import host environment entries, expose environment
names/values in evidence, support environment mutation, concurrent copies,
`GetEnvironmentStringsA`, general process-heap allocation, arbitrary block
sizes, code-page conversion, or title output. `FreeEnvironmentStringsW` is
implemented and publicly tested but is not reached in the cumulative session
before the new `WideCharToMultiByte` stop.
