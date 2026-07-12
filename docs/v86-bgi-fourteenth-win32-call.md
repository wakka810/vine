# BGI fourteenth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-fourteenth-win32-call` version `1`
- Report size／SHA-256: 110,236 bytes／`4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`

This checkpoint continues [v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md). It validates BGI's zero-argument `TlsAlloc` call, allocates the first Vine-owned logical TLS index, returns it without importing the native index, and stops at the first consumer. It does not dispatch `TlsSetValue` or any dynamic FLS endpoint.

## 2. Native TLS evidence

The opt-in `--observe tls-lifecycle` profile hooks `TlsAlloc`, `TlsFree`, `TlsGetValue`, and `TlsSetValue`, including LastError immediately before and after each call. Raw JSONL remains under `.work/` and is not committed.

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| primary | 6,138,442 | 19,816 | `214ca138648d6674860ba3fc43e18c017470f0b31e023f1e7e9e4afa88dae08b` |
| short confirmation | 62,273 | 223 | `fd23eb23fca34953216b0a1cf11c5ce4d88959618c5bc1173b58eab5199ad30e` |

Both captures agree on the startup sequence:

| API | BGI caller RVA | Arguments | Result | LastError |
|---|---:|---|---:|---|
| `TlsAlloc` | `0xA58AA` | `[]` | native index `0x1C` | `87 -> 87` |
| `TlsSetValue` | `0xA58C1` | `[0x1C, native FlsGetValue address]` | `1` | `87 -> 87` |

The native index and native `FlsGetValue` address are host-process state and are discarded. `TLS_OUT_OF_INDEXES = 0xFFFFFFFF` is recorded as the Win32 failure sentinel; this checkpoint observes and implements only the successful path.

## 3. Controlled logical allocation

Rust accepts only profile `tlsalloc-first-controlled-index` owned by `checkpoint-thread-local-storage`. It creates metadata with namespace `vine.logical-tls-index.v1`, index `0`, lifecycle `allocated`, storage scope `thread`, and zero initialized thread values. Backing remains metadata-only until a reached TLS operation requires a value.

The successful return preserves the existing thread LastError by policy. The runtime does not yet materialize LastError state, so the report records `preserve-existing-thread-last-error-unmodeled` rather than fabricating a value.

## 4. Checked call and preservation

The static import is promoted from an unsupported trap to a declared endpoint while retaining ID `70`, address `0x7F000450`, and IAT `0x100D11C0`. Rust and the adapter require:

- zero-argument stdcall frame `[0x100A58AA]`;
- exact six-byte indirect IAT call ending at the return address;
- bound IAT value `0x7F000450`;
- all four dynamic endpoint stores:

| Store | Value |
|---:|---:|
| `0x100FEEBC` | `0x7F020000` (`FlsAlloc`) |
| `0x100FEEC0` | `0x7F020010` (`FlsGetValue`) |
| `0x100FEEC4` | `0x7F020020` (`FlsSetValue`) |
| `0x100FEEC8` | `0x7F020030` (`FlsFree`) |

The live stack and image data pages reconcile transactionally at generation `30`:

| Reconciled page | Owner | Changed bytes | Content SHA-256 |
|---|---:|---:|---|
| stack `0x006FF000` | `4` | `3` | `3595f775ff05d8b86348f1ddc3f931cc11b2bd9e769af5f012919daa888f91da` |
| image data `0x100FE000` | `1` | `3` | `705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8` |

Generation, acknowledgement, owner count, and dirty-page count remain `30`, `30`, `9`, and `0`. Reflection remains 389 pages with SHA-256 `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`.

## 5. First consumer

Returning index `0` changes EAX from `0x7F020030` to `0` and moves ESP from `0x006FFF70` to `0x006FFF74`. Seven instructions later execution reaches:

| Field | Observed |
|---|---|
| Cumulative instructions | `165` |
| Additional instructions | `7` |
| Execute／budget stops | `7`／`6` |
| Next thunk | ID `68`, `KERNEL32.dll!TlsSetValue`, `0x7F000430` |
| Disposition | `unsupported-trap` |
| IAT | `0x100D11B8` |
| Return address | `0x100A58C1` |
| Raw stack words | `[0x100A58C1,0,0x7F020010,0,0,0x100A375B,0xE93F6D87,0]` |

The first two arguments exactly pair Vine's logical index `0` with Vine's `FlsGetValue` endpoint. This is evidence for the next checkpoint only; `TlsSetValue` is not dispatched here.

## 6. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-fourteenth-win32-call
```

Two consecutive runs produced the same 110,236-byte report and SHA-256. The thirteenth report remains 103,179 bytes with SHA-256 `1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`.

## 7. Non-claims and next checkpoint

This is one successful TLS index allocation. It is not a general TLS allocator, `TlsSetValue`／`TlsGetValue`／`TlsFree`, FLS endpoint dispatch, callback cleanup, multiple guest threads, or a complete LastError model.

The next checkpoint should implement only the observed `TlsSetValue(0, 0x7F020010)` variant. It must verify that index `0` is allocated, associate the endpoint value with the initial controlled thread, preserve LastError on success, return BOOL `1`, and resume to the next exact runtime-owned trap.
