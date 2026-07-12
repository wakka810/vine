# BGI InterlockedIncrement

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-interlocked-increment`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-fiber-local-storage-value`
- First unsupported capability: `kernel32-private-heap-second-allocation`

## Purpose

The cumulative BGI session reached static endpoint ID `124`,
`kernel32.dll!InterlockedIncrement(0x100F08E0)`. This checkpoint connects the
one-argument stdcall to checked Rust-owned guest memory, returns the incremented
LONG, and continues every reached occurrence of the same contract until the next
distinct unsupported capability.

The implemented boundary is:

```text
checked non-null, 4-byte-aligned InOut LONG
  -> read and write preflight before handler execution
  -> signed i32 wrapping increment
  -> one GuestMemory write and host delta
  -> incremented LONG bits in EAX with LastError preserved
  -> existing critical-section and process contracts
  -> reached HeapAlloc(..., 0x800) variant stop
```

Adjacent interlocked operations, cross-worker shared memory, and general
multi-thread scheduling are not part of this checkpoint. The operation is
indivisible within the current single Runtime Worker cooperative execution lane;
the runtime does not claim host-thread atomicity outside that ownership model.

## Native evidence

Two opt-in `--observe interlocked-increment` captures remain under `.work/`:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 272,588 | 875 | `917f73f6ad71737c406b2be2324c94abd7e6c59135f771ffe23f7664178706e5` |
| Confirmation | 335,188 | 1,054 | `719ad5fd2f36ad10897c414bb528a8140ae7256e3981a29010fe0971b6b36c3a` |

Both captures agree on the first three BGI-owned callers:

| Caller return RVA | Target offset | Before | After / result | LastError |
|---|---:|---:|---:|---:|
| `0xA5577` | `0xF08E0` | `0` | `1` | `87 -> 87` |
| `0xA7351` | `0xF0F70` | `1` | `2` | `87 -> 87` |
| `0xA73C7` | `0xF0EBC` | `0` | `1` | `87 -> 87` |

The first caller later increments `-1` to `0` and returns `0`, with LastError
`0 -> 0`. That pair fixes signed wrapping behavior without importing native
addresses. The mapped targets are the corresponding BGI image addresses
`0x100F08E0`, `0x100F0F70`, and `0x100F0EBC`.

Timeout termination left an unrelated `Direct3DCreate9` call open in each raw
trace, so full-trace summarization is intentionally not used as the oracle. The
exact `InterlockedIncrement` enter/leave pairs are balanced by call ID.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-interlocked-increment.jsonl `
  --seconds 2 --kill-after `
  --observe interlocked-increment
```

## Runtime contract

`InterlockedIncrement` uses the common `ApiDispatcher` with:

- argument: required 4-byte-aligned `InOut LONG` pointer;
- return: incremented 32-bit value;
- calling convention: x86 stdcall with 4 argument bytes;
- LastError policy: preserve;
- arithmetic: `i32::wrapping_add(1)`.

Frame decoding preflights read and write access for the full LONG. The handler
reads exactly four bytes, computes the wrapping successor, and returns one
required `ApiImmediate` guest write. Immediate completion applies that single
write before advancing EIP／ESP. Null, unaligned, unmapped, and read-only targets
fail before guest bytes, runtime state, registers, or LastError change.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  interlocked_increment_contract_wraps_and_rejects_invalid_targets_before_mutation `
  -- --exact
```

The test fixes `0 -> 1`, wrapping `-1 -> 0`, stdcall cleanup, LastError
preservation, exact guest writes, and mutation-free rejection of read-only,
unmapped, and page-crossing unaligned targets.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi-interlocked-increment
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 21 |
| Completed generic calls | 50 |
| `InterlockedIncrement` calls | 3 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 51 / 86 |
| CPU-changed bytes | 667 |
| Host delta batches | 26 |
| Final synchronized generation | 106 |
| Report bytes | 234,775 |
| Report SHA-256 | `4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643` |

The three completion frames retain fail-closed before/after evidence:

| Instruction | Target | Before -> after | Write generation | Return address |
|---:|---:|---:|---:|---:|
| 707 | `0x100F08E0` | `0 -> 1` | 92 | `0x100A5577` |
| 759 | `0x100F0F70` | `1 -> 2` | 98 | `0x100A7351` |
| 823 | `0x100F0EBC` | `0 -> 1` | 100 | `0x100A73C7` |

Each write is a four-byte, one-page, data-only delta with no TLB or instruction
cache effect. Existing `LeaveCriticalSection`, second `EnterCriticalSection`,
second `GetCurrentThreadId`, and second `GetStartupInfoW` occurrences then return
through their existing contracts.

The compatibility anchors remain byte-identical:

| Profile | Bytes | SHA-256 |
|---|---:|---|
| Frozen 17-call | 140,660 | `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4` |
| Generic re-entry | 96,244 | `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` |
| Critical-section capability | 167,719 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |
| Pointer-decoding capability | 171,356 | `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e` |
| FLS-allocation capability | 175,123 | `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c` |
| Private-heap capability | 183,102 | `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4` |
| FLS-value capability | 195,821 | `f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f` |

## Next structured stop

```text
kernel32.dll!HeapAlloc(0x68010001, HEAP_ZERO_MEMORY, 0x800)
static endpoint ID: 75
endpoint address:   0x7F0004A0
IAT address:        0x100D11D4
return address:     0x100AC3CC
instruction:        934
```

The existing contract returns `heap-alloc-variant-unsupported` before mutation.
The next checkpoint must connect this already measured second allocation size to
the existing bounded private heap while retaining the first allocation and
preserving LastError. It must not add free／realloc／destroy, zero-size, other
flags or sizes, or resource-failure success behavior.
