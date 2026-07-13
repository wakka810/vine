# BGI sixteenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-sixteenth-win32-call` version `1`
- Report size／SHA-256: 129,530 bytes／`310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`

This checkpoint continues [v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md). It returns a deterministic process-local token for the observed `EncodePointer(NULL)` variant, preserves the logical TLS state, and stops at BGI's next `EncodePointer` call. It does not import a native process cookie or support non-NULL pointer encoding.

## 2. Native evidence

The opt-in `pointer-encoding` profile produced two focused captures:

| Capture | Bytes／lines | SHA-256 | Native result | LastError |
|---|---:|---|---:|---|
| primary | 2,213,858／6,979 | `71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3` | `0x6F713C58` | `0 -> 0` |
| confirmation | 312,314／1,040 | `4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c` | `0x765B8B70` | `87 -> 87` |

Both calls use source pointer `NULL` and return at caller RVA `0xA5467`. The differing nonzero results are process-cookie-dependent host values, so Vine records them only as native evidence and discards both. The wrapper returns to RVA `0xA4FD6`, where BGI moves EAX into ESI and uses it as an initialization sentinel.

## 3. Controlled encoding

The supported variant is owner `checkpoint-pointer-encoding`, variant `encodepointer-null-process-token`. It creates this typed value:

| Field | Controlled value |
|---|---|
| namespace | `vine.logical-encoded-pointer.v1` |
| value | `0xEC010001` |
| source kind／pointer | `null`／`0` |
| lifetime | `process-runtime` |
| process cookie materialization | `not-imported` |
| native result imported | `false` |

This token is deterministic checkpoint state, not Windows' encoding algorithm or a claim that arbitrary pointers can be encoded or decoded.

## 4. Checked direct IAT call

The static import is promoted from an unsupported trap to the fourteenth declared endpoint while retaining ID `89`, address `0x7F000580`, and IAT `0x100D120C`:

```text
0x100A545F  PUSH 0
0x100A5461  CALL DWORD PTR [0x100D120C]
0x100A5467  return address
```

The adapter and Rust require the exact eight-byte `6A 00 FF 15 0C 12 0D 10` sequence, bound IAT, and frame `[0x100A5467,0]` at ESP `0x006FFF60`. Rust also requires the retained logical TLS slot `(index 0, initialized values 1)` and initial-thread value `(thread 8192, endpoint ID 2／0x7F020010)` before returning the token.

## 5. Stack reconciliation and return

The transaction reconciles only the current stack page:

| Page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `13` | `5f103e07657780d9f8a17053498cf300c1edee9a17272a20455ffd453fdff42a` |

Generation and acknowledged generation remain `30`, dirty pages remain `0`, and the 389-page reflection SHA-256 remains `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`. The stdcall return places `0xEC010001` in EAX and moves ESP from `0x006FFF60` to `0x006FFF68`.

## 6. Next stop

BGI executes 55 more instructions and calls the same declared import again:

| Field | Observed |
|---|---|
| Cumulative instructions | `229` |
| Additional instructions | `55` |
| Execute／budget stops | `55`／`54` |
| Next thunk | ID `89`, `KERNEL32.dll!EncodePointer`, `0x7F000580` |
| IAT | `0x100D120C` |
| Return address | `0x100A9697` |
| Argument | `0x100A9608` |
| Raw stack words | `[0x100A9697,0x100A9608,0x100A4FFC,0xEC010001,0xEC010001,0xEC010001,0xEC010001,0xEC010001]` |

This non-NULL input is outside the controlled NULL variant and is evidence for the next checkpoint only.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-sixteenth-win32-call
```

Two consecutive pinned runs produced the same 129,530-byte report and SHA-256. A separate fifteenth-mode run retained its 120,499-byte report and SHA-256 `25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`.

## 8. Non-claims and next checkpoint

This is one deterministic encoded-NULL sentinel. It is not general `EncodePointer`／`DecodePointer`, process-cookie emulation, pointer provenance, cross-process behavior, or a LastError model. General TLS／FLS and dynamic endpoint invocation also remain unimplemented.

The next checkpoint should measure native `EncodePointer(0x100A9608)` behavior and its first consumer, determine the semantic role of that source address, and add only the observed non-NULL variant before resuming to the next runtime-reachable trap.
