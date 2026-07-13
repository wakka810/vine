# Win32 semantics and compatibility policy

## Status

- Status: implemented controlled policy boundary
- Date: 2026-07-14
- Semantic authority: Rust `vine-core::win32`
- Policy schema: `vine.kernel32-compatibility-policy` version `1`
- Active policy: `fixtures/runtime-policies/kernel32-private-heap-size-1-v1.json`
- Evidence selection: `docs/current-checkpoint.json`

This checkpoint separates Win32 ABI and state effects from the bounded variants
currently enabled for the primary runtime session. The implementation module has
no BGI identity, caller RVA, checkpoint order, or `Reached*` branch.

```text
Win32 semantics
  one ABI contract and implementation ID per API
  argument validation, result, LastError, runtime/memory effects
                         |
                         v
Compatibility policy
  versioned selectors, file types, HeapAlloc pairs, process-view/code-page enablement
  bounded validation and mutation-free unsupported result
                         |
                         v
Evidence profile
  active capability ID, native caller/arguments/results, next stop
  selects a policy file, never a handler implementation branch
```

## Policy contract

`Kernel32CompatibilityPolicy` is deserialized and validated before generic
runtime execution. It contains only target-independent bounded data:

- enabled standard-handle selectors;
- enabled standard-handle file-type classifications;
- enabled `SetHandleCount` values;
- opt-in deterministic ANSI view of the process-parameter command line;
- opt-in controlled UTF-16 environment authority and copy lifetime;
- opt-in active ANSI code page and bounded UTF-16 source limit;
- enabled `HeapAlloc` flag and byte-length pairs.

Policy IDs are bounded ASCII. Entries are bounded and duplicate-free. Selectors,
file types, heap flags, zero size, and the private-heap capacity are rejected if
the semantic implementation cannot support them. Invalid policy data stops the
probe before guest execution.

The current files form eleven monotonic controlled policies:

| Policy | Standard handles | File types / process option | Heap allocations |
|---|---|---|---|
| `kernel32-controlled-v1` | stdin | stdin = character | zeroed `0x214` |
| `kernel32-heap-two-v1` | stdin | stdin = character | zeroed `0x214`, `0x800` |
| `kernel32-standard-output-v1` | stdin, stdout | stdin = character | zeroed `0x214`, `0x800` |
| `kernel32-standard-output-file-type-v1` | stdin, stdout | stdin = character, stdout = pipe | zeroed `0x214`, `0x800` |
| `kernel32-standard-error-v1` | stdin, stdout, stderr | stdin = character, stdout = pipe | zeroed `0x214`, `0x800` |
| `kernel32-standard-error-file-type-v1` | stdin, stdout, stderr | stdin = character, stdout／stderr = pipe | zeroed `0x214`, `0x800` |
| `kernel32-set-handle-count-v1` | stdin, stdout, stderr | stdin = character, stdout／stderr = pipe; SetHandleCount = 32 | zeroed `0x214`, `0x800` |
| `kernel32-command-line-a-v1` | stdin, stdout, stderr | preceding options + process-owned ASCII command line | zeroed `0x214`, `0x800` |
| `kernel32-environment-strings-w-v1` | stdin, stdout, stderr | preceding options + controlled wide-environment copy lifetime | zeroed `0x214`, `0x800` |
| `kernel32-wide-char-to-multi-byte-v1` | stdin, stdout, stderr | preceding options + Windows-932 ACP／65,536-unit conversion bound | zeroed `0x214`, `0x800` |
| `kernel32-private-heap-size-1-v1` | stdin, stdout, stderr | preceding options | zeroed `0x214`, `0x800`; flags-zero size `1` |

Unlisted selectors and file classifications remain unsupported. The handler
reports API identity through its endpoint and a policy-specific reason before
runtime or memory mutation.

## Semantic tests

`api_dispatcher` uses the same `kernel32.heap-alloc.v2` contract with multiple
in-memory policies. One public fixture enables `0xBC` without loading or editing
the BGI evidence profile, proving that another valid input does not create a new
handler branch. `GetStdHandle` similarly uses one
`kernel32.get-std-handle.v2` contract for the baseline, stdout, and stderr policies.
`GetFileType` uses one `kernel32.get-file-type.v2` handler for stdin, stdout,
and stderr classifications; the policy does not select an implementation branch.
`SetHandleCount` similarly uses one `kernel32.set-handle-count.v1` handler. The
reached policy authorizes `32`; historical policies reject it before mutation.
`GetCommandLineA` uses one `kernel32.get-command-line-a.v1` handler and a
policy-enabled bootstrap view; the handler derives neither a caller-specific
branch nor a host path.
`GetEnvironmentStringsW` and `FreeEnvironmentStringsW` use distinct semantic
handler IDs over one process-owned lifetime record. The policy only enables the
controlled wide block; it does not supply environment names, pointer values, or
caller-specific results.
`WideCharToMultiByte` uses one semantic handler for query and conversion. The
policy fixes `CP_ACP` to Windows-932 and a source bound; it contains no BGI
caller, observed source length, output bytes, host locale, or next-stop data.
The public fixture exercises a different valid UTF-16 input without editing the
evidence profile, while unsupported code pages／flags stop before mutation.
The one-byte heap policy uses the existing `kernel32.heap-alloc.v2` handler and
adds only `(flags=0, byteLength=1)`. Allocation initialization, alignment,
capacity, identity, and mutation rollback remain semantic implementation
behavior; native caller／pointer／consumer relations remain evidence data.
`GetLastError` uses one policy-independent semantic handler. It returns the
selected Rust runtime thread's full 32-bit LastError and preserves that state;
there is no reached value, caller, or allowlist entry in compatibility policy.
`TlsGetValue` is also policy-independent. It validates the process-owned TLS
allocation registry, reads only the selected-thread value, and distinguishes a
valid NULL from an invalid index through handler-owned LastError `0`／`87`.
Neither the reached index nor its stored endpoint value appears in policy data.
`FlsGetValue` is policy-independent as well. It validates the process slot and
selected thread's FLS-storage lifecycle, returning LastError `0` for an
initialized set／unset lane and `87` for an invalid index or a thread without FLS
storage. The reached slot, value, and dynamic endpoint address are not policy.
`SetLastError` is policy-independent. Its one semantic handler writes the full
32-bit argument only to the selected Rust runtime thread through the generic
dispatcher LastError policy. Reached value `87`, caller RVA, occurrence count,
and native EAX values remain evidence and do not select an implementation branch.
`GetACP` uses one policy-backed semantic handler. It reads the active ANSI code
page already present in the conversion policy, returns it without state or guest
memory mutation, and preserves LastError. A policy without that capability stops
before mutation. The native caller, observed value `932`, consumer, and call
position remain evidence rather than handler branches.

Policy JSON parsing and validation is covered independently:

```powershell
npm run check:win32-policy
cargo test -p vine-core --test win32_compatibility_policy
cargo test -p vine-core --test api_dispatcher
```

The current generic BGI command loads the active policy path from
`docs/current-checkpoint.json`. Its report records the exact policy object as
evidence identity. Native caller RVA, observed sizes, call ordering, and next
stop remain evidence fields only.

## Non-claims

This boundary does not implement broad `HeapAlloc`, general handle-table sizing,
broad file classification, file I/O, non-ASCII command-line conversion, host
environment import, environment mutation／ANSI environment strings, arbitrary
code-page selection or mutation／conversion flags／explicit default-character pointers, or a general
Windows console. Adding a policy entry is valid only when
the semantic handler already implements that input domain and a public semantic
fixture covers it.
