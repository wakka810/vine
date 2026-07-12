# BGI twelfth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-twelfth-win32-call` version `1`
- Report size／SHA-256: 89,289 bytes／`28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`

This checkpoint continues [v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md). It preserves the first two FLS export endpoints and their guest stores, validates BGI's third `GetProcAddress` call, returns a distinct endpoint for `FlsSetValue`, observes BGI store that endpoint, and stops at the fourth exact lookup. It does not call any FLS endpoint.

## 2. Native evidence

The focused `--observe dynamic-export` trace remains the native oracle. Its raw JSONL is 320,917 bytes with SHA-256 `5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe`; it is not committed. The relevant ordered calls are:

| Symbol | Caller RVA | Native result | LastError before／after |
|---|---:|---:|---:|
| `FlsAlloc` | `0xA582F` | `0x758CD300` | `87`／`87` |
| `FlsGetValue` | `0xA583C` | `0x758D33F0` | `87`／`87` |
| `FlsSetValue` | `0xA5849` | `0x758CB2E0` | `87`／`87` |
| `FlsFree` | `0xA5856` | `0x758E0AA0` | `87`／`87` |

Native addresses are discarded. Only the third identity, `kernel32.dll!FlsSetValue`, is newly resolved here.

## 3. Ordered endpoint arena

The same atomic installer validates all three identities before mutation, writes their ordered `UD2` slots into one R／X page, hashes the completed page, and seals the owner once:

| ID | Address | Identity | Lifetime |
|---:|---:|---|---|
| `1` | `0x7F020000` | `kernel32.dll!FlsAlloc` | `process-runtime` |
| `2` | `0x7F020010` | `kernel32.dll!FlsGetValue` | `process-runtime` |
| `3` | `0x7F020020` | `kernel32.dll!FlsSetValue` | `process-runtime` |

All records use namespace `vine.dynamic-export-endpoint.v1`, owner `9`, arena `[0x7F020000,0x7F030000)`, 16-byte stride, and page SHA-256 `3b6345284f0929d025fc45cb9ae5f1d720c7575ec110cb41e7fbcaaa2fad2439`. IDs, addresses, identities, and lifetime of endpoints `1` and `2` remain unchanged. The tenth and eleventh modes retain their original one- and two-endpoint page/report hashes.

## 4. Third lookup provenance

The third lookup again reuses ESI loaded from the static `GetProcAddress` IAT:

```text
0x100A583C  PUSH 0x100D24F4
0x100A5841  PUSH EDI
0x100A5842  MOV [0x100FEEC0], EAX
0x100A5847  CALL ESI
0x100A5849  return address
```

The adapter reports live ESI `0x7F000950`; Rust checks it against exact thunk ID `150`, validates the 13-byte sequence, bound IAT `0x100D1300`, and frame `[0x100A5849,0x4D010001,0x100D24F4]`. A bounded ASCII read yields `FlsSetValue\0`; its 12 bytes have SHA-256 `24e391b36bf5f29d67cdb94094f5662081c98677e63407f52f1dc1e924034c7d`.

## 5. Prior endpoint preservation

The runner supplies the current stack and writable image page to one Rust transaction at generation `30`:

| Reconciled page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `3` | `c4f6754b2889228cfd3f7382adff164a4c9bb8c9722ac1f5f0a38e03471a9286` |
| image data `0x100FE000` | `1` | `3` | `1cb318f989df8cbf93f59a84f1f5ab9b62d57f3237f7e7162c8df79456ca9d68` |

After reconciliation, Rust requires `0x100FEEBC = 0x7F020000` and `0x100FEEC0 = 0x7F020010`. A mismatch rolls back both pages. No host-originated guest write occurs, so generation `30`, acknowledged generation `30`, and zero dirty pages remain unchanged. The request／receipt are 17,336／3,233 bytes excluding newlines.

## 6. Reflection and next stop

The third slot adds no owner, region, or physical page. The mode remains 9 owners, 433 reserved pages, 389 committed／reflected pages, and initial generation `27`. Reflection SHA-256 is `8025131a9a8687ec01f68969f6cda2c102eaff9258a45bfcd29c5c5c3ae4b344`; offer／receipt sizes are 4,922,584／1,798 bytes.

The stdcall return changes EAX from `0x7F020010` to endpoint `0x7F020020` and moves ESP from `0x006FFF68` to `0x006FFF74`. BGI stores endpoint `3` at `0x100FEEC4`, then reaches the fourth lookup after 5 instructions:

| Field | Observed |
|---|---|
| Cumulative instructions | `146` |
| Execute／budget stops | `5`／`4` after the twelfth trap |
| Next thunk | ID `150`, `KERNEL32.dll!GetProcAddress`, `0x7F000950` |
| Return address | `0x100A5856` |
| Arguments | `[0x4D010001,0x100D24EC]` -> logical KERNEL32／ASCII `FlsFree` |
| Raw stack words | `[0x100A5856,0x4D010001,0x100D24EC,0,0,0x100A375B,0xE93F6D87,0]` |

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-twelfth-win32-call
```

Two consecutive runs produced the same 89,289-byte report and SHA-256. The eleventh report remains 76,844 bytes with SHA-256 `40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`. Raw reports stay under `.work/`.

## 8. Non-claims and next checkpoint

This is one additional controlled name lookup, one ordered endpoint allocation, and one bounded two-page reconciliation. It is not general v86 dirty-page discovery, a general dynamic symbol table, endpoint invocation, FLS semantics, export parsing, forwarder resolution, DLL lifecycle, or a LastError model.

The next checkpoint should return `GetProcAddress(logicalKernel32,"FlsFree")`, preserve endpoints `1` through `3`, allocate endpoint ID `4` at `0x7F020030`, and stop at the next runtime-reachable trap without implementing FLS invocation semantics in advance.
