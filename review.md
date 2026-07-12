# Vine Repository Review

## 0. 文書の位置づけ

本書は、Vine リポジトリに対する3件のレビューを統合し、実装エージェントがそのまま作業順序・設計判断・受け入れ条件として使用できる形に再構成したものである。

- 対象リポジトリ: `wakka810/vine`
- 確認基準: `main`
- 確認コミット: `c12c3d86a8c5350e46c8ac2e44775f27663f8a7e`
- 現在のcheckpoint: `bgi-seventeenth-win32-call`
- 現在の到達点: 17 Win32 call return、累計238 guest instructions、memory generation 30
- 判定: **Conditional Go — 継続価値は高いが、API逐次追加から汎用runtime kernel構築へ直ちに軌道修正すること**

本書は、既存の検証証跡を否定するものではない。現在までのcheckpointを保存しつつ、それをproduction runtimeへ接続可能な構造へ移行するためのレビュー指示である。

既存の `AGENTS.md`、architecture、roadmap、testing文書と本書が矛盾する場合、事実確認を行ったうえで、同一コミット内で関連文書を整合させること。現在状態の手書き複製を増やしてはならない。

---

## 1. 総合判定

Vineは、対象を「fingerprint固定のPE32/i386タイトル1本をChrome上で全編プレイ可能にする」に限定する限り、技術的な成立可能性がある。

現在までに、以下の難しい基礎要素は実コードと再現可能な証拠で成立している。

- bounded PE32 parsingとmaterialization
- sparse guest memory、page protection、generation、invalidation
- relocationとimport binding
- unresolved importのidentity-preserving trap
- atomic import commitとrollback
- v86 adapter上のbounded execution
- exact `UD2` trap
- ABI return、stack reconciliation、guest callback
- controlled TEB／PEB／process parameters
- logical heap handle、module handle、TLS index、encoded pointer
- synthetic window/message/pixel fixture
- Canvas pixel presentation checkpoint
- deterministic evidenceとWindows oracleの区別

したがって、プロジェクトを中止または全面破棄する理由はない。

ただし、現在の成果は「Windows互換ランタイムが一部完成した状態」ではない。正確には、次を証明する高品質な成立性実験基盤である。

> fingerprint固定のPE32 imageを安全に構築し、v86上で限定命令列を実行し、既知のWin32 API境界で決定的に停止・検査・復帰できる。

full playに必要な一般DLL lifecycle、process/thread model、scheduler、wait、message、heap backing、VFS、registry、browser runtime、graphics、audio、storage、長時間安定性は未成立である。

### 結論

| 観点 | 判定 |
|---|---|
| 研究・成立性検証 | 高品質。継続すべき |
| 固定1タイトルの技術的可能性 | あり。ただし高リスク |
| 現在のAPI逐次追加方式 | 持続不能 |
| 現在のコード構造 | production runtime化の前に再編が必要 |
| v86採用 | working baseline。production採用は未確定 |
| full play到達可能性 | Phase 1のmemory／scheduler／Worker／performance gate完了まで判定不能 |
| 次の主要作業 | 汎用dispatcher、RuntimeState、memory authority、Chrome Worker vertical slice |

---

## 2. 守るべき強み

以下はVineの主要資産であり、リファクタリング中も維持すること。

### 2.1 Fail-closed compatibility

- 未対応APIをsilent success stubにしない。
- unsupported identityを保持した構造化trapで停止する。
- pointer、length、RVA、handle、path、calling conventionを境界で検査する。
- source imageを変更せず、runtime stateと分離する。
- title hashやEIPだけを条件に処理をskipしない。

### 2.2 Plan / validate / atomic commit

PE mapping、import binding、runtime endpoint installationでは、可能な限り以下を維持する。

1. read-only plan
2. 全入力の検証
3. generation／precondition再確認
4. atomic mutation
5. seal
6. failure時rollback

### 2.3 Host pointerをguestへ漏らさない

以下はlogical identityとして管理する。

- module
- heap
- thread
- TLS／FLS
- handle
- dynamic export endpoint
- encoded pointer
- callback frame
- browser-side resource

host address、native handle、process cookieをそのままguestへ返してはならない。

### 2.4 Evidence-driven development

- 実装したことと、まだ証明していないことを分けて記録する。
- Windows native oracle、Linux recorded evidence、Node harness、Chrome実行を混同しない。
- BGIで発見した一般契約は、可能な限り小さい公開fixtureへ還元する。
- exact snapshotだけでなくsemantic contractを残す。

---

## 3. 現在の最大リスク

### 3.1 API一件ごとのcheckpoint実装が製品構造を代替している

現在は、BGIの各API callについて個別に以下が増えている。

- npm script
- CLI flag
- boolean mode
- request型
- receipt型
- session function
- fixed argument
- fixed caller RVA
- fixed report
- exact hash

`tools/run-runtime-v86-bgi-first-trap.mjs` は約9,000行、`vine-pe-probe/src/main.rs` は約6,200行に達している。17 call時点でこの規模であり、数百API、COM method、callback、wait、file I/O、message loopへ同じ方式を拡張することはできない。

**API call数をproduction進捗指標として扱ってはならない。**

最重要指標は次である。

> 現在の17 callを、API固有の実行モードなしで、共通のRuntimeState、scheduler、ABI dispatcherから再現できるか。

### 3.2 authoritative runtime stateが分散している

現状では、概ね以下の状態が分かれている。

- Rust:
  - guest memory
  - PE／import metadata
  - controlled module／heap／TLS／pointer state
  - runtime endpoints
- JavaScript harness:
  - v86 orchestration
  - window/class/HWND/HDC
  - message queue
  - paint state
  - surface
  - checkpoint progression

この分散状態では、以下の一貫した実装が困難になる。

- thread termination
- per-thread LastError
- wait／wake
- callback reentrancy
- DLL attach／detach
- TLS cleanup
- handle close
- object destruction
- window destruction
- message dispatch
- host request cancellation

process、thread、module、handle、wait、message、callback frame、LastErrorのauthoritative stateを一つのruntime kernelへ集約する必要がある。

### 3.3 GuestMemory transactionが全体deep cloneである

**解消（2026-07-12）:** `GuestMemory::begin_transaction()` はchanged-state／
frame before-image journalへ置換された。transaction開始時に`GuestMemory`、
page table、committed frame bytesをcloneせず、既存frameの初回writeまたは
release時だけ4 KiBを保持する。4 MiB baselineに対する開始payload 0 bytes、
1／2 changed pageで4,096／8,192 bytesをsemantic testで固定した。

旧実装は `self.clone()` をrollback snapshotとして保持し、
`PhysicalFrames`のpage bytesもclone対象だった。

これは現在の数百pageでは動作するが、以下が加わると、transaction開始コストが全commit済みメモリ量に比例する。

- heap
- 複数stack
- DLL graph
- framebuffer／surface
- texture
- decoded image
- file cache
- audio buffer
- snapshot

デフォルト上限65,536 page、256 MiBを考えると、これは単なる最適化課題ではなく、full play成立を妨げる構造問題である。

### 3.4 Rust memoryとv86 memoryの二重所有が未解決

現在はRust側のsparse memoryをsemantic sourceとし、v86へreflectionする。一方、v86-originated writeは必要なpageをcheckpointごとに手動reconcileしている。

未成立の要素:

- automatic dirty detection
- arbitrary multi-page writeback
- write conflict policy
- code-page writeとgenerated-code invalidation
- heap／surface／file bufferの継続同期
- snapshotとの整合
- Worker scheduling中のownership

「両方がbyte copyを持ち、必要箇所だけ手動で戻す」状態をproductionへ持ち込んではならない。

### 3.5 JSON中心のmemory transportはproduction境界に不適切

現在のJSON report／reflectionは、診断・証跡としては有用である。しかし、byte arrayをJSON化してNode、Worker、Wasm間の実行経路に使用すると、以下の問題が生じる。

- 大きな文字列生成
- parse時の一時メモリ
- GC pressure
- byte数の膨張
- control metadataとbulk bytesの混在
- frame／audio deadlineに不向き
- API追加ごとのprotocol型増殖

production経路では、binary header、typed array、transferable、Wasm memory view、ring buffer等を用い、control messageとbulk bytesを分離する必要がある。JSONはhuman-readable evidence用途に限定する。

### 3.6 fixed arena／one-shot endpoint installation

固定import、callback、dynamic-export arenaはcontrolled fixtureとして妥当だが、general runtimeには以下が必要である。

- process単位address-space layout
- stable endpoint registry
- module＋name／ordinalのcanonical key
- incremental slot allocation
- pageの増分commit
- 同一`GetProcAddress`結果の安定性
- forwarder
- ordinal
- callback thunk
- COM vtable thunk
- unload時lifetime policy
- multiple guest threadへの対応

固定アドレス自体を即時廃止する必要はない。ただし、値を各subsystemへ直接埋め込まず、`ProcessAddressSpaceLayout`または同等の一元管理へ移すこと。

### 3.7 v86のsynthetic実用性能はgo、正確性breadthは未判定

現在のv86 adapterは重要な基礎契約を満たしている。

- instruction budget
- state save／restore
- structured faults
- exact trap
- callback
- limited integer／x87／SSE smoke

Step 8のversion 1 Chrome gateはCPU／trap／callback／Worker／memory／background／
audioの全thresholdを満たし、交換可能なbaseline interpreterとして継続`go`となった。
ただし、production engineの最終採用に必要な以下は未判定である。

- x87／SSE correctness breadth
- adopted exception／privileged-path behavior breadth
- 238-instruction checkpointを越えるBGI／title workload performance
- generated-code pathとpermission consistency

API対応数を増やす前に、これらを測定しなければならない。

---

## 4. P0指示 — 次のAPI追加より先に実施すること

### P0-0. bespoke checkpoint拡張を凍結する

現在の17 checkpointは削除しない。回帰証跡として保持する。

ただし、以下を禁止する。

- 19件目以降のAPI専用CLI flag追加
- APIごとの新しいboolean mode chain
- APIごとの専用request／receipt型を上位orchestrationへ追加
- `package.json`への同型の連番script追加
- checkpoint固有stateをproduction coreへ追加
- generic runtimeを経由しないBGI先行実装

現在の次callである `EncodePointer(0x7F020000)` は、pointer-encoding調査を閉じるために必要なら扱ってよい。ただし次の条件を満たすこと。

- 新しい専用実行経路を作らない。
- generic dispatcherの最初のmigration caseとして扱う。
- `check:runtime:bgi-eighteenth-win32-call` を追加する場合も、内部では共通dispatcherとcheckpoint manifestを使用する。
- その後のAPIへ進む前にP0 gateを完了する。

### P0-1. generic API schemaとdispatcherを実装する

最小schemaは以下を表現すること。

```text
ApiContract
  module
  name | ordinal
  endpoint identity
  calling convention
  argument schema
  pointer / string / struct constraints
  in / out / inout
  return schema
  stack cleanup
  LastError policy
  reentrancy policy
  immediate | deferred behavior
  callback policy
  implementation ID
```

共通実行経路:

```text
owned trap
 -> endpoint lookup
 -> ApiContract lookup
 -> ApiCallFrame construction
 -> checked argument decode
 -> handler invocation
 -> Immediate | Deferred | GuestCallback | Fault
 -> guest writes
 -> EAX / EDX / flags / ESP / EIP / LastError restoration
 -> scheduler transition
```

最低限必要な型／責務:

- `ApiContract`
- `AbiSchema`
- `ApiDispatcher`
- `ApiCallFrame`
- `ExecutionSession`
- `DispatchOutcome`
- `HostRequest`
- `HostCompletion`
- structured `Fault`

#### 受け入れ条件

- 現在の17 callが同じdispatcherを通る。
- APIごとのCLI modeが不要になる。
- 呼出規約、stack cleanup、LastError、guest writeを共通処理する。
- unsupported endpointはidentityを保ったままfail-closedする。
- semantic testが個別report hashに依存せず成立する。
- BGI checkpointはmanifestまたはdata tableで表現される。

### P0-2. authoritative RuntimeStateを導入する

以下を一つのruntime state modelへ集約する。

```text
RuntimeState
├── ProcessState
│   ├── AddressSpaceLayout
│   ├── ModuleGraph
│   ├── HeapSet
│   ├── ObjectTable
│   ├── HandleTable
│   ├── RegistryView
│   ├── VfsView
│   └── ClockPolicy
├── ThreadState
│   ├── CpuState
│   ├── Stack
│   ├── TEB
│   ├── LastError
│   ├── TLS / FLS
│   ├── MessageQueue
│   ├── WaitState
│   └── CallbackFrames
├── Scheduler
│   ├── Runnable
│   ├── WaitingHost
│   ├── WaitingObject
│   ├── InCallback
│   └── Terminated
└── RuntimeRegistries
    ├── ThunkRegistry
    ├── DynamicEndpointRegistry
    └── HostRequestTable
```

#### Rust／TypeScript境界

同期的・決定的・guest pointerに接する状態は原則Rust/Wasm側に置く。

Rust/Wasm側:

- process／thread state
- scheduler core
- LastError
- TLS／FLS
- handle／object table
- wait objects
- message queue
- heap metadata
- module graph
- ABI marshalling
- synchronous Win32 semantics
- logical GDI／COM state

TypeScript側:

- DOM input
- Canvas／WebGL／WebGPU presentation
- AudioWorklet transport
- OPFS／File System Access／IndexedDB adapter
- permission／user activation
- browser lifecycle
- diagnostics UI

境界はper-APIの細粒度往復ではなく、coarse-grained command、event、buffer、completionとする。

#### 受け入れ条件

- process ID、thread ID、LastError、TLS、module、heap、endpointが同一RuntimeStateから取得される。
- JavaScript runnerがWindows object stateの正本にならない。
- callback nested execution中もthread／LastError／wait stateが一貫する。
- host async completionはguest stackへPromiseを露出せずscheduler eventとして復帰する。

### P0-3. memory architectureを確定する

#### 3.1 transactionの置換

全体deep cloneを以下のいずれかへ置き換える。

- undo journal
- page単位copy-on-write
- changed-page before-image
- transactional overlay
- metadata transactionとbyte transactionの分離

最低条件:

- transaction開始コストが全commit済みpage数に比例しない。
- byte copyは最初のwrite時または変更pageだけに限定される。
- existing atomic import／bootstrap testsを維持する。
- failure injectionで完全rollbackを証明する。

#### 3.2 byte authorityのADRを作成する

以下のいずれかを選び、理由、ownership、同期点、failure policyを文書化する。

1. Rust-owned bytesをauthorityとし、v86 dirty bitmapをslice終了時に反映する。
2. v86 backingをbyte authorityとし、Rustはchecked metadata／access layerを持つ。
3. shared backingを用い、Rustとadapterが同一memoryを参照する。

選択しないまま実装を継続してはならない。

#### 3.3 guest-originated writeback

必要機能:

- automatic dirty tracking
- arbitrary multi-page reconciliation
- stack／heap／global／surface writeback
- code-page write検出
- TLB／instruction-fetch／generated-code invalidation
- conflict／generation policy
- bounded memory accounting
- cancellation時の整合性

#### 3.4 binary transport

- control metadataとpage bytesを分離する。
- `ArrayBuffer`、typed array、transferable、shared queue、Wasm memory view等を用いる。
- JSON reportはexecution transportではなくevidence artifactへ限定する。

#### 受け入れ条件

- guestが複数pageへ書いた結果が、API固有のreconcile listなしでRust／runtime stateへ反映される。
- transaction追加memoryが変更page数に比例する。
- 連続slice後もgeneration、dirty、cache effectsが一貫する。
- heap、stack、surfaceを用いたbenchmarkが存在する。

### P0-4. Chrome内のproduction vertical sliceを完成させる

現在のCanvas checkpointは、生成済みeventをChromeへ渡してpixel readbackする段階である。次は、guest executionを含む全経路をChrome内で通す。

```text
Chrome main thread
  UI / Canvas / DOM input
          |
          v
Runtime Worker
  scheduler / host protocol
          |
          v
Rust/Wasm runtime
  RuntimeState / memory / PE / ABI dispatcher
          |
          v
v86 adapter
          |
          v
controlled PE32 fixture
          |
          v
Win32 call / callback / present / host completion
```

必要なvertical slice:

- guest CPUをRuntime Workerで実行
- generic API trap
- synchronous API return
- deferred host service
- guest suspend／resume
- nested guest callback
- periodic yield
- cancellation
- watchdog
- main thread non-blocking
- test pixel
- audio pulse
- persistent byte
- clean process exit

#### 受け入れ条件

- guest CPU、PE loader、dispatcher、message flowがChrome内で実行される。
- main threadでCPU loopを実行しない。
- asynchronous browser operation中にguest threadが正しくwaitingへ移行する。
- cancel／timeout後にdangling callbackまたはhost requestが残らない。
- pixel、audio、persistent byteが同一runtime sessionから得られる。
- 30分stressでmemory slopeとresponsivenessを記録する。

### P0-5. v86 go/no-go benchmarkを実施する

測定項目:

- integer instructions/sec
- x87／SSE representative workload
- API trap round-trip/sec
- callback round-trip cost
- context save／restore cost
- 1／5／10／16 ms slice overhead
- page reflection帯域
- guest writeback帯域
- code invalidation cost
- Worker cold start
- steady-state memory
- 30分memory slope
- background tab behavior
- audio deadline併走時のyield
- Wasm↔JS crossing count

benchmarkはpublic synthetic fixtureと、可能ならprivate BGI-like traceの両方を持つ。

#### 判定規則

- 測定前にv86依存を広げない。
- 結果が明確なblockerを示す場合、同一adapter contractで代替engineを1つだけ比較する。
- 広いengine調査や自作interpreterへ無制限に脱線しない。
- go/no-go判断、環境、Chrome version、commit、artifact hashを保存する。

---

## 5. P1指示 — P0成立後に実施すること

### P1-1. production codeとfixture／evidence codeを分離する

以下はtestkit／fixture／evidence側へ移す。

- `CONTROLLED_*`
- fixed process／thread identity
- fixed clock
- fixed 1-page stack
- fixed TEB／PEB／command-line addresses
- BGI-specific caller RVA
- exact API call sequence
- expected report hash
- native oracle record
- checkpoint-only heap／TLS／pointer variants

推奨する論理依存方向:

```text
vine-core
  checked addresses / errors / identities / limits

vine-memory
  region / page / protection / delta / transaction

vine-loader
  PE parser / mapper / module graph / imports / exports / DLL lifecycle

vine-runtime
  Process / GuestThread / Scheduler / ObjectTable / Dispatcher

vine-win32
  API metadata / Kernel32 / User32 / GDI / COM semantics

vine-v86
  versioned adapter ABI / execution / fault translation

vine-browser
  Worker protocol / graphics / audio / storage / input adapters

vine-testkit or vine-evidence
  controlled profiles / BGI manifests / native traces / golden evidence
```

物理crate分割は即時必須ではない。まずmodule依存方向を作り、安定後にcrate分割する。

### P1-2. loader／process基盤を一般化する

必要機能:

- process address-space allocator
- module graph
- export lookup
- forwarder
- ordinal
- DLL dependency order
- TLS callback
- process／thread attach
- detach／unload
- loader list
- process exit
- stack growth／guard
- heap backing
- per-thread LastError

### P1-3. stateful ThunkRegistryを実装する

要件:

- canonical module identity
- name／ordinal key
- stable endpoint reuse
- incremental allocation
- additional page commit
- endpoint kind
- calling convention metadata
- unload lifetime
- stale handle／endpoint detection
- callback／COM endpoint support
- deterministic snapshot／replay identity

### P1-4. checkpoint harnessをmanifest化する

**完了（2026-07-12）:** `fixtures/checkpoints/bgi-startup-win32.json`を正本とし、
17 callのdispatch順序、occurrence、引数、結果、guest write、next stop、evidence
digestをdata化した。Node／Rustは同じcheckpoint IDを読み、API別CLI modeではなく
generic session runner／handler registryで既存checkpointを選択する。

checkpoint dataはコード分岐ではなく、dataとして表現する。

例:

```yaml
id: bgi-seventeenth-win32-call
extends: bgi-sixteenth-win32-call
dispatch:
  module: kernel32.dll
  symbol: EncodePointer
  occurrence: 2
expected:
  arguments:
    - 0x100A9608
  result:
    logicalToken: 0xEC010002
  guestWrites:
    - address: 0x100FF62C
      value: 0xEC010002
nextStop:
  module: kernel32.dll
  symbol: EncodePointer
  arguments:
    - 0x7F020000
```

harness本体は以下へ分離する。

- session builder
- manifest loader
- generic dispatcher
- semantic assertions
- evidence serializer

### P1-5. testをsemantic contractとevidence snapshotへ分離する

#### Semantic contract — 必須gate

- API identity
- argument decode
- calling convention
- stack cleanup
- EAX／EDX／flags
- LastError
- guest write
- callback order
- object identity
- fault class
- no partial write
- lifecycle／cleanup

#### Evidence snapshot — 意図的更新

- full report
- SHA-256
- exact generation
- exact owner ID
- exact instruction count
- trace size
- exact caller RVA
- environment detail

内部リファクタリングによりsnapshotが変化しても、semantic contractが同一なら更新可能とする。snapshot digestを通常の意味論gateと混同しない。

### P1-6. test hermeticityとfuzzを改善する

- clean checkoutで `cargo test --workspace --locked` が成立する構成を目指す。
- `.work/toolchain-spike/*.exe` 前提のtestは、fixture generationを明示的なtest dependencyへする。
- PRでは短時間seed regression／fuzz smokeを行う。
- scheduled jobで長時間fuzz campaignを行う。
- PE offset、RVA、section overlap、import string、range arithmetic、mapping sequence、delta acknowledgementを対象にする。

### P1-7. CIを再編する

推奨job:

```text
static-and-rust
  docs / JS syntax / fmt / clippy / unit / property

adapter-build
  v86 source build / adapter ABI / artifact hash

browser-e2e
  Worker vertical slice / Chrome / pixel / audio / storage

windows-differential
  public PE fixture oracle / ABI / callback / path / GDI

scheduled
  fuzz / benchmark / soak / current Chrome canary
```

必須改善:

- GitHub Actionsをcommit SHAへpinする。
- `dtolnay/rust-toolchain@master` を使用しない。
- Chromeをpinned baselineとcurrent stable canaryへ分ける。
- Chrome、Node、Rust、LLVM、adapter versionをartifactへ保存する。
- failure時にexecution report、memory stats、browser console、screenshot、traceをuploadする。
- cargo cacheを明示する。
- private BGI canonical runは、利用可能なローカル／self-hosted環境で定期実行し、binaryではなく結果manifestを保存する。
- private BGIから得た一般契約をpublic fixtureへ還元する。

現時点で、mainの最新commitに対する成功済みworkflow runを確認できていない。workflow定義の存在とmainがgreenであることを同一視しない。

### P1-8. 文書のsingle source of truthを作る

現在状態の正本を、`docs/current-checkpoint.json`または後継manifestへ集約する。

生成対象:

- README current status
- supported call一覧
- next checkpoint
- npm command一覧
- current generation
- compatibility summary
- evidence index
- phase status
- open risk status

手書き文書に残すもの:

- 設計判断
- rationale
- contracts
- invariant
- risk
- acceptance criteria
- revisit condition

APIごとの詳細文書はimmutable evidence recordとして保持してよいが、README、AGENTS、roadmapへ同じ数値と長文を複製しない。

---

## 6. P2指示 — runtime kernel成立後

以下はP0／P1を通過してから開始する。

1. User32／GDIの共通object model
2. message queue、wait object、input routing
3. VFS、Windows path、sharing、registry
4. source snapshotとwritable overlay
5. save persistenceとdurability
6. DirectSound／WinMM frontend
7. AudioWorklet transportとclock ownership
8. bounded D3D9 COM／resource graph
9. DirectInput 8
10. font／Windows-932／metrics
11. title scenario replay
12. save／load／choice regression
13. full-play soakとcomplete scenario coverage

---

## 7. エージェント向け実装順序

### Step 1 — 現状固定

- `npm run check` とprivate corpusの再現条件を記録する。
- current checkpoint manifestを保存する。
- 17 callのsemantic contractを抽出する。
- 新しい専用API flagを追加しない。

### Step 2 — dispatcher最小縦断

以下の異なる形のAPIを5件選び、共通schemaへ移す。

- zero-argument scalar return
- output pointer write
- VOID return
- multi-argument BOOL return
- logical pointer／handle return

推奨候補:

- `GetCurrentProcessId`
- `QueryPerformanceCounter`
- `GetStartupInfoW`
- `TlsSetValue`
- `EncodePointer`

### Step 3 — RuntimeState導入

- controlled process／thread／clock／TLS／module／heapをRuntimeStateへ移す。
- dispatcherがRuntimeStateのみを参照する。
- LastError policyを明示する。
- callback frame stackを追加する。

### Step 4 — 17 call migration

- 残り12 callをgeneric dispatcherへ移す。
- checkpointをmanifest化する。
- bespoke session functionsを削減する。
- old golden evidenceとsemantic equivalenceを確認する。

### Step 5 — memory transaction置換

**完了（2026-07-12）:** changed-state／frame before-image journalへ置換し、
import、process bootstrap、reconciliation／release rollback、17-call evidenceを
維持した。tracked peak frame payloadは4 MiB baselineに対して開始0 bytes、
1／2 changed pageで4,096／8,192 bytesである。

- COW／journal設計を実装する。
- import、bootstrap、reconciliationのrollback testを維持する。
- peak memoryを計測する。

### Step 6 — memory authorityとwriteback

**完了（2026-07-12）:** ADR-003でRust-owned `GuestMemory`をbyte authority、
v86 pagingをexecution mirrorとしてAcceptedにした。bounded slice後のPTE
Dirty scan、owner／generation付きatomic multi-page writeback、binary
reflection／delta／writeback transportを実装し、連続2 sliceのstack／heap／
surface／RWX writeをgeneration `18 -> 21`で検証した。

- ADRを作る。
- automatic multi-page dirty writebackを実装する。
- binary transportへ移す。
- stack／heap／surface fixtureで検証する。

### Step 7 — Chrome Worker vertical slice

**完了（2026-07-12）:** zero-import Rust/Wasm coreとsource-v86 mirrorを
Chrome Runtime Workerへ載せ、controlled PE32を46命令／10 slicesで実行した。
2 deferred host completions、depth-2 nested callback、7 automatic writeback
batches、Canvas RGBA、880 Hz Web Audio、IndexedDB `0x5a`、clean exitを同じ
sessionで通した。pending host request cancelと4-instruction watchdogはいずれも
dangling work 0。30分／11,326-session stressはfailure 0、heap slope
136 bytes/min、maximum heartbeat gap 16.28 msだった。

- Rust/Wasm packaging
- Runtime Worker
- v86 execution
- generic dispatcher
- async host service
- callback
- pixel
- audio pulse
- persistent byte
- cancellation／watchdog

### Step 8 — benchmarkとengine gate

**完了（2026-07-12）:** commit `a0d99e5`のclean worktreeからstable Chrome
`150.0.7871.115`でversion 1 matrixを測定し、threshold failure 0／`go`を得た。
integer 134.1M ips、x87 52.5M ips、SSE2 83.7M ips、production generic API
11,279 roundtrips/s、active steady memory 14.58 MB、hidden-tab ratio 0.985、
AudioWorklet max gap 51.315 msである。report SHA-256は
`e50ad355361f91e01baae1dcb6f62fb813b2ddc1202466682c60eda7e959a1e7`。
v86を交換可能なbaselineとして継続し、CIは同じtracked thresholdを実行してreport／
screenshot／artifact manifestを保存する。

- required metricsを取得する。
- v86継続／交換判断を記録する。
- performance regression baselineをCIへ追加する。

### Step 9 — loader／scheduler／object model

**Foundation 1完了（2026-07-12）:** `RuntimeState`へversion 1
`RuntimeObjectTable`を導入した。type 8／generation 8／slot 16のhandle encoding、
separate object／handle lifetime、multiple process／thread handles、close／slot reuse、
stale／double-close／type mismatch、process／thread wait signal、subsystem-owned
module／heap policyをRust contractで固定した。既存`0x4D010001`／`0x68010001`と
BGI 17-call digestを維持し、Chrome Worker／benchmarkも回帰なしである。

**Foundation 2完了（2026-07-12）:** process-owned `RuntimeModuleGraph`を
`RuntimeState`へ導入した。bounded ASCII canonical basename、main EXE／guest DLL／
runtime system module class、dependency cycle rejection、process／runtime／static import／
dynamic thread reference ownership、registered／ready／attached／detached state、
dependency-first attach／strict reverse detachをatomic Rust contractで固定した。guest DLL
mapping／export／forwarder／`DllMain`は未実装のまま、Worker／BGI／benchmarkを維持した。

**Foundation 3完了（2026-07-12）:** controlled process／initial threadへ
`bootstrapping/initialized -> running -> exit-requested -> exited` lifecycleを導入した。
scheduler `initialized`／`terminating`、explicit start、exit request／completion、pending
host／callback cleanup、request中のnonsignaled object、thread／process terminal signalを
targeted atomic planで固定した。RuntimeState全体はcloneせず、Worker／BGI／benchmarkを
維持した。

**Foundation 4完了（2026-07-12）:** controlled second thread registryを導入した。
unique TID／typed handle、per-thread LastError／TLS／callback／scheduler lane、explicit
selection、thread-only exit、TID昇順のall-lane process terminationをatomic Rust contractで
固定した。CPU context／stack／TEB、wait queue、DLL thread callbackは未実装である。

**Foundation 5完了（2026-07-12）:** typed process／thread objectをcontrolled wait queueへ
接続した。monotonic registration、object-indexed FIFO、`WaitingObject` lane、別thread exitの
signalによるdeterministic wake／exact event、already-signaled path、process-exit cancellationを
atomic Rust contractで固定した。wait-any／wait-all／timeout／message wakeは未実装である。

**Foundation 6完了（2026-07-12）:** bounded per-thread raw message FIFOを導入した。
process-global sequence、opaque payload、empty receiveの`WaitingMessage`、first enqueueだけの
runnable wake／exact event、owner FIFO dequeue、reblock／rewake、exit cleanupをatomic Rust
contractで固定した。User32 filter／WM_QUIT／Send／dispatchは未実装である。

**Foundation 7完了（2026-07-12）:** existing typed logical private heapをbounded Rust
`GuestMemory` backingへ接続した。32 x 16 MiB controlled arena、8-byte first-fit、monotonic
allocation ID、zero／deterministic nonzero initialization、free poison／coalesce／reuse、invalid／
double free、typed heap retirement、exit-request中のbacked／metadata-only process teardownを一つの
`ExecutionSession` transactionで固定した。Guest `HeapAlloc` dispatchとbroad allocator policyは
未実装である。

- module graph
- process／thread lifecycle
- handles
- waits
- message queue
- heap backing
- DLL lifecycle

### Step 10 — BGIへ戻る

BGIをgeneric runtime pathへ戻し、first unsupported pointを再測定する。以後はAPI call番号ではなく、runtime capability clusterとvertical checkpointを単位に進める。

---

## 8. P0完了のDefinition of Done

以下をすべて満たすまで、広いWin32／graphics実装へ進まない。

- [x] 現在の17 callが一つのgeneric dispatcher経路を通る。
- [x] APIごとの専用CLI flagがproduction executionに不要である。
- [x] checkpointはmanifest／data tableで表現される。
- [x] ProcessState／ThreadState／Scheduler／LastErrorが存在する。
- [x] host async operationをwaiting／completionとして扱える。
- [x] transactionが全GuestMemoryをcloneしない。
- [x] guest-originated multi-page writeが自動同期される。
- [x] byte authorityがADRで決定されている。
- [x] production memory transportがbinaryである。
- [x] Chrome Runtime Worker内でguest CPUが動作する。
- [x] pixel、audio pulse、persistent byteを一つのsessionで通す。
- [x] cancellation、watchdog、nested callbackが通る。
- [x] v86 benchmarkとgo/no-go判断が保存されている。
- [x] 30分stressのmemory slopeとresponsivenessが記録されている。
- [ ] semantic testsとevidence snapshotsが分離されている。
- [x] CI artifactにtool versionとfailure evidenceが残る。

上の完了項目はcontrolled 17-call runtime-kernel checkpointの範囲を示す。
host completionはRust scheduler eventとしてmodel／test済みだが、Chrome Workerの
browser serviceには未接続であり、後続のWorker項目を完了扱いにはしない。

---

## 9. 禁止事項

- API対応数をfull-play進捗率として報告しない。
- unsupported trapをAPI実装済みとして数えない。
- BGI専用値をgeneral Win32 semanticsとして扱わない。
- native host pointer／handle／cookieをguestへ漏らさない。
- Promiseをguest call stackへ直接露出しない。
- guest memory authorityを曖昧なままheap／graphicsへ進まない。
- 全memory clone transactionを大容量runtimeへ持ち込まない。
- JSON byte arraysをproduction hot pathに使用しない。
- main threadでguest CPU loopを実行しない。
- title hash／EIPによる処理skipを追加しない。
- CI workflowの存在をgreen statusの証拠として扱わない。
- current stateを複数文書へ手作業で複製しない。
- 未測定の性能を楽観的に仮定しない。
- multi-title／Win64／public release scopeをactive workへ持ち込まない。

---

## 10. 最終判断

Vineの基礎技術、検証品質、fail-closed設計、証拠管理は高く評価できる。現在までの17 callは無駄ではなく、generic runtimeを設計するための有力なregression corpusである。

最大の問題は、検証ハーネスの成長をruntimeの成長と取り違え始めていることである。

したがって、最終判断は次のとおりとする。

> **Conditional Go。**
>
> 現在の方向性を「BGIの次のAPIを専用経路で一件ずつ追加すること」と解釈するなら、full play達成は困難である。
>
> 現在の証拠を維持しつつ、generic dispatcher、authoritative RuntimeState、scalable memory ownership、Chrome Worker vertical slice、v86 performance gateへ切り替えるなら、固定タイトルfull playは依然として成立可能である。

次の成功条件は「18番目、19番目のAPIへ進んだこと」ではない。

> **現在の17 callを、API固有の実行モードなしで、同一のproduction runtime kernelから再現できること。**

これを満たした時点で、Vineは「精密な成立性実験」から「完成可能性を持つ固定タイトル向けbrowser-native Windows runtime」へ移行したと判断できる。
