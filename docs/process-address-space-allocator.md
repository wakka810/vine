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

process bootstrapは次の36 recordsを登録する。

| Purpose | Classification | Records | Policy |
|---|---|---:|---|
| import thunks | runtime control | 1 | preferred v1 range、permanent、executable |
| callback returns | runtime control | 1 | preferred v1 range、permanent、executable |
| dynamic exports | runtime control | 1 | preferred v1 range、permanent、executable |
| private heap band | heap | 1 | preferred v1 range、permanent |
| private heap slots | heap | 32 children | 16 MiB per slot、permanent |

`RuntimeHeapBacking`はhandle slotからbaseを再計算せず、対応するheap-slot recordのIDとrangeを保存する。
GuestMemoryのreserve／commit／decommit／releaseはこのidentityへ帰属できる。現在のheap mappingはrecord
IDへ接続済みである。

## Search reservation

同じallocatorへguest DLL image、複数thread stack、COM endpointをsearch requestとして予約できる。
testはresult addressを固定せず、alignment、guard metadata、non-overlap、lifetime、release policyを
検査する。mapped fileとprocess environmentも同じpurpose/classification modelへ追加できる。

## Atomicity and non-claims

invalid requestとexhaustionはregistryを変更しない。explicit allocationだけをreleaseでき、live childを
持つparentとpermanent v1 recordはreleaseできない。

このcheckpointは既存control arenaをregistryへ登録し、heap backingをallocation identityへ接続する。
import／callback／dynamic-export installerのGuestMemory transaction APIはまだallocation recordを引数に
取らない。image、initial stack、TEB／PEB／process parametersの実mappingをregistryへ登録する作業も、
各subsystemを次に変更するcheckpointで行う。

## Regression

```powershell
cargo test -p vine-core runtime::address_space::tests --locked
cargo test -p vine-core --test runtime_heap_backing --locked
```

Allocator testsはv1 registration、DLL／two stacks／COM search、alignment、overlap、32-bit overflow、
exhaustion、release policy、failure時不変を固定する。Heap integrationはfirst backingのallocation IDと
record containmentを固定する。Current production regressionは
[`current-checkpoint.json`](current-checkpoint.json)のcanonical commandを使う。
