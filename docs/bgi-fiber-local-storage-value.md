# BGI fiber-local-storage value

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-fiber-local-storage-value`
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Previous capability checkpoint: `bgi-private-heap-allocation`
- First unsupported capability: `kernel32-interlocked-increment`

## Purpose

The cumulative BGI session reached dynamic endpoint ID `3`,
`kernel32.dll!FlsSetValue(0, 0x20000000)`. This checkpoint connects that
two-argument stdcall to the selected thread's FLS value map, returns the native
success shape, and continues through the existing runtime until the next owned
endpoint without a contract.

The implemented boundary is:

```text
process-owned logical FLS slot 0
  -> checked FlsSetValue stdcall frame
  -> selected thread 8192 value store
  -> BOOL 1 with LastError preserved
  -> existing GetModuleHandleW and EnterCriticalSection contracts
  -> next owned InterlockedIncrement trap
```

`FlsGetValue`, `FlsFree`, slot reuse, thread-exit callback invocation, process
teardown callbacks, and concurrent guest threads are not part of this
checkpoint.

## Native evidence

The two existing opt-in `--observe fls-lifecycle` traces remain under `.work/`:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 441,487 | 1,459 | `2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281` |
| Confirmation | 453,151 | 1,498 | `d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186` |

Both contain the exact BGI-owned call at caller return RVA `0xA5955`:

| Capture | Native index | Native value | Result | LastError |
|---|---:|---:|---:|---:|
| Primary | `9` | `0x043605C0` | `1` | `87 -> 87` |
| Confirmation | `9` | `0x03E505C0` | `1` | `0 -> 0` |

The native index and addresses are process-specific and are discarded. The two
samples establish BOOL success and LastError preserve behavior for the reached
call without treating LastError as the success signal.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-fls-lifecycle.jsonl `
  --seconds 2 --kill-after `
  --observe fls-lifecycle
```

## Runtime contract

`FlsSetValue` uses the common `ApiDispatcher` with:

- arguments: logical FLS index and nullable opaque `U32` value;
- return: `BOOL`;
- calling convention: x86 stdcall with 8 argument bytes;
- LastError policy: preserve;
- ownership: process slot, selected-thread value.

`RuntimeThreadState::fls_values` is separate from TLS state. The handler checks
that the process and selected thread are running, the scheduler lane is active,
and the process-owned index is allocated before inserting the value. It does
not dereference the opaque value. An unallocated index or inactive lane stops
with a structured fault before state, guest memory, or LastError changes;
native failure return and LastError behavior remain unmeasured.

Representative test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  fls_set_value_contract_owns_current_thread_values -- --exact
```

The test fixes BOOL return, stdcall cleanup, LastError preservation, independent
values for two selected threads, retained initial-thread state, mutation-free
unknown-index rejection, and unchanged guest memory.

## Generic BGI continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-value
```

| Evidence | Result |
|---|---:|
| Implemented contracts | 20 |
| Completed generic calls | 42 |
| `FlsSetValue` calls | 1 |
| Initial reflection | 389 pages, generation 27 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches / dirty pages | 43 / 75 |
| CPU-changed bytes | 497 |
| Host delta batches | 19 |
| Final synchronized generation | 91 |
| Report bytes | 195,821 |
| Report SHA-256 | `f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f` |

The completion frame records the authoritative state without a guest write:

```text
thread ID: 8192
FLS index: 0
value:     0x20000000
result:    BOOL 1
LastError: 87 (preserved)
```

The post-return path reaches the second `GetModuleHandleW` at instruction `676`
and the first `EnterCriticalSection` at instruction `699`. Those existing
contracts account for the later writeback and host delta; `FlsSetValue` itself
changes only Rust thread state.

The compatibility anchors remain byte-identical:

| Profile | Bytes | SHA-256 |
|---|---:|---|
| Frozen 17-call | 140,660 | `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4` |
| Generic re-entry | 96,244 | `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa` |
| Critical-section capability | 167,719 | `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69` |
| Pointer-decoding capability | 171,356 | `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e` |
| FLS-allocation capability | 175,123 | `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c` |
| Private-heap capability | 183,102 | `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4` |

## Next structured stop

```text
kernel32.dll!InterlockedIncrement(0x100F08E0)
static endpoint ID: 124
endpoint address:   0x7F0007B0
IAT address:        0x100D1298
return address:     0x100A5577
instruction:        707
```

The runtime returns `endpoint-contract-missing` in capability cluster
`kernel32-api-contract`. The next checkpoint must measure the reached atomic
increment and connect it to checked writable guest memory through the common
dispatcher. It must not add adjacent interlocked APIs or FLS teardown behavior.
