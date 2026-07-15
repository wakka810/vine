# Vine agent guide

この文書は、Vineを継続開発する際の恒久的な判断原則だけを示す。現在のAPI数、call数、hash、次のstop、完了履歴、個別command一覧は書かない。変動する事実をここへ複製せず、正本から確認する。

## 1. 目的と正本

Vineは、Windows向けVN／ADVの元のPE executableとDLLを変更せず、ブラウザ上で再ホストするWindows userspace互換runtimeである。active completion targetは、fingerprint済みのサクラノ詩をChromeで起動し、描画、入力、音声、全シナリオ、save／loadを含めて不自由なくプレイできる状態である。

判断時は次の順で参照する。

1. 利用者の現在の依頼と明示した制約
2. `docs/current-checkpoint.json`: 現在状態、active profile、次のstructured stop、canonical reproduction
3. `docs/requirements.md`、`docs/target-architecture.md`、`docs/roadmap.md`、`docs/testing-and-acceptance.md`: 目標、境界、依存、受け入れ条件
4. `docs/corpus-inventory.md`、`docs/risks-and-open-questions.md`、関連subsystem文書: corpus evidence、未確定事項、詳細契約
5. 実装、fixture、test、生成済みevidence

ルートREADMEのCurrent Statusはmanifestから生成された人間向け表示である。個別checkpoint文書やWindows evidenceの過去値は履歴であり、active queueとして解釈しない。削除済み文書、古い計画、コメント内のTODOを現在状態の正本にしない。

## 2. Scope

Active scopeは、primary fingerprintが実際に必要とするPE32／i386、x86 ABI、Win32、COM、GDI／GDI+、bounded D3D9、DirectInput 8、DirectSound／到達WinMM、Windows-932、file／registry／persistence、Chrome backendである。

PE32+／x64、Win64、D3D11／DXGI、XAudio2、Unity、multi-title一般化、第三者向けreleaseは、利用者が明示的に再開しない限りdeferredとする。Windows全体、kernel／driver、タイトル固有VMやasset decoderの再実装を目標にしない。

静的importの存在だけでscopeへ昇格させない。primary scenarioで到達したbehavior、または高リスクな成立性を早期判定するために必要なbounded spikeだけを扱う。

## 3. 作業の選び方

進捗はAPI数やinstruction数ではなく、次のscenario milestoneで測る。

- bootとprocess初期化
- logical windowとmessage flow
- immutable source／VFSからのasset access
- first presentと読める画面
- user inputの消費
- audible output
- save、browser再起動、load
- representative flow、全編、long-run

`nextStructuredStop`は直近の実装入口であり、project全体の優先順位そのものではない。到達したstopを次の三種類に分類する。

- semantic leaf: 既存subsystem内で閉じる小さなcontract。正確に実装して次へ進む。
- subsystem gateway: file、window、thread／wait、COM／graphics、input、audio、storage等の新しい責務境界。API単体patchを重ねず、最小vertical sliceと受け入れ条件を先に定める。
- feasibility blocker: browser能力、corpus要求、性能、correctness、data safetyが設計前提を脅かすもの。追加実装より先にevidenceと選択肢を固定する。

primary implementation taskは常に一件にする。別領域の発見は再現情報とともに保留し、現在のcheckpointを壊して横道へ入らない。一方、高影響の未知を放置したまま低リスクAPIを大量追加しない。D3D9 support band、audio、threading、VFS／saveなどのHigh-impact riskは、到達前でもbounded trace／spikeで成立性を確認してよい。

## 4. Authorityと責務境界

### Rust／Wasm core

Rustはguest-visibleなWindows semanticsとauthoritative stateを所有する。Guest memory、PE／module、ABI／API dispatch、process／thread、scheduler、object／handle、COM identity／refcount、logical heap／synchronization／filesystem／graphics／audio stateは、対応するRust subsystemが正本を持つ。

`RuntimeKernel`はaggregate rootであり、private subsystemの所有とcross-subsystem transactionの調整だけを担う。新しいcollection、identity allocator、lifecycle invariantをkernelへ直接追加しない。各subsystemは自身のcollection、counter、index、validation、plan／commitをprivateに所有し、kernelへmutable collectionを公開しない。

### Browser／TypeScript

TypeScriptはDOM、Canvas／WebGL／WebGPU、Web Audio、browser storage、permission、UI、lifecycle等のbrowser resourceとadapterを所有する。Windows object、guest pointer、handle、COM refcount、thread stateの第二の正本を作らない。Browser resource IDはRust identityと別namespaceにし、関連付けは明示的かつ原則non-owningとする。

sync guest APIとasync browser APIの差は、versioned Worker protocolとscheduler／host-service boundaryで吸収する。callback、completion、cancel、timeoutはthread、request、generation、lifetimeを検証してからstateを進める。

### CPUとmemory

Rust `GuestMemory`がbyte、mapping、owner、protection、generationのauthorityであり、v86 pagingは交換可能なexecution mirrorである。v86固有layoutやprivate stateをruntime全体へ漏らさない。CPUとの同期はbounded slice、checked binary transport、dirty discovery、atomic reconciliationを通す。

guest address、host offset、size、allocation、mapping operationにはchecked型と明示的identityを使う。guest pointer、Win32 handle、COM pointer、Rust object ID、browser resource ID、host pointerを相互代用しない。

### Domain boundary

- source executable／DLL／assetはimmutable、save／設定は別のwritable overlayとする。
- graphics frontend、logical surface／resource、browser rendererを分離する。
- audio frontend、logical buffer／clock、browser audio transportを分離する。
- COM identity／lifetimeはWin32 handle tableやbrowser resource lifetimeと混在させない。
- compatibility serializationはimmutable borrowから組み立て、旧shape維持のためにauthorityを重複させない。

## 5. State変更と失敗原子性

一つのdatumには一つのauthorityを置く。関連index、allocator、lifetime、guest bytesを別々に先行更新しない。

単一subsystemの変更は、そのownerがvalidationとmutation-free planを作り、stale／duplicate commitを拒否してtargeted commitする。複数subsystemまたはguest memoryをまたぐ変更は、全ownerのcapacity、identity、lifecycle、memory accessをpreflightした後にcommitする。失敗時はobservable state、allocator、mapping、refcount、queue、generationを部分更新しない。

aggregate root全体やsubsystem rootをcloneして差し替える方式を通常のrollbackに使わない。transaction journal、opaque mutation plan、before-image等、変更量と失敗境界に比例する仕組みを使う。

unsupported、malformed input、resource exhaustionはmutation前に具体的なfaultとして返す。silent success、host値の偶発的流用、後段で壊れるno-op stubを禁止する。

## 6. Compatibility実装

Win32／COM capabilityは次の層を混同しない。

1. ABI catalog: module、symbol／ordinal、calling convention、arguments、return、pointer contract
2. semantic handler: 一般化できるWindows behaviorとstate transition
3. compatibility policy: primary targetで許可するbounded variant、locale、仮想環境値
4. evidence profile: reached occurrence、arguments、consumer、期待結果、次のstop

BGIのEIP、callsite、hash、occurrenceをsemantic handlerの分岐条件にしない。title由来の値が必要ならversioned policyまたはevidenceへ置き、handlerは明示的contractとして解釈する。重複するABI metadataやstatus一覧を手作業で増やさず、単一catalogからvalidation、test data、文書を生成・照合する。

新しいAPIやCOM methodは、native traceだけでなくcallerが戻り値、LastError、output、identity、lifetimeをどう利用するかまで確認する。未到達variantを一般化しないが、NULL、範囲外、stale、wrong-owner、failure atomicity等、同じinvariantを守るためのnegative caseは固定する。

## 7. 設計と保守性

局所的に次のstopを越えることより、authority、lifetime、failure ordering、交換可能境界を守ることを優先する。既存の正しいboundaryへ機能を追加し、別経路、API固有memory同期、TypeScript側のWindows state、title固有patchを作らない。

抽象化は、複数の実到達case、交換可能性、または守るべきinvariantが確認できた時に導入する。未到達機能のframeworkを先に作らない。逆に、同じ事実を三箇所以上へ複製する、または一つのcapability追加が多数の独立表現を手修正させる場合は、次の追加前にcatalog化、生成、責務分割を検討する。

file sizeだけを理由に分割しない。state owner、変更理由、lifetime、failure domainが異なるところで分割する。public APIは小さく保ち、subsystem internalsをaggregateやadapterへ漏らさない。schemaや永続形式を変更する場合はversion、migration、rollback、旧evidenceとの互換性を明示する。

v86の置換や最適化は、correctness blockerまたはversioned workloadのthreshold未達とprofile結果がある場合だけ行う。推測によるhot-path最適化をactive workへ入れない。

## 8. Evidence、test、文書

oracleは目的別に使い分ける。

- Windows native evidence: Win32／COM semantics、caller behavior、ordering
- Rust unit／model test: state invariant、identity、lifetime、failure atomicity
- controlled PE fixture: ABI、guest memory、callback、loader integration
- Node semantic runner: deterministic headless coreの回帰
- Chrome E2E: Worker、browser adapter、graphics、input、audio、storage、lifecycle
- private target: primary fingerprintの到達性とscenario acceptance

whole-report hashやhost固有値はartifact identityであり、guest semanticsの代用にしない。semantic value、required ordering、opaque identity relation、performance distributionを区別して検証する。

各checkpointには、変更したinvariantを代表するautomated test、関連regression、必要なnative／model oracle、再現commandを含める。core boundary変更ではpublic fixture E2E、primary capability変更ではNode／Chromeとprivate targetを確認する。benchmark／stressは性能、memory、Worker crossing、graphics／audio、lifecycleに影響する変更またはscenario milestoneで実行し、無関係なleaf変更ごとに新しい性能正本を作らない。

現在値は`docs/current-checkpoint.json`だけを更新し、READMEはgeneratorで同期する。stable architectureやsubsystem contractは対応文書へ書く。単純なAPI occurrenceごとに長い説明文書を増やさず、新しいsemantics、architecture decision、non-obvious oracleがある場合だけ人間向け文書を追加する。raw trace、private binary、save、generated reportはGitへ入れず、`.work/`等のignored領域に置く。

## 9. 作業手順

1. `git status --short --branch`、直近commit、`docs/current-checkpoint.json`、関連する安定文書を確認する。
2. unrelatedなuser changeを特定し、stage、revert、format、renameしない。
3. 今回閉じるscenario／invariant、非対象、exit conditionを短く定義する。
4. 必要ならnative trace、controlled fixture、risk spikeを先に取得する。
5. 正しいownerとtransaction boundaryへ最小のcohesive実装を加える。
6. subsystem testから関連E2Eへ段階的に検証し、文書とmanifestを同期する。
7. `git diff --check`、diff、untracked file、最終statusを確認する。
8. commit／pushは利用者の現在の依頼が許可した場合だけ、再開可能なcheckpoint単位で行う。

既存check名をこの文書へ固定しない。`package.json`、ルートREADME、`docs/checks-and-ci.md`、manifestのcanonical commandから選ぶ。失敗したtestをskip、retry、threshold緩和で隠さない。

## 10. Doneとblocker

checkpointは、behaviorとnon-claimsが明確で、authority／identity／lifetime／failure invariantが保たれ、関連testとregressionがgreen、current manifestと文書が一致し、次のstructured stopまたはscenario milestoneが再現できた時に完了とする。実装量、API数、report生成だけではDoneにしない。

通常のunsupported APIはblockerではなく、次に分類すべき作業である。次の場合は構造化blockerとして停止する。

- repeatableなoracleまたは必要なcorpusを取得できず、correctnessを判定できない
- required behaviorがactive scope外のkernel／driver／service、利用不能なbrowser capability、または成立しない性能／memory条件を要求する
- data safety、authority、transaction invariantを守る複数案の選択が長期設計を変え、利用者判断が必要である
- 継続に新しい権限、破壊的操作、配布file変更、scope拡大が必要である

blockerにはexact reproduction、最初のfailure、取得済みevidence、試した選択肢、守るべきinvariant、再開条件を残す。

## 11. Repository安全規則

- userのunrelated changeを保持し、勝手にrevert、overwrite、squashしない。
- destructive Git操作、history rewrite、force pushを行わない。
- stage／commit前に対象pathを確認し、作業scope外のfileを含めない。
- game本体、同梱DLL、asset、raw trace、save、secret、`.work/`、dependency build outputをcommitしない。
- 配布fileはread-onlyを原則とする。意図的変更が必要なら、事前に利用者の許可を得てbackupと変更前後hashを残す。
- source executable／DLLのhash不変と、writable overlayの分離をfull-play完了条件として維持する。
