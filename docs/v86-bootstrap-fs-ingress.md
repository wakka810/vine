# Bootstrap FS ingress

## 1. Status

- Status: Verified controlled ingress
- Date: 2026-07-11
- Fixture: `.work/toolchain-spike/bootstrap-fs.exe`
- Fixture size／SHA-256: 2,560 bytes／`5712377ff8a8bd2be292f8678902d19d3b4b2f1c1919922cce9d373f0696637c`
- Report schema: `vine.v86-bootstrap-fs-ingress` version `1`
- Report size: 8,214 bytes
- Report SHA-256: `4e2e951d53c9a51792767b0fc506c5d8162fb889960ee24dcce9c8333c9670f7`

This checkpoint continues the exact zero-FS-base fault in [v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md). Rust now owns the minimum TEB／PEB pages, the adapter restores the TEB as FS base through CPU state ABI version `1`, and the same native-oracled fixture returns `0` to the existing process-entry sentinel.

## 2. Rust-owned environment pages

`InitialProcessEnvironment` uses separate anonymous owners and typed region kinds:

| Region | Owner | Range | Protection | Initialized content |
|---|---:|---|---|---|
| `thread-environment-block` | `4` | `[0x7FFDE000,0x7FFDF000)` | RW／NX | self pointer at `+0x18`, PEB pointer at `+0x30` |
| `process-environment-block` | `5` | `[0x7FFDF000,0x7FFE0000)` | RW／NX | zero page |

Both pages are explicitly zero-initialized before the two TEB pointers are written and both owners are sealed. The TEB page SHA-256 is `5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e`; the zero PEB page is `ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7`.

The fixed addresses and owner IDs are version 1 checkpoint choices. They are not a process-layout or ASLR policy.

## 3. Reflection and CPU state

The two page commits and two owner seals advance the prior stack generation `6` to generation `10`. Reflection contains nine pages with content SHA-256 `14a986f7f10229da07f54d11fd5eda29be02e0639f06aba6151307e35f0206f3`:

```text
five import-free image pages
one runtime control page
one Rust-owned initial stack page
one TEB page
one PEB page
```

Rust reports five owners／regions, 24 reserved pages, and nine committed／physical／dirty pages. The adapter uses 14 pages／56 KiB: one page directory, page tables for directory indices `1`, `64`, `508`, and `511`, and the nine reflected pages.

CPU state keeps the controlled selector value `0` but restores the FS segment base as `0x7FFDE000`. TLB and generated-code invalidation occur before Rust acknowledges generation `10`. The receipt re-reads the first 64 bytes of both environment pages and verifies the exact pointer fields before execution.

## 4. Execution result

The fixture executes with instruction budget `3`. It reaches process-entry sentinel ID `2`／`0x7F010010` after nine execute stops and eight budget stops:

| Field | Result |
|---|---:|
| Stop reason／error | exact sentinel `#UD` `0x106`／`-1` |
| Entry result | EAX `0` |
| Final ESP | `0x00700000` |
| FS selector／base | `0`／`0x7FFDE000` |
| `FS:[0x18]` | `0x7FFDE000` |
| `FS:[0x30]` | `0x7FFDF000` |
| `*(TEB + 0x18)` | `0x7FFDE000` |
| `*(TEB + 0x30)` | `0x7FFDF000` |

All four values are also written by the fixture to its reflected `.data` page and read back by the adapter. The prior first-stop command remains byte-identical at 5,982 bytes／SHA-256 `9bca668c33208fa1bc6a4e8253dafb247bd9520a106761a24fd06cf46364f7f9`.

The checkpoint command is:

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:bootstrap-fs-first-stop
npm run check:runtime:bootstrap-fs
```

## 5. Non-claims and next checkpoint

This is not a general Windows PEB／TEB implementation. The PEB page remains empty, the TEB contains only offsets `0x18` and `0x30`, FS selector `0` is controlled adapter state rather than a Windows selector contract, and there are no process parameters, loader lists, TLS slots, LastError, client IDs, exception chain, stack bounds, multi-thread ownership, teardown, or adapter-to-Rust mutation synchronization.

The next checkpoint should obtain a separate native oracle for the minimum process-parameters path before adding more internal fields. A candidate is the i386 PEB process-parameters pointer and one bounded command-line observation; its exact offsets and layout must be measured rather than inferred from this checkpoint.
