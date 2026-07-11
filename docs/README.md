# Vine documentation

## 文書ステータス

- ステータス: Proposed
- 基準日: 2026-07-10
- 対象: current `main` worktree
- 実装成熟度: Phase 0 discovery／W1／W2／W3／W7 PE32 load／memory／entry return／stack bootstrap／bootstrap FS first-stop ingress。native TEB／PEB offset oracleとzero-FS-base exact faultは存在するが、general v86 memory／process integration、Rust-owned PEB／TEB、DLL／export resolution、Win32 runtime、browser app、CIはまだない
- 開発形態: 1 人、完全個人用
- primary 開発環境: Windows 11 25H2（profileはPro、現checkerはHome）、Chrome、`V:\src\vine`
- 主目標: fingerprint 済みサクラノ詩 `BGI.exe` の full-play runtime

このディレクトリは、Vine を「Windows 向け VN/ADV 実行ファイルを、元の配布ファイルを原則として変更せずブラウザ上で再ホストする Windows userspace 互換ランタイム」として開発するための基準文書である。

調査基準リビジョン `4e8f29d` にはルートの `README.md` しかなかった。現在は本ドキュメント、Node check、再現生成するPE32 fixtures、固定したv86 source adapter、Windows trace、W1／W2／W3、high-address memory、Rust W7 load／memory／entry return／stack bootstrap、bootstrap FS first-stop ingressがある。各checkpointは限定したcorpus／feasibility／oracle evidenceであり、production Windows compatibility runtimeの完成を示さない。

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
| [corpus-inventory.md](corpus-inventory.md) | 何を最初に動かすか | ローカル title inventory、primary target、静的依存、調査待ち evidence |
| [toolchain-spike.md](toolchain-spike.md) | 何を再現生成できるか | LLVM／Wasm baseline、PE32 fixture、次の engine 比較条件 |
| [x86-engine-scorecard.md](x86-engine-scorecard.md) | どの x86 engine を先に試すか | 4 候補の比較、v86 実行 evidence、adapter blocker と採用 gate |
| [adr-003-guest-memory.md](adr-003-guest-memory.md) | guest memoryをどこが所有するか | high-address／R／W／X測定、候補比較、provisional CPU integration boundary |
| [guest-memory-ingress.md](guest-memory-ingress.md) | production memory ingressが何を所有するか | sparse region／page API、fault／invalidation、fixture／BGI layout evidence |
| [pe-loader-ingress.md](pe-loader-ingress.md) | immutable PE32 planをどこまで作れるか | Rust workspace、bounded parser／plan、fixture contract、BGI W7 stop |
| [pe-materialization.md](pe-materialization.md) | PE image bytesをどこまで構築できるか | source copy、zero-fill、`HIGHLOW`、IAT-original verification、BGI stop |
| [pe-import-binding.md](pe-import-binding.md) | runtime import endpointをどこまで所有できるか | thunk arena／ID、IAT prefix binding、owner seal、controlled fixture／BGI stop |
| [v86-entry-ingress.md](v86-entry-ingress.md) | production memoryをCPUへどこまで渡せるか | semantic reflection、invalidation ack、bounded stack、entry-to-thunk evidence |
| [v86-cdecl-ingress.md](v86-cdecl-ingress.md) | 最初のproduction serviceをどこまでreturnできるか | exact thunk metadata、fixed cdecl frame／return、guest cleanup、callback-service ingress |
| [v86-callback-return-ingress.md](v86-callback-return-ingress.md) | production callbackをどこまで往復できるか | separate control owner／sentinel、frame ID、checked stack、exact callback return |
| [v86-outer-return-ingress.md](v86-outer-return-ingress.md) | outer callback serviceをどこまで完了できるか | frame retirement、fixed outer cdecl return、guest result／entry continuation、structured fetch fault |
| [v86-entry-return-ingress.md](v86-entry-return-ingress.md) | guest entry completionをどこまで所有できるか | typed process-entry sentinel、exact return classification、result／stack balance |
| [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md) | initial stackをどこまで所有できるか | Rust stack owner／region、bootstrap metadata、reflection、adapter-local stack removal |
| [v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md) | i386 FS bootstrapの未実装境界はどこか | native TEB／PEB self-oracle、import-free reflection、zero-FS-base exact `#PF` |
| [windows-development.md](windows-development.md) | Windows で何から始めるか | toolchain、Frida trace、最初の縦断 checkpoint、test 方針 |
| [windows-native-evidence.md](windows-native-evidence.md) | Windows で何が実測済みか | W0 version、BGI trace v1／v2、selected IDirect3D9 root、W1／W2／W3、W7 loader／memory／entry／return／stack／FS first-stop ingress |
| [requirements.md](requirements.md) | 何を、どこまで作るか | 目的、スコープ、機能要件、非機能要件、personal target 定義 |
| [target-architecture.md](target-architecture.md) | どの境界で作るか | コンポーネント、実行モデル、同期・非同期境界、runtime robustness |
| [risks-and-open-questions.md](risks-and-open-questions.md) | 何が未確定で、何が失敗要因か | 意思決定待ち、リスク台帳、停止条件、ADR 候補 |
| [testing-and-acceptance.md](testing-and-acceptance.md) | 何をもって動作したと判定するか | テスト階層、fixture、品質指標、リリース判定 |
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
