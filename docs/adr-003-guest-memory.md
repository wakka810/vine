# ADR-003 Guest Memory Ownership

## 1. Status

- Status: Accepted
- Measurement date: 2026-07-11
- Decision date: 2026-07-12
- Scope: `P0-ARCH-03` partial evidence for 4 GiB guest VA、page protection、page ownership
- Decision: Rust-owned `GuestMemory` is byte authority; v86 paging is a bounded execution mirror
- Implemented ingress: sparse region／page API、bounded zeroed frames、logical execute gate、generation-tagged `MemoryDelta`、binary reflection／delta／writeback、PTE Dirty discovery、atomic multi-page reconciliation、changed-state rollback transaction
- Not decided: guard、snapshot、production physical backing、Worker memory ceiling

このADRはbyte authorityとCPU mirror間の同期方針を確定する。fixed high-address fixtureで2候補を比較し、production Rust ingress、host-originated delta、automatic CPU writebackを同じgeneration contractへ接続した。guard、snapshot、general process mapping、Chrome Worker、performance gateは未完了であり、`P0-ARCH-03`全体や`AC-P1`の完了を意味しない。

## 2. Accepted Decision

`vine-core::GuestMemory`がregion、owner、R／W／X、seal、committed page bytes、generationのsemantic sourceである。v86は同じbytesをlow physical pagesとx86 page tablesへ反映して命令を実行するmirrorであり、そのphysical address、PTE、TLB、generated-code cacheをruntime stateの正本にしない。TypeScriptはbinary bufferを運ぶがguest pointerやpage bytesのauthorityを持たない。

同期点はbounded CPU sliceの前後に限定する。

1. 初回slice前にRustが`VINERF01` reflectionを発行し、mirrorが全mapping／bytesを適用してTLB／instruction cacheをclearした後だけgenerationをacknowledgeする。
2. host側mapping／byte変更は`VINEDL01` deltaとしてmirrorへ適用し、required cache effectsの実行後だけacknowledgeする。pending deltaがある間はguest executionとCPU writebackを開始しない。
3. bounded slice終了時、API trap、callback return、budget yield、fault、wait遷移のいずれでも、mirrorは全writable mappingのPTE Dirty bitをscanする。dirty pageのaddress／ownerと4 KiB payloadを`VINEWB01` batchへ置き、Dirty bitをclearして`full_clear_tlb()`を行う。
4. Rustはbatch全体についてbase generation、current acknowledgement、page count、alignment、duplicate、mapping、write permission、owner、payload lengthを先に検査する。一件でも不一致ならbyte／generationを変更しない。valid batchは実際にbyteが変わったpageだけをone transactionで置換し、batch全体でgenerationを一度だけ進める。mirrorは既に同じbytesを持つため、そのgenerationを同期済みとして確定する。
5. RWX pageは同じdirty scanで検出し、receiptでexecutable page数を記録する。guest自身のwriteに必要なexecution-local invalidationはv86 write pathの責務であり、Rustから同じbytesを再送しない。hostが後からcode pageを変更する場合は通常のdeltaがinstruction-fetch／generated-code invalidationを要求する。

Dirty bit clear後からRust receipt受領まで、binary batchはhost側で保持する。generation conflict、transport truncation、worker cancellation、receipt lossが起きた場合はguestをresumeしない。Rustがbatchを受理していないと確認できる場合だけ同じbufferをretryできる。commit済みか不明、またはreceiptを失った場合はsessionを破棄し、Rust authorityからmirrorを再構築する。partial continuationや片側だけのrollbackは行わない。

binary execution transportはcontrol metadataとcontiguous page payloadを別bufferにする。`VINERF01`、`VINEDL01`、`VINEWB01`を`VINEFR01` frameで運び、JSON numeric page arraysはevidence reportだけに限定する。現在のPTE discoveryはsliceごとにO(writable pages)、Dirty clearはTLB全clearである。このcostはAccepted decisionの正確性baselineである。Step 8 synthetic benchmarkはreflection／writeback／invalidation／active memory thresholdを満たして継続`go`となったが、title workloadで再測定する。

## 3. Required Contract

Vine guest memoryは、次を同時に満たす必要がある。

- 32-bit guest virtual address全域をhost／Wasm offsetと1:1に対応させない
- reserveとcommit、read／write／execute、unmapped、guardをWindows-visible region stateとして保持する
- committed pageだけをbounded physical quotaへ割り当てる
- cross-page accessを全pageでpreflightし、fault前のpartial writeを残さない
- protect／remap後にdata TLB、instruction fetch、generated codeを一貫してinvalidateする
- region、physical page、CPU stateを同じsnapshot generationとして保存／復元する

比較対象は次の2案である。

1. v86 pagingを使い、Vine-owned region tableとphysical allocatorからx86 page tableを生成する。
2. v86のmemory accessをVine-owned checked sparse memoryへ差し替える。

## 4. Reproduction

`fixtures/pe32/high-memory/fixture-memory.c`から、importなしのfixed PE32 imageを生成する。

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:memory
npm run check:runtime:memory-core
npm run check:runtime:memory-writeback
```

| Artifact | Observed |
|---|---|
| Fixture | `high-memory.exe`、3,072 bytes、SHA-256 `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7` |
| PE contract | PE32／i386、LAA、fixed image base `0xF1000000`、image size `0x6000`、NX compatible |
| Sections | `.text` RX、`.rdata` R、`.data` RW、`.nxprobe` R、`.rprobe` R |
| Adapter | source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`、Wasm SHA-256 `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Report | `.work/v86-memory-spike/report.json`、schema version `1` |
| Production ingress | `vine-core` guest memory、`vine.guest-memory-ingress` version `1` |

runnerは同じpage mapをsource-built v86 adapterとchecked sparse referenceへ与える。referenceはCPUへ接続せず、Windows-visible R／W／Xとfault orderingのexpected contractだけを表す。

## 5. Measured Result

### 5.1 Mapping

| Property | Result |
|---|---:|
| Guest virtual address space | 4 GiB |
| v86 contiguous physical pool | 2 MiB |
| Mapped virtual pages | 11 pages／44 KiB |
| Initial physical allocation | 15 pages／60 KiB |
| Page structures | 1 directory + 3 page tables |
| Highest mapped page | `0xF1005000` |
| Identity mapping | No |

high image、high stack、cross-page probeをlow physical pagesへmapし、entryはEIP `0xF1001000`から9命令でreturn trap `0xF1001070`へ到達した。EAX `0`、writable result `0x11223345`である。これは4 GiBをWasm linear memoryとして確保せずhigh VAを実行できることを示すが、2 MiB pool自体はまだcontiguousなfixture allocationである。

### 5.2 Fault And Invalidation

| Probe | v86 paging | Checked sparse reference |
|---|---|---|
| write R page `0xF1005000` | `#PF`, PFEC `7`, no write | fault `7`, no write |
| read unmapped `0xE1000000` | `#PF`, PFEC `4` | fault `4` |
| write unmapped `0xE1000000` | `#PF`, PFEC `6` | fault `6` |
| cross-page write at `0xE2000FFE` | second page `0xE2001000`でPFEC `6`、first page unchanged | same ordering／no partial write |
| fetch unmapped `0xE1000000` | PFEC `4` | instruction-fetch fault `20` |
| execute R／NX fixture page `0xF1004000` | executed、EAX `0x12345678` | instruction-fetch fault `21` |

v86のfixed revisionはinstruction fetchにread translationを使い、PAE page walkもNXをunsupportedとする。したがってPE sectionのexecute bitはnative v86 PTEだけでは表現できず、PFEC instruction-fetch bitも失われる。silent successにせず、この差をadapter-owned execute gateの必須条件とする。

RW PTEをread-onlyへ変更した直後はcached TLBによりwriteが通り、`full_clear_tlb`後はPFEC `7`になった。virtual code pageを別physical pageへremapした場合も、clear前は旧value `0x12345678`、clear後は新value `0x87654321`を実行した。protect／remap transactionはpage metadata更新とTLB／code invalidationを一つのowner boundaryで行う必要がある。

## 6. Candidate Comparison

| Property | v86 paging + Vine ownership | Checked sparse replacement |
|---|---|---|
| CPU integration | Existing interpreter pathでverified | Production Rust boundaryはCPU未接続 |
| High VA | Verified | Production contractでverified |
| Present／R／W | Verified | Production contractでverified |
| Execute | Vine execute gateが必要 | Logical gateをproduction contractでverified |
| Physical memory | bounded poolは可能だがv86 RAMはcontiguous | committed page単位にできる |
| TLB／code cache | v86 invalidation semanticsを使う | 全fetch／decode pathを再接続する必要がある |
| Change surface | region／allocator／fetch gate中心 | virtual access、physical access、JIT assumptionsの広い変更 |

CPU-integrated high-address executionとR／W fault orderingを保持でき、automatic writebackまで接続できたため、前者をAccepted baselineとする。checked sparse案は境界が明快だが、現在はCPUに接続されておらず、hot pathとgenerated-code pathを変更する根拠が不足する。v86 paging案がexecute gate、quota、snapshot、Worker ceiling、またはStep 8 performance gateで成立しない場合に再評価する。

## 7. Production Ingress Result

`vine-core`は次をproduction-oriented Rust APIとして実装した。

- checked guest address／rangeと4 KiB owner／region／page state
- fixed reserve、commit、protect、decommit、releaseとbounded zeroed physical allocation
- image planから1 regionをreserveし、header／section pageをfinal R／W／X metadataでcommitするingress
- instruction fetch前のlogical execute permission／owner sealとstructured fault address／PFEC
- 全page preflightによるcross-page no-partial-write
- mapping operationとdirty page payloadをcache effectから分離するgeneration-tagged delta journal
- ordinary RW data writeのdirty-only deltaとwrite-to-codeのfetch／generated-code invalidation
- offered generationまでのdirty stateだけを消し、後続mutationを保持するexact acknowledgement
- owner／region／frame／bytes／protection／seal／generation／pending deltaを復元するtransaction

high-memory contractは6 image pagesと1 anonymous probe pageをexact quota内で扱い、BGI planは380 image pagesをexact quotaで所有した。詳細なfault sequence、report hash、structured stopは[guest-memory-ingress.md](guest-memory-ingress.md)を正とする。

後続のcontrolled entry contractはsealed image 5 pagesとruntime thunk 1 pageをguest VA／owner／R／W／X／bytesだけのsemantic reflectionとしてv86へ渡した。同じRust memory objectはgeneration `6`をpendingに保ち、adapterのstate restore／`full_clear_tlb`／`jit_clear_cache_js`後のackだけを受理した。ack後はinitial dirty stateが0になる。2 MiB poolのpage directory／3 tables／6 reflected pagesとharness stackからentryを実行し、owned thunk ID `1`へ到達した。詳細は[v86-entry-ingress.md](v86-entry-ingress.md)を正とする。

BGI continuationは同じboundaryを388 semantic pages／generation `24`へ拡張し、4,904,602-byte offerを8 MiB protocol上限内で転送した。v86はphysical `[0x000A0000,0x000C0000)`をVGA MMIOとして扱うため、naive contiguous placementは`mmap_read32(0x000A883F)`へ入った。adapter allocatorはこの32-page holeを明示的に除外し、directory 1 page、tables 4 pages、reflected 388 pagesの393 allocationsを2 MiB内へ置いた。値をfabricateするMMIO handlerは使っていない。この配置でBGI entryは19命令後にowned thunk ID `104`へ到達した。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

`npm run check:runtime:memory-delta`は初回snapshotのack後にnon-executable RW pageへ8 bytesを書き、generation `11`の1-page／4 KiB deltaをlive v86 mappingへ適用する。cache effectsはすべてfalseで、guest x86はEDX:EAX `0x8877665544332211`を読む。mapping／write-to-code／partial ackとtransaction failpointのRust contractを含む詳細は[memory-delta-and-transactions.md](memory-delta-and-transactions.md)を正とする。

`npm run check:runtime:memory-writeback`は9 production pagesをbinary reflectionし、8 writable PTEをAPI固有page listなしでscanする。first sliceはstack 2、heap 2、surface 2、RWX target 1の計7 dirty pagesを検出し、49 changed bytesをgeneration `18 -> 19`としてatomic reconcileする。Rustからscratch pageへのbinary deltaをgeneration `19 -> 20`で適用した後、second sliceは別のstack／heap／surface／RWX 4 pagesだけを検出し、13 changed bytesをgeneration `20 -> 21`へ進める。各scanはDirty bit clear後にTLBをflushし、untouched scratch pageはfirst sliceでclean、Rustはreceipt後dirty 0／pending invalidationなしとなる。詳細は[memory-writeback.md](memory-writeback.md)を正とする。

BGI first-call continuationは、outputとlive call frameが同じstack pageにあるcaseを固定した。adapterがcurrent 4 KiB pageをgeneration `24`／owner `4`付きで供給し、Rustはalignment／size／mapping／write permissionをchecked reconcileする。この取込はgenerationを進めずdirtyにもせず、直後の8-byte host writeだけがgeneration `25` deltaを作る。full delta pageがcurrent adapter page＋outputと一致することを検査した。これはexplicit one-page handoffであり、自動dirty trackingではない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

次は未実装であり、後続checkpointで閉じる。

- process-wide stack／heap／module mappingとexecute gate、fault classificationのgeneralization
- region／page／CPU generationを揃えるsnapshot contract
- guard、mapped file、copy-on-write、sharing、memory pressure、Worker ceiling
- writable-page scan／full TLB clearのtitle-workload performanceと必要なincremental dirty index

host TypeScriptはguest pointerやhot accessを所有せず、loader／Win32 facadeからsemantic map requestを渡す。page tableのraw bytes、physical page number、v86 private offsetをWindows API層へ公開しない。

## 8. Non-Claims And Next Gate

production ingress、controlled reflection、automatic multi-page writebackは完了したが、`P0-ARCH-03`、`P1-MEM-01`～`P1-MEM-03`、`P1-CPU-01`、`AC-P1`を完了しない。guard、copy-on-write、mapped file、all-width access、PAE／NX、decode-aware cross-page execute permission、snapshot、title-scale memory pressureは未検証である。Rust sparse implementationはhost／CPU両方向のchanged-page transferをgeneration単位で行え、controlled Chrome Workerとactive steady-memory gateを通したが、general heap／module lifecycleやsnapshotには未接続である。

owned W7 plan、entry completion、initial stack、typed bootstrap reflection、atomic import binding、MemoryDelta、BGI 17-call continuation、automatic CPU writebackまで進み、同じbinary／generation boundaryをcontrolled Chrome Runtime Worker vertical sliceとStep 8 benchmarkで使用した。Step 9のtyped object／handle lifetimeも同じRust authorityへ接続した。次はmodule graphであり、title-scale heap／module／snapshot performanceは実装到達時に再測定する。
