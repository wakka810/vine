# BGI private-heap 544-byte allocation

## Status

- Status: verified
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-private-heap-size-544`
- Previous capability checkpoint: `bgi-get-acp`
- First unsupported capability: `kernel32-get-cp-info`

## Purpose

The cumulative BGI session uses the active ANSI code page returned by `GetACP`,
then requests a 544-byte non-`HEAP_ZERO_MEMORY` block from its existing private
heap. This checkpoint adds only the reached policy pair and reuses the existing
target-independent `kernel32.heap-alloc.v2` semantics.

```text
GetACP() -> 932
  -> HeapAlloc(privateHeap, flags=0, size=0x220)
  -> deterministic guest allocation ID 4 at 0x20000A20
  -> BGI copies 0x88 DWORDs into the block and clears its first DWORD
  -> existing LastError/TLS/FLS helpers continue
  -> next stop is GetCPInfo(932, 0x006FF9C0)
```

## Native evidence

The focused `heap-allocation` observer records only `HeapAlloc` and `HeapFree`
calls made directly by the main executable. It captures a bounded fingerprint
of each returned buffer and an x86 EBP caller chain, while retaining normal
call-ID pairing and diagnostic checks.

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-heap-size-544-20260714-r4.jsonl `
  --seconds 0.1 --kill-after --observe heap-allocation
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 361,127 | 169 | 59 | `da2eb2df7be81130ccb52c37be0573a6b04f5d4563fefe18d22f458951195b08` |
| Confirmation | 361,343 | 169 | 59 | `1daa8f2f1ca3766c6cdc078aee3557cb5aa7f37b27d28e1b004ec83279e001b9` |

Both summaries have zero hook, vtable, call-stack, agent-message, or cleanup
diagnostics. A separate two-second capture is excluded because termination
occurred inside an unrelated later `HeapAlloc`; the summarizer rejected the
unbalanced trace instead of accepting partial evidence.

At BGI return RVA `0xA4A33`, both accepted runs call
`HeapAlloc(nativeHeap, 0, 544)`, return a non-null process pointer, and preserve
LastError `0 -> 0`. Native heap handles and result pointers differ per process
and are discarded.

The returned 544 bytes contain 519 NUL bytes in each run, but their FNV-1a
fingerprints differ (`0xDDE20A18` and `0xC8C82863`). Therefore the captures do
not establish a stable initialization value. The runtime does not copy or
zero-fill these native bytes.

The bounded frame walk distinguishes the retry wrapper at RVA `0xA4E78` from
the actual consumer at RVA `0xA71C5`:

```text
mov ebx, eax
test ebx, ebx
je RVA 0xA7316
mov ecx, 0x88
mov esi, dword ptr [edi + 0x68]
mov edi, ebx
rep movsd dword ptr es:[edi], dword ptr [esi]
and dword ptr [ebx], 0
push ebx
```

Thus BGI checks the allocation, overwrites all 544 requested bytes, clears the
first DWORD, and passes the same pointer onward. Neither accepted 100 ms trace
observes a related `HeapFree`; this proves only that the block remains live
through the observed consumer, not that it is never freed later.

## Runtime contract

Policy `kernel32-private-heap-size-544-v1` adds exactly `(flags=0,
byteLength=544)` beside the prior zeroed `0x214`／`0x800` and flags-zero size-1
variants. The policy contains no BGI caller RVA, native pointer, result, call
order, or handler branch.

The unchanged `kernel32.heap-alloc.v2` handler validates the logical heap,
policy pair, nonzero size, alignment, capacity, and address-space ownership
before mutation. It returns allocation ID `4` at guest address `0x20000A20`,
with requested and reserved lengths both `0x220`. Flags zero selects
`deterministic-uninitialized` bytes (`0xA5`) for replay stability. This is a
runtime compatibility choice, not a claim about native initial content.

The heap arena is already reserved and committed, so allocation host delta
sequence `83` is dirty-only: generation `146 -> 147`, owner `10`, page
`0x20000000`, payload SHA-256
`104d76ea818bcc3969bad0e0a277dc5954a2b175dd508f7952b9eddf98136fb4`.
The following guest consumer is reconciled by writeback sequence `84`; it
changes 617 bytes across the stack and heap pages at generation `147 -> 148`.
Its payload SHA-256 is
`e96d08f3b4a9e8663b3b381c23737195780a648af108bd99004ceaf70438d151`.

Unsupported flag or size pairs still stop with
`heap-alloc-variant-unsupported` before heap metadata or guest memory changes.

Representative tests:

```powershell
cargo test -p vine-core --test api_dispatcher `
  heap_alloc_policy_enables_bounded_sizes_without_changing_semantics `
  -- --exact
cargo test -p vine-core --test win32_compatibility_policy --locked
npm run check:win32-policy
npm run check:windows-trace
```

The Rust test fixes mutation-free rejection under the preceding policy,
allocation identity and lengths, deterministic initialization, LastError,
stdcall cleanup, and dirty-only delta shape. Policy tests validate both the
preceding and new JSON profiles without target-evidence identity.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-private-heap-size-544
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 33 / 87 | 33 / 87 |
| Instruction at next stop | 4,523 | 4,523 |
| CPU writeback batches / dirty pages | 88 / 142 | 537 / 588 |
| Host mapping deltas | 35 | 35 |
| Final generation | 152 | 589 |
| Memory content SHA-256 | `c420169199f66f0bec6dc619b5d694e1f4987ea1990ae98f4d6a7773c63fcb26` | same |
| Report bytes | 386,921 | 75,422 |
| Report SHA-256 | `355c6c2dfc089c8d6f1ca30ea6cbe3b697f4e7afdfa6ed1ec4f2b0ea15593781` | `67403b09f026983ccb9175d3f017eeb5faa720cc46481adf5f7593284ce979f9` |

Chrome used 599 slices, 511 budget yields, 663 Rust/Wasm bridge operations,
and 1,798 execution-state transitions. Success, cancellation, and watchdog
all finish in `stopped` with zero pending host request records.

The separate 180-second workload completed 1,723 sessions with no failure.
All 16 versioned thresholds passed; session p95 was
`98.40499997138977 ms` and slice p99 was `0.6600000262260437 ms`. The
777,280-byte report has SHA-256
`ce143afbbbddb682c98fce14e4d2be7dcab3165e8f90ba32f4aca5a67784c3a1`.

## Next structured stop

```text
KERNEL32.dll!GetCPInfo(932, 0x006FF9C0)
static endpoint ID: 155
endpoint address:   0x7F0009A0
IAT address:        0x100D1314
return address:     0x100A6D13
instruction:        4523
fault:              endpoint-contract-missing
```

The next checkpoint must trace the reached `CPINFO` output shape, LastError
relation, and immediate consumer, then implement only the active-code-page
variant through checked guest-memory output. It must not infer arbitrary code
page tables or broaden adjacent locale APIs without evidence.

## Non-claims

This checkpoint does not reproduce native uninitialized bytes, expose native
heap pointers, enable arbitrary heap flags／sizes／failure, implement
`HeapFree`／`HeapReAlloc`, implement `GetCPInfo`, generalize code-page metadata,
or reach title rendering, audio, input, VFS, COM, or full play.
