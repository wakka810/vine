# Runtime private-heap backing foundation

## Purpose

review Step 9のseventh foundationとして、既存のtyped logical private heapをRust-owned
`GuestMemory`へ接続した。Allocation identity、x86 alignment、zero／nonzero initialization、
free／reuse、invalid／double free、heap destroy、process-exit teardownを一つのauthoritative
boundaryで固定する。

Architecture refactor checkpoint S5では、logical heap metadataとGuestMemory backing metadataを
private `runtime/heaps.rs::RuntimeHeaps`へ移し、同じsubsystem ownerの下へ統合した。
`RuntimeState`はobject table、process address-space allocation、GuestMemory transactionとの
cross-subsystem coordinationだけを行い、heap／backing collectionを直接mutateしない。

このfoundation自体はguest dispatchを追加せず、`HeapCreate(0, 0x1000, 0)`をmetadata-onlyの
ままreturnする境界として成立した。続く
[bgi-private-heap-allocation.md](bgi-private-heap-allocation.md)は、reached
`HeapAlloc(0x68010001,HEAP_ZERO_MEMORY,0x214)`だけをsame transactionへ接続した。
Later [bgi-private-heap-free-size-1.md](bgi-private-heap-free-size-1.md) connects
the reached exact flags-zero `HeapFree` to this same transaction. Guest
[`bgi-private-heap-size-128.md`](bgi-private-heap-size-128.md) then proves a
larger zeroed dirty-only allocation without importing native address reuse.
`HeapDestroy`／`HeapReAlloc` and broad allocator／free policy remain unimplemented.

## Runtime subsystem ownership

`RuntimeProcessState`はprivate `RuntimeHeaps`を`serde(flatten)`で保持する。既存reportの
`process.heaps` shapeは維持し、public inspectionは`RuntimeProcessState::heaps()`のimmutable borrowに
限定する。GuestMemory backing mapはserializeせず、logical heap mapと同じownerだけが更新する。

Heap creation、allocation、free、destructionはsubsystem内でmutation-free planを作り、aggregateが
process／thread lifecycle、typed object、address allocation、GuestMemory transactionをpreflightした後、
targeted commitする。Creationはobject-table registrationが成功してからlogical heapをcommitする。
Automatic handle allocationは候補slotをlocal cursorで探索し、heapとobjectのregistrationが成功した時だけ
`next_private_heap_slot`を進める。したがってobject-table conflictはslot allocatorを消費しない。

## Controlled address and memory policy

Version 1 foundationはguest band `[0x20000000, 0x40000000)`を32個の16 MiB slotへ分ける。
Logical heap handleの16-bit slotをprocess address-space allocatorの同じheap-slot allocation recordへ
対応させるため、first heap
`0x68010001`は`0x20000000`、second heap `0x68010002`は`0x21000000`から始まる。

- growable heapのcontrolled capacityは16 MiB／heap
- fixed maximumは4 KiBへround upし、16 MiBを超える要求をstructured resource limitにする
- initial sizeは4 KiBへround upし、first allocation時に必要なtailまでcommitする
- ownerは`MemoryOwnerKind::Anonymous`、regionは`MemoryRegionKind::Heap`
- committed pagesはRW／NXであり、`GuestMemory`だけがbytes／mapping authorityを持つ
- live allocation上限は65,536／heap、backing上限は32 heaps

Version 1 heap-slot recordsはpreferred固定layoutである。Backingはallocation IDとrecord rangeを保持し、
handleからbaseを直接再計算しない。Image、stack、runtime arena等と衝突した場合、別addressへ黙って
移動せず`memory-region-overlap`で停止する。Owner creationを含むmemory mutationは
`GuestMemoryTransaction`でrollbackされ、logical heap metadata、generation、owner／region countを
変えない。同じheapでcollisionを除去した後にretryできる。

このbandとcapacityはcontrolled foundation policyであり、full-title memory profileの最終値ではない。
General module placementとheap workloadを測定した後、process address-space policyとして再評価する。

## Allocation, initialization, and reuse

`ExecutionSession`が`GuestMemory`と`RuntimeState`を同時にborrowし、次を一 operationとしてcommitする。

1. target heap、process／thread lifecycle、capacity、allocation countをpreflightする。
2. 8-byte aligned blockをlowest-address first-fit free block、なければhigh-water tailへplanする。
3. 必要なpage tailをmemory transaction内でcommitする。
4. aligned block全体をselected policyでinitializeする。
5. memory operationがすべて成功した後、target heap metadataだけをcommitする。

Allocationはprocess-local monotonic `RuntimeHeapAllocationId`、guest address、requested length、aligned
reserved length、initialization policyを持つ。Fresh／reused pointerが同じでもIDは再利用しない。
Live blocksとcoalesced free blocksは`[0, high_water_mark)`をgap／overlapなしにpartitionする。

Initialization policy:

| Policy | Aligned block bytes |
|---|---:|
| `Zeroed` | `0x00` |
| `DeterministicUninitialized` | `0xA5` |
| freed block poison | `0xDD` |

`0xA5`／`0xDD`はVineのdeterministic diagnostic policyであり、native Windowsが同じbytesを返すという
主張ではない。Reached `HEAP_ZERO_MEMORY` handlerは`Zeroed`へmapする。Flagなしを含む
他variantとzero-byte allocation semanticsはbroad allocator policyまでexplicit unsupportedである。

Freeはexact live baseを要求する。同じfree-block baseのsecond freeは
`heap-allocation-double-free`、interior／foreign／unknown pointerは
`heap-allocation-pointer-invalid`となり、metadata／bytesを変えない。Free blockはadjacent rangesを
coalesceし、次のfirst-fit allocationがlowest addressをreuseする。

## Heap and process teardown

`destroy_private_heap`はlive allocationsを含むwhole backing regionをreleaseし、memory owner、logical
heap payload、typed subsystem-owned object／handleを同じoperationでretireする。Old handleはobject-table
generation policyにより`runtime-handle-stale`となる。

Process-wide cleanupはexisting lifecycleのrequest／completion boundaryを使う。

```text
running
  -> request_current_process_exit
  -> ExecutionSession::teardown_process_heaps
  -> complete_current_process_exit
  -> exited
```

Teardownはbacked heapとmetadata-only heapをhandle orderでretireする。Live logical heapsが残る場合、
exit completionは`process-heaps-live-at-exit-completion`、one-shot direct terminationは
`process-heaps-live-at-direct-termination`で停止する。これによりexternal `GuestMemory`を見ずに
`RuntimeState`だけを破棄してmappingをstrandする経路を作らない。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --lib runtime::heaps::tests --locked
cargo test -p vine-core --test runtime_heap_backing --locked
```

The controlled sequence fixes:

- mutation-free creation／allocation plans, co-owned logical／backing commit, and stable
  `process.heaps` serialization
- object-table registration failure with unchanged heap collections and private-heap slot allocator
- first allocation ID `1`: `0x20000000`, requested `0x214`, aligned `0x218`, zeroed
- second allocation ID `2`: `0x20000218`, requested／aligned `0x800`, `0xA5`
- free poison `0xDD`, exact double-free rejection, interior-pointer rejection
- reused allocation ID `3`: `0x20000000`, requested／aligned `0x100`, zeroed
- explicit destroy: two live allocations discarded, 16 MiB reserved／4 KiB committed released
- process teardown: one backed and one metadata-only heap, final memory generation／ack `13`

S5 regression on the same implementation:

- Node BGI oracle: 284 calls／25,256 instructions、next `GetCurrentThread`; semantic report
  SHA-256 `0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`
- Chrome Worker: Node-equivalent memory／next stop、cancel／watchdog clean、browser report
  SHA-256 `d47056b546b35caac768035928e980caad799957345291cd4d5cf3fe33586a84`
- 180-second BGI workload: 624 sessions、failure `0`、all 17 thresholds passed; report
  SHA-256 `297afb5f8ba3073b347619529d868b5dd9645262fbdbde4a654c5f44cf41ae15`
- Rust runtime core Wasm: 2,087,633 bytes／SHA-256
  `cce4e68d59599631729fbf211c16aa94338bf3845f770a94532baf2365633f3c`

The clean commit `a0d99e5` report remains the engine decision authority; current benchmark numbers are
regression evidence, not a replacement baseline.

## Non-claims and next dependency

The reached guest policy currently covers seven measured nonzero allocation
flag-size pairs and one exact flags-zero free. It does not implement other
allocation/free variants, failure return, zero-size behavior, realloc,
size／validate／compact／walk, heap lock, LFH,
serialization, concurrent allocation, corruption termination, guard pages, per-block decommit, or a
full-title capacity profile. It is partial evidence for `REQ-MEM-01`／`REQ-API-02` and `P3-WIN-01`,
not their completion.

Mapped guest-module／export identity and dependency-ordered `DllMain` lifecycle are implemented in
[runtime-module-images.md](runtime-module-images.md) and
[runtime-dll-lifecycle.md](runtime-dll-lifecycle.md). Capability expansion remains paused at the saved
`GetCurrentThread` stop. The next responsibility checkpoint applies the same private ownership and
plan／preflight／commit boundary to module metadata, image, import binding, and DLL lifecycle state.
