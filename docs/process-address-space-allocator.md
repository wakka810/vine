# Process address-space allocator

## Purpose

`runtime/address_space.rs`の`ProcessAddressSpaceAllocator`をsingle-process guest virtual address
reservationのRust authorityとする。subsystemは新しいfixed result addressを追加せず、purpose、owner、
preferred addressまたはsearch range、reserve size、alignment、guard pages、classification、lifetime、
release policy、executable policyをrequestへ入れる。

## Allocation identity

各成功reservationはmonotonic `ProcessAddressAllocationId`とrecordを返す。recordはusable byte
lengthを保持し、reserved rangeをpage単位へ切り上げてからguard pagesを含める。root allocation同士、同じparentの
child allocation同士でoverlapを検査する。childはparent rangeを出られない。

alignment、empty range、32-bit overflow、guard size overflow、overlap、search exhaustion、ID
exhaustion、release policyをallocatorだけが検査する。preferred rangeが利用できないsearch requestは
aligned first-fitへfallbackする。fixed v1 registrationはfallbackせずstructured overlapで停止する。

## Version 1 registry

base runtime bootstrapは36 recordsを登録し、ANSI command-line profileはmaterialized initial
process environmentを37番目のpermanent root recordとして追加する。

| Purpose | Classification | Records | Policy |
|---|---|---:|---|
| import thunks | runtime control | 1 | preferred v1 range、permanent、executable |
| callback returns | runtime control | 1 | preferred v1 range、permanent、executable |
| dynamic exports | runtime control | 1 | preferred v1 range、permanent、executable |
| private heap band | heap | 1 | preferred v1 range、permanent |
| private heap slots | heap | 32 children | 16 MiB per slot、permanent |
| initial process environment | process environment | 1 when enabled | existing contiguous 4-page materialization、permanent |

`RuntimeHeapBacking`はhandle slotからbaseを再計算せず、対応するheap-slot recordのIDとrangeを保存する。
GuestMemory regionはoptional 64-bit allocation IDを保持し、allocation-backed regionの
reserve／commit／protect／decommit／releaseはすべて同じIDを`MappingOperation`へ載せる。Heap mappingと
runtime control mappingはこの経路へ接続済みである。

## Record-driven mapping

import thunk、callback return、dynamic export installerは対応するlive fixed recordを受け取る。
process environmentはallocation reservationとmemory mutationを先にplanし、GuestMemory transaction内で
4 pageを同じIDへ帰属させてからallocatorとprocess metadataをtargeted commitする。memory側またはruntime側の
preflightが失敗した場合、どちらも変更しない。

mapping前にpurpose、string owner、range containment、classification、lifetime、release policy、executable
policyを検証する。owner、range、lifetimeを改変したrecordはowner作成やgeneration更新より前にstructured
errorで拒否する。

`VINEDL01` version 2は40-byte mapping descriptorへ64-bit allocation IDを追加した。0はallocator導入前の
unbound mapping、nonzeroはallocation-backed mappingを表す。Browser mirrorはmemory ownerだけでなくallocation
IDもreserve／commit／protect／decommit／releaseの全batch preflightで照合し、部分適用しない。

## Search reservation

同じallocatorへguest DLL image、複数thread stack、COM endpointをsearch requestとして予約できる。
testはresult addressを固定せず、alignment、guard metadata、non-overlap、lifetime、release policyを
検査する。mapped fileとprocess environmentも同じpurpose/classification modelへ追加できる。

Search planはlow-to-high first-fitに加えてbounded top-downを持つ。Top-downは全live root allocationを
mutationなしで走査し、指定range内のhighest aligned gapを選ぶ。Planはallocator generationとnext identityを
保持し、重複またはstale commitを拒否する。Primary `VirtualAlloc(NULL, 0x06000000, MEM_RESERVE |
MEM_TOP_DOWN, PAGE_READWRITE)`はこの経路でallocation ID 60、range `[0x79000000,0x7F000000)`を予約する。

`RuntimeKernel::reserve_virtual_memory`はaddress plan、GuestMemory owner capacity、anonymous reserveを全て
preflightしてからGuestMemory transaction内でmappingを作り、最後にallocator planをcommitする。Regionは同じ
allocation IDを持つがpageをcommitせず、physical pageとpayloadを増やさない。Policy外のpreferred address、size、
allocation type、protection、allocator exhaustion、memory overlapは両authorityとowner IDを変更しない。

## Atomicity and non-claims

invalid requestとexhaustionはregistryを変更しない。explicit allocationだけをreleaseでき、live childを
持つparentとpermanent v1 recordはreleaseできない。

このcheckpointは既存control arena、heap backing、active initial process environmentをallocation identityへ
接続する。Guest imageとinitial stackのallocator record自体は存在するが、legacy materializerが作る最初の
GuestMemory reserveをrecord-driven APIへ移すことはこのcontrol-mapping checkpointの範囲外である。
Primary virtual-memory contractはreserve-onlyであり、preferred non-NULL、commit、decommit、protect、release、
placeholder、large page、write-watch、failure returnの一般化は未実装である。

## Regression

```powershell
cargo test -p vine-core runtime::address_space::tests --locked
cargo test -p vine-core --test address_allocation_mapping --locked
cargo test -p vine-core --test runtime_heap_backing --locked
cargo test -p vine-core --test kernel32_virtual_memory_dispatcher --locked
npm run check:execution-engine-contract:prepared
```

Allocator testsはv1 registration、DLL／two stacks／COM search、alignment、overlap、32-bit overflow、
exhaustion、release policy、failure時不変を固定する。Heap integrationはfirst backingのallocation IDと
record containmentを固定する。Current production regressionは
[`current-checkpoint.json`](current-checkpoint.json)のcanonical commandを使う。
