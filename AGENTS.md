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
- guest memory のsemantic sourceはRust-owned sparse region／page tableとbounded zeroed frame allocatorに置く。v86 paging／physical backingへの反映方式とsnapshotは未決定。
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
- fixed T1 image の PE32 section map、IAT thunk、cdecl host call、1 段 guest callback、return sentinel
- fixed W2 image の3 DLL／13 IAT thunk、`GetModuleHandleA(NULL)`、checked `WNDCLASSEXA`、host-owned logical class／ATOM／HWND／HDC、window／message／paint／nested destroy／quit flow、callback／entry return sentinel
- fixed W2 resultからversion 1 browser present eventを1件生成し、Chrome Canvas 2Dへ1回presentしてpixel readbackするW3 first-pixel boundary
- fixed high-memory PE32 imageを2 MiB physical poolから`0xF1000000`へmapし、high EIP／stack、present／R／W、cross-page fault ordering、TLB invalidationを通すmemory spike。native execute protectionの欠落も構造化する
- production Rust sparse memoryでhigh imageの6 pagesとBGI planの380 pagesをbounded ownershipへ入れ、logical execute seal、cross-page preflight、generation-tagged invalidation requirementを通すCPU未接続のguest-memory ingress
- owned W7 planだけからfixed T1とBGIを`0x10000000`へmaterializeし、source copy、explicit zero-fill／readback、`HIGHLOW` relocation、original IAT verification、mapped-content digestを通すCPU未接続のPE32 mapping layer
- version 1 runtime export catalogと`[0x7F000000, 0x7F010000)`のowned `UD2` thunk arenaでsupported import prefixをIATへbindする。fixed T1の2 import／両owner sealと、BGIの`Direct3DCreate9` 1件／次のunsupported symbol stopを通すCPU未接続のimport-binding layer
- sealed fixed T1の6 production pagesをsemantic reflectionでprovisional v86 pagingへcopyし、actual TLB／fetch／generated-code invalidation後にgeneration `4`をacknowledgeする。harness-only 4 KiB stackからentry `0x10001000`を実行し、exact owned thunk ID `1`へ到達するv86 entry ingress
- ordered production thunk metadataでexact thunk ID `1`を分類し、fixed cdecl frame `[return,19,23]`をchecked readしてEAX `42`を返す。guest側のargument cleanup後にexact thunk ID `2`へ到達し、callback target `0x10001030`／value `42`を構造化するcdecl service ingress

未検証または未実装:

- general v86-connected guest memory／page ownership／execute gate、snapshot
- general stdcall／thiscall／fastcall、arbitrary nested callback、SEH と production ABI gateway
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
12. fixed `abi-smoke.exe` を 8 MiB identity memory へ map し、runtime-owned thunk、cdecl host service、guest callback、budget yield、return sentinel を通す W1 contract。
13. SDK 非依存で再現生成する PE32 Win32 window／message／pixel fixture と native self-oracle。
14. fixed `window-pixel.exe`をmapし、全13 importをowned thunkへ解決して最初のhost serviceを構造化するW2 loader ingress contract。
15. `GetModuleHandleA(NULL)`をlogical main moduleへ解決し、1引数stdcallでguestをresumeして`RegisterClassExA`を構造化するW2 module ingress contract。
16. `RegisterClassExA`のfixed `wndclassexa-v48-local-ascii` variantをchecked readし、copied semantic stateとATOM `0xC000`を登録して`CreateWindowExA`を構造化するW2 class ingress contract。
17. `CreateWindowExA`のfixed `createwindowexa-v12-local-ascii-wm-create` variantで12 argumentsとmodule-scoped classを検査し、harness-local HWND、48-byte `CREATESTRUCTA`、guest WndProcの`WM_CREATE` stdcall callback／return sentinelを通して`ShowWindow`を構造化するW2 window-create ingress contract。
18. `ShowWindow`のfixed `showwindow-sw-show-hidden-to-visible` variantでcreated／hiddenのlive HWNDと`SW_SHOW`を検査し、logical visibilityをtrueへ遷移してprevious visibilityのBOOL `0`をstdcall returnし、`InvalidateRect`を構造化するW2 visibility ingress contract。
19. `InvalidateRect`のfixed `invalidaterect-whole-client-no-erase` variantでlive HWND、NULL RECT、FALSE eraseを検査し、whole-client update regionとthread-owned generated-paint candidate 1件を作ってBOOL `1`をstdcall returnし、`GetMessageA`を構造化するW2 invalidation ingress contract。
20. `GetMessageA`のfixed `getmessagea-unfiltered-generated-paint-zero-metadata` variantでcandidate 1件を28-byte `MSG`へmaterialize／dequeueし、paint stateを`queued`から`in-flight`へ遷移してBOOL `1`をstdcall returnする。続くfixed `dispatchmessagea-generated-wm-paint-callback`で`CALL ESI`をIAT thunkへ帰属し、guest WndProcの`WM_PAINT` callbackへ入ってreentrant `BeginPaint`を構造化するW2 message dispatch ingress contract。
21. harness-only 2x2 logical clientへfixed `BeginPaint`／`SetPixel`／`EndPaint`を通し、typed HDC `0x48010001`のsession lifetimeとpixel `(1,1) = 0x000000FF`を検証する。`DestroyWindow`からnested `WM_DESTROY` callback／`PostQuitMessage(0)`へ入り、strict reverse callback／API unwind、2回目の`GetMessageA`による`WM_QUIT`、entry return sentinelまで完了するW2 paint／quit flow contract。
22. W2 harnessの最終logical pixelを`vine.browser-present` schema version `1`のsparse `rgba8` eventへ変換し、clean temporary profileのChrome `150.0.7871.115`でone-shot Canvas 2D sinkへ1回presentする。Canvas readbackは`(1,1) = [255,0,0,255]`、present countは`1`となるW3 browser first-pixel contract。
23. native trace／summary schema version `2`で全hooked callへcaller module＋RVA、global call ID、thread-local parent call ID、leave直後のLastError、durationを追加する。明示的`--vtable idirect3d9`だけが`IDirect3D9` root 17 slotsをhookし、15秒のclean-source BGI captureで11 methods／53 calls、3回の`CreateDevice`、3,529 balanced call pairs、max depth `3`、diagnostics `0`を観測するtrace attribution contract。
24. fixed high-memory PE32 fixtureをimage base `0xF1000000`で再現生成し、source-built v86の2 MiB physical poolへ11 virtual pagesをnon-identity mapする。high entry／stack、R page write PFEC `7`、unmapped read／write PFEC `4`／`6`、cross-page no-partial-write、protect／remap後のexplicit TLB invalidationを検証し、R／non-executable sectionが実行されるv86 NX gapをchecked sparse referenceのfetch fault `21`と比較するmemory contract。
25. Rust 1.97.0のroot workspaceに`vine-core`／`vine-pe-probe`を作り、owned immutable module bytes、map-time SHA-256、bounded PE32 header／section／directory／import／`HIGHLOW` relocation planをversion 1 JSONへ変換する。fixed `abi-smoke.exe`を代表contractにし、fingerprint済みBGI.exeを5 section、14 module／330 import、237 relocation block／18,308 wordへplanして`process-bootstrap`で構造化停止するW7 loader ingress contract。
26. `vine-core`にsparse 4 GiB guest address、owner／region／page table、bounded zeroed physical allocator、reserve／commit／protect／decommit／release、logical execute seal、cross-page preflight、generation-tagged invalidation protocolを実装する。fixed high-memory imageの6 pagesとBGI planの380 pagesをexact quotaで所有し、`image-materialization`／`image-bytes-relocations-import-binding-unimplemented`で停止するproduction guest-memory ingress contract。
27. `abi-smoke.exe`をpreferred `0x00400000`から`0x10000000`へmaterializeし、4 source range／2,560 bytes、5 zero range／17,920 bytes、5 `HIGHLOW`、2 original IAT slot、20,480 mapped bytes digestを検証する。同じversion 1 boundaryでBGIを380 pagesへcopy／zero-fillし、18,199 `HIGHLOW`と330 IAT slotを検証して`d3d9.dll!Direct3DCreate9`の`import-binding`／`iat-binding-unimplemented`で停止するW7 materialization contract。
28. 64 KiBのproduction runtime thunk rangeへ16-byte stride／1-based stable IDでowned `UD2` slotを割り当てる。controlled `abi-smoke.exe`の2 named importを全bindしてimage／runtime ownerをsealし、BGIではnative-reachable `d3d9.dll!Direct3DCreate9`だけをbindして、次の`DINPUT8.dll!DirectInput8Create`をmodule／symbol／IAT付きで構造化停止するW7 import-binding contract。
29. 同じlive `GuestMemory`から6 sealed pagesをversion 1 reflectionとしてv86へ渡し、page directory／3 tables／6 reflected pages／1 harness stackを2 MiB poolの11 pagesへ配置する。state ABI restore、`full_clear_tlb`、`jit_clear_cache_js`後だけgeneration `4`をacknowledgeし、instruction budget `3`でentryからthunk ID `1`へ到達するproduction-memory entry ingress contract。
30. 同じreflection sessionとordered thunk metadataを使い、thunk ID `1`のfixed two-argument cdecl serviceをdispatchする。EAX `42`、callee `RET`相当のESP `+4`でguestへresumeし、guest自身のargument cleanup後にthunk ID `2`／callback target `0x10001030`／value `42`へ到達するproduction cdecl service ingress contract。

2026-07-10 に Windows 11 25H2、Chrome 150、Frida 17.15.4 で `npm run check` と BGI fingerprint を再確認した。BGI startup trace は main window、D3D9、DirectInput 8、DirectSoundCreate8、GDI+ と title resource band まで到達し、native `abi-smoke.exe` は callback 後に exit code `0` となった。同じ fixture を source-built v86 adapter 上で実行する W1 contract は result `43`、final EAX `0`、ESP `0x00700000`、budget stop 5 回で完了した。続く native `window-pixel.exe` は `WM_CREATE -> WM_PAINT -> WM_DESTROY`、queued dispatch、1 pixel、clean quit を自己検証して exit code `0` となった。2026-07-11 に同じfixed imageを使うW2 loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flowを再実行し、3 DLL／13 importをowned thunkへ解決して13 fixed partial service returnと6 callback eventを完了した。harness-only bounds `[0,0,2,2]`の`PAINTSTRUCT`とtyped HDC `0x48010001`を作り、pixel `(1,1) = 0x000000FF`を保持した。Dispatch API ID `3`／`WM_PAINT` callback ID `4`の内側でDestroy API ID `5`／`WM_DESTROY` callback ID `6`をnested実行し、`PostQuitMessage(0)`、両callback／API return、`GetMessageA`の`MSG [NULL, WM_QUIT, 0, 0, 0, 0, 0]`／return `0`を経てentry sentinel `0x00500F00`へEAX `0`、ESP `0x00700000`で到達した。66 budget stop、83 execute stop、17 owned `UD2`、231命令である。同じfinal stateからversion 1 browser present eventを1件emitし、Chrome `150.0.7871.115`のCanvas 2D sinkで1回presentして`(1,1) = RGBA [255,0,0,255]`をreadbackした。詳細は `docs/windows-native-evidence.md` を参照する。logical client／HDC／surface／queue／frame layoutとsparse eventはcheckpoint-localであり、native geometry、production common surface、general Win32、renderer backendの決定ではない。W1／W2 flowはNode.js上の固定harness、W2 fixtureはnative Windows oracleである。W3でChromeが実行するのは生成済みeventの最小Canvas sinkだけであり、guest CPU、Win32 lifecycle、Runtime Workerのbrowser実行は未実施である。

同日、commit `e4548ec`のclean worktreeからBGI fingerprintを再検証してtrace v2を15秒取得した。`IDirect3D9` rootだけを選択し、factory 4 callsとroot 11 methods／53 callsをcaller RVA付きで分離した。slot `16`の`CreateDevice`は3回とも`BGI.exe + 0x5ACA5`、return `0`であり、内部root callsはparent call IDで各CreateDeviceへ帰属する。これは`P6D-COR-01`のroot-interface部分へのpartial evidenceであり、device／resource vtable、presentation parameters内容、format／state／draw／Present／Reset、DirectInput／DirectSound methodを確定しない。LastErrorは各leave直後の観測値であり、APIが値を定義しない場合の成功判定には使わない。

同日、3,072-byte／SHA-256 `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7`の`high-memory.exe`を作り、2 MiB physical poolの60 KiBをdirectory／table／mapped pageへ割り当てて44 KiBのhigh virtual mappingを実行した。v86 pagingはpresent／R／Wとcross-page fault orderingを満たし、protect／remapは`full_clear_tlb`後に反映された。v86はPE sectionのexecute bitをnativeに表現せず`.nxprobe`を実行したため、Vine region table＋bounded physical allocator＋adapter execute gateをprovisional candidateとし、checked sparse replacementはCPU未接続のreferenceに留めた。詳細は`docs/adr-003-guest-memory.md`を正とする。

同日、Rust W7 ingressを同じBGI.exeへ2回適用し、1,976,261-byte／SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be`のdeterministic local reportを得た。329 named importと`DSOUND.dll` ordinal 11、109 `ABSOLUTE`／18,199 `HIGHLOW`を保持し、元BGI.exeのsize／SHA-256は不変だった。stopは`process-bootstrap`／`peb-teb-process-parameters-unimplemented`であり、guest memory materialization、IAT binding、PEB／TEB／initial stack、entry executionはnot attemptedである。詳細は`docs/pe-loader-ingress.md`を正とする。

同日、W7 planをproduction Rust guest-memory ingressへ接続した。high-memory fixtureは`0xF1000000`の6 image pagesを所有し、unsealed／NX execute PFEC `21`、read-only write PFEC `7`、second-page unmapped write PFEC `6`とno-partial-write、protect、quota、decommit／reuse、release／teardownを1 contractで通した。BGI planはexact quota 380 pagesで380 readable／100 writable／208 executable pagesを所有し、version 1 reportは1,796 bytes／SHA-256 `355b514d657bc7639bfd5f2b9eb2043dc94166a7944710c865b30c86c437947c`となった。379 pagesではoutputを作らず失敗し、default W7 reportはbyte-for-byte不変だった。source copy、zero-fill verification、`HIGHLOW`適用、IAT binding、owner seal、v86 invalidation／executionはnot attemptedである。詳細は`docs/guest-memory-ingress.md`を正とする。

同日、production PE32 materializationを追加した。fixed `abi-smoke.exe`を`0x10000000`へrelocateし、mapped-content SHA-256 `1cfa1ba4ef560433b38e2598ba291ae6cbd0f8139408274c2192c30e4a185110`を得た。BGIはexact 380-page quotaで6 range／1,193,472 bytesをcopyし、5 range／363,008 bytesをzero-fill／readbackし、18,199 `HIGHLOW`と330 original IAT slotを検証した。mapped 1,556,480 bytesのSHA-256は`d9dc33d121ea2da6ba5e459a9da13b70bf8bfc1c668593a80c6b3689d841199b`、2回のversion 1 reportは2,448 bytes／SHA-256 `e6ce8d5599922c7ea11470475313907e6732c1c9e4a2ca0ecd4457cd0d502019`で一致した。ownerは未seal、generation 1 invalidationはpendingで、IAT write、DLL／export resolution、v86接続、bootstrap／executionはnot attemptedである。詳細は`docs/pe-materialization.md`を正とする。

同日、production import bindingを追加した。`abi-smoke.exe`はphysical 6 pagesでthunk ID `1`／`2`を`0x7F000000`／`0x7F000010`へ割り当て、IAT `0x10002034`／`0x10002038`をbindしてimage／runtime ownerをsealした。arena SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`、version 1 reportは4,421 bytes／SHA-256 `04d6f001e157c0e0fc3f1878516fb913a131e5cfe187162cf265b375e949472d`である。BGIは381-page quotaでIAT `0x100D14F4`をthunk ID `1`へbindし、329 importsを残して`DINPUT8.dll!DirectInput8Create`／IAT `0x100D1050`で停止した。BGI arena SHA-256は`4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059`、reportは4,258 bytes／SHA-256 `64217ed90cd86098396d27652ed41828ffeb6354f29f96bb7cc981daaab34fc0`である。BGI imageは未sealで、general DLL／export resolution、service behavior、v86接続、bootstrap／executionはnot attemptedである。詳細は`docs/pe-import-binding.md`を正とする。

同日、controlled production memoryをprovisional v86 pagingへ接続した。Rust offerは6 pages／content SHA-256 `3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e`をphysical frame番号なしで渡し、Node adapterは2 MiB poolの11 pages／44 KiBへpage directory、3 tables、reflection、harness-only stackを配置した。state ABI restore／`full_clear_tlb`／`jit_clear_cache_js`後にgeneration `4`をacknowledgeし、Rust normal accessでIAT／entry／thunkを再検証した。entryは2 execute stop／1 budget stopでthunk ID `1`／`0x7F000000`へ達し、ESP `0x006FFFF0`、return `0x1000100A`、arguments `[19,23]`となった。version 1 reportは4,831 bytes／SHA-256 `9b32ed50a4fa53d60a2743267230d43acf754bb6bc490851dd754955e70cbcb8`である。host service dispatch、callback、production stack／bootstrap、snapshot、BGI executionはnot attemptedである。詳細は`docs/v86-entry-ingress.md`を正とする。

続けてexact thunk ID `1`をordered Rust-owned metadataから分類し、fixed cdecl `fixture_add(19,23)`をdispatchした。version 1 CPU state ABIでEAX `42`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x1000100A`へresumeし、guestが2 argumentsをcleanupした後、4 execute stop／2 budget stopでthunk ID `2`／`0x7F000010`へ到達した。callback-service frameはreturn `0x10001019`、target `0x10001030`、value `42`であり、target pageのsealed／execute metadataも検査した。version 1 reportは5,501 bytes／SHA-256 `5ee8871673b667a097c2b25be4ca2f810555785ad4e4a563b9c43681c968d953`である。callback frame／return sentinel、callback execution、outer service return、general ABI gatewayはnot attemptedである。詳細は`docs/v86-cdecl-ingress.md`を正とする。

まだ存在しないもの:

- production TypeScript／Wasm browser workspaceとRust coreのWasm packaging
- general v86 memory／process integration、general DLL／export resolutionとDLL lifecycle／loader
- broad calling convention、nested callback、SEH を扱う production ABI gateway
- scheduler／Runtime Worker
- production／general Win32 API compatibility implementation
- browser application
- graphics、audio、storage backend
- BGI.exe をv86-connected Vine guest memoryでexecuteした evidence

現状を「production runtime が一部完成している」と誤認しないこと。成立しているのは toolchain、CPU adapter、Windows native evidence、trace v2 attribution／selected IDirect3D9 root evidence、fixed T1 の W1 headless ABI checkpoint、W2 native fixture oracle／runtime flow、W3 browser first-pixel boundary、fixed high-address memory measurement、W7 Rust load-plan／guest-memory／image-materialization／import-binding／controlled entry／fixed cdecl service ingressである。controlled production imageはv86上でfixed serviceを1件returnしてcallback-service thunkまで実行済みだがcallback未実行で、BGIは1 importだけbindした未seal／未実行imageである。

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

`npm run check` は documentation、LLVM PE／Wasm fixture、Windows 上の native callback oracleとwindow／message／pixel oracle、Rust PE32 loader ingress contract、pin 済み v86 prebuilt smoke、trace summarizer contract を実行する。PE32 system import libraryはtracked DEFから`llvm-dlltool -m i386 -k`で生成し、Windows SDK `.lib`には依存しない。

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

利用者がタイトル画面まで操作したら `Ctrl+C` で収集を終了する。process を終了したい場合は `--kill-after` を加えるか、収集後にエージェントが終了してよい。`--vtable idirect3d9`はこのroot interfaceを選んだcaptureだけに加え、通常のexport traceまたは別subsystem traceでは省略する。

v86 source adapter を再現 build するときだけ、`make` と `sh` が `PATH` にある PowerShell で実行する。

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:abi
npm run check:runtime:memory
npm run check:runtime:window-loader
npm run check:runtime:window-browser
npm run check:runtime:pe
npm run check:runtime:memory-core
npm run check:runtime:materialization
npm run check:runtime:imports
npm run check:runtime:entry
npm run check:runtime:cdecl
```

`prepare:engine` は固定 source を取得し、workspace-local Rust 1.97.0 を用意し、overlay を適用して build と一つの adapter contract check を行う。`check:runtime:abi`、`check:runtime:memory`、`check:runtime:window-loader`は先に`npm run check:toolchain`で生成したfixed fixtureも必要とし、それぞれW1 PE／ABI、high-address／protection、W2 runtime flow contractを実行する。`check:runtime:window-browser`は同じW2 harnessがemitしたeventを一時local serverからclean Chrome profileへ渡し、Canvas pixelをreadbackする。`check:runtime:entry`はproduction Rust reflectionとsource-built v86をsession接続してcontrolled entryからfirst owned thunkまで実行し、`check:runtime:cdecl`は同じboundaryでfixed cdecl return後にcallback-service thunkまで進める。`check:runtime:pe`、`check:runtime:memory-core`、`check:runtime:materialization`、`check:runtime:imports`はroot Rust workspaceのW7 plan／memory／mapped-byte／IAT-thunk contractを個別に実行し、すべてを含む`check:runtime:core`がcanonical checkに入る。source adapterを使う前6 checkpointは通常の`npm run check`には含めない。

Generated output は `.work/` に置く。raw artifact は commit せず、重要な結果だけ小さな manifest または Markdown に要約する。

## 10. Active Queue

Windows W0、最初の focused trace／summary、native ABI oracle、W1 fixed PE／ABI slice、W2 native window／message／pixel fixtureは 2026-07-10、W2 runtime flow、W3 browser first-pixel boundary、trace v2 attribution／selected IDirect3D9 root capture、high-address／protection memory measurement、W7 Rust PE32 load-plan／guest-memory／image-materialization／import-binding／controlled v86 entry／fixed cdecl service ingressは 2026-07-11 までに完了した。controlled `abi-smoke.exe`の6 production pagesはgeneration `4`のactual invalidation後にv86へ反映され、harness-only 4 KiB stackからentryを実行した。thunk ID `1`でfixed result `42`をreturnし、guest側cleanup後にthunk ID `2`／callback target `0x10001030`／value `42`へ到達した。BGIは`d3d9.dll!Direct3DCreate9`だけをbindし、次の`DINPUT8.dll!DirectInput8Create`で停止した未実行imageである。2 MiB v86 pool、physical placement、one-page stack、flat CPL3 state、`0x7F000000` arenaはcheckpoint上のversion 1選択であり、final process layout／bootstrap／snapshotの決定ではない。長期 roadmap の全 task を日常の checklist にせず、現在は次を一つずつ進める。

1. production-owned callback return sentinel／frame identityを割り当て、checked stack spaceへ`[return sentinel, 42]`を積み、callback target `0x10001030`へ入ってexact callback-return sentinelでresult `43`とともに停止する。
2. callback result `43`をouter thunk ID `2`のfixed cdecl returnとしてguest `0x10001019`へ返し、その後のentry continuationを構造化する。

項目1が次の primary implementation checkpointである。fixed cdecl return／callback-service ingressをgeneral page reflection、process bootstrap、host ABI gateway、callback、snapshot、`AC-P1`／`AC-P2`の完成と扱わない。次checkpointはproduction-owned callback sentinel／frame identityと1段callback entry／return stopに限定し、outer service returnは続くcheckpointへ残す。

## 11. Major Open Questions

優先順:

1. IDirect3D9 rootでは11 methodsへ到達したが、device／resource vtable、presentation parameters内容、format／state／draw／Present／Reset、DirectInput／DirectSound methodは何か。
2. controlled reflection／actual invalidationは成立したが、general mutation、bidirectional dirty-page ownership、snapshotを同じv86 paging boundaryへどう結び付けるか。
3. version 1 production import thunkのaddress／ID ownershipをcallback sentinel、Win32 exception delivery、generated API metadataとどう統合するか。
4. cooperative guest threads、wait／message、async host responseをどうsuspend／resumeするか。
5. root Rust workspaceに続くTypeScript package、schema generation、Wasm build／ownership boundaryをどう固定するか。
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
| `docs/adr-003-guest-memory.md` | high-address／protection測定、memory候補比較、provisional CPU integration boundary |
| `docs/guest-memory-ingress.md` | production Rust sparse memory、fixture／BGI layout、fault／invalidation contract |
| `docs/pe-loader-ingress.md` | Rust workspace、immutable PE32 plan、W7 BGI result／structured stop |
| `docs/pe-materialization.md` | source copy／zero-fill／`HIGHLOW`、fixture／BGI mapped-content evidence、IAT stop |
| `docs/pe-import-binding.md` | runtime thunk arena／ID、IAT prefix binding、owner seal、unsupported stop |
| `docs/v86-entry-ingress.md` | production page reflection／invalidation ack、bounded stack、entry-to-thunk evidence |
| `docs/v86-cdecl-ingress.md` | exact thunk metadata、fixed cdecl return、guest cleanup、callback-service ingress |
| `docs/roadmap.md` | long-term phase、task、dependency、acceptance mapping |
| `docs/risks-and-open-questions.md` | unresolved decisions、risk、go／no-go |
| `docs/testing-and-acceptance.md` | test candidate、evidence、phase acceptance |
| `docs/x86-engine-scorecard.md` | engine comparison、v86 evidence、remaining gates |
| `docs/toolchain-spike.md` | LLVM PE／Wasm fixture baseline |
| `docs/windows-development.md` | Windows bring-up、trace、checkpoint commands |
| `docs/windows-native-evidence.md` | Windows W0、BGI trace v1／v2、selected IDirect3D9 root、native ABI oracle、W1 runtime、W2 native fixture／runtime flow、W3 browser first pixel、W7 loader／memory／materialization／import-binding／entry／cdecl ingress |
| `spikes/x86-engine/README.md` | v86 source adapter reproduction |

文書内の長い test／task listは将来coverageの地図であり、今すぐ全件実装する指示ではない。本書のactive queueと「広く浅く、代表test 1本」を優先する。
