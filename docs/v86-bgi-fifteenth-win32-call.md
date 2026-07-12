# BGI fifteenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-fifteenth-win32-call` version `1`
- Report size／SHA-256: 120,499 bytes／`25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`

This checkpoint continues [v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md). It validates the first `TlsSetValue` consumer, associates Vine's `FlsGetValue` endpoint with the initial controlled thread, returns BOOL success, and stops at the next static import. It does not invoke the endpoint or implement a general TLS table.

## 2. Native evidence and controlled arguments

The two `tls-lifecycle` captures documented by the previous checkpoint agree that BGI caller RVA `0xA58C1` calls `TlsSetValue(nativeIndex,nativeFlsGetValueAddress)`, returns BOOL `1`, and preserves LastError `87 -> 87`. Both native arguments are discarded.

The controlled call instead uses:

| Argument | Controlled value | Identity |
|---|---:|---|
| index | `0` | `vine.logical-tls-index.v1` |
| value | `0x7F020010` | dynamic endpoint ID `2`, `kernel32.dll!FlsGetValue` |

The initial controlled thread is owner `checkpoint-thread-identity`, thread ID `8192`. The TLS slot remains owner `checkpoint-thread-local-storage`, allocation variant `tlsalloc-first-controlled-index`.

## 3. Checked register-indirect call

The static import is promoted from an unsupported trap to a declared endpoint while retaining ID `68`, address `0x7F000430`, and IAT `0x100D11B8`. BGI's call sequence is:

```text
0x100A58B8  PUSH DWORD PTR [0x100FEEC0]
0x100A58BE  PUSH EAX
0x100A58BF  CALL ESI
0x100A58C1  return address
```

The adapter and Rust require frame `[0x100A58C1,0,0x7F020010]`, live ESI `0x7F000430`, exact `FF D6`, and the bound IAT. All four dynamic endpoint stores remain unchanged.

## 4. Logical state transition

The Rust session carries the allocation metadata from `TlsAlloc` into this call. The slot transitions as follows:

| Field | Before | After |
|---|---|---|
| lifecycle | `allocated` | `allocated` |
| initialized thread values | `0` | `1` |
| backing materialization | `metadata-only-until-first-reached-tls-operation` | `initial-thread-value-metadata` |

The one thread-value record stores thread ID `8192`, logical index `0`, value kind `dynamic-export-endpoint`, endpoint ID `2`, and lifecycle `set`. It does not store the native function pointer or host TLS state.

## 5. Three-page reconciliation

BGI stores the allocated index at `0x100F0764` before calling `TlsSetValue`. The transaction reconciles three live v86 pages so later code can read the semantic Rust memory:

| Reconciled page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `8` | `0f62e02ebf62129f93e1bbad8e4b2f2739783536fd70af2df31a7cd5ef3accec` |
| endpoint stores `0x100FE000` | `1` | `0` | `705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8` |
| TLS index store `0x100F0000` | `1` | `12` | `b25cb15bb316bf0e716d70f9503a982c60ed5189eeb903b29d602141896d3c2c` |

Rust verifies `0x100F0764 = 0`. The transaction leaves generation／acknowledged generation `30`, dirty pages `0`, and the same 389-page reflection SHA-256 `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`.

## 6. Next stop

Returning BOOL `1` moves ESP from `0x006FFF68` to `0x006FFF74`. Nine instructions later execution reaches:

| Field | Observed |
|---|---|
| Cumulative instructions | `174` |
| Additional instructions | `9` |
| Execute／budget stops | `9`／`8` |
| Next thunk | ID `89`, `KERNEL32.dll!EncodePointer`, `0x7F000580` |
| Disposition | `unsupported-trap` |
| IAT | `0x100D120C` |
| Return address | `0x100A5467` |
| Raw stack words | `[0x100A5467,0,0x100A4FD6,0x7F000430,0x100A58CE,0,0,0x100A375B]` |

The one-argument frame is `EncodePointer(NULL)`. It is evidence for the next checkpoint only.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-fifteenth-win32-call
```

Two consecutive runs produced the same 120,499-byte report and SHA-256. The fourteenth report remains 110,236 bytes with SHA-256 `4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`.

## 8. Non-claims and next checkpoint

This is one initial-thread `TlsSetValue` record. It is not general index validation, multiple threads, overwrite semantics, `TlsGetValue`／`TlsFree`, endpoint invocation, FLS callbacks, or a LastError model.

The next checkpoint should obtain focused native evidence for BGI's `EncodePointer(NULL)` result, LastError behavior, and first consumer. It must not import the process-specific encoded host pointer.
