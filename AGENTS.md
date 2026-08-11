# Vine agent guide

この文書はVineの通常開発をforward-firstに保つための恒久原則である。現在の到達点は`docs/current-checkpoint.json`だけを正本とし、過去の`docs/bgi-*.md`、`docs/v86-bgi-*.md`、巨大report、個別checkpoint profileをactive queueとして扱わない。

## 1. 目的と優先順位

Vineは、Windows向けVN／ADVの元のPE executable、DLL、assetを変更せず、ブラウザ上で再ホストするWindows userspace互換runtimeである。active completion targetは、fingerprint済みのサクラノ詩`BGI.exe`をChromeで起動し、描画、入力、音声、save／load、全シナリオ、long-runを含めて不自由なくプレイできる状態である。

判断順は次のとおり。

1. 利用者の現在の依頼
2. source／save safetyとauthority invariant
3. active user-visible milestoneへの前進
4. reached behaviorのcorrectness
5. focused regression
6. documentationとevidence

API数、call数、instruction数、report size、profile数、文書数は進捗指標ではない。進捗は、readable frameまたはvisible input response、primary audio、save／restart／load、representative flow、full play／long-runの順で測る。`next stop`は実装入口であって、user-visible progressより上位ではない。

## 2. Active scope

Active scopeはprimary fingerprintが実際に必要とするPE32／i386、x86 ABI、Win32、COM、GDI／GDI+、bounded D3D9、DirectInput 8、DirectSound／到達WinMM、Windows-932、file／registry／persistence、Chrome backendである。

PE32+／x64、Win64、D3D11／DXGI、XAudio2、Unity、multi-title一般化、第三者向けreleaseは利用者が明示的に再開しない限りdeferredとする。静的importだけでscopeを増やさず、実タイトルで到達したbehaviorか、高影響リスクのbounded spikeだけを扱う。

## 3. 通常のforward loop

通常開発は`npm run dev:target`を入口とする。

1. `git status --short --branch`、HEAD、dirty／untracked fileを確認する。
2. release Chrome product pathのgeneric forward runを実行する。
3. 最初のunsupported、semantic fault、crash、またはuser-visible milestoneを取得する。
4. callerがresult、LastError、output、identity、lifetimeをどう使うか確認する。
5. 正しいRust subsystem ownerへreached variantのbounded semanticsを実装する。
6. ownerのfocused testと必要最小限のintegration checkを実行する。
7. forward runを直ちに再開する。
8. user-visible milestone、subsystem gateway、またはtrue blockerまで複数leafをまとめて進める。

一つのsemantic leafで作業完了にしない。同じfamilyのleafは意味のある機能群としてまとめる。通常runは10分以内を原則とし、超えたらfull evidence、debug build、全call保持、guest-write展開、instrumentation overheadを先に疑う。同じ長時間runをvalidatorやterminal expectationの差だけで繰り返さない。

## 4. Stopの分類

### Semantic leaf

既存owner内で閉じる小さなcontractである。原則として変更対象はABI／catalog、semantic handler、reached policy、代表的focused testだけとする。通常はprotocol、browser schema、README、current manifest、長文documentを変更しない。未到達variantは明示的unsupportedとし、成功を偽らない。

### Subsystem gateway

file、graphics、audio、input device、storage、threading等の新しい責務境界である。先に巨大frameworkを作らず、実タイトルが次へ進む最小vertical sliceと、そのauthority／failure boundaryを実装してforward runを続ける。

### True blocker

次だけをblockerとする。

- 必要なcorpusまたはoracleがなく実装判断不能
- required browser capabilityが実測上利用不能
- source／save safetyを維持できない
- active scope外のkernel、driver、serviceが本当に必須
- 測定済みperformance／memory条件が成立しない
- 長期architectureを変える複数案に利用者判断が不可欠

普通のunsupported API、未実装COM method、call budget、古いvalidator、stale manifest、report shape mismatchはblockerではない。

## 5. Runtime authorityとsafety

Rustはguest-visibleなWindows semanticsとauthoritative stateを所有する。Guest memory、PE／module、ABI dispatch、process／thread、scheduler、objects／handles、COM identity／refcount、logical heap／sync／filesystem／graphics／audio stateは対応するRust subsystemが正本を持つ。`RuntimeKernel`はaggregate rootとしてprivate owner間を調整し、ownerのcollectionやallocatorを重複所有しない。

TypeScriptはDOM、Canvas／WebGL／WebGPU、Web Audio、browser storage、permission、UI、lifecycle等のbrowser resourceとadapterだけを所有する。Win32 handle、message queue、LastError、COM refcount、thread state、guest pointerの第二authorityを作らない。Browser resource IDはRust identityと別namespaceにする。

`GuestMemory`がguest bytes、mapping、owner、protection、generationのauthorityであり、v86 pagingはexecution mirrorである。guest address、host offset、size、allocation、handle、COM pointer、browser resource IDを相互代用せず、host使用前にchecked rangeとowner／lifetimeを検証する。

source executable、DLL、assetはimmutableとし、save、設定、registry stateはtitle-local writable overlayへ分離する。配布fileを意図的に変更する必要がある場合は、利用者許可、backup、変更前後hashが必要である。

単一ownerの変更はvalidation後にtargeted commitする。複数ownerまたはguest memoryをまたぐ変更は全preflight後にcommitし、失敗時にallocator、mapping、refcount、queue、generation、persistent bytesを部分更新しない。EIP、callsite、title hash、occurrenceだけでruntime semanticsを分岐させない。

## 6. Development modeとacceptance mode

### Forward development mode

通常はこちらを使う。release build、Chrome product path、bounded semantic tail、small reportを用い、次のunsupported／fault／milestoneまで進める。全call payload、巨大JSON、checkpoint固有profile、per-stop schema、Node full replayを要求しない。reportは`.work/`へ置きGitへ追加しない。

新API到達だけではWorker protocol versionを上げない。wire contract自体を変更した時だけ更新する。historical capability profilesはfreezeし、通常開発で増やさない。

### Milestone acceptance mode

readable frame、visible input response、primary audio、save／restart／load、representative flow、full play／long-run、またはauthority／storage safetyを変えるarchitecture変更の時だけ一度実行する。必要に応じてfull Node replay、private Chrome equivalence、workspace regression、artifact digest、manifest／README同期をまとめて行う。validator修正だけなら既存artifactを再利用する。

development snapshotは、instrumentationを落としたrelease forward runでも同じprefixが繰り返し60秒以上を占めると測定された場合だけ検討する。`.work/`専用、commit／source／runtime／engine identityでinvalidate、production persistence非互換、mismatch時cold bootで十分とする。

## 7. Tests、docs、checks

semantic leafはownerのunit／model testと必要なsmall fixtureだけを使う。subsystem gatewayは境界のfailure atomicityと最小vertical sliceを確認する。milestoneでのみprivate Chrome、full Node、broader regressionをまとめる。native oracleは文書だけで曖昧、callerが微妙なorderingへ依存、identity／lifetime不明、または実divergenceがある場合だけ取得する。

`check:fast`を通常gate、`check:milestone`を節目、`check:full`をstress／historical用とする。tracked private evidenceの鮮度やwhole-report hashを`check:fast`の条件にしない。

現在値はcompactな`docs/current-checkpoint.json`だけへ置き、README statusはgeneratorで同期する。単純なAPI occurrenceごとにmanifest、README、protocol、profile、文書を更新しない。stable contractだけをactive docsへ書き、raw trace、private binary、save、generated reportは`.work/`へ置く。

## 8. 禁止事項

- per-API checkpoint、per-occurrence document、per-stop capability profile
- exact call countやwhole-report hashを通常のguest correctness gateにすること
- source textへregexを当てるだけのworkflow validator
- run-specific endpoint、pacing、memory generationの過剰固定
- 通常開発で数十MB／数百MB reportを生成すること
- 一件のAPI実装後に作業完了とすること
- 数時間のreplayを結果待ちし続けること
- 古いevidence machineryを守るため本体実装を止めること
- unsupported、malformed input、resource exhaustionをsilent successにすること

## 9. Repository safetyとdelivery

利用者のunrelated changeをstage、revert、format、rename、削除しない。destructive Git、history rewrite、force pushを行わない。stage前に対象pathを確認し、game本体、DLL、asset、raw trace、save、secret、`.work/`、dependency outputをcommitしない。

commitはAPI単位ではなく、forward-first governance／tooling、runtime progression、必要な最終整合の1〜3個のcohesive checkpointにまとめる。利用者が許可した場合だけcommit／pushし、`git diff --check`、対象diff、untracked、最終statusを確認する。

作業配分はgovernance／docs 15%以内、generic tooling 20%程度、runtime progression 65%以上を目安にする。diffや時間の大半がmanifest、validator、documentへ戻ったら直ちにruntimeへ軸を戻す。
