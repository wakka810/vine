# Bootstrap process-parameters ingress

## 1. Status

- Status: Verified controlled Rust ownership and source-v86 completion
- Date: 2026-07-11
- Fixture: `.work/toolchain-spike/bootstrap-params.exe`
- Fixture size／SHA-256: 2,560 bytes／`50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40`
- Report schema: `vine.v86-bootstrap-params-ingress` version `1`
- Report size／SHA-256: 10,521 bytes／`c1e76294d4bba6760cbc8d022e022ef66b557d370b567d9a498798b8b2f7ac66`

This checkpoint continues [v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md). Rust now owns the two pages needed for the measured command-line path, connects them to the existing PEB, and runs the same native-oracled fixture to result `0` and the exact process-entry sentinel.

## 2. Owned layout

The new mode keeps the existing image, control, stack, TEB, and PEB owners and adds two typed anonymous owners:

| Owner | Region kind | Guest range | Protection | Initialized content |
|---:|---|---|---|---|
| `6` | `process-parameters` | `[0x7FFDC000,0x7FFDD000)` | RW／NX | command-line `UNICODE_STRING` at `+0x40` |
| `7` | `command-line` | `[0x7FFDD000,0x7FFDE000)` | RW／NX | UTF-16 `bootstrap-params.exe` plus NUL |

The measured fields are initialized as follows:

```text
*(PEB 0x7FFDF000 + 0x10) = 0x7FFDC000

ProcessParameters 0x7FFDC000 + 0x40:
  Length        = 40
  MaximumLength = 42
  Buffer        = 0x7FFDD000
```

`Length` excludes the trailing UTF-16 NUL and `MaximumLength` includes it. The command line has 20 UTF-16 units. Exact page SHA-256 values are:

| Page | SHA-256 |
|---|---|
| TEB | `5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e` |
| PEB with `+0x10` pointer | `780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39` |
| Process parameters | `5fb4eed3eb10240c39a428c43621e7198ccf0c3089ffff6e6aea233759533f75` |
| Command line | `f6e0abfe79cb7fafa42be5f1450e91539b3d0386f7140523e44109249f1e19bd` |

All four bootstrap owners are sealed after initialization. The pages remain logically writable through normal guest access; sealing prevents further initialization writes and fixes the reflected starting state.

## 3. Reflection and execution

`npm run check:runtime:bootstrap-params` reflects 11 production pages at generation `21`. The combined content SHA-256 is `f9a59953a06999030e40e962a007f1ec5f0ecb73b077c4df760e212ba3d3acbb`.

Rust reports 7 owners／regions, 26 reserved pages, and 11 committed／physical／dirty pages. The adapter uses one page directory, four page tables, and 11 reflected pages: 16 physical pages／64 KiB total. Both new virtual pages remain in directory index `511`; no new page-table class is introduced.

After generation acknowledgement, the probe reads back the PEB, the first 72 process-parameters bytes, and all 42 command-line bytes through normal guest access. Source-v86 then observes:

| Field | Result |
|---|---:|
| PEB | `0x7FFDF000` |
| Process parameters | `0x7FFDC000` |
| Command-line Length／MaximumLength | `40`／`42` |
| Command-line buffer | `0x7FFDD000` |
| Basename match index | `0` |
| Entry result | EAX `0` |
| Final ESP | `0x00700000` |
| Execute／budget stops | `94`／`93` |
| Completion trap | process-entry sentinel `0x7F010010` |

Two consecutive runs produced the same reflection and report hashes. The zero-FS, minimum TEB／PEB, and zero process-parameters reports also remained byte-identical.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:bootstrap-params
```

## 4. Transaction and preflight

Command-line UTF-16 conversion, embedded NUL, `UNICODE_STRING.Length` and
`MaximumLength`, one-page capacity, four fixed ranges, field-address arithmetic,
owner／region quotas, and four physical pages are validated before creating the
TEB owner. The measured layout explicitly rejects multi-page command lines as
`multi-page-command-line-unimplemented`.

TEB, PEB, process-parameters, and command-line owners are then created inside a
single `GuestMemory` transaction. Failpoints after each reserve, commit, zero,
pointer／length／buffer write, command-line copy, and owner seal restore owners,
regions, frames, page bytes／protection, seal state, generation, dirty state, and
pending delta. Every injected failure is followed by a successful retry on the
same `GuestMemory`.

## 5. Non-claims and next checkpoint

The fixed addresses, one page per structure, exact command-line text, and zero-filled unmeasured bytes are version 1 checkpoint choices. This is not a general `RTL_USER_PROCESS_PARAMETERS` implementation. Vine does not yet provide image path, current directory, DLL path, environment, standard handles, console／window fields, loader lists, TLS, LastError, process heap, multi-thread lifecycle, or mutation writeback from adapter pages.

The native oracle supports only the measured i386 offsets used here. It does not establish undocumented layout compatibility beyond those fields, and this controlled fixture is not evidence that BGI has entered guest execution.

The explicit binding and BGI execution continuations are complete in [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md) and [v86-bgi-first-trap.md](v86-bgi-first-trap.md). The primary run uses the same measured fields with controlled command line `BGI.exe` and reaches exact thunk ID `104` without expanding this minimum process-parameters claim.
