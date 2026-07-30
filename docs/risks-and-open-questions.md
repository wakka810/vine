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
| Q-THREADS | Required SEH／VEH／APCとadvanced thread behavior | Reached CreateThread／ResumeThread、dynamic stack／TEB／CPU bootstrap、logical priority state、multiple guest context、host-wait schedulingは成立。priority-weighted selectionと高度なWindows delivery semanticsは未確定 | Primary fairness／wait traceとcontrolled block／wake／timeout／cancel／exception fixture |
| Q-LOADER | Required PE／DLL feature setとdeterministic module resolution | Mapped image、import／export、module graph、process attach／detach foundationは成立 | 到達dynamic load／unload、TLS／delay import、alias／manifest、search order、failure unwind trace |
| Q-ABI | ABI metadata生成と到達Win32／COM／variadic support band | Generic checked dispatchとexplicit unsupported stopがbaseline | Nested structure、callback、union、size-versioned API、COM class use、caller consumer、format grammar fixture |
| Q-VFS | Windows path／share／mappingの残るrequired subsetとsource／profile lifecycle | Immutable manifestとmetadata-only writable profile version 3をpath-disjointに登録し、Rustがhandle／cursor／hydrated committed bytes／staged bytes／file・directory identity／generation、same-origin serviceがsource bytes／SQLite committed generationを所有する。Profile contentはbounded binary rangeでhydrateし、bounded read／write／seek／flush／close、allowlisted durable directory create、cancel、failed commit retryは成立 | Replacement／TOCTOU、large-corpus streaming、broader path／share、rename／delete、enumeration、mapped file test |
| Q-SAVE | Write／flush／close successの厳密なWindows-visible時点、backup／migration | Title/profile-scoped SQLite、file／directory generation CAS、idempotency ledger、quota、rollback、restart／response-loss modelに加え、transaction内epoch lease検証、terminal commit barrier、recoverable Win32 failure completionが成立。既存BGI.gdbはnew DBだけへseedし配布fileへ書き戻さず、private directory checkはrun-local DBだけを変更する | Controlled Chrome guest file write／browser restart／load、commit各段階のbrowser／Worker／server kill、permission revoke、migration、export／import test |
| Q-GRAPHICS | Bounded DirectInput／D3D9、common surface boundary、renderer、日本語font behavior | Logical frontend stateとbrowser／GPU resourceを分離する | Primary vtable／state／format／draw／input trace、upload／readback benchmark、context loss、glyph／metric／visual oracle |
| Q-AUDIO | Audio transport、clock／drift、到達WinMM／media scope | Rust-owned DirectSound8 root、primary／secondary buffer descriptor、PCM format、full-range secondary Lock／Unlock backing、content generation、looping playback stateをbrowser audio resourceから分離し、AudioWorkletをconsumer候補とする | Cursor／notification／Stop／Restore oracle、browser PCM transport、long playback drift／underrun、activation／background recovery、到達command trace |
| Q-055 | Full-play performance、fingerprint内容、soak／fuzz budget、acceptance evidence | Component／browser workload gateはあるがfull-play behaviorの代用ではない | Representative scene、versioned fuzz profile、long-run distribution、save／restart session、fingerprint collision／update test |

Cooperative schedulerかsingle CPU contextかという旧判断は閉じた。Runtime Workerはper-thread CPU
contextを保存／復元し、通常のhost work待機中に別guest threadを実行する。Reached waitable-timer pacingだけは
wall timingがguest orderを選ばないようsingle emulated CPUをholdし、Rust completionのowner laneを一度だけ
直接復帰するbounded exceptionである。残る判断はcontext switchの有無ではなく、実corpusが要求するWindows
behaviorとgeneral fairnessである。

## 4. Active structural risk

| ID | Risk／trigger | 現在のcontrol | 必須response |
|---|---|---|---|
| R-SCOPE | Static import、title固有branch、deferred platformによりscopeが膨張する | Primary-scenario evidence、declarative policy、fixture-first fix、explicit deferred scope | Expansionを凍結し、general contractへ還元してから再開する |
| R-CPU | x86 flags、floating point、exception、generated-code invalidationが後続sceneだけでdivergeする | Replaceable engine boundary、differential fixture、bounded slice、write invalidation | First instruction divergenceをbisectし、再現blockerがある場合だけengine変更／optimizationを行う |
| R-MEMORY | Full-play／snapshot memoryがsemantic／mirror ceilingを超える、Worker pressureまたはstale mappingを起こす | Rust byte authority、bounded growing pool、page reclamation、typed preflight exhaustion | Title peakを測定しversioned limitを見直す。Atomicityを弱めずchecked sparse mirrorを比較する |
| R-ASYNC | Sync Win32とasync host serviceがnested callback、wait、cancel、reentrancyでdeadlockする | Thread／request identity、per-thread CPU context、explicit state、scheduler-owned host request | 最小multi-thread／wait fixtureを追加し、unsupported reentrancyをmutation前に拒否する |
| R-FAIRNESS | Cooperative schedulingがtiming-sensitive threadをstarveさせる、またはtitle-relevant raceを変える | Runnable-lane selectionとbounded Worker execution | Thread traceからfairness／virtual-time policyを定義し、測定上必要な場合だけSAB／multi-Workerを比較する |
| R-LIFECYCLE | Background throttle／page lifecycle／resumeがtimer storm、desync、save corruptionを起こす | Worker isolation、Rust-owned logical timer registration、generation-checked cancel／stale rejection、remaining-delay freeze／resume、one-per-task catch-up | `WM_TIMER`／TIMERPROC、audio、saveへ広げる前に各resourceのclock authorityとlong-idle browser E2Eを固定する |
| R-COMPAT | Subtle Win32／COM behaviorまたはsilent stubがbootを進め、後段stateを壊す | Caller-consumer oracle、typed fault、mutation-free negative path | Stubを正しいstateまたはfirst divergenceでのexplicit structured stopへ置き換える |
| R-GRAPHICS | Pixel format、pitch／ROP、font metric、backend差、context lossで画面が読めない | Logical surface／resourceとbrowser rendererを分離する | Supported format／stateをboundし、semantic＋visual oracleを追加し、非対応browser／GPUをtier分けする |
| R-MEDIA | Audio timing／media dependencyまたはD3D9 scopeがplanned browser subsetを超える | Corpus-bounded frontendとseparate backend adapter | Early feasibility traceを行い、広いAPI実装前にsupport bandを再設計する |
| R-DATA | Crash、quota、path差、external replacementでsaveを失う、またはsource moduleが変わる | Immutable source／writable overlay ruleとexplicit durability boundary | Read-only failureまたはlast committed stateへrollbackし、source EXE／DLLをshadow／rewriteしない |
| R-EXHAUSTION | Malformed inputまたはguest resource growthがmemory、thread、surface、log、parser limitを枯渇させる | Memory-safe bounded parser、quota、watchdog、typed exhaustion | Partial commitなしでtitleを停止し、最小diagnosticとregression fixtureを残す |
| R-BROWSER | Browser feature／hosting policy差で同revisionのbehaviorが分岐する | Versioned adapterとcapability detection | Supported browser／deployment tierを狭めるか、測定済みfallbackを用意する |
| R-PERFORMANCE | Ambient host loadまたはtitle workload growthでversioned latency／memory thresholdを外れ、clean oracleを判定できない | Fixed fixture／duration／threshold、repeated distribution、same-host bounded comparison、structured blocker | 外部workloadを勝手に停止せずisolated hostで同条件を再実行する。再現時だけprofileに基づくbounded optimizationを行う |
| R-MAINTENANCE | Toolchain drift、solo maintainerのcontext switch、demo優先がreproducibility／correctness gateを弱める | Locked dependency、WIP one、checkpoint evidence、authority rule | 一つのreproducible blockerへ戻り、source／save safetyとsemantic testを削らない |

Current primary `kernel32-archive-source-read` profileはNode／Chromeのordered semanticsとfinal memoryで
一致した。旧24-session／8,000-ms契約は17,865-call workload由来であり、89,386-callのversion 2 workloadへ
同じabsolute limitを適用したfailureはsame-workload regressionを示さなかった。Duration、workload identity、
semantic／memory digest、他15 thresholdを維持したversion 2は14 sessions／14,000 msとし、valid environmentの
最終runで17 sessions／11,232.015 ms、17/17 threshold passとなった。R-PERFORMANCEはactive blockerでは
なくなった。現在の89,429-call workloadとの差は一度だけ通るresize／system-menu continuationで、browser resource、
host wait、hot loopを追加しないためperformance gateは再baselineしていない。将来のworkload増加ではversionを
跨いだ絶対時間だけで回帰判定しない。Exact evidenceと
再測定条件は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Primary BGI executionはfile-drop acceptance flagの適用後、旧`MessageBoxA` stopがsystem.arc欠落の
error branchだったと判定し、immutable boot-archive snapshot mount経由でsystem.arcのheader／index／
`ipl._bp` member readsを完了してscript interpreterを起動した。Primaryのbounded
`USER32.dll!SetWindowPos` resizeは、USER32 geometryとGDI window surfaceをNCCALCSIZE completionで
failure-atomicに更新し、`WM_SIZE`内の`GetSystemMenu(hwnd,FALSE) -> EnableMenuItem(SC_CLOSE,0)`と最終
`WM_WINDOWPOSCHANGED`まで完了した。Logical HMENUはwindow-owned identityとしてRustに置き、専用menu generationにより
外側resize planをstaleにしない。続く`Sleep(0)`は既存scheduler authorityへ接続するbounded yieldとして閉じ、
`GlobalMemoryStatus`は既存system-information policyから分離しないdeterministic x86 snapshotとして閉じた。Host
wall-clock、browser timer、host／browser memory pressure、runtime allocator quotaはguest-visible authorityへimportしない。
`GetFileAttributesA`は既存VFS ownerのregistered file、derived directory、committed writable directoryだけを照会し、
host／browser filesystemをauthorityへしないsemantic leafとして閉じた。続く`CreateDirectoryA` gatewayは
`RuntimeFiles` ownerの別directory collection／identity／generation／planを導入し、SQLite lease／CAS／idempotencyが
durably publishした後だけRust existenceをcommitする。Private Node／Chromeはshared profileを変更しないrun-local DBで
success、duplicate／parent／file-conflict oracle、cancel、terminal drainを固定した。既存logical windowへ到達した
`USER32.dll!SetWindowPos` repeat activation variantは、既存三relationがtargetのときの
`WM_WINDOWPOSCHANGING`一回だけのvalidate-only leafとして閉じ、geometry、visibility、relation、generation不変を固定した。
続く旧`MessageBoxA`はD3D9 texture failure branchと判明したため、version 17はnativeで再現した三つの
`2048x1024` descriptorとsystem-memory surface fill chainだけを追加した。Node／Chromeはrenderer resourceを作らず
同じRust COM／graphics authorityで完了した。続く`SetWindowPos(HWND_TOP,317,154,0,0,0x25)`は、
`WM_WINDOWPOSCHANGING -> WM_NCCALCSIZE -> WM_WINDOWPOSCHANGED -> nested WM_MOVE`のbounded frame-change
moveとして閉じた。GeometryはNCCALCSIZE completionでのみcommitし、GDI surfaceと三つのwindow relationを保持する。
現在は`ShowWindow(hwnd,SW_SHOWNORMAL)`の`endpoint-contract-missing` stopでNode／Chromeが一致する。
これは新しいstorage／browser capability blockerではない。
このevidenceはgeneral menu tree／window resize、rendering、input、audio milestone、general asset breadthを閉じない。

初期storage locationの選択はADR-006で閉じた。Remote service、cloud同期、OPFS／IndexedDBへの移行は現在の未解決実装ではなく、localhost vertical slice後に具体的な要求が出た場合だけ再開する。

Primary `SetTimer`のidentity、owner thread、interval、TIMERPROC、replacement generation、process teardownに加え、
16回のlogical expiry、empty current-thread queueへのlow-priority `WM_TIMER` materialization、missed-period coalescing、
`TranslateMessage` validation、同一registration generationでのTIMERPROC dispatchまでRust authorityとして成立した。
Native bounded traceと同数のrecurring breadthに加え、253回目のsemantic main-timer cycleでprimary left-button actionを
commitし、その消費後にも2件のTIMERPROCを完了する合成profileをNode／Chromeで一致させた。Browser task timingを
guest-visible clockへ流入させないinput／timer compositionは閉じたが、third `Present`やvisible responseには進まない。
続くprofileは同じ`RuntimeInput` mapping、logical cursor、foreground window、`RuntimeMessages`、scheduler wakeをjoint
preflightし、`WM_LBUTTONDOWN`をWndProcへdispatchしてmatching releaseをRust queueに保持した。Queued pointer
message productionの成立性riskは閉じた。残るriskは`SetFocus` continuationとrelease dispatch、capture／move／wheel、
replacement／`KillTimer` race、background freeze／resumeとcatch-upをscheduler、message queue、page lifecycleで
順序付けることである。

Primary `CreateThread`／`ResumeThread`のlogical identity、stack／TEB／FS bootstrap、typed handle、suspend count、
Worker context registrationに加え、直後の`SetThreadPriority`でabove-normal logical stateをthread ownerへ保存する
contractはRust authorityを保って成立した。残るthread riskはcreation／priority mutation feasibilityではなく、caller
block時のselection、priorityが実際のfairnessへ影響する必要性、DLL／TLS lifecycle、last-thread semanticsである。
Synchronous resumeはnative caller orderingを保つため即時preemptせず、現checkpointのpriorityはhost／browser
schedulerへimportしない。

Primary `GetDC(NULL)`のlogical primary-monitor relationに加え、到達した`EnumFontFamiliesExA`のWindows-932
filter、single-record catalog／metrics、callback lifetime／result propagationはRust authorityを保って成立した。
Native未初期化tailもsemantic stringと分離でき、exact `CreateFontA` requestからdistinct logical font objectを
`RuntimeGdi`へfailure-atomicに作成できた。NULL-source compatible memory DC、共有default font／bitmap、exact
`DeleteDC` lifetime、DIB bytes／object selection／introspection／deletion、selected FontAのbounded outline metric
outputも同じownerへ接続し、host／browser font discoveryやnative HDCは同期APIへ取り込んでいない。次のgraphics
riskはnative evidenceでguest-visibleと確認した`TextOutA`のDIB mutation、glyph rasterization／substitutionを可読性要件へ
接続できるかである。Host HDCやDOM `Screen`、browser `FontFace`を第二のauthorityにしない。

Default IME windowのlogical HWND、thread default HIMC、app-window association、status-window close／open requestは
Rust ownerへ分離し、NULL detachとruntime-owned restoreをfailure-atomicにmodel化した。BGI.gdbはglobal stateを持つ
writable profile objectであり、そのcontent hash変化をsource corpus identity mismatchとして扱わない。既存objectは
new SQLite profileだけへseedし、以後のread／writeを配布directoryから分離する。Canonical rerunはdetach後のpollingを
runnable childが選択されないscheduler progress不足と分類し、競合中だけ数える64 API-boundary quantumでNode／Chromeの
両thread実行と一回のswitchを一致させた。Reached timer waitはsingle current laneのbounded immediate roundtripまで
成立した。今回のscheduler round tripで二つのrunnable lane選択は固定できたが、残るthread riskはCPU-bound laneのpreemption、cross-thread wait／timer wake ordering、
child exit／cleanupをprimary flowで固定できていないことである。Storage riskはprimary guest write、browser／server restart後
load、kill injectionであり、配布fileを復元・書換して解消しない。IME compositionやDOM inputへscopeを広げる前に、
wait pacingとstorage vertical sliceをそれぞれのowner境界で進める。

Primary `SetWaitableTimer`のrelative one-shot armと
`WaitForSingleObjectEx(handle,10,TRUE)`のlogical deadline、exact generation expiration、manual-reset signal、
finite timeout、alertable-flag acceptanceはtimer／object／wait／schedulerのjoint preflightで成立した。
Current semantic handlerはhost／browser clockをauthorityへimportせずdeadlineまでdeterministically fast-forwardする。
wait result correctnessと到達idle-cycle pacingはbounded profileで固定された。残るriskはcontinuous
main-loop progressionとreal-time pacingの長時間／browser-lifecycle挙動である。Async delay completionは
thread／request／generation／lifetimeを再検証し、cancel／teardown、background freeze／resume、bounded catch-upを
partial mutationなしで順序付ける必要がある。APC deliveryは別scopeである。

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
