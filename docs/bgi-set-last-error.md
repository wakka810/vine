# BGI SetLastError

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-set-last-error`
- Previous capability checkpoint: `bgi-fls-get-value`
- First unsupported capability: `kernel32-get-acp`

## Purpose

The cumulative BGI session reaches `SetLastError(87)` after reading the existing
TLS and FLS values. This checkpoint connects the one-argument VOID contract to
the selected Rust runtime thread's authoritative LastError field.

```text
selected RuntimeThread LastError = 0
  -> SetLastError(87) changes only that thread's LastError to 87
  -> the local helper overwrites EAX from ESI while returning
  -> the same helper is reached three times
  -> existing contracts continue to GetACP()
```

## Native evidence

The focused `set-last-error` observer records argument, LastError and EAX before
and after each call, the first BGI consumer, and a bounded sentinel oracle. Two
complete captures agree:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 1,889,260 | 5,671 | 2,800 | `aff200fc1e726988d2fbaee436959835572eef5fb5d99e50fe8d2d5a2afd1022` |
| Confirmation | 1,892,095 | 5,680 | 2,804 | `59c64d74d12510264e50c282b7f4caaffc47b29c8928dd969137a9dd5fd00841` |

Both summaries have contiguous sequence numbers, balanced call-ID pairs,
maximum depth `4`, one completed oracle, and zero hook, vtable, call-stack,
agent-message, or cleanup diagnostics.

The first BGI call returns to RVA `0xA5647`. Its argument is `87`, LastError
changes from `0` to `87`, and the immediate consumer is identical in both runs:

```text
pop edi
mov eax, esi
pop esi
ret
```

Native `SetLastError` does not preserve EAX, but both the entry and exit values
are process-specific implementation state. The caller overwrites EAX before
returning from the helper, so no native register value becomes a target
compatibility requirement.

The x86 oracle invokes values `0`, `87`, `122`, and `0x12345678`. For every
case, the hook argument, hook LastError after return, and `SystemFunction`
captured LastError equal the full 32-bit sentinel. Native EAX is clobbered for
all four calls and is retained only as evidence classification data.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-set-last-error.jsonl `
  --seconds 2 --kill-after --observe set-last-error
```

## Runtime contract

`kernel32.set-last-error.v1` has stdcall ABI, one 32-bit error argument, a VOID
return, callee cleanup of four argument bytes, immediate behavior, forbidden
callback, and `LastErrorPolicy::FromHandler`. The handler returns the argument
as the dispatcher-owned LastError update. It changes no guest byte and no
process state other than the selected thread's LastError.

The generic VOID return policy preserves guest EAX. This does not claim native
EAX preservation: Win32 exposes no return value, native values vary by process,
and the measured BGI consumer overwrites EAX immediately. No compatibility
policy option is added, and the handler contains no reached value, caller RVA,
or call-count branch.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  set_last_error_writes_only_the_selected_thread_without_guest_memory_mutation `
  -- --exact
```

The test fixes VOID ABI, full 32-bit and zero values, selected-thread isolation,
EAX preserve policy, no guest writes, and no memory-generation change.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-last-error
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 32 / 81 | 32 / 81 |
| `SetLastError` occurrences | 3 | 3 |
| Instruction at next stop | 1,936 | 1,936 |
| CPU writeback batches / dirty pages | 82 / 132 | 225 / 271 |
| Host mapping deltas | 34 | 34 |
| Final generation | 145 | 278 |
| Memory content SHA-256 | `3f212a5faa9d76896ed9f464e344aafe7c1fae60baae03856815a4c89cb417fe` | same |
| Report bytes | 363,609 | 70,464 |
| Report SHA-256 | `a915d7b91d65e6b1d4805bc4f0ab62d6444573693402bed9fd473518f56f9657` | `39635a36c5a73beb78e3741554bfbb369df7aea81fe2b451fa883f237d5aad65` |

The three setter calls occur at instructions `1709`, `1777`, and `1897` and
all set LastError `87`. Existing `GetLastError`, TLS/FLS reads, and critical
section contracts carry the session to `GetACP`. Chrome used 274 slices, 192
budget yields, 344 Rust/Wasm bridge operations, and 823 successful state
transitions. Success, cancel, and watchdog all stop with no pending request.
The Rust runtime Wasm is 1,728,307 bytes with SHA-256
`e6904030653ac3f1c48335a8e7b64ee7183e5f1c61474618848c1158793bb731`.

The separate 180-second workload gate completed 2,086 sessions with no failure;
all 16 thresholds passed. Session p95 was `80.55500000715256 ms`, slice p99 was
`0.6500000357627869 ms`, and the 937,568-byte report has SHA-256
`111e84e4a5b1d017091ecec07b6c801699a27dd3fa1a5780de51cd9a363ddb3b`.

## Next structured stop

```text
KERNEL32.dll!GetACP()
static endpoint ID: 156
endpoint address:   0x7F0009B0
IAT address:        0x100D1318
return address:     0x100A6F72
instruction:        1936
fault:              endpoint-contract-missing
cluster:            kernel32-api-contract
```

The next checkpoint must measure the active-code-page return, LastError
behavior, and immediate consumer before connecting it to the existing
policy-owned Windows-932 active code page.

## Non-claims

This checkpoint does not implement `GetACP`, arbitrary code-page selection,
TLS/FLS free or teardown, the later `HeapFree`, title rendering, audio, input,
VFS, COM, or full play.
