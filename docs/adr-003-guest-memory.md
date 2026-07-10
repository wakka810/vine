# ADR-003 Guest Memory Ownership

## 1. Status

- Status: Provisional candidate／In discovery
- Measurement date: 2026-07-11
- Scope: `P0-ARCH-03` partial evidence for 4 GiB guest VA、page protection、page ownership
- Working candidate: Vine region table + bounded physical page allocator reflected into v86 paging
- Not decided: production API、execute gate、guard、snapshot、generated-code invalidation、Worker memory ceiling

この文書はmemory architectureのaccepted decisionではない。fixed high-address fixtureで2候補を比較し、次のproduction sliceで使うworking directionと、acceptance前に閉じるgateを固定する。

## 2. Required Contract

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

## 3. Reproduction

`fixtures/pe32/high-memory/fixture-memory.c`から、importなしのfixed PE32 imageを生成する。

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:memory
```

| Artifact | Observed |
|---|---|
| Fixture | `high-memory.exe`、3,072 bytes、SHA-256 `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7` |
| PE contract | PE32／i386、LAA、fixed image base `0xF1000000`、image size `0x6000`、NX compatible |
| Sections | `.text` RX、`.rdata` R、`.data` RW、`.nxprobe` R、`.rprobe` R |
| Adapter | source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`、Wasm SHA-256 `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Report | `.work/v86-memory-spike/report.json`、schema version `1` |

runnerは同じpage mapをsource-built v86 adapterとchecked sparse referenceへ与える。referenceはCPUへ接続せず、Windows-visible R／W／Xとfault orderingのexpected contractだけを表す。

## 4. Measured Result

### 4.1 Mapping

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

### 4.2 Fault And Invalidation

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

## 5. Candidate Comparison

| Property | v86 paging + Vine ownership | Checked sparse replacement |
|---|---|---|
| CPU integration | Existing interpreter pathでverified | Reference only。CPU未接続 |
| High VA | Verified | Reference verified |
| Present／R／W | Verified | Reference verified |
| Execute | Vine execute gateが必要 | Reference verified |
| Physical memory | bounded poolは可能だがv86 RAMはcontiguous | committed page単位にできる |
| TLB／code cache | v86 invalidation semanticsを使う | 全fetch／decode pathを再接続する必要がある |
| Change surface | region／allocator／fetch gate中心 | virtual access、physical access、JIT assumptionsの広い変更 |

現時点では、CPU-integrated high-address executionとR／W fault orderingを保持できる前者をworking candidateとする。checked sparse案は境界が明快だが、現在はCPUに接続されておらず、hot pathとgenerated-code pathを変更する根拠が不足する。v86 paging案がexecute gate、quota、snapshot、Worker ceilingのいずれかで成立しない場合に再評価する。

## 6. Required Production Boundary

次のproduction memory sliceは、少なくとも次をRust-owned APIとして持つ。

- checked guest address／rangeと4 KiB region／page state
- reserve／commit／protect／unmapとbounded physical allocation
- region stateから生成するpage directory／PTE ownership
- instruction fetch前のexecute permission checkとstructured fault address／PFEC
- protect／remap時のdata TLB、fetch mapping、generated-code invalidation
- region／page／CPU generationを揃えるsnapshot contract

host TypeScriptはguest pointerやhot accessを所有せず、loader／Win32 facadeからsemantic map requestを渡す。page tableのraw bytes、physical page number、v86 private offsetをWindows API層へ公開しない。

## 7. Non-Claims And Next Gate

このspikeは`P0-ARCH-03`、`P1-MEM-01`～`P1-MEM-03`、`P1-CPU-01`、`AC-P1`を完了しない。guard、copy-on-write、mapped file、all-width cross-page access、self-modifying code、PAE／NX、generated JIT、snapshot、memory pressure、Chrome Workerは未検証である。checked sparse implementationもproduction codeではない。

次のvertical checkpointはBGI.exeをproduction loader候補へ入力し、parse／map計画のfirst unsupported importまたはbootstrap pointを構造化することである。guest codeを実行する前に、このADRのregion／page ownerとexecute gateをproduction Rust boundaryへ接続する。
