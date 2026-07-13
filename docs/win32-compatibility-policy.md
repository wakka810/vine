# Win32 semantics and compatibility policy

## Status

- Status: implemented controlled policy boundary
- Date: 2026-07-13
- Semantic authority: Rust `vine-core::win32`
- Policy schema: `vine.kernel32-compatibility-policy` version `1`
- Active policy: `fixtures/runtime-policies/kernel32-standard-error-file-type-v1.json`
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
  versioned selectors, file types, HeapAlloc flag/size pairs
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
- enabled `HeapAlloc` flag and byte-length pairs.

Policy IDs are bounded ASCII. Entries are bounded and duplicate-free. Selectors,
file types, heap flags, zero size, and the private-heap capacity are rejected if
the semantic implementation cannot support them. Invalid policy data stops the
probe before guest execution.

The current files form six monotonic controlled policies:

| Policy | Standard handles | File types | Heap allocations |
|---|---|---|---|
| `kernel32-controlled-v1` | stdin | stdin = character | zeroed `0x214` |
| `kernel32-heap-two-v1` | stdin | stdin = character | zeroed `0x214`, `0x800` |
| `kernel32-standard-output-v1` | stdin, stdout | stdin = character | zeroed `0x214`, `0x800` |
| `kernel32-standard-output-file-type-v1` | stdin, stdout | stdin = character, stdout = pipe | zeroed `0x214`, `0x800` |
| `kernel32-standard-error-v1` | stdin, stdout, stderr | stdin = character, stdout = pipe | zeroed `0x214`, `0x800` |
| `kernel32-standard-error-file-type-v1` | stdin, stdout, stderr | stdin = character, stdout／stderr = pipe | zeroed `0x214`, `0x800` |

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

This boundary does not implement broad `HeapAlloc`, broad file classification,
file I/O, or a general Windows console. Adding a policy entry is valid only when
the semantic handler already implements that input domain and a public semantic
fixture covers it.
