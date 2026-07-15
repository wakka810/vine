# BGI GetLastError

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-get-last-error`
- Previous capability checkpoint: `bgi-private-heap-size-1`
- First unsupported capability: `kernel32-tls-get-value`

## Purpose

The cumulative BGI session reaches a zero-argument `GetLastError` import after
creating and consuming the one-byte conversion buffer. This checkpoint exposes
the already authoritative selected-thread LastError value through one generic
Kernel32 contract. It adds no compatibility-policy option and no second copy of
thread state.

```text
Rust RuntimeThreads selected-thread record owns LastError = 87
  -> GetLastError() returns 87
  -> dispatcher preserves LastError = 87
  -> BGI copies EAX into EDI for the following call chain
  -> next stop is TlsGetValue(0)
```

## Native evidence

The focused observer hooks `GetLastError`, records the stable caller RVA and
four post-return instructions, and runs a bounded sentinel oracle. Two complete
captures agree:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 8,083,335 | 19,771 | 9,848 | `871e44f554e8a889cc77bf547e2898a0edb6434cec7aec8cdc529305946af325` |
| Confirmation | 8,062,489 | 19,757 | 9,841 | `1f29ca901ab1791ed3ed50a9cf27e5b9f47b9191b3984c4cfbaabd43d6560b57` |

Both summaries are balanced and contain zero hook, vtable, call-stack, agent,
or cleanup diagnostics. In both runs, the first BGI call at return RVA
`0xA55DD` observes LastError `87`, returns `87`, and leaves LastError `87`.

The return address disassembles to the same immediate consumer shape in both
runs:

```text
push dword ptr [global]
mov  edi, eax
call local-helper
call eax
```

The first instruction is independent of EAX; `mov edi,eax` retains the
`GetLastError` result before the subsequent calls. Absolute native addresses,
thread IDs, total hot-call counts, and timing are run identity.

The x86 oracle injects thread LastError sentinels `0`, `87`, `122`, and
`0x12345678` at the API boundary. Every call returns the sentinel and preserves
the same value after return. This fixes the full 32-bit read behavior without
encoding BGI's reached value into the handler.

## Runtime contract

`kernel32.get-last-error.v1` has stdcall ABI, zero arguments, a 32-bit return,
callee cleanup of zero argument bytes, immediate behavior, forbidden callback,
and `LastErrorPolicy::Preserve`. The handler reads
`ExecutionSession::last_error()`, whose authority is the selected
`RuntimeThread` record, and returns it without memory or runtime mutation.

No new field is added to `vine.kernel32-compatibility-policy`. The active
`kernel32-private-heap-size-1-v1` policy still bounds earlier handles, heap
pairs, environment access, and Windows-932 conversion; `GetLastError` itself
has no target-specific valid-input variant to enable.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_last_error_reads_selected_thread_state_without_mutation `
  -- --exact
```

The test fixes zero-argument ABI, implementation identity, initial-thread
return, a second selected thread with a distinct LastError, restoration of the
initial lane, state equality across each call, no guest writes, and no memory
generation change.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-last-error
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 29 / 68 | 29 / 68 |
| Instruction at next stop | 1,695 | 1,695 |
| CPU writeback batches / dirty pages | 69 / 115 | 197 / 238 |
| Host mapping deltas | 32 | 32 |
| Final generation | 130 | 248 |
| Memory content SHA-256 | `ea204befffc2c6eebbf5aed3a6e11f792644a35bce5c54a23c996678f2ccb909` | same |
| Report bytes | 312,644 | 62,734 |
| Report SHA-256 | `29b3517baa9bc71d3b34b523f0a57c0cb17a0ff09e1dba9a842a0fc1339516b6` | `0ab0357b78760894db9ad18b0902816470ac1f6ec191f66371b3687cded26f00` |

Chrome used 241 slices, 172 budget yields, 301 Rust/Wasm bridge operations,
and 724 execution-state transitions. Success, cancel, and watchdog all end in
`stopped` with zero pending request records. The Rust runtime Wasm is 1,720,151
bytes with SHA-256
`0e25639317d172fefa9610204f0f99126dcbbb58c945938193bc7e41b8ae0830`.

The separate 180-second workload gate completed 2,151 sessions with no failure.
All 16 versioned thresholds passed. The 966,127-byte report has SHA-256
`22a395242d4e1d7f1c8de4e624c27dfb2d78609e489007a8ab0304cf216c3ea9`.

## Next structured stop

```text
KERNEL32.dll!TlsGetValue(index=0)
static endpoint ID: 69
endpoint address:   0x7F000440
IAT address:        0x100D11BC
return address:     0x100A54A0
instruction:        1695
fault:              endpoint-contract-missing
```

The process already owns logical TLS index `0` and the selected thread's value,
but no `TlsGetValue` ABI contract is registered. The next checkpoint must trace
native success／failure, LastError behavior, consumer, and per-thread value
relation before connecting that existing state.

## Non-claims

This checkpoint does not implement `TlsGetValue`, `SetLastError`, TLS free or
teardown, arbitrary TLS indices, guest-created CPU threads, the later
`HeapFree`, title rendering, audio, input, VFS, COM, or full play.
