# 目標アーキテクチャ

<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [`current-checkpoint.json`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
<!-- END CURRENT CHECKPOINT REFERENCE -->

## 1. 文書情報

- ステータス: Proposed
- 目的: 交換可能な技術と壊してはいけない境界を分離する
- 注意: 大半のコンポーネントは設計案である。実装済み範囲、non-claims、次の停止点は上のmanifest参照を正本とする

## 2. アーキテクチャ原則

1. **保存元と実行状態を分ける。** 元の `.exe` / `.dll` の保存バイト列と hash は変えない。relocation、IAT 解決、section zero-fill 等の必須 loader 処理は guest memory 上だけで行い、title 固有の code patch は行わない。
2. **guest を未信頼とする。** PE parser、CPU、API marshalling、VFS のすべてで、guest address、length、handle、path を検査する。
3. **main thread を塞がない。** guest CPU と scheduler は Worker 側に置き、DOM event と rendering presentation だけを必要に応じて main thread へ渡す。
4. **同期 API を async host に漏らさない。** guest thread の suspend / resume を runtime scheduler が管理し、Web API の Promise を guest stack へ直接露出しない。
5. **compatibility をデータとして扱う。** CPU feature、PE feature、API status、title result を versioned metadata にする。
6. **正確性を先に測る。** interpreter を成立性の基準候補とし、DBT、generated Wasm、parallel execution は benchmark と differential test 後に選ぶ。
7. **縦断経路を早く通す。** 各 subsystem を広く作る前に、合成 PE から pixel、audio、save まで最小の一経路を完成させる。
8. **Rust と TypeScript の authority を分ける。** Rust runtime kernelはguest-visibleなWindows stateとscheduler policyを所有する。TypeScriptはbrowser resource、DOM lifecycle、host request deliveryを所有し、Windows stateのmirrorを作らない。

### 2.1 Runtime authority

| State | Authority | Mirror / adapter |
|---|---|---|
| guest virtual memory bytes、region、protection、owner | Rust `GuestMemory` | execution engine memory |
| process、thread、handle、module、heap、wait、message、TLS／FLS | Rust runtime kernel | なし |
| guest scheduler state、wait registration、host request identity | Rust runtime kernel | Worker event loop |
| CPU context | per-thread runtime-owned context record | execution engine state buffer |
| Windows path、mount、snapshot／object descriptor、file／directory、handle、cursor、share state | Rust `RuntimeFiles` | storage object identity |
| Fingerprint済みimmutable source bytes、committed save／registry generation | Local Vine server | Rust logical source／transaction state |
| Canvas、WebGL／WebGPU、Audio resource | TypeScript browser backend | Rust logical resource identity |
| DOM input、permission、user activation、browser lifecycle | TypeScript | Rust scheduler event |
| Win32 argument／pointer validationと同期semantics | Rust | なし |
| async Web API開始、取消、completion transport | TypeScript | Rust host request record |

TypeScriptはhandle、wait、message、LastError、module stateを更新しない。RustはDOM node、
`AudioNode`、GPU object、browser file handleを保持しない。境界を通過できるidentityはversioned
request ID、thread ID、logical resource IDであり、payloadはcontractごとの上限を持つ。

async operation開始時のguest inputは開始前にRust-owned bufferへcopyする。completion時にguestへ
writeするpointerは、allocation identityでpinするか、current mapping／owner／protection／lengthを
再検証する。現在のBrowser fixtureはscalar wordsをowned payloadへcopyするため、async中にguest
pointerを保持しない。

scheduler transitionはRustだけが決定する。Rustは`waiting-host`とrequest／thread identityを発行し、
TypeScriptはbrowser eventをdeliveryする。completion後の`runnable`または`in-callback`復帰もRustが
検証してからCPU contextへ適用する。Worker event loopは独自にguest threadをwakeしない。

Worker protocol version 7のstate machineは、Rust scheduler laneを`ready`、`running`、
`budget-yield`、`memory-fault`、`api-trap`、`thread-switch`、`thread-exit`、`waiting-host`、
`host-completion`、`guest-callback`、`callback-return`、`stopping`、`stopped`の閉じたtransition
graphで駆動する。illegal transitionはstate mutation前に
拒否する。host deliveryはglobal optionではなくrequest ID／thread ID／cancel state／completion
policyを持つtableへ置き、wrong-thread、duplicate、cancel後のlate completionを区別して拒否する。
terminal reportは必ず`stopped`かつpending record zeroとし、watchdogだけが原因thread、active
request、直前transitionのdiagnostic contextを持つ。

Version 7はsessionごとに`full`／`summary` evidence levelを明示する。`full`はSetでsequence identityを
検証し、terminal時に一度だけsortしたordered call payloadを公開する。`summary`はprivate BGI benchmarkに
限定し、sequence uniquenessとcall countを保持しつつ詳細call objectを生成しない。これはobserver payloadの
所有境界であり、Rust-owned Windows state、guest memory、API validation、terminal digestを省略・複製しない。

Version 6では、同期guest callbackの完了がouter API returnだけでなく、同じdepthで次の
`guest-callback`へatomic replacementできる。Rustはretireするframe IDと新しいframe ID、thread、
completion policyを検証し、callback stackとlogical Windows stateをcommitしてからWorkerへ次の
targetを返す。WorkerはCPU contextを新frameへ切り替えるだけで、window lifecycleやcallback identityを
所有しない。Final completionに付くlogical window snapshotもRust authorityのread-only observationである。
Callback引数の構造体は下降するx86 stackと分離したprocess-permanent scratch arenaのdepth slotへ置き、
slot identity、mapping、bytes、stack frame、callback mutationをpreflight後に一つのtransactionで進める。

test／evidenceはguest semantic、required ordering、opaque identity、performance、host
nondeterministic value、artifact hashへ分類する。schedulerの独立eventはpartial orderを許すが、
request-before-completion等のrelationは必須とする。whole-report hashはartifact identityであり、
historical production ABIにしない。詳細は[evidence-classification.md](evidence-classification.md)を正とする。

protocol version 7のproduction CPU境界はgeneric `ExecutionEngine`とする。上位が扱うのはcaller
budget、structured yield／trap／fault／halt、versioned opaque CPU contextの生成／保存／復元、guest mapping operation、
owner／address付きchanged-page batch、data-TLB／instruction-fetch／generated-code effectだけである。
現在のsource-v86 implementationはnon-PAE PTE、Dirty bit scan、private offset、native stop reason、
`full_clear_tlb()`をadapter内に閉じる。profileはsource revision、Wasm SHA-256、CPU-context ABIを
pinし、build時とWorker guest startup前の両方で不一致を拒否する。

Runtime Workerはthread IDごとのopaque CPU contextを保持し、switch前にlive contextを保存してRustが
選択したexecutable laneだけを復元する。Host completionはrequest ownerのsuspended contextへ適用し、
Rust schedulerがそのthreadをexecutableに戻すまで実行しない。Controlled Browser sessionはinitial
threadのhost wait中にsecond guest threadを実行／exitし、その後initial contextへ復帰する。CPU layoutは
adapter内に閉じ、TypeScriptはWindows thread stateの第二authorityを持たない。

## 3. 論理構成

```text
User-selected title directory / package
                  |
                  v
+-------------------------------- Browser shell (main thread) -----------------------------+
| import UI | title selection | canvas presentation | input | lifecycle | diagnostics UI |
+-------------------------------+---------------------------+-------------------------------+
                                | typed messages / event queues
                                v
+------------------------------- Runtime Worker -------------------------------------------+
|  TypeScript event delivery / watchdog / trace                                             |
|                                                                                           |
|  +-----------------+   +-----------------+   +--------------------+                       |
|  | PE/DLL loader   |-->| guest address   |<->| x86 execution      |                       |
|  | import resolver |   | space / memory  |   | engine adapter     |                       |
|  +--------+--------+   +--------+--------+   +----------+---------+                       |
|           |                     |                       |                                  |
|           +---------------------+-----------+-----------+                                  |
|                                             v                                              |
|                                  ABI gateway / API dispatcher                              |
|                                             |                                              |
|           +------------------+--------------+---------------------+                        |
|           v                  v                                    v                        |
|  synthetic Win32 DLLs   GDI / DDraw / D3D9 / DInput / DSound   VFS / registry             |
|           |                  |                                    |                        |
|           +------------------+--------------+---------------------+                        |
|                                             v                                              |
|                                        host services                                      |
+-------------------------+-------------------+-------------------+--------------------------+
                          |                   |                   |
                          v                   v                   v
                  Canvas/WebGL/WebGPU   AudioWorklet/Web Audio   Local Vine server
```

この図は責務の境界を示す。実装が単一 Wasm module、複数 package、TypeScript、Rust、C/C++ のどれになるかは ADR で決める。

## 4. 実行フロー

### 4.1 Import から entry point まで

1. Browser shell が user gesture により選択された file handle または package stream を取得する。
2. Import service が file 数、容量、path、archive 展開率、PE signature を上限付きで検査する。
3. 元 file を read-only source volume に登録し、`.exe`／`.dll` は content-addressed managed blob 等の stable byte snapshot に固定して title fingerprint を作る。large asset の lazy source handle とは lifetime を分ける。
4. PE loader が main module を parse し、machine、subsystem、imports、CPU feature 候補を inventory にする。
5. Policy engine が unsupported architecture、driver、executable form、resource limit を分類する。静的 preflight 通過を互換性保証にしない。
6. Guest address space が image、stack、heap、PEB／TEB 相当領域を割り当て、flat segment、FS base、process parameters、initial register／stack を adopted bootstrap profile に従って初期化する。
7. Loader が section、relocation、imports、TLS、同梱 DLL を guest memory 内に配置する。
8. Synthetic DLL resolver が実装 export を API thunk address に結び付ける。
9. DLL attach、TLS callback を仕様順に呼び、最後に executable entry point へ制御を渡す。
10. Scheduler が命令 slice ごとに CPU を進め、event、timer、host response、watchdog を処理する。

### 4.2 API call

```text
guest CALL / JMP import thunk
  -> execution engine trap
  -> ABI schema lookup
  -> pointer / length / handle validation
  -> compatibility implementation
       -> immediate: result + LastError を返す
       -> deferred: guest thread を WAITING にし host request を発行
  -> host response / event
  -> guest thread を RUNNABLE に戻す
  -> return register / stack を ABI 規則どおり復元
```

API implementation は `Promise` を guest へ見せない。deferred operation の途中でも、別 guest thread、message、cancel、timeout をRust schedulerが扱い、TypeScriptはbrowser completionをeventとして返す。

現在のcontrolled sliceはfixed service／callback／entry return、Rust-owned stack、typed one-page TEB／PEB／process-parameters／command-lineを通す。native FS contractとzero-base faultを保持し、CPU stateのFS baseをTEBへ向けてmeasured command-line observationとentry completionまで実行した。single processはexplicit lifecycleとcontrolled two-thread registryを持ち、logical laneごとのLastError／TLS／callback／scheduler state、thread exit、all-lane termination、one-object wait／wake、bounded raw message FIFOを固定する。Typed logical private heapは32個のordinary 16 MiB guest slot、8-byte allocation、free／reuse、exit-request teardownへ接続済みであり、active `HeapAlloc` policyはflags `0`／`HEAP_ZERO_MEMORY`、ordinary nonzero `1..=16 MiB`、およびexplicit exact 64 MiB variantを受理する。Exact large variantはprocess-permanent address recordとsingle live backingを`RuntimeHeaps`が所有し、reserve-only allocation後のgeneric page-fault serviceで4 KiBずつGuestMemoryへtransactional materializeする。`HeapFree`／`HeapSize`は到達済みflag subsetを維持する。second threadのCPU context／stack／TEB、general ABI schema、stack overflow、wait-any／wait-all／timeout、User32 filter／dispatch、zero-size／other flags／realloc、intermediate／multiple large allocation等のheap API、general process parametersは未実装である。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)、[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md)、[runtime-thread-registry.md](runtime-thread-registry.md)、[runtime-wait-queue.md](runtime-wait-queue.md)、[runtime-message-queue.md](runtime-message-queue.md)、[runtime-heap-backing.md](runtime-heap-backing.md)を正とする。

### 4.3 Host から guest callback

Window procedure、timer procedure、enumeration callback 等は host event から guest code へ戻る。DirectSound notification は callback ではなく、runtime の event handle を signal して wait 中の guest thread を wake する。

- callback target が executable page 内にあるかを検証する。
- calling convention と引数 schema を metadata から組み立てる。
- 現在の API call へ再入可能か、queue すべきかを API policy に記録する。
- callback 用 frame を guest stack に積み、sentinel return thunk まで CPU を実行する。
- nested callback の深さと命令 budget を制限する。
- exception、stack imbalance、destroyed handle は structured failure として trace する。

callback と再入は後付けしない。Phase 1 の vertical slice で実証する。

controlled callback／entry return ingressでは、sentinel kindをcontrol address rangeから推測せず、typed exact ID／address metadataで分類する。frame IDはactive outer thunk、saved ESP／return address、target／value、callback sentinel identityを保持する。outer returnでその1 frameをretireし、process-entry sentinelはframeとは別のcompletion kindとして扱う。Production callback stackはbounded LIFOとしてnested entryとsame-depth replacementを扱うが、arbitrary cross-thread callback、unbounded reentrancy、general callback fault recoveryは実装済みとしない。

## 5. Browser execution topology

### 5.1 推奨 baseline

| Context | 責務 | 禁止事項 |
|---|---|---|
| Main thread | shell UI、DOM input、user activation、canvas presentation、permission prompt | guest 命令 loop、長い parse、同期 wait |
| Runtime Worker | Rust kernel／CPU adapterの駆動、browser request delivery、watchdog、trace | DOM直接操作、guest-visible Windows stateの所有 |
| AudioWorklet | deadline に沿った PCM consume、sample clock、underrun counter | guest API、file parse、重い allocation |
| Local Vine server | same-origin UI／API配信、immutable source read、SQLite save transaction | Windows path／handle semantics、guest pointer参照、任意directory公開 |
| Optional storage worker | 将来のOPFS／IndexedDB adapterが要求するtransaction／snapshot | guest-visible file stateの所有、guest pointer参照 |

OffscreenCanvas、SharedArrayBuffer、Atomics、OPFS sync access 等の採用は browser matrix と cross-origin isolation のコストを測って決める。これらがない環境では、copy queue または support Tier の引き下げを選べるようにする。

### 5.2 Guest thread model

最初の候補は、複数の Windows thread を一つの Runtime Worker 上で cooperative に schedule する方式である。

- 各 thread は register set、FPU state、stack、TEB／FS、LastError、Win32 TLS slot、COM apartment、wait state を持つ。
- 命令 count、virtual time、API boundary、wait で yield する。
- Windows wait object と message queue を runtime 内で解決する。
- browser main thread や Promise の完了を、runtime event として enqueue する。
- priority、APC、alertable wait、true parallelism は corpus の必要性を測って拡張する。

この方式は data race の再現性を高める一方、multi-core 性能や一部 timing-sensitive code を制約する。SharedArrayBuffer を使う 1 guest thread / 1 Worker 方式は、Phase 1 の benchmark と semantic test を通った場合だけ採用候補にする。

version 1 Rust lifecycle foundationはprocess `bootstrapping／running／exit-requested／exited`、thread `initialized／running／exit-requested／exited`、scheduler `initialized／terminating`を所有する。controlled registryはsecond TID／typed handle、thread-local LastError／TLS／callback／logical affinity／COM apartment／lane、explicit selectionを加え、process terminationをTID昇順thread signal＋process-last signalとして最大3 object recordsへtargeted atomic commitする。COM apartmentはmodelとsuccessful initialization countだけをthread recordで所有し、COM object identity／refcountを所有する`RuntimeCom`とは分離する。wait queueはmonotonic registration／object FIFO、`WaitingObject`、thread signalによるrunnable wake／exact event、exit cancellationを所有する。message queueはper-thread bounded FIFO、global sequence、`WaitingMessage`、first enqueue wake、exit cleanupを所有する。CPU context／second-thread stack／TEB、last-thread semantics、wait-any／wait-all／timeout、User32 message semantics、cross-thread COM marshalingは未実装である。詳細は[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md)、[runtime-thread-registry.md](runtime-thread-registry.md)、[runtime-wait-queue.md](runtime-wait-queue.md)、[runtime-message-queue.md](runtime-message-queue.md)、[com-iunknown-foundation.md](com-iunknown-foundation.md)を正とする。

`runtime.rs`はsubsystem宣言とpublic re-exportだけのfacadeである。実体のaggregate rootは
`runtime/kernel.rs::RuntimeKernel`で、既存caller向けの`RuntimeState`はそのtype aliasとする。
この最終ownership auditをarchitecture refactor checkpoint S13とする。
`RuntimeKernel`のdirect fieldはprocess state、address allocator、各private subsystem ownerだけであり、
subsystem collectionやscalar allocatorを持たない。Process lifecycle／environment／directory／standard
handle／policyは`runtime/process.rs::RuntimeProcessState`が所有する。既存diagnostic JSONは
`RuntimeProcessView`がheap、module、synchronization、timer resolution、TLS／FLS ownerをimmutable borrowして従来の
`process` shapeへflattenするが、このviewはmutation authorityではない。

Immutable source stateはprivate `runtime/files.rs::RuntimeFiles`が所有する。Manifest、Windows-932-aware path index、monotonic file ID、cursor、pending readはkernelへmutable collectionとして公開しない。Typed file handle generationは`RuntimeObjectTable`、file payload lifetimeは`RuntimeFiles`が所有し、kernelはopen／last close、scheduler request／completion、process exitを全owner preflight後にcommitする。詳細は[runtime-vfs.md](runtime-vfs.md)を正とする。

Mutable thread records、process-wide TLS index／FLS slot registries、TLS／FLS／callback-frame ID allocatorsは
private `runtime/thread.rs::RuntimeThreads`だけが所有する。Registration、LastError／TLS／FLS、callback
frames、initial-stack metadata、lifecycle transitionはmutation-free thread/resource planを使う。Aggregateは
scheduler、object、message、wait、memoryをpreflightしてからinfallible commitを呼び、thread map／record／
resource allocatorを直接mutateしない。Existing process TLS/FLS fieldsとroot next-ID fieldsはborrowed
serialization viewである。Scheduler laneはthread collectionへ統合せずscheduler authorityに残す。

Mutable scheduler lanes／current selector／event FIFO／pending host-request table／request ID allocatorは
private `runtime/scheduler.rs::RuntimeScheduler`、wait registration／object FIFO／registration ID allocatorは
private `runtime/waits.rs::RuntimeWaits`だけが所有する。Wait stateはscheduler serializationへflattenし、host
requestとwait allocatorはroot `registries.hostRequests`／`nextHostRequestId`／`nextWaitRegistrationId`への
borrowed compatibility viewで既存shapeを維持するが、authorityは各private ownerだけにある。Host start／completionはlaneとrequest identityを
同じmutation-free planで検査してtargeted commitする。Thread exitはthread／message／waitとscheduler-owned
host cancellation planをcross-subsystem preflightしてからcommitし、aggregateはscheduler／wait／host-request
mapを直接mutateしない。

Mutable dynamic endpoint table、encoded-pointer token table／source index、pointer namespace／prefix policy、
token ID allocatorはprivate `runtime/registries.rs::RuntimeRegistries`だけが所有する。Endpoint registrationは
aggregateがprocess dynamic-export rangeを渡し、subsystem plan／address preflight／infallible commitを通す。
Pointer allocationはtokenとsourceの両indexおよびcounterをsame ownerでcommitする。Root pointer policy／
source／counter serializationはborrowed compatibility viewであり、同じ`registries` JSON objectにある
host requestは引き続き`RuntimeScheduler` authorityである。

Mutable logical heap metadata、GuestMemory backing metadata、automatic private-heap slot allocatorはprivate
`runtime/heaps.rs::RuntimeHeaps`だけがco-ownする。`RuntimeProcessView`がimmutable borrowをflattenして既存
`process.heaps` serializationを維持し、root `nextPrivateHeapSlot`はborrowed compatibility viewとする。
Inspectionはimmutable borrowに限定する。Automatic creationはheap identityとallocator cursorを同じ
mutation-free planへ束ね、allocation、free、destructionもsubsystem planを作る。Aggregateはlifecycle、
typed object、process address allocation、GuestMemory transactionをpreflightしてからtargeted commitする。
Aggregateはheap／backing map／slot allocatorを直接mutateせず、failed object registrationではheapと
allocatorを両方不変にする。`RuntimeKernel`自身にはdirect scalar allocator fieldを残さない。詳細は
[runtime-heap-backing.md](runtime-heap-backing.md)を正とする。

Mutable logical COM object／interface／IID／pointer／endpoint indexとshared reference countはprivate
`runtime/com.rs::RuntimeCom`だけが所有する。COM identityはWin32 HANDLE用`RuntimeObjectTable`へ混在させず、
aggregateはCOM plan、process address allocation、GuestMemory transactionをpreflightしてからtargeted
commitする。Final `Release`は全interface index、guest mapping、memory owner、allocation IDを同時に退役
させる。Child objectはparent interfaceを一reference保持し、cascading final releaseでは全COM backing、
address allocation、関連logical resource retirementをpreflightしてから一括commitする。Browser-side Web
API resourceは別namespaceの非所有associationであり、COM refcount authorityを持たない。詳細は
[com-iunknown-foundation.md](com-iunknown-foundation.md)を正とする。

Mutable process module recordsはprivate `runtime/modules.rs::RuntimeModuleRecords`、graph、mapped
images、import bindings、runtime export tableはprivate
`runtime/modules.rs::RuntimeModules`だけが所有する。既存`process.modules`／root `moduleGraph`
serializationはflatten adapterで維持し、inspectionはimmutable borrowに限定する。Registrationは
mutation-free graph planを作り、typed objectをpreflightした後にrecords／graphをinfallible commitするため、
failed object registrationでrollback mutationを行わない。Guest import bindingもmemory work成功後にだけ
candidate graphとmetadataをcommitする。Aggregateはこれらのcollectionを直接mutateしない。DLL lifecycle
phase、attach／unwind／detach operation、callback progress／failure identityもprivate
`RuntimeDllLifecycle`が所有する。Subsystemはnext callback／graphをmutation-free planし、aggregateは
thread stack／scheduler laneをpreflight後にtargeted commitする。連続callbackはpop＋pushをatomic
replacementにし、中間のempty stack／Runnable stateを公開しない。Active operationはgraph／image／binding
mutationをsame owner内で拒否する。詳細は
[runtime-module-graph.md](runtime-module-graph.md)、
[runtime-module-images.md](runtime-module-images.md)、
[runtime-dll-lifecycle.md](runtime-dll-lifecycle.md)を正とする。

Process-owned critical-section／mutex collectionsとnamed indexは`runtime/synchronization.rs`のprivate
`RuntimeSynchronization`が所有する。`RuntimeKernel`はprocess／selected-thread／scheduler lifecycleを
preflightしてcurrent thread identityを渡すcoordinatorに限定する。Critical-section and mutex transitionsは
subsystem内のmutation-free plan後にcommitし、mutex create／last closeはgeneric object tableと両方を
preflightする。Object tableはpayload-backed close-handle objectsに共通のopaque
`RuntimeCloseHandleObjectMutationPlan`を返し、kernelはobject／synchronization
collectionをcloneまたは置換せず、両planの成功後にownerのtargeted commitだけを呼ぶ。詳細は
[runtime-synchronization.md](runtime-synchronization.md)を正とする。

Mutable WinMM timer-resolution request counts／generationとwaitable-timer ID／reset-mode payloadはprivate
`runtime/timers.rs::RuntimeTimers`だけが所有する。Effective periodとtotal countは同じmapからderiveし、
compatibility serializationはimmutable snapshotを返す。Begin／endはexpected generationとtarget countを
含むmutation-free planを使い、stale／duplicate commit、unmatched end、capacity／counter exhaustionを
mutation前に拒否する。Process exitはprocess／thread planとtimer teardownを全てpreflightした後にrequest
tableを一回でretireする。

Waitable timerのtyped handle／object ID／wait stateは`RuntimeObjectTable`が別authorityとして所有する。
Creation、last close、process teardownはtimer payload planとopaque close-handle object planを両方preflightし、
timer map、object index、handle slot、allocatorを部分更新しない。Browser scheduler／audio adapterはrequest
tableもwaitable-timer identityも所有せず、WorkerはRust completionのimmutable snapshotだけを運ぶ。詳細は
[winmm-compatibility-policy.md](winmm-compatibility-policy.md)と
[bgi-create-waitable-timer-a.md](bgi-create-waitable-timer-a.md)を正とする。

Guest-visible OS version is compatibility data, not browser capability
detection. `Kernel32CompatibilityPolicy` owns one bounded ANSI profile while
the generic dispatcher owns the size-versioned pointer contract. Accepted
structures are assembled by merging policy fields into a caller before-image
and committing one declared span, so native-unwritten padding and failure
atomicity share the ordinary `GuestMemory` boundary. TypeScript and browser
reports observe the Rust completion only; they do not import `navigator` or
host Windows version state. Details are in
[bgi-get-version-ex-a.md](bgi-get-version-ex-a.md).

### 5.3 Clock ownership

clock を browser の複数時刻源へ分散させない。

- monotonic guest clock: performance counter、timer、sleep、scheduler の基準
- audio sample clock: 音声 consume の実測。drift correction の観測源
- wall clock: date/time API 用。test では固定可能
- frame clock: presentation cadence。guest clock を勝手に進めない

background tab、debugger pause、audio suspend 後に catch-up するか freeze するかは title profile ではなく runtime policy として決め、trace に記録する。

WinMMのcapability queryとresolution requestもclock sourceそのものから分離する。
WinMM capability queryのvirtual rangeはversioned Rust compatibility policyが所有し、queryは
clockを進めずhost／browser granularityをimportしない。Resolution begin／end APIsは
dedicated Rust timer-resolution ownerがperiod、request count、matching end、process teardownを
plan／commitし、browser task timerやAudioContextをguest-visible request stateの代用にしない。
詳細は[winmm-compatibility-policy.md](winmm-compatibility-policy.md)を正とする。

## 6. Guest address space と memory

### 6.1 要求される性質

- 32 bit virtual address と unsigned wrap semantics
- page 単位の reserve / commit / protection
- low/high address、image base、stack guard、shared mapping
- unaligned access と little-endian load/store
- code page invalidation と self-modifying code の検知
- host metadata と guest bytes の分離
- guest pointer + length の overflow-safe な範囲検証
- snapshot、state hash、debug dump の取得

Wasm32 linear memory の offset と guest virtual address を同一と仮定しない。推奨案は sparse page table または region table を介し、CPU と marshaller が共通の checked memory interface を使う方式である。hot path の TLB/cache は profile 後に追加する。

現在の`vine-core` ingressは4 KiB sparse owner／region／page tableとbounded zeroed frame allocatorを実装する。typed TEB／PEB／process-parameters／command-lineを加えたgeneration `21`の11 pagesをreflectし、measured command-line observationとentry completionまで実行した。guest mutationはadapter bytesからRustへ戻らず、unmeasured bootstrap fieldsはzeroのままである。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。

### 6.2 Page state

```text
UNMAPPED -> RESERVED -> COMMITTED
                           |
                           +-> READ / WRITE / EXECUTE の論理 protection
                           +-> DIRTY / CODE / SHARED / GUARD metadata
```

host の実行権限とは無関係に、guest `EXECUTE` を論理検査する。write-to-code が発生した場合は decode/cache を無効化し、policy によって継続または診断停止する。

current ingressはRust `GuestMemory`をbyte authority、`ExecutionEngine` memoryをexecution mirrorとし、page R／W／Xとowner sealをfetch前に検査する。初回reflection、host側mapping／dirty delta、CPU側writebackはbinary control metadataとcontiguous 4 KiB payloadを分離する。bounded slice後はengineがowner／address付きchanged-page batchを返し、Rustへatomic reconcileしてgenerationを一度だけ進める。現在のv86 adapterは内部で全writable PTE Dirty bitをscanするが、Rust protocolとWorker loopはその発見方式を要求しない。host-originated通常data writeのlive roundtrip、連続2 sliceのstack／heap／surface／RWX writeback、generation／owner付きBGI reconciliationが成立した。Browser Workerはmapping-changingなreserve／commit／protect／decommit／releaseをclone registryで全件preflightしてからone batchで適用する。Mappingを変えないpage-only host deltaはlive mappingのowner／range／protection／generationを全件検証し、registry全体を複製せず同じatomic batch境界で適用する。Private heapのlazy mappingとlater dirty-only updateは同じsessionで成立する。protectはgeneric TLB／instruction-fetch／generated-code effectへ接続し、decommit／release後のstale accessはstructured stopになる。pending host delta中のexecution、generation conflict、partial batchはfail-closedであり、cancellationは同期batchの途中へ割り込まない。receiptが曖昧な場合はCPU contextを保持してRust current reflectionからmirrorを再構築する。

Execution mirrorのcapacityはRust prepared metadataをauthorityとする。v86 poolは64 KiBからpreflight済み需要へ幾何成長し、BGI profileは16,384 committed guest pages／64 MiBを上限とする。全page-table overheadとVGA MMIO holeを含むmirror ceilingは71,442,432 bytesである。成長時はfresh adapterへversioned CPU context、mapping bytes、protection、dirty stateを移し、physical identityをsemantic stateへ昇格させない。Decommit／releaseはzeroed free-listと空page-table回収へ接続し、上限超過は`v86-physical-pool-exhausted`としてmutation前に停止する。

general stack lifecycle、zero-size／other flags／reallocation等のheap API、decode-aware execute gate、snapshot generation、adapter-local O(writable-pages) scanのperformanceは未接続／未判定である。詳細は[adr-003-guest-memory.md](adr-003-guest-memory.md)、[memory-writeback.md](memory-writeback.md)、[runtime-heap-backing.md](runtime-heap-backing.md)を正とする。

Process-wide `ProcessAddressSpaceAllocator`はv1 import／callback／dynamic-export control arenas、private
heap bandと32 child slotsをallocation recordsとして登録する。Heap backingはrecord IDへ帰属する。
同じregistryはfixed result addressなしでDLL image、追加stack、COM endpointをaligned search予約できる。
Control installerとinitial environmentはrecord-driven mappingへ移行し、64-bit allocation IDをRust
mapping operationからBrowser mirrorまで保持する。owner／range／lifetime不一致はmutation前に拒否し、
environment reservation、memory transaction、process metadataは一つのatomic planでcommitする。
詳細は[process-address-space-allocator.md](process-address-space-allocator.md)を正とする。

### 6.3 Heap と mapped file

初期heap foundationはruntime-owned allocatorとしてguest memory内へblockを配置する。Version 1は32 x 16 MiB fixed band、8-byte first-fit、monotonic allocation ID、deterministic initialization／free poison、heap／process teardownを実装する。Win32 schema-v2 policyはimplemented flagsと16 MiB以下のnonzero rangeだけをこのallocatorへ渡し、historical schema-v1 exact-size policyをactive authorityにしない。Zero-size、other flags、failure-return／exception、reallocとtitle workload capacityは未完了である。VFS mapped file、image section、anonymous allocationは同じregion abstractionで表現するが、flush、copy-on-write、sharingのsemanticsは別metadataとする。詳細は[runtime-heap-backing.md](runtime-heap-backing.md)を正とする。

## 7. x86 execution engine

### 7.1 Adapter contract

CPU engine は次の interface の背後に置く。

- reset / load thread state / save thread state
- execute up to instruction or time budget
- checked memory read / write / fetch
- trap on API thunk、exception、breakpoint、yield、halt
- invalidate decoded code range
- deterministic trace hook
- instruction feature report

既存 engine を採用する場合も、Wasm portability、correctness corpus、callback trap、memory adapter、bundle size を評価する。decoder だけを再利用する場合と full emulator を再利用する場合を区別する。

### 7.1.1 Current feasibility candidate

2026-07-10 時点では v86 を adapter feasibility の先行候補とする。prebuilt fallback Wasm で full-PC boot を迂回した 32 bit integer／stack／x87／SSE program が動き、debug Wasm では CPL3 `#UD` を host hook へ返せた。詳細と再現 command は [x86-engine-scorecard.md](x86-engine-scorecard.md) に記録する。

固定 source overlay はexact instruction budget、structured CPL3 fault stop、versioned adapter ABI `1`、versioned 640-byte CPU state ABIを提供する。zero-FS-base first stopに続き、FS segment base `0x7FFDE000`をrestoreしてminimum TEB／PEB fieldsとnative-oracled command-line bytesを実行した。Rustのexecute metadataはadapter-owned page bitmapへ同期し、interpreterがopcodeと実際に消費するdecode byteごとに検査する。page末尾の短命令を許可しつつNX pageへ跨ぐdecodeはEIP／destination不変のstructured stopになる。changed-page batchはmulti-page mutationをRust authorityへ戻す。controlled Chrome Runtime Workerは同じbinary boundaryで46 guest instructions、3 periodic yields、2 deferred completions、depth-2 callback、7 writeback batches、clean exitを通した。current 152-call BGI sessionも同じWorker内のRust/Wasm authorityでNode oracleとsemanticに一致し、timer clamp非依存yieldによる3分／961-session workload gateはsession／slice、memory slope、linear memory、Wasm crossing thresholdを通した。general process parameters、stack growth、snapshot、broad ABI、guest SEH delivery、title output、full-play performanceは未検証である。詳細は[adr-003-guest-memory.md](adr-003-guest-memory.md)、[memory-writeback.md](memory-writeback.md)、[browser-runtime-worker.md](browser-runtime-worker.md)、[bgi-browser-benchmark.md](bgi-browser-benchmark.md)、[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。

### 7.2 段階的な命令範囲

1. i386 integer、flags、branch、stack、string、segment の必要部分
2. x87 と Windows compiler が生成する floating-point sequence
3. CPUID、RDTSC 等の環境／時間依存命令を仮想化
4. privileged、I/O、kernel／direct-system-call 命令を guest fault／unsupported として隔離
5. MMX、SSE、SSE2 を corpus inventory 順に追加
6. atomic、lock prefix、thread interaction
7. self-modifying code と最適化 engine の cache coherence

命令を「実装済み」とする条件は decode できることではなく、flags、fault ordering、NaN／rounding、memory side effect を fixture と oracle で確認できることである。

### 7.3 Optimization gate

Interpreter、threaded interpreter、IR interpreter、DBT、generated Wasm module を Phase 0-1 で比較する。最適化方式は以下を満たしてから既定にする。

- CSP と browser support Tier を満たす
- code cache が guest write、DLL unload、snapshot と整合する
- cold start と compilation pause が UX budget 内
- interpreter との differential trace が一致する
- malformed input で runtime state を破壊しない
- dependency と配布 size が release 方針に合う

## 8. PE / DLL loader

Loader は file parse、image mapping、module lifecycle、symbol resolution を分離する。

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
version 1 loader ingressは、owned source bytesをmap-timeに再hashし、PE32 section／import／`HIGHLOW` relocationのchecked planを作る。memory ingressは`SizeOfImage`を1 sparse regionへreserveし、planned pagesをfinal R／W／X metadataでcommitする。materialization layerは同じplanからsource copy、explicit zero-fill／readback、wrapping `HIGHLOW`、original IAT verification、mapped-range digestを行う。import-binding layerは全importをmemory mutationなしで検査し、`[0x7F000000,0x7F010000)`の16-byte owned `UD2` slotsをplan orderで割り当てる。明示catalogのendpointとoptional identity-preserving unsupported trapsを区別し、complete planだけをtransaction内でcommitしてruntime／image ownersをsealする。BGIはdeclared-only policyで329 unresolved identitiesとmemory不変を保持し、explicit seventeenth-call policyでは14 declared／316 unsupported dispositionsで全330 importsをbindする。base 388-page reflectionにseparate `[0x7F020000,0x7F030000)` dynamic-export arenaのone RX pageを加え、entryからfirst seventeen runtime-owned APIsをreturnする。typed logical private-heap／loaded-KERNEL32／TLS-index／initial-thread-value／encoded-pointer metadata、four ordered FLS endpoint identities／guest storesを保持する。

separate runtime-kernel profileはbinding-scoped dynamic endpoint dispatch、strict pointer inverse、bounded TLS／FLS／LastError state、private-heap allocation／free／size、checked LONG updates、logical standard handles、controlled process environment／Windows-932 and NLS views、bootstrap-owned image path／current directory、explicit processor-feature presence、process-owned filter／critical sections／named mutexを含むgeneric contractsを通す。Win32 ABI／validation／state effectはsingle semantic handler IDを使い、bounded selector／file type／heap／process-view／code-page／processor-feature／default critical-section／current-directory／named-mutex subsetだけをversioned compatibility policyから受け取る。HeapAllocのactive policyはimplemented flags、ordinary nonzero最大size、explicit large-size variantsを持ち、観測size一覧をsemantic gateにしない。`RuntimeProcessDirectories`はcurrent-directory bytesとgenerationをprivateに所有し、`RuntimeSynchronization`はmutex name／owner／recursionをprivateに所有する。`SetCurrentDirectoryA`と`CreateMutexA`はguest pointerを保持せずowned identityをcopyし、aggregate plan／lifecycle preflight／commitを通す。BGI caller RVA、observed arguments、call order、next stopはevidence profileに残り、implementation branchを選ばない。process-owned module graphはcanonical identity、main／guest／runtime class、dependency／reference ownership、dependency-first attach／reverse detach metadataを持つ。Logical private heapとinitial-thread stackはprocess allocation identityへ接続し、reserve／commit／dirty deltaをv86 mirrorと同じRust authorityで処理する。Current stop等の変動値は`current-checkpoint.json`を正とする。

PE resource treeはimmutable `Pe32LoadPlan`のoptional catalogとして解析し、type／name／languageと
leaf RVA／size／code pageを保持する。Mapped-module ownerはsealed image record、load-base index、
resource catalogを一緒に所有し、consumerはcatalog identityとguest rangeの両方を照合してから読む。
Guest memory、TypeScript、個別API handlerがresource treeを再解析して第二のauthorityを作らない。
Directory／entry／name countとoffset arithmeticはbounded plan作成時にfail closedとする。

system-DLL image、broad export／forwarder resolution、guest DLL callback、TLS／FLS free・callback teardown、host environment import／mutation、broad file／heap／NLS semantics、general processor-feature discovery、SEH dispatch／filter invocation／vectored handlers、general FLS lifecycle、other interlocked operations／cross-Worker atomicity、critical-section contention／delete／arbitrary raw initialization、initial stack growth／overflow、general pointer-cookie semanticsはnot attemptedである。policy boundaryは[win32-compatibility-policy.md](win32-compatibility-policy.md)、memory boundaryは[guest-memory-ingress.md](guest-memory-ingress.md)、module metadataは[runtime-module-graph.md](runtime-module-graph.md)、heap backingは[runtime-heap-backing.md](runtime-heap-backing.md)、runtime continuationは[bgi-initialize-critical-section.md](bgi-initialize-critical-section.md)、frozen entry／API executionは[v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md)を正とする。
<!-- END FROZEN HISTORICAL EVIDENCE -->

### 8.1 Parse layer

- bounded reader を使い、offset + length を overflow-safe に検査する
- DOS、COFF、optional header、data directory、section を immutable model にする
- warning と fatal error を taxonomy で区別する
- unsupported machine／feature を mapping 前に検出する

### 8.2 Mapping layer

- image base を選び section を map する
- zero-fill と alignment を適用する
- relocation を guest memory 内だけに書く
- section protection を最終状態に切り替える
- import／delay import の IAT を guest thunk address へ結ぶ

### 8.3 Module lifecycle

- module identity、reference count、load order、dependency graph を保持する
- export by name / ordinal、forwarder、dynamic lookup を扱う
- corpus が必要とする API-set／system module alias、manifest／activation context を policy 付きで解決する
- TLS template と callback、DLL process/thread attach/detach の順序を検証する
- unload 中の callback、stale function pointer、cycle を診断する

System DLL 名の解決順は policy で固定する。Vine synthetic DLL と title 同梱 DLL を曖昧に混ぜず、置換可否を module ごとに定義する。Microsoft system DLL を title directory から読み込むことを前提にしない。

Main executable と load 対象 DLL は、import 時に hash と stable snapshot を記録した immutable source object から map する。外部 process が元 file handle の内容を import 後や delay-load 前に差し替えても、実行中／再起動後の module bytes が変わってはならない。content-addressed managed copy を推奨 baseline とし、別方式を採る場合は map 直前の再検証と lifetime が同じ保証を満たすことを ADR で証明する。writable overlay が同じ path の `.exe`／`.dll` を shadow して module resolution を変えないようにする。guest が source file の persistent self-update を要求した場合は既定で拒否して診断し、新 version の取り込みは runtime 外の明示的な再 import として扱う。mapped guest image page への書き込みは別問題として page protection／code invalidation policy で扱う。

### 8.4 PE32 process bootstrap

Entry point へ渡す前に、compiler／CRT support band ごとに次を一貫して構築する。

- adopted flat CS／DS／ES／SS と TEB を指す FS selector／base
- stack base／limit、initial ESP、guard、return sentinel、initial register policy
- TEB の exception chain、self、PEB pointer、TLS slots、LastError 等の adopted field
- PEB の image base、loader module list、process heap／flags 等の adopted field
- process parameters の command line、environment、current directory、window／startup data の adopted field
- static PE TLS template／callback と Win32 TLS Alloc／Get／Set／Free の別々の lifecycle

PEB／TEB は undocumented field を含む全面互換とは表現せず、supported compiler／CRT startup fixture と direct-access trace で保証する layout を versioned profile にする。未採用 field の direct access は silent zero ではなく、可能な範囲で trace／fault へ結び付ける。

## 9. ABI gateway と API metadata

API 定義は、可能な限り単一の machine-readable metadata から次を生成する。

- DLL、export name、ordinal、calling convention
- parameter type、direction、optional、count／length relation
- structure size、packing、nested pointer
- ANSI / Unicode variant と code page
- return type、LastError policy、handle ownership
- callback schema と reentrancy policy
- implementation status と semantic test link
- resource cost と execution mode

Marshaller は scalar、pointer、string、array、structure、callback を checked guest memory interface 経由で読む。raw guest pointer は call boundary を越えて保持しない。API 実装後に output buffer を書く際は thread cancellation と lifetime を再検証する。

Handle は tagged table entry と generation を持ち、guest-visible integer から host object を直接推測できない形にする。version 1 Rust foundationはtype 8／generation 8／slot 16、separate object／handle record、process／thread wait state、subsystem-owned module／heap close policyを実装した。Mutable object／handle／slot／identity collectionsはprivate `runtime/objects.rs::RuntimeObjectTable`だけが所有し、aggregateはtyped operationとplan／commitを通して調整する。close 後の handle、型違い、double closeを一貫した error にする。詳細は[runtime-object-model.md](runtime-object-model.md)を正とする。

## 10. Synthetic Windows DLL layer

### 10.1 レイヤー

| Layer | 主な責務 |
|---|---|
| Native boundary | PEB／TEB／FS、LastError、exception、virtual memory、loader 内部契約 |
| Base services | process／thread、heap、file、time、sync、encoding、module API |
| USER | logical window、message queue、focus、input、timer |
| GDI | DC、object、bitmap、text、blit、clip |
| COM subset | IUnknown、class registry、apartment の必要範囲 |
| DirectX facade | DirectDraw、DirectSound。将来 D3D、DXGI、XAudio2 |

COM object／interface identity、vtable、shared reference countはprivate `RuntimeCom`、
thread-local apartment model／successful initialization countはprivate `RuntimeThreads`が所有する。
`RuntimeKernel`は両者を統合したcollectionを持たず、必要なcross-subsystem preflightだけを調整する。
Browser resource associationとhost COMはどちらのguest-visible stateのauthorityにもならない。

`stub` は成功を返すだけの意味にしない。status は次で区別する。

- `implemented`: contract test と必要 scenario を通る
- `partial`: 対応 variant と差分を明示する
- `diagnostic_stub`: 副作用なしで既知の error を返し trace する
- `unsupported`: call site を含む structured failure で停止する

## 11. VFS と registry

### 11.1 Volume model

```text
C:\Game\...      read-only source volume + title-local writable overlay
C:\Users\...     title-local writable profile
temp              quota 付き ephemeral volume
explicit mount    user consent を得た場合のみ
```

Path normalization は Windows semantics と mount resolution を同じ関数で混同しない。まず drive、separator、relative path、case-insensitive name、reserved name を guest path model で解決し、その後 mount 内の object ID へ変換する。

### 11.2 Persistence

- source と save を別 namespace／quota にする
- immutable module snapshot と large asset の lazy source handle を別 object／lifetime にする
- write は journal または copy-then-commit で crash consistency を持たせる
- guest API へ write 成功を返す時点、flush／close、storage transaction commit の durability を定義する
- watchdog／quota／runtime defect による hard stop では未commit transaction を rollback し、停止を理由に自動commit しない
- schema version と runtime version を保存する
- migration 前 snapshot、失敗時 rollback、明示 export を提供する
- backend quota、permission loss、database／schema incompatibilityを起動前に診断する
- title fingerprint ごとに storage を隔離する

初期backendはsame-origin localhost Vine serverとし、immutable source filesystemはread-only、save／registryはtitle fingerprint別SQLite transactionを使う。Rust VFSがWindows path、mount、handle、cursor、access／share、LastErrorのauthorityであり、serverはsource snapshot bytesとcommitted generationのdurabilityだけを所有する。Workerはscheduler-owned host requestを運び、別のfile stateを作らない。

実装済みsource sliceはprocess-static version-1 manifestをRustへ登録し、`CreateFileA`／`GetFileSize`／`ReadFile`／`SetFilePointer`／`CloseHandle`を一つのread-only mountへ接続する。Worker requestはprepared manifestへbindingされ、HTTP URLとresponse headerでtitle fingerprint、snapshot、source、object、offset、lengthを往復検証し、Workerと異なるoriginのsource manifest／byte serviceはfetch前に拒否する。Serverは明示登録fileの全length／SHA-256を先に検証したimmutable copyだけをrange deliveryする。Wrong／stale／duplicate completion、memory write failure、cancel、process exitはcursor、guest bytes、handle allocatorを部分更新しない。

このsource sliceはwritable overlay、directory enumeration、mapped file、large-corpus streaming、server restart、save／load、SQLite durabilityを実装済みとしない。

Storage requestにはtitle fingerprint、snapshot／profile ID、mount内object／path key、range、base generationを渡し、生のguest pointer、Win32 handle、任意host absolute pathを渡さない。UIとAPIをloopbackの同一originから配信し、利用者が登録したsource rootとVine管理profile root以外を公開しない。Flushまたは成功したCloseを最初のdurability boundaryとし、cancel／watchdog／server killでは未commit write-setをrollbackする。詳細は[ADR-006](adr-006-vfs-local-server.md)を正とする。

OPFS、IndexedDB、memory adapterは同じstorage contractの将来backendとする。Remote upload／cloud同期はactive scope外であり、同期accessが可能でもschedulerを無制限にblockしない。

### 11.3 Registry

Registry は typed key-value tree とし、VFS file へ無理に模倣しない。HKCU と title-local classes を中心に、default view、case-insensitive key、value type、enumeration order、delete semantics を検証する。host browser setting や OS registry へ接続しない。

## 12. Window、message、input

Guest の HWND は logical object であり、DOM element を直接公開しない。最初は一つの top-level canvas に複数 logical window を composition する方式を候補とする。

Guest の HMONITOR と monitor geometry も Rust-owned logical display state とし、native HMONITOR、DOM `Screen`、browser window identityを直接公開しない。Compatibility policyがbounded profileを選び、display subsystemがlogical handle namespace、primary identity、geometryをconfigure-once plan／commitで所有する。User32 enumerationはこのimmutable viewからcallback scratchを組み立て、browser topologyを第二の正本にしない。

`GetSystemMetrics`のscreen dimensionも同じdisplay authorityから導出し、caption／frame等のbounded virtual non-client値だけをversioned User32 policyが所有する。TypeScript、DOM、native hostにmetric tableを複製せず、profile／primary geometry不一致と未到達indexはguest-visible stateを変える前に停止する。

Mouseのprimary／secondary mappingもRust-owned logical input stateとする。現在の`RuntimeInput` foundationはprofile identity、initial／current swapped state、generationをconfigure-onceとstale-plan拒否で所有する。`SwapMouseButton`、将来のUser32 mouse message、DirectInput mouse frontendは同じmappingを参照し、native `SM_SWAPBUTTON`やDOM adapterへ別tableを作らない。

Module iconもRust-owned User32 stateとする。`RuntimeUser32`はresource identity、monotonic icon ID、
logical HICON namespace、resource／handle index、generationをprivateに所有し、repeated shared loadは
同じidentityを返す。Process teardownがidentityをretireする。Browser rendererが後にbitmap resourceを
関連付ける場合もassociationはnon-owningであり、native HICON、GDI handle、DOM image IDをlogical
HICONとして流用しない。Size／language／format selectionはversioned User32 policyに置く。

Cursorも同じUser32 ownerの別identity namespaceに置く。`RuntimeUser32`はsystem predefinedまたは
mapped-module resourceから成るsource identity、monotonic cursor ID、logical HCURSOR、source／handle
indexをprivateに所有するが、icon ID／handleと相互代用しない。Module sourceはregistered module
ID／base／source hashとgroup／payloadのtype、name、language、RVA、size、dimensions、hotspotを保持し、
aggregate rootがmodule resource catalogとの一意なrelationをpreflightしてからUser32 planをcommitする。
Shared system／module cursorのrepeatはidentityを再利用し、icon／cursorを含むprocess teardownを一つの
stale-checked lifecycle planでretireする。Selection bandはversioned policy、CSS cursor／DOM input／
browser bitmapとのassociationは明示的なnon-owning adapter stateとし、native HCURSORやbrowser
resource IDをguest-visible identityへimportしない。

Process-local window classも`RuntimeUser32`の別identity domainとする。Class collection、ASCII
case-foldした`(module ID, name)` index、logical ATOM index、monotonic allocator、generation、
process teardownをprivateに所有する。Class recordはmapped module hash／base、same-module executable
WndProcのRVA／x86 stdcall descriptor、copied name／styleと、logical HICON／HCURSORへのnon-owning
identity pairを保持し、guestのstructure／string pointerを保持しない。登録は全field、module、memory
execute access、resource lifetime、capacityをpreflightした後にtargeted commitする。TypeScriptへ渡す
class metadataはimmutable completion snapshotであり、DOM class、native ATOM、callback lifetimeの第二の
正本にはしない。

Reached synchronous messageもlogical HWNDやWndProc identityをWorker側へ複製しない。Current bounded
`SendMessageA(WM_SETICON, ICON_SMALL)` profileは`RuntimeUser32`のimmutable lookupでlive `created`
window、selected owner thread、registered class／WndProc、live shared 16×16 HICONを全てpreflightし、
既存schedulerのstdcall 4-argument callback frameへ渡す。Nested `DefWindowProcA`は同じidentity relationを
再検証し、owner-local mutation planでwindowのsmall-icon relationだけをcommitしてprevious HICONを返す。
WndProcの`LRESULT`はouter API resultへ戻す。Wrong／stale identity、callback preflight、duplicate／stale
commitはwindow／icon relationとallocatorを部分更新しない。Cross-thread send、queue integration、
timeout／cancel、browser icon associationは後続message gatewayで別contractとして扱う。

Reached `SetWindowPos` profileでは、geometry、visibility、topmostとsmall-icon relationのauthorityを同じ
`RuntimeUser32` window recordへ置く。Show operationはlogical displayとversioned non-client policyから
off-screen outer／client geometryをplanし、`WM_WINDOWPOSCHANGED`からnested `WM_MOVE`、同depth replacementの
`WM_GETICON(ICON_SMALL2)`へ進む。Expected icon identityまで確認したfinal callbackだけがgeometry、visible、
topmostをtargeted commitする。Hide operationも`WM_WINDOWPOSCHANGED`完了後にvisibilityだけをcommitし、
positionを保持する。

Same-geometry activationはwindow recordへactive／focus booleanを複製せず、`RuntimeUser32`がthread別active
window／focus window relationとprocess foreground relationをprivateに所有する。最初の
`WM_WINDOWPOSCHANGING`完了後だけactive／foregroundをcommitし、続く`WM_ACTIVATEAPP -> WM_NCACTIVATE ->
WM_ACTIVATE`を検証する。`DefWindowProcA(WM_ACTIVATE)`はnested `WM_IME_SETCONTEXT -> WM_IME_NOTIFY`と
same-depth replacementの`WM_SETFOCUS`をpreflightした後にfocus relationをcommitする。Native z-order HWNDと
external thread IDはopaque host identityのためlogical relationへimportしない。Callback scratch、frame stack、
operation／relation generation、window lifecycleを全てpreflightし、callback result failureやstale／duplicate
planではguest-visible stateを変更しない。Workerが受け取るCreateWindow／SetWindowPos final window snapshotは
read-only observationであり、DOM／Canvas側に第二のWindows stateを作らない。General resize、z-order、
cross-thread／multi-window activationは別contractである。

- Rust User32／window subsystemがlogical window tree、z-order、focus、capture、invalid regionを所有する
- Runtime WorkerはCanvas／DOM等のbrowser resourceと明示的なnon-owning associationだけを所有する
- Main thread は DOM event を timestamp 付きで normalized input event にする
- User32 layer が virtual key、scan code、mouse coordinate、message sequence へ変換する
- synchronous send と asynchronous post を scheduler 上で区別する
- paint は invalidation、BeginPaint / EndPaint、presentation の順序を trace する
- browser shortcut と OS-reserved key は support matrix に差分を記録する
- DirectInput 8 frontend は同じ normalized keyboard／mouse state を読み、独立した DOM listener や別の入力時刻系を持たない
- Acquire／Unacquire、cooperative level、buffered event は logical HWND／focus state と明示的に接続する

IME、clipboard、file drop は personal target の keyboard path と分離して deferred とする。

## 13. Graphics

### 13.1 共通 surface model

GDI bitmap と DirectDraw surface が共有できる CPU-visible pixel storage abstraction を候補とする。D3D9 resource は同じ object model へ押し込まず、guest-visible lock staging、format conversion、presentation boundary だけを共有候補とする。

- width、height、pitch、orientation
- indexed 8 bit、RGB555／565、24 bit、32 bit 等の format
- palette、color key、alpha policy
- lock count、dirty region、owner、lost state
- CPU read/write と GPU upload/download の同期

GDI semantics と DirectDraw semantics を同一 object に押し込まず、storage と presentation backend だけを共有する。D3D9 の render target、texture、vertex buffer、device state は別 frontend が所有する。

Direct3Dの世代境界もguest-facing stateでは統合しない。D3D8、D3D9と、将来scopeを再開した場合の
D3D10／11は、それぞれのABI、COM identity、device／resource state、loss semanticsを独立したfrontendが
所有する。WineD3Dに相当する共有層はWindows interfaceを模倣する層ではなく、frontendが検証済みの
logical device／resource／commandを渡すRust内部の`RuntimeGraphics` boundaryとする。これは複数frontendの
共通事実を抽出する場所であり、COM pointer、vtable slot、D3D version固有enumをauthorityとして保持しない。

`RuntimeGraphics`はD3D9 `CreateDevice` gatewayで最初のlogical deviceとpresentation contract、
`CreateTexture` gatewayで最初のdevice-related logical texture descriptor、`GetSurfaceLevel` gatewayで
最初のlogical texture subresource descriptorを所有するprivate Rust subsystemである。Window relation、
normalized presentation、COM-object association、device-to-texture／texture-to-surface index、monotonic
device／texture／surface IDとgenerationをowner-local plan／commitで管理し、各4,096-object capacity、stale
plan、process teardownをmutation前に検証する。`Direct3DCreate9`、IUnknown lifetime、display query等のroot leafだけでは
graphics stateを作らない。Browser側はさらにその下のrenderer adapterだけを所有し、Rust device/resource IDとの
関連は別namespaceのnon-owning associationにする。この順序により、未到達D3D8／10／11 frameworkを先行実装せず、
後からfrontendを追加できるtranslation seamを今のauthority境界で確保する。

`CreateDevice`はlive HWND／display profile、56-byte x86 presentation input、guest output spans、COM child
registration、graphics capacityを全てpreflightする。Guest presentation prefix／device pointer、child COM
identity、logical graphics deviceは一つの外部観測可能transactionとしてpublishする。Deviceはroot interfaceを
一reference保持し、final releaseはCOM backing／process allocation／graphics associationをまとめてretireし、
必要ならroot retirementへcascadeする。Renderer resourceはこのgatewayでは作らない。

`CreateTexture`はlive device association、bounded descriptor、guest output span、COM child registration、
graphics texture capacityを全てpreflightする。Guest texture pointer、child COM identity、logical textureは一つの
外部観測可能transactionとしてpublishする。Textureはdevice interfaceを一reference保持し、final releaseは
texture associationを退役して必要ならdevice／root retirementへcascadeする。Logical descriptorはtexture bytes、
lock staging、browser renderer resourceの代用ではなく、それらは到達methodごとの後続gatewayで追加する。

`GetSurfaceLevel`はlive texture association、bounded level、guest output span、既存surface identityまたは新規
COM child registration、graphics surface capacityを全てpreflightする。初回はguest surface pointer、child COM
identity、logical surfaceを一transactionでpublishし、repeat取得は同じsurface identityへAddRefするだけである。
Surfaceはtexture interfaceを一reference保持し、final releaseはsurface associationを退役して必要なら
texture／device／root retirementへcascadeする。Logical surface descriptorはpixel storage、pitch、DC、lock
staging、browser renderer resourceの代用ではなく、到達した後続gatewayで追加する。

### 13.2 GDI frontend

DC state、selected object、mapping、clip、ROP、font metric を runtime model に保持する。Canvas text の結果を Windows GDI と同一と仮定せず、glyph rasterization と font substitution の選択を ADR にする。

### 13.3 DirectDraw frontend

COM vtable と HRESULT の ABI を提供し、surface create、lock、blit、flip、palette、cooperative level を共通 surface command に落とす。fullscreen は browser Fullscreen API と canvas scaling で表現し、display mode の変更を host OS へ要求しない。

### 13.4 Bounded D3D9 frontend

サクラノ詩の runtime trace で観測した interface／method／format／state だけを versioned support band とする。

- `Direct3DCreate9` から IDirect3D9／IDirect3DDevice9／到達resource interfaceの COM identity と lifetime を構築する
- CreateDevice／Reset／cooperative state と logical HWND／canvas presentation を接続する
- guest-visible Lock／Unlock memory と backend texture／buffer allocation を分離する
- fixed-function state、vertex format、blend、sampler、draw を WebGL2 または WebGPU command へ変換する
- device loss／context loss 後は reconstruct 可能な resource と guest に再作成を要求する resource を区別する
- 未観測 method を成功扱いせず、call site、vtable slot、arguments を診断して停止する

shader model や一般 D3D9 application compatibility はこの frontend の初期要件ではない。runtime trace で shader path が必須と判明した場合は、Phase 6D の scope／feasibility を再審査する。

### 13.5 GDI+ image subset

GDI+ は一般的な drawing stack として再実装せず、サクラノ詩で到達する bitmap load／save、DIB conversion、LockBits／UnlockBits を GDI surface、VFS、browser image codec のどこで処理するか ADR-007 で決める。OS codec の暗黙差に依存せず、format と pixel result を fixture に記録する。

### 13.6 Backend selection

| 候補 | 評価項目 |
|---|---|
| Canvas 2D | GDI primitive／text の補助実装、pixel access cost、再現性。D3D9 backend にはしない |
| WebGL2 | D3D9 fixed-function translation、shader による blit／palette、readback cost、context loss |
| WebGPU | D3D9 state translation、compute／copy 性能、browser availability、device loss、bundle complexity |

最初の renderer は知名度で選ばず、8 bit palette、RGB565、color key、partial dirty update、text、readback に加え、primary trace の D3D9 state／draw workload を同じ候補 backend で測る。

## 14. Audio

DirectSound frontend は buffer object と Windows-visible cursor semantics を所有し、backend は PCM block と control event を受け取る。

```text
guest Lock/Unlock -> DirectSound buffer state -> mixer/resampler -> ring buffer
                                                               -> AudioWorklet
                                                               -> Web Audio output
```

- AudioWorklet を real-time consumer とし、重い変換や guest access を置かない
- play cursor と write cursor は browser callback の瞬間値ではなく定義した clock model から計算する
- wrap-around lock、loop、volume、pan、frequency、event-handle notification／thread wake を semantic test する
- logical HWND、SetCooperativeLevel、focus／deactivation、primary format permission、buffer lost／Restore を DirectSound state として扱う
- browser user activation、AudioContext suspend、background、output device change は別の state machine とし、DirectSound cooperative level の代用にしない
- underrun、overrun、queue depth、drift を常時計測可能にする

MCI、ACM、DirectShow、video codec は DirectSound と同じものとして暗黙対応しない。サクラノ詩が静的 import する `mciSendCommandA` は primary scenario の到達性を測り、到達する command subset だけを Phase 6 の明示 task とする。

## 15. Browser shell

Shell はランタイムの機能説明ページではなく、実行に必要な操作面を提供する。

- library／recent title と local import
- preflight result と必要 browser feature
- launch、pause、resume、stop、reset
- canvas、fullscreen、audio activation
- save storage の使用量、export、import、delete
- compatibility status と diagnostic export
- runtime update と migration status

Shell は file handle や persistent permission が browser ごとに異なることを明示的な状態として扱う。

## 16. Diagnostics と compatibility data

全 subsystem は共通 event envelope を使う。

| Field | 例 |
|---|---|
| runtime | version、build ID、schema version |
| guest | virtual timestamp、thread、EIP、module、symbol |
| event | category、operation、result、duration |
| API | DLL、name／ordinal、status、LastError／HRESULT |
| resource | handle type、guest address range、quota delta |

Trace level は off、error、API summary、debug、instruction のように段階化し、instruction trace を通常運用で有効にしない。

## 17. Runtime robustness

runtime は互換性 failure が browser tab 全体の停止や再現不能な state corruption へ広がらないよう、次を共通基盤として持つ。

- PE、DLL、resource、archive の bounded parser と checked arithmetic
- guest pointer、length、handle、callback lifetime の一貫した検査
- instruction、wall time、memory、file、thread、handle、surface、audio の上限
- Worker watchdog と明示的に停止できる lifecycle
- malformed／boundary corpus、fuzz、negative test
- dependency pin と build artifact hash

## 18. Error taxonomy

| Class | 例 | 既定動作 |
|---|---|---|
| User action required | audio activation、file permission | guest を pause し shell で要求 |
| Unsupported | API、instruction、pixel format | call site 付きで安全停止 |
| Guest fault | access violation、invalid opcode | guest exception。処理不能なら title のみ停止 |
| Invalid input | malformed PE、archive bomb | mapping 前に拒否 |
| Resource limit | instruction、memory、storage quota | title を pause／stop し、commit 済み state を保持して未commit transaction を rollback |
| Host transient | context loss、audio suspend | bounded retry または再初期化 |
| Runtime defect | invariant violation、marshaller mismatch | fail closed、diagnostic を生成し、該当 build を不合格 |

失敗時に shell 全体を reload するだけの実装を避け、title runtime、storage、UI の failure domain を分離する。

## 19. 将来拡張

### 19.1 PE32+ / x86_64

再利用を目指すのは parser framework、module graph、API metadata、VFS、handle、diagnostics、backend である。次は新規または大幅な再検証が必要になる。

- 64 bit guest address space と pointer marshalling
- x86_64 execution、SSE baseline、Win64 calling convention
- unwind metadata、exception、TEB／PEB layout
- memory64 または multi-region memory の browser 成立性
- 64 bit structure layout と handle／pointer truncation audit

### 19.2 D3D9 generalization / D3D11 / DXGI / XAudio2 / Unity

サクラノ詩向け bounded D3D9 subset の完了を D3D9 全体の互換性とは扱わない。shader translation、追加 resource state、swap chain、threading、media、managed runtime、JIT 等を含む拡張は、それぞれ feasibility milestone と技術 review を持つ。

## 20. 先行 ADR

実装開始前または Phase 1 中に、少なくとも以下を決定する。

| ADR | 決定対象 | 決定期限 |
|---|---|---|
| ADR-001 | Rust + TypeScript の workspace、ownership boundary、Wasm toolchain | Phase 0 |
| ADR-002 | x86 engine の自作／再利用と baseline execution mode | Phase 0 |
| ADR-003 | sparse guest memory、page size、protection、snapshot | Phase 0 |
| ADR-004 | Runtime Worker、guest scheduling、async suspension | Phase 0 |
| ADR-005 | API metadata、thunk、callback、marshaller の生成方式 | Phase 1 前半 |
| ADR-006 | [Local VFS backend、immutable source、SQLite overlay、durability／hard-stop rollback](adr-006-vfs-local-server.md) | Accepted initial backend |
| ADR-007 | renderer、GDI／DirectDraw surface、D3D9 resource／presentation boundary | Phase 3 まで |
| ADR-008 | DirectSound mixer、AudioWorklet、clock ownership | Phase 3 まで |
| ADR-009 | browser support Tier、SAB、COOP / COEP | Phase 1 終了時 |
| ADR-011 | compatibility profile と API inventory schema | Phase 0 |

各選択肢と決定方法は [risks-and-open-questions.md](risks-and-open-questions.md) に記載する。
