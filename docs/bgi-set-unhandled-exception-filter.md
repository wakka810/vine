# BGI `SetUnhandledExceptionFilter` checkpoint

## Status

- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-set-unhandled-exception-filter`
- Frozen historical prefix: `bgi-seventeenth-win32-call`
- Implemented contract: `kernel32.set-unhandled-exception-filter.v1`
- Compatibility policy: unchanged `kernel32-is-processor-feature-present-v1`
- First unsupported capability: `kernel32-heap-size`

This checkpoint records one executable guest filter as process-owned Rust
state, returns the previous filter, and keeps host SEH／browser exception
machinery outside the guest-visible authority. It then reuses the existing
critical-section and pointer-decoding contracts before stopping at `HeapSize`.

## Native evidence

The final focused captures are uncommitted private artifacts:

| Capture | Bytes / lines | SHA-256 | Balanced API pairs |
|---|---:|---|---:|
| r1 | 2,238,917 / 7,031 | `d97a220135ce703dd0cfb7be263d264319073d0de3165b5ab6217b6ee2e7664b` | 3,478 |
| r2 | 2,240,926 / 7,037 | `ebc23df0b5d621490dcf8be5232d6a3510e9c87cab19ab36f428a7daea8a26df` | 3,481 |

Both traces have contiguous sequence metadata, balanced calls, and zero
diagnostics. In both runs BGI calls the API once from return RVA `0x000A81BC`
with filter RVA `0x000A816F`. Native returns `NULL` and preserves LastError
`0 -> 0`. The wrapper immediately executes `xor eax,eax; ret`; its outer
caller at RVA `0x000A5060` tests EAX and branches only for a nonzero previous
filter.

The `x86-process-filter-replacement-v1` oracle installs callback A, replaces it
with callback B, removes the filter with `NULL`, and restores the original
filter. The returned chain is `original, A, B, NULL`; the original filter was
`NULL` in both captures. Sentinels `0`, `87`, `0x12345678`, and `122` are
preserved by both the hook and the system call. Temporary native callback
addresses and the observed empty initial state are evidence only.

## Runtime contract

`RuntimeProcessState.unhandled_exception_filter` is the sole guest-visible
authority. `SetUnhandledExceptionFilter`:

- decodes one nullable x86 pointer and returns a nullable previous pointer;
- preflights a non-NULL filter for executable guest access before mutation;
- replaces the process-owned record atomically and permits `NULL` removal;
- uses stdcall callee cleanup (`ESP += 8`) and preserves LastError;
- performs no guest-memory write and never installs a host callback;
- retains the record through exit request and retires it at process exit
  completion or direct termination.

The public fixture installs A, replaces it with B, removes it, rejects an
unmapped pointer without changing runtime or memory, and verifies process-exit
retirement. The handler contains no BGI address, call sequence, or reached
previous value. No new compatibility-policy selector is needed because the
semantic domain is the checked nullable executable pointer itself.

## Node continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-set-unhandled-exception-filter
```

Call 203 registers `0x100A816F`, returns `NULL`, and preserves LastError 0.
The next writeback covers the wrapper／outer consumer. Existing contracts then
complete call 204 `EnterCriticalSection(0x100FF6C0)` and calls 205／206
`DecodePointer(0xEC010010)`, both resolving the earlier allocation token to
`0x20000C58`.

| Evidence | Value |
|---|---:|
| Supported calls / contracts | 206 / 42 |
| CPU writebacks / dirty pages | 207 / 316 |
| Changed bytes | 4,690 |
| Host deltas | 57 |
| Final generation | 294 |
| Memory SHA-256 | `563d391b69ea3dcad1669d68f167bb35fad5b4d48adbd153a9f95ad1fb072f97` |
| Report bytes | 912,806 |
| Report SHA-256 | `92caa9ef24a63fc1abbac0bfc063361b2c7b9998cd14d2d2ee02e2d5c999defa` |

## Chrome equivalence and workload gate

`npm run check:runtime:bgi-browser` reproduces all 206 ordered completions,
the memory digest, and the `HeapSize` stop in Chrome. The Worker uses 2,452
slices, 2,245 budget yields, 1,817 writeback batches over 2,379 pages, 57
mapping deltas, and 2,084 Rust／Wasm crossings. It ends at generation 1,628;
success, cancellation, and watchdog all stop with no pending work.

The canonical 222,793-byte browser report has SHA-256
`9c67ec46b12a9b6ca74c4e097d5ae5548d51f00a8f757e9cf1f935b11f6e8cec`.
The Rust runtime core is 1,872,810 bytes with SHA-256
`f3cee0de9eeea878748c0eb83e7b5493bec3daeca0b619452bc7e948d28a8a7c`.

The 180-second workload completed 411 sessions with zero failure and passed
all 17 thresholds. Session P95 was 526.395 ms, slice P99 was 2.125 ms, and
heartbeat P99／maximum were 16.140／62.570 ms. Main and Worker heap slopes were
-63,540.23 and 1,476,679.03 bytes/min; backing-storage slope was
2,936,927.79 bytes/min. The 968,104-byte report has SHA-256
`35e1abe5b4ec65ff9f750388ad2c7f95c11a2c69be5ef4d7c8dfd310cd0b59a3`.

## Next structured stop

```text
KERNEL32.dll!HeapSize(0x68010001, 0, 0x20000C58)
endpoint ID:       73
endpoint address:  0x7F000480
IAT:               0x100D11CC
return:            0x100A4F77
instruction:       19096
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must trace return size, failure sentinel, LastError,
pointer-to-live-allocation relation, flags, and consumer. It must not turn the
single reached allocation into unrestricted heap introspection.

## Non-claims

This checkpoint stores a top-level filter but does not dispatch SEH, invoke the
filter, model exception records／contexts, implement vectored handlers or
unhandled-exception UI, or bridge guest exceptions to JavaScript exceptions. It
also does not implement broad `HeapSize`, `HeapReAlloc`, heap validation, title
rendering, audio, input, VFS, COM, or full play.
