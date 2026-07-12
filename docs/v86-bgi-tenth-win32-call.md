# BGI tenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-tenth-win32-call` version `1`
- Report size／SHA-256: 65,858 bytes／`54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`

This checkpoint continues [v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md). It declares exact thunk ID `150`, validates the two-argument stdcall frame, register-indirect IAT provenance, logical KERNEL32 handle, and bounded guest ASCII `FlsAlloc` name in Rust, returns one identity-preserving guest callable endpoint, and stops at the next exact owned thunk. It does not execute `FlsAlloc` or import a host function pointer.

## 2. Native observation

The focused Windows trace was captured with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\bgi-dynamic-export-20260712.jsonl" `
  --seconds 3 `
  --kill-after `
  --observe dynamic-export
```

The uncommitted raw trace is 320,917 bytes with SHA-256 `5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe`. It contains 1,046 lines, 486 balanced call pairs, max depth `3`, and no diagnostics. The four BGI-owned lookups are:

| Symbol | Caller RVA | Native result | LastError before／after |
|---|---:|---:|---:|
| `FlsAlloc` | `0xA582F` | `0x758CD300` | `87`／`87` |
| `FlsGetValue` | `0xA583C` | `0x758D33F0` | `87`／`87` |
| `FlsSetValue` | `0xA5849` | `0x758CB2E0` | `87`／`87` |
| `FlsFree` | `0xA5856` | `0x758E0AA0` | `87`／`87` |

All four native addresses are host-process and ASLR specific, so none enters Vine state. This checkpoint resolves only the first observed identity, `kernel32.dll!FlsAlloc`.

## 3. Callable endpoint

Rust installs one endpoint before initial reflection:

| Field | Controlled value |
|---|---:|
| Namespace | `vine.dynamic-export-endpoint.v1` |
| Arena | `[0x7F020000,0x7F030000)` |
| Endpoint ID／address／stride | `1`／`0x7F020000`／`16` bytes |
| Memory owner | `9`, separate runtime owner |
| Committed page | `0x7F020000`, R／X, sealed |
| Endpoint bytes | `UD2`, followed by `HLT` padding |
| Identity | `kernel32.dll!FlsAlloc` |
| Lifetime | `process-runtime` |
| Page SHA-256 | `4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059` |

The endpoint arena is separate from static import thunks at `[0x7F000000,0x7F010000)` and return sentinels at `[0x7F010000,0x7F020000)`. The endpoint metadata retains module and export identity for a later callable dispatch; a successful lookup is not represented by a host address.

## 4. Checked register-indirect stdcall

This mode adds `GetProcAddress` as the eleventh declared import endpoint, leaving 319 identity-preserving unsupported traps. BGI loads that endpoint from the IAT into ESI, then calls through ESI:

```text
0x100A5821  MOV ESI, [0x100D1300]
0x100A5827  PUSH 0x100D250C
0x100A582C  PUSH EDI
0x100A582D  CALL ESI
0x100A582F  return address
```

Rust checks the full 14-byte sequence, the still-bound IAT value `0x7F000950`, and frame `[0x100A582F,0x4D010001,0x100D250C]`. The second argument is classified as a name because it exceeds `0xFFFF`; ordinal lookup is explicitly unsupported. A 64-byte bounded read yields ASCII `FlsAlloc\0`, whose 9 bytes have SHA-256 `574f9b2cb1c509be66aa90427c3250a3249a19c7a243b86f4ab13b4040abbd1e`.

## 5. Reflection and reconciliation

The separate endpoint owner changes only the tenth-call mode's initial reflection:

| Field | Value |
|---|---:|
| Owners／reserved／committed pages | `9`／`433`／`389` |
| Initial generation | `27` |
| Reflection SHA-256 | `e7a3cc956e167db7e078feefb30eb1d9f2204c8e17ca008f72c4359cce752456` |
| Offer／receipt bytes | `4,921,244`／`1,579` |
| Subsequent write generations | system time `28`, counter `29`, startup info `30` |

Earlier modes remain at 388 pages and keep their prior report hashes. At the first `GetProcAddress` trap, Rust reconciles generation `30` stack page `0x006FF000`; 15 bytes differ and the page SHA-256 is `0ec9e7bed9bb310bdab5ae447581656314c744cf4d8fe2104a36d9fe297215c7`. Lookup writes no guest bytes, so generation `30`, zero dirty pages, and acknowledged state are preserved. The request／receipt are 8,821／1,872 bytes excluding newlines.

## 6. Return and next stop

The stdcall return changes EAX from logical module handle `0x4D010001` to endpoint `0x7F020000` and cleans 8 argument bytes, moving ESP from `0x006FFF68` to `0x006FFF74`. BGI stores the result and reaches the next lookup after 5 more instructions:

| Field | Observed |
|---|---|
| Cumulative instructions | `136` |
| Execute／budget stops | `5`／`4` after the tenth trap |
| Next thunk | ID `150`, `KERNEL32.dll!GetProcAddress`, `0x7F000950` |
| Return address | `0x100A583C` |
| Arguments | `[0x4D010001,0x100D2500]` -> logical KERNEL32／ASCII `FlsGetValue` |
| Raw stack words | `[0x100A583C,0x4D010001,0x100D2500,0,0,0x100A375B,0xE93F6D87,0]` |

The immutable source fingerprint and every earlier checkpoint report hash remain unchanged.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-tenth-win32-call
```

Two consecutive runs produced the same 65,858-byte report and SHA-256. `npm run check:private-corpus` runs the first-trap contract and all ten cumulative call-return contracts. Raw traces and generated reports remain under `.work/` and are not committed.

## 8. Non-claims and next checkpoint

This is one controlled name lookup and one callable guest endpoint identity. It is not a general PE export parser, ordinal or forwarder resolver, DLL graph, host function bridge, FLS implementation, dynamic endpoint growth policy, loader-list implementation, or general LastError model.

The next checkpoint should return the second observed `GetProcAddress(logicalKernel32,"FlsGetValue")`, allocate endpoint ID `2` at `0x7F020010` while preserving the first endpoint identity, and stop at the next exact owned trap. It must not execute either FLS endpoint or predeclare later lookups without preserving their distinct identities.
