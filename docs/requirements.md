# 要件定義

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 対象: PE32 / i386 のサクラノ詩 full-play runtime
- 開発形態: 1 人、完全個人用
- 前提: production compatibility runtime は未完成。static corpus inventory、LLVM／PE32／Wasm toolchain、v86 bounded adapter／CPU state spike、W1／W2／W3、high-address memory、Rust PE32 load／memory／entry return／stack bootstrap、native TEB／PEB／process-parameters oracle、Rust-owned minimum command-line ingressは完了済みである。general process integration／parameters、DLL／export graph、general service dispatchは未実装である

## 2. 目的

Vine は、Windows 向け VN/ADV の元の `.exe` と同梱 `.dll` を原則として変更せず、ブラウザ内の互換ランタイムで再ホストする。

対象は Windows userspace の実行契約であり、ゲームエンジン、シナリオ VM、個別アセット形式ではない。Windows API と DirectX API の必要部分を、ブラウザの graphics、audio、storage、input API に直接接続する。

## 3. 成功状態

最終成功状態は、今回 fingerprint を記録したサクラノ詩 `BGI.exe` について、次の利用シナリオを Chrome だけで完了し、全編を native Windows 実行と同様に不自由なくプレイできることである。

1. 利用者がローカルのゲームディレクトリまたはパッケージを選択する。
2. Vine が対象を検査し、実行形式、既知の未対応機能、静的には判定できない依存、必要容量を表示する。preflight 通過を互換性保証とは表示しない。
3. import した `.exe` / `.dll` の immutable snapshot から、タイトルの entry point を起動する。
4. ウィンドウ描画、文字表示、キーボード／マウス入力、BGM／効果音を利用できる。
5. セーブデータと必要な registry 状態をブラウザ storage に永続化する。
6. ブラウザ再起動後に同じタイトルを再開し、保存したデータをロードできる。
7. 序盤だけでなく全シナリオ、選択肢、演出、ending まで進行でき、途中で未対応機能により停止しない。
8. 失敗時に、診断 bundle と未対応 API 一覧を取得できる。

「Windows 7 / Windows 10 以降で動く 32 bit アプリケーション」は候補タイトルの目安であり、Windows のバージョン互換を包括的に保証する表現ではない。Vine の保証範囲は、実装済み API inventory、ABI／CPU feature、検証済み title profile で示す。

## 4. 利用者とユースケース

### 4.1 主利用者

- 手元の VN/ADV を browser 上で実行し、同じ repository を開発・保守する単一利用者

### 4.2 主要ユースケース

| ID | ユースケース | Active target |
|---|---|---|
| UC-01 | ローカルのゲーム一式を import し、互換性を事前検査する | 必須 |
| UC-02 | PE32 entry point からタイトルを起動する | 必須 |
| UC-03 | GDI と target-required D3D9 subset の画面を表示する | 必須 |
| UC-04 | DirectSound の音声を再生、停止、loop する | 必須 |
| UC-05 | keyboard / mouse で操作する | 必須 |
| UC-06 | ファイルと registry を保存し、次回起動で復元する | 必須 |
| UC-07 | 対応外の機能を安全に停止し、診断する | 必須 |
| UC-08 | title profile を export / import する | Should |
| UC-09 | PE32+ / x86_64 タイトルを実行する | 将来 |
| UC-10 | サクラノ詩が実際に呼ぶ bounded D3D9／DirectInput 8 subset を実行する | 必須 |
| UC-11 | DirectDraw title を比較実行する | Should |
| UC-12 | D3D11 / DXGI / XAudio2 title を実行する | 将来 |

## 5. スコープ

### 5.1 初期ターゲット

| 領域 | 初期範囲 |
|---|---|
| executable | PE32、`IMAGE_FILE_MACHINE_I386` |
| execution | x86 32 bit user mode、必要な x87 / MMX / SSE / SSE2 の段階的対応 |
| ABI | cdecl、stdcall、thiscall、必要に応じ fastcall、guest callback |
| Windows API | サクラノ詩が必要とする `ntdll` 相当の内部境界、Kernel32、User32、GDI32、GDI+、COM、および限定した補助 DLL |
| graphics | GDI、GDI+ image subset、bounded D3D9。DirectDraw は比較 branch。backend は成立性検証で WebGL2 / WebGPU を中心に決定 |
| audio | DirectSound から Web Audio API への接続。WinMM／MCI は primary scenario で到達する command subset |
| storage | in-memory VFS と永続 backend。OPFS / IndexedDB 等の採用は ADR で決定 |
| input | User32 keyboard／mouse／focus と、サクラノ詩が呼ぶ DirectInput 8 subset。IME と gamepad は優先度を別途決定 |
| locale | 日本語 VN に必要な Unicode / Windows-932、時刻、基本 locale、font mapping |
| browser | current stable Chrome を最初の target とし、evidence に実 version を記録。最低 version は成立性検証後に固定 |

### 5.2 Active corpus

- Primary T3 target: サクラノ詩 `BGI.exe`
- Non-blocking comparison: Nursery Rhyme `NurseryRhyme.exe`
- Deferred corpus: School Days HQ、Shiny Days、PURIPA、しゅがてん！、ふぃぎゅ@メイト
- 詳細な fingerprint と dependency は [corpus-inventory.md](corpus-inventory.md) を正とする

### 5.3 将来ターゲット

- PE32+ / x86_64 と Win64 ABI
- D3D9 support band の一般化、D3D11、DXGI
- XAudio2
- Unity 系 runtime が要求する userspace API と実行機能
- 対応 browser、OS、input device の拡大

将来ターゲットは、サクラノ詩 full-play の受け入れ条件には含めない。初期設計ではアドレス幅、calling convention、renderer、audio backend を交換できる境界だけを確保する。

### 5.4 明示的な非対象

- 特定ゲームエンジンの移植
- タイトルごとのシナリオ VM 再実装
- アセット形式ごとの decoder や変換 pipeline
- Windows driver、kernel mode、service、完全な Windows OS の再現
- OS 全体の仮想化

## 6. 制約と前提

| ID | 制約／前提 |
|---|---|
| CON-01 | browser は native x86 を直接実行できないため、guest 命令は interpreter 等の隔離された実行系で扱う |
| CON-02 | Win32 の同期 API と非同期 Web API の差を scheduler と host service 境界で吸収する |
| CON-03 | DOM、audio、storage には thread、user activation、permission、quota の制約がある |
| CON-04 | guest の 32 bit 仮想アドレス空間を host Wasm memory へ単純に 1:1 対応できるとは仮定しない |
| CON-07 | 初期性能の成立性が証明されるまで、DBT/JIT や SharedArrayBuffer を必須構成と断定しない |

## 7. 機能要件

優先度は Must、Should、Could、Deferred で示す。Must は該当マイルストーンの受け入れに必須である。

### 7.1 Import と launch

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-IMP-01 | 利用者が明示的に選択した directory、複数 file、または定義済み package から title data を読み込める | Must |
| REQ-IMP-02 | import 前に file 数、合計容量、entry point 候補、PE machine type を検査し、resource limit 超過を拒否できる | Must |
| REQ-IMP-03 | import した元 file を不変の source layer として扱い、save／設定は別の writable overlay に保存する | Must |
| REQ-IMP-04 | PE32+、driver、managed-only、破損 PE、その他の未対応 executable form を理由付きで拒否または追加診断できる | Must |
| REQ-IMP-05 | title の fingerprint とローカル profile を作成し、同じ title の保存領域を安定して再利用できる | Must |
| REQ-IMP-07 | import 時に `.exe`／`.dll` の stable byte snapshot と hash を取得し、module load はその immutable object だけから行う。外部 file の差替え、overlay shadowing、persistent self-update を既定で拒否、診断する | Must |

### 7.2 PE loader と guest memory

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-PE-01 | DOS / COFF / optional header と section table を境界検査付きで parse できる | Must |
| REQ-PE-02 | section の RVA、alignment、zero-fill、protection を guest address space に map できる | Must |
| REQ-PE-03 | base relocation、import、export、IAT、entry point を処理できる | Must |
| REQ-PE-04 | TLS data、TLS callback、resource、delay import の要否を corpus で評価し、必要分を実装する | Must |
| REQ-PE-05 | application 同梱 native DLL を同じ loader で依存順に load / unload できる | Must |
| REQ-PE-06 | malformed RVA、integer overflow、section overlap、循環 import で host memory を破壊せず、構造化 error を返す | Must |
| REQ-PE-07 | API-set／system module alias、manifest／activation context、bound import／load-config の使用を inventory 化し、corpus に必要な解決範囲を実装する | Must |
| REQ-MEM-01 | guest virtual allocation、commit、free、protection、stack、heap の必要 semantics を提供する | Must |
| REQ-MEM-02 | guest pointer を検査してから host 側で参照し、範囲外、overflow、use-after-free 相当を隔離する | Must |
| REQ-MEM-03 | executable、writable 等の guest protection を論理的に追跡し、self-modifying code を検出できる | Should |

現在のproduction guest-memory ingressは、sparse 4 GiB range、owner-scoped reserve／commit／protect／decommit／release、bounded physical quota、cross-page preflight、logical execute seal、initial full reflectionとincremental `MemoryDelta`を実装する。通常RW／NX data writeはdirty pageだけをlive v86へ適用でき、write-to-codeはinstruction-fetch／generated-code effectsを要求する。controlled sealed pagesはactual acknowledgement後にv86 entryからfirst thunkまで実行し、BGIの388-page stateはfirst four API returnsまで進んだ。generation／owner付きone-page CPU reconciliationは成立したが、stackはfixed 1 pageであり、heap／guard／snapshot、automatic v86 dirty ownership、general process integrationは未実装なので、`REQ-MEM-01`～`REQ-MEM-03`はいずれもpartialである。詳細は[guest-memory-ingress.md](guest-memory-ingress.md)、[memory-delta-and-transactions.md](memory-delta-and-transactions.md)、[v86-entry-ingress.md](v86-entry-ingress.md)、[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

続くPE32 materializationはowned planだけからheader／section copy、explicit zero-fill／readback、nonpreferred `HIGHLOW`、original IAT verificationを行う。version 1 import binderは全importをread-only planし、未解決があれば全identityを返してmemoryを変更せず、complete planだけをtransaction内でruntime-owned `UD2` thunkへ結ぶ。optional policyはcatalog-missing identitiesへreturnなしのunsupported trapsを割り当て、controlled fixtureとBGIの全IAT／両owner sealを検証した。BGIはfourth-call modeで5 declared／325 unsupported dispositionsを保持し、ID `104`／`103`／`149`／`13`をreturnしてruntime-reachable ID `157`まで実行した。general DLL／export／forwarder resolutionとAPI dispatchは未実装であり、`REQ-PE-02`／`REQ-PE-03`／`REQ-PE-06`はいずれもpartialである。詳細は[pe-materialization.md](pe-materialization.md)、[pe-import-binding.md](pe-import-binding.md)、[pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md)、[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

### 7.3 Process bootstrap、x86 execution、ABI

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-BOOT-01 | PE entry point 実行前に、adopted flat segment、FS base、initial register／stack／return sentinel を一貫して初期化する | Must |
| REQ-BOOT-02 | corpus／compiler が必要とする TEB、PEB、loader list、process parameters、command line、environment、current directory、TLS slot の最小 support band を提供する | Must |
| REQ-BOOT-03 | supported compiler／CRT ごとの startup fixture を entry point から user code、process exit まで実行し、layout 差を compatibility profile に記録する | Must |
| REQ-CPU-01 | 対象 corpus が使用する x86 integer、control flow、flags、stack、string 命令を正しく実行する | Must |
| REQ-CPU-02 | x87、MMX、SSE、SSE2 を inventory と fixture に基づき段階実装する | Must |
| REQ-CPU-03 | invalid opcode、divide error、memory fault、breakpoint を guest exception として扱い、host crash を起こさない | Must |
| REQ-CPU-04 | 命令 budget、wall-clock budget、cancel を持ち、無限 loop や過負荷を停止できる | Must |
| REQ-CPU-05 | thread ごとの register、FPU、TLS、last-error、stack 状態を分離する | Must |
| REQ-CPU-06 | 実行 trace を address、module、symbol、API call 単位で opt-in 取得できる | Should |
| REQ-CPU-07 | privileged、I/O、kernel／direct-system-call 命令を host 操作へ接続せず、guest fault または明示的な unsupported として処理する | Must |
| REQ-ABI-01 | cdecl、stdcall、thiscall と corpus が必要とする fastcall の引数、戻り値、stack cleanup を扱う | Must |
| REQ-ABI-02 | guest から host API への call と、host API から guest callback への再入を扱う | Must |
| REQ-ABI-03 | structure packing、alignment、handle、pointer、ANSI / Unicode string を検証可能な schema で marshal する | Must |
| REQ-ABI-04 | SEH / unwind / vectored exception の必要範囲を corpus で決定し、未対応経路を明示停止する | Must |
| REQ-ABI-05 | varargs は API ごとに format／argument contract と上限を定義して検証し、schema のない汎用 variadic bridge を暗黙に許可しない | Must |

controlled bootstrapはproduction-owned image／control／stackにtyped one-page TEB／PEB／process-parameters／command-lineを加え、CPU state ABIでFS baseをrestoreしてnative-oracled fields、UTF-16 bytes、entry returnを通した。general stack reserve／growth／guard、process lifecycle／parameters、loader list、TLS、LastError、bidirectional dirty-page ownershipは未実装なので、`REQ-BOOT-01`／`REQ-BOOT-02`等はpartialである。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。

### 7.4 Windows userspace compatibility

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-API-01 | synthetic DLL の export 名／ordinal を解決し、API ごとに implemented / partial / stub / unsupported を記録する | Must |
| REQ-API-02 | `GetLastError`、handle lifetime、object type、access check、error code の共通規則を提供する | Must |
| REQ-API-03 | process、thread、critical section、interlocked、TLS slot、event、mutex、semaphore、wait、timer の corpus 必要範囲を提供する | Must |
| REQ-API-04 | `LoadLibrary` / `GetProcAddress` 相当の動的解決と guest DLL lifecycle を提供する | Must |
| REQ-API-05 | time、performance counter、sleep の一貫した仮想 clock を提供する | Must |
| REQ-API-06 | unsupported API を silent success にせず、policy に従い fail、diagnostic、または明示的な限定 stub とする | Must |
| REQ-API-07 | COM initialization、class factory、BSTR／VARIANT 等の automation/runtime は代表 corpus の要求に基づいて範囲を決定する | Should |
| REQ-COM-01 | DirectSound／D3D9／DirectInput 8 と比較用 DirectDraw に必要な x86 COM method ABI、IUnknown、QueryInterface identity、reference count、HRESULT、vtable thunk を提供する | Must |
| REQ-COM-02 | CoInitialize、apartment、class factory、BSTR／VARIANT 等は `REQ-COM-01` と分離し、corpus-driven な support band とする | Should |

### 7.5 Window、message、input、locale

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-UI-01 | window class、logical HWND、create / destroy、focus、size、visibility の必要状態を管理する | Must |
| REQ-UI-02 | message queue、`GetMessage` / `PeekMessage` / dispatch、timer、paint invalidation の必要 semantics を提供する | Must |
| REQ-UI-03 | keyboard と mouse event を Windows virtual key、scan code、button、coordinate、modifier へ変換する | Must |
| REQ-UI-04 | focus loss、browser shortcut、pointer capture、resize、device pixel ratio の変化を安全に扱う | Must |
| REQ-UI-05 | clipboard、IME、drag and drop は browser permission と lifecycle を含めて個別に設計する | Should |
| REQ-DI-01 | `DirectInput8Create`、interface identity、keyboard／mouse device create、enumeration の target-required subset を提供する | Must |
| REQ-DI-02 | data format、cooperative level、Acquire／Unacquire、GetDeviceState／GetDeviceData のうち runtime trace で到達する semantics を提供する | Must |
| REQ-DI-03 | focus loss、reacquire、buffer overflow、unsupported device を deterministic に扱い、DOM input state と対応付ける | Must |
| REQ-I18N-01 | Unicode と Windows-932 の変換、NUL termination、invalid sequence の方針を一貫して実装する | Must |
| REQ-I18N-02 | title profile ごとに locale、timezone、font fallback を設定できる | Must |
| REQ-I18N-03 | Windows font metrics との差を測定し、fixture の advance／baseline／line-break threshold と critical scene の可読性を超える文字切れ、重なり、改行差を partial／unsupported と判定する | Must |

### 7.6 File system と registry

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-FS-01 | drive、current directory、separator、`.` / `..`、case-insensitive lookup を含む Windows path model を提供する | Must |
| REQ-FS-02 | read-only source と writable overlay を統合して guest に見せ、source file を変更しない | Must |
| REQ-FS-03 | create、read、write、seek、truncate、enumeration、metadata の corpus 必要範囲を提供する | Must |
| REQ-FS-04 | sharing、locking、delete-on-close、memory-mapped file の必要範囲を測定し、実装状態を明示する | Must |
| REQ-FS-05 | save data を crash-consistent に永続化し、write 成功、flush／close、browser transaction commit の durability point を定義して export、backup、delete できる | Must |
| REQ-REG-01 | HKCU を中心とした virtual registry の key、value、type、enumeration を提供する | Must |
| REQ-REG-02 | machine-wide、COM registration 等の hive は必要な read-only default または title-local overlay として隔離する | Should |
| REQ-REG-03 | registry 状態を title 単位で versioning して永続化、export、reset できる | Must |

### 7.7 Graphics

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-GDI-01 | DC、bitmap、brush、pen、font、palette、region と object lifetime の必要範囲を提供する | Must |
| REQ-GDI-02 | BitBlt / StretchBlt、text、basic shape、clipping、ROP の corpus 必要範囲を描画する | Must |
| REQ-GDI-03 | DIB、bottom-up image、pixel format、palette、alpha／color key の変換を検証可能に実装する | Must |
| REQ-GDIP-01 | サクラノ詩が import する GDI+ startup／shutdown、bitmap load／save、DIB conversion、LockBits／UnlockBits の runtime-reachable subset を提供する | Must |
| REQ-D3D9-01 | `Direct3DCreate9`、IDirect3D9／IDirect3DDevice9 identity、capability query、CreateDevice、presentation parameters の観測 subset を提供する | Must |
| REQ-D3D9-02 | primary scenario が使う surface／texture／buffer、format、pitch、Lock／Unlock、upload／readback semantics を提供する | Must |
| REQ-D3D9-03 | primary scenario が使う fixed-function state、transform、vertex format、blend／sampler state、draw path を browser backend に写像する | Must |
| REQ-D3D9-04 | Reset、cooperative state、device/context loss と未対応 vtable method を trace し、silent success せず復帰または停止する | Must |
| REQ-DD-01 | DirectDraw object、primary／offscreen surface、lock / unlock、blit、flip の比較対象に必要な範囲を提供する | Should |
| REQ-DD-02 | cooperative level、display mode、windowed / fullscreen 相当を browser canvas に写像する | Should |
| REQ-DD-03 | surface loss、restore、resize、graphics context loss から復帰できる | Should |
| REQ-DD-04 | comparison corpus が呼ぶ DirectDraw vtable method を分類し、実装または具体的に unsupported と診断する | Should |
| REQ-GFX-01 | renderer を GDI／GDI+／DirectDraw／D3D9 frontend から分離し、pixel golden と frame timing を計測できる | Must |

### 7.8 Audio

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-DS-01 | DirectSound device と primary / secondary buffer の作成、再生、停止、loop を扱う | Must |
| REQ-DS-02 | PCM format、sample rate、volume、pan、frequency、buffer cursor の必要 semantics を提供する | Must |
| REQ-DS-03 | lock / unlock、notification、streaming buffer を corpus 必要範囲で提供する | Must |
| REQ-DS-04 | browser の user activation 前は停止状態を明示し、操作後に破綻なく再開する | Must |
| REQ-DS-05 | SetCooperativeLevel の HWND／flag、focus／deactivation、primary format permission を logical window contract と接続する | Must |
| REQ-DS-06 | buffer lost／Restore と invalid／destroyed HWND を adopted DirectSound semantics で処理する | Must |
| REQ-AUD-01 | guest producer と AudioWorklet 等の consumer 間で underrun、overrun、A/V drift を計測する | Must |
| REQ-AUD-02 | unsupported codec を暗黙に変換せず、必要 codec を診断する | Must |
| REQ-AUD-03 | `PlaySoundA` と `mciSendCommandA` の primary scenario 到達性を trace し、到達する command／notification subset を提供する | Must |

### 7.9 Browser shell と diagnostics

| ID | 要件 | 優先度 |
|---|---|---|
| REQ-SHELL-01 | title 選択、起動、pause、stop、fullscreen、audio activation、save 管理を提供する | Must |
| REQ-SHELL-02 | unsupported browser feature、permission denial、quota shortage を起動前または発生時に説明する | Must |
| REQ-SHELL-03 | runtime update と title storage schema migration でデータ消失を防ぐ | Must |
| REQ-DIAG-01 | module、API、exception、thread、frame、audio、storage の構造化 event を記録できる | Must |
| REQ-DIAG-03 | 最初の未対応 API とその call site を特定できる | Must |
| REQ-DIAG-04 | API inventory と title compatibility result を機械可読形式で出力する | Must |

## 8. 非機能要件

数値が `provisional` の項目は、Phase 0 の基準端末と Phase 1 の vertical slice を使って再設定する。

| ID | 分類 | 要件／指標 |
|---|---|---|
| NFR-COR-01 | 正確性 | CPU、ABI、loader は fixture と reference oracle の差分テストで検証し、既知差分を version 管理する |
| NFR-COR-02 | 決定性 | test clock、seed、input trace を固定した試験は同じ最終 state hash を返す |
| NFR-ROB-01 | 制御可能性 | memory、file、thread、surface、audio、instruction に上限を設け、超過時に実行制御を runtime へ戻す |
| NFR-ROB-02 | parser 堅牢性 | malformed corpus と fuzzing で runtime crash、panic、out-of-bounds を許容しない |
| NFR-PERF-01 | 起動 | Phase 1 で CPU／memory／host bridge の provisional budget を決め、Phase 4 と Phase 7 で cached title の起動操作から最初の guest frame までの目標値を確定する |
| NFR-PERF-02 | 描画 | 代表 scene で target cadence を維持し、frame miss と CPU time の回帰 budget を CI benchmark に持つ |
| NFR-PERF-03 | 音声 | warm-up 後 30 分の検証で underrun counter 0、実機確認で audible defect 0、A/V drift の provisional 上限 50 ms |
| NFR-PERF-04 | 応答性 | stop 操作または quota 超過後、provisional 100 ms 以内に実行制御を host へ戻す |
| NFR-REL-01 | 永続性 | commit 済み save は browser process の異常終了を模擬した再起動後も旧版か新版の整合した状態で読める |
| NFR-REL-02 | 復旧性 | graphics context loss、audio suspend、focus loss、storage error を分類し、再開または安全停止する |
| NFR-COMP-01 | 互換性 | runtime revision ごとに API coverage、CPU coverage、title scenario の結果と既知制約を記録する |
| NFR-PORT-01 | browser | browser support を Tier 1 / Tier 2 / unsupported に分け、各 Tier の自動試験範囲を定義する |
| NFR-MNT-01 | 保守性 | public subsystem boundary、error taxonomy、trace schema、API metadata を version 管理する |
| NFR-MNT-02 | 再現性 | fresh clone と pin 済み toolchain から、文書化した単一 entry command で build と test を再現できる |
| NFR-SUP-01 | 診断性 | 互換性 failure は最初の unsupported／mismatch、guest call stack、runtime version と関連付けられる |
| NFR-ACC-01 | host UI | browser shell の操作、focus、status、error は keyboard と支援技術から利用できる |

## 9. Milestone

### 9.1 Execution feasibility slice (Phase 1)

固定 T1 image または loader-lite を使い、guest memory、x86 execution、API bridge、callback、Worker scheduler から test 用 pixel command、audio pulse、persistent byte service までを一度ずつ通過する。本番 PE／Win32／graphics／audio compatibility の完成とは扱わない。

### 9.2 Synthetic compatibility prototype (Phase 6)

合成 PE32 が、本番 PE loader、Win32 message、GDI／DirectDraw、DirectSound、persistent file を通過する。compatibility product の保証はまだ行わない。

### 9.3 Personal full-play target

synthetic fixture suite とサクラノ詩について、起動から title、全シナリオ、User32／DirectInput input、GDI／GDI+／D3D9 表示、DirectSound、save／load、ending まで通る。途中の短い scenario 合格は進捗 checkpoint であり、この milestone の完了ではない。

### 9.4 Optional multi-title expansion

personal target 完了後、必要になった場合だけ比較 corpus へ互換範囲を拡張する。3 title／2 runtime 系統という旧 MVP gate は active scope から外し、拡張開始時に改めて本数と scenario を決める。

### 9.5 Deferred beta

第三者向け beta は現在計画しない。開始する場合は support matrix、migration、diagnostic、性能 baseline、配布運用を別 milestone として再計画する。

## 10. 互換性の表現

Vine は「Win32 対応」を単一の真偽値で表さない。runtime revision ごとに次を記録する。

- PE feature: required / implemented / partial / unsupported
- CPU instruction group と例外挙動
- DLL / API / variant ごとの実装 status と contract test 数
- graphics / audio capability と精度
- browser Tier と必要 feature／header
- title ごとの `boot`、`menu`、`render`、`input`、`audio`、`save`、`load`、`long-run` 結果

互換性 profile は runtime version、title fingerprint、browser、test scenario、結果、既知差分を含む。未試験を `compatible` と表記してはならない。

## 11. 要件からフェーズへの対応

| 要件群 | 主フェーズ |
|---|---|
| CON、NFR の基準、corpus、governance | Phase 0 |
| REQ-MEM、REQ-CPU、REQ-ABI の成立性 | Phase 1 |
| REQ-PE、REQ-BOOT、REQ-ABI、REQ-API の headless 基盤 | Phase 2 |
| REQ-API、REQ-COM core、REQ-UI の message 基盤、REQ-I18N | Phase 3 |
| REQ-FS、REQ-REG | Phase 3 |
| REQ-GDI、REQ-GFX | Phase 4 |
| REQ-DD | Phase 5 comparison branch |
| REQ-DS、REQ-AUD | Phase 6 |
| REQ-D3D9、REQ-DI | Phase 6D |
| REQ-IMP、REQ-SHELL、title integration | Phase 7 |
| REQ-DIAG | Phase 1 から Phase 8 まで横断 |
| 全 NFR、release hardening | Phase 8 |
| PE32+、Win64、D3D11／DXGI、XAudio2 | Future track |

詳細な依存関係、タスク、出口条件は [roadmap.md](roadmap.md) を参照する。
