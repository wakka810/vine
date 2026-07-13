# BGI generic runtime re-entry

## Status

- Status: review Step 10 verified
- Date: 2026-07-13
- Frozen compatibility anchor: `bgi-seventeenth-win32-call`
- Runtime-kernel checkpoint: `bgi-generic-runtime-reentry`
- First unsupported capability: `kernel32-critical-section-synchronization`
- Follow-on checkpoint: `bgi-critical-section-synchronization` verified

## Purpose

The original BGI evidence runner reached 17 calls through the common Rust API
dispatcher, but every call-specific adapter still selected one or more v86 pages
and sent a distinct JSON request／receipt shape. This checkpoint returns the same
workload to the production runtime boundaries established by review Steps 6 and 9:

```text
BGI mapped image
  -> process-owned module graph and runtime export table
  -> v86 bounded execution mirror
  -> PTE Dirty discovery
  -> VINEWB01 atomic Rust writeback
  -> one persistent ApiDispatcher / RuntimeState / ExecutionSession
  -> VINEDL01 host delta
  -> generic register completion
```

The numbered 17-call report remains immutable evidence. The new run validates its
semantics separately, continues from the same third
`EncodePointer(0x7F020000)` stop without adding an eighteenth-call mode, and stops
at the first missing runtime capability.

## Process and import contract

Rust registers `BGI.exe` as the main executable and all 14 imported DLL identities
as runtime-system modules. The main module owns 14 static dependency edges. All
330 imports bind atomically through the process runtime export table at
`[0x7F000000, 0x7F010000)`; the 13 implemented Kernel32 identities receive API
contracts and every other owned endpoint remains fail-closed through
`endpoint-contract-missing`.

The resulting process has 15 modules, main handle `0x4D01000F`, KERNEL32 handle
`0x4D010001`, four process-lifetime dynamic FLS endpoints, a running initial
thread, and callback-free `DLL_PROCESS_ATTACH` completion. The mapped BGI image is
registered after import binding and remains the same sealed owner used by the CPU
mirror.

`ApiEndpointIdentity::from_runtime_guest_import` now derives dispatcher identity
from the committed process import binding. `ApiDispatcher::dispatch_owned` derives
the x86 argument byte count from the registered `ApiContract`; Node does not select
an ABI schema or handler. `kernel32_contract_for_endpoint` is the single lookup for
the 13 currently implemented contracts.

## Memory synchronization

At every owned trap boundary, Node scans all writable v86 mappings and selects only
PTE Dirty pages. It clears the selected bits, flushes the TLB, and sends a framed
`VINEWB01` control buffer plus contiguous 4 KiB payload. Rust validates owner and
generation, reconciles the entire batch atomically, and returns a binary receipt.

API output writes travel in the other direction as framed `VINEDL01` host deltas.
The adapter acknowledges each generation before Rust emits the generic completion.
No API handler owns a reconciliation page list, and no page byte array is carried
inside a JSON request or receipt. The initial full reflection intentionally retains
the frozen evidence protocol; this checkpoint changes the live bidirectional path,
not the immutable snapshot format.

Measured synchronization:

| Evidence | Result |
|---|---:|
| Initial reflection | 389 pages, generation 30 |
| Reflection SHA-256 | `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a` |
| CPU writeback batches | 22 |
| Dirty／changed pages | 38／38 |
| Changed bytes | 268 |
| Host delta batches | 3 |
| Final synchronized generation | 52 |

The automatically discovered pages preserve all four FLS endpoint stores, the TLS
index store, the encoded process-initializer store, and the later stack frames.

## Frozen semantic equivalence

The generic loop validates all 17 manifest entries at their original trap
instruction counts, including decoded arguments, return register policy, output
writes, post-return guest stores, and the next owned identity. It reaches the same
third `EncodePointer(0x7F020000)` trap at instruction 238.

The legacy command remains byte-identical:

- report bytes: 140,660
- report SHA-256: `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- reflection SHA-256: `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`

This separates the semantic gate from the historical evidence snapshot: internal
transport and generation changes do not require rewriting the frozen report.

## Continued execution and first unsupported cluster

The same existing `EncodePointer` contract handles four additional occurrences:

| Occurrence | Argument | Result | Trap instruction |
|---:|---:|---:|---:|
| 3 | `0x7F020000` (`FlsAlloc`) | `0xEC010003` | 238 |
| 4 | `0x7F020010` (`FlsGetValue`) | `0xEC010004` | 242 |
| 5 | `0x7F020020` (`FlsSetValue`) | `0xEC010005` | 246 |
| 6 | `0x7F020030` (`FlsFree`) | `0xEC010006` | 250 |

After the sixth return, BGI reaches runtime endpoint ID `100` at `0x7F000630`:

```text
KERNEL32.dll!InitializeCriticalSectionAndSpinCount(
    0x100FF630,
    4000
)
```

The exact stop is instruction 266, return address `0x100AB5E7`, IAT
`0x100D1238`. Rust classifies it as
`kernel32-critical-section-synchronization` and returns structured
`endpoint-contract-missing`; it does not fabricate success or mutate the guest
critical-section storage.

The deterministic generic report is 96,244 bytes with SHA-256
`a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa`.

## Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi -- --checkpoint bgi-seventeenth-win32-call
npm run check:runtime:bgi -- --profile baseline
```

The generic command was run twice and produced the same report digest. The source
BGI size and SHA-256 remain `1,193,472` bytes and
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

## Non-claims and follow-on dependency

At this historical checkpoint, Windows critical sections and FLS dispatch were
not implemented. It still does not claim arbitrary dynamic export dispatch,
general multi-thread CPU contexts, or BGI browser execution. Initial
reflection remains the private evidence adapter rather than a title-import browser
protocol.

The reached critical-section cluster is now closed by
[bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md).
That profile preserves this report, adds process-owned recursive state, completes
14 BGI initializations, and stops at `DecodePointer(0xEC010003)`. The subsequent
[bgi-pointer-decoding.md](bgi-pointer-decoding.md) profile also preserves this
report, resolves the owned token, and reaches `FlsAlloc(0x100A5666)`. The
[bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md)
profile preserves it again, returns logical FLS index `0`, and reaches
`HeapAlloc(0x68010001,8,0x214)`. Numbered
API modes and handler-specific writeback lists remain frozen.
