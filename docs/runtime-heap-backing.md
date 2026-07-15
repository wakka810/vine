# Runtime private-heap backing foundation

## Purpose

review Step 9のseventh foundationとして、既存のtyped logical private heapをRust-owned
`GuestMemory`へ接続した。Allocation identity、x86 alignment、zero／nonzero initialization、
free／reuse、invalid／double free、heap destroy、process-exit teardownを一つのauthoritative
boundaryで固定する。

Architecture refactor checkpoint S5では、logical heap metadataとGuestMemory backing metadataを
private `runtime/heaps.rs::RuntimeHeaps`へ移し、同じsubsystem ownerの下へ統合した。
`RuntimeKernel`（既存名`RuntimeState`）はobject table、process address-space allocation、GuestMemory transactionとの
cross-subsystem coordinationだけを行い、heap／backing collectionを直接mutateしない。
Architecture refactor checkpoint S12ではautomatic private-heap handle slot allocatorも同じ
`RuntimeHeaps`へ移した。Heap identity、以前のcursor、commit後のcursorを一つのmutation-free creation
planへ束ね、object registrationが成功した場合だけheap metadataとallocatorを同時にcommitする。
これによりaggregate rootから最後のdirect scalar allocator fieldを除去した。

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

`RuntimeHeaps`は`RuntimeProcessState`のfieldではなく`RuntimeKernel`が独立したsubsystem ownerとして保持する。
既存reportの`process.heaps` shapeは`RuntimeProcessView`がimmutable borrowをflattenして維持し、public
inspectionは`RuntimeKernel::heaps()`のimmutable borrowに限定する。GuestMemory backing mapとslot allocatorは
serializeせず、logical heap mapと同じownerだけが更新する。既存root `nextPrivateHeapSlot`はkernel serializerがownerからimmutable borrowする
compatibility viewであり、第二authorityではない。

Heap creation、allocation、free、destructionはsubsystem内でmutation-free planを作り、aggregateが
process／thread lifecycle、typed object、address allocation、GuestMemory transactionをpreflightした後、
targeted commitする。Creationはobject-table registrationが成功してからlogical heapをcommitする。
Automatic handle allocationはheap subsystem内で候補slotを探索し、live heap slotをskipする。Planは
選択したheapとallocatorのprevious／next cursorを保持し、aggregateによるobject-table registration後に
両方をtargeted commitする。したがってplanning、object-table conflict、allocator exhaustionはslotを
消費しない。Caller-supplied heap identityを受けるexplicit `create_heap`はautomatic cursorを進めない。

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

- mutation-free automatic creation plan、live-slot skip、exhaustion rejection、explicit creationで不変の
  allocator、co-owned logical／backing commit
- stable `process.heaps` and root `nextPrivateHeapSlot` compatibility serialization
- object-table registration failure with unchanged heap collections and private-heap slot allocator
- first allocation ID `1`: `0x20000000`, requested `0x214`, aligned `0x218`, zeroed
- second allocation ID `2`: `0x20000218`, requested／aligned `0x800`, `0xA5`
- free poison `0xDD`, exact double-free rejection, interior-pointer rejection
- reused allocation ID `3`: `0x20000000`, requested／aligned `0x100`, zeroed
- explicit destroy: two live allocations discarded, 16 MiB reserved／4 KiB committed released
- process teardown: one backed and one metadata-only heap, final memory generation／ack `13`

S12 regression preserves the same heap behavior:

- Node BGI oracle: 284 calls／25,256 instructions、next `GetCurrentThread`; semantic report
  SHA-256 `0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`
- Chrome Worker: Node-equivalent memory／next stop、cancel／watchdog clean、browser report
  SHA-256 `854725373a91fb8ca30577755c42c2351208d7bb7299cbef0b73c4e9ab1d3019`
- 180-second BGI workload: 620 sessions、failure `0`、all 17 thresholds passed; report
  SHA-256 `86028b5ea79ecd6640cc4ec7ab55f0d89e44d8590bf32034031b32af5b3acbf2`
- Rust runtime core Wasm: 2,090,657 bytes／SHA-256
  `c7b888d70a6c29ca0ce42f725dec573b6013cd40f4039345c948ec2d7ec1c3a5`

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
`GetCurrentThread` stop. Module metadata, image, import binding, and runtime export collections now
use the same private ownership and plan／preflight／commit boundary. DLL lifecycle phase and in-flight
operation state now share that module owner as well. Responsibility checkpoint S13 completes the
remaining process/kernel aggregate audit and preserves the same serialized compatibility shape.
