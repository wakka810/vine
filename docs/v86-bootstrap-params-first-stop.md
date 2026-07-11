# Bootstrap process-parameters first stop

## 1. Status

- Status: Verified native oracle and controlled first unsupported result
- Date: 2026-07-11
- Fixture: `.work/toolchain-spike/bootstrap-params.exe`
- Fixture size／SHA-256: 2,560 bytes／`50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40`
- Report schema: `vine.v86-bootstrap-params-first-stop` version `1`
- Report size／SHA-256: 8,351 bytes／`b1c05c3e9df1529196276c8905f3b97aee306610c870461d458a857c80aadde5`

This checkpoint fixes the minimum i386 process-parameters path before Vine supplies it. The fixture proves the selected x86 offsets against native Windows, then runs against the existing Rust-owned minimum TEB／PEB and returns an explicit failure result because Vine's PEB process-parameters pointer is still zero.

## 2. Native contract

`fixtures/pe32/bootstrap-params/fixture-bootstrap-params.c` is import-free and checks:

```text
FS:[0x30]                 -> PEB
*(PEB + 0x10)            -> RTL_USER_PROCESS_PARAMETERS
ProcessParameters + 0x40 -> CommandLine UNICODE_STRING
CommandLine + 0x04       -> UTF-16 buffer pointer
```

The fixture bounds `UNICODE_STRING.Length` to 32 KiB, requires an even nonzero length, `MaximumLength >= Length`, and a nonzero buffer. It scans no more than `Length / 2` UTF-16 units and requires case-insensitive ASCII basename `bootstrap-params.exe`. Native Windows returns exit code `0`, so the pointer and command-line layout identify the intended fields rather than merely reading nonzero memory.

The selected offsets are a measured i386 support profile. They do not promise undocumented PEB／`RTL_USER_PROCESS_PARAMETERS` layout beyond this contract.

## 3. Controlled guest comparison

`npm run check:runtime:bootstrap-params-first-stop` relocates the fixture to `0x10000000` and installs the previously verified image-independent bootstrap state:

- control owner `2` and process-entry sentinel `0x7F010010`
- stack owner `3`
- TEB owner `4` at `0x7FFDE000`
- zero PEB owner `5` at `0x7FFDF000`
- FS base `0x7FFDE000`

The nine-page generation `15` reflection has SHA-256 `51357b546920ec78aa535488ce8d280a8027805e1d1d24e138b85264fbab9c39`. The adapter allocation remains 14 pages／56 KiB with directory indices `1`, `64`, `508`, and `511`.

The guest PEB's DWORD at `+0x10` is exactly zero. The fixture writes the PEB address and zero process-parameters pointer to its `.data` page, returns its defined result `2`, and reaches process-entry sentinel ID `2` cleanly:

| Field | Result |
|---|---:|
| Entry result | EAX `2` (`process parameters missing`) |
| Final ESP | `0x00700000` |
| FS base | `0x7FFDE000` |
| PEB | `0x7FFDF000` |
| `*(PEB + 0x10)` | `0` |
| Command-line observations | all zero／not reached |
| Execute／budget stops | `8`／`7` |
| Completion trap | process-entry sentinel `0x7F010010` |

This is a logical unsupported result, not a CPU fault. It prevents an empty PEB from being mistaken for a successful process bootstrap.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:bootstrap-params-first-stop
```

## 4. Non-claims and next checkpoint

Vine does not yet own an `RTL_USER_PROCESS_PARAMETERS` region or UTF-16 command-line buffer. It does not initialize `PEB + 0x10`, current directory, image path, DLL path, environment, standard handles, console fields, loader lists, TLS, or LastError. The fixture's 32 KiB observation bound is a test limit, not a final command-line policy.

The Rust-owned continuation is complete in [v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md). It initializes only the measured `PEB + 0x10` pointer, command-line `UNICODE_STRING`, and UTF-16 bytes, then requires the same fixture to find `bootstrap-params.exe` and return `0` without changing this first-stop report.
