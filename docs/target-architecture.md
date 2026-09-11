# 目標アーキテクチャ

## System boundary

VineはPE32 Windows userspaceをRust／Wasm coreで実行し、browser resourceへ明示的なadapterを通して接続する。ゲーム固有VMやasset decoderを置き換えず、元のexecutable、DLL、assetをそのまま実行する。

```text
immutable source -> PE/module loader -> GuestMemory <-> x86 execution mirror
                                      |
                                      v
                    Rust Windows userspace runtime
                 process/thread/handles/COM/VFS/GDI/D3D/audio
                                      |
                         versioned Worker boundary
                                      |
                   Chrome adapters and local storage service
              Canvas/WebGL, Web Audio, input, OPFS/SQLite
```

## Authority model

### Rust／Wasm core

Rust owns all guest-visible Windows state:

- PE images、guest address allocation、memory bytes／mapping／protection
- ABI decoding、API／COM dispatch、LastError、callback frames
- process、thread、scheduler、waits、timers、message queues
- objects、handles、COM identity／interfaces／reference count
- logical heap、VFS、registry、graphics、audio、input state

`RuntimeKernel` is the aggregate root. It coordinates private subsystem owners but does not duplicate their collections, identity allocators, indexes, or lifecycle invariants. Compatibility serialization is built from immutable snapshots and never becomes another authority.

### Browser／TypeScript

TypeScript owns only browser resources and adapter lifecycle:

- Worker lifecycle、DOM、Canvas／OffscreenCanvas、WebGL／WebGPU
- Web Audio nodes and transport buffers
- browser keyboard／pointer source events
- File System Access、IndexedDB／OPFS、permission、quota
- browser resource IDs and presentation receipts

It does not own Win32 handles、guest pointers、LastError、message queues、COM refcounts、thread state、logical file cursors. Browser resource identity is separate from Rust object identity; bindings are explicit and normally non-owning.

## Guest memory and CPU

`GuestMemory` is authoritative for bytes、mapping、owner、protection、generation. v86 paging is a replaceable execution mirror. CPU synchronization uses bounded slices、checked binary transport、dirty discovery、and atomic reconciliation.

Performance execution uses one page-aligned arena owned by `GuestMemory` in the same Wasm
instance as the CPU. Its reserved prefix holds adapter page tables; Rust alone allocates
and releases guest frames. The CPU borrows the arena exclusively for each run, releases
that borrow before Windows semantics execute, and cannot replace its runtime while the
backing is pinned. Normal/debug retain the separate copying mirror.

Performance JIT also lowers proven single-page counted x86 loops through a bounded
typed IR. It retains used MMX values in Wasm locals and validates each stream's TLB
permissions once per page-bounded chunk. Invariant tables may span two proven
contiguous pages. Chunks cannot consume the final iteration or exceed instruction
headroom; the ordinary JIT handles terminal branches, faults, code-page writes and
unsupported forms. Register roles and access ranges come from decoded operations
and forward-path range analysis, never title addresses or byte signatures. Exit
flags, MMX metadata and instruction accounting remain compatible with v86.

Direct CPU stores already modify authoritative bytes. At stops, dirty-page descriptors
are checked against the current generation, mapping, write permission and exact backing
pointer before acknowledging the observed writes. No page payload is copied and no exact
changed-byte count is claimed; the existing writeback-page metric counts observed pages.
Host deltas still validate their complete operation sequence and cache effects, but the
direct adapter installs only the final committed mappings. Released arena bytes remain
available for rollback; a before-image is copied only if the transaction reuses that frame.
Protection-only deltas retain a page registration when its checked owner, allocation and
backing match. Rust reuses free frames in address order to preserve contiguous browser
borrows where possible; live arena frames never move, and fragmented ranges still copy.

All address arithmetic is checked. Guest address、host offset、size、allocation identity、Win32 handle、COM pointer、browser ID are distinct concepts. API-specific direct writes into the execution mirror are not allowed.

Memory changes follow validate／plan／commit:

1. decode and validate all pointers、ranges、owners、capacity、lifetime
2. prepare a mutation-free owner plan
3. preflight cross-owner and guest-memory effects
4. commit targeted changes and publish one coherent generation
5. on failure, expose no partial allocator、mapping、queue、refcount、or byte mutation

## Process, threads, and synchronous browser gaps

Rust owns process／thread registries、CPU contexts、scheduler states、wait queues、timer clocks、callbacks、and teardown. The Worker runs bounded CPU slices and services host requests without making async browser state look synchronously successful.

Host requests carry request、thread、generation、lifetime identities. Completion validates all identities before state advances. Cancel、timeout、freeze／resume、stale completion、thread exit all retire pending work deterministically.

## PE, ABI, and compatibility

The loader separates parse、map、relocate、bind、initialize、unload. System DLL imports bind to runtime-owned endpoints; bundled native DLLs use the same loader graph. Unsupported endpoints remain explicit traps.

Compatibility has four layers:

1. ABI catalog: module、symbol／ordinal、calling convention、argument and pointer contract
2. semantic handler: reusable Windows behavior and state transition
3. reached policy: bounded target-required variant or virtual environment value
4. diagnostics: reached endpoint and caller observation

EIP、callsite、title hash、or occurrence alone never select semantics. A new reached API normally changes only the catalog、owner handler、policy when needed、and focused tests.

## Storage and source safety

The VFS presents immutable source objects and a separate writable overlay. Executable、DLL、and asset reads can never resolve to writable save objects. Save／settings／registry writes are scoped by title fingerprint and profile identity.

Browser persistence uses an explicit service boundary and generation-based transactions. Commit validates title、profile、lease、base generation、quota、path、and content before publication. Restart/load reopens committed overlay state; development snapshots are unrelated `.work/` caches and are never production persistence.

## Window, message, and input

Rust owns logical windows、classes、styles、geometry、focus／foreground／capture、cursor state、message queues、timers、and DirectInput devices. Browser events are normalized source observations. Rust validates the target window／thread and commits Win32 input and message state.

DispatchMessage callback frames are Rust-owned guest frames. TypeScript transfers source events and renders snapshots, but cannot consume or synthesize Windows queue state independently.

## Graphics

Graphics is split into:

- frontend semantics: GDI、GDI+、bounded D3D9 calls and COM lifetime
- logical resources: surfaces、textures、buffers、device state、draw submissions、presented frames
- browser backend: validated snapshot upload and Canvas／WebGL publication

Rust commits a complete logical Present before browser publication. The renderer validates dimensions、format、resource identity、capacity、and backing range before creating or updating browser resources. Device loss／Reset and broader formats are implemented only when reached.

## Audio

Audio follows the same split:

- DirectSound／WinMM frontend semantics and COM identity in Rust
- logical buffers、cursor、loop、volume、clock、play state in Rust
- Web Audio transport and browser resource lifecycle in TypeScript

Browser timing cannot silently become the guest clock authority. Transport underrun、suspend、permission、cancel、and stale resource completion are explicit diagnostics.

## Development and evidence boundary

Forward development mode uses release Chrome, current support band, bounded semantic tail, and a small `.work/` report. It ignores historical capability checkpoint completion and stops at the next real unsupported／fault／milestone.

Milestone acceptance may enable full semantic evidence and Node／Chrome comparison once per user-visible boundary. Full report identity is artifact metadata, not runtime semantics. Historical per-API profiles are frozen and are not part of the normal architecture path.
