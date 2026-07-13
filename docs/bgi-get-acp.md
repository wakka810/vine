# BGI GetACP

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-get-acp`
- Previous capability checkpoint: `bgi-set-last-error`
- First unsupported capability: `kernel32-private-heap-size-544`

## Purpose

The cumulative BGI session reaches `GetACP()` after three repetitions of the
thread LastError/TLS/FLS helper. This checkpoint connects the zero-argument
getter to the active ANSI code page already owned by the versioned Kernel32
compatibility policy.

```text
selected thread LastError = 87
  -> GetACP() returns policy active code page 932
  -> LastError remains 87 and no guest byte changes
  -> the outer caller stores and compares the returned code page
  -> execution continues to HeapAlloc(0x68010001, 0, 0x220)
```

## Native evidence

The focused `get-acp` observer records the first BGI return site, its local
cleanup path, the EBP-frame caller continuation, and a bounded sentinel oracle.
Two complete captures agree:

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 269,228 | 875 | 401 | `91030f4ca8afb53e285739bdacadc8d3e8bef7ee35730eb3b2d605529e6363f3` |
| Confirmation | 269,229 | 875 | 401 | `a5f3c00132344a43c51467ef4f3e83309edd49140fd6f2391891c84aea54b2b4` |

Both summaries have contiguous sequence numbers, balanced call-ID pairs,
maximum depth `3`, one completed oracle, and zero hook, vtable, call-stack,
agent-message, or cleanup diagnostics.

The BGI call returns to RVA `0xA6F72`, has no arguments, returns `932`, and
preserves LastError `87 -> 87`. The local continuation jumps to the helper's
cleanup at RVA `0xA6F4F`; neither cleanup branch overwrites EAX before `ret`.
The recorded outer frame then consumes the returned value at RVA `0xA71AF`:

```text
mov dword ptr [ebp + 8], eax
cmp eax, dword ptr [ebx + 4]
je  RVA 0xA7312
```

The x86 oracle invokes `GetACP` with LastError sentinels `0`, `87`, `122`, and
`0x12345678`. Every call returns `932`; the hook result and `SystemFunction`
result agree, and both observations preserve the full sentinel LastError.

Reproduction:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work/windows-traces/bgi-get-acp.jsonl `
  --seconds 2 --kill-after --observe get-acp
```

## Runtime contract

`kernel32.get-acp.v1` has stdcall ABI, no arguments, a 32-bit unsigned return,
callee cleanup of only the return address, immediate behavior, forbidden
callback, and `LastErrorPolicy::Preserve`. The semantic handler reads
`active_code_page()` from `Kernel32CompatibilityPolicy`; it contains no BGI
caller, sequence, or reached-value branch. The current policy supplies
Windows-932. A policy without an active ANSI code page stops with
`get-acp-policy-disabled` before state mutation.

The successful call sets guest EAX to `932`, preserves the selected thread's
LastError, performs no guest write, and does not change the memory generation or
runtime state.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  get_acp_reads_only_the_policy_owned_active_code_page `
  -- --exact
```

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-acp
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 33 / 82 | 33 / 82 |
| `GetACP` occurrences | 1 | 1 |
| Instruction at next stop | 1,978 | 1,978 |
| CPU writeback batches / dirty pages | 83 / 134 | 231 / 277 |
| Host mapping deltas | 34 | 34 |
| Final generation | 146 | 284 |
| Memory content SHA-256 | `7ed5262e4eef60c3f665765d5741872b595205c5022193ec4633194e5ffa25ea` | same |
| Report bytes | 367,384 | 70,985 |
| Report SHA-256 | `b897324ff6b85383b2b51be1505e84868e53d9fa5488e5d8f0f4da0ed36b476d` | `d897812a8a0cb6f1da2bd225db39ca32d96ae580030954c9ecf5e7030bce665c` |

Chrome used 280 slices, 197 budget yields, 351 Rust/Wasm bridge operations,
and 841 successful state transitions. Success, cancellation, and watchdog all
stop with no pending request. The Rust runtime Wasm is 1,729,257 bytes with
SHA-256 `d535144c026009d2af718723057e0f9c0baa283ebc50345b5d0e5f5dbaaf4ce7`.

The separate 180-second workload gate completed 2,066 sessions with no failure;
all 16 thresholds passed. Session p95 was `81.71000003814697 ms`, slice p99 was
`0.5600000023841858 ms`, and the 928,361-byte report has SHA-256
`d5566879c6a30a8f1376552c961057631bc7b0e52aa4fb4433360f19f67dd65c`.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(heap=0x68010001, flags=0, bytes=0x220)
static endpoint ID: 75
endpoint address:   0x7F0004A0
IAT address:        0x100D11D4
return address:     0x100A4A33
instruction:        1978
fault:              heap-alloc-variant-unsupported
cluster:            kernel32-private-heap-allocation
```

The next checkpoint must measure the native allocation result, initialization,
consumer, and lifetime relation before adding only the reached `(flags=0,
size=0x220)` pair to compatibility policy data.

## Non-claims

This checkpoint does not implement arbitrary ACP selection or mutation,
`HeapAlloc` size `0x220`, broad heap flags/sizes/failure, `HeapFree`, TLS/FLS
teardown, title rendering, audio, input, VFS, COM, or full play.
