# BGI thirteenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-thirteenth-win32-call` version `1`
- Report size／SHA-256: 103,179 bytes／`1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`

This checkpoint continues [v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md). It preserves the first three FLS export endpoints and guest stores, validates BGI's fourth `GetProcAddress` call, returns a distinct endpoint for `FlsFree`, observes BGI store it, and stops at the next runtime-reachable static import. It does not call any FLS endpoint.

## 2. Native lookup evidence

The focused `--observe dynamic-export` trace remains the lookup oracle. Its raw JSONL is 320,917 bytes with SHA-256 `5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe`; it is not committed. The fourth call is `GetProcAddress(nativeKernel32,"FlsFree")` from caller RVA `0xA5856`, returns native host address `0x758E0AA0`, and preserves LastError `87 -> 87`. The host address is discarded.

## 3. Ordered endpoint arena

The atomic installer writes all four ordered `UD2` slots into one R／X page and seals the owner once:

| ID | Address | Identity | Lifetime |
|---:|---:|---|---|
| `1` | `0x7F020000` | `kernel32.dll!FlsAlloc` | `process-runtime` |
| `2` | `0x7F020010` | `kernel32.dll!FlsGetValue` | `process-runtime` |
| `3` | `0x7F020020` | `kernel32.dll!FlsSetValue` | `process-runtime` |
| `4` | `0x7F020030` | `kernel32.dll!FlsFree` | `process-runtime` |

All records use namespace `vine.dynamic-export-endpoint.v1`, owner `9`, arena `[0x7F020000,0x7F030000)`, 16-byte stride, and page SHA-256 `82ff4f595efb66795e9086158d940bdfb0f8e30a73d6d16879d599eb5afab615`. Endpoints `1` through `3` retain their IDs, addresses, identities, and lifetime. The one-, two-, and three-endpoint modes retain their original hashes.

## 4. Fourth lookup provenance

The fourth lookup uses the same persistent ESI call pattern:

```text
0x100A5849  PUSH 0x100D24EC
0x100A584E  PUSH EDI
0x100A584F  MOV [0x100FEEC4], EAX
0x100A5854  CALL ESI
0x100A5856  return address
```

The adapter and Rust require live ESI `0x7F000950`, exact static thunk ID `150`, bound IAT `0x100D1300`, the 13-byte sequence, and frame `[0x100A5856,0x4D010001,0x100D24EC]`. A bounded read yields ASCII `FlsFree\0`; its 8 bytes have SHA-256 `b25da59ca0b82d9b4b8ddb0a22566cbce73af231e364ed982046c841b26c8dd8`.

## 5. Prior endpoint preservation

The current stack and image page are reconciled in one generation `30` Rust transaction:

| Reconciled page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `2` | `ecbad1e057c0e424b1bdc8c863bd9a4ae85d77f6bee312ce7ea02704160dedf8` |
| image data `0x100FE000` | `1` | `3` | `c166a29b61c167001046de381b7dd5ba238ef5e5694021a152d626c9024ca7d0` |

Rust requires stores `0x100FEEBC = 0x7F020000`, `0x100FEEC0 = 0x7F020010`, and `0x100FEEC4 = 0x7F020020` before returning endpoint `4`. A mismatch rolls back both reconciliations. Generation `30`, acknowledged generation `30`, and zero dirty pages remain unchanged. Request／receipt sizes are 17,404／3,736 bytes excluding newlines.

## 6. Reflection and next stop

The fourth slot adds no owner, region, or physical page. The mode remains 9 owners, 433 reserved pages, 389 committed／reflected pages, and initial generation `27`. Reflection SHA-256 is `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`; offer／receipt sizes are 4,923,019／1,860 bytes.

The stdcall return changes EAX from `0x7F020020` to `0x7F020030` and moves ESP from `0x006FFF68` to `0x006FFF74`. BGI stores endpoint `4` at `0x100FEEC8`. Twelve instructions later, execution reaches a static import rather than an FLS endpoint:

| Field | Observed |
|---|---|
| Cumulative instructions | `158` |
| Additional instructions | `12` |
| Execute／budget stops | `12`／`11` |
| Next thunk | ID `70`, `KERNEL32.dll!TlsAlloc`, `0x7F000450` |
| Disposition | `unsupported-trap` |
| IAT | `0x100D11C0` |
| Return address | `0x100A58AA` |
| Raw stack words | `[0x100A58AA,0,0,0x100A375B,0xE93F6D87,0,0,0]` |

The zero-argument frame is evidence for the next checkpoint only. `TlsAlloc` is not dispatched here.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-thirteenth-win32-call
```

Two consecutive runs produced the same 103,179-byte report and SHA-256. The twelfth report remains 89,289 bytes with SHA-256 `28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`. Raw reports stay under `.work/`.

## 8. Non-claims and next checkpoint

This completes four controlled name lookups and endpoint allocations. It is not general export parsing, forwarder resolution, DLL lifecycle, FLS invocation semantics, TLS allocation, per-thread storage, callback cleanup, or a LastError model.

The next checkpoint should obtain focused native evidence for BGI's `TlsAlloc` result, failure sentinel, LastError behavior, and first consumer. It should then implement only the observed zero-argument allocation variant with a typed logical TLS index and resume to the next runtime-reachable trap.
