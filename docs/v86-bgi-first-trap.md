# BGI bounded reflection and first runtime trap

## 1. Status

- Status: Verified primary-title entry-to-trap checkpoint
- Date: 2026-07-11
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-first-trap` version `1`
- Report size／SHA-256: 9,591 bytes／`a74ed888aa95923a1bcb5c20ad5a58e945ab269419d834a19eefa10d6b33be3b`

This checkpoint continues [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md). It transfers the sealed BGI image, all import traps, and the controlled bootstrap state into source-built v86, acknowledges the real invalidation generation, starts the primary entry point, and stops at the first exact runtime-reachable owned thunk. It does not dispatch or return from that API.

## 2. Bounded transfer

The existing reflection session remains limited to 16 semantic pages and 512 KiB protocol messages. BGI uses an explicit `--large-image-reflection` mode with hard Rust limits of 400 pages and 8 MiB per serialized JSONL message; the dedicated runner enforces the same line limit before parsing. The actual offer is 4,904,601 bytes excluding its newline; the receipt is 1,468 bytes.

The probe also accepts an explicit controlled command line while preserving `bootstrap-params.exe` as the existing default. This run installs UTF-16 `BGI.exe` with `Length = 14` and `MaximumLength = 16`.

| Owner | Role | Pages | R／W／X pages | Content SHA-256 |
|---:|---|---:|---:|---|
| 1 | relocated／bound BGI image | 380 | 380／100／208 | `1802331aead82ee870702f16d0fba9d33c400a485122765a00e3a882503c4af1` |
| 2 | import thunk arena | 2 | 2／0／2 | `88f1b31a1ed2f724903f9ddaf16c789240b2010077537b9203faa44a63c20dde` |
| 3 | callback／entry return control page | 1 | 1／0／1 | `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc` |
| 4 | initial stack | 1 | 1／1／0 | `3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079` |
| 5 | TEB | 1 | 1／1／0 | `5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e` |
| 6 | PEB | 1 | 1／1／0 | `780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39` |
| 7 | process parameters | 1 | 1／1／0 | `ac17565d250da1d27dc2429b2c81bd242fe26d67373495b2238eadbb762b6d33` |
| 8 | command line | 1 | 1／1／0 | `c9b83dbe193708a3418517c43a4dadd26ca50a4006b00bd11701f3bb6ef0e6cc` |

All 388 pages are sealed and dirty loader state. Rust reports 8 owners／regions, 417 reserved pages, 388 committed／physical pages, and pending generation `16`. The reflection content SHA-256 is `f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`. Normal Rust reads after acknowledgement verify the first IAT, entry／thunk／sentinel bytes, stack top, TEB, PEB, process parameters, and command line.

## 3. Adapter placement

The adapter keeps the existing 2 MiB physical pool. A naive contiguous page allocator first placed an image page in v86's physical VGA MMIO range `[0x000A0000,0x000C0000)`, causing `mmap_read32(0x000A883F)` during instruction fetch. This was adapter placement, not a guest bootstrap access. The dedicated bounded allocator now excludes that fixed 32-page range and never supplies a value through the MMIO callback.

The successful placement uses 393 allocated pages: one directory, four page tables, and 388 reflected pages. Directory indices and table addresses are:

| PDE index | Page-table physical address |
|---:|---:|
| 1 | `0x00002000` |
| 64 | `0x00004000` |
| 508 | `0x001A1000` |
| 511 | `0x001A5000` |

The highest allocation ends at `0x001AA000`, below the 2 MiB limit. CPU-state restore, explicit `full_clear_tlb`, and `jit_clear_cache_js` complete data-TLB, instruction-fetch, and generated-code invalidation before Rust accepts generation `16`.

## 4. Runtime-reachable stop

The runner starts entry `0x100A383E` with FS base `0x7FFDE000`, initial ESP `0x006FFFFC`, and an instruction budget of 1. The outer bound is 4,096 execute calls／instructions. Checking the sealed logical execute permission before every instruction compensates for this v86 revision's missing native NX enforcement. BGI reaches an owned `UD2` after 19 execute calls／18 budget stops and 19 instructions.

| Field | Observed |
|---|---|
| Thunk | ID `104`, `KERNEL32.dll!GetSystemTimeAsFileTime` |
| Disposition／address | `unsupported-trap`／`0x7F000670` |
| IAT | `0x100D1248` |
| EIP／previous EIP | `0x7F000670`／`0x7F000670` |
| Stop | `#UD`, reason `0x106`, error `-1` |
| ESP／return address | `0x006FFFD0`／`0x100A88D2` |
| Raw first stack words | `[0x100A88D2, 0x006FFFEC, 0, 0, 0, 0, 0, 0]` |

The second word is recorded as raw stack evidence; this checkpoint does not yet declare or execute the API's stdcall marshalling. The runner resolves the EIP only through exact ordered thunk metadata. It neither assumes static ID `1` is first at runtime nor treats an arbitrary `UD2` as an import call.

The source size and SHA-256 are checked before and after the run and remain identical. Two locked runs reproduce the same reflection and report hashes.

## 5. Reproduction

The source adapter and primary title must already be present:

```powershell
npm run prepare:engine
npm run check:runtime:bgi-first-trap
```

Generated protocol data and the report remain under `.work/v86-bgi-first-trap/` and are not committed.

## 6. Non-claims and next checkpoint

This is BGI entry execution evidence, not title startup or a `GetSystemTimeAsFileTime` implementation. It does not provide a clock source, validate the output pointer, write a `FILETIME`, apply stdcall cleanup, return to `0x100A88D2`, or execute a twentieth instruction. It also does not implement DLL lifecycle, loader lists, TLS callbacks, LastError, heap, general process parameters, dirty-page writeback, snapshot, Worker integration, or any of the other 329 imports.

The next checkpoint should define one controlled clock sample, validate the one-argument writable 8-byte `FILETIME` frame for exact thunk ID `104`, perform its stdcall return without fabricating unrelated state, resume BGI, and stop at the next exact owned trap.
