# BGI TlsGetValue

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-tls-get-value`
- Previous capability checkpoint: `bgi-get-last-error`
- First unsupported capability: `kernel32-fls-get-value`

## Purpose

The cumulative BGI session reaches `TlsGetValue(0)` after reading the selected
thread's LastError. This checkpoint exposes the existing process-owned TLS index
and selected-thread value through one policy-independent Kernel32 contract.

```text
Rust process owns allocated TLS index 0
Rust selected thread owns value 0x7F020010
  -> TlsGetValue(0) returns 0x7F020010 and sets LastError to 0
  -> BGI retains and invokes the returned dynamic endpoint
  -> next stop is FlsGetValue(0) at that endpoint
```

No host TLS value, native pointer, or evidence result is imported into guest
state. The reached value is already the Rust-owned identity of the logical
`FlsGetValue` endpoint installed by the earlier dynamic-export checkpoint.

## Native evidence

The focused `tls-lifecycle` observer records the first BGI consumer and a
bounded success／unset／invalid-index oracle. Two complete captures agree:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 5,867,455 | 18,489 | 9,207 | `d9a85cadcd1d1891480e5a3c3285a14b0f111c9f805770faf4866733c24beaa5` |
| Confirmation | 6,331,907 | 19,955 | 9,940 | `78b951d688dce1ffcaa5a243b1e71e3a062af346d711794ba7f6be8f9afb4ba6` |

Both summaries contain zero hook, vtable, call-stack, agent, or cleanup
diagnostics. At BGI return RVA `0xA54A0`, the argument is the index returned by
the preceding same-thread `TlsAlloc`; the result equals the value previously
stored by `TlsSetValue`, and LastError changes from `87` to `0`.

The immediate consumer is stable in both runs:

```text
mov  esi, eax
test esi, esi
jne  local-success
push dword ptr [global]
call dword ptr [DecodePointer]
mov  esi, eax
```

The oracle fixes the NULL ambiguity and failure behavior independently of BGI:

| Case | Result | LastError |
|---|---:|---:|
| allocated index set to `0x12345678` | `0x12345678` | `0` |
| allocated but unset index | `NULL` | `0` |
| invalid index `0xFFFFFFFF` | `NULL` | `87` (`ERROR_INVALID_PARAMETER`) |

A separate native PE32/i386 fixture verifies thread isolation. Its 3,072-byte
image has SHA-256
`83bdaf9f52b246af3924106280feadce6419e1de78b266ea6b36b469cea7e6c3`
and exits with code `0`: the main thread reads `0x12345678`, a worker initially
reads NULL and then its own `0x23456789`, and the main-thread value remains
unchanged.

## Runtime contract

`kernel32.tls-get-value.v1` has stdcall ABI, one 32-bit index argument, a nullable
opaque-pointer return, callee cleanup of four argument bytes, immediate behavior,
and forbidden callback. `LastErrorPolicy::FromHandler` distinguishes a valid
NULL value from an invalid index:

- allocated／set returns the selected thread's stored value and LastError `0`;
- allocated／unset returns NULL and LastError `0`;
- unallocated index returns NULL and LastError `87`.

`RuntimeState::tls_value_or_unset` first validates the process-owned allocation
registry, then reads only the selected `RuntimeThread` map. The read does not
change TLS state or guest memory. The dispatcher applies only the contract's
LastError result to the selected thread.

No option is added to `vine.kernel32-compatibility-policy`; the active
`kernel32-private-heap-size-1-v1` policy remains unchanged. The runtime handler
contains no BGI caller, reached index, reached value, or endpoint ID.

The same checkpoint also makes missing-contract classification reflect endpoint
ownership. Static IAT endpoints retain `endpoint-contract-missing`; a
process-owned dynamic export now stops as `dynamic-export-contract-missing`.
This changes diagnostics only and does not select or synthesize a handler.

Representative public checks:

```powershell
cargo test -p vine-core --test api_dispatcher `
  tls_get_value_reads_only_the_selected_thread_and_sets_win32_last_error `
  -- --exact
npm run check:toolchain
```

The Rust test fixes ABI and implementation identity, set／unset／invalid cases,
two selected thread lanes, LastError transitions, no guest writes, and no memory
generation change. The toolchain gate builds and executes the native PE32
thread-isolation fixture on Windows.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-tls-get-value
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 30 / 69 | 30 / 69 |
| Instruction at next stop | 1,703 | 1,703 |
| CPU writeback batches / dirty pages | 70 / 116 | 198 / 239 |
| Host mapping deltas | 32 | 32 |
| Final generation | 131 | 249 |
| Memory content SHA-256 | `3b2a4d71de4d796d93b92b16a2cb0ab1b62b45cab0cdaf173c5d6f09895736ba` | same |
| Report bytes | 316,360 | 63,088 |
| Report SHA-256 | `f14401c55cac1c5247050aad297cc19e35cdd2b4f772ccb842568e0de21ee2d9` | `8c94f375a2fc1a6929c209ba4b9f1f36429099c941385adcfe3c4b9fe2b61644` |

Chrome used 242 slices, 172 budget yields, 303 Rust/Wasm bridge operations,
and 727 successful execution-state transitions. Success, cancel, and watchdog
all end in `stopped` with zero pending request records. The Rust runtime Wasm is
1,722,580 bytes with SHA-256
`d01e45f055059107fbec8c51e28998cdf913a97048826ae43b1f0b570a0e0de4`.

The separate 180-second workload gate completed 2,145 sessions with no failure;
all 16 versioned thresholds passed. Session p95 was `76.56000000238419 ms` and
slice p99 was `0.4750000238418579 ms`. The 963,320-byte report has SHA-256
`96e0fa6ed93909abff5e3a06b0494bd898297dc0dc822b40dd87b8dca4fd29e5`.

## Next structured stop

```text
KERNEL32.dll!FlsGetValue(index=0)
dynamic endpoint ID: 2
endpoint address:    0x7F020010
return address:      0x100A55EC
instruction:         1703
fault:               dynamic-export-contract-missing
cluster:             kernel32-fiber-local-storage
```

The process already owns FLS slot `0` and a selected-thread value, but the
callable dynamic endpoint has no ABI contract. The next checkpoint must fix
native success／failure, LastError, immediate consumer, and per-thread relation
before connecting that state.

## Non-claims

This checkpoint does not implement `TlsFree`, TLS teardown, `FlsGetValue`, FLS
free／callback teardown, arbitrary TLS allocation policy, guest-created CPU
threads, the later `HeapFree`, title rendering, audio, input, VFS, COM, or full
play.
