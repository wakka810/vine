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
- guest memory のbyte／metadata authorityはRust-owned sparse `GuestMemory`、v86 pagingはbounded execution mirrorとする。初回reflection、host `MemoryDelta`、CPU writebackはbinary control＋contiguous page payloadを使う。bounded slice後に全writable PTE Dirty bitをscanし、owner／generation付きmulti-page batchをRustへatomic reconcileする。production physical backing、snapshotは未決定。
- renderer は共通 surface model の背後で WebGL2／WebGPU を比較する。方式は未決定。
- audio は DirectSound／WinMM frontend と Web Audio backend を分離する。mixer、AudioWorklet、clock ownership は未決定。

## 6. Engine Decision

v86 を現在の working baseline とする。version 1 Chrome benchmarkは全thresholdを通り継続`go`である。具体的な blocker が出ない限り、他 engine の広い比較や自作 interpreter に時間を使わない。ただし production engine として固定せず、adapter contract を維持し、必要なら交換できる状態にする。

### Pinned evidence

- npm package: `v86@0.5.424`
- source revision: `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`
- source build Rust: `1.97.0`
- adapter artifact size: `1,882,836` bytes
- adapter SHA-256: `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26`
- Linux CI adapter artifact size: `2,141,050` bytes
- Linux CI adapter SHA-256: `ac8349cbaccd31d6f5c68a84012fc7bc3a73eda2c06e519331396b43d8c42540`
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
- version 1 runtime export catalogと`[0x7F000000, 0x7F010000)`のowned `UD2` thunk arenaで全importをread-only planし、complete planだけをtransaction内でIATへbindする。fixed T1の2 import／両owner sealと、BGI declared-only planの329 unresolved identity／memory不変／catalog追加後retryを通すCPU未接続のatomic import-binding layer
- optional explicit trap policyでcatalog-missing importにもidentity付きowned `UD2` slotを割り当てる。controlled fixtureの1 unsupported trapと、BGIの1 declared／329 unsupported importsを全bindしてimage／runtime ownerをsealするCPU未接続のtrap-binding layer
- opt-in 400-page／8 MiB-line bounded reflectionでBGIの388 production pagesを2 MiB v86 poolへ渡す。VGA MMIO holeをphysical allocationから除外し、generation `24` acknowledgement後にentry `0x100A383E`から19命令でexact thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`へ到達してreturn前に止めるprimary-title entry ingress
- exact thunk ID `104`を2番目のdeclared endpointとし、1引数stdcall frame／IAT callをchecked処理する。v86-mutated stack owner `4`の1 pageをgeneration `24`へreconcileし、controlled `FILETIME`をgeneration `25`の1-page `MemoryDelta`で返して4命令後のexact thunk ID `103`／`KERNEL32.dll!GetCurrentProcessId`へ進むprimary-title first Win32 call return
- exact thunk ID `103`を3番目のdeclared endpointとし、generation `25`のlive zero-argument stdcall frame／IAT callをRustでchecked処理する。owner `checkpoint-process-identity`のcontrolled PID `4096`をEAXへ返し、3命令後のexact thunk ID `149`／`KERNEL32.dll!GetCurrentThreadId`へ進むprimary-title second Win32 call return
- exact thunk ID `149`を4番目のdeclared endpointとし、generation `25`のlive zero-argument stdcall frame／IAT callをRustでchecked処理する。owner `checkpoint-thread-identity`のcontrolled TID `8192`をEAXへ返し、3命令後のexact thunk ID `13`／`KERNEL32.dll!GetTickCount`へ進むprimary-title third Win32 call return
- exact thunk ID `13`を5番目のdeclared endpointとし、generation `25`のlive zero-argument stdcall frame／IAT callをRustでchecked処理する。owner `checkpoint-monotonic-clock`のcontrolled tick count `3,600,000`をEAXへ返し、5命令後のexact thunk ID `157`／`KERNEL32.dll!QueryPerformanceCounter`へ進むprimary-title fourth Win32 call return
- exact thunk ID `157`を6番目のdeclared endpointとし、generation `25`のlive one-argument stdcall frame／IAT callをRustでchecked処理する。owner `checkpoint-monotonic-clock`のcontrolled counter `12,345,678,901`をstack上の64-bit outputへgeneration `26` deltaで書き、BOOL `1`をEAXへ返して44命令後のexact thunk ID `83`／`KERNEL32.dll!GetStartupInfoW`へ進むprimary-title fifth Win32 call return
- exact thunk ID `83`を7番目のdeclared endpointとし、generation `26`のlive one-argument stdcall frame／IAT callをRustでchecked処理する。focused native traceで68-byte x86 `STARTUPINFOW`のfull overwriteを測定し、owner `checkpoint-process-startup`のcontrolled profileをgeneration `27` deltaで書く。VOID policyでEAXを保持して9命令後のexact thunk ID `84`／`KERNEL32.dll!HeapSetInformation`へ進むprimary-title sixth Win32 call return
- exact thunk ID `84`を8番目のdeclared endpointとし、generation `27`のlive four-argument stdcall frame／IAT callをRustでchecked処理する。focused native traceで`[NULL,1,NULL,0]`、BOOL `1`、LastError unchangedを測定し、owner `checkpoint-process-heap-policy`のtermination-on-corruption variantだけを受け付ける。generation／dirtyを変えずBOOL `1`を返して21命令後のexact thunk ID `102`／`KERNEL32.dll!HeapCreate`へ進むprimary-title seventh Win32 call return
- exact thunk ID `102`を9番目のdeclared endpointとし、generation `27`のlive three-argument stdcall frame／IAT callをRustでchecked処理する。focused native heap-lifecycle traceで`[0,0x1000,0]`、nonzero host result、LastError `0 -> 0`、first allocation consumerを測定する。owner `checkpoint-process-private-heaps`へnamespace `vine.logical-private-heap.v1`のtyped handle `0x68010001`とmetadata-only live heapを作り、generation／dirtyを変えず返して14命令後のexact thunk ID `72`／`KERNEL32.dll!GetModuleHandleW`へ進むprimary-title eighth Win32 call return
- exact thunk ID `72`を10番目のdeclared endpointとし、generation `27`のlive one-argument stdcall frame／IAT callとbounded guest UTF-16をRustでchecked処理する。focused native traceで`GetModuleHandleW(L"KERNEL32.DLL")`、host HMODULE、LastError unchanged、first `GetProcAddress` consumerを測定する。owner `checkpoint-loaded-modules`へnamespace `vine.logical-loaded-module.v1`のtyped handle `0x4D010001`とmetadata-only runtime moduleを作り、generation／dirtyを変えず返して9命令後のexact thunk ID `150`／`KERNEL32.dll!GetProcAddress`へ進むprimary-title ninth Win32 call return
- exact thunk ID `150`を11番目のdeclared endpointとし、separate owner／arena `[0x7F020000,0x7F030000)`へID `1`／`0x7F020000`の`kernel32.dll!FlsAlloc` callable endpointをRX sealする。focused native traceで4件のFLS export lookupとLastError unchangedを測定し、logical KERNEL32 handle、bounded ASCII name／ordinal discrimination、`MOV ESI,[IAT] ... CALL ESI` provenanceをRustでchecked処理する。389-page generation `27` reflectionとshift後generation `30` stack reconciliationからhost pointerではないendpointを返し、5命令後、累計136命令で同じID `150`／`GetProcAddress(...,"FlsGetValue")`へ進むprimary-title tenth Win32 call return
- 同じdynamic arenaへID `1`／`FlsAlloc`とID `2`／`FlsGetValue`をordered／atomic installし、shared RX pageを一度だけsealする。second ID `150`のpersistent ESI target、13-byte call sequence、bounded ASCII nameをchecked処理し、stack owner `4`とimage owner `1`の2 pagesをgeneration `30`でtransactional reconcileする。BGI slot `0x100FEEBC`のfirst endpointを検証して`0x7F020010`を返し、guest store `0x100FEEC0`後5命令、累計141命令でthird `GetProcAddress(...,"FlsSetValue")`へ進むprimary-title eleventh Win32 call return
- ordered arenaへID `3`／`FlsSetValue`を追加し、先行2 endpointsのidentity／addressを保持する。third ID `150`のpersistent ESI sequenceとbounded ASCIIをchecked処理し、generation `30`のstack／image pagesをtransactional reconcileしてBGI slots `0x100FEEBC`／`0x100FEEC0`を検証する。`0x7F020020`を返し、guest store `0x100FEEC4`後5命令、累計146命令でfourth `GetProcAddress(...,"FlsFree")`へ進むprimary-title twelfth Win32 call return
- ordered arenaへID `4`／`FlsFree`を追加し、先行3 endpoints／storesを保持する。fourth ID `150`のpersistent ESI sequenceとbounded ASCIIをchecked処理し、generation `30`のstack／image pagesをtransactional reconcileする。`0x7F020030`を返してguest store `0x100FEEC8`を観測し、12命令後、累計158命令でexact thunk ID `70`／`KERNEL32.dll!TlsAlloc`へ進むprimary-title thirteenth Win32 call return
- exact thunk ID `70`を12番目のdeclared static endpointとし、focused TLS lifecycle traceでzero-argument `TlsAlloc`、native index `0x1C`、LastError unchanged、immediate `TlsSetValue` consumerを測定する。native indexを破棄し、owner `checkpoint-thread-local-storage`／namespace `vine.logical-tls-index.v1`のindex `0`を返す。four endpoint storesとgeneration `30`のstack／image reconciliationを保持し、7命令後、累計165命令でID `68`／`KERNEL32.dll!TlsSetValue(0,0x7F020010)`へ進むprimary-title fourteenth Win32 call return
- exact thunk ID `68`を13番目のdeclared static endpointとし、frame `[return,0,0x7F020010]`、live ESI／bound IAT、allocated slot、initial controlled thread ID `8192`をchecked処理する。stack／endpoint-store／TLS-index-storeの3 pagesをgeneration `30`でtransactional reconcileし、endpoint ID `2`をthread valueへ関連付ける。BOOL `1`を返して9命令後、累計174命令でID `89`／`KERNEL32.dll!EncodePointer(NULL)`へ進むprimary-title fifteenth Win32 call return
- exact thunk ID `89`を14番目のdeclared static endpointとし、focused native traceで`EncodePointer(NULL)`のprocess-cookie-dependent result 2件とLastError unchangedを測定する。native valuesを破棄し、owner `checkpoint-pointer-encoding`／namespace `vine.logical-encoded-pointer.v1`のtoken `0xEC010001`を返す。direct IAT frameとretained TLS state、generation `30`のstack reconciliationをchecked処理し、55命令後、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ進むprimary-title sixteenth Win32 call return
- same exact thunk ID `89`のsecond callについて、focused native trace／static disassemblyからsource `0x100A9608`をexecutable process-initializer guest functionとして分類する。11-byte direct-IAT frame、17-byte source prefix、prior encoded-NULL／TLS stateをchecked処理し、token `0xEC010002`を返す。guest global store `0x100FF62C`を観測し、9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ進むprimary-title seventeenth Win32 call return
- review Step 2のfirst migrationとして`ApiContract`／`AbiSchema`／`ApiDispatcher`／`ApiCallFrame`／`ExecutionSession`／typed outcome／faultをRustへ導入する。`GetCurrentProcessId`、`QueryPerformanceCounter`、`GetStartupInfoW`、`TlsSetValue`、`EncodePointer`の5 contract／6 existing call occurrenceがowned endpoint lookup、checked frame、pointer access、guest write、return／LastError policyの共通経路を通る。代表public semantic testと累積BGI runを通し、238命令／140,660-byte report／SHA-256 `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`を維持するgeneric API dispatcher first migration
- review Step 3のfoundationとしてsingle process／controlled initial threadのauthoritative `RuntimeState`をRustへ導入する。process／thread identity、per-thread LastError、clock／startup、heap／module／dynamic endpoint、TLS／encoded pointerを同じsession stateへ集約し、5 migrated handlerからJSON result注入を除く。strict LIFO callback frameとnested callback中の`WaitingHost` completion／scheduler eventを代表testで通し、同じ238命令／report／reflection digestを維持するruntime state foundation
- review Step 4の17-call migrationとして残り11 occurrence／8 API contractを追加し、13 distinct APIs／17 existing occurrencesをすべて`ApiDispatcher`／`dispatch_kernel32_api`へ通す。`NameOrOrdinal` schemaで`GetProcAddress` nameをbounded処理しordinalを明示的unsupportedにする。callsite provenance／page reconciliation／evidence serializationはharnessへ残し、semantic test、238命令、140,660-byte report／既存digestを維持するgeneric dispatcher cumulative migration
- sealed fixed T1の6 production pagesをsemantic reflectionでprovisional v86 pagingへcopyし、actual TLB／fetch／generated-code invalidation後にgeneration `6`をacknowledgeする。harness-only 4 KiB stackからentry `0x10001000`を実行し、exact owned thunk ID `1`へ到達するv86 entry ingress
- ordered production thunk metadataでexact thunk ID `1`を分類し、fixed cdecl frame `[return,19,23]`をchecked readしてEAX `42`を返す。guest側のargument cleanup後にexact thunk ID `2`へ到達し、callback target `0x10001030`／value `42`を構造化するcdecl service ingress
- separate runtime ownerのRX control pageへcallback return sentinel ID `1`／`0x7F010000`を置き、frame ID `1`でchecked stackへ`[sentinel,42]`を積む。guest callback `0x10001030`を実行し、exact sentinelへEAX `43`で戻るcallback return ingress
- frame ID `1`をretireし、outer cdecl thunk ID `2`からEAX `43`、ESP `0x006FFFF4`、EIP `0x10001019`でguestへ戻す。guest自身のargument cleanup、result `43`、entry result `0`、final ESP `0x00700000`を通し、harness-only unmapped return `0xDEADC0DE`のstructured `#PF`まで測るouter return ingress
- 同じruntime control pageのslot `2`／`0x7F010010`へprocess-entry return sentinel ID `2`を追加し、initial stackからguest entryをexact sentinelへEAX `0`、ESP `0x00700000`でreturnさせる。fixture result `43`を保持し、その他のunmapped returnを`#PF`のままにするentry return ingress
- Rust-owned owner `4`／stack region `[0x006FF000,0x00700000)`をRW commitし、initial ESPへprocess-entry sentinelをchecked initializationする。generation `12`の8 production pagesをreflectし、adapter-local stack allocationなしで同じentry completionへ到達するstack bootstrap ingress
- import-free PE32 bootstrap fixtureをnative Windowsでself-oracleし、production Rustの7 pagesをreflectしたsource-v86へFS base `0`で渡す。最初の`FS:[0x18]`をprevious EIP `0x10001000`、CR2 `0x18`、PFEC `4`で止めるbootstrap FS first-stop ingress
- separate owner `4`／`5`のtyped TEB／PEB pagesへself `+0x18`とPEB `+0x30`だけをchecked initializationし、generation `15`の9 pagesをreflectする。CPU state ABIでFS base `0x7FFDE000`をrestoreし、同じfixtureをentry result `0`／sentinel `0x7F010010`まで通すbootstrap FS ingress
- separate import-free fixtureでnative i386の`PEB+0x10` process-parameters pointer、`ProcessParameters+0x40` command-line `UNICODE_STRING`、buffer `+0x04`をself-oracleする。同じminimum TEB／PEBでzero pointerを観測し、silent successせずentry result `2`へ戻すprocess-parameters first-stop ingress
- typed owner `6`／`7`のprocess-parameters／command-line pagesへ測定済みpointerと`UNICODE_STRING`、UTF-16 `bootstrap-params.exe`をchecked initializationする。generation `21`の11 pagesをreflectし、同じfixtureをmatch index `0`／entry result `0`まで通すprocess-parameters ingress
- 初回full reflection acknowledgement後の通常RW／NX data writeをgeneration `11`の1-page／4 KiB `MemoryDelta`としてlive v86 mappingへ適用し、cache flushなしでguest x86がEDX:EAX `0x8877665544332211`を読むdata-write roundtrip。mapping operation、partial generation ack、write-to-code cache effectもRust contractで分離する
- changed-state／frame before-image `GuestMemoryTransaction`でtransaction開始時のfull cloneを除去する。owner／region／pageは初回mutationだけ、既存frame bytesは初回write／releaseだけをjournalし、新規frameは逆allocator操作で捨てる。process environment／import／reconciliation rollbackを保持し、4 MiB baselineで開始0 bytes、1／2 changed pageのpeak frame payload 4／8 KiBを通す
- review Step 6としてADR-003をAcceptedにし、Rust `GuestMemory` byte authority／v86 execution mirrorを確定する。`VINERF01` reflection、`VINEDL01` host delta、`VINEWB01` CPU writebackをcontrol metadata＋contiguous 4 KiB payloadへ分離し、`VINEFR01`でbounded frame化する。9 reflected／8 writable pagesの連続2 sliceでAPI固有listなしにstack／heap／surface／RWXの7 pages／49 bytesと別4 pages／13 bytesをPTE Dirty bitから検出し、Dirty clear＋TLB flush、generation `18 -> 19 -> 20 -> 21`、atomic Rust reconciliationを通すautomatic memory writeback
- review Step 7としてzero-import Rust/Wasm coreとsource-v86 mirrorをChrome Runtime Workerへ載せる。controlled PE32を46命令／10 slices／3 periodic yieldsで実行し、2 deferred host completions、depth-2 nested callback、7 writeback batches／9 pages、Canvas `[255,0,0,255]`、880 Hz Web Audio、IndexedDB `0x5A`、exact clean exitを通す。pending-request cancellationと4-instruction watchdogはいずれもdangling host／callback 0。30分／11,326-session stressはfailure 0、31 memory samples、heap slope 136 bytes/min、maximum heartbeat gap 16.28 msとなるChrome Worker vertical slice
- review Step 8としてcommit `a0d99e5`のclean Chrome `150.0.7871.115` runでversion 1 engine matrixを測定する。integer 134.1M ips、x87 52.5M ips、SSE2 83.7M ips、adapter trap 254,518/s、production API 11,279/s、active steady memory 14.58 MB、hidden-tab ratio 0.985、audio max gap 51.315 ms、threshold failure 0でv86継続`go`とする。reportは18,161 bytes／SHA-256 `e50ad355361f91e01baae1dcb6f62fb813b2ddc1202466682c60eda7e959a1e7`となるbrowser engine gate
- review Step 9 foundation 1としてRust `RuntimeState`へversion 1 `RuntimeObjectTable`を導入する。type 8／generation 8／slot 16 handle、separate object／handle lifetime、multiple process／thread handles、close／reuse、stale／double-close／type mismatch、process／thread wait signal、subsystem-owned module／heap policyを固定する。initial handles `0x50010001`／`0x54010001`と既存heap／module valuesを通し、runtime core Wasmは687,523 bytes／SHA-256 `ad1941f95e96577f2e84462d534b95ca9e09f39d1e39e9f4f922ba0fac00b633`となるruntime object-model foundation
- review Step 9 foundation 2としてprocess-owned `RuntimeModuleGraph`をRust `RuntimeState`へ導入する。bounded ASCII canonical basename、main EXE／guest DLL／runtime system module class、process root／runtime service／static import／dynamic thread reference、cycle rejection、registered／ready／attached／detached state、dependency-first attach／strict reverse detachをatomic contractで固定する。runtime core Wasmは690,816 bytes／SHA-256 `5bfa7b7141a3f0a30acd1c58e6a28afe2c5fc1e8f8b5e5859c97e3068a11cac0`となり、Chrome Worker／BGI 17-call／browser `go`を維持するruntime module-graph foundation
- review Step 9 foundation 3としてcontrolled process／initial threadへexplicit lifecycleを導入する。process `bootstrapping -> running -> exit-requested -> exited`、thread `initialized -> running -> exit-requested -> exited`、scheduler `initialized`／`terminating`、pending host／callback cleanup、request中nonsignaled、thread／process terminal signalをtargeted atomic planで固定する。RuntimeState全体をcloneせず最大2 wait recordsだけをplanし、runtime core Wasmは710,027 bytes／SHA-256 `a641572313cf256ea28ff5c2b53b7b4a04bc6a7f9eab294780a59eed5bb4c38f`、Chrome Worker／BGI 17-call／browser `go`を維持するruntime process-thread lifecycle foundation
- review Step 9 foundation 4としてcontrolled second thread registryを`RuntimeState`へ導入する。unique TID、typed thread object／handle、per-thread LastError／TLS／callback／scheduler lane、explicit selection、thread-only exit、TID昇順のprocess-wide terminationを固定する。controlled IDs `4097`／`8192`、second handle `0x54010002`、signal order `4097 -> 8192 -> process`を通し、runtime core Wasmは732,779 bytes／SHA-256 `dcbade608dc66c75c3179d4d7b67d7c0ddc32891c6769b1fdc38e20dd70d4ab8`、Chrome Worker／BGI 17-call／browser `go`を維持するruntime thread-registry foundation
- review Step 9 foundation 5としてtyped process／thread objectをcontrolled wait queueへ接続する。registration ID、object-indexed FIFO、`WaitingObject` lane、thread-exit completionによるsignal／wake、exact `WaitSatisfied` event、already-signaled fast path、process-exit cancellationをatomic planで固定する。thread `8192`がhandle `0x54010002`をwaitしthread `4097`のexit code `23`でrunnableへ戻る。runtime core Wasmは765,531 bytes／SHA-256 `3b92ff588f944db29e59cd46ae9a873bbb1d52a1433958499c6eaddd641d59aa`、Chrome Worker／BGI 17-call／browser `go`を維持するruntime wait-queue foundation
- review Step 9 foundation 6としてbounded per-thread FIFO message queueを`RuntimeThreadState`へ導入する。process-global monotonic message ID、opaque HWND／message／WPARAM／LPARAM、empty receiveの`WaitingMessage`、first enqueueだけの`MessageReady` wake、FIFO dequeue、thread/process exit cleanupを固定する。threads `4097 -> 8192`のIDs `1,2,3`とprocess-exit discard `2`を通し、runtime core Wasmは769,885 bytes／SHA-256 `db29e4846e40dc840bcf79ad70e613e2c32d251fcb05aeece1bf94ba09299db3`、Chrome Worker／BGI 17-call／browser `go`を維持するruntime message-queue foundation
- review Step 9 foundation 7としてexisting typed logical private heapをbounded Rust `GuestMemory` backingへ接続する。32 x 16 MiB controlled arena、8-byte lowest-address first-fit、monotonic allocation ID、zero／`0xA5` initialization、`0xDD` free poison／coalesce／reuse、invalid／double free、typed heap retirement、exit-request中のbacked／metadata-only process teardownをtransactional `ExecutionSession`で固定する。IDs `1,2,3`とpointer reuse `0x20000000 -> 0x20000218 -> 0x20000000`、final generation／ack `13`を通し、runtime core Wasmは772,038 bytes／SHA-256 `aa0d5d69da04fd9a9b6678d8e034e525f81b191351a1603b7fc5fe0f5420d85c`、Chrome Worker 46命令、BGI 17-call digest、browser `go`を維持するruntime heap-backing foundation
- review Step 9 foundation 8としてbounded PE32 export planとprocess graph-keyed `RuntimeModuleImageRegistry`をRustへ導入する。sealed image owner／source／range／pages、module／handle／load identity、zero entrypointをchecked登録し、case-sensitive name／ordinal、name／ordinal forwarder、32-hop bound、duplicate／overlap／unknown／cycle failure時のregistry不変を固定する。fixed 2,048-byte DLLのdirect `0x10001000`／`0x10001010`と`bridge.dll` forwardingを通し、runtime core Wasmは791,970 bytes／SHA-256 `94b0a39b7c862016f02a2af6c050cb196dab4a4badfe40bad48070827b60ca23`、Chrome Worker 46命令、BGI 17-call digest、browser `go`を維持するruntime module-image／export foundation
- review Step 9 foundation 9としてcontrolled EXE／guest DLL importsをmapped exportsとordered runtime export catalogへ接続する。3 direct guest targetsとID `1`／`0x7F000000`のruntime endpoint、3 static dependency edgesをIAT／owner sealとatomic commitする。codec -> engine attach、engine `FALSE`時のcodec unwind／ready-state retry、engine -> codec detach、strict callback frame IDs `1..7`を固定する。15-page generation `12` reflection、runtime core Wasm 794,331 bytes／SHA-256 `bbc24294dbd5acf79d9cff0a753193dda1dd5e8c6dfc4026cd811d86c9b2e05d`となり、BGI 17-call digestを維持するruntime guest-import／DLL process lifecycle
- review Step 10としてfrozen BGI 17-call workloadをprocess-owned 15-module graph、330 runtime endpoints、persistent `RuntimeState`／`ApiDispatcher`／`ExecutionSession`、PTE Dirty由来のbinary CPU writebackへ戻す。17 manifest callsをsemantic検証した後も同じgeneric loopでfour additional `EncodePointer` callsをreturnし、計21 calls／266命令でID `100`／`KERNEL32.dll!InitializeCriticalSectionAndSpinCount(0x100FF630,4000)`へ到達する。22 writeback batches／38 pages／268 bytes、3 host deltas、generation `30 -> 52`をAPI固有reconciliation listなしで通す。96,244-byte report／SHA-256 `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa`を2回一致させ、frozen 140,660-byte report／既存digestを不変に保つBGI generic runtime re-entry
- review Step 11としてnative 24-byte x86 critical-section layout、BOOL／LastError、same-thread recursive enter／leave、delete、BGI caller／consumer RVAsを測定する。Rust `RuntimeState`へguest address-keyed process-owned stateを追加し、initialize／enter／leaveの3 contracts、controlled TID、exact guest bytes、contention時のstructured unsupportedをcommon dispatcherへ接続する。same generic sessionで14 initializationsを含む35 calls、36 writeback batches／65 dirty pages／344 changed bytes、17 host deltas、final generation `80`を通し、instruction 575のID `88`／`DecodePointer(0xEC010003)`へ進む。167,719-byte report／SHA-256 `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69`を固定し、先行2 reportsを不変に保つBGI critical-section synchronization
- review Step 12としてexisting pointer traces 2件のnative inverse／LastError／immediate consumerを固定し、registered Vine tokenだけをprocess-owned source indexへ戻す`DecodePointer` contractをcommon dispatcherへ接続する。unknown／malformed tokenはmutation前に拒否する。same generic sessionで36 calls、37 writeback batches／66 dirty pages／348 changed bytes、17 host deltas、final generation `81`を通し、instruction 577のdynamic ID `1`／`FlsAlloc(0x100A5666)`へ進む。171,356-byte report／SHA-256 `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e`を固定し、先行3 reportsを不変に保つBGI pointer-decoding contract
- review Step 13として2件のnative `fls-lifecycle` traceでcallback RVA `0xA5666`、native index `9`、LastError preserve、immediate index storeを固定する。native indexを破棄し、128-slot bounded process tableへnamespace `vine.logical-fls-index.v1`のindex `0`とcallbackを保持する。binding-scoped endpoint identityでstatic／dynamic ID `1`を共存させ、dynamic `FlsAlloc`をcommon dispatcherへ接続する。same generic sessionで37 calls、38 writeback batches／68 dirty pages／399 changed bytes、17 host deltas、final generation `82`を通し、instruction 618のID `75`／`HeapAlloc(0x68010001,8,0x214)`へ進む。175,123-byte report／SHA-256 `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c`を固定し、先行4 reportsを不変に保つBGI fiber-local-storage allocation contract
- review Step 14としてexisting heap traceの`HeapAlloc(nativeHeap,0x8,0x214)`／nonnull result／LastError preserveを固定し、exact flags／sizeだけをexisting `ExecutionSession` transactionへ接続する。zeroed guest pointer `0x20000000`とreserve／commit generation `82 -> 85` deltaをv86へmapする。same generic sessionで39 calls、40 writeback batches／70 dirty pages／415 changed bytes、18 host deltas、final generation `87`を通し、instruction 644のdynamic ID `3`／`FlsSetValue(0,0x20000000)`へ進む。183,102-byte report／SHA-256 `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4`を固定し、先行5 reportsを不変に保つBGI private-heap allocation contract
- review Step 15としてexisting FLS tracesのcaller return RVA `0xA5955`でBOOL `1`／LastError preserveを固定し、dynamic `FlsSetValue(0,0x20000000)`をselected thread `8192`のlogical slot `0`へ保存する。same generic sessionでexisting second `GetModuleHandleW`／first `EnterCriticalSection`を含む42 calls、43 writeback batches／75 dirty pages／497 changed bytes、19 host deltas、final generation `91`を通し、instruction 707のID `124`／`InterlockedIncrement(0x100F08E0)`へ進む。195,821-byte report／SHA-256 `f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f`を固定し、先行6 reportsを不変に保つBGI fiber-local-storage value contract
- review Step 16としてfocused `interlocked-increment` trace 2件でcaller RVA `0xA5577`／`0xA7351`／`0xA73C7`のLONG `0 -> 1`／`1 -> 2`／`0 -> 1`、LastError preserve、later `-1 -> 0` wrappingを固定する。required aligned `InOut LONG`をchecked read／one guest write／stdcall returnへ接続し、same generic sessionで3 occurrencesとexisting critical-section／thread／startup callsを含む50 calls、51 writeback batches／86 dirty pages／667 changed bytes、26 host deltas、final generation `106`を通す。instruction 934のID `75`／`HeapAlloc(0x68010001,8,0x800)`をstructured variant stopとし、234,775-byte report／SHA-256 `4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643`を固定し、先行7 reportsを不変に保つBGI InterlockedIncrement contract
- review Step 17としてexisting heap traceのsecond `HeapAlloc(nativeHeap,0x8,0x800)`／nonnull／LastError preserveをversioned first-two-reached contractへ接続する。Frozen v1のvariant rejectionを保持し、新profileだけがallocation ID `2`／guest `0x20000218`／zeroed `0x800`をfirst blockの後へ作る。unsealed heap ownerのdirty-only deltaをmirror mapping identityへ接続し、same generic sessionで51 calls、52 writeback batches／89 dirty pages／906 changed bytes、27 host deltas、final generation `108`を通す。instruction 1395のID `94`／`GetStdHandle(STD_INPUT_HANDLE)`へ進み、239,789-byte report／SHA-256 `fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8`を固定し、先行8 reportsを不変に保つBGI second private-heap allocation contract
- review Step 18としてfocused `standard-handle` trace 2件でBGI caller RVA `0xA87B6`の`GetStdHandle(STD_INPUT_HANDLE)`／nonnull native result `0x208`／LastError `87 -> 87`と、INVALID／NULL check後の`GetFileType` consumerを固定する。native handleを破棄し、process-owned namespace `vine.logical-standard-handle.v1`のlogical stdin `0x53010001`をone-argument stdcall contractから返す。same generic sessionで52 calls、53 writeback batches／90 dirty pages／911 changed bytes、27 host deltas、final generation `109`を通す。instruction 1403のID `101`／`GetFileType(0x53010001)`へ進み、243,749-byte report／SHA-256 `5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e`を固定し、先行9 reportsを不変に保つBGI standard input handle contract
- review Step 19としてfocused `file-type` trace 2件でBGI caller RVA `0xA87C8`のstdin result `FILE_TYPE_CHAR (2)`／LastError `87 -> 87`とcharacter／pipe consumerを固定する。registered logical stdinだけを`kernel32.get-file-type-standard-input.v1`からclassifyし、existing 15th critical-section initializationを通す。same generic sessionで54 calls、55 writeback batches／94 dirty pages／938 changed bytes、28 host deltas、final generation `112`を通す。instruction 1439のID `94`／`GetStdHandle(STD_OUTPUT_HANDLE)`をmutation-free variant stopとし、252,994-byte report／SHA-256 `dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf`を固定し、先行10 reportsを不変に保つBGI file type contract
- review Step 20としてexisting `standard-handle` trace 2件でBGI caller RVA `0xA87B6`の`STD_OUTPUT_HANDLE`がnative result `0x234`／LastError `87 -> 87`を返し、same `GetFileType` consumerへ進むことを固定する。frozen input-only implementationを保ち、new profileだけを`kernel32.get-std-handle-input-output.v1`へ接続してlogical stdout `0x53010002`を返す。same generic sessionで55 calls、56 writeback batches／95 dirty pages／943 changed bytes、28 host deltas、final generation `113`を通す。instruction 1447のID `101`／`GetFileType(0x53010002)`をmutation-free variant stopとし、257,008-byte report／SHA-256 `0c6047e5c5007f244edaa5bd7ea963dd410283801d3bb572fa8a3aa82b10795d`を固定し、先行11 reportsを不変に保つBGI standard output handle contract
- valid low-alignment PEを`image-layout`／`low-section-alignment-materialization-unimplemented`としてmalformed inputと区別し、4 KiB backing未対応を明示する。15-byte conservative execute gateはpage境界を跨いで次pageがNXのsynthetic instructionを実行前に構造化停止する

未検証または未実装:

- general v86-connected process mapping、decode-aware execute gate、snapshot、O(writable-pages) dirty scanのtitle performance
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
28. 64 KiBのproduction runtime thunk rangeへ16-byte stride／1-based stable IDでowned `UD2` slotを割り当てる。controlled `abi-smoke.exe`の2 named importをatomic commitしてimage／runtime ownerをsealする。BGI declared-only policyは`Direct3DCreate9`をplanしつつ残る329 identityを全報告し、arena／IAT／generationを変更しないW7 import-binding contract。
29. 同じlive `GuestMemory`から6 sealed pagesをversion 1 reflectionとしてv86へ渡し、page directory／3 tables／6 reflected pages／1 harness stackを2 MiB poolの11 pagesへ配置する。state ABI restore、`full_clear_tlb`、`jit_clear_cache_js`後だけgeneration `6`をacknowledgeし、instruction budget `3`でentryからthunk ID `1`へ到達するproduction-memory entry ingress contract。
30. 同じreflection sessionとordered thunk metadataを使い、thunk ID `1`のfixed two-argument cdecl serviceをdispatchする。EAX `42`、callee `RET`相当のESP `+4`でguestへresumeし、guest自身のargument cleanup後にthunk ID `2`／callback target `0x10001030`／value `42`へ到達するproduction cdecl service ingress contract。
31. optional callback modeでseparate runtime owner `3`の`[0x7F010000,0x7F020000)`へcallback return sentinel ID `1`を割り当て、generation `9`の7 production pagesをreflectする。frame ID `1`、ESP `0x006FFFE8`へ`[sentinel,42]`を積んでguest callbackへ入り、6 execute stop／3 budget stop後にexact sentinelへEAX `43`、ESP `0x006FFFEC`で到達するproduction callback return ingress contract。
32. callback frame ID `1`をretireし、outer cdecl thunk ID `2`のreturn addressだけをconsumeしてguest `0x10001019`へresult `43`を返す。guest callerの2 argument cleanup、`0x10003000 = 43`、entry EAX `0`、ESP `0x00700000`を通し、9 execute stop／5 budget stop後にunmapped `0xDEADC0DE` fetchのPFEC `4`で停止するproduction outer return ingress contract。
33. optional entry-return modeでcontrol owner `3`のslot `2`へprocess-entry sentinel ID `2`／`0x7F010010`を置き、combined page SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`をgeneration `9`でsealする。initial ESP `0x006FFFFC`へexact addressを置き、9 execute stop／5 budget stop後にEAX `0`、ESP `0x00700000`でentry completionを分類するproduction entry return ingress contract。
34. `MemoryRegionKind::Stack`のowner `4`としてone-page controlled stackをreserve／RW commitし、`0x006FFFFC`へentry sentinel addressをwriteしてpage SHA-256 `3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079`でsealする。generation `12`の8 production pagesをreflectし、adapter `harness-stack` allocationを0件にして9 execute stop／5 budget stopのexact entry completionを保持するproduction stack bootstrap ingress contract。
35. import-free `bootstrap-fs.exe`で`FS:[0x18]` TEB selfと`FS:[0x30]` PEB pointerをnative exit code `0`へself-verifyする。5 image pages、control owner `2`、stack owner `3`のgeneration `9`／7-page reflectionをsource-v86へ渡し、FS base `0`の最初のreadをreason `0x10E`、PFEC `4`、CR2 `0x18`、current／previous EIP `0x10001007`／`0x10001000`で構造化するbootstrap FS first-stop contract。
36. `thread-environment-block` owner `4`／`0x7FFDE000`と`process-environment-block` owner `5`／`0x7FFDF000`をRW／NXでcommitし、TEB `+0x18`／`+0x30`だけをself／PEB pointerへ初期化する。generation `15`の9-page reflectionとFS base restoreを通し、9 execute stop／8 budget stop後に4 observation一致、EAX `0`、ESP `0x00700000`でentry sentinelへ到達するbootstrap FS ingress contract。
37. import-free `bootstrap-params.exe`でnative i386の`FS:[0x30] -> PEB`、`PEB+0x10 -> RTL_USER_PROCESS_PARAMETERS`、`+0x40 -> CommandLine`、`UNICODE_STRING+0x04 -> Buffer`をbounded self-oracleし、exit code `0`を得る。同じgeneration `15` minimum environmentでは`PEB+0x10 == 0`を観測してEAX `2`を返し、8 execute stop／7 budget stop後にentry sentinelへ到達するprocess-parameters first-stop contract。
38. `process-parameters` owner `6`／`0x7FFDC000`と`command-line` owner `7`／`0x7FFDD000`をRW／NXでcommitし、PEB `+0x10`、parameters `+0x40`のLength `40`／MaximumLength `42`／Buffer、20-unit UTF-16 basename＋NULを初期化する。generation `21`の11-page reflectionを通し、94 execute stop／93 budget stop後にmatch index `0`、EAX `0`、ESP `0x00700000`でentry sentinelへ到達するprocess-parameters ingress contract。
39. catalog-missing importだけへ`unsupported-trap` dispositionを付け、同じowned `UD2` slotへbindするoptional policyを追加する。controlled fixtureは1 declared／1 unsupportedで両owner sealを検証し、BGIは382-page quotaで330 importsを全bind、329 unsupported identities、2-page arena、generation `6`、image／runtime owner sealを得るexplicit trap-binding contract。
40. explicit large-image modeで上限400 pages、専用runnerで8 MiB protocol lineを固定し、BGI image 380 pages、thunks 2、control 1、stack 1、TEB／PEB 2、parameters／command line 2の計388 pagesをgeneration `24`でreflectする。v86 physical VGA hole `[0xA0000,0xC0000)`を除外した393 allocationsでentry `0x100A383E`を実行し、19命令後にruntime-reachable thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`／`0x7F000670`へ到達してdispatch前に停止するBGI first-trap contract。
41. 初回snapshot後のhost-originated通常data writeをdirty-page-only `MemoryDelta`へ変換し、generation単位のpartial acknowledgement、mapping operations、cache effectsを分離する。live v86へ1 pageを適用してguest readbackまで通すmemory-delta contract。
42. `GuestMemoryTransaction`を導入し、drop／error時に既存状態を含む全memory stateを復元する。controlled process environmentとimport commitの全mutation checkpointでrollback後のsame-memory retryを通すtransaction contract。
43. import bindingをread-only all-import planningとtransactional commitへ分離する。未解決時は全identityを返してmemory不変、catalog追加後retry、unsupported-trap full bind、stale plan／quota／arena collision／mid-commit rollback、successful rebind拒否を通すatomic binding contract。
44. canonical checksを`check:fast`／`check:fixture-e2e`／`check:adapter`／`check:private-corpus`へ分け、public fixture E2EとLinux GitHub Actionsを追加する。private BGI absenceは理由付きskipとする。
45. valid low-section-alignment PEをstructured unsupportedとして早期拒否し、PE planner fuzz target、randomized GuestMemory reference-model test、cross-page NX execute fixtureを追加するfoundation hardening contract。
46. controlled host clock sample `2026-07-11T00:00:00Z`を`FILETIME 134282016000000000`へ固定し、BGIのlive stack pageをgeneration／owner付きでRustへreconcileする。ID `104`のchecked frame／IAT、8-byte output、generation `25` delta、stdcall returnを通して累計23命令でID `103`／`GetCurrentProcessId`へ到達するBGI first Win32 call contract。
47. owner `checkpoint-process-identity`のcontrolled PID `4096`を固定し、ID `103`のlive stack pageをgeneration `25`へreconcileしてzero-argument stdcall frame／IATをRustで検査する。EAXへPIDを返し、累計26命令でID `149`／`GetCurrentThreadId`へ到達するBGI second Win32 call contract。
48. owner `checkpoint-thread-identity`のcontrolled TID `8192`を固定し、ID `149`のlive stack pageをgeneration `25`へreconcileしてzero-argument stdcall frame／IATをRustで検査する。EAXへTIDを返し、累計29命令でID `13`／`GetTickCount`へ到達するBGI third Win32 call contract。
49. owner `checkpoint-monotonic-clock`のcontrolled tick count `3,600,000`をfixed wall-clock sampleとpairし、ID `13`のlive stack pageをgeneration `25`へreconcileしてzero-argument stdcall frame／IATをRustで検査する。EAXへsampleを返し、累計34命令でID `157`／`QueryPerformanceCounter`へ到達するBGI fourth Win32 call contract。
50. 同じmonotonic ownerのcontrolled counter `12,345,678,901`をtick sampleとpairし、ID `157`のlive stack pageをgeneration `25`へreconcileする。one-argument stdcall frame／IAT、8-byte output、generation `26` delta、BOOL returnを通し、累計78命令でID `83`／`GetStartupInfoW`へ到達するBGI fifth Win32 call contract。
51. focused native traceでtarget caller RVA `0xA36E7`の68-byte x86 `STARTUPINFOW` full overwriteを測定する。ID `83`のlive stack pageをgeneration `26`へreconcileし、profile `startupinfow-v68-zero-optional-fields`、generation `27` delta、VOID EAX-preserve returnを通して累計87命令でID `84`／`HeapSetInformation`へ到達するBGI sixth Win32 call contract。
52. focused native traceでtarget caller RVA `0xA36FC`の`HeapSetInformation(NULL,1,NULL,0)`がBOOL `1`を返しLastError `87`を変更しないことを測定する。ID `84`のlive stack pageをgeneration `27`へreconcileし、controlled termination-on-corruption policy、generation／dirty不変、BOOL returnを通して累計108命令でID `102`／`HeapCreate`へ到達するBGI seventh Win32 call contract。
53. focused native heap-lifecycle traceでtarget caller RVA `0xA8886`の`HeapCreate(0,0x1000,0)`、nonzero host result、LastError `0 -> 0`、first `HeapAlloc` consumerを測定する。ID `102`のlive stack pageをgeneration `27`へreconcileし、namespace `vine.logical-private-heap.v1`のtyped handle `0x68010001`とbounded metadata-only heap stateを返して累計122命令でID `72`／`GetModuleHandleW`へ到達するBGI eighth Win32 call contract。
54. focused native module-handle traceでtarget caller RVA `0xA5811`の`GetModuleHandleW(L"KERNEL32.DLL")`、host HMODULE、LastError `87 -> 87`、first `GetProcAddress` consumerを測定する。ID `72`のlive stack pageをgeneration `27`へreconcileし、bounded UTF-16／case-insensitive canonical identity、namespace `vine.logical-loaded-module.v1`のtyped handle `0x4D010001`を返して累計131命令でID `150`／`GetProcAddress`へ到達するBGI ninth Win32 call contract。
55. focused dynamic-export traceでBGI caller RVA `0xA582F`から`0xA5856`の`FlsAlloc`／`FlsGetValue`／`FlsSetValue`／`FlsFree` lookupとLastError `87 -> 87`を測定する。ID `150`のregister-indirect IAT provenance、logical module、bounded ASCII `FlsAlloc`をchecked処理し、namespace `vine.dynamic-export-endpoint.v1`のendpoint `0x7F020000`を返して累計136命令でsecond `GetProcAddress`／`FlsGetValue`へ到達するBGI tenth Win32 call contract。
56. dynamic endpoint installerをordered multi-identityへ拡張し、one-endpoint tenth outputを不変に保つ。second ID `150`のpersistent ESI call、stack page、BGI writable image pageをtransaction内でreconcileし、first endpoint store `0x100FEEBC = 0x7F020000`を検証する。ID `2`／`0x7F020010`の`FlsGetValue` endpointを返し、BGI store `0x100FEEC0`後、累計141命令でthird `GetProcAddress`／`FlsSetValue`へ到達するBGI eleventh Win32 call contract。
57. same ordered endpoint pageへID `3`／`0x7F020020`／`FlsSetValue`を追加する。third ID `150`のpersistent ESI callとtwo-argument frameをchecked処理し、first／second guest storesをtransactional reconciliation後に検証する。third endpointを返してBGI store `0x100FEEC4`を観測し、累計146命令でfourth `GetProcAddress`／`FlsFree`へ到達するBGI twelfth Win32 call contract。
58. same ordered pageへID `4`／`0x7F020030`／`FlsFree`を追加する。fourth ID `150`のframe／persistent ESI call、first three endpoint storesをchecked処理し、fourth endpointを返してguest storeを観測する。累計158命令でstatic unsupported thunk ID `70`／`TlsAlloc`へ到達するBGI thirteenth Win32 call contract。
59. focused `tls-lifecycle` traceを2回取得し、`TlsAlloc()`のnative index／failure sentinel／LastError／first consumerを固定する。ID `70`のzero-argument frame／IAT、all four endpoint storesをchecked処理し、native indexを捨ててlogical index `0`を返す。累計165命令でstatic unsupported thunk ID `68`／`TlsSetValue(0,0x7F020010)`へ到達するBGI fourteenth Win32 call contract。
60. same TLS traceのsuccessful `TlsSetValue`をoracleとし、logical index `0`、initial thread ID `8192`、FlsGetValue endpoint ID `2`だけをchecked処理する。BGI TLS-index globalを含む3 pagesをtransactional reconcileし、one thread-value metadataを作ってBOOL `1`を返す。累計174命令でstatic unsupported thunk ID `89`／`EncodePointer(NULL)`へ到達するBGI fifteenth Win32 call contract。
61. focused pointer-encoding trace 2件で`EncodePointer(NULL)`のvarying native resultとLastError preserveを測定する。native result／process cookieを破棄し、typed controlled token `0xEC010001`を返す。retained TLS stateとone stack pageをchecked処理し、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ到達するBGI sixteenth Win32 call contract。
62. same tracesとstatic disassemblyでRVA `0xA9608`をprocess-initializer function、RVA `0xFF62C`をencoded store、downstreamをdecode-and-call pathとして固定する。token `0xEC010002`を返してguest storeを観測し、累計238命令でsame ID `89`／`EncodePointer(FlsAlloc endpoint)`へ到達するBGI seventeenth Win32 call contract。
63. `GuestMemoryTransaction`の全memory snapshotをchanged-state／frame before-image journalへ置換する。transaction開始はcommitted page数に依存するbyte copyを行わず、owner／region／page metadata、frame allocator、generation／delta offerをrollbackする。4 MiB／1,024-page baselineで開始0 bytes、1／2 changed pageのpeak frame payload 4,096／8,192 bytesを測定し、process environment／import failpoint、reconciliation＋release、public E2E、BGI 17-call report digestを維持するscalable transaction contract。
64. Rust `GuestMemory`をbyte authority、v86 pagingをexecution mirrorとしてADR-003をAcceptedにする。binary reflection／host delta／CPU writebackとbounded frame、全writable PTE Dirty scan、owner／generation付きatomic multi-page reconciliationを実装し、stack／heap／surface／RWXの連続2 sliceを7＋4 dirty pages、generation `18 -> 21`で通すautomatic memory writeback contract。
65. reproducible `browser-vertical.exe`とzero-import `vine-runtime-wasm` bridgeを追加し、Chrome Runtime Worker上のsource-v86 guest CPUからgeneric dispatcher、async host completion、nested callback、Canvas／Web Audio／IndexedDB、cancel／watchdog、entry sentinelまで通す。desktop／mobile screenshotと30分 repeated lifecycle stressをclean Chrome profileで固定するbrowser runtime vertical contract。
66. version 1 benchmarkでinteger／x87／SSE2、adapter trap、production generic API、callback、context、1／5／10／16 ms slice、reflection／writeback、invalidation、Worker cold start／active steady memory、background tab、AudioWorklet、Wasm crossingを測定する。clean reference runで全thresholdを満たし、v86継続`go`を記録して同じpublic gateをCIへ追加するbrowser engine decision contract。
67. generation付きtyped runtime object／handle tableを追加し、controlled process／initial thread、existing private heap／loaded moduleを同じidentity／lifetime boundaryへ入れる。multiple handle、close／reuse、stale／type mismatch、wait signal、subsystem close policyを代表Rust testで固定し、browser／BGI／benchmark regressionを維持するruntime object-model contract。
68. process-owned module graphを追加し、canonical basename／class、dependency edge／cycle rejection、process／runtime／static／dynamic reference、registered／ready／attached／detached state、dependency-first attach／reverse detachを代表Rust testで固定する。existing typed module handleへ接続し、browser／BGI／benchmark regressionを維持するruntime module-graph contract。
69. controlled process／initial threadへbootstrap／running／exit-requested／exited lifecycleとscheduler initialized／terminating statesを追加する。exit request cleanup、request中nonsignal、thread／process signal batch、duplicate／invalid transition時のstate不変、whole RuntimeState cloneなしのtargeted atomic planを代表Rust testで固定し、browser／BGI／benchmark regressionを維持するruntime lifecycle contract。
70. controlled second threadをunique TID／typed object／handleとともに登録し、LastError／TLS／callback／scheduler laneをthread-localに保持する。explicit selection、thread-only exit、all-lane preflight、TID昇順thread signal＋process-last signal、invalid process termination時のstate不変を代表Rust testで固定し、browser／BGI／benchmark regressionを維持するruntime thread-registry contract。
71. selected threadのone-object wait registrationをtyped process／thread objectへ接続し、nonsignaled queue、`WaitingObject` lane、別thread exitによるsignal、deterministic runnable wake／exact event、already-signaled immediate completion、process-exit cancellationを代表Rust testで固定し、browser／BGI／benchmark regressionを維持するruntime wait-queue contract。
72. bounded per-thread FIFOへ別registered threadからopaque raw messageをenqueueし、empty receiveの`WaitingMessage`、first-messageだけのrunnable wake／exact event、monotonic IDs、owner FIFO dequeue、reblock／rewake、exited-target rejection、process-exit discardを代表Rust testで固定し、browser／BGI／benchmark regressionを維持するruntime message-queue contract。
73. existing typed logical private heapをbounded Rust guest-memory arenaへ接続し、allocation ID／8-byte alignment、zero／deterministic nonzero initialization、free poison／coalesce／reuse、invalid／double free、transaction rollback、typed heap destroy、exit-request中のbacked／metadata-only process teardownを代表Rust testで固定する。guest HeapAlloc dispatchを追加せずbrowser／BGI／benchmark regressionを維持するruntime heap-backing contract。
74. fixed `vine_fixture.dll`のbounded export directoryをparseし、sealed image owner／source／range／page identityをprocess module graphへ登録する。direct name／ordinal、name／ordinal forwarderをguest addressへ解決し、unsealed retry、duplicate／overlap registration、unknown target／cycle resolution failure時のregistry不変を代表Rust testで固定する。guest import binding／`DllMain` callbackを追加せずbrowser／BGI／benchmark regressionを維持するruntime module-image／export contract。
75. fixed EXE／engine DLL／codec DLLとone runtime-system importをsame process graphへbindし、direct guest addresses／process-owned runtime thunk、IAT／dependency atomicity、dependency-first attach、failure unwind／retry、reverse detachを代表Rust testで固定する。`LoadLibrary`／`FreeLibrary`、TLS／thread callback、independent module retirementを追加せずbrowser／BGI／benchmark regressionを維持するruntime guest-import／DLL lifecycle contract。
76. frozen BGI 17-call imageをprocess-owned module／runtime endpoint graphへ登録し、automatic PTE Dirty writebackとpersistent Rust dispatcher sessionで21 callsまで進める。four later `EncodePointer` calls、generation `30 -> 52`、独立generic reportを固定し、instruction 266の`InitializeCriticalSectionAndSpinCount`をfirst unsupported capability clusterとしてguest bytes不変のまま構造化停止するBGI generic runtime re-entry contract。
77. opt-in native traceとcontrolled oracleでBGIの14 critical-section initializations、24-byte layout、recursive enter／leave、LastError preserve、deleteを固定する。process-owned Rust stateとinitialize／enter／leave contractsをsame generic sessionへ接続し、contentionをmutation前にstructured unsupportedとする。35 calls／instruction 575まで進め、`DecodePointer(0xEC010003)`を次のfirst unsupported capability clusterとして固定するBGI critical-section synchronization contract。
78. existing pointer tracesとstatic consumerで`DecodePointer(0xEC010003)`のinverse identity、LastError preserve、immediate dynamic callを固定する。process-owned token／source indicesを相互検証し、unknown／malformed valueをmutation前に拒否する。36 calls／instruction 577まで進め、`FlsAlloc(0x100A5666)`を次のfirst unsupported capability clusterとして固定するBGI pointer-decoding contract。
79. opt-in `fls-lifecycle` trace 2件とstatic consumerで`FlsAlloc(0x100A5666)`のcallback、native index、LastError preserve、index store／failure checkを固定する。process-owned logical index `0`をbounded tableへ割り当て、dynamic endpointをbinding-scoped common dispatcherからreturnする。37 calls／instruction 618まで進め、`HeapAlloc(0x68010001,8,0x214)`を次のfirst unsupported capability clusterとして固定するBGI fiber-local-storage allocation contract。
80. existing heap-lifecycle traceで`HeapAlloc(nativeHeap,0x8,0x214)`のnonnull result／LastError preserveとdeferred `0x800` variantを固定する。reached variantをexisting transactional heapへ接続してguest pointer `0x20000000`を返し、lazy mapping deltaをv86へ適用する。39 calls／instruction 644まで進め、dynamic `FlsSetValue(0,0x20000000)`を次のfirst unsupported capability clusterとして固定するBGI private-heap allocation contract。
81. existing FLS lifecycle tracesで`FlsSetValue(nativeIndex9,nativeHeapPointer)`のBOOL `1`／LastError preserveを固定する。process-owned slotとselected-thread valueを分離し、logical `(thread 8192,index 0) = 0x20000000`をcommon dynamic dispatcherから保存する。42 calls／instruction 707まで進め、`InterlockedIncrement(0x100F08E0)`を次のfirst unsupported endpointとして固定するBGI fiber-local-storage value contract。
82. focused `interlocked-increment` trace 2件で3 reached callerとsigned wrapを固定し、required aligned `InOut LONG`をcommon dispatcherのchecked wrapping updateへ接続する。50 calls／instruction 934まで進め、second `HeapAlloc(0x68010001,8,0x800)`を次のfirst unsupported variantとして固定するBGI InterlockedIncrement contract。
83. existing heap-lifecycle pairでsecond `HeapAlloc(0x68010001,8,0x800)`を固定し、versioned contractだけでfirst blockを保持したzeroed allocation ID `2`を作る。unsealed owner deltaをmirrorへ適用し、51 calls／instruction 1395まで進め、`GetStdHandle(STD_INPUT_HANDLE)`を次のfirst unsupported endpointとして固定するBGI second private-heap allocation contract。
84. focused standard-handle trace 2件とstatic consumerで`GetStdHandle(STD_INPUT_HANDLE)`のnonnull result／LastError preserve／`GetFileType` consumerを固定する。host handleを破棄してprocess-owned logical stdin `0x53010001`をcommon dispatcherから返し、52 calls／instruction 1403まで進め、`GetFileType(0x53010001)`を次のfirst unsupported endpointとして固定するBGI standard input handle contract。
85. focused file-type trace 2件とstatic consumerで`GetFileType(stdin)`の`FILE_TYPE_CHAR`／LastError preserveを固定する。registered logical stdinだけをclassifyし、existing critical-section initializationを通して54 calls／instruction 1439まで進め、`GetStdHandle(STD_OUTPUT_HANDLE)`を次のfirst unsupported variantとして固定するBGI file type contract。
86. existing standard-handle trace 2件とsame consumerで`GetStdHandle(STD_OUTPUT_HANDLE)`のnonnull result／LastError preserveを固定する。versioned input／output contractだけがprocess-owned logical stdoutを返し、55 calls／instruction 1447まで進め、`GetFileType(0x53010002)`を次のfirst unsupported variantとして固定するBGI standard output handle contract。

2026-07-10 に Windows 11 25H2、Chrome 150、Frida 17.15.4 で `npm run check` と BGI fingerprint を再確認した。BGI startup trace は main window、D3D9、DirectInput 8、DirectSoundCreate8、GDI+ と title resource band まで到達し、native `abi-smoke.exe` は callback 後に exit code `0` となった。同じ fixture を source-built v86 adapter 上で実行する W1 contract は result `43`、final EAX `0`、ESP `0x00700000`、budget stop 5 回で完了した。続く native `window-pixel.exe` は `WM_CREATE -> WM_PAINT -> WM_DESTROY`、queued dispatch、1 pixel、clean quit を自己検証して exit code `0` となった。2026-07-11 に同じfixed imageを使うW2 loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flowを再実行し、3 DLL／13 importをowned thunkへ解決して13 fixed partial service returnと6 callback eventを完了した。harness-only bounds `[0,0,2,2]`の`PAINTSTRUCT`とtyped HDC `0x48010001`を作り、pixel `(1,1) = 0x000000FF`を保持した。Dispatch API ID `3`／`WM_PAINT` callback ID `4`の内側でDestroy API ID `5`／`WM_DESTROY` callback ID `6`をnested実行し、`PostQuitMessage(0)`、両callback／API return、`GetMessageA`の`MSG [NULL, WM_QUIT, 0, 0, 0, 0, 0]`／return `0`を経てentry sentinel `0x00500F00`へEAX `0`、ESP `0x00700000`で到達した。66 budget stop、83 execute stop、17 owned `UD2`、231命令である。同じfinal stateからversion 1 browser present eventを1件emitし、Chrome `150.0.7871.115`のCanvas 2D sinkで1回presentして`(1,1) = RGBA [255,0,0,255]`をreadbackした。詳細は `docs/windows-native-evidence.md` を参照する。logical client／HDC／surface／queue／frame layoutとsparse eventはcheckpoint-localであり、native geometry、production common surface、general Win32、renderer backendの決定ではない。W1／W2 flowはNode.js上の固定harness、W2 fixtureはnative Windows oracleである。W3でChromeが実行するのは生成済みeventの最小Canvas sinkだけであり、guest CPU、Win32 lifecycle、Runtime Workerのbrowser実行は未実施である。

同日、commit `e4548ec`のclean worktreeからBGI fingerprintを再検証してtrace v2を15秒取得した。`IDirect3D9` rootだけを選択し、factory 4 callsとroot 11 methods／53 callsをcaller RVA付きで分離した。slot `16`の`CreateDevice`は3回とも`BGI.exe + 0x5ACA5`、return `0`であり、内部root callsはparent call IDで各CreateDeviceへ帰属する。これは`P6D-COR-01`のroot-interface部分へのpartial evidenceであり、device／resource vtable、presentation parameters内容、format／state／draw／Present／Reset、DirectInput／DirectSound methodを確定しない。LastErrorは各leave直後の観測値であり、APIが値を定義しない場合の成功判定には使わない。

同日、3,072-byte／SHA-256 `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7`の`high-memory.exe`を作り、2 MiB physical poolの60 KiBをdirectory／table／mapped pageへ割り当てて44 KiBのhigh virtual mappingを実行した。v86 pagingはpresent／R／Wとcross-page fault orderingを満たし、protect／remapは`full_clear_tlb`後に反映された。v86はPE sectionのexecute bitをnativeに表現せず`.nxprobe`を実行したため、Vine region table＋bounded physical allocator＋adapter execute gateをprovisional candidateとし、checked sparse replacementはCPU未接続のreferenceに留めた。詳細は`docs/adr-003-guest-memory.md`を正とする。

同日、Rust W7 ingressを同じBGI.exeへ2回適用し、1,976,261-byte／SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be`のdeterministic local reportを得た。329 named importと`DSOUND.dll` ordinal 11、109 `ABSOLUTE`／18,199 `HIGHLOW`を保持し、元BGI.exeのsize／SHA-256は不変だった。stopは`process-bootstrap`／`peb-teb-process-parameters-unimplemented`であり、guest memory materialization、IAT binding、PEB／TEB／initial stack、entry executionはnot attemptedである。詳細は`docs/pe-loader-ingress.md`を正とする。

同日、W7 planをproduction Rust guest-memory ingressへ接続した。high-memory fixtureは`0xF1000000`の6 image pagesを所有し、unsealed／NX execute PFEC `21`、read-only write PFEC `7`、second-page unmapped write PFEC `6`とno-partial-write、protect、quota、decommit／reuse、release／teardownを1 contractで通した。BGI planはexact quota 380 pagesで380 readable／100 writable／208 executable pagesを所有し、version 1 reportは1,831 bytes／SHA-256 `c46e40fdeb0301de1d852b4532bc0be0c4033bc5fd5f79479ab695663f7a119e`となった。379 pagesではoutputを作らず失敗し、default W7 reportはbyte-for-byte不変だった。source copy、zero-fill verification、`HIGHLOW`適用、IAT binding、owner seal、v86 invalidation／executionはnot attemptedである。詳細は`docs/guest-memory-ingress.md`を正とする。

同日、production PE32 materializationを追加した。fixed `abi-smoke.exe`を`0x10000000`へrelocateし、mapped-content SHA-256 `1cfa1ba4ef560433b38e2598ba291ae6cbd0f8139408274c2192c30e4a185110`を得た。BGIはexact 380-page quotaで6 range／1,193,472 bytesをcopyし、5 range／363,008 bytesをzero-fill／readbackし、18,199 `HIGHLOW`と330 original IAT slotを検証した。mapped 1,556,480 bytesのSHA-256は`d9dc33d121ea2da6ba5e459a9da13b70bf8bfc1c668593a80c6b3689d841199b`である。version 1 reportはcontrolled 2,436 bytes／SHA-256 `5ad5e461d0de7c40578239ee875fc84eed69ccc88af50c266706b75e4b552c68`、BGI 2,481 bytes／SHA-256 `862a57027a5e009ecef86f701c6edf13b173fb242dea4df3c629ebe0575426ee`で各2回一致した。ownerは未seal、generation `2` deltaはpendingで、IAT write、DLL／export resolution、v86接続、bootstrap／executionはnot attemptedである。詳細は`docs/pe-materialization.md`を正とする。

同日、production import bindingを追加し、後にall-or-nothingへ改修した。`abi-smoke.exe`はphysical 6 pagesでthunk ID `1`／`2`を`0x7F000000`／`0x7F000010`へ割り当て、IAT `0x10002034`／`0x10002038`をtransaction内でbindしてimage／runtime ownerをsealした。arena SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`、version 1 reportは4,483 bytes／SHA-256 `bdcc35c73320d5c628cd917b6ab0b0c9a2d2c3aefad7673ca7dc03e169a9b661`である。BGI declared-only planは`Direct3DCreate9`をID `1`へ割り当てつつ329 unresolved identitiesを全報告し、arenaを作らずIATとgeneration `2`を不変に保つ。reportは86,073 bytes／SHA-256 `6887944898fc8cecb132d63774573e6823e4ade25aba8aee6de98b31d4b6a69f`である。catalog追加後retry、commit failure rollback、successful rebind拒否を検証した。詳細は`docs/pe-import-binding.md`を正とする。

同日、controlled production memoryをprovisional v86 pagingへ接続した。Rust offerは6 pages／content SHA-256 `3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e`をphysical frame番号なしで渡し、Node adapterは2 MiB poolの11 pages／44 KiBへpage directory、3 tables、reflection、harness-only stackを配置した。state ABI restore／`full_clear_tlb`／`jit_clear_cache_js`後にgeneration `6`をacknowledgeし、Rust normal accessでIAT／entry／thunkを再検証した。entryは2 execute stop／1 budget stopでthunk ID `1`／`0x7F000000`へ達し、ESP `0x006FFFF0`、return `0x1000100A`、arguments `[19,23]`となった。version 1 reportは4,831 bytes／SHA-256 `c254b3be79eccbfd944dbae3d00376e9db81597eaccdec3488670408611a817c`である。host service dispatch、callback、production stack／bootstrap、snapshot、BGI executionはnot attemptedである。詳細は`docs/v86-entry-ingress.md`を正とする。

続けてexact thunk ID `1`をordered Rust-owned metadataから分類し、fixed cdecl `fixture_add(19,23)`をdispatchした。version 1 CPU state ABIでEAX `42`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x1000100A`へresumeし、guestが2 argumentsをcleanupした後、4 execute stop／2 budget stopでthunk ID `2`／`0x7F000010`へ到達した。callback-service frameはreturn `0x10001019`、target `0x10001030`、value `42`であり、target pageのsealed／execute metadataも検査した。version 1 reportは5,501 bytes／SHA-256 `199923cb0e0b03e0fdc042db095a22f3ef697d78010834280dca1eb2b8116c7f`である。callback frame／return sentinel、callback execution、outer service return、general ABI gatewayはnot attemptedである。詳細は`docs/v86-cdecl-ingress.md`を正とする。

続けてoptional callback modeを追加した。Rust coreはimport arenaと分離したruntime owner `3`／RX control pageへsentinel ID `1`／`0x7F010000`を置き、page SHA-256 `4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059`でsealした。7 production pagesのreflectionはgeneration `9`／content SHA-256 `45d980f7ab5aa3c98942777693438a9c780f5d6a4f0e86f2f8db63211a3af406`で、adapterは12 pages／48 KiBを使う。frame ID `1`のESP `0x006FFFE8`へ`[0x7F010000,42]`をchecked writeし、callback target `0x10001030`からexact sentinelへEAX `43`、ESP `0x006FFFEC`で戻った。6 execute stop／3 budget stop、version 1 report 7,028 bytes／SHA-256 `29ede77d61369e560197dedb35311e662e0d3c56a3c6ce5dadc9743c02c10771`である。frame retirement、outer service return、v86-originated writeback、general callback stackはnot attemptedである。詳細は`docs/v86-callback-return-ingress.md`を正とする。

続けてframe ID `1`をretireし、outer thunk ID `2`からEAX `43`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x10001019`でfixed cdecl returnした。guestは2 argumentsをcleanupし、`0x10003000`へ`43`を書いてentry result EAX `0`を作り、final `RET`でESP `0x00700000`へ到達した。initial stackのharness-only `0xDEADC0DE`はunmappedのため、9 execute stop／5 budget stop後にEIP／CR2 `0xDEADC0DE`、PFEC `4`のstructured `#PF`となった。version 1 reportは8,124 bytes／SHA-256 `de6ce28c1aa6f990fe211abd81c71415a0c908fb7a47ba6651ad0aa355cdd841`である。production-owned entry sentinel、process exit lifecycle、v86-originated writeback、general callback stackは未実装である。詳細は`docs/v86-outer-return-ingress.md`を正とする。

続けてprocess-entry sentinel ID `2`／`0x7F010010`を同じcontrol pageのslot `2`へ追加した。callback slot `1`と合わせたpage SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`、7 reflected pagesのcontent SHA-256は`b184b076d28abbf59401e35113aaeb30ca1f29b8cee590ff1d71897313bfb9db`で、generation `9`／12 adapter pagesを維持する。initial ESP `0x006FFFFC`へexact sentinelを置き、9 execute stop／5 budget stop後にEIP／PREVIOUS_EIP `0x7F010010`、EAX `0`、ESP `0x00700000`、fixture result `43`でentry completionを分類した。version 1 reportは8,674 bytes／SHA-256 `c5edbb527fad5fc0907d8784cd22df9d0c5a93bd26aa63fd413c0a6c5b75ce59`である。initial stackはなおharness-onlyで、process／thread exit lifecycle、PEB／TEB、process parametersは未実装である。詳細は`docs/v86-entry-return-ingress.md`を正とする。

続けてcontrolled initial stackをRust `GuestMemory`へ移した。anonymous owner `4`のstack region `[0x006FF000,0x00700000)`を1 page RW commitし、initial ESP `0x006FFFFC`へentry sentinel addressをchecked writeしてpage SHA-256 `3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079`でsealした。8 production pagesのreflectionはgeneration `12`／content SHA-256 `d7471bcc744e2f769861e1a40cc5520f5c0d452f2ea8539233a8573315b6f1cd`である。adapterはpage directory／3 tables／8 reflected pagesの12 pages／48 KiBを使い、`harness-stack` allocation／writeを行わない。同じ9 execute stop／5 budget stopでfixture result `43`、entry EAX `0`、ESP `0x00700000`を保持した。version 1 reportは9,430 bytes／SHA-256 `a4e0fbb559f98e32218d5c49595ed47a113f6dd18b9f3bd4411e26efcc8592c2`である。stack growth／guard、v86-originated writeback、PEB／TEB、process parametersは未実装である。詳細は`docs/v86-stack-bootstrap-ingress.md`を正とする。

続けて`bootstrap-params.exe`をnative Windowsで実行し、PEB process-parameters pointerとcommand-line layoutをexit code `0`へself-oracleした。2,560-byte fixtureのSHA-256は`50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40`である。同じfixtureをexisting minimum TEB／PEBへ渡すと`PEB+0x10`はzeroのままであり、explicit result EAX `2`、ESP `0x00700000`でentry sentinelへ戻った。generation `15`の9-page reflectionはSHA-256 `51357b546920ec78aa535488ce8d280a8027805e1d1d24e138b85264fbab9c39`、8 execute stop／7 budget stop、version 1 reportは8,351 bytes／SHA-256 `b1c05c3e9df1529196276c8905f3b97aee306610c870461d458a857c80aadde5`である。process-parameters regionとcommand-line bytesはまだ未実装である。詳細は`docs/v86-bootstrap-params-first-stop.md`を正とする。

続けてprocess-parameters／command-lineをRust ownershipへ移した。owner `6`／`7`のpagesへPEB pointer、Length `40`、MaximumLength `42`、buffer `0x7FFDD000`、UTF-16 `bootstrap-params.exe`＋NULを初期化した。PEB／parameters／command-line page SHA-256は`780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39`／`5fb4eed3eb10240c39a428c43621e7198ccf0c3089ffff6e6aea233759533f75`／`f6e0abfe79cb7fafa42be5f1450e91539b3d0386f7140523e44109249f1e19bd`である。generation `21`の11-page reflectionはSHA-256 `f9a59953a06999030e40e962a007f1ec5f0ecb73b077c4df760e212ba3d3acbb`となり、94 execute stop／93 budget stop後にmatch index `0`、EAX `0`、ESP `0x00700000`でentry sentinelへ到達した。version 1 reportは10,521 bytes／SHA-256 `c1e76294d4bba6760cbc8d022e022ef66b557d370b567d9a498798b8b2f7ac66`で2回一致し、先行3 reportはbyte-identicalである。詳細は`docs/v86-bootstrap-params-ingress.md`を正とする。

続けてexplicit unsupported trap bindingを追加した。declared-only controlled reportは4,483 bytes／SHA-256 `bdcc35c73320d5c628cd917b6ab0b0c9a2d2c3aefad7673ca7dc03e169a9b661`である。controlled fixtureは`fixture_add`だけをdeclared endpointとし、`fixture_call_callback`をID `2`／`0x7F000010`のunsupported trapへbindして両ownerをsealした。このreportは4,487 bytes／SHA-256 `298797b19f15d4a1a95c048e24820b1af9fd1aaedd7833f0fa1560a46cc429bf`である。BGIは`Direct3DCreate9`をID `1`に保ち、ID `2`の`DirectInput8Create`からID `330`の`DSOUND.dll` ordinal `11`まで329 identitiesをunsupported trapsへbindした。2-page arena SHA-256は`88f1b31a1ed2f724903f9ddaf16c789240b2010077537b9203faa44a63c20dde`、generation `6`でimage／runtime ownerはseal済みである。2回のreportは126,633 bytes／SHA-256 `7f9d96c502ae1b5cacd27da7a71cad87ada56a9fcf31ea9bc68b73bdf6ff54dc`で一致し、BGI source hashは不変だった。guest execution／API returnはnot attemptedである。詳細は`docs/pe-unsupported-trap-binding.md`を正とする。

続けてBGIのlarge-image reflectionとentry executionを追加した。existing 16-page modeを維持し、opt-in Rust limit 400 pages／8 MiBとdedicated protocol limit 8 MiBで388 sealed pagesをtransferした。4,904,602-byte offerのreflection SHA-256は`f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`、generation `24`である。naive physical placementがv86 VGA MMIO holeへ重なったため`[0xA0000,0xC0000)`を除外し、directory 1／tables 4／reflected 388の393 allocationsを2 MiB poolへ配置した。logical execute gateを毎命令確認し、entry `0x100A383E`は19 execute stop／18 budget stop／19命令でthunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`／IAT `0x100D1248`へ到達した。ESP `0x006FFFD0`、return `0x100A88D2`、raw stack `[return,0x006FFFEC,0,...]`を記録し、dispatch／returnは行っていない。version 1 reportは9,592 bytes／SHA-256 `4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840`で2回一致し、BGI source hashは前後不変である。詳細は`docs/v86-bgi-first-trap.md`を正とする。

2026-07-12にfirst reachable APIを1件returnした。ID `104`をdeclared endpointへ上げ、call site `0x100A88CC`、return `0x100A88D2`、frame `[return,0x006FFFEC]`をchecked readした。host clock owner `checkpoint-host-clock`はUTC `2026-07-11T00:00:00Z`／`FILETIME 134282016000000000`を供給する。v86が変更したstack page `0x006FF000`／owner `4`はgeneration `24`とownerを照合してRustへreconcileし、11 changed bytes／SHA-256 `46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785`を得た。8-byte writeはgeneration `25`、dirty 1 page／4 KiB、mapping／cache effects 0、delta SHA-256 `dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840`である。stdcall return後4命令、累計23命令でID `103`／`KERNEL32.dll!GetCurrentProcessId`／`0x7F000660`へ到達した。14,540-byte reportのSHA-256は`7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16`である。詳細は`docs/v86-bgi-first-win32-call.md`を正とする。

続けてsecond reachable APIをreturnした。ID `103`をdeclared endpointへ上げ、call site `0x100A88D8`、return `0x100A88DE`、zero-argument frameをchecked処理した。hostではなくowner `checkpoint-process-identity`がPID `4096`を所有する。generation `25`のcurrent stack pageとの差は4 bytes、SHA-256は`5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f`であり、reconciliation後もgeneration `25`／dirty 0を保持する。EAXへPIDを返した3命令後、累計26命令でID `149`／`KERNEL32.dll!GetCurrentThreadId`／`0x7F000940`へ到達した。18,472-byte reportのSHA-256は`bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67`である。詳細は`docs/v86-bgi-second-win32-call.md`を正とする。

続けてthird reachable APIをreturnした。ID `149`をdeclared endpointへ上げ、call site `0x100A88E0`、return `0x100A88E6`、zero-argument frameをchecked処理した。hostではなくowner `checkpoint-thread-identity`がTID `8192`を所有する。generation `25`のcurrent stack pageとの差は1 byte、SHA-256は`b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617`であり、reconciliation後もgeneration `25`／dirty 0を保持する。EAXへTIDを返した3命令後、累計29命令でID `13`／`KERNEL32.dll!GetTickCount`／`0x7F0000C0`へ到達した。22,383-byte reportのSHA-256は`d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa`である。詳細は`docs/v86-bgi-third-win32-call.md`を正とする。

続けてfourth reachable APIをreturnした。ID `13`をdeclared endpointへ上げ、call site `0x100A88E8`、return `0x100A88EE`、zero-argument frameをchecked処理した。owner `checkpoint-monotonic-clock`はhost uptimeに依存しないtick count `3,600,000`／sample ID `uptime-ms-3600000`を既存wall-clock sampleとpairする。generation `25`のcurrent stack pageとの差は1 byte、SHA-256は`f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b`であり、reconciliation後もgeneration `25`／dirty 0を保持する。EAXへtick countを返した5命令後、累計34命令でID `157`／`KERNEL32.dll!QueryPerformanceCounter`／`0x7F0009C0`へ到達した。26,558-byte reportのSHA-256は`79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111`である。詳細は`docs/v86-bgi-fourth-win32-call.md`を正とする。

続けてfifth reachable APIをreturnした。ID `157`をdeclared endpointへ上げ、call site `0x100A88F4`、return `0x100A88FA`、frame `[return,0x006FFFE4]`をchecked処理した。同じmonotonic ownerのsample ID `qpc-ticks-12345678901`はcounter `12,345,678,901`をtick sampleとpairするが、frequency／originを確定しない。generation `25`のcurrent stack pageとの差は5 bytes、SHA-256は`a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc`である。8-byte writeはgeneration `26`、dirty 1 page／4 KiB、delta SHA-256 `e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951`となった。ack後にBOOL `1`を返した44命令後、累計78命令でID `83`／`KERNEL32.dll!GetStartupInfoW`／`0x7F000520`へ到達した。31,427-byte reportのSHA-256は`eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1`である。詳細は`docs/v86-bgi-fifth-win32-call.md`を正とする。

続けてsixth reachable APIをreturnした。opt-in focused traceでnative BGI caller RVA `0xA36E7`の68-byte x86 outputをenter／leave観測し、未初期化inputが全書込されること、native outputが`cb=68`、empty reserved／desktop、absolute image-path title、flags `0x100`、3 inherited handlesを持つことを測定した。host-specific pointers／handlesはcontrolled stateへ入れず、owner `checkpoint-process-startup`のprofile `startupinfow-v68-zero-optional-fields`として`cb=68`と残りzeroを選んだ。ID `83`のframe `[0x100A36E7,0x006FFF90]`とIATをchecked処理し、generation `26` stack pageとの差31 bytes／SHA-256 `75e2211e3e9a79f92313319d9897bb1e868d74bb15f0dc513cc9cb393c9bb843`をreconcileした。68-byte writeはgeneration `27`、dirty 1 page／4 KiB、delta SHA-256 `a2c82c133e41916b0b9af10314376ece9a281a8503db4b811031e5ce52919de8`となった。VOID returnでEAXを保持した9命令後、累計87命令でID `84`／`KERNEL32.dll!HeapSetInformation`／`0x7F000530`へ到達した。frameは`[return,NULL,1,NULL,0]`、39,031-byte reportのSHA-256は`208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c`である。詳細は`docs/v86-bgi-sixth-win32-call.md`を正とする。

続けてseventh reachable APIをreturnした。opt-in focused traceでnative BGI caller RVA `0xA36FC`のarguments `[NULL,1,NULL,0]`、BOOL `1`、LastError `87 -> 87`を2回観測した。owner `checkpoint-process-heap-policy`のpolicy `heap-enable-termination-on-corruption`だけを受け付け、ID `84`のframeとIATをchecked処理した。generation `27` stack pageとの差12 bytes／SHA-256 `d121a898c549de8cf2eea5d76912375a993cac9a7dfb41d101ef2eb2b68fa3da`をreconcileし、guest writeなしでgeneration `27`／dirty 0を保持した。BOOL `1`を返した21命令後、累計108命令でID `102`／`KERNEL32.dll!HeapCreate`／`0x7F000650`へ到達した。frameは`[return,0,0x1000,0]`、43,980-byte reportのSHA-256は`09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f`である。詳細は`docs/v86-bgi-seventh-win32-call.md`を正とする。

続けてeighth reachable APIをreturnした。opt-in focused heap-lifecycle traceでnative BGI caller RVA `0xA8886`のarguments `[0,0x1000,0]`、native result `0x04A40000`、LastError `0 -> 0`、first BGI-owned `HeapAlloc(createdHeap,0x8,0x214)`を観測した。host pointerは破棄し、owner `checkpoint-process-private-heaps`へvariant `heapcreate-growable-initial-4096`、logical handle `0x68010001`、active／growable／zero-allocationのmetadata-only stateを作った。ID `102`のframeとIATをchecked処理し、generation `27` stack pageとの差8 bytes／SHA-256 `48f30a675492096d7110c249c8f8bc35da1b58ab3db07df9553e168e56e05d93`をreconcileした。guest writeなしでgeneration `27`／dirty 0を保持してreturnし、14命令後、累計122命令でID `72`／`KERNEL32.dll!GetModuleHandleW`／`0x7F000470`へ到達した。argument `0x100D24D0`はimmutable image内のUTF-16 `KERNEL32.DLL`で、49,699-byte reportのSHA-256は`f8c8ca1e678bb1e118248a6626825ddf585dc240aa59e1abc428429cbab3a1ad`である。詳細は`docs/v86-bgi-eighth-win32-call.md`を正とする。

続けてninth reachable APIをreturnした。opt-in focused module-handle traceでnative BGI caller RVA `0xA5811`の`GetModuleHandleW(L"KERNEL32.DLL")`、native result `0x758B0000`、LastError `87 -> 87`、immediate `GetProcAddress(nativeKernel32,"FlsAlloc")`を観測した。host HMODULEは破棄し、owner `checkpoint-loaded-modules`へvariant `getmodulehandlew-kernel32-loaded-runtime-module`、logical handle `0x4D010001`、canonical `kernel32.dll`、no-reference-incrementのmetadata-only stateを作った。ID `72`のframe／IATと26-byte UTF-16 nameをchecked処理し、generation `27` stack pageとの差9 bytes／SHA-256 `2d08da9d16a6782982e4b2b9e4617909c4ec2a65fbc1702479972ad4618f4b2e`をreconcileした。guest writeなしでgeneration `27`／dirty 0を保持してreturnし、9命令後、累計131命令でID `150`／`KERNEL32.dll!GetProcAddress`／`0x7F000950`へ到達した。argumentsは`[0x4D010001,0x100D250C]`／ASCII `FlsAlloc`で、56,729-byte reportのSHA-256は`147a3599cf942af03034c7f410d333b35c0360812936e39a7473c43008d8d5be`である。詳細は`docs/v86-bgi-ninth-win32-call.md`を正とする。

続けてtenth reachable APIをreturnした。opt-in focused dynamic-export traceでnative BGIが`FlsAlloc`、`FlsGetValue`、`FlsSetValue`、`FlsFree`を同じKERNEL32 handleからlookupし、4件ともLastError `87 -> 87`を保持することを観測した。host addressesは破棄し、separate runtime owner `9`のarena `[0x7F020000,0x7F030000)`へnamespace `vine.dynamic-export-endpoint.v1`、ID `1`、address `0x7F020000`、identity `kernel32.dll!FlsAlloc`のRX endpointを作った。endpoint追加によりtenth modeだけ389 pages／generation `27`でinitial reflectionし、既存write generationは`28`／`29`／`30`へshiftする。ID `150`のframe `[0x100A582F,0x4D010001,0x100D250C]`、14-byte register-indirect IAT sequence、9-byte ASCII `FlsAlloc\0`をchecked処理し、generation `30` stack pageとの差15 bytes／SHA-256 `0ec9e7bed9bb310bdab5ae447581656314c744cf4d8fe2104a36d9fe297215c7`をreconcileした。host pointerではなくendpointを返した5命令後、累計136命令で同じID `150`／`GetProcAddress`、ASCII `FlsGetValue`へ到達した。65,858-byte reportのSHA-256は`54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`である。詳細は`docs/v86-bgi-tenth-win32-call.md`を正とする。

続けてeleventh reachable APIをreturnした。same focused traceのRVA `0xA583C`／`FlsGetValue`をoracleとし、dynamic pageへID `1`／`0x7F020000`／`FlsAlloc`とID `2`／`0x7F020010`／`FlsGetValue`をordered installした。shared page SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`、389-page reflection SHA-256は`de382ad25a1487a1c8b038f48eace933d0270cf745820419b776689c87a50eeb`である。second callのframe `[0x100A583C,0x4D010001,0x100D2500]`、persistent ESI `0x7F000950`、13-byte code sequence、12-byte ASCII nameをchecked処理した。generation `30`でstack pageとimage page `0x100FE000`をtransactional reconcileし、各2 changed bytes、image slot `0x100FEEBC = 0x7F020000`を検証した。ID `2`を返し、BGIが`0x100FEEC0`へstoreした5命令後、累計141命令でsame ID `150`／ASCII `FlsSetValue`へ到達した。76,844-byte reportのSHA-256は`40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`である。詳細は`docs/v86-bgi-eleventh-win32-call.md`を正とする。

続けてtwelfth reachable APIをreturnした。same traceのRVA `0xA5849`／`FlsSetValue`をoracleとし、ordered pageへID `3`／`0x7F020020`を追加した。page SHA-256は`3b6345284f0929d025fc45cb9ae5f1d720c7575ec110cb41e7fbcaaa2fad2439`、389-page reflection SHA-256は`8025131a9a8687ec01f68969f6cda2c102eaff9258a45bfcd29c5c5c3ae4b344`である。third frame `[0x100A5849,0x4D010001,0x100D24F4]`、persistent ESI、13-byte sequence、12-byte ASCII nameをchecked処理した。generation `30`でstack／image pagesをtransactional reconcileし、各3 changed bytes、slots `0x100FEEBC = 0x7F020000`／`0x100FEEC0 = 0x7F020010`を検証した。ID `3`を返し、BGI store `0x100FEEC4`後5命令、累計146命令でsame ID `150`／ASCII `FlsFree`へ到達した。89,289-byte reportのSHA-256は`28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`である。詳細は`docs/v86-bgi-twelfth-win32-call.md`を正とする。

続けてthirteenth reachable APIをreturnした。same traceのRVA `0xA5856`／`FlsFree`をoracleとし、ordered pageへID `4`／`0x7F020030`を追加した。page SHA-256は`82ff4f595efb66795e9086158d940bdfb0f8e30a73d6d16879d599eb5afab615`、389-page reflection SHA-256は`0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`である。fourth frame `[0x100A5856,0x4D010001,0x100D24EC]`、persistent ESI、13-byte sequence、8-byte ASCII nameをchecked処理した。generation `30`でstack／image pagesをtransactional reconcileし、2／3 changed bytes、first three storesを検証した。ID `4`を返してBGI store `0x100FEEC8`を観測し、12命令後、累計158命令でID `70`／`KERNEL32.dll!TlsAlloc`／`0x7F000450`へ到達した。103,179-byte reportのSHA-256は`1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`である。詳細は`docs/v86-bgi-thirteenth-win32-call.md`を正とする。

続けてfourteenth reachable APIをreturnした。opt-in `tls-lifecycle` traceは6,138,442 bytes／SHA-256 `214ca138648d6674860ba3fc43e18c017470f0b31e023f1e7e9e4afa88dae08b`、confirmation traceは62,273 bytes／SHA-256 `fd23eb23fca34953216b0a1cf11c5ce4d88959618c5bc1173b58eab5199ad30e`である。両方でRVA `0xA58AA`の`TlsAlloc()`はnative index `0x1C`、LastError `87 -> 87`を返し、RVA `0xA58C1`の`TlsSetValue(0x1C,native FlsGetValue address)`が直後のconsumerだった。host valuesは破棄した。ID `70`のzero-argument frame／IATとall four endpoint storesをchecked処理し、generation `30`でstack／image pagesを各3 changed bytesへreconcileした。namespace `vine.logical-tls-index.v1`のindex `0`を返し、7命令後、累計165命令でID `68`／`TlsSetValue`／frame `[0x100A58C1,0,0x7F020010]`へ到達した。110,236-byte reportのSHA-256は`4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`である。詳細は`docs/v86-bgi-fourteenth-win32-call.md`を正とする。

続けてfifteenth reachable APIをreturnした。same native evidenceのRVA `0xA58C1`／BOOL `1`／LastError `87 -> 87`に限定し、ID `68`のframe `[0x100A58C1,0,0x7F020010]`、live ESI `0x7F000430`、`CALL ESI`、bound IATをchecked処理した。generation `30`でstack `0x006FF000`／endpoint stores `0x100FE000`／TLS index global page `0x100F0000`をone transactionでreconcileし、changed bytesは`8`／`0`／`12`、TLS index store `0x100F0764 = 0`を検証した。initial thread ID `8192`へdynamic endpoint ID `2`／`FlsGetValue`をmetadataとして関連付け、BOOL `1`を返した9命令後、累計174命令でID `89`／`KERNEL32.dll!EncodePointer(NULL)`へ到達した。120,499-byte reportのSHA-256は`25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`である。詳細は`docs/v86-bgi-fifteenth-win32-call.md`を正とする。

続けてsixteenth reachable APIをreturnした。focused pointer tracesはRVA `0xA5467`の`EncodePointer(NULL)`からnative result `0x6F713C58`／`0x765B8B70`、LastError `0 -> 0`／`87 -> 87`を観測し、process-specific valuesを破棄した。ID `89`のframe `[0x100A5467,NULL]`、direct IAT call、retained logical TLS slot／initial-thread endpoint valueをchecked処理し、namespace `vine.logical-encoded-pointer.v1`のtoken `0xEC010001`を返した。generation `30`のstack pageは13 bytes changed、SHA-256 `5f103e07657780d9f8a17053498cf300c1edee9a17272a20455ffd453fdff42a`である。55命令後、累計229命令でsame ID `89`／`KERNEL32.dll!EncodePointer(0x100A9608)`へ到達した。129,530-byte reportのSHA-256は`310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`である。詳細は`docs/v86-bgi-sixteenth-win32-call.md`を正とする。

続けてseventeenth reachable APIをreturnした。same tracesはRVA `0xA9697`のsource RVA `0xA9608`からnative results `0x6F766E99`／`0x71094A70`、LastError unchangedを観測する。static disassemblyはsourceをprocess-initializer guest function、first consumerをglobal RVA `0xFF62C` store、downstreamを`DecodePointer`後のindirect callとして固定した。frame `[0x100A9697,0x100A9608]`、11-byte call、17-byte executable prefix、prior encoded-NULL／TLS stateをchecked処理し、token `0xEC010002`を返した。stack changed bytesは`32`で、guest store `0x100FF62C = 0xEC010002`を観測した9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ到達した。140,660-byte reportのSHA-256は`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`である。詳細は`docs/v86-bgi-seventeenth-win32-call.md`を正とする。

まだ存在しないもの:

- controlled fixtureを越えるproduction TypeScript／Wasm browser packageとtitle import lifecycle
- general v86 process integration、bound multi-module CPU execution、x86 callback stack、dynamic DLL load／unload lifecycle
- broad calling convention、arbitrary callback／reentrancy、SEH を扱う production ABI gateway
- CPU-context-backed cooperative multi-thread／general wait／message scheduler
- production／general Win32 API compatibility implementation
- primary titleをimport／launchできるbrowser application
- production graphics、audio、storage backend
- stdout `GetFileType`以降のreachable capability dispatch／returnと、その先のentry execution evidence

現状を「production runtime が一部完成している」と誤認しないこと。成立しているのは toolchain、CPU adapter、Windows native evidence、W1／W2／W3、fixed high-address memory measurement、W7 Rust load／memory／entry return／stack／minimum TEB・PEB／process-parameters ingress、explicit import trap binding、BGI firstからseventeenth Win32 call return、23 API contractsのgeneric dispatcher、single-process RuntimeState foundation、17-call checkpoint manifest／generic runner、changed-state memory transaction journal、Rust byte authority、automatic binary v86 writeback、controlled Chrome Runtime Worker vertical slice、version 1 browser engine gate、typed runtime object／handle、process-owned module graph、controlled lifecycle／two-thread registry／one-object wait／raw message queue／private-heap backing、mapped image／bounded PE32 export、controlled guest／runtime import bindingとDLL process attach unwind／retry／detach、process-owned critical-section initialization／same-thread recursion、process-owned encoded-pointer inverse mapping、bounded process-owned FLS allocation、two reached zeroed private-heap allocations／selected-thread FLS value／checked `InterlockedIncrement`／logical standard input／output handles／stdin file typeである。per-thread CPU bootstrap／general multi-thread／wait-any／wait-all／timeout、critical-section contention blocking／delete、User32 message semantics、broad HeapAlloc flags／sizes／failure／free／realloc、standard error、output／broad file type／file I/O、adjacent interlocked operations／cross-worker atomicity、bound multi-module guest CPU execution、x86 `DllMain` stack materialization、`LoadLibrary`／`FreeLibrary`／TLS callback／module retirement、general TLS／FLS get／free／callback teardown、BGI browser execution、production browser host scheduler、title workload performanceは未実装／未測定である。unsupported trapやfixture host serviceはWin32 API実装ではない。

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
npm run check:fast
npm run check:fixture-e2e
npm run check
```

`check:fast`はdocumentation、runtime checkpoint manifest、LLVM PE／Wasm fixture、Rust fmt／test／clippy `-D warnings`、JavaScript syntax、pin済みv86 prebuilt smoke、trace summarizer contractを実行する。`check:fixture-e2e`はsource adapterを準備し、public entry／cdecl／callback／return／stack／bootstrap／MemoryDelta／window／browser presentationを通す。Linuxではexact fixture hashへ紐づくrecorded Windows bootstrap oracleをprovenance付きで使い、native PE実行済みとは扱わない。source adapterはWindows GNUとLinux GNUのexact artifact profileを別々に検証する。`check`はこの2群を順に実行する。PE32 system import libraryはtracked DEFから`llvm-dlltool -m i386 -k`で生成し、Windows SDK `.lib`には依存しない。adapterの広いmemory／CPU checkは`check:adapter`、BGI依存は`check:private-corpus`へ分離する。

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
npm run check:runtime:browser-worker
npm run check:runtime:browser-worker:stress
npm run check:runtime:v86-benchmark
npm run check:runtime:pe
npm run check:runtime:memory-core
npm run check:runtime:materialization
npm run check:runtime:imports
npm run check:runtime:entry
npm run check:runtime:cdecl
npm run check:runtime:callback
npm run check:runtime:outer-return
npm run check:runtime:entry-return
npm run check:runtime:stack-bootstrap
npm run check:runtime:bootstrap-fs-first-stop
npm run check:runtime:bootstrap-fs
npm run check:runtime:bootstrap-params-first-stop
npm run check:runtime:bootstrap-params
npm run check:runtime:memory-delta
npm run check:runtime:memory-writeback
npm run check:runtime:bgi-first-trap
npm run check:runtime:bgi-first-win32-call
npm run check:runtime:bgi-second-win32-call
npm run check:runtime:bgi-third-win32-call
npm run check:runtime:bgi-fourth-win32-call
npm run check:runtime:bgi-fifth-win32-call
npm run check:runtime:bgi-sixth-win32-call
npm run check:runtime:bgi-seventh-win32-call
npm run check:runtime:bgi-eighth-win32-call
npm run check:runtime:bgi-ninth-win32-call
npm run check:runtime:bgi-tenth-win32-call
npm run check:runtime:bgi-eleventh-win32-call
npm run check:runtime:bgi-twelfth-win32-call
npm run check:runtime:bgi-thirteenth-win32-call
npm run check:runtime:bgi-fourteenth-win32-call
npm run check:runtime:bgi-fifteenth-win32-call
npm run check:runtime:bgi-sixteenth-win32-call
npm run check:runtime:bgi-seventeenth-win32-call
npm run check:runtime:bgi-generic-runtime-reentry
npm run check:runtime:bgi-critical-section-synchronization
npm run check:runtime:bgi-pointer-decoding
npm run check:runtime:bgi-fiber-local-storage-allocation
npm run check:runtime:bgi-private-heap-allocation
npm run check:runtime:bgi-fiber-local-storage-value
npm run check:runtime:bgi-interlocked-increment
npm run check:runtime:bgi-private-heap-second-allocation
npm run check:runtime:bgi-standard-input-handle
npm run check:runtime:bgi-file-type
npm run check:runtime:bgi-standard-output-handle
```

`prepare:engine` は固定 source を取得し、workspace-local Rust 1.97.0 を用意し、overlay を適用して build と一つの adapter contract check を行う。`check:runtime:abi`、`check:runtime:memory`、`check:runtime:window-loader`は先に`npm run check:toolchain`で生成したfixed fixtureも必要とする。`check:runtime:entry`から`check:runtime:entry-return`はproduction reflectionとfixed ABI returnを段階的に固定し、`check:runtime:stack-bootstrap`はcontrolled stackをRust ownershipへ移す。`check:runtime:bootstrap-fs-first-stop`はnative FS oracleとzero-base fault、`check:runtime:bootstrap-fs`はRust-owned TEB／PEBとFS-base restore後のentry completion、`check:runtime:bootstrap-params-first-stop`はnative command-line oracleとzero process-parameters pointer、`check:runtime:bootstrap-params`は測定済みparameters／UTF-16 bytesのRust ownershipとentry completionを固定する。`check:runtime:memory-delta`は初回reflection後のdirty data pageをlive v86へ差分適用し、`check:runtime:memory-writeback`は連続slice後のPTE Dirty pagesをbinary batchでRust authorityへ戻す。`check:runtime:browser-worker`はguest CPUからbrowser output／cleanupまでをclean Chromeで通し、`:stress`は30分 lifecycle slopeを測る。numbered BGI checksはfirst trapと17件のAPI returnを累積再現し、generic re-entry checkは同じfrozen semanticsを独立検証して21 calls／first unsupported capabilityまで進める。critical-section capability checkは同じgeneric pathで35 calls／`DecodePointer` first stop、pointer-decoding capability checkは36 calls／dynamic `FlsAlloc` first stop、FLS-allocation capability checkは37 calls／static `HeapAlloc` first stop、private-heap capability checkは39 calls／dynamic `FlsSetValue` first stop、FLS-value capability checkは42 calls／`InterlockedIncrement` first stop、InterlockedIncrement capability checkは50 calls／second `HeapAlloc(...,0x800)` variant stop、second-allocation capability checkは51 calls／`GetStdHandle(STD_INPUT_HANDLE)` first stop、standard-input capability checkは52 calls／`GetFileType(0x53010001)` first stop、file-type capability checkは54 calls／`GetStdHandle(STD_OUTPUT_HANDLE)` variant stop、standard-output capability checkは55 calls／`GetFileType(0x53010002)` variant stopまで進める。これらはprimary BGI fileとsource adapterを必要とする。source adapterを使うpublic fixture checkpointは`check:fixture-e2e`を通じて通常の`npm run check`へ含まれ、BGIだけはprivate corpus checkに留める。

Generated output は `.work/` に置く。raw artifact は commit せず、重要な結果だけ小さな manifest または Markdown に要約する。

## 10. Active Queue

Windows W0、W1／W2／W3、trace v2、high-address memory、W7 Rust PE32 load／memory／entry return／stack bootstrap／bootstrap FS／process-parameters ingress／explicit unsupported trap binding／BGI first runtime trapに加え、MemoryDelta、atomic import binding、canonical public E2E／CI、low-alignment rejection、negative／randomized／fuzz ingress、cross-page execute gateは 2026-07-11 までに完了した。2026-07-12にはBGIの最初の17 APIをchecked returnしてsame exact ID `89`／`EncodePointer(0x7F020000)`へ到達し、review Step 2／3とStep 4の13 contracts／17 occurrence generic dispatcher migration、17-call expectation manifest／generic session runner、Step 5のchanged-state transaction journal、Step 6のAccepted byte authority／automatic binary multi-page writeback、Step 7のChrome Runtime Worker vertical slice／30分stress、Step 8のversion 1 browser benchmark／v86継続`go`、Step 9 foundation 1のgeneration付きtyped object／handle、foundation 2のprocess-owned module graph、foundation 3のcontrolled process／initial-thread lifecycle、foundation 4のcontrolled two-thread registry、foundation 5のcontrolled one-object wait queue、foundation 6のcontrolled raw message queue、foundation 7のcontrolled private-heap backing、foundation 8のmapped module／bounded PE32 export、foundation 9のguest import／runtime catalog IAT bindingとDLL process attach failure unwind／retry／detachまで通した。2026-07-13にはStep 10でfrozen 17-call workloadをgeneric process／memory／dispatcher sessionへ戻して21 calls後のfirst unsupported capabilityを再測定し、Step 11でnative critical-section evidence、process-owned recursion、14 BGI initializationsをsame pathへ接続して35 calls／instruction 575の`DecodePointer(0xEC010003)`へ進め、Step 12でprocess-owned inverse mappingを追加して36 calls／instruction 577のdynamic `FlsAlloc(0x100A5666)`へ進め、Step 13でbounded process-owned FLS allocationをdynamic common-dispatch pathへ追加して37 calls／instruction 618の`HeapAlloc(0x68010001,8,0x214)`へ進めた。BGI専用checkpointの追加は凍結したまま、次を一つずつ進める。

Step 14でreached private-heap allocationをexisting transactionへ接続し、guest pointer
`0x20000000`、lazy reserve／commit delta、second `DecodePointer`を通して39 calls／
instruction 644のdynamic `FlsSetValue(0,0x20000000)`へ進めた。

Step 15でexisting native BOOL／LastError evidenceを再確認し、process-owned logical FLS slot
`0`とselected thread `8192`のvalue `0x20000000`をsame dynamic dispatcherへ接続した。
Existing second `GetModuleHandleW`／first `EnterCriticalSection`を通して42 calls／instruction
707のID `124`／`InterlockedIncrement(0x100F08E0)`へ進めた。

Step 16でfocused native trace 2件の3 reached caller／LastError／signed wrappingを固定し、
required aligned `InOut LONG`をcommon dispatcherのchecked one-write returnへ接続した。
Existing critical-section／thread／startup contractsを含む50 calls／instruction 934の
`HeapAlloc(0x68010001,8,0x800)`へ進めた。

Step 17でsecond measured allocationだけをversioned HeapAlloc contractへ接続し、first blockを
保持してallocation ID `2`／guest `0x20000218`をzero initializeした。Unsealed heap ownerの
dirty-only deltaをmirror mapping identityへchecked適用し、51 calls／instruction 1395の
ID `94`／`GetStdHandle(STD_INPUT_HANDLE)`へ進めた。

Step 18でfocused native trace 2件のnonnull result／LastError preserveとstatic consumerを固定し、
host handleを破棄してprocess-owned logical stdin `0x53010001`をone-argument contractから返した。
52 calls／instruction 1403のID `101`／`GetFileType(0x53010001)`へ進めた。

Step 19でfocused file-type trace 2件の`FILE_TYPE_CHAR`／LastError preserveとconsumerを固定し、
registered logical stdinだけをclassifyした。Existing 15th critical-section initializationを通して
54 calls／instruction 1439のID `94`／`GetStdHandle(STD_OUTPUT_HANDLE)`へ進めた。

Step 20でexisting standard-handle trace 2件のstdout result／LastError preserveとconsumerを固定し、
new versioned profileだけがprocess-owned logical stdout `0x53010002`を返すようにした。
Frozen input-only profileを保ち55 calls／instruction 1447のID `101`／
`GetFileType(0x53010002)`へ進めた。

1. exact `GetFileType(0x53010002)`のnative `FILE_TYPE_PIPE`／LastError／immediate consumerを固定し、registered logical stdoutだけをversioned file-type contractへ接続してsame generic sessionの次のexact stopまで進める。Standard error、console writes、redirection、close、broad file I/Oを同時に追加しない。

項目1が次の primary implementation checkpointである。Standard output handleはnative／public semantic／private BGI evidenceを通し、frozen `bgi-seventeenth-win32-call`と先行11 runtime-kernel reportsをcompatibility anchorsとして保持する。dedicated eighteenth-call returnは追加しない。詳細は`docs/bgi-standard-output-handle.md`、`docs/generic-api-dispatcher.md`、`review.md` Step 20を正とする。

## 11. Major Open Questions

優先順:

1. reached stdout `FILE_TYPE_PIPE`をbrowser-native output path／later console／file semanticsへどう接続するか。
2. IDirect3D9 rootでは11 methodsへ到達したが、device／resource vtable、presentation parameters内容、format／state／draw／Present／Reset、DirectInput／DirectSound methodは何か。
3. controlled heap backingとbidirectional dirty-page syncは成立したが、title capacity／general stack mapping、snapshot、O(writable-pages) scan optimizationを同じpaging boundaryへどう結び付けるか。
4. version 1 production import thunkのaddress／ID ownershipをcallback sentinel、Win32 exception delivery、generated API metadataとどう統合するか。
5. cooperative guest threads、wait／message、async host responseをどうsuspend／resumeするか。
6. controlled browser packageの手書きprotocolをschema generation、version negotiation、production title importへどう拡張するか。
7. D3D9 COM vtable、format、state、draw pathの実到達subsetは何か。
8. DirectInput 8 device／format／Acquire pathとDirectSound ordinal／interfaceは何か。
9. WebGL2／WebGPU、AudioWorklet、OPFS／IndexedDBのどれをbaselineにするか。
10. interpreter性能がfull-play workloadに足りるか。足りない場合、どのhot pathだけを最適化するか。
11. 全編プレイ中にのみ到達するAPI、media、save、font、locale差をどう段階的に閉じるか。

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
| `docs/current-checkpoint.json` | 現在のprimary checkpoint、owned trap、次checkpointの機械可読manifest |
| `docs/requirements.md` | functional／non-functional requirements |
| `docs/corpus-inventory.md` | title fingerprint、dependency、corpus role |
| `docs/target-architecture.md` | component boundary、flow、ADR候補 |
| `docs/adr-003-guest-memory.md` | Rust byte authority、v86 execution mirror、slice synchronization／failure policy |
| `docs/guest-memory-ingress.md` | production Rust sparse memory、fixture／BGI layout、fault／invalidation contract |
| `docs/pe-loader-ingress.md` | Rust workspace、immutable PE32 plan、W7 BGI result／structured stop |
| `docs/pe-materialization.md` | source copy／zero-fill／`HIGHLOW`、fixture／BGI mapped-content evidence、IAT stop |
| `docs/pe-import-binding.md` | read-only all-import plan、runtime thunk arena／ID、atomic IAT commit／rollback、unresolved retry |
| `docs/pe-unsupported-trap-binding.md` | catalog-missing identity trap policy、controlled contract、sealed BGI image evidence |
| `docs/memory-delta-and-transactions.md` | initial reflection、incremental dirty-page delta、generation ack、transaction rollback、cross-page execute gate |
| `docs/memory-writeback.md` | Rust byte authority、PTE Dirty discovery、binary multi-page CPU writeback evidence |
| `docs/generic-api-dispatcher.md` | 23 API contracts、RuntimeState authority、frozen manifest runner、capability profiles |
| `docs/bgi-generic-runtime-reentry.md` | process module graph、automatic writeback、21 generic calls、critical-section first stop |
| `docs/bgi-critical-section-synchronization.md` | native 24-byte oracle、process-owned recursion、14 BGI initializations、DecodePointer first stop |
| `docs/bgi-pointer-decoding.md` | native inverse evidence、process-owned token lookup、dynamic FlsAlloc first stop |
| `docs/bgi-fiber-local-storage-allocation.md` | native callback／index evidence、bounded process slots、dynamic dispatch、HeapAlloc first stop |
| `docs/bgi-private-heap-allocation.md` | native flags／size evidence、transactional guest block、FlsSetValue first stop |
| `docs/bgi-fiber-local-storage-value.md` | native BOOL／LastError evidence、selected-thread FLS value、InterlockedIncrement first stop |
| `docs/bgi-interlocked-increment.md` | native wrapping evidence、checked LONG update、second HeapAlloc variant stop |
| `docs/bgi-private-heap-second-allocation.md` | versioned second heap block、unsealed-owner delta、GetStdHandle first stop |
| `docs/bgi-standard-input-handle.md` | native standard input result／consumer、logical handle、GetFileType first stop |
| `docs/bgi-file-type.md` | native stdin classification／consumer、logical GetFileType、stdout first stop |
| `docs/bgi-standard-output-handle.md` | native stdout result／consumer、logical handle、output file-type first stop |
| `docs/browser-runtime-worker.md` | Worker guest CPU、async completion、nested callback、browser outputs、cancel／watchdog、30分stress evidence |
| `docs/v86-browser-benchmark.md` | version 1 CPU／ABI／memory／Worker benchmark、threshold、v86 go decision |
| `docs/runtime-object-model.md` | typed object／handle identity、generation／close／stale、process／thread waitable lifetime |
| `docs/runtime-module-graph.md` | canonical module identity、dependency／reference ownership、attach／detach order |
| `docs/runtime-module-images.md` | sealed mapped image identity、bounded name／ordinal／forwarder resolution、failure atomicity |
| `docs/runtime-dll-lifecycle.md` | guest／runtime import IAT、static dependency、DLL process attach unwind／retry／detach |
| `docs/runtime-process-thread-lifecycle.md` | bootstrap／running、exit request／completion、atomic thread／process signal order |
| `docs/runtime-thread-registry.md` | controlled second thread、thread-local state、selection／exit、all-lane termination order |
| `docs/runtime-wait-queue.md` | one-object wait registration、thread signal／wake、already-signaled／exit cancellation |
| `docs/runtime-message-queue.md` | per-thread raw FIFO、empty block／first-message wake、ordered dequeue／exit cleanup |
| `docs/runtime-heap-backing.md` | bounded guest arena、allocation identity／initialization、free／reuse、heap／process teardown |
| `docs/checks-and-ci.md` | canonical check groups、private corpus skip、Linux CI、fuzz reproduction |
| `docs/v86-entry-ingress.md` | production page reflection／invalidation ack、bounded stack、entry-to-thunk evidence |
| `docs/v86-cdecl-ingress.md` | exact thunk metadata、fixed cdecl return、guest cleanup、callback-service ingress |
| `docs/v86-callback-return-ingress.md` | separate control owner／sentinel、frame ID、one-level guest callback return evidence |
| `docs/v86-outer-return-ingress.md` | callback frame retirement、outer cdecl return、guest result／entry continuation evidence |
| `docs/v86-entry-return-ingress.md` | process-entry sentinel、exact successful entry completion、stack balance evidence |
| `docs/v86-stack-bootstrap-ingress.md` | Rust-owned initial stack、bootstrap metadata、adapter-local stack removal evidence |
| `docs/v86-bootstrap-fs-first-stop.md` | native TEB／PEB FS contract、import-free reflection、zero-base exact fault evidence |
| `docs/v86-bootstrap-fs-ingress.md` | typed Rust TEB／PEB regions、FS-base restore、exact field／entry completion evidence |
| `docs/v86-bootstrap-params-first-stop.md` | native process-parameters／command-line contract、zero PEB pointer、explicit unsupported result evidence |
| `docs/v86-bootstrap-params-ingress.md` | typed process-parameters／command-line regions、exact UTF-16 bytes、entry completion evidence |
| `docs/v86-bgi-first-trap.md` | bounded large-image reflection、v86 physical placement、BGI first runtime-owned trap evidence |
| `docs/v86-bgi-first-win32-call.md` | controlled clock sample、bounded stack reconciliation、`FILETIME` delta、first API return／next trap evidence |
| `docs/v86-bgi-second-win32-call.md` | controlled process identity、generation-preserving reconciliation、second API return／next trap evidence |
| `docs/v86-bgi-third-win32-call.md` | controlled thread identity、generation-preserving reconciliation、third API return／next trap evidence |
| `docs/v86-bgi-fourth-win32-call.md` | controlled monotonic sample、generation-preserving reconciliation、fourth API return／next trap evidence |
| `docs/v86-bgi-fifth-win32-call.md` | controlled performance counter、generation 26 delta、fifth API return／next trap evidence |
| `docs/v86-bgi-sixth-win32-call.md` | native startup-info observation、controlled x86 struct、generation 27 delta、sixth API return／next trap evidence |
| `docs/v86-bgi-seventh-win32-call.md` | native heap-policy observation、generation-preserving reconciliation、seventh API return／next trap evidence |
| `docs/v86-bgi-eighth-win32-call.md` | native heap lifecycle observation、typed logical private heap、generation-preserving eighth API return／next trap evidence |
| `docs/v86-bgi-ninth-win32-call.md` | native module-handle observation、bounded UTF-16 lookup、typed logical loaded module、ninth API return／next trap evidence |
| `docs/v86-bgi-tenth-win32-call.md` | native dynamic-export observation、register-indirect IAT provenance、callable FlsAlloc endpoint、tenth API return／next lookup evidence |
| `docs/v86-bgi-eleventh-win32-call.md` | ordered endpoint pair、persistent ESI provenance、first endpoint guest store、eleventh API return／next lookup evidence |
| `docs/v86-bgi-twelfth-win32-call.md` | ordered endpoint triple、prior endpoint guest stores、twelfth API return／next lookup evidence |
| `docs/v86-bgi-thirteenth-win32-call.md` | ordered four-endpoint page、prior guest stores、thirteenth API return／`TlsAlloc` next trap evidence |
| `docs/v86-bgi-fourteenth-win32-call.md` | logical TLS index allocation、four endpoint stores、fourteenth API return／`TlsSetValue` next trap evidence |
| `docs/v86-bgi-fifteenth-win32-call.md` | initial-thread TLS endpoint value、three-page reconciliation、fifteenth API return／`EncodePointer` next trap evidence |
| `docs/v86-bgi-sixteenth-win32-call.md` | focused pointer evidence、controlled encoded-NULL token、sixteenth API return／next non-NULL call evidence |
| `docs/v86-bgi-seventeenth-win32-call.md` | executable initializer source、second pointer token／guest store、seventeenth API return evidence |
| `docs/roadmap.md` | long-term phase、task、dependency、acceptance mapping |
| `docs/risks-and-open-questions.md` | unresolved decisions、risk、go／no-go |
| `docs/testing-and-acceptance.md` | test candidate、evidence、phase acceptance |
| `docs/x86-engine-scorecard.md` | engine comparison、v86 evidence、remaining gates |
| `docs/toolchain-spike.md` | LLVM PE／Wasm fixture baseline |
| `docs/windows-development.md` | Windows bring-up、trace、checkpoint commands |
| `docs/windows-native-evidence.md` | Windows W0、BGI trace v1／v2、selected IDirect3D9 root、W1／W2／W3、W7 loader／memory／entry return／stack／FS ingress |
| `spikes/x86-engine/README.md` | v86 source adapter reproduction |

文書内の長い test／task listは将来coverageの地図であり、今すぐ全件実装する指示ではない。本書のactive queueと「広く浅く、代表test 1本」を優先する。
