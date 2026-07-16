# Win32 semantics and compatibility policy

## Status

- Status: implemented controlled policy boundary
- Date: 2026-07-16
- Semantic authority: Rust `vine-core::win32`／`vine-core::user32`
- Policy schemas: `vine.kernel32-compatibility-policy` version `2`（version `1` read compatibilityあり）、`vine.user32-compatibility-policy` version `1`
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
  versioned selectors, file types, bounded HeapAlloc allocator／HeapFree／HeapSize variants, process-view/code-page/current-directory/mutex enablement
  measured InitializeCriticalSection raw spin-state field
  explicit processor-feature presence
  virtual process affinity mask and deterministic tick-count read progression
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
- opt-in active ANSI code page, explicit opt-in for the `CP_ACP` selector, bounded UTF-16/multibyte source limits, allowed multibyte flags, measured code-page information fields, Windows-932 private-use byte mappings, bounded CTYPE1 information types, bounded locale mapping variants, and an independently explicit system-default LANGID;
- opt-in current-process image-path lookup with a bounded ANSI output capacity;
- opt-in process-image-directory-only `SetCurrentDirectoryA` with a bounded
  Windows-932 input scan;
- opt-in named `CreateMutexA` with null security attributes and a bounded
  Windows-932 name scan;
- enabled `HeapAlloc` flags and maximum nonzero allocation size, `HeapFree` flags, and `HeapSize` flags;
- an opt-in x86 `InitializeCriticalSection` raw spin-state field for the measured
  Windows compatibility profile;
- explicit processor-feature index and presence mappings;
- an optional nonzero virtual process affinity mask;
- an optional bounded deterministic `GetTickCount` increment per read.

Policy IDs are bounded ASCII. Entries are bounded and duplicate-free. Active
schema v2 accepts `HeapAlloc` only when the flag is `0` or `HEAP_ZERO_MEMORY`
and the requested size is in the inclusive range `1..=16 MiB`. Per-heap backing
capacity、GuestMemory quota、live-allocation count remain independent runtime
limits and can reject a policy-allowed request atomically. Zero size and other
flags remain unsupported. Invalid policy data stops the probe before guest
execution.

Twenty-seven schema-v1 files remain immutable historical reproduction policies
with exact flag／size pairs. Schema v2 removes those sizes from runtime
selection; the active profile extends the preceding bounded-heap checkpoint
with x86 virtual system information and an explicit system-default LANGID:

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
| `kernel32-create-mutex-a-v1` | stdin, stdout, stderr | preceding options + named mutexes／32 KiB input bound; unnamed disabled | preceding allocation/free/size variants |
| `kernel32-bounded-heap-v2` | stdin, stdout, stderr | same current process／NLS／mutex options + virtual process affinity mask `1` + `GetTickCount` increment `1` | flags `0`／`HEAP_ZERO_MEMORY`, nonzero sizes through 16 MiB; `HeapFree` flags `0`; `HeapSize` flags `0`／`HEAP_NO_SERIALIZE` |
| `kernel32-get-system-info-v1` | stdin, stdout, stderr | preceding schema-v2 options + bounded PE32 address／processor profile; active mask and count derive from affinity | same bounded allocation/free/size variants |
| `kernel32-get-system-default-lang-id-v1` | stdin, stdout, stderr | preceding schema-v2 options + explicit Japanese system-default LANGID `0x0411` | same bounded allocation/free/size variants |
| `kernel32-get-drive-type-a-v1` | stdin, stdout, stderr | preceding schema-v2 options + process-image drive only／32 KiB ANSI input bound | same bounded allocation/free/size variants |

Unlisted selectors and file classifications remain unsupported. The handler
reports API identity through its endpoint and a policy-specific reason before
runtime or memory mutation.

## User32 logical-display policy

`User32CompatibilityPolicy` is a distinct versioned policy because logical
display identity and geometry do not belong to Kernel32 process policy. Its
controlled version enables one profile, `single-monitor-1920x1080-v1`, while
`runtime/displays.rs::RuntimeDisplays` remains the authoritative owner of the
configured monitor. The policy neither stores native HMONITOR values nor reads
DOM／browser screen topology.

The current `EnumDisplayMonitors` variant accepts only NULL HDC and NULL clip,
invokes one executable stdcall callback with logical handle namespace
`vine.logical-monitor.v1`, NULL monitor DC, a Rust-owned RECT scratch pointer,
and the caller's unchanged LPARAM. Callback zero maps to outer BOOL FALSE and
nonzero maps to TRUE. LastError is preserved. Non-NULL variants, missing or
mismatched display authority, invalid callback targets, and callback-frame
capacity failures stop before observable mutation.

Display configuration is configure-once plan／commit. Callback scratch and
arguments are preflighted as one contiguous x86 stack band; Rust callback-frame
identity is committed before execution and retired only by the exact top frame.
Node and Worker keep outer call-entry order while nested API traps execute.
Detailed native, core, Node, Chrome, and performance evidence is in
[bgi-enum-display-monitors.md](bgi-enum-display-monitors.md).

## Semantic tests

`api_dispatcher` uses the same `kernel32.heap-alloc.v2` contract with historical
exact policies and the active bounded allocator. A non-listed `0xBC` allocation
succeeds through the range policy without loading or editing BGI evidence;
`0x01000001` and an unsupported flag fail before runtime／memory mutation. This
proves that valid sizes do not create handler or policy-entry branches.
`GetStdHandle` similarly uses one
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
The historical one-byte and 544-byte v1 policies retain their exact checkpoint
behavior. They are accepted only for old reproduction commands and are not the
active allocator authority. Allocation initialization, alignment, capacity,
identity, and mutation rollback remain semantic implementation behavior;
native caller／pointer／consumer relations remain evidence data.
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
`GetSystemDefaultLangID` reads a separate optional WORD field. Validation
requires the bounded Japanese locale layer and value `0x0411`, but the default
identity is not derived from `LCMapStringW.allowed_locales`. The zero-argument
handler preserves LastError, writes no memory, imports no host locale, and
fails before mutation when the field is disabled. Native upper EAX bits are
unspecified; the typed U16 completion zero-extends the policy value. Detailed
evidence is in
[bgi-get-system-default-lang-id.md](bgi-get-system-default-lang-id.md).
`GetDriveTypeA` has a separate optional enablement record containing only the
process-image-drive subset and ANSI scan bound. Drive letter and type are not
policy data: `RuntimeProcessDirectories` derives them from its existing image
and current-directory identities. NULL, exact root, non-root, relative,
unterminated, and disabled-policy behavior is covered without importing native
or browser topology. Detailed evidence is in
[bgi-get-drive-type-a.md](bgi-get-drive-type-a.md).
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

`CreateMutexA` uses one policy-backed three-argument semantic handler. The
policy enables named objects, disables unnamed objects, and bounds the ANSI name
scan to 32 KiB; it contains no BGI name, guest pointer, caller, result handle,
call order, or next stop. The handler accepts only null security attributes,
copies and decodes the Windows-932 name before planning state, and returns a
logical typed handle with LastError `0` for a new object or `183` for an existing
name. Public fixtures use an independent mutex name and cover caller-buffer
ownership, distinct handles sharing one object, initial ownership, ABI cleanup,
policy-disabled／security-attribute／unnamed／unterminated mutation-free rejection,
runtime wait／release, last-close recreation, and process teardown without
loading BGI evidence.

`SetThreadAffinityMask` uses one two-argument semantic handler. Policy supplies
only a nonzero virtual process mask; it contains no native process／system mask,
host CPU count, thread handle, caller, or occurrence. The handler accepts the
reached current-thread pseudo-handle, returns the previous logical mask, and
preserves LastError. Public tests use a wider synthetic mask to verify
per-thread selection and restore, plus zero／outside-mask and handle failures.

`GetTickCount` retains one zero-argument semantic handler. When the optional
policy increment is present, the process owner returns the current DWORD and
advances its stored sample by the bounded increment through plan／commit.
Historical policies omit the option and reproduce the fixed sample. Neither
path reads host uptime or wall time; the policy does not encode the BGI loop
count, duration, caller, or stop identity.

`GetSystemInfo` uses one VOID one-argument semantic handler and a checked
36-byte x86 `SYSTEM_INFO` output. Policy supplies the bounded PE32 application
address profile and processor fields. The reserved word is always zero;
`dwActiveProcessorMask` is the existing thread-affinity process mask and
`dwNumberOfProcessors` is its popcount, so topology is not duplicated across
policy fields. Schema-v1 policies and the preceding schema-v2 policy omit the
option and stop before mutation. The handler reads no host CPU or browser
topology. Public tests use a synthetic multi-bit mask and distinct processor
level／revision, then cover NULL, read-only, and cross-page output rejection
without partial writes.

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

This boundary does not implement zero-byte `HeapAlloc`, `HEAP_NO_SERIALIZE`／
`HEAP_GENERATE_EXCEPTIONS`, allocation-failure return／exception semantics,
`HeapReAlloc`／`HeapDestroy`, general handle-table sizing,
broad file classification, file I/O, non-ASCII command-line conversion, host
environment import, environment mutation／ANSI environment strings, arbitrary
code-page selection or mutation／other conversion flags／explicit default-character pointers,
negative-length or broad `GetStringTypeW`, `CT_CTYPE2`／`CT_CTYPE3`,
negative-length／arbitrary-locale／combined-flag `LCMapStringW`, explicit-module
file-name lookup／host process-path import, broad locale
classification, other interlocked APIs／cross-Worker atomicity, or a general Windows console.
The processor-feature mapping is bounded to explicit policy entries; it is not
general CPU discovery or browser-host feature reflection.
The affinity mask is virtual compatibility data and does not control host
scheduling or expose host topology. Deterministic per-read tick progression is
not a general timer, wait-deadline, sleep, or real-time clock implementation.
The system-information profile is the bounded PE32 view required by the active
runtime; it does not expose host CPU count, NUMA groups, x64 architecture,
process-affinity APIs, or dynamic topology changes.
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
The drive-type policy exposes only one derived fixed process-image drive. It
does not implement removable／remote／optical variants, UNC or volume identity,
per-drive current directories, mount resolution, or host/browser enumeration.
The mutex policy does not implement security descriptors, inheritance, unnamed
objects, `OpenMutex`, access masks, abandonment, cross-process state,
cross-thread blocking／wake, or guest wait／release／close API coverage.
Adding a policy entry is valid only when
the semantic handler already implements that input domain and a public semantic
fixture covers it.
