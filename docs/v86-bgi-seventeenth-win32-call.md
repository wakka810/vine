# BGI seventeenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-seventeenth-win32-call` version `1`
- Report size／SHA-256: 140,660 bytes／`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`

This checkpoint continues [v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md). It identifies the first non-NULL source as an executable BGI initialization routine, returns a distinct deterministic token, observes BGI storing that token, and stops at the next pointer operation. It does not implement arbitrary pointer encoding or decoding.

## 2. Native evidence

The existing two `pointer-encoding` captures contain the exact second call:

| Capture | Source／result | LastError |
|---|---|---|
| primary | `0x003A9608 -> 0x6F766E99` | `0 -> 0` |
| confirmation | `0x003A9608 -> 0x71094A70` | `87 -> 87` |

Both calls return at BGI RVA `0xA9697`. The native source is module RVA `0xA9608`; the two process-cookie-dependent results are retained only as evidence and discarded. Capture sizes and hashes remain 2,213,858 bytes／`71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3` and 312,314 bytes／`4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c`.

## 3. Source and consumer role

Static disassembly identifies RVA `0xA9608` as executable guest code. The encoding wrapper and immediate consumer are:

```text
0x004A968C  PUSH 0x004A9608
0x004A9691  CALL DWORD PTR [0x004D120C]
0x004A9697  MOV DWORD PTR [0x004FF62C], EAX
0x004A969C  RET
```

A downstream path loads global RVA `0xFF62C`, calls `DecodePointer` at return RVA `0xA966C`, and calls the decoded EAX at RVA `0xA9674`. The semantic source role is therefore `process-initializer-guest-function`, not an opaque data pointer.

## 4. Controlled encoding

The supported variant is owner `checkpoint-pointer-encoding`, variant `encodepointer-process-initializer-function-token`:

| Field | Controlled value |
|---|---|
| namespace | `vine.logical-encoded-pointer.v1` |
| value | `0xEC010002` |
| source kind | `guest-executable-function` |
| source pointer | `0x100A9608` |
| lifetime | `process-runtime` |
| process cookie materialization | `not-imported` |

Rust requires the previous encoded-NULL object `0xEC010001`, logical TLS slot, initial-thread endpoint value, executable source page owner `1`, and this 17-byte relocated prefix:

```text
6A 08 68 F8 B2 0E 10 E8 5C BC FF FF E8 33 C0 FF FF
```

Its SHA-256 is `ca74fa61a06f6555a4ec69ea76cdb33d4e1a461a970975d416644ecbeaf43b68`.

## 5. Checked call, reconciliation, and store

The adapter validates the exact `PUSH imm32; CALL [0x100D120C]` sequence and frame `[0x100A9697,0x100A9608]` at ESP `0x006FFF48`. Rust reconciles the stack page:

| Page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `32` | `3091514821863563f1c42861b1835da0639a0aecf15286732f71dec102615893` |

Generation and acknowledged generation remain `30`, dirty pages remain `0`, and the reflection SHA-256 remains `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`. The stdcall return moves ESP to `0x006FFF50` and places `0xEC010002` in EAX.

BGI then changes global `0x100FF62C` from `0` to `0xEC010002`. The resulting v86 page `0x100FF000` has SHA-256 `d8e3945ee6c5f229abe4f6e66c9ff547432323a81c23457829ec236d83674b10`; Rust reconciliation of this guest-originated store is deferred to its first consumer checkpoint.

## 6. Next stop

Nine instructions after the return, BGI reaches the same declared import:

| Field | Observed |
|---|---|
| Cumulative instructions | `238` |
| Additional instructions | `9` |
| Execute／budget stops | `9`／`8` |
| Next thunk | ID `89`, `KERNEL32.dll!EncodePointer`, `0x7F000580` |
| Return address | `0x100A58DC` |
| Argument | `0x7F020000`, dynamic endpoint ID `1`／`FlsAlloc` |
| Raw stack words | `[0x100A58DC,0x7F020000,0,0,0x100A375B,0xE93F6D87,0,0]` |

The native traces observe this same call with native `FlsAlloc` address `0x758CD300`, results `0x61C0A638`／`0xC7C1EB7E`, and unchanged LastError. Those host values are evidence for the next checkpoint only.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-seventeenth-win32-call
```

Two consecutive pinned runs produced the same 140,660-byte report and SHA-256. A separate sixteenth-mode run retained its 129,530-byte report and SHA-256 `310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`.

## 8. Non-claims and next checkpoint

This is one encoded guest function and one observed store. It is not `DecodePointer`, arbitrary guest／runtime pointer provenance, process-cookie emulation, endpoint invocation, or a LastError model. General TLS／FLS also remains unimplemented.

The observed next BGI action remains encoding dynamic endpoint ID `1`／`FlsAlloc` at RVA `0xA58DC`, but bespoke BGI continuation is frozen. Runtime object／handle and module-graph foundations are complete; process／thread lifecycle proceeds next. No eighteenth-call flag or receipt is added from this document.
