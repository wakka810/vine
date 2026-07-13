# Vine documentation

## 文書ステータス

- ステータス: Proposed
- 開発形態: 1 人、完全個人用
- 主目標: fingerprint 済みサクラノ詩 `BGI.exe` の full-play runtime

このディレクトリは、Vine を「Windows 向け VN/ADV 実行ファイルを、元の配布ファイルを原則として変更せずブラウザ上で再ホストする Windows userspace 互換ランタイム」として開発するための基準文書である。

現在状態の正本は[`current-checkpoint.json`](current-checkpoint.json)であり、ルート
READMEのCurrent Statusはそこから生成する。本書と個別checkpoint文書はarchitecture、観測理由、
non-claimsを記録するが、current call数やnext stopの第二の正本にはしない。

## プロジェクトの位置づけ

```text
Browser-native Windows userspace compatibility runtime
for Windows VN/ADV executables
```

日本語では、Windows 向け VN/ADV 実行ファイルをブラウザ上で再ホストするための、Web ネイティブな Windows userspace 互換ランタイムと位置づける。

```text
Windows PE32 / PE32+ executable and DLL
                  |
                  v
            PE image loader
                  |
                  v
         x86 / x86_64 execution
                  |
                  v
       Win32 / Win64 compatibility
                  |
                  v
  GDI / DirectX / audio compatibility
                  |
                  v
       Browser Web API backends
```

初期実装は PE32 / i386 と Win32 を基盤とし、GDI、DirectSound、サクラノ詩が要求する bounded D3D9／DirectInput 8 subset を active scope とする。DirectDraw は同系統 title との比較 branch として扱う。PE32+ / x86_64、D3D11 / DXGI、XAudio2、Unity 系 runtime は将来の独立した成立性検証対象である。

active completion target は第三者向けの汎用 runtime や multi-title release ではなく、fingerprint 済みのサクラノ詩を手元の Chrome で全編不自由なくプレイできる 1-title runtime である。短い scenario は段階的 checkpoint として使うが、最終完了の代用にはしない。拡張可能な Windows userspace 互換境界は維持するが、個人開発の critical path に一般化や公開 release の作業を置かない。

## 文書一覧

| 文書 | 役割 | 主な内容 |
|---|---|---|
| [current-state-manifest.md](current-state-manifest.md) | current stateをどこで更新するか | schema区分、README生成、canonical再現command、検証境界 |
| [corpus-inventory.md](corpus-inventory.md) | 何を最初に動かすか | ローカル title inventory、primary target、静的依存、調査待ち evidence |
| [toolchain-spike.md](toolchain-spike.md) | 何を再現生成できるか | LLVM／Wasm baseline、PE32 fixture、次の engine 比較条件 |
| [x86-engine-scorecard.md](x86-engine-scorecard.md) | どの x86 engine を先に試すか | 4 候補の比較、v86 実行 evidence、adapter blocker と採用 gate |
| [adr-003-guest-memory.md](adr-003-guest-memory.md) | guest memoryをどこが所有するか | Rust byte authority、v86 execution mirror、slice同期／failure policy |
| [guest-memory-ingress.md](guest-memory-ingress.md) | production memory ingressが何を所有するか | sparse region／page API、fault／invalidation、fixture／BGI layout evidence |
| [memory-delta-and-transactions.md](memory-delta-and-transactions.md) | mutationと失敗をどう同期するか | bidirectional dirty pages、binary transport、generation ack、rollback |
| [memory-writeback.md](memory-writeback.md) | guest writeをどうRust authorityへ戻すか | PTE Dirty discovery、atomic multi-page reconciliation、two-slice evidence |
| [generic-api-dispatcher.md](generic-api-dispatcher.md) | API固有checkpointを共通runtimeへどう移すか | 23 contracts、RuntimeState、manifest／generic runner、dynamic endpoint dispatch |
| [bgi-generic-runtime-reentry.md](bgi-generic-runtime-reentry.md) | frozen BGI evidenceをproduction runtimeへどう戻すか | process module graph、automatic binary writeback、21 generic calls、critical-section first stop |
| [bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md) | reached x86 critical sectionをどう所有するか | native 24-byte oracle、process-owned recursion、14 BGI initializations、DecodePointer first stop |
| [bgi-pointer-decoding.md](bgi-pointer-decoding.md) | process pointer tokenをどう逆引きするか | native inverse evidence、strict owned-token lookup、FlsAlloc dynamic stop |
| [bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md) | reached FLS slotをどう所有するか | native callback/index evidence、bounded process slots、dynamic dispatch、HeapAlloc first stop |
| [bgi-private-heap-allocation.md](bgi-private-heap-allocation.md) | reached private-heap blockをどう所有するか | native flags/size evidence、transactional guest backing、FlsSetValue first stop |
| [bgi-fiber-local-storage-value.md](bgi-fiber-local-storage-value.md) | reached FLS valueをどこへ保存するか | native BOOL/LastError evidence、selected-thread state、InterlockedIncrement first stop |
| [bgi-interlocked-increment.md](bgi-interlocked-increment.md) | reached atomic LONGをどう更新するか | native wrapping evidence、checked guest write、second HeapAlloc variant stop |
| [bgi-private-heap-second-allocation.md](bgi-private-heap-second-allocation.md) | reached second heap blockをどう保持するか | versioned size gate、unsealed-owner delta、GetStdHandle first stop |
| [bgi-standard-input-handle.md](bgi-standard-input-handle.md) | reached standard input identityをどう所有するか | native result／consumer、logical handle、GetFileType first stop |
| [bgi-file-type.md](bgi-file-type.md) | reached stdin typeをどう分類するか | native FILE_TYPE_CHAR、logical lookup、stdout first stop |
| [bgi-standard-output-handle.md](bgi-standard-output-handle.md) | reached stdout identityをどう所有するか | versioned selector profile、logical stdout、output type first stop |
| [browser-runtime-worker.md](browser-runtime-worker.md) | Chrome内でruntime全経路をどこまで通せるか | Worker guest CPU、async completion、nested callback、pixel／audio／storage、cancel／watchdog、30分stress |
| [pe-loader-ingress.md](pe-loader-ingress.md) | immutable PE32 planをどこまで作れるか | Rust workspace、bounded parser／plan、fixture contract、BGI W7 stop |
| [pe-materialization.md](pe-materialization.md) | PE image bytesをどこまで構築できるか | source copy、zero-fill、`HIGHLOW`、IAT-original verification、BGI stop |
| [pe-import-binding.md](pe-import-binding.md) | runtime import endpointをどこまで所有できるか | all-import planning、atomic IAT commit／rollback、unresolved retry、thunk arena／ID |
| [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md) | 未実装static importをどうseal可能にするか | identity付きunsupported traps、controlled test、BGI full bind |
| [v86-entry-ingress.md](v86-entry-ingress.md) | production memoryをCPUへどこまで渡せるか | semantic reflection、invalidation ack、bounded stack、entry-to-thunk evidence |
| [v86-cdecl-ingress.md](v86-cdecl-ingress.md) | 最初のproduction serviceをどこまでreturnできるか | exact thunk metadata、fixed cdecl frame／return、guest cleanup、callback-service ingress |
| [v86-callback-return-ingress.md](v86-callback-return-ingress.md) | production callbackをどこまで往復できるか | separate control owner／sentinel、frame ID、checked stack、exact callback return |
| [v86-outer-return-ingress.md](v86-outer-return-ingress.md) | outer callback serviceをどこまで完了できるか | frame retirement、fixed outer cdecl return、guest result／entry continuation、structured fetch fault |
| [v86-entry-return-ingress.md](v86-entry-return-ingress.md) | guest entry completionをどこまで所有できるか | typed process-entry sentinel、exact return classification、result／stack balance |
| [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md) | initial stackをどこまで所有できるか | Rust stack owner／region、bootstrap metadata、reflection、adapter-local stack removal |
| [v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md) | i386 FS bootstrapの未実装境界はどこか | native TEB／PEB self-oracle、import-free reflection、zero-FS-base exact `#PF` |
| [v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md) | 最小TEB／PEBをどこまで所有できるか | typed Rust regions、exact fields、FS-base restore、entry completion |
| [v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md) | process parametersの未実装境界はどこか | native command-line oracle、PEB `+0x10` zero、explicit result `2` |
| [v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md) | 測定済みcommand-line pathをどこまで所有できるか | typed regions、exact UTF-16 bytes、source-v86 entry completion |
| [v86-bgi-first-trap.md](v86-bgi-first-trap.md) | primary titleをどこまで実行できるか | bounded 388-page transfer、VGA hole回避、BGI entry、first runtime-owned trap |
| [v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md) | first reachable APIをどこまでreturnできるか | controlled clock、stack page reconciliation、`FILETIME` delta、stdcall return、next trap |
| [v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md) | second reachable APIをどこまでreturnできるか | controlled process identity、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md) | third reachable APIをどこまでreturnできるか | controlled thread identity、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md) | fourth reachable APIをどこまでreturnできるか | controlled monotonic sample、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md) | fifth reachable APIをどこまでreturnできるか | controlled 64-bit counter、generation 26 delta、BOOL return、next trap |
| [v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md) | sixth reachable APIをどこまでreturnできるか | native x86 observation、controlled `STARTUPINFOW`、generation 27 delta、VOID return、next trap |
| [v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md) | seventh reachable APIをどこまでreturnできるか | native heap-policy observation、exact four-argument variant、generation-preserving BOOL return、next trap |
| [v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md) | eighth reachable APIをどこまでreturnできるか | native heap lifecycle observation、typed logical heap handle、generation-preserving `HeapCreate` return、next trap |
| [v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md) | ninth reachable APIをどこまでreturnできるか | native module-handle observation、bounded UTF-16 lookup、typed logical loaded-module handle、next trap |
| [v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md) | tenth reachable APIをどこまでreturnできるか | native dynamic-export observation、register-indirect IAT provenance、callable `FlsAlloc` endpoint、next lookup |
| [v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md) | eleventh reachable APIをどこまでreturnできるか | ordered endpoint pair、persistent ESI provenance、first endpoint guest store、`FlsGetValue` return／next lookup |
| [v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md) | twelfth reachable APIをどこまでreturnできるか | ordered endpoint triple、prior endpoint guest stores、`FlsSetValue` return／next lookup |
| [v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md) | thirteenth reachable APIをどこまでreturnできるか | ordered four-endpoint page、`FlsFree` return／store、`TlsAlloc` next trap |
| [v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md) | fourteenth reachable APIをどこまでreturnできるか | typed logical TLS index、`TlsAlloc` return、`TlsSetValue` first consumer |
| [v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md) | fifteenth reachable APIをどこまでreturnできるか | initial-thread TLS endpoint value、three-page reconciliation、`EncodePointer` next trap |
| [v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md) | sixteenth reachable APIをどこまでreturnできるか | focused pointer evidence、controlled encoded-NULL token、next non-NULL call |
| [v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md) | seventeenth reachable APIをどこまでreturnできるか | executable initializer source、second token／guest store、`FlsAlloc` endpoint next call |
| [windows-development.md](windows-development.md) | Windows で何から始めるか | toolchain、Frida trace、最初の縦断 checkpoint、test 方針 |
| [windows-native-evidence.md](windows-native-evidence.md) | Windows で何が実測済みか | W0 version、BGI trace v1／v2、selected IDirect3D9 root、W1／W2／W3、W7 loader／memory／entry／stack／FS ingress |
| [requirements.md](requirements.md) | 何を、どこまで作るか | 目的、スコープ、機能要件、非機能要件、personal target 定義 |
| [target-architecture.md](target-architecture.md) | どの境界で作るか | コンポーネント、実行モデル、同期・非同期境界、runtime robustness |
| [risks-and-open-questions.md](risks-and-open-questions.md) | 何が未確定で、何が失敗要因か | 意思決定待ち、リスク台帳、停止条件、ADR 候補 |
| [testing-and-acceptance.md](testing-and-acceptance.md) | 何をもって動作したと判定するか | テスト階層、fixture、品質指標、リリース判定 |
| [checks-and-ci.md](checks-and-ci.md) | canonical gateをどう実行するか | fast／fixture E2E／benchmark／adapter／private corpus、Linux GitHub Actions、fuzz |
| [v86-browser-benchmark.md](v86-browser-benchmark.md) | v86を継続するか | Chrome Worker CPU／ABI／memory／audio／background／steady-memory gate |
| [runtime-object-model.md](runtime-object-model.md) | runtime object identityをどう所有するか | typed handle、generation、close／stale、waitable process／thread lifetime |
| [runtime-module-graph.md](runtime-module-graph.md) | process module identityと順序をどう所有するか | canonical identity、dependency／reference、attach／detach order |
| [runtime-module-images.md](runtime-module-images.md) | mapped guest module／exportをどう所有するか | sealed image identity、name／ordinal／forwarder resolution、failure atomicity |
| [runtime-dll-lifecycle.md](runtime-dll-lifecycle.md) | guest importとDLL process callbackをどう結ぶか | direct／runtime IAT、static dependency、attach unwind／retry、reverse detach |
| [runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md) | process／thread terminationをどう所有するか | bootstrap／running、exit request／completion、atomic signal order |
| [runtime-thread-registry.md](runtime-thread-registry.md) | logical thread stateをどう分離するか | controlled second thread、selection／exit、deterministic all-lane termination |
| [runtime-wait-queue.md](runtime-wait-queue.md) | waitable objectとschedulerをどう接続するか | one-object registration、signal／wake event、already-signaled／exit cancellation |
| [runtime-message-queue.md](runtime-message-queue.md) | thread messageをどこが所有するか | bounded raw FIFO、empty block／first-message wake、ordered dequeue／exit cleanup |
| [runtime-heap-backing.md](runtime-heap-backing.md) | private heap bytes／lifetimeをどこが所有するか | bounded guest arena、allocation ID／alignment／initialization、free／reuse、heap／process teardown |
| [roadmap.md](roadmap.md) | どの順序で実装するか | 依存関係、フェーズ、タスク分解、各フェーズの受け入れ条件 |

推奨する読み順は、corpus 台帳、要件、目標アーキテクチャ、リスク、検証、ロードマップの順である。ロードマップ内の `REQ-*`、`NFR-*`、`R-*`、`Q-*`、`AC-*` は各文書の識別子を参照する。

## 開発上の原則

1. タイトル固有のエンジン移植ではなく、PE と Windows userspace API の互換境界を実装する。
2. Windows API／DirectX API の必要部分を browser Web API へ直接接続する。
3. ブラウザでネイティブ x86 コードを直接実行せず、隔離された guest 実行器で命令を emulate または translate する。
4. 最初から広い Win32 互換を名乗らず、機械可読な API 実装状況とタイトル別互換性を記録する。
5. 実ゲームより先に、合成 PE fixture で各層を検証する。
6. malformed input、無限 loop、resource 上限を合成 fixture で再現可能に検査する。
7. 将来の PE32+ / x86_64 / D3D11 対応点は確保するが、サクラノ詩 prototype の設計を抽象化しすぎない。
8. Windows native trace で到達した経路から、広く浅い vertical checkpoint を一つずつ通す。subsystem の全面実装を先行させない。
9. automated test は各実装境界につき代表 contract 1 本を原則とし、実 regression、曖昧な fault／ABI semantics、title failure を最小化した場合だけ増やす。

## 用語

| 用語 | 意味 |
|---|---|
| guest | Vine 内でロード、実行される Windows 実行ファイルと、その仮想アドレス空間 |
| host | ブラウザ、Web API、および Vine の JavaScript/Wasm 実行環境 |
| compatibility runtime | PE loader、CPU 実行系、ABI、Windows API facade、Web backend の総体 |
| synthetic DLL | Microsoft の DLL バイナリを含まず、Vine が export と API 挙動を提供する論理 DLL |
| fixture | 挙動検証のために作成した小さな PE 実行ファイル |
| title corpus | 互換性検証に使用する代表タイトル群 |
| compatibility profile | タイトル、必要 API、実装状態、既知の制約、検証結果を結び付ける記録 |
| vertical slice | PE load から CPU、API bridge、Web backend までを最小機能で貫通させる検証 |

## ステータス表記

| 表記 | 意味 |
|---|---|
| Proposed | 合意前の提案 |
| Accepted | ADR またはレビューで採用済み |
| In progress | 実装または検証中 |
| Implemented | 受け入れ条件を満たし、CI 証跡がある |
| Deferred | 現在のリリース範囲外 |
| Rejected | 理由を記録して不採用 |

文書内に明記がない項目は `Proposed` とみなす。機能を `Implemented` に変更するには、[testing-and-acceptance.md](testing-and-acceptance.md) に定義した証跡が必要である。

## 文書の更新規則

- 仕様や技術選択を確定するときは ADR を追加し、関連する `Q-*` を `Resolved` にする。
- マイルストーンの完了は、タスクの実装完了ではなく、対応する `AC-*` の証跡で判定する。
- 対応 API を追加したら、API inventory、テスト、互換性 profile を同じ変更で更新する。
- 日程見積もりは成立性検証後に更新し、未検証の見積もりを納期として扱わない。
