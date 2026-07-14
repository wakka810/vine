# Win32 semantics and compatibility policy

## Status

- Status: implemented controlled policy boundary
- Date: 2026-07-14
- Semantic authority: Rust `vine-core::win32`
- Policy schema: `vine.kernel32-compatibility-policy` version `1`
- Active policy: `fixtures/runtime-policies/kernel32-set-current-directory-a-v1.json`
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
  versioned selectors, file types, HeapAlloc/HeapFree/HeapSize variants, process-view/code-page/current-directory enablement
  measured InitializeCriticalSection raw spin-state field
  explicit processor-feature presence
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
- opt-in active ANSI code page, explicit opt-in for the `CP_ACP` selector, bounded UTF-16/multibyte source limits, allowed multibyte flags, measured code-page information fields, Windows-932 private-use byte mappings, bounded CTYPE1 information types, and bounded locale mapping variants;
- opt-in current-process image-path lookup with a bounded ANSI output capacity;
- opt-in process-image-directory-only `SetCurrentDirectoryA` with a bounded
  Windows-932 input scan;
- enabled `HeapAlloc` flag and byte-length pairs, `HeapFree` flags, and `HeapSize` flags;
- an opt-in x86 `InitializeCriticalSection` raw spin-state field for the measured
  Windows compatibility profile;
- explicit processor-feature index and presence mappings.

Policy IDs are bounded ASCII. Entries are bounded and duplicate-free. Selectors,
file types, heap flags, zero size, and the private-heap capacity are rejected if
the semantic implementation cannot support them. Invalid policy data stops the
probe before guest execution.

The current files form twenty-six monotonic controlled policies:

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
| `kernel32-private-heap-size-544-v1` | stdin, stdout, stderr | preceding options | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-get-cp-info-v1` | stdin, stdout, stderr | preceding options + active Windows-932 CPINFO fields | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-multi-byte-to-wide-char-v1` | stdin, stdout, stderr | preceding options + explicit Windows-932 flags `1`／65,536-byte decode bound | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-get-string-type-w-v1` | stdin, stdout, stderr | preceding options + `CT_CTYPE1`／65,536-unit classification bound | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-lc-map-string-w-v1` | stdin, stdout, stderr | preceding options + locale `1041`／lowercase and uppercase／65,536-unit mapping bound | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-get-module-file-name-a-v1` | stdin, stdout, stderr | preceding options + main-module-only path lookup／32 KiB capacity | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `544` |
| `kernel32-private-heap-size-16-v1` | stdin, stdout, stderr | preceding options | zeroed `0x214`, `0x800`; flags-zero sizes `1`, `16`, `544` |
| `kernel32-private-heap-size-4-v1` | stdin, stdout, stderr | preceding options | zeroed `4`, `0x214`, `0x800`; flags-zero sizes `1`, `16`, `544` |
| `kernel32-private-heap-free-size-1-v1` | stdin, stdout, stderr | preceding options | preceding allocation pairs; `HeapFree` flags `0` |
| `kernel32-private-heap-size-128-v1` | stdin, stdout, stderr | preceding options | zeroed `4`, `128`, `0x214`, `0x800`; flags-zero sizes `1`, `16`, `544`; `HeapFree` flags `0` |
| `kernel32-is-processor-feature-present-v1` | stdin, stdout, stderr | preceding options + processor feature `10` present | preceding allocation/free variants |
| `kernel32-heap-size-v1` | stdin, stdout, stderr | preceding options + `HeapSize` flags `0`／`HEAP_NO_SERIALIZE` | preceding allocation/free variants |
| `kernel32-initialize-critical-section-v1` | stdin, stdout, stderr | preceding options + measured default x86 critical-section spin-state field | preceding allocation/free/size variants |
| `kernel32-multi-byte-to-wide-char-cp-acp-v1` | stdin, stdout, stderr | preceding options + `CP_ACP` selector／flags `0,1`／65,536-byte decode bound | preceding allocation/free/size variants |
| `kernel32-set-current-directory-a-v1` | stdin, stdout, stderr | preceding options + registered process-image directory only／32 KiB input bound | preceding allocation/free/size variants |

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
The 544-byte policy extends the same flags-zero entry with only the reached
`544` length. The semantic handler retains deterministic-uninitialized fill,
alignment, allocation identity, capacity, and transactional rollback; native
buffer fingerprints and the BGI copy consumer remain evidence data.
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
`GetCPInfo` uses one policy-backed semantic handler and a checked 18-byte output
contract. The active policy supplies character width, default bytes, and lead-byte
ranges independently of the native caller and pointer. Only the explicit active
page is enabled; other pages, disabled metadata, and invalid output ranges stop
before mutation. Native guard measurements and the BGI classification-table
consumer remain evidence rather than implementation branches.
`MultiByteToWideChar` uses one policy-backed six-argument semantic handler for
both query and conversion. Historical policy enables explicit Windows-932 and
flags `1`; the active policy additionally opts into resolving `CP_ACP` to the
policy-owned page and flags `0`. Both retain a broad source bound and contain no
reached source length, pointer, path, caller, output table, or next stop. Source
count `-1` includes its checked trailing NUL in output and return length. The
semantic decoder combines checked lead/trail pairs with native-observed standalone
`0xA0`／`0xFD..0xFF` private-use mappings. The public fixture independently covers
the explicit 932 regression, `CP_ACP` query/conversion, a Japanese pair, the
complete native 256-unit table, insufficient-buffer prefix write, invalid shape,
LastError, and mutation-free unsupported selectors/flags.
`GetStringTypeW` uses one policy-backed four-argument semantic handler. The
policy enables information type `CT_CTYPE1` and a broad positive source bound;
it does not contain the reached 256-unit source, output words, pointers, caller,
or next stop. The host-locale-independent classifier covers only the measured
code-unit domain. The public fixture independently fixes representative ASCII,
control, whitespace, Japanese/CJK, fullwidth/halfwidth, and Windows-932
private-use words, zero-count failure, LastError, ABI cleanup, and
mutation-free unsupported type／negative count／overlap／code-unit paths.
`LCMapStringW` uses one policy-backed six-argument semantic handler for both
query and conversion. The policy enables locale `1041`, exact lowercase and
uppercase flags, and a broad positive source bound; it does not contain the
reached pointers, 256-unit source, output tables, callers, or next stop. The
public fixture independently covers lower／upper query and output, Japanese and
fullwidth units, insufficient capacity, invalid zero／NULL shapes, LastError,
ABI cleanup, and mutation-free unsupported locale／flags／negative count／overlap／
code-unit paths.
`InterlockedDecrement` is policy-independent. Its one-argument semantic handler
requires a naturally aligned writable LONG, performs checked wrapping decrement,
returns the new signed bits, and preserves LastError. The reached pointer,
initial values, caller RVAs, order, and next stop remain evidence only. The
public fixture independently covers the two reached transitions, `INT_MIN`
wrapping, ABI cleanup, exact write, and mutation-free NULL／unaligned／unmapped／
read-only failures.
`GetModuleFileNameA` uses one policy-backed three-argument semantic handler. The
policy enables only the current process image and a 32 KiB output bound; the
Rust bootstrap owns the Windows-932 path bytes. Native installation directory,
reached destination, caller RVAs, and call order remain evidence only. The
public fixture independently covers generous and exact success, modern
truncation, one-byte and zero capacity, NULL output, ABI cleanup, LastError,
full-span writable preflight, policy limit, explicit-module rejection, and
mutation-free faults.
The 16-byte path-buffer policy reuses `kernel32.heap-alloc.v2` and adds only
`(flags=0, byteLength=16)`. The native installation path produces a different
36-byte request; neither that host-derived value nor native uninitialized bytes
enter policy. The public heap fixture fixes preceding-policy rejection,
deterministic initialization, allocation identity／alignment, dirty-only delta,
LastError, and stdcall cleanup through the same handler.
The four-byte environment-pointer-table policy adds only
`(flags=HEAP_ZERO_MEMORY, byteLength=4)`. Native requests 188 bytes for 46 host
environment entries plus a terminator, while the controlled empty environment
requests one four-byte terminator slot. Host entries and the native size do not
enter policy. The public heap fixture fixes preceding-policy rejection, full
reserved-range zero initialization, allocation identity／alignment, dirty-only
delta, LastError, and stdcall cleanup through the same handler.

The one-byte-free policy then adds only `heapFreeFlags: [0]`. Allocation
address, ID, call order, and caller RVA remain evidence data rather than policy.
`kernel32.heap-free.v1` validates the logical heap and exact live allocation;
unsupported flags stop before heap metadata or guest bytes change. NULL,
interior, foreign-heap, and stale pointers are rejected by ABI/runtime semantic
validation rather than added as reached compatibility variants.

The 128-byte fixed-table policy adds only
`(flags=HEAP_ZERO_MEMORY, byteLength=128)`. Native requests the same size from
a checked `32 * 4` source and happens to reuse a just-freed larger host block;
the native pointer/reuse relation does not enter policy. The public heap fixture
fixes preceding-policy rejection, allocation ID and address, exact requested
and reserved lengths, full zero initialization, dirty-only delta, LastError,
and stdcall cleanup through the unchanged `kernel32.heap-alloc.v2` handler.

The processor-feature policy adds only `{index: 10, present: true}`.
`kernel32.is-processor-feature-present.v1` returns the policy-owned BOOL through
one DWORD stdcall contract, preserves LastError, and performs no guest-memory or
runtime-state mutation. The native callers, occurrence count, consumer stores,
and host CPU capabilities remain evidence rather than policy. The public fixture
also exercises an explicit false mapping and rejects an unregistered index
before mutation, proving that neither the handler nor the policy imports host CPU
feature discovery.

`SetUnhandledExceptionFilter` is policy-independent. Its one semantic handler
accepts a nullable guest pointer, preflights every non-NULL replacement for
execute access before mutation, atomically replaces the Rust process-owned
filter, returns the previous pointer, and preserves LastError. The reached filter
RVA, initial NULL value, caller, replacement order, and next stop remain evidence
only. The handler never registers the guest pointer with host SEH or JavaScript
exception machinery. Public semantics independently cover replacement, removal,
unmapped-pointer rejection without mutation, stdcall cleanup, and process-exit
retirement; no new compatibility-policy selector is introduced.

The heap-size policy adds only flags 0 and `HEAP_NO_SERIALIZE`.
`kernel32.heap-size.v1` reads the exact live allocation record from the Rust
process-owned heap backing and returns its requested byte length rather than the
aligned reservation or committed capacity. NULL, interior, foreign-heap, and
freed pointers return `SIZE_T_MAX` with LastError 87; unknown logical heaps and
unlisted flags stop before mutation. The native pointer, requested 128-byte
sample, caller, call count, and consumer remain evidence rather than policy.
The public fixture independently queries a 129-byte allocation reserved as 136
bytes and covers both flags, all invalid pointer relations, LastError, ABI
cleanup, and unchanged runtime／memory state.

The InitializeCriticalSection policy adds one raw x86 spin-state field. The
one-argument Win32 semantic handler remains a single VOID stdcall contract: it
validates a 24-byte output, preserves EAX and LastError, and creates state in the
process-owned critical-section subsystem. A missing policy stops before
runtime／memory mutation. The active value `0x020007D0` is the measured Windows
11 private structure state, not a portable definition of zero spin and not a
BGI callsite selector. Policy validation rejects unknown high-byte state flags.
The public semantic fixture uses distinct value `0x02001234` and independently
checks exact bytes, ABI, policy-disabled atomic failure, ownership, and duplicate
initialization.

`SetCurrentDirectoryA` uses one policy-backed one-argument semantic handler.
The policy enables only the process image directory already registered by Rust
bootstrap and a 32 KiB ANSI scan bound; it contains no BGI pointer, native path,
caller, result, call order, or next stop. The handler performs checked
Windows-932 read and path normalization, plans the process-owned identity change,
preflights process lifecycle, and commits once. Success preserves LastError;
null and empty paths return the Windows failure codes independently of policy.
Valid but unregistered absolute／relative paths and bounded resource failures
stop before mutation. Public fixtures cover alternate case／separator spelling,
DBCS trail `0x5C`, caller-buffer ownership, generation, ABI cleanup, stale plan,
lifecycle conflict, and mutation-free rejection without loading BGI evidence.

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
code-page selection or mutation／other conversion flags／explicit default-character pointers,
negative-length or broad `GetStringTypeW`, `CT_CTYPE2`／`CT_CTYPE3`,
negative-length／arbitrary-locale／combined-flag `LCMapStringW`, explicit-module
file-name lookup／host process-path import, broad locale
classification, other interlocked APIs／cross-Worker atomicity, or a general Windows console.
The processor-feature mapping is bounded to explicit policy entries; it is not
general CPU discovery or browser-host feature reflection.
The policy-independent top-level filter record does not implement SEH dispatch,
filter invocation, exception records／contexts, vectored handlers, or host
exception bridging.
The heap-size policy does not enable host/process heap introspection, arbitrary
flags, heap walking／validation, or `HeapReAlloc`.
The InitializeCriticalSection policy does not implement arbitrary private raw
state layouts, contention, deletion, wait scheduling, or cross-thread blocking.
The current-directory policy does not implement general relative resolution,
filesystem existence, directory enumeration, file identity, host path import,
mounts, or a browser VFS backend.
Adding a policy entry is valid only when
the semantic handler already implements that input domain and a public semantic
fixture covers it.
