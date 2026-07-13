# BGI WideCharToMultiByte

## Status

- Status: verified
- Date: 2026-07-13
- Runtime-kernel checkpoint: `bgi-wide-char-to-multi-byte`
- Previous capability checkpoint: `bgi-environment-strings-w`
- First unsupported capability: `kernel32-private-heap-size-1`

## Purpose

The cumulative BGI session reached a two-pass
`kernel32.dll!WideCharToMultiByte` conversion. This checkpoint adds a bounded,
host-locale-independent Windows-932 path and keeps the conversion inside the
generic dispatcher write contract.

```text
controlled UTF-16 environment copy
  -> WideCharToMultiByte(CP_ACP, ..., destination=NULL) returns 1 byte
  -> BGI requests a one-byte block from its existing private heap
  -> later conversion and FreeEnvironmentStringsW remain ordered consumers
```

## Native evidence

The focused observer records source and destination shape plus comparison
hashes. It does not record environment names or values. Two captures agree:

| Capture | Bytes | Lines | SHA-256 |
|---|---:|---:|---|
| Primary | 307,869 | 882 | `36244d5e122a3a6f6433c4013522946af680b2bd0867c7389daec8394176e299` |
| Confirmation | 308,089 | 883 | `cef9895a09956273c0c9ac3a678ce05faa0c04ad6f713c6a175b3a05423f4d16` |

The native active ANSI code page is `932`. BGI queries at return RVA
`0xA8599` and converts at return RVA `0xA85BC`. Its launch-environment source
contains 3,637 UTF-16 units; query and conversion both return 3,637 bytes and
preserve LastError `87 -> 87`. The source/output sizes and hashes are evidence
about that launch only, not a required runtime environment.

A controlled x86 oracle fixes the reached semantic boundaries:

| Case | Result | LastError | Output / used-default |
|---|---:|---:|---|
| one NUL, size query | 1 | preserves 87 | no destination |
| one NUL, conversion | 1 | preserves 88 | `00` |
| `A` + U+3042 + NUL, query | 4 | preserves 89 | used-default `0` |
| same conversion | 4 | preserves 90 | `41 82 A0 00`, used-default `0` |
| surrogate pair + NUL with `?` default | 3 | preserves 91 | `3F 3F 00`, used-default `1` |
| one-byte destination for `A` + U+3042 + NUL | 0 | 122 | partial prefix `41`, used-default `0` |
| zero source count | 0 | 87 | no destination |

The observer also sees positive-count and bounded `-1` NUL-terminated calls in
the process. Pointer values and content hashes identify a run and are not guest
ABI.

## Runtime contract

Policy `kernel32-wide-char-to-multi-byte-v1` fixes `CP_ACP` to Windows-932. It
does not ask the browser, Node, Windows host, or Rust locale for a code page.
The same semantic handler also accepts explicit code page `932`; other code
pages and nonzero flags stop before mutation.

The handler supports positive UTF-16 unit counts and `-1` scanning up to the
policy limit of 65,536 units. A size query requires both destination pointer and
capacity to be zero. A conversion requires both to be nonzero. Null source,
zero or less-than-`-1` count, mixed destination pointer/capacity, and overlapping
source/destination return zero with `ERROR_INVALID_PARAMETER` (`87`).

Windows-932 BMP characters are encoded with a deterministic Rust table.
Unmappable UTF-16 units use the system default byte `?`; an optional
`lpUsedDefaultChar` BOOL reports substitution. A too-small destination receives
only complete encoded-unit prefixes, returns zero, and sets
`ERROR_INSUFFICIENT_BUFFER` (`122`). Successful query/conversion preserves the
incoming LastError. An explicit `lpDefaultChar` pointer remains a structured
unsupported variant even though the native oracle measured the `?` case.

`ApiDynamicPointerConstraint` represents the nullable, one-byte-aligned,
bounded variable-length destination. The dispatcher validates every returned
write against that contract before committing the output and optional BOOL in
one guest-memory transaction; the handler does not bypass dispatcher write
ownership.

Representative public test:

```powershell
cargo test -p vine-core --test api_dispatcher `
  wide_char_to_multi_byte_models_windows_932_query_conversion_and_failures `
  -- --exact
```

The test fixes query and conversion results, Windows-932 bytes, substitution,
partial output, LastError transitions, overlap and invalid-count behavior, an
unsupported code page, unchanged state on structured rejection, and the
dynamic output-write contract.

## Node and Chrome continuation

```powershell
npm run check:runtime:bgi -- --profile kernel32-wide-char-to-multi-byte
npm run check:runtime:bgi-browser
```

| Evidence | Node | Chrome Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 28 / 64 | 28 / 64 |
| Instruction at next stop | 1,591 | 1,591 |
| CPU writeback batches / dirty pages | 65 / 109 | 185 / 225 |
| Host mapping deltas | 30 | 30 |
| Final generation | 124 | 234 |
| Memory content SHA-256 | `9b2b6b23255736f59b3c02c75bb3dd5adf811fb99e93ce40bc31a601ba55fddb` | same |
| Report bytes | 295,762 | 60,527 |
| Report SHA-256 | `ea0eb0f6a14869ca4e75643f01bd42c8c2e7360584c893e7578f6c21423b56cf` | `04077ae35232eb97d4673885acdc4acf5467d12e66db8f7feb57185c90bca3ee` |

Chrome used 226 slices, 161 budget yields, 283 Rust/Wasm bridge operations, and
679 execution-state transitions. Success, cancel, and watchdog all end in
`stopped` with zero pending request records.

The separate 180-second workload gate completed 2,182 sessions with no failure.
All 16 versioned thresholds passed. The 979,287-byte report has SHA-256
`32de48665ee3e23eee4c54a94445c45f8c6f9f6d96ad6ec9e67f068e2451a386`.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(
  heap=0x68010001, flags=0, byteLength=1)
static endpoint ID: 75
endpoint address:   0x7F0004A0
IAT address:        0x100D11D4
return address:     0x100A4A33
instruction:        1591
fault:              heap-alloc-variant-unsupported
```

The one-byte size-query result is the allocation size. The existing heap
implementation supports only policy-listed flag/size pairs, so this new
nonzero, non-`HEAP_ZERO_MEMORY` variant stops before heap or memory mutation.
The next checkpoint must trace the allocation result, initialization guarantee,
consumer, later free/lifetime, and failure behavior before extending policy.

## Non-claims

This checkpoint does not import the host locale, support arbitrary code pages
or flags, accept explicit default-character pointers, implement best-fit flags,
allocate the destination, broaden heap allocation, expose environment content,
or reach the later conversion/free calls in the cumulative runtime session.
