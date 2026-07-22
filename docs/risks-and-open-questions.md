# 未解決事項と構造的リスク

## 1. 目的と正本

この文書は、active architecture、support band、acceptance pathを今後も変えうる判断とriskだけを示す。
Resolved choiceはADR／subsystem文書へ置き、current count、hash、measurement、exact next stopは
[current-checkpoint.json](current-checkpoint.json)だけに置く。

Active targetとdeferred scopeは[requirements.md](requirements.md)、各判断がgateになる順序は
[roadmap.md](roadmap.md)を正とする。

## 2. 判断規則

Open questionは、選択したboundary、evidence、影響、再検討triggerを記録した時だけ閉じる。一つの
checkpointで偶然動いたことだけではResolvedにしない。

各gatewayでは次を行う。

1. Guest-visible invariantとownerを特定する。
2. Bounded corpus traceまたはcontrolled fixtureを得る。
3. Authority、lifetime、cancellation、failure orderingを守る案だけを比較する。
4. Supported／unsupported boundaryを記録する。
5. 測定値はこの文書でなくcurrent manifestへ移す。

## 3. 未解決の構造判断

| ID | 未解決の判断 | 現在のbaseline | Gateを閉じるためのevidence |
|---|---|---|---|
| Q-PLATFORM | Supported Chrome tier、cross-origin isolation／SABを必須にするか | Chrome、専用Runtime Worker、same-origin localhost Vine serverがactive baseline。SABは測定済みtransport requirementが出るまでoptional | Local-host smoke、storage／audio capability matrix、lifecycle E2E、fallback cost |
| Q-MEMORY | Execute protection、guard／exception delivery、self-modifying code、snapshot policy | Rust page metadataとadapter fetch checkがauthority、mapping invalidationはexplicit | Compiler／SEH fixture、write-to-code、guard delivery、snapshot identity／memory-ceiling test |
| Q-THREADS | Required SEH／VEH／APCとadvanced thread behavior | Multiple guest CPU contextとhost-wait schedulingは成立。高度なWindows delivery semanticsは未確定 | Primary thread traceとcontrolled create／block／wake／timeout／cancel／exception fixture |
| Q-LOADER | Required PE／DLL feature setとdeterministic module resolution | Mapped image、import／export、module graph、process attach／detach foundationは成立 | 到達dynamic load／unload、TLS／delay import、alias／manifest、search order、failure unwind trace |
| Q-ABI | ABI metadata生成と到達Win32／COM／variadic support band | Generic checked dispatchとexplicit unsupported stopがbaseline | Nested structure、callback、union、size-versioned API、COM class use、caller consumer、format grammar fixture |
| Q-VFS | Windows path／share／mappingの残るrequired subsetとsource snapshot lifecycle | Read-only manifest／path／share／cursor／pending read、same-origin object service、実asset read、wrong-root／cancelは成立。Rust VFSがsemantics authority | Replacement／TOCTOU、large-corpus streaming、server restart、broader path／share、mapped file test |
| Q-SAVE | Write／flush／close successの厳密なWindows-visible時点、backup／migration | 初期storeはtitle-local SQLite。Serverがcommitted generation、Rustがlogical write-setを所有する | Commit各段階のbrowser／Worker／server kill、quota-full、stale generation、migration、export／import test |
| Q-GRAPHICS | Bounded DirectInput／D3D9、common surface boundary、renderer、日本語font behavior | Logical frontend stateとbrowser／GPU resourceを分離する | Primary vtable／state／format／draw／input trace、upload／readback benchmark、context loss、glyph／metric／visual oracle |
| Q-AUDIO | Audio transport、clock／drift、到達WinMM／media scope | Rust-owned DirectSound8 root、primary／secondary buffer descriptor、PCM format、full-range secondary Lock／Unlock backing、content generation、looping playback stateをbrowser audio resourceから分離し、AudioWorkletをconsumer候補とする | Cursor／notification／Stop／Restore oracle、browser PCM transport、long playback drift／underrun、activation／background recovery、到達command trace |
| Q-055 | Full-play performance、fingerprint内容、soak／fuzz budget、acceptance evidence | Component／browser workload gateはあるがfull-play behaviorの代用ではない | Representative scene、versioned fuzz profile、long-run distribution、save／restart session、fingerprint collision／update test |

Cooperative schedulerかsingle CPU contextかという旧判断は閉じた。Runtime Workerはper-thread CPU
contextを保存／復元し、ownerがhost work待機中に別guest threadを実行する。残る判断はcontext switchの
有無ではなく、実corpusが要求するWindows behaviorとfairnessである。

## 4. Active structural risk

| ID | Risk／trigger | 現在のcontrol | 必須response |
|---|---|---|---|
| R-SCOPE | Static import、title固有branch、deferred platformによりscopeが膨張する | Primary-scenario evidence、declarative policy、fixture-first fix、explicit deferred scope | Expansionを凍結し、general contractへ還元してから再開する |
| R-CPU | x86 flags、floating point、exception、generated-code invalidationが後続sceneだけでdivergeする | Replaceable engine boundary、differential fixture、bounded slice、write invalidation | First instruction divergenceをbisectし、再現blockerがある場合だけengine変更／optimizationを行う |
| R-MEMORY | Full-play／snapshot memoryがsemantic／mirror ceilingを超える、Worker pressureまたはstale mappingを起こす | Rust byte authority、bounded growing pool、page reclamation、typed preflight exhaustion | Title peakを測定しversioned limitを見直す。Atomicityを弱めずchecked sparse mirrorを比較する |
| R-ASYNC | Sync Win32とasync host serviceがnested callback、wait、cancel、reentrancyでdeadlockする | Thread／request identity、per-thread CPU context、explicit state、scheduler-owned host request | 最小multi-thread／wait fixtureを追加し、unsupported reentrancyをmutation前に拒否する |
| R-FAIRNESS | Cooperative schedulingがtiming-sensitive threadをstarveさせる、またはtitle-relevant raceを変える | Runnable-lane selectionとbounded Worker execution | Thread traceからfairness／virtual-time policyを定義し、測定上必要な場合だけSAB／multi-Workerを比較する |
| R-LIFECYCLE | Background throttle／page lifecycle／resumeがtimer storm、desync、save corruptionを起こす | Worker isolationとexplicit cancel／watchdog | Background executionをclaimする前にfreeze／resumeとbounded catch-upを定義する |
| R-COMPAT | Subtle Win32／COM behaviorまたはsilent stubがbootを進め、後段stateを壊す | Caller-consumer oracle、typed fault、mutation-free negative path | Stubを正しいstateまたはfirst divergenceでのexplicit structured stopへ置き換える |
| R-GRAPHICS | Pixel format、pitch／ROP、font metric、backend差、context lossで画面が読めない | Logical surface／resourceとbrowser rendererを分離する | Supported format／stateをboundし、semantic＋visual oracleを追加し、非対応browser／GPUをtier分けする |
| R-MEDIA | Audio timing／media dependencyまたはD3D9 scopeがplanned browser subsetを超える | Corpus-bounded frontendとseparate backend adapter | Early feasibility traceを行い、広いAPI実装前にsupport bandを再設計する |
| R-DATA | Crash、quota、path差、external replacementでsaveを失う、またはsource moduleが変わる | Immutable source／writable overlay ruleとexplicit durability boundary | Read-only failureまたはlast committed stateへrollbackし、source EXE／DLLをshadow／rewriteしない |
| R-EXHAUSTION | Malformed inputまたはguest resource growthがmemory、thread、surface、log、parser limitを枯渇させる | Memory-safe bounded parser、quota、watchdog、typed exhaustion | Partial commitなしでtitleを停止し、最小diagnosticとregression fixtureを残す |
| R-BROWSER | Browser feature／hosting policy差で同revisionのbehaviorが分岐する | Versioned adapterとcapability detection | Supported browser／deployment tierを狭めるか、測定済みfallbackを用意する |
| R-PERFORMANCE | Ambient host loadまたはtitle workload growthでversioned latency／memory thresholdを外れ、clean oracleを判定できない | Fixed fixture／duration／threshold、repeated distribution、same-host bounded comparison、structured blocker | 外部workloadを勝手に停止せずisolated hostで同条件を再実行する。再現時だけprofileに基づくbounded optimizationを行う |
| R-MAINTENANCE | Toolchain drift、solo maintainerのcontext switch、demo優先がreproducibility／correctness gateを弱める | Locked dependency、WIP one、checkpoint evidence、authority rule | 一つのreproducible blockerへ戻り、source／save safetyとsemantic testを削らない |

初期storage locationの選択はADR-006で閉じた。Remote service、cloud同期、OPFS／IndexedDBへの移行は現在の未解決実装ではなく、localhost vertical slice後に具体的な要求が出た場合だけ再開する。

## 5. Scope reduction／no-go trigger

次がrepeatableならdownstream実装を止め、scopeまたはarchitectureを明示的に判断する。

- Required guest address spaceまたはfull-play memoryをsafe browser ceiling内に収められない。
- Sync／async、callback、required wait semanticsをmain thread非blockかつthread identity保持で再現できない。
- Primary graphics／audio pathがunbounded API bandまたは利用不能なbrowser capabilityを必要とする。
- Source immutabilityまたはcrash-consistent save／loadを維持できない。
- Required kernel、driver、protected service、codec、online dependencyがactive scope外である。
- 必要なcorpus／oracleを取得できずcorrectnessを判定できない。
- Versioned title workloadの性能未達に対し、profile上bounded optimization pathがない。

No-goはscenario／support bandの縮小、別architecture、research runtimeへの位置付け変更を選べるが、
full-play acceptanceを黙示的に下げてはならない。

## 6. 更新規則

この文書は、未解決判断の追加／解決、または構造的riskのtrigger／control／required responseが変わった
場合だけ更新する。Measurement、artifact identity、exact stop、pass／fail statusは
[current-checkpoint.json](current-checkpoint.json)だけを更新する。
