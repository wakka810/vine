# BGI eleventh supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-eleventh-win32-call` version `1`
- Report size／SHA-256: 76,844 bytes／`40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`

This checkpoint continues [v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md). It preserves the first `FlsAlloc` endpoint, validates BGI's second `GetProcAddress` call and first endpoint store, returns a distinct endpoint for `FlsGetValue`, observes BGI store that endpoint, and stops at the third exact lookup. It does not call either endpoint.

## 2. Native evidence

The same focused `--observe dynamic-export` trace remains the native oracle. Its raw JSONL is 320,917 bytes with SHA-256 `5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe`; it is not committed. The relevant ordered calls are:

| Symbol | Caller RVA | Native result | LastError before／after |
|---|---:|---:|---:|
| `FlsAlloc` | `0xA582F` | `0x758CD300` | `87`／`87` |
| `FlsGetValue` | `0xA583C` | `0x758D33F0` | `87`／`87` |
| `FlsSetValue` | `0xA5849` | `0x758CB2E0` | `87`／`87` |
| `FlsFree` | `0xA5856` | `0x758E0AA0` | `87`／`87` |

The native addresses are discarded. Only the second identity, `kernel32.dll!FlsGetValue`, is newly resolved here.

## 3. Ordered endpoint arena

The multi-endpoint installer validates all identities before mutation, reserves one runtime arena, writes ordered `UD2` slots into one R／X page, hashes the completed page, and seals the owner once. The eleventh mode contains:

| ID | Address | Identity | Lifetime |
|---:|---:|---|---|
| `1` | `0x7F020000` | `kernel32.dll!FlsAlloc` | `process-runtime` |
| `2` | `0x7F020010` | `kernel32.dll!FlsGetValue` | `process-runtime` |

Both records use namespace `vine.dynamic-export-endpoint.v1`, owner `9`, arena `[0x7F020000,0x7F030000)`, 16-byte stride, and page SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`. The first endpoint's ID, address, module, name, and lifetime remain unchanged. The shared page hash changes because slot `2` is now populated. The one-endpoint tenth mode remains byte-for-byte unchanged with its original page and report hashes.

## 4. Persistent-register call provenance

The second lookup reuses ESI loaded from the static `GetProcAddress` IAT during the first lookup:

```text
0x100A582F  PUSH 0x100D2500
0x100A5834  PUSH EDI
0x100A5835  MOV [0x100FEEBC], EAX
0x100A583A  CALL ESI
0x100A583C  return address
```

The adapter reports live ESI `0x7F000950`; Rust checks it against exact thunk ID `150`, validates the 13-byte sequence, bound IAT `0x100D1300`, and frame `[0x100A583C,0x4D010001,0x100D2500]`. A bounded ASCII read yields `FlsGetValue\0`; its 12 bytes have SHA-256 `51598ab3ab558b58b22deb49515346e61d997d79ab4215fd040ce7125ea0818f`.

## 5. First endpoint preservation

BGI stores the prior EAX before making the second call. The runner supplies both mutated pages to one Rust transaction at generation `30`:

| Reconciled page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `2` | `5a4cdc83d919e8a3dfc1f0617bde34c9fc0f7eb3cee2a6597aa9f22e8e7e67ef` |
| image data `0x100FE000` | `1` | `2` | `098e6d79168e51989225bbab5e1c59d889363412c53b29d7478a0ad4174101f6` |

After reconciliation, Rust reads `0x100FEEBC` and requires exact value `0x7F020000`. A mismatch rolls back both reconciliations. No host-originated guest write occurs, so generation `30`, acknowledged generation `30`, and zero dirty pages remain unchanged. The request／receipt are 17,273／2,704 bytes excluding newlines.

## 6. Reflection and next stop

The second slot does not add an owner, region, or physical page. The eleventh mode remains 9 owners, 433 reserved pages, 389 committed／reflected pages, and initial generation `27`. Its reflection SHA-256 is `de382ad25a1487a1c8b038f48eace933d0270cf745820419b776689c87a50eeb`; offer／receipt sizes are 4,922,145／1,736 bytes.

The stdcall return changes EAX from `0x7F020000` to endpoint `0x7F020010` and moves ESP from `0x006FFF68` to `0x006FFF74`. BGI stores endpoint `2` at `0x100FEEC0`, then reaches the third lookup after 5 instructions:

| Field | Observed |
|---|---|
| Cumulative instructions | `141` |
| Execute／budget stops | `5`／`4` after the eleventh trap |
| Next thunk | ID `150`, `KERNEL32.dll!GetProcAddress`, `0x7F000950` |
| Return address | `0x100A5849` |
| Arguments | `[0x4D010001,0x100D24F4]` -> logical KERNEL32／ASCII `FlsSetValue` |
| Raw stack words | `[0x100A5849,0x4D010001,0x100D24F4,0,0,0x100A375B,0xE93F6D87,0]` |

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-eleventh-win32-call
```

Two consecutive runs produced the same 76,844-byte report and SHA-256. The tenth report remains 65,858 bytes with SHA-256 `54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`. Raw reports stay under `.work/`.

## 8. Non-claims and next checkpoint

This is one additional controlled name lookup, one ordered endpoint allocation, and one two-page reconciliation. It is not general v86 dirty-page discovery, a general dynamic symbol table, endpoint invocation, FLS semantics, export parsing, forwarder resolution, DLL lifecycle, or a LastError model.

The next checkpoint should return `GetProcAddress(logicalKernel32,"FlsSetValue")`, preserve endpoints `1` and `2`, allocate endpoint ID `3` at `0x7F020020`, and stop at the fourth lookup for `FlsFree`.
