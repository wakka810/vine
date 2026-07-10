# Vine agent guide

このファイルは、Vine の作業を別のエージェントまたは新しいセッションが継続するための最初の参照点である。作業開始時は必ず全文を読み、現在の Git 状態と active queue を確認する。詳細仕様は `docs/` を正本とするが、本書に記録した利用者の最新決定と矛盾する記述を見つけた場合は、作業を進める前に関連文書を本書へ揃える。

## 1. Mission

Vine は、Windows 向け VN/ADV の元の `.exe`／`.dll` をブラウザ上で実行するための、browser-native Windows userspace compatibility runtime である。

```text
Windows exe / dll
        |
        v
PE32 / PE32+ loader
        |
        v
x86 / x64 execution engine
        |
        v
Win32 / Win64 API compatibility
        |
        v
GDI / DirectX / audio compatibility
        |
        v
Browser Web API backends
```

ゲームエンジン、シナリオ VM、タイトル固有 asset decoder を移植するのではなく、Windows executable と Windows userspace API の実行契約を再ホストする。

### Primary target

- Title: サクラノ詩
- Entry: `V:\softs\サクラノ詩\BGI.exe`
- Format: PE32 / i386
- Size: `1,193,472` bytes
- SHA-256: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Runtime string: `Ethornell - BURIKO General Interpreter ( Version : 1.614.1 - Compatibility : 1.72 )`

最終到達点は、特定の短いデモや代表シーンではない。この fingerprint のサクラノ詩を Chrome 上で起動し、描画、入力、音声、全シナリオ、選択肢、save／load を含め、native Windows 実行と同様に全編を不自由なくプレイできる状態である。

この目標はサクラノ詩 1 build の完全プレイを意味する。任意の Win32 application、全 VN、別 build、PE32+／x64 まで同時に保証するものではない。

## 2. Scope

### Active scope

- PE32 / i386 user-mode execution
- 必要な x87、MMX、SSE、SSE2
- cdecl、stdcall、thiscall、必要な fastcall、guest callback
- PE loader、DLL lifecycle、process bootstrap
- Kernel32、User32、GDI、GDI+、COM の到達範囲
- サクラノ詩が到達する D3D9、DirectInput 8、DirectSound、WinMM／MCI
- Windows-932／Unicode、font、file、registry、save persistence
- Chrome の Canvas／WebGL2／WebGPU、Web Audio、OPFS／IndexedDB、DOM input への接続

### Deferred scope

- PE32+ / x86_64 / Win64 ABI
- D3D11 / DXGI / XAudio2
- Unity runtime
- multi-title compatibility の一般化
- 第三者向け配布や public beta

### Non-goals

- 特定ゲームエンジンの移植
- タイトルごとのシナリオ VM 再実装
- asset 形式ごとの decoder／変換 pipeline
- Windows driver、kernel、service、OS 全体の再現
- OS 全体の仮想化

## 3. User And Environment

- 利用者、開発者、maintainer は同一の 1 人である。
- 完全個人用プロジェクトである。team process や public release process を active work に持ち込まない。
- OS profile: Windows 11 Pro 25H2。2026-07-10 の `Win32_OperatingSystem` は Windows 11 Home、build `10.0.26200.8655` を返した。edition 差は未解決として記録し、runtime gate には build／architecture の実測値を使う。
- Repository: `V:\src\vine`
- Primary browser: current stable Chrome。evidence には実際の version を記録する。
- Primary shell: PowerShell
- MSYS2: v86 source adapter の再 buildに必要な `make`／`sh` を提供する。`C:\msys64\usr\bin` を user `PATH` に置く。
- Production language direction: Rust + TypeScript

### Language ownership

Rust は CPU adapter、guest memory、PE loader、ABI gateway、低レベル runtime state を担当する。TypeScript は Windows API facade、scheduler integration、browser graphics／audio／storage／input backend、browser UI、diagnostics、開発ツールを担当する。

実測により境界を調整してよいが、hot guest execution や guest pointer ownership を TypeScript 側へ無秩序に漏らさない。

### Agent authority

エージェントは、作業を進めるために次を個別確認なしで実行してよい。

- 開発ツール、package、SDK、debugger の導入と更新
- executable の起動、Frida／WinDbg の attach、trace、breakpoint、dump
- 不要になった対象 process の終了
- repository 内の実装、fixture、tool、documentation の追加と変更
- build artifact、trace、save、設定、調査用 file の作成
- 意味のある checkpoint 単位の commit と remote への push

ゲーム上のクリックや選択など、native title の手動操作は利用者が行う。エージェントは必要な操作とタイミングを短く伝え、それ以外の起動、attach、収集、停止、解析を自律的に行う。

### Distributed game files

配布ファイル全体は、通常は変更しない。native 観測は元ディレクトリで実行してよく、ゲーム自身が設定や save を作ることも許容される。意図的に配布ファイルを編集する実験が必要な場合は、先に copy または backup を作り、対象、理由、変更前後の hash を記録する。

browser runtime の通常動作では、import した source を immutable snapshot として扱い、save／設定は別の writable overlay へ置く。ゲーム本体、同梱 DLL、asset、raw trace、save data を Git に追加しない。

## 4. Engineering Principles

### Broad, shallow, demonstrable progress

- subsystem を一つずつ完成させようとしない。
- headless execution、window、first frame、input、audio、persistence の短い vertical checkpoint を順に通す。
- primary implementation task は常に一つだけ In progress にする。
- 別 subsystem の発見事項は backlog に記録し、その場で横道へ入らない。
- 各 session の最後に、再現 command、観測結果、次の最小 action を残す。

### Trace-driven compatibility

- static import の総数で実装順を決めない。
- native trace で runtime-reachable と確認した module、API、COM method、format、state を優先する。
- 未到達 API を先回りして網羅実装しない。
- API が未対応なら、成功を装う silent stub ではなく、最初の mismatch を構造化して停止・記録する。
- title failure は可能なら一般的な CPU／ABI／Win32 contract に還元する。EIP や title hash だけで code を skip しない。

### Controlled fixtures before complex applications

最初の runtime milestone には Windows 11 の `notepad.exe` を使わない。現行 Notepad は小さな制御可能 PE32 fixture ではなく、architecture と配布形態の違いが初期 scope を不必要に広げる可能性がある。

最初は repository の `fixtures/pe32/abi-smoke/` を使い、その次に最小の Win32 window／message／pixel fixture を追加する。fixture で execution、PE、ABI、API bridge の境界を一つずつ通した後、BGI.exe を first unsupported point まで進める。

### Minimal tests

- 一つの実装境界または checkpoint につき、代表 automated contract test 1 本を原則とする。
- test を増やすのは、実 regression、calling convention／fault ordering の曖昧さ、または title failure を最小 fixture へ還元した場合だけである。
- 未実装 subsystem の test suite、広い compiler matrix、長時間 fuzz／soak を先に作らない。
- test 数ではなく、checkpoint が再現可能で次の層へ接続できることを進捗とする。

### Conservative code structure

- Rust／TypeScript とも既存 pattern を優先する。
- CPU engine は adapter の背後に置き、v86 private state を runtime 全体へ漏らさない。
- guest address、host offset、size の変換は checked type／function に集約する。
- sync Win32 API と async Web API の差は scheduler／host service boundary で吸収する。
- graphics frontend と backend、audio frontend と backend、source storage と writable overlay を分離する。
- 将来交換する可能性が明確な境界だけ抽象化し、未到達機能のための framework を作らない。

## 5. Current Architecture Direction

```text
Chrome main thread
  UI / canvas / DOM input / lifecycle
                 |
                 v
Runtime Worker
  scheduler / Windows API facade / diagnostics
                 |
                 v
Rust/Wasm core
  PE loader / guest memory / x86 adapter / ABI
                 |
                 v
v86 interpreter baseline
```

- Guest thread はまず一つの Runtime Worker 上で cooperative scheduling する案を baseline とする。
- x86 execution は caller-defined instruction budget で必ず host へ制御を戻す。
- API import は runtime-owned thunk address へ解決し、guest exception／sentinel から host dispatcher へ戻す。
- host から guest callback を呼ぶときは、guest stack frame と callback return sentinel を明示的に作る。
- guest memory は 4 GiB virtual address view と bounded physical allocation を分離する必要がある。方式は未決定。
- renderer は共通 surface model の背後で WebGL2／WebGPU を比較する。方式は未決定。
- audio は DirectSound／WinMM frontend と Web Audio backend を分離する。mixer、AudioWorklet、clock ownership は未決定。

## 6. Engine Decision

v86 を現在の working baseline とする。具体的な blocker が出ない限り、他 engine の広い比較や自作 interpreter に時間を使わない。ただし production engine として固定せず、adapter contract を維持し、必要なら交換できる状態にする。

### Pinned evidence

- npm package: `v86@0.5.424`
- source revision: `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`
- source build Rust: `1.97.0`
- adapter artifact size: `1,882,836` bytes
- adapter SHA-256: `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26`
- Wasm imports／exports: `22`／`809`
- CPU state ABI: version `1`, size `640` bytes

Verified behavior:

- full PC boot なしの i386 integer／stack／call／x87／SSE smoke
- exact instruction budgets `0`、`7`、続けて `5`
- structured CPL3 `#DE`、`#UD`、`#GP`、`#PF`
- CPL0 halt reason
- GPR、flags、segment／control state、x87、XMM／MXCSR の save／reset／restore／resume 1 sequence

未検証または未実装:

- sparse guest memory と page ownership
- API thunk classification と callback return
- Chromium Worker integration
- broad ISA correctness と target workload performance
- optimized execution path

詳細は `docs/x86-engine-scorecard.md` と `spikes/x86-engine/README.md` を参照する。

## 7. Work Completed From The Initial Repository

初期 repository は一行の `README.md` だけだった。現在までに次を作成した。

1. 要件、architecture、risk、test、roadmap、corpus、toolchain、engine、Windows handoff の文書一式。
2. 7 title の PE32／i386 fingerprint と static dependency inventory。
3. LLVM 22 だけで再現生成できる PE32 EXE／DLL／guest callback fixture。
4. host JavaScript から動かす freestanding Wasm smoke。
5. pin 済み v86 prebuilt Wasm の CPU smoke と artifact contract check。
6. v86 source 用 bounded-execution patch、state-module patch、tracked Rust state module。
7. exact budget、fault、CPU state round trip を検証する source adapter runner。
8. Windows toolchain／BGI fingerprint checker。
9. Frida による focused startup tracer。module load、dynamic symbol lookup、file、window、GDI／GDI+、D3D9、DirectInput、DirectSound、WinMM、COM entry を JSONL に出す。
10. raw trace の sequence、thread-local API pairing、module、API、A/W file wrapper を検証する deterministic summarizer と contract test。
11. Windows W0、60 秒 BGI startup trace、native EXE -> DLL -> EXE callback oracle の tracked evidence。

2026-07-10 に Windows 11 25H2、Chrome 150、Frida 17.15.4 で `npm run check` と BGI fingerprint を再確認した。BGI startup trace は main window、D3D9、DirectInput 8、DirectSoundCreate8、GDI+ と title resource band まで到達し、native `abi-smoke.exe` は callback 後に exit code `0` となった。詳細は `docs/windows-native-evidence.md` を参照する。Chrome 上で実行する runtime はまだ存在しないため、Chrome execution smoke は未実施である。

まだ存在しないもの:

- production Rust／TypeScript workspace
- guest memory implementation
- PE parser／loader implementation
- ABI thunk／callback gateway
- scheduler／Runtime Worker
- Win32 API compatibility implementation
- browser application
- graphics、audio、storage backend
- BGI.exe を Vine 内で load／executeした evidence

現状を「runtime が一部完成している」と誤認しないこと。成立しているのは toolchain、CPU adapter、Windows native evidence の checkpoint である。

## 8. Corpus

ゲーム本体は repository に含めない。全 entry executable は PE32／i386 と確認済みである。

| Priority | Title | Windows path | Role |
|---:|---|---|---|
| 1 | サクラノ詩 | `V:\softs\サクラノ詩\BGI.exe` | Primary full-play target |
| 2 | Nursery Rhyme | `V:\softs\Lump of Sugar\Nursery Rhyme\NurseryRhyme.exe` | Same-family DirectDraw comparison |
| 3 | PURIPA | `V:\softs\PURIPA\PURIPA.EXE` | Deferred DirectDraw comparison |
| 4 | School Days HQ | `V:\softs\overflow\SCHOOLDAYS HQ\SCHOOLDAYS HQ.exe` | Deferred D3D9／video comparison |
| 5 | Shiny Days | `V:\softs\overflow\SHINYDAYS\SHINYDAYS.exe` | Deferred D3D9／video comparison |
| 6 | しゅがてん！ | `V:\softs\suger\しゅがてん！\syugaten.exe` | Deferred dynamic-resolution comparison |
| 7 | ふぃぎゅ@メイト | `V:\softs\ふぃぎゅ@メイト\figuat.exe` | Deferred GDI／MCI comparison |

全 size、SHA-256、static dependency は `docs/corpus-inventory.md` を正とする。サクラノ詩以外を active path に入れるのは、現在の blocker を小さく比較できる場合だけである。

## 9. Canonical Commands

PowerShell で実行する。

```powershell
Set-Location V:\src\vine
npm ci
npm run check
```

`npm run check` は documentation、LLVM PE／Wasm fixture、Windows 上の native callback oracle、pin 済み v86 prebuilt smoke、trace summarizer contract を実行する。

Environment と BGI fingerprint:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File tools/windows/check-environment.ps1 `
  -BgiExe "V:\softs\サクラノ詩\BGI.exe"
```

次の native trace。runner は既存 output を既定で拒否する。

```powershell
$trace = ".work\windows-traces\sakura-startup-{0}.jsonl" -f `
  (Get-Date -Format "yyyyMMdd-HHmmss")
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output $trace `
  --seconds 0

node tools/summarize-windows-trace.mjs `
  --input $trace `
  --output "$trace.summary.json"
```

利用者がタイトル画面まで操作したら `Ctrl+C` で収集を終了する。process を終了したい場合は `--kill-after` を加えるか、収集後にエージェントが終了してよい。

v86 source adapter を再現 build するときだけ、`make` と `sh` が `PATH` にある PowerShell で実行する。

```powershell
npm run prepare:engine
```

この command は固定 source を取得し、workspace-local Rust 1.97.0 を用意し、overlay を適用して build と一つの adapter contract check を行う。通常の変更ごとに再実行する必要はない。

Generated output は `.work/` に置く。raw artifact は commit せず、重要な結果だけ小さな manifest または Markdown に要約する。

## 10. Active Queue

Windows W0、最初の focused trace／summary、native ABI oracle、次の slice 選択は 2026-07-10 に完了した。長期 roadmap の全 task を日常の checklist にせず、現在は次を一つずつ進める。

1. source adapterを準備し、`abi-smoke.exe`をfixed T1 imageとして8 MiB identity memoryへmapするloader-liteを作る。
2. IAT 2 slotを runtime-owned `UD2` thunkへ解決し、所有 addressだけを host callに分類する。
3. cdecl host service、guest `add_one` callback、callback／entry return sentinelを single-thread bounded loopで通す。
4. instruction budget `3`でcallback result `43`、final EAX `0`、stack balance、最低1 budget yieldを一つのrepresentative contract testで検証する。
5. この slice の測定から guest memory案を更新する。8 MiB前後の低位 identity mappingを sparse 4 GiB案の決定として扱わない。
6. checkpoint後、最小 Win32 window／message／pixel fixtureへ広げる。
7. 次の subsystem-focused native trace前に caller module＋RVA、call ID、last errorを追加し、選んだvtableだけを観測する。
8. BGI.exe を loaderへ投入し、first unsupported import／bootstrap pointまで進める。

項目1～4は一つの W1 checkpoint であり、primary implementation taskを別 subsystemへ分岐させない。

## 11. Major Open Questions

優先順:

1. サクラノ詩の title resource bandで到達した factory以降のD3D9／DirectInput／DirectSound method、format、stateは何か。
2. v86 paging と bounded physical allocatorを使うか、CPU memory accessを独自 sparse memoryへ接続するか。
3. runtime-owned thunk／callback sentinelとWin32 exception deliveryをどう統合するか。
4. cooperative guest threads、wait／message、async host responseをどうsuspend／resumeするか。
5. Rust／TypeScript workspace、schema generation、Wasm boundaryをどう固定するか。
6. D3D9 COM vtable、format、state、draw pathの実到達subsetは何か。
7. DirectInput 8 device／format／Acquire pathとDirectSound ordinal／interfaceは何か。
8. WebGL2／WebGPU、AudioWorklet、OPFS／IndexedDBのどれをbaselineにするか。
9. interpreter性能がfull-play workloadに足りるか。足りない場合、どのhot pathだけを最適化するか。
10. 全編プレイ中にのみ到達するAPI、media、save、font、locale差をどう段階的に閉じるか。

これらを推測だけで確定しない。native evidence、synthetic fixture、測定のいずれかを取得してADRまたは該当文書を更新する。

## 12. Session Workflow

1. `AGENTS.md` を全文読む。
2. `git status --short`、直近 commit、現在 branchを確認する。
3. `docs/roadmap.md` の active queue と関連する詳細文書だけ読む。
4. user changeや別作業のdirty fileを保持し、勝手にrevertしない。
5. active queueから一つのcheckpointを選び、完了条件を一文で定義する。
6. 必要なnative traceまたはfixtureを先に取得する。
7. checkpointを通す最小実装を行う。
8. 代表 automated test 1 本と関連する既存checkだけを実行する。
9. behavior、decision、未解決事項、次の一手を文書へ反映する。
10. `git diff --check` と対象checkを通す。
11. cohesiveなcheckpointとしてcommitし、remoteへpushしてよい。
12. 結果、verification、残るblocker、次のcommandを利用者へ報告する。

実装詳細は合理的に判断して進める。mission、final target、配布ファイルへの意図的変更、architectureを大きく固定する判断に新しい選択肢が生じた場合だけ利用者へ確認する。

## 13. Git Rules

- commit／pushは許可済みである。
- 小さな途中状態を大量にcommitせず、再開可能なcheckpoint単位にする。
- commit前にdiffとuntracked fileを確認し、game file、raw trace、save、`.work/`、`node_modules/`を含めない。
- userのunrelated changeをrevert、overwrite、squashしない。
- generated evidenceを残す場合は、raw dataではなく再生成command、hash、重要な数値をcommitする。
- push失敗時にhistory rewriteやforce pushを勝手に行わない。

## 14. Documentation Map

| File | Purpose |
|---|---|
| `AGENTS.md` | 最新の運用方針、現在地、次の一手 |
| `docs/README.md` | 文書索引とproject vocabulary |
| `docs/requirements.md` | functional／non-functional requirements |
| `docs/corpus-inventory.md` | title fingerprint、dependency、corpus role |
| `docs/target-architecture.md` | component boundary、flow、ADR候補 |
| `docs/roadmap.md` | long-term phase、task、dependency、acceptance mapping |
| `docs/risks-and-open-questions.md` | unresolved decisions、risk、go／no-go |
| `docs/testing-and-acceptance.md` | test candidate、evidence、phase acceptance |
| `docs/x86-engine-scorecard.md` | engine comparison、v86 evidence、remaining gates |
| `docs/toolchain-spike.md` | LLVM PE／Wasm fixture baseline |
| `docs/windows-development.md` | Windows bring-up、trace、checkpoint commands |
| `docs/windows-native-evidence.md` | Windows W0、BGI startup trace、native ABI oracle、選択した W1 slice |
| `spikes/x86-engine/README.md` | v86 source adapter reproduction |

文書内の長い test／task listは将来coverageの地図であり、今すぐ全件実装する指示ではない。本書のactive queueと「広く浅く、代表test 1本」を優先する。
