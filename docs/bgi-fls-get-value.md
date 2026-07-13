# BGI FlsGetValue

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-fls-get-value`
- Previous capability checkpoint: `bgi-tls-get-value`
- First unsupported capability: `kernel32-set-last-error`

## Purpose

The cumulative BGI session calls the process-owned dynamic endpoint returned by
the earlier `GetProcAddress("FlsGetValue")` and retained through TLS. This
checkpoint connects that endpoint to the existing process-owned FLS slot and
selected-thread value without importing a native index or pointer.

```text
Rust process owns allocated FLS slot 0
Rust selected thread owns value 0x20000000
  -> FlsGetValue(0) returns 0x20000000 and sets LastError to 0
  -> BGI retains the non-NULL value in ESI
  -> next stop is SetLastError(87)
```

## Native evidence

The focused `fls-lifecycle` observer now records the first BGI consumer and a
bounded success／unset／invalid-index oracle. Two complete captures agree:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 441,333 | 1,441 | 685 | `318c76a6ce0da8c7c47d335957876a343e74d2b4bf8f82e013407671e615471e` |
| Confirmation | 434,981 | 1,421 | 675 | `0eb6161706e16a84f97325b35021ea2db0084893a8d14e55b929f3b146cf69d2` |

Both summaries have contiguous sequence numbers, balanced call-ID pairs, max
depth `3`, one oracle result, and zero hook, vtable, call-stack, agent-message,
or cleanup diagnostics. One intervening capture ended with an unrelated
`CreateThread` hook still open at timeout and is excluded.

At BGI return RVA `0xA55EC`, the argument is the index returned by the earlier
`FlsAlloc`; the result equals the value stored by same-thread `FlsSetValue`, and
LastError remains `0`. Native indices and values differ across processes and are
used only for the relation.

The immediate consumer is stable in both accepted runs:

```text
mov  esi, eax
test esi, esi
jne  local-success
push 0x214
push 1
call local-private-heap-helper
```

The non-NULL reached result takes `local-success`; the fallback allocation is
consumer context, not part of the `FlsGetValue` contract.

The same-thread oracle fixes the NULL ambiguity:

| Case | Result | LastError |
|---|---:|---:|
| allocated slot set to `0x12345678` | `0x12345678` | `0` |
| allocated but unset slot | `NULL` | `0` |
| invalid index `0xFFFFFFFF` | `NULL` | `87` (`ERROR_INVALID_PARAMETER`) |

A separate 3,072-byte native PE32/i386 fixture has SHA-256
`82b7074bb21b71a81af6520888bd7f3d4a930174b2afecec0289a31d91b64f66`
and exits with code `0`. It establishes a Windows FLS distinction that TLS does
not have: a newly created thread without FLS storage first reads NULL with
LastError `87`; `FlsSetValue` initializes that lane, after which it reads its own
`0x23456789` with LastError `0`. The main thread retains `0x12345678`.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-fls-get-value.jsonl `
  --seconds 2 --kill-after --observe fls-lifecycle
npm run check:toolchain
```

## Runtime contract

`kernel32.fls-get-value.v1` has stdcall ABI, one 32-bit index argument, a nullable
opaque-pointer return, callee cleanup of four argument bytes, immediate behavior,
and forbidden callback. `LastErrorPolicy::FromHandler` produces:

- allocated／initialized／set: stored selected-thread value, LastError `0`;
- allocated／initialized／unset: NULL, LastError `0`;
- unallocated index: NULL, LastError `87`;
- thread lane without initialized FLS storage: NULL, LastError `87`.

`RuntimeThreadState::fls_storage_initialized` starts false. Successful
`FlsAlloc` initializes the allocating thread's storage, and `FlsSetValue`
initializes the selected lane before storing its value. `fls_value_or_unset`
validates the process slot, thread-storage lifecycle, and selected-thread map.
The read itself changes neither FLS state nor guest memory; the dispatcher
applies only the contract's LastError result.

No option is added to `vine.kernel32-compatibility-policy`; the active
`kernel32-private-heap-size-1-v1` policy remains unchanged. The handler contains
no BGI caller, native index, reached value, or endpoint address.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  fls_get_value_models_thread_storage_and_win32_last_error `
  -- --exact
```

The test fixes dynamic-endpoint identity, ABI, initialized set／unset,
unallocated-index failure, new-thread storage failure, per-thread values,
LastError transitions, no guest writes, and no memory-generation change.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-fls-get-value
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 31 / 70 | 31 / 70 |
| Instruction at next stop | 1,709 | 1,709 |
| CPU writeback batches / dirty pages | 71 / 117 | 199 / 240 |
| Host mapping deltas | 32 | 32 |
| Final generation | 132 | 250 |
| Memory content SHA-256 | `de703069e1242705785faa744b240ab843cf6ee1aa7fa78ddf4ea835e2c4e811` | same |
| Report bytes | 320,070 | 63,746 |
| Report SHA-256 | `f7672697a5f95f709fc242ce1c31b8f95039152a0d06f516aa042c8dafcacbcf` | `a1c336fb7898d378e677a1c523bc75e5569d1d430eebac2b373a22f7854ae066` |

Chrome used 243 slices, 172 budget yields, 305 Rust/Wasm bridge operations,
and 730 successful execution-state transitions. Success, cancel, and watchdog
all end in `stopped` with zero pending request records. The Rust runtime Wasm is
1,726,881 bytes with SHA-256
`a8d9f323c8f457b76d946ddb6e27e757b8f8e4ac4e2bcb3c74246f80af4f6090`.

The separate 180-second workload gate completed 2,181 sessions with no failure;
all 16 versioned thresholds passed. Session p95 was `74.68500000238419 ms`,
slice p99 was `0.5 ms`, and the 978,954-byte report has SHA-256
`6d423e119d0e351aa3ccd38a834957f6d14d1467225f7391758a63fbf2f38d6f`.

## Next structured stop

```text
KERNEL32.dll!SetLastError(error=87)
static endpoint ID: 154
endpoint address:   0x7F000990
IAT address:        0x100D1310
return address:     0x100A5647
instruction:        1709
fault:              endpoint-contract-missing
cluster:            kernel32-api-contract
```

The next checkpoint must measure the zero-return setter's state effect and
immediate consumer before connecting it to the selected-thread LastError field.

## Non-claims

This checkpoint does not implement `FlsFree`, FLS callback teardown, fiber
switching, arbitrary guest-created threads, `SetLastError`, the later
`HeapFree`, title rendering, audio, input, VFS, COM, or full play.
