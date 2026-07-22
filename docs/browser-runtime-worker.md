# Chrome Runtime Worker vertical slice

## Status

- Status: Verified controlled fixture and private BGI semantic checkpoint
- Date: 2026-07-22
- Browser: Chrome `150.0.7871.129`
- Protocol: `vine.browser-runtime-worker` version `7`
- Runtime core: Rust/Wasm, zero imports; current artifact identity is manifest-owned
- CPU mirror: pinned source-v86 adapter in a dedicated Runtime Worker
- Controlled PE32 fixture: `browser-vertical.exe`
- Evidence: `.work/browser-runtime/browser-report.json`
- BGI evidence: `.work/browser-runtime/bgi-browser-report.json`
- BGI benchmark evidence: `.work/browser-runtime/bgi-browser-benchmark-report.json`
- Stress evidence: `.work/browser-runtime/browser-stress-report.json`

This checkpoint runs guest CPU execution inside Chrome rather than presenting a
Node-generated event. It crosses PE32 load, Rust-owned memory, v86 execution,
generic API dispatch, asynchronous host completion, nested guest callbacks,
Canvas, Web Audio, IndexedDB, cancellation, watchdog termination, and exact
process exit in one controlled browser application.

It does not claim a general Win32 process, title rendering, a production
renderer or mixer, or full-play steady performance. The fixture APIs are
checkpoint services and are not counted as implemented Windows APIs.

## Runtime path

```text
Chrome main thread
  operational UI / Canvas / AudioContext / IndexedDB / heartbeat
                         |
                         | versioned messages and host completions
                         v
Runtime Worker
  browser event delivery / watchdog / cancellation
                         |
             +-----------+-----------+
             |                       |
             v                       v
Rust/Wasm runtime core          ExecutionEngine adapter
  PE loader                       source-v86 Wasm
  GuestMemory authority           paging / dirty discovery
  RuntimeState                    CPU context / cache effects
  ApiDispatcher
```

`crates/vine-runtime-wasm` owns the immutable PE snapshot, load plan,
materialized bytes, import endpoints, initial stack, `RuntimeState`, decoded API
frames, suspended calls, request identity, scheduler policy, and callback stack.
Its raw buffer ABI keeps JSON metadata separate from `VINERF01`, `VINEDL01`,
and `VINEWB01` page payloads and has no Wasm imports. Fast ABI version 1 handles
hot acknowledgement and CPU writeback receipts as scalar exports, so those
crossings do not copy the whole bridge input or serialize／parse JSON; faults
still return the normal structured metadata. TypeScript does not become
guest byte, pointer, handle, LastError, wait, message, module, or scheduler
authority.

The Worker passes the initial reflection to `ExecutionEngine`, acknowledges
cache effects, and restores a versioned CPU context. Every CPU run has an
explicit caller budget and returns structured `budget-yield`／`trap`／`fault`／
`halt` state. At each slice boundary the engine returns address／owner／bytes for
changed pages, which the Worker atomically reconciles into Rust before
classifying or resuming the guest. The upper loop does not know how changes are
discovered.

Before guest entry, the same Worker applies seven ordered `VINEDL01` batches.
The first real private-heap allocation lazily reserves its 16 MiB backing,
commits one page, and transfers that page; the next batch is dirty-only. The
remaining batches protect read-only／read-write, decommit, recommit, and release
the same backing. Mapping operations and dirty pages are preflighted against a
cloned engine registry before any mapping or byte mutation. Protect invalidation
is applied through generic data-TLB, instruction-fetch, and generated-code
effects.

The private BGI path loads the active Kernel32 schema-v2 policy selected by the
checkpoint manifest. `HeapAlloc` accepts implemented flags `0`／
`HEAP_ZERO_MEMORY` for ordinary nonzero sizes through the 16 MiB per-heap
ceiling and the explicit exact 64 MiB process-wide large variant;
observed allocation sizes are evidence, not Worker or Rust dispatch branches.
Decommit and release both produce a code-bearing unmapped-access stop on a stale
probe. A current full reflection rebuild preserves the CPU context and proves
the receipt-loss recovery path without changing Rust acknowledgement state.

For the large variant, Rust first emits a reserve-only delta for
`[0x40000000,0x44000000)`. A later execution fault is sent to one generic Wasm
page-fault bridge, which may resolve either current-thread stack growth or a
Rust-owned large-heap page. The Worker validates resolution identity,
generation, one 4 KiB RW／NX commit, owner, and initialization bytes before
applying the delta. It records evidence metadata but does not retain a Windows
allocation or materialized-page authority. Rejected faults return no memory
delta and resume the existing structured-stop path.

## Execution engine contract

Protocol version 7 binds the production loop to `ExecutionEngine`, not to v86
exports or numeric stop reasons. The interface owns budgeted execution,
structured stops, versioned opaque CPU-context creation／save／restore, guest mapping
operations, owner-tagged changed-page batches, CPU resume／callback entry, and
data-TLB／instruction-fetch／generated-code effects. `runtime-worker.ts` contains
no PTE address, v86 private offset, `full_clear_tlb`, or v86 reason constant.

The current implementation remains the source-v86 adapter. Its non-PAE tables,
PTE Dirty scan, TLB flush, and generated-code hook are confined to
`v86-mirror.ts`. Adapter ABI version `1` exposes memory-size setup, the
instruction counter, and a 4 GiB page execute bitmap without exposing v86
private offsets to the Worker or runtime runners. The pinned profile is
[`fixtures/execution-engines/v86-browser-v1.json`](../fixtures/execution-engines/v86-browser-v1.json):
source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f` and CPU-context ABI version `1`／
640 bytes are common. Its artifact allowlist contains exact host builds: Windows
GNU is 1,857,800 bytes／SHA-256
`99a0948c222a6ac2f21984798c9e9213dec9e43b95dff6fc5c8acf1f4dc7954f`;
Linux GNU is 2,116,017 bytes／SHA-256
`21b0fb4f63e9604a66e6522839c9744a4ae17f9ead1c2e201ff2e46fccff20c4`.
Build preparation matches the actual Wasm to one allowlist entry and records
that selected identity, rather than the whole allowlist, in the build manifest.
Worker startup checks artifact bytes／SHA-256 before compilation and exported
adapter／context ABI versions and context size before guest initialization.

Rust remains the execute-permission authority. The mirror resets and rebuilds
the adapter bitmap from reflected mappings, then updates it for every
commit／protect／decommit／release operation. The interpreter validates the
opcode and each additional decode byte when it is consumed. A one-byte
instruction at the end of an executable page therefore completes, while the
next opcode on an NX page and an immediate crossing into that page stop with
reason `3`, PFEC-equivalent `0x15`, and CR2 set to the denied byte. The crossing
case restores the instruction EIP before exposing the structured stop and does
not update its destination register. This is a host-visible execution stop; it
does not yet claim general guest SEH delivery for execute faults.

Rust prepared metadata also supplies the semantic committed-page limit. The
v86 mirror starts with a 64 KiB physical pool and grows geometrically only after
a complete reflection／host-delta preflight. Growth reinstantiates the cached
adapter module and preserves CPU context、mappings、bytes、protection、PTE Dirty
state; Rust remains byte authority. The BGI limit is 16,384 guest pages
（64 MiB）and the corresponding page-directory／all-page-table／VGA-hole-inclusive
mirror ceiling is 71,442,432 bytes. Decommit／release zero and free data pages,
empty page tables are reclaimed, and recommit reuses zeroed pages. Capacity
failure is the mutation-free typed code `v86-physical-pool-exhausted`.

The verified adapter module is cached once per Worker; each runtime session
still creates a fresh Wasm instance, CPU context, mapping registry, and guest
memory. This placement removes repeated adapter fetch／hash／compile backing
storage without sharing guest state. `npm run check:execution-engine-contract:prepared`
uses the actual Wasm to fix CPU context, structured yield／trap, all five mapping
operations, changed-page owner/address, cache effects, and revision／ABI／hash
mismatch rejection. The same contract expands a live mirror beyond 2 MiB,
resumes its CPU context, verifies release／reuse, and rejects over-limit mappings
without changing the mirror.

## Execution state contract

Protocol version 7 represents execution through an explicit state machine:
`ready`, `running`, `budget-yield`, `memory-fault`, `api-trap`, `thread-switch`,
`thread-exit`, `waiting-host`, `host-completion`, `guest-callback`,
`callback-return`, `stopping`, and `stopped`. Prepared Rust metadata supplies
the owning process and registered thread bootstraps. TypeScript combines the
selected lane with Worker lifecycle state; it does not replace Rust scheduler
authority.

Every transition is checked against a closed adjacency table before state,
sequence, or last-transition metadata is changed. API traps remember whether a
synchronous completion resumes `ready` or a nested `guest-callback`. Permitted
async starts enter a table whose records contain request ID, thread ID, service,
cancel state, and `resume-ready`／`resume-callback` policy. Completion must match
both identities. Wrong-thread, duplicate, cancelled-late, unknown, and reused
request completions are rejected without completing another record.

The Worker stores one opaque CPU context per live thread. A blocking host
request saves its owner context, asks Rust for the next executable lane, and
restores only the selected context. Host completion targets the request owner
even while another lane is selected; register resume is applied to that saved
context before Rust can schedule it again. The controlled fixture proves this
with a second guest EIP／ESP that writes a marker and exits while the initial
thread remains in `WaitingHost`.

Cancellation retires every active delivery record before final reporting.
Success, cancellation, structured stop, watchdog, and fault all pass through
`stopping -> stopped`; terminal reports require zero pending request records.
Only a watchdog report may contain diagnostic context, which records the cause
thread, current request when one exists, and the last transition observed
before termination. Pending host calls and callback stacks are keyed by owner
thread; neither is represented by a global single slot.

Protocol v7 also makes diagnostic retention explicit. Every private BGI session
feeds normalized observable calls to one ordered digest. Its next-sequence and
bounded pending map reject duplicate or conflicting sequence identity without a
second completed-sequence collection. Canonical top-level fields stream directly
into reusable SHA-256 state; ASCII fragments bypass `TextEncoder`, while
non-ASCII uses the bounded UTF-8 fallback. Normal semantic gates and repeated
benchmarks select summary evidence: they retain the exact call count, algorithm,
digest, source completions, memory digest, structured stop, and teardown checks
without publishing detailed call objects. Full evidence remains an explicit
diagnostic mode. Summary evidence is rejected for other workloads.

## Fixture contract

The reproducible PE32/i386 fixture imports three symbols from
`vine_browser_fixture.dll`:

1. `fixture_deferred_add(19, 23)` suspends the guest, sends one coarse host
   request, and resumes with `42`.
2. `fixture_call_callback` enters an outer guest callback. That callback calls
   the same API for an inner callback, producing strict depth `2` and result
   `44` before both frames unwind.
3. `fixture_commit_outputs(0xff0000ff, 880, 0x5a)` suspends again. The main
   thread presents an RGBA8 test pixel, schedules an 880 Hz Web Audio pulse,
   writes byte `0x5a` to IndexedDB, reads it back, and returns success.

The fixture returns through the exact Rust-owned process-entry sentinel with
exit code `0`. The native Windows self-oracle also exits `0`.

The executable also exports a controlled second-thread entry and marker. Rust
registers a distinct thread stack and scheduler lane, while the Worker creates
an independent opaque CPU context. When the initial thread begins the first
deferred request, Rust selects the second lane. That guest code writes its
marker, returns through the thread sentinel with its own exit code, and is
retired before the initial host completion restores the first context.

Host operations never expose a Promise to the guest stack. `RuntimeState`
enters `WaitingHost`, records a request ID and resume state, and returns to the
Worker. Protocol version 7 transports the lossless decimal request ID, owning
thread ID, bounded copied scalar payload, and matching Rust `waiting-host`
authorization. TypeScript retains a bounded delivery record, not guest stack
state. A typed
completion restores the Rust scheduler state and completes the saved generic
dispatcher frame before CPU registers are resumed. The fixture retains no guest
pointer across the asynchronous boundary.

## Browser result

The clean-profile Chrome gate requires all of the following semantic evidence:

- both registered guest threads execute with distinct CPU contexts;
- the second thread runs and exits while the initial thread owns a pending host request;
- the initial context resumes and completes both deferred browser services and nested callbacks;
- changed guest pages and mapping operations reconcile into Rust authority;
- Canvas, Web Audio, and IndexedDB outputs read back correctly;
- success, cancellation, watchdog, and fault paths finish in `stopped` with no pending delivery,
  callback, or Rust host work.

The private BGI gate uses the same Worker and Rust/Wasm authority. Node generates only a semantic
oracle; Chrome must match ordered completions, caller-visible results, final guest bytes, and the
same structured stop. The reached D3D9 path now includes one Rust-owned logical device; three
logical textures covering the exact system-memory／default／dynamic-default initial band; the first
texture's level-zero surface; a typed surface HDC; one bounded graphics fill command; and their
COM／GDI lifetime relations. It also creates one Rust-owned 112-byte logical vertex buffer, exposes
its guest backing through the exact full-range Lock, consumes the guest write, and returns the backing
to read-only on Unlock. The device then consumes the exact four fixed-function `SetRenderState`
settings through Rust-owned typed state and reaches `GetRasterStatus`. The Worker observes
completions and mirrors checked guest bytes, but owns neither identity, lock state, protection,
resource content, nor render state. No texture／surface pixel or browser
renderer resource is created. A separate repeated-session benchmark evaluates the versioned latency,
heartbeat, heap, backing-storage, linear-memory, and bridge-crossing thresholds.

A benchmark threshold failure does not invalidate a passing semantic-equivalence run, but it
blocks the next primary checkpoint. Exact failure, host-load evidence, protected thresholds, and
resume condition are recorded in the current manifest; the Worker document does not duplicate
those changing measurements.

Current measurements, artifact identities, exact stop data, and report hashes are recorded only
in [current-checkpoint.json](current-checkpoint.json). Detailed private-run non-claims remain in
[bgi-browser-worker.md](bgi-browser-worker.md), and benchmark methodology remains in
[bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Reproduction

```powershell
npm run check:runtime:browser-worker
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
npm run serve:runtime:browser-worker
npm run check:runtime:browser-worker:stress
```

The first command generates the PE fixture, prepares the pinned adapter, builds
the Rust core for `wasm32-unknown-unknown`, type-checks and bundles the browser
application, then runs success, pending-cancel, and watchdog probes in a clean
Chrome profile. The serve command exposes the operational UI on loopback. The
stress command adds the 30-minute repeated lifecycle run.

## Remaining gates

- full-play peak committed pages and memory-pressure ceiling beyond the bounded 64 MiB guest limit
- general process, thread, DLL, heap, wait, message, and object lifecycle
- long-idle heap-capacity plateau and full-play growth slope
- isolated rerun of any active private-workload performance blocker
- the next unsupported BGI capability and title output path

The Step 8 synthetic matrix, background-tab probe, AudioWorklet deadline probe,
and active steady-memory measurement passed their version 1 thresholds. See
[v86-browser-benchmark.md](v86-browser-benchmark.md).
