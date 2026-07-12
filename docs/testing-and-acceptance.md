# テスト戦略と受け入れ条件

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 現状: static corpus inventory、documentation check、PE32 fixture、Windows native oracle、LLVM／Wasm smoke、v86 bounded execution／CPU state、W1／W2／W3、high-address memory、Rust PE32 load／memory／entry return／stack／minimum TEB・PEB／process-parameters bootstrap、MemoryDelta／automatic binary CPU writeback／transaction／atomic import、BGI firstからseventeenth Win32 call return、controlled Chrome Runtime Worker vertical test、version 1 v86 browser benchmark／go threshold、generation付きtyped runtime object／handle、process-owned module graph contracts、Windows trace v1／v2 contracts、Linux public CIは存在する。production title Runtime Workerと対応browser matrixは未選定

この文書は、各実装を「動いたように見える」状態から、再現可能な互換性証拠へ進めるための基準を定義する。現在の canonical command は `npm run check` であり、`check:fast`とpublic source-adapter経路の`check:fixture-e2e`を順に実行する。広いadapter checkは`check:adapter`、private BGIは`check:private-corpus`へ分離する。

## 2. 品質原則

1. 一つの実装境界または vertical checkpoint につき、代表する automated contract test 1 本を baseline とする。
2. test を増やすのは、実 regression、calling convention／fault ordering の曖昧さ、または title failure を小さな fixture へ還元した場合に限る。
3. Windows native trace と短い title smoke を進捗の主 evidence とし、重要な semantic boundary だけ synthetic fixture で固定する。
4. Windows と browser の環境差をすべて byte equality で比較せず、契約上同じものと許容差を要するものを分ける。
5. test clock、input trace、locale、timezone、browser version は結果解釈に必要な範囲だけ記録する。
6. phase 完了は test 数ではなく、選んだ checkpoint の acceptance evidence で判定する。
7. 長時間 fuzz、soak、compiler／browser matrix は既知リスクまたは実 failure が要求するまで deferred とする。

## 3. Test asset の階層

| Tier | 内容 | 主な実行先 | 目的 |
|---|---|---|---|
| T0 | hand-authored byte vector、malformed PE fragment、API data vector | unit／fuzz test | parser、marshaller、negative test、fuzz seed |
| T1 | source から再現生成する synthetic PE32 EXE / DLL | CI | ISA、ABI、loader、Win32、GDI／GDI+、DDraw、D3D9、DInput、DSound contract |
| T2 | VN-like sample application | browser E2E | vertical slice と browser integration |
| T3 | representative title | local compatibility run | 現実 workload と title scenario |

### 3.1 Asset manifest

各 fixture は次を manifest に持つ。

- stable fixture ID と version
- source／binary hash
- compiler、linker、flags、runtime dependency
- PE feature、CPU instruction、API、期待結果
- Windows oracle を取得した環境

### 3.2 Fixture compiler diversity

初期 baseline は現在の LLVM fixture 一組とする。次の差は primary trace、compiler dependency、または unexplained mismatch が必要性を示したものだけ追加する。

- hand-authored assembly と minimal linker image
- C compiler が生成する cdecl／stdcall／fastcall sequence
- C++ compiler が生成する thiscall、vtable、exception を必要範囲で扱う fixture
- dynamic DLL load、ordinal、forwarder、TLS、delay import
- x87／SSE code generation と structure return／packing

## 4. Oracle

### 4.1 Oracle の優先順位

| Oracle | 用途 | 注意点 |
|---|---|---|
| 仕様／model oracle | PE bounds、path normalization、handle state machine | 仕様の解釈自体を review する |
| Windows differential oracle | CPU、ABI、API、loader lifecycle | Windows version、locale、timer、font 差を記録する |
| Reference interpreter | optimized CPU engine の比較 | reference 側にも bug があるため独立 vector を併用 |
| Golden image／state | graphics、window state、save state | font／GPU の許容差を分類する |
| Offline audio oracle | mixer、cursor、loop、resample | realtime scheduling は別試験が必要 |
| Metamorphic property | parser、VFS、CPU、renderer | 単一期待値を作りにくい入力に使う |

Windows differential harness は Vine の一部を Windows で動かすという意味ではない。同じ自作 fixture を Windows 環境で実行して観測結果を生成し、Vine の結果と比較する開発用 test system である。

### 4.2 Normalize してよい差

- wall-clock の絶対時刻
- nondeterministic handle 値、address 値。ただし同値関係と lifetime は比較する
- timer の限定した許容幅
- browser／font substitution が承認された場合の pixel 差
- audio resampler の承認された数値誤差

error code、callback 順、stack cleanup、LastError preservation、file content、registry type、surface pitch、loop point 等を、便宜的に normalize してはならない。

## 5. Test layer

以下は project 全体の coverage 候補であり、着手時に一括して test case 化する checklist ではない。各 checkpoint では到達した範囲から代表 case を一つ選ぶ。

現在のrepresentative Windows trace contractは、v2 eventのglobal call ID、thread-local parent call ID、caller module＋RVA、enter／leave target identity、LastError、duration、explicit selected vtableを一つのdeterministic nested A／W wrapper fixtureで検査する。`IDirect3D9`は17 named slotsとslot 16 `CreateDevice` pairを含む。call ID gap／mismatch、wrong parent、missing caller、invalid LastError／durationを拒否し、historical v1 inputはlegacy thread-stack pairingで読み込む。別vtableやmethodごとのtestは実capture failureが要求するまで追加しない。

representative adapter memory measurementは、fixed PE32／LAA imageを`0xF1000000`へnon-identity mapし、high EIP／stack、R page write、unmapped read／write、cross-page write、protect／remap前後を一つのsource-built v86 runnerで検査する。PFEC `7`／`4`／`6`、second-page fault前のno partial write、explicit TLB invalidationをassertする。同じmetadataをchecked sparse referenceへ与え、execute-denied／unmapped fetchをPFEC相当`21`／`20`として検査する。v86がnon-executable sectionを実行する結果もgapとしてassertし、成功に読み替えない。

representative production memory contractは同じhigh-memory planをRust-owned sparse regionへ準備し、6 image pagesと1 anonymous probe pageでpending invalidation、unsealed／NX fetch PFEC `21`、read-only write PFEC `7`、cross-page unmapped write PFEC `6`／no partial write、protect、exact quota failure、decommit／reuse、release／owner teardownを一つのsequenceで検査する。BGIの380-page layoutはversion 1 probe evidenceとして別に記録し、同じautomated testを複製しない。guard、all-width access、snapshot、actual generated-code invalidation、Worker testは具体的なfailureまたは次gateまで増やさない。詳細は[guest-memory-ingress.md](guest-memory-ingress.md)を正とする。

incremental memory contractは初回full reflection後に通常RW／NX pageへ8 bytesを書き、dirty 1 pageだけのgeneration `11` deltaをlive v86へ適用してguest readbackする。automatic writeback contractはbinary reflection後の連続2 sliceで全writable PTEをscanし、API固有listなしにstack／heap／surface／RWXの7 page／49 bytes、host delta後の別4 page／13 bytesをRust authorityへatomic reconcileする。untouched page、Dirty clear、TLB flush、generation `18 -> 21`、stale generation／wrong owner／truncated payloadのno-partial-change、all five binary mapping descriptorsを固定する。Rust testsはcross-page／same-page writes、consecutive generation、partial／stale／duplicate／future ack、write-to-code、protect、decommit／release、pending offer中mutationを固定する。transaction contractはprocess environmentの20 mutation checkpointとimport arena／各thunk／各IAT／両seal checkpointを失敗させ、stats、owners、regions、frames、bytes、protection、seal、generation、pending deltaの復元とsame-memory retryを検査する。randomized reference-model sequenceはcommit／protect／read／fetch／write／decommit／release／owner replacement／rollback／delta適用を比較する。詳細は[memory-delta-and-transactions.md](memory-delta-and-transactions.md)と[memory-writeback.md](memory-writeback.md)を正とする。

### 5.1 Static と repository checks

- formatter、lint、type check、dependency policy
- generated file が schema から再現可能で差分なし
- API metadata の重複 export、ordinal collision、unknown type 検査
- unsafe／raw pointer boundary の audit
- dependency integrity と artifact manifest
- documentation link、ID、support matrix の整合性

### 5.2 Unit と property test

- checked integer、RVA／offset、guest pointer range
- page table、allocation、protection transition
- register、flags、decode、effective address
- calling convention、structure packing、string conversion
- handle generation、object state、LastError
- path、case folding、wildcard、overlay resolution
- message queue、wait object、timer、clock
- pixel conversion、palette、pitch、clip region
- PCM format、cursor、ring wrap、mixer

Property test では allocation/free sequence、path segment、PE section layout、surface rectangle、audio chunk size 等を生成し、不変条件を検査する。

### 5.3 PE loader tests

正常系 fixture は少なくとも次を分離して持つ。

- preferred base と relocated base
- multiple section、zero-fill、unusual but valid alignment
- import by name／ordinal、forwarder、circular dependency failure
- delay import、resource、TLS data／callback
- adopted API-set／system module alias、manifest／activation context
- EXE -> DLL -> second DLL の attach／detach 順
- dynamic LoadLibrary／GetProcAddress／FreeLibrary
- entry point より前の relocation、IAT、TLS callback

negative corpus は truncated header、invalid signature、RVA overflow、overlapping section、directory out-of-range、relocation overflow、import recursion、oversized count／string、PE32+ on PE32-only build を含む。

合格条件は、valid fixture が expected module trace を返し、invalid fixture が host crash、panic、unbounded allocation を起こさず stable error class を返すことである。

現在のW7 representative contractは、SHA-256固定の2,560-byte `abi-smoke.exe`をRustのowned immutable snapshotへ取り込み、strict PE32／i386 header、4 section、1 module／2 named import、1 relocation block／6 wordsをversion 1 load planへ変換する。1 `ABSOLUTE`／5 `HIGHLOW`、entry RVA `0x1000`、source hash、deterministic JSONを検査し、`process-bootstrap`／`peb-teb-process-parameters-unimplemented`で停止する。同じplannerはfingerprint済みBGI.exeについて5 section、14 modules／330 imports、237 relocation blocks／18,308 wordsを返した。guest memoryへのmap、IAT binding、DLL graph、PEB／TEB、entry実行はcontract外である。詳細は[pe-loader-ingress.md](pe-loader-ingress.md)を正とする。

続くguest-memory ingress contractはW7 planのheader／section mapped rangesを1 image ownerへcommitし、final R／W／X metadataとbounded physical quotaを検査する。allocator-zeroed pagesを所有するだけでsource copy、PE zero-fill、relocation、IAT、owner seal、CPU実行はcontract外であり、stopは`image-materialization`／`image-bytes-relocations-import-binding-unimplemented`である。

現在のPE32 materialization contractは同じ`abi-smoke.exe`を`0x10000000`へrelocateする。4 source ranges／2,560 bytes、5 zero ranges／17,920 bytes、5 `HIGHLOW` target、2 original IAT slots、20,480 mapped bytesのSHA-256を1 sequenceで検査する。source-derived expected sectionへdeltaを独立適用し、全copy／zero／relocated DWORD／IAT／pending invalidation／source immutabilityをassertして`import-binding`／`iat-binding-unimplemented`で停止する。BGI replayは18,199 relocationsと330 IAT slotsを同じboundaryで通したがautomated testを複製しない。詳細は[pe-materialization.md](pe-materialization.md)を正とする。

現在のimport-binding contractは全importをread-only planし、未解決が1件でもあれば全identityを返してarena、IAT、generationを不変に保つ。catalog追加後のsame-image retryを通し、complete planだけが16-byte strideのowned thunk IDをtransaction内で全IATへ書いてruntime／image ownerをsealする。6-page exact quota、`UD2`／`HLT` bytes、arena digest、generation `6` acknowledgement前のaccess block、acknowledgement後のIAT／fetch、sealed IAT write PFEC `7`、stale plan、arena conflict、quota、capacity、mid-commit rollback、successful rebind拒否を検査する。追加testはcatalogを1 endpointに絞り、2番目をidentity付き`unsupported-trap`へbindする。BGI declared-only replayは329 unresolved identitiesをmemory変更なしで返し、explicit policyは1 declared／329 unsupported thunksを全bindしてimageをsealする。詳細は[pe-import-binding.md](pe-import-binding.md)と[pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md)を正とする。

`npm run check:runtime:entry`は同じlive Rust memoryから6 sealed pagesをversion 1 semantic reflectionとしてsource-built v86へ渡す。2 MiB poolへpage directory／3 tables／6 reflected pages／1 harness stackを置き、state ABI restore、explicit TLB／generated-code clear後だけgeneration `6`をacknowledgeする。Rust normal read／fetch receipt、reflection metadata／content digest、slice-boundary execute gate、entry `0x10001000`、instruction budget `3`、first thunk ID `1`、ESP／return／2 arguments、deterministic reportを1 sequenceで検査する。詳細は[v86-entry-ingress.md](v86-entry-ingress.md)を正とする。

`npm run check:runtime:cdecl`は同じsessionを継続し、offerのordered thunk metadata、exact thunk ID `1`、fixed frame `[return,19,23]`、EAX `42`／ESP `+4`／guest returnへのstate restore、guest側argument cleanup、exact thunk ID `2`、callback return／target／value、sealed executable target pageを1 sequenceで検査する。4 execute stop／2 budget stopと5,501-byte deterministic reportを固定するが、callback frameを構築または実行しない。詳細は[v86-cdecl-ingress.md](v86-cdecl-ingress.md)を正とする。

`npm run check:runtime:callback`はoptional Rust-owned control owner／RX sentinel page、generation `9` reflection、actual invalidation acknowledgement、frame ID `1`、checked writable stack `[sentinel,42]`、executable callback target、exact callback return trap、EAX `43`／ESP `0x006FFFEC`を1 sequenceで検査する。6 execute stop／3 budget stopと7,028-byte deterministic reportを固定するが、frame retirementとouter service returnを行わない。詳細は[v86-callback-return-ingress.md](v86-callback-return-ingress.md)を正とする。

`npm run check:runtime:outer-return`はそのexact stopからframe ID `1`をretireし、outer cdecl return state EAX `43`／ESP `0x006FFFF4`／EIP `0x10001019`を復元する。guest caller cleanup、result `0x10003000 = 43`、entry EAX `0`、final ESP `0x00700000`、unmapped harness return `0xDEADC0DE`のreason `0x10E`／PFEC `4`／CR2を1 sequenceで検査する。9 execute stop／5 budget stopと8,124-byte deterministic reportを固定するが、production-owned entry sentinelやprocess exitを実装しない。詳細は[v86-outer-return-ingress.md](v86-outer-return-ingress.md)を正とする。

`npm run check:runtime:entry-return`は同じcontrol ownerのslot `2`へtyped process-entry sentinel ID `2`／`0x7F010010`を追加し、combined page／7-page reflection digest、generation `9` acknowledgement、initial stack value、exact `#UD` classification、entry EAX `0`、ESP `0x00700000`、fixture result `43`を1 sequenceで検査する。9 execute stop／5 budget stopと8,674-byte deterministic reportを固定し、既存outer modeのunmapped `#PF`も保持するが、production stack／process lifecycleを実装しない。詳細は[v86-entry-return-ingress.md](v86-entry-return-ingress.md)を正とする。

`npm run check:runtime:stack-bootstrap`はRust owner `4`のone-page stack region、RW commit、checked sentinel initialization、page digest、generation `12` seal、8-page reflection、normal-read receiptを検査する。adapter allocationに`harness-stack`がなくowner `4` reflected pageが1件あること、同じ9 execute stop／5 budget stop、entry result／ESP／fixture resultを固定する。9,430-byte reportを検査するが、general reserve／commit、guard／growth、PEB／TEB、v86-originated writebackを実装しない。詳細は[v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md)を正とする。

`npm run check:runtime:bootstrap-fs-first-stop`はimport-free PE32 fixtureをnative実行し、`FS:[0x18]` TEB self／`FS:[0x30]` PEB pointerとTEB内fieldの一致をexit code `0`で検査する。同じfixtureの5 image pages、control、Rust-owned stackをgeneration `9`でreflectし、FS base zeroのfirst instructionがreason `0x10E`、PFEC `4`、CR2 `0x18`、previous EIP `0x10001000`で停止することと5,982-byte reportを固定する。guest TEB／PEBやnonzero FS baseは実装しない。詳細は[v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md)を正とする。

`npm run check:runtime:bootstrap-fs`はtyped TEB／PEB regions、checked zero／pointer initialization、page digest、generation `15`の9-page reflection、FS base restoreを検査する。同じfixtureの4 observation、entry EAX `0`、ESP `0x00700000`、exact process-entry sentinel、9 execute stop／8 budget stopと8,214-byte reportを固定する。process parameters、loader list、TLS、LastErrorは実装しない。詳細は[v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md)を正とする。

`npm run check:runtime:bootstrap-params-first-stop`はseparate import-free fixtureをnative実行し、i386の`PEB+0x10`、`ProcessParameters+0x40`、`UNICODE_STRING.Buffer+0x04`からbounded `bootstrap-params.exe` matchまでをexit code `0`で検査する。同じfixtureをexisting minimum TEB／PEBへ渡し、zero process-parameters pointer、explicit entry result `2`、ESP `0x00700000`、8 execute stop／7 budget stop、8,351-byte reportを固定する。process-parameters region／command-line bytesは実装しない。詳細は[v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md)を正とする。

`npm run check:runtime:bootstrap-params`はtyped owner `6`／`7`、RW／NX commit、PEB `+0x10` pointer、parameters `+0x40`のLength `40`／MaximumLength `42`／Buffer、20 UTF-16 units＋NUL、3 page digests、generation `21`の11-page reflectionを検査する。ack後のnormal read、match index `0`、entry EAX `0`、ESP `0x00700000`、94 execute stop／93 budget stop、10,521-byte reportを固定し、先行3 reportsのbyte identityも保持する。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。

`npm run check:runtime:bgi-first-trap`はprimary fingerprintを前後照合し、explicit 400-page Rust limit／8 MiB protocol limit内で388 pagesをreflectする。generation `24`、reflection digest、owner別page digest、VGA MMIO holeを除外した2 MiB physical placement、PDE indices `[1,64,508,511]`、actual invalidation acknowledgementを検査する。entryから19命令でexact thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`へ到達し、EIP／ESP／return／raw stack wordsを固定する。API dispatch、output write、stdcall return、guest resumeは行わない。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

`npm run check:runtime:bgi-first-win32-call`は同じprimary fingerprint／388 pagesからID `104`をdeclared endpointとして検査する。fixed clock sample、one-argument stdcall frame、IAT call、RW／NX output、generation `24`／owner `4`の4 KiB stack reconciliation、11 changed bytes、generation `25`のone-page delta、full-page merge、zero cache effects、Rust acknowledgement、VOID EAX-preserve policy、ESP `+8`を1 sequenceで固定する。return後4命令、累計23命令でexact ID `103`／`GetCurrentProcessId`へ停止し、14,540-byte reportのSHA-256を検査する。general writeback、clock、ABI gateway、process identityは実装しない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

`npm run check:runtime:bgi-second-win32-call`はID `103`をdeclared endpointへ上げ、host PIDに依存しないowner／PID `4096`、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、4 changed bytes、generation／dirty不変、EAX result、ESP `+4`を固定する。return後3命令、累計26命令でexact ID `149`／`GetCurrentThreadId`へ停止し、18,472-byte reportのSHA-256を検査する。general process／handle／thread identityは実装しない。詳細は[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md)を正とする。

`npm run check:runtime:bgi-third-win32-call`はID `149`をdeclared endpointへ上げ、host TIDに依存しないowner／TID `8192`、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、1 changed byte、generation／dirty不変、EAX result、ESP `+4`を固定する。return後3命令、累計29命令でexact ID `13`／`GetTickCount`へ停止し、22,383-byte reportのSHA-256を検査する。general process／thread／TEB client-ID／scheduler modelは実装しない。詳細は[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md)を正とする。

`npm run check:runtime:bgi-fourth-win32-call`はID `13`をdeclared endpointへ上げ、host uptimeに依存しないowner／tick count `3,600,000`、paired wall-clock sample、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、1 changed byte、generation／dirty不変、EAX result、ESP `+4`を固定する。return後5命令、累計34命令でexact ID `157`／`QueryPerformanceCounter`へ停止し、26,558-byte reportのSHA-256を検査する。general monotonic clock／timer／scheduler／snapshot modelは実装しない。詳細は[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

`npm run check:runtime:bgi-fifth-win32-call`はID `157`をdeclared endpointへ上げ、controlled counter `12,345,678,901`、one-argument stdcall frame、IAT call、generation `25`のstack reconciliation、5 changed bytes、8-byte output、generation `26` delta／ack、BOOL result、ESP `+8`を固定する。return後44命令、累計78命令でexact ID `83`／`GetStartupInfoW`へ停止し、31,427-byte reportのSHA-256を検査する。counter frequency／origin、general clock／timer／scheduler modelは実装しない。詳細は[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md)を正とする。

`npm run check:runtime:bgi-sixth-win32-call`はfocused native traceでtarget callerの68-byte x86 layoutとfull overwriteを測定し、ID `83`をdeclared endpointへ上げる。controlled process-startup owner／profile、one-argument stdcall frame、IAT call、generation `26`のstack reconciliation、31 changed bytes、68-byte output、generation `27` delta／ack、VOID EAX-preserve policy、ESP `+8`を固定する。return後9命令、累計87命令でexact ID `84`／`HeapSetInformation`へ停止し、39,031-byte reportのSHA-256を検査する。general process creation、desktop、title path、console／standard handlesは実装しない。詳細は[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md)を正とする。

`npm run check:runtime:bgi-seventh-win32-call`はfocused native traceでtarget callerのarguments `[NULL,1,NULL,0]`、BOOL `1`、LastError unchangedを測定し、ID `84`をdeclared endpointへ上げる。controlled process heap-policy owner／ID、four-argument stdcall frame、IAT call、generation `27`のstack reconciliation、12 changed bytes、generation／dirty不変、BOOL result、ESP `+20`を固定する。return後21命令、累計108命令でexact ID `102`／`HeapCreate`へ停止し、43,980-byte reportのSHA-256を検査する。general heap allocator、corruption detection／termination、per-thread LastErrorは実装しない。詳細は[v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md)を正とする。

`npm run check:runtime:bgi-eighth-win32-call`はfocused heap-lifecycle traceで`HeapCreate(0,0x1000,0)`、nonzero native result、LastError `0 -> 0`、first `HeapAlloc(createdHeap,HEAP_ZERO_MEMORY,0x214)` consumerを測定し、ID `102`をdeclared endpointへ上げる。Rustはthree-argument stdcall frame／IAT call、generation `27` stack reconciliation、8 changed bytesを検査し、namespace `vine.logical-private-heap.v1`のtyped handle `0x68010001`とbounded metadata-only heap stateを返す。guest writeなしでgeneration／dirtyを保持し、14命令後、累計122命令でexact ID `72`／`GetModuleHandleW`へ停止する。49,699-byte reportのSHA-256を検査し、native pointer、heap backing、allocation semantics、general loader modelは実装しない。詳細は[v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md)を正とする。

`npm run check:runtime:bgi-ninth-win32-call`はfocused module-handle traceで`GetModuleHandleW(L"KERNEL32.DLL")`、native HMODULE、LastError `87 -> 87`、immediate `GetProcAddress(...,"FlsAlloc")` consumerを測定し、ID `72`をdeclared endpointへ上げる。Rustはone-argument stdcall frame／IAT call、64-code-unit bounded UTF-16 read、case-insensitive canonical module identity、generation `27` stack reconciliation、9 changed bytesを検査し、namespace `vine.logical-loaded-module.v1`のtyped handle `0x4D010001`を返す。guest writeなしでgeneration／dirtyを保持し、9命令後、累計131命令でexact ID `150`／`GetProcAddress`へ停止する。56,729-byte reportのSHA-256を検査し、native HMODULE、guest system-DLL image、general loader／export modelは実装しない。詳細は[v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md)を正とする。

`npm run check:runtime:bgi-tenth-win32-call`はfocused dynamic-export traceで4件のFLS lookup、native function addresses、LastError `87 -> 87`を測定し、ID `150`をdeclared endpointへ上げる。Rustはlogical KERNEL32 handle、two-argument stdcall frame、14-byte register-indirect IAT provenance、64-byte bounded ASCII／ordinal discriminationを検査する。separate owner／arenaのRX endpoint ID `1`／`0x7F020000`へ`kernel32.dll!FlsAlloc` identityを保持し、host pointerを返さない。tenth modeは389 pages／initial generation `27`、post-write generation `30`となり、15 changed stack bytesをreconcileする。5命令後、累計136命令でsame ID `150`／`FlsGetValue`へ停止する。65,858-byte reportのSHA-256 `54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`を検査し、general export／forwarder resolutionとFLS semanticsは実装しない。詳細は[v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-eleventh-win32-call`はsame native traceの`FlsGetValue` lookupを使い、ordered endpoint ID `1`／`2`をone RX pageへatomic installする。Rustはpersistent ESI call target、13-byte sequence、bounded ASCII nameを検査し、generation `30`のstack／writable image pagesをone transactionでreconcileする。BGI first endpoint store `0x100FEEBC = 0x7F020000`をrequired invariantとし、ID `2`／`0x7F020010`を返す。BGIのsecond store後5命令、累計141命令でsame ID `150`／`FlsSetValue`へ停止する。76,844-byte reportのSHA-256 `40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`を検査し、tenth report hashも不変に保つ。詳細は[v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md)を正とする。

`npm run check:runtime:bgi-twelfth-win32-call`はsame native traceの`FlsSetValue` lookupを使い、ordered endpoint ID `1`～`3`をone RX pageへatomic installする。Rustはthird persistent ESI call、13-byte sequence、bounded ASCII name、generation `30`のstack／writable image pagesを検査する。BGI stores `0x100FEEBC = 0x7F020000`／`0x100FEEC0 = 0x7F020010`をrequired invariantsとし、ID `3`／`0x7F020020`を返す。third store後5命令、累計146命令でsame ID `150`／`FlsFree`へ停止する。89,289-byte reportのSHA-256 `28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`を検査し、eleventh report hashも不変に保つ。詳細は[v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md)を正とする。

`npm run check:runtime:bgi-thirteenth-win32-call`はsame native traceの`FlsFree` lookupを使い、ordered endpoint ID `1`～`4`をone RX pageへatomic installする。Rustはfourth persistent ESI call、bounded ASCII name、generation `30` stack／image reconciliation、first three storesを検査し、ID `4`／`0x7F020030`を返す。fourth store後12命令、累計158命令でstatic unsupported ID `70`／`TlsAlloc`へ停止する。103,179-byte reportのSHA-256 `1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`を検査し、twelfth report hashも不変に保つ。詳細は[v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-fourteenth-win32-call`はtwo focused `tls-lifecycle` tracesで`TlsAlloc()`、native index `0x1C`、LastError preserve、immediate `TlsSetValue` consumerを測定する。ID `70`をdeclared endpointへ上げ、Rustはzero-argument frame／IAT、generation `30`のstack／image reconciliation、all four endpoint storesを検査する。native indexを捨ててnamespace `vine.logical-tls-index.v1`のindex `0`を返し、7命令後、累計165命令でunsupported ID `68`／`TlsSetValue(0,0x7F020010)`へ停止する。110,236-byte reportのSHA-256 `4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`を検査し、thirteenth report hashも不変に保つ。詳細は[v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-fifteenth-win32-call`はsame native evidenceのsuccessful `TlsSetValue` variantだけをID `68`へbindする。Rustはframe `[return,0,0x7F020010]`、live ESI／bound IAT、allocated slot、initial controlled thread、endpoint ID `2`、all four storesを検査する。stack／endpoint-store／TLS-index-storeの3 pagesをgeneration `30`でtransactional reconcileし、thread-value metadataを1件作ってBOOL `1`を返す。9命令後、累計174命令でunsupported ID `89`／`EncodePointer(NULL)`へ停止する。120,499-byte reportのSHA-256 `25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`を検査し、fourteenth report hashも不変に保つ。詳細は[v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-sixteenth-win32-call`はtwo focused `pointer-encoding` tracesの`EncodePointer(NULL)`、varying native result、LastError preserveをoracleとする。ID `89`をdeclared endpointへ上げ、Rustはframe `[0x100A5467,NULL]`、direct IAT call、retained TLS slot／initial-thread valueを検査する。native results／process cookieを捨ててnamespace `vine.logical-encoded-pointer.v1`のtoken `0xEC010001`を返し、stack pageを13 bytes reconcileする。55命令後、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ停止する。129,530-byte reportのSHA-256 `310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`を検査し、fifteenth report hashも不変に保つ。詳細は[v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-seventeenth-win32-call`はsame tracesのsecond `EncodePointer`とstatic disassemblyをoracleとする。Rustはframe `[0x100A9697,0x100A9608]`、11-byte direct IAT call、17-byte executable source prefix、prior encoded-NULL／TLS stateを検査する。native result／cookieを捨ててprocess-initializer function token `0xEC010002`を返し、stack pageを32 bytes reconcileする。BGI global `0x100FF62C`へのstoreを観測し、9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ停止する。140,660-byte reportのSHA-256 `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`を検査し、sixteenth report hashも不変に保つ。詳細は[v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md)を正とする。

現在のW2 loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flow representative contractは、SHA-256固定の`window-pixel.exe`をpreferred baseへchecked mapし、3 DLL／13 named importの全IATを一意なowned thunkへ解決する。`GetModuleHandleA(NULL)`へmain image baseを返した後、`RegisterClassExA`のfixed `wndclassexa-v48-local-ascii` variantを48-byte checked readと64-byte上限のASCII NUL readで処理し、host-owned class stateとlogical ATOM `0xC000`を登録してresumeする。続くfixed `createwindowexa-v12-local-ascii-wm-create` variantは12 argumentsをchecked readし、class／titleをbounded ASCIIからcopyし、module-scoped class lookupを通してharness-local typed HWND `0x57010001`を作る。

runtime-owned scratch `0x00501000`へ48-byte `CREATESTRUCTA`を書き、ESP `0x006FFFAC`へreturn sentinel `0x00500E00`と`[HWND, WM_CREATE, 0, CREATESTRUCTA*]`を積んでWndProc `0x00401260`を呼ぶ。callbackはEAX `0`、ESP `0x006FFFC0`でsentinelへ戻り、outer stdcallはEAX `HWND`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x0040109B`でresumeする。logical windowは`creating`から`created`へ移り、hiddenのまま保持される。fixed `showwindow-sw-show-hidden-to-visible`はcreated／hiddenのlive objectだけをvisibleへ遷移し、previous visibilityのBOOL `0`を返す。fixed `invalidaterect-whole-client-no-erase`はcall site `0x004010B3`、ESP `0x006FFFE4`のarguments `[HWND, NULL, FALSE]`をchecked readし、NULL pointerをsymbolic whole-client regionへ変換してerase falseを保持する。owner guest threadのgenerated-paint laneへsequence 1のcandidate `[HWND, WM_PAINT, 0, 0]`を1件置き、paint stateをqueuedへ遷移する。BOOL `1`をEAXへ返し、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010B9`でresumeする。

fixed `getmessagea-unfiltered-generated-paint-zero-metadata`はcall／return `0x004010CC`／`0x004010D2`、IAT／thunk `0x004020E0`／`0x00500070`、ESP `0x006FFFE0`、arguments `[0x00403034, NULL, 0, 0]`を検査する。28-byte writable MSG spanへ`[0x57010001, WM_PAINT, 0, 0, 0, 0, 0]`を全書込し、generated-paint laneを1件から0件へdequeueしてpaint stateを`queued`から`in-flight`へ遷移する。sequence 1、whole-client update region、erase falseを保持し、BOOL `1`をEAXへ返してESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010D2`でstdcall resumeする。

fixtureはdispatch／dispatched-paint countを各1へ進めた後、IAT `0x004020D8`からESIへloadしたthunk `0x00500050`を`CALL ESI`する。fixed `dispatchmessagea-generated-wm-paint-callback`はcall／return `0x004010F5`／`0x004010F7`、outer ESP `0x006FFFEC`、argument `[0x00403034]`と7-DWORD MSG copyを検査する。API call ID `3`の内側でcallback ID `4`をESP `0x006FFFD8`へ作り、sentinel `0x00500E00`と`[HWND, WM_PAINT, 0, 0]`を積んでWndProc `0x00401260`へ入る。

fixed `beginpaint-whole-client-harness-surface`はcall／return `0x0040133D`／`0x00401343`、IAT／thunk `0x004020C8`／`0x00500010`、ESP `0x006FFFC8`、arguments `[HWND, 0x0040308C]`を検査する。whole-client updateをsnapshotしてwindowからclearし、paint stateを`in-flight`から`painting`へ遷移する。harness-only client bounds `[0,0,2,2]`、typed HDC `0x48010001`、active sessionを作り、64-byte PAINTSTRUCTを`[HDC, fErase=0, rcPaint=(0,0,2,2), fRestore=0, fIncUpdate=0, reserved=zero]`へ全書込する。この2x2 boundsはrequested outer size `160x120`やnative client geometryから導出しない。

fixed `setpixel-harness-surface-exact-point`はcall／return `0x00401357`／`0x0040135D`、IAT／thunk `0x004020F8`／`0x005000C0`、arguments `[HDC, 1, 1, 0x000000FF]`を検査し、harness-local pixel mapへ1件を書いて同じCOLORREFを返す。fixed `endpaint-matching-active-session`はcall／return `0x0040138D`／`0x00401393`、IAT／thunk `0x004020DC`／`0x00500060`、同じ`[HWND, PAINTSTRUCT*]`と16 DWORDを再検証し、session／HDCをretireしてpaint stateを`none`、sequenceをNULLへ戻しBOOL `1`を返す。

fixed `destroywindow-synchronous-wm-destroy`はcall／return `0x004013D6`／`0x004013DC`、IAT／thunk `0x004020D4`／`0x00500040`、HWNDを検査し、windowを`created -> destroying`へ遷移する。Dispatch API ID `3` -> `WM_PAINT` callback ID `4` -> Destroy API ID `5` -> nested `WM_DESTROY` callback ID `6`の4 frameを保持し、callback ID `6`内のfixed `postquitmessage-thread-slot-exit-zero`がexit code `0`をthread quit slotのsequence `2`へ置く。`WM_DESTROY` return `0`、Destroy BOOL `1`／window `destroyed`、`WM_PAINT` return `0`、Dispatch LRESULT `0`の順にstrict reverse unwindしてframe depthを0へ戻す。

2回目のfixed `getmessagea-unfiltered-thread-quit-zero-metadata`はIAT `0x004020E0`からEDIへloadしたthunk `0x00500070`を`CALL EDI`し、MSG `[NULL, WM_QUIT, 0, 0, 0, 0, 0]`を全書込してquit slotをconsumeし、return `0`でloopを抜ける。fixtureの全counter／event／pixel／quit assertion後、entry sentinel `0x00500F00`へEAX `0`、ESP `0x00700000`、callee-saved registers復元で到達する。13 completed partial service return、6 callback event、66 budget stop、83 execute stop、17 owned `UD2`、231命令を含めて一致した場合だけ成功する。

これはpreferred-base固定のNode.js harnessである。既存invalidation checkpointのdeltaは`P1-ABI-01`／`P1-ABI-02`／`P1-DIAG-01`／`P1-SCH-01`／`P3-WIN-01`／`P3-WIN-03`／`P4-UI-02`への限定したpartial evidenceとして維持する。`P3-WIN-01`はlive typed lookup、`P3-WIN-03`はthread-owned generated-paint candidate 1件、`P4-UI-02`はsymbolic whole-client invalidation／erase stateだけを示す。hidden window、non-NULL RECT、region union／clip／client geometry、erase TRUEとpre-existing erase state、repeated invalidation／coalescing、invalid／stale／destroyed HWNDとfailure／LastErrorは引き続き検証しない。

message-dispatch checkpointの既存deltaは`P1-ABI-01`／`P1-ABI-02`／`P1-ABI-03`／`P1-DIAG-01`／`P1-SCH-01`／`P3-WIN-01`／`P3-WIN-03`／`P4-UI-02`への限定したpartial evidenceとして維持する。paint／quit checkpointの新しいdeltaはこれらに`P4-UI-01`と`P4-GDI-01`を加える。証明するのは2 guest callback level／4 API-callback frameのfixed unwind、bounded same-thread quit-ready path、typed HWNDの`created -> destroying -> destroyed`、1回のPostQuit／WM_QUIT Get、1 whole-client paint completion、BeginPaintからEndPaintまでのtyped HDC lifetimeだけである。

`P4-GFX-01`はclaimしない。1 pixel mapはcommon CPU surface、format、pitch、orientation、dirty-region coreではない。`P4-GDI-04`／`REQ-GDI-02`もclaimしない。`SetPixel`はfirst pixel command用test-only APIであり、BGIのcorpus primitive requirementへ昇格しない。対応するrequirementは`REQ-ABI-01`／`REQ-ABI-02`／`REQ-ABI-03`、`REQ-API-01`／`REQ-API-02`、`REQ-UI-01`／`REQ-UI-02`、`REQ-GDI-01`、`REQ-DIAG-01`へのpartial evidenceに限る。今回の成功完走による`REQ-API-06`／`REQ-DIAG-03`の新しいdeltaはなく、以前のunsupported診断evidenceだけを保持する。`AC-P1-03`、`AC-P3-02`、`AC-P4-01`、`AC-P4-02`を含むacceptance criterionは一つも完了しない。

general BeginPaint client geometry／region／clip／erase／background／nesting、invalid／stale／wrong HWND／HDC／PAINTSTRUCT、general DC state、SetPixelの別座標／clip／color／failure、paint中のreinvalidation、mismatched／double EndPaintは未検証である。DestroyWindowのchild／owner／focus／capture／timer／cross-thread／repeat、`WM_CLOSE`／`WM_NCDESTROY`／class／resource cleanup、arbitrary callback depth／SendMessage reentrancy／SEH／LastErrorも未検証である。PostQuitの別code／multiple／ordering／wake、GetMessageのfilter／block／error／他message／nonzero metadata、process teardown／DLL detach、DOM／Canvas presentation、production object／surface／queue layoutも未実装または未検証である。

`P1-SCH-01`について示すのは、instruction budget下で同じguest threadをresumeし、fixed synchronous nested callbackをunwindして既にposted済みのquit slotをconsumeできる範囲だけである。run queue、virtual clock、empty-queue wait／wakeは未実装である。

process-owned module graphの代表Rust contractはbounded ASCII basename／class、unique main、
mirrored dependency、cycle rejection時のmemory不変、static／dynamic reference、ready gate、
dependency-first attach、strict reverse detach、object-table registration rollbackを1 graphで
固定する。`RuntimeState` contractはexisting typed handleを通す2-module orderを追加する。
これはmodule metadata orderのevidenceであり、DLL image mapping、export／forwarder、TLS／
`DllMain` callback、unloadを実装済みとしない。詳細は
[runtime-module-graph.md](runtime-module-graph.md)を正とする。

### 5.4 CPU と ABI tests

CPU test vector は instruction encoding、initial register／flags／memory、expected result／fault を持つ。

- arithmetic／logical flags: CF、PF、AF、ZF、SF、OF
- branch、call、return、far／segment の必要部分
- string instruction と DF、repeat、page boundary fault
- unaligned／cross-page access、write protection、guard
- divide、invalid opcode、breakpoint、access violation
- x87 stack、control word、rounding、NaN、80 bit intermediate の方針
- MMX／SSE／SSE2 と state transition
- lock prefix と scheduler interaction

ABI fixture は stack に sentinel を置き、call 前後の ESP、callee／caller-saved register、return value、FPU state を比較する。cdecl、stdcall、thiscall、採用した fastcall、varargs、small／large structure、callback、nested callback、exception を別 case にする。

現在の W1 representative contract は固定 `abi-smoke.exe` の cdecl host call、1段 callback、callback／entry sentinel、callee-saved register、stack balanceだけを検証する。result `43`、final EAX `0`、ESP `0x00700000`、budget stop 5回をWindows native oracleと対応付けたが、他 calling convention、nested callback、exception、FPU ABI の代用にはしない。

production-memory cdecl representative contractは同じfixtureのfirst serviceだけをordered production thunk metadataからdispatchする。calleeの`RET`相当でreturn addressだけをconsumeしてEAX `42`を返し、guest callerが2 argumentsをcleanupした後にcallback-service thunk ID `2`へ到達する。これはfixed two-DWORD cdecl returnのevidenceであり、general cdecl schema、他calling convention、callback frame／return、LastError、SEHの代用にはしない。

production callback representative contractはimport thunkと分離したsentinel ID／address、active frame ID／parent thunk、outer ESP／return address、target／valueを保持し、one-level cdecl callbackの`RET`をexact sentinelで分類する。これはone frameのentry／return stopだけであり、outer unwind、nested／reentrant callback、fault／cancellation、general frame allocatorの代用にはしない。

production outer-return representative contractはそのfixed frameをretireし、callee `RET`相当でouter return addressだけをconsumeしてresult `43`をguest cdecl callerへ返す。guest自身の2 argument cleanup、result write、entry comparison、stack balanceを検証するが、process-entry return sentinel、general entry lifecycle、nested／reentrant frame unwind、callback fault／cancellationの代用にはしない。

production entry-return representative contractはtyped sentinel ID `2`をinitial stackへ置き、guest entry result `0`とbalanced ESPをexact `#UD`で分類する。別modeでunmapped returnがPFEC `4`のままであることも検査するが、adapter-local one-page stackをproduction stack、process／thread exit、PEB／TEB、process parametersの代用にはしない。

production stack-bootstrap representative contractは同じone-page bytesとbootstrap metadataをRust owner／region／page reflectionから供給し、adapter-local map／writeを0件にする。これはownership handoffのevidenceであり、PE headerの任意stack size、guard／growth、multi-thread、teardown、mutation writebackの代用にはしない。

bootstrap FS first-stop representative contractはnative self-oracleとguest exact-faultを同じimport-free imageで結ぶ。FS base `0`からCR2 `0x18`になることを要求し、TEBがあるように見せるsilent zero-page mappingを許さない。これはoffset／fault baselineであり、PEB／TEB layout全体、process parameters、TLS、LastErrorの代用にはしない。

bootstrap FS ingress representative contractはfirst-stopと同じimageへ2 typed pagesだけを追加し、TEB self／PEB pointerのdirect／indirect observationとentry returnを要求する。fixed addresses、selector `0`、empty PEBをgeneral Windows process environmentの代用にはしない。

process-parameters first-stop representative contractはnative command-line self-oracleとguest zero-pointer resultを同じimport-free imageで結ぶ。fixture-defined result `2`とexact entry returnを要求し、empty PEBをsilent successさせない。測定した3 offsetをgeneral `RTL_USER_PROCESS_PARAMETERS`、current directory、environment、loader／process lifecycleの代用にはしない。

process-parameters ingress representative contractはfirst-stopと同じimageへ2 typed pagesだけを追加し、PEB pointer、8-byte `UNICODE_STRING`、42 command-line bytesのexact readbackとentry result `0`を要求する。fixed addresses、exact basename、zero-filled unmeasured fieldsをgeneral process parametersやBGI bootstrapの代用にはしない。

W2のcreate／invalidation ingress segmentは`GetModuleHandleA(NULL)`とfixed `RegisterClassExA`をstdcall resumeし、fixed `CreateWindowExA`では48-byte argument bandを保持したまま1段のWndProc stdcall callbackを往復する。callback frameはreturn sentinelを含む20 bytesで、WndProcの`ret 16`後にouter ESP `0x006FFFC0`へ戻る。outer service完了時はreturn addressと12 argumentsの52 bytesをpopし、EAX `0x57010001`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x0040109B`とする。fixed `ShowWindow`は2 DWORD argumentsをpopし、previous visibilityのBOOL `0`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010AE`でresumeする。fixed `InvalidateRect`は3 DWORD argumentsをpopし、BOOL `1`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010B9`でresumeする。host boundaryのcallee-saved registerとentry prologueのsaved ESI／EDIを保持する。後続のpaint／quit flowはfixed 2-level callbackとfixture entry returnまで通すが、これらをgeneral stdcall、invalid stack、thiscall／fastcall、arbitrary nested callback／reentrant service、callback fault／failure、general process entry lifecycleの代用にはしない。

最適化 engine は reference path と instruction boundary または basic block boundary の state hash を比較できるようにする。

### 5.5 Windows API contract tests

API metadata は、到達した variant について必要な contract 項目を関連付ける。項目ごとに独立 test を作らず、同じ API band の代表 test へまとめる。

- valid case
- invalid handle／pointer／size
- LastError／HRESULT
- object lifetime と close
- ANSI／Unicode variant
- callback／reentrancy がある場合の順序
- cancellation／timeout／thread exit
- Windows oracle の有無と既知差分

API count だけを coverage としない。status ごとに implementation variants、semantic assertions、実 title call count を集計する。

### 5.6 VFS と registry tests

- drive、absolute／relative、`.`／`..`、separator、case、trailing dot／space の policy
- case collision、enumeration、wildcard、rename、replace
- read-only source と writable overlay の通常 file shadowing、および executable／DLL shadowing の拒否
- sharing mode、delete-on-close、locking、mapped view の採用範囲
- CP932／Unicode filename、invalid sequence、long path の採用範囲
- quota full、permission revoke、browser kill、commit interruption
- registry key case、value type、enumeration、delete、default value
- runtime schema upgrade／downgrade refusal、backup／restore
- import 後／delay-load 前の host source replacement と、managed module snapshot の restart 後 hash

crash consistency test は commit の各 injection point で process／Worker kill を模擬し、再起動後に旧状態または新状態のどちらか一方が完全に読めることを確認する。

### 5.7 Window、message、input tests

現在のW2 native oracleは関連eventだけを`WM_CREATE -> WM_PAINT -> WM_DESTROY`としてexactに記録し、queued `WM_PAINT`の`GetMessageA`／`DispatchMessageA`経由、BeginPaint／SetPixel／EndPaint、destroy／quit、exit code `0`を自己検証する。可変な補助messageは順序contractに含めない。これはWindows oracleであり、Vineのlogical HWND、host surface、browser presentation、`AC-P4`の証明ではない。

現在のW2 class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flowはfixed valid caseだけを持つ。48-byte `WNDCLASSEXA`、executable guest WndProc、main-module HINSTANCE、ASCII nameをhost-owned class stateへcopyし、local class keyとATOM `0xC000`の双方向lookupを検証する。`CreateWindowExA`ではmodule identity、class key／ATOM／copied name、copied title、styles、semantic `CW_USEDEFAULT` position、requested outer size、null parent／menu、`creating -> created` lifecycleをhost-owned logical windowへ保持する。`ShowWindow`は同じlive objectを`visible=false -> true`へ遷移し、`showCommand=5`を追加する。native oracleとの比較はclass registration／window createがnonzeroで成功し、関連eventの先頭が1回の`WM_CREATE`でcallbackが0を返すsemanticまでとする。native fixtureは`ShowWindow` returnを無視するため、previous visibilityのBOOL `0`はfixed harness contractでありnative goldenではない。ATOM／HWND数値、raw補助message、`CREATESTRUCTA` bytes、callback stack／sentinel、物理的な表示もnative比較に含めない。

同じlive visible objectへ、`InvalidateRect`はsymbolic whole-client update region、erase false、paint state queuedを保持し、owner guest threadへgenerated-paint candidate 1件を追加する。native fixtureは`InvalidateRect`のnonzero returnをbranchで要求し、最終的に関連`WM_PAINT`が1回dispatchされることを自己検証するが、Vineのregion表現、thread lane、candidate sequence、coalescingをgoldenにしない。今回のfixture callは1回だけなのでhidden windowとrepeat coalescingは未検証である。

fixed `GetMessageA`はcandidateをzero metadataのMSGへmaterialize／dequeueし、fixed `DispatchMessageA`は同じMSGからregistered WndProcの`WM_PAINT` branchへ入る。fixed paint／quit continuationはBeginPaint／SetPixel／EndPaint、nested WM_DESTROY／PostQuit、strict unwind、WM_QUIT、entry returnまで進み、native oracleの関連event／counter／COLORREF／quit code／exit `0`と一致する。native oracleはVineの2x2 bounds、PAINTSTRUCT bytes、HDC／HWND数値、pixel map、zero metadata、call ID／stack addressをgoldenにしない。Vine側の1 pixelはlogical stateであり、物理的な表示は未検証である。

W3 representative browser contractは、このW2 harnessが成功後にemitする`vine.browser-present` version `1` eventをexactly 1件だけ受け取る。payloadはframe `2x2`、`rgba8` sparse pixel `(1,1) = [255,0,0,255]`である。one-shot Canvas 2D sinkはschema／version／payloadを検査して1回だけ`putImageData`し、clean temporary Chrome profileでpresent count `1`、Canvas `2x2`、`getImageData` readback `[255,0,0,255]`をassertする。これはbrowser-facing pixel boundaryの代表test 1本であり、guest execution／Worker protocol、production surface／queue／renderer、frame timing、visible monitor output、general pixel conversionを代用しない。

後続のChrome Runtime Worker contractはW3とは別に、controlled PE32のguest CPUをWorker内で実行し、generic trap、2 deferred host completions、depth-2 callback、automatic writeback、Canvas／Web Audio／IndexedDB、pending cancel、watchdog、entry sentinelを通す。46 instructions／10 slices／3 yields、30分／11,326 sessionsのevidenceは[browser-runtime-worker.md](browser-runtime-worker.md)を正とする。これは`AC-P1-04`、`AC-P1-05`、`AC-P1-07`、`AC-P1-10`へのpartial evidenceであり、general wait／wake、host timeout、versioned performance threshold、title workloadを代用しない。

- create／show／resize／destroy と message order
- PeekMessage／GetMessage、SendMessage／PostMessage、nested dispatch
- timer、paint invalidation、focus、capture
- key down／up、repeat、modifier、scan code、mouse button／move／wheel
- focus loss 中の stuck key 解消
- canvas coordinate、DPR、aspect ratio、fullscreen transition
- browser reserved shortcut と support matrix
- test clock と recorded input trace の deterministic replay
- DirectInput 8 interface／device identity、data format、cooperative level、Acquire／Unacquire
- GetDeviceState／GetDeviceData、buffer overflow、focus loss／reacquire と DOM replay の一致

### 5.8 Graphics tests

Graphics は三種類の判定を使う。

1. **Exact pixel:** integer blit、format conversion、palette、color key、clip、surface memory。
2. **Semantic state:** DC selected object、dirty region、lock count、lost／restore、flip chain。
3. **Tolerance image:** font substitution、anti-aliasing、GPU backend 差。許容差と対象領域を fixture ごとに明記する。

GDI fixture は DIB orientation、1／4／8 bit palette の必要範囲、RGB555／565、24／32 bit、BitBlt／StretchBlt、ROP、clip、text baseline を含む。

GDI+ fixture は startup／shutdown、bitmap file／DIB create、pixel format、LockBits／UnlockBits、load／save の primary-reachable subset を含む。

DirectDraw fixture は primary／offscreen surface、pitch、lock rectangle、wrap／nested lock policy、blit、color key、palette update、flip chain、cooperative level、logical mode switch、surface loss／restore を含む。

D3D9 fixture は COM identity、CreateDevice／presentation parameters、target-required resource／format、Lock／Unlock／pitch、fixed-function state、vertex format、blend／sampler、draw、Present、Reset、cooperative state、context／device loss を含む。method／format／state coverage は [corpus-inventory.md](corpus-inventory.md) から派生する versioned call manifest と比較する。

Browser E2E では resize、DPR change、fullscreen、tab hide／show、graphics context loss を注入する。

### 5.9 Audio tests

Offline test は mixer output を buffer として回収し、次を比較する。

- PCM format と endian／signedness
- volume、pan、frequency／resample
- buffer lock の wrap-around
- play／stop／resume、loop boundary
- play／write cursor と notification order
- multiple secondary buffer mix と clipping

Realtime test は AudioWorklet を実際に動かし、queue depth、underrun／overrun、sample count、A/V drift、main thread responsiveness を測る。user activation 前、activation 後、AudioContext suspend／resume、background、output device change 相当を含む。

primary scenario で到達する `PlaySoundA`／`mciSendCommandA` は command、flags、callback／notification、state transition、format を Windows oracle と比較する。未到達の MCI command や video codec をこの結果から対応済みとしない。

audible correctness を CI の virtual audio device だけで断定しない。定期的な実機確認を acceptance evidence に含める。

### 5.10 Browser E2E と title scenario

Clean browser profile から次を自動化する。

```text
feature preflight
 -> local fixture import
 -> source hash verification
 -> launch
 -> canvas first frame
 -> input replay
 -> audio activation / output counters
 -> save
 -> runtime stop and browser context close
 -> reopen
 -> load
 -> final state / screen / file hash verification
 -> diagnostic export
```

T3 title scenario は同じ harness interface を local run で使用し、scenario ID、runtime／browser version、pass／fail、API coverage、failure class を記録する。

## 6. Fuzzing と robustness tests

### 6.1 Fuzz target

- PE／COFF header、section、data directory、resource
- import／export／relocation／TLS table
- archive／package manifest と path
- x86 decoder と memory operand
- ABI metadata-driven marshaller
- Windows string／structure と length relation
- path／registry key normalization
- image dimensions、pitch、palette、rectangle
- audio format、buffer size、cursor／lock range
- diagnostic decoder／import

### 6.2 Invariant

- runtime out-of-bounds、use-after-free、uncaught panic／exception がない
- allocation、loop、recursion、log size が limit 内
- invalid guest pointer が marshaller を通過しない
- Worker を watchdog で停止できる
- stop／quota error で commit 済み save を壊さない

Fuzzer の単純な実行時間だけで合否を決めない。target 数、exec count、coverage、unique crash、sanitizer／runtime、seed corpus version を evidence に残す。

Phase 1 の `Q-055` で versioned fuzz profile を定める。profile は target／harness version、seed hash、dictionary、minimum exec count または wall time、coverage baseline、sanitizer／runtime mode、memory／timeout limit、zero unresolved reproducible crash を含む。`AC-P2-05` と `AC-P8-01` はこの profile ID を evidence に記録する。

## 7. 性能と信頼性の測定

### 7.1 基準環境

Phase 0 で実際に利用する desktop を primary reference device として定める。記録項目は CPU、memory、OS、browser build、power mode、display refresh、audio sample rate、cross-origin isolation、graphics backend である。追加端末は比較が必要になった時点で加える。

### 7.2 指標

| ID | 指標 | 測定点 | Phase 1 で決める threshold |
|---|---|---|---|
| MET-CPU-01 | guest instructions／second または normalized workload time | CPU-only fixture、cold／warm | title deadline に対する余裕 |
| MET-START-01 | import metadata cached 後の launch から first guest frame | browser E2E | p50／p95 |
| MET-FRAME-01 | present cadence、missed frame、runtime CPU time | representative scene | target FPS ごとの p95／p99 |
| MET-UI-01 | main thread long task、stop latency | browser trace | long-task count、p95 stop latency |
| MET-MEM-01 | committed guest bytes、host heap、peak／growth | 30／120 分 soak | peak と steady growth |
| MET-AUD-01 | underrun／overrun、queue depth、drift | realtime audio | warm-up 後の許容量 |
| MET-SAVE-01 | save commit／reload time、bytes written | small／large save | p95 と write amplification |
| MET-SIZE-01 | initial／lazy bundle size、compile time | clean cache | local deployment に沿う budget |

要件で provisional とした `underrun counter 0`、`A/V drift 50 ms`、`stop 100 ms` は Phase 1-6 の実測と UX 評価で確定または根拠付きで変更する。audible defect は別の実機 listening evidence で判定する。

### 7.3 Soak

- CPU／scheduler vertical slice: 30 分から開始
- graphics／audio integrated fixture: 2 時間
- サクラノ詩 representative flow: 60 分以上
- multi-title／beta soak: Deferred phase を再開した場合に再定義

steady-state memory、handle、surface、audio node、pending request、log buffer が単調増加しないことを確認する。

## 8. CI と再現性

CI は実装済み checkpoint に合わせて段階的に広げる。長期的な候補は次のとおりであり、未実装 subsystem の job を先に作らない。

1. format／lint／type／generated diff
2. unit／property tests
3. PE／CPU／ABI／API contract tests
4. fixture build と reproducibility check
5. Wasm／browser application build
6. headless browser E2E on Tier 1 browsers
7. deterministic replay と golden diff
8. selected fuzz smoke と dependency integrity scan
9. benchmark smoke と baseline comparison
10. artifact manifest／SBOM

長時間 fuzz、全 browser matrix、Windows oracle、T3 title、2-8 時間 soak は scheduled／local job として分離する。

Flaky test は retry で成功扱いにせず、owner、issue、隔離期限を持つ。互換性、save、runtime robustness の test は原則として quarantine 不可とする。

Phase gate の required test は skip 0 を原則とする。browser／device feature により実行不能な場合は、その環境を該当 support Tier から外すか、AC 自体を scope change で改訂する。`skip` の記録だけで合格にしない。`adopted suite`、精度、threshold は suite ID／version、reference device／browser set、decision record ID を evidence manifest に固定する。

## 9. Acceptance evidence

各 gate は次を一つの evidence manifest にまとめる。

- source commit と dirty state
- toolchain／dependency lock hash
- generated API／fixture schema version
- fixture ID、hash
- browser／OS／reference device
- exact command と configuration
- pass／fail／skip 数と skip 理由
- compatibility matrix diff
- benchmark と baseline diff
- fuzz／soak summary
- known issue、accepted deviation、approver

手動確認しかできない項目は、手順、expected result、実施者 role、日時、runtime version を残す。

## 10. Phase 別受け入れ条件

以下は最低条件であり、[roadmap.md](roadmap.md) の各 phase task と組み合わせて使う。

### AC-P0: Foundation ready

- `AC-P0-01`: personal target、active／deferred scope、非対象、互換性の表現を単一 maintainer が review し Accepted にしている
- `AC-P0-02`: canonical command、decision record、backup、dependency update policy が決定済み
- `AC-P0-03`: pin 済み toolchain と lockfile により fresh clone から canonical command で build／test できる
- `AC-P0-04`: CI が format、lint、unit、browser smoke、dependency integrity check を main branch で通す
- `AC-P0-05`: T0／T1 fixture、fingerprint 済み T3 corpus、サクラノ詩 critical scenario／checkpoint、Windows oracle 手順が記録済み
- `AC-P0-06`: resource limit、browser Tier 候補、primary reference desktop、測定指標、D3D9／DirectInput dynamic inventory が定義済み
- `AC-P0-07`: ADR-001、002、003、004、006、011 が Accepted。ADR-005 と ADR-009 は候補、測定方法、Phase 1 の決定期限が明示済み
- `AC-P0-08`: 全 Must requirement が少なくとも一つの owner task と test／AC へ到達する完全展開 traceability manifest を機械検査できる
- `AC-P0-09`: pin 済み Markdown check が採用した CJK／table policy で通り、文書 link／ID／fence／table 構造を検査する

### AC-P1: Execution feasibility proven

- `AC-P1-01`: sparse guest memory が low／high address、reserve／commit／protect、cross-page fault を fixture どおり扱う
- `AC-P1-02`: selected x86 baseline が採用 ISA test set と compiler fixture を定義済み精度で通し、privileged／I/O／direct-system-call 経路を guest fault または明示的 unsupported として返す
- `AC-P1-03`: cdecl／stdcall／thiscall、採用 fastcall、host API trap、guest callback、nested callback で stack sentinel が保たれる
- `AC-P1-04`: Runtime Worker 上で periodic yield、cancel、watchdog、thread wait／wake の prototype が main thread を block しない
- `AC-P1-05`: sync guest API から async host response への suspend／resume が timeout、cancel、reentrancy test を通す
- `AC-P1-06`: baseline または bounded optimization prototype の representative CPU／memory／callback workload が、versioned reference devices／Tier 1 browsers で Phase 0 の go threshold を実測で満たす
- `AC-P1-07`: 30 分 stress で silent state divergence／unrecoverable UI stall がなく、memory、pending request、queue の peak と steady-state slope が versioned threshold 内にある
- `AC-P1-08`: no-go 条件を review し、次 phase へ進む判断を ADR／milestone record に残す
- `AC-P1-09`: API metadata、thunk、callback、marshaller の生成方式を定めた ADR-005 が Accepted
- `AC-P1-10`: controlled T1 image が guest instruction -> API trap -> host service -> guest callback -> yield を通り、test 用 pixel command、audio pulse、persistent byte write の expected evidence を返す

現在のproduction memory／entry／return／stack／minimum TEB・PEB／process-parameters／bidirectional binary memory sync／BGI firstからseventeenth-call ingress、controlled Chrome Worker、version 1 browser performance gateは`AC-P1-01`～`AC-P1-08`、`AC-P1-10`へのpartial evidenceである。controlled CPUはprimary titleの17 API return、two pointer tokens、one observed guest storeまで進み、public fixtureはbrowser callback serviceとpixel／audio／persistence、synthetic CPU／ABI／memory thresholdを通したが、adopted ISA suite、general process／thread／wait lifecycle、broader PEB・TEB、host timeout、guard／heap backing／system-DLL image／snapshot、title browser workloadを欠くため各criterionと`AC-P1`全体は未完了である。

### AC-P2: Headless PE runtime complete

- `AC-P2-01`: valid PE32 EXE／DLL fixture の header、section、relocation、import by name／ordinal、forwarder、resource、TLS、必要な delay import を処理する
- `AC-P2-02`: TLS callback と DLL attach が entry point より前かつ期待順で実行される
- `AC-P2-03`: synthetic Kernel32 最小 export を介し、EXE -> DLL -> callback -> `ExitProcess` 相当まで deterministic に完了する
- `AC-P2-04`: dynamic load／lookup／unload、per-thread LastError、heap／virtual memory の最小 contract test が通る
- `AC-P2-05`: malformed PE corpus と loader fuzz で host crash、out-of-bounds、unbounded allocation がない
- `AC-P2-06`: unresolved import は module、name／ordinal、requesting call chain を含む診断を返す
- `AC-P2-07`: PE32+／driver 等の非対象 input を stable unsupported error で拒否する
- `AC-P2-08`: corpus が必要とする API-set／system module alias と manifest／activation context の採用範囲を解決し、未対応 dependency を具体的に診断する
- `AC-P2-09`: adopted compiler／CRT fixture が flat segment／FS、initial stack、PEB／TEB、process parameters、TLS を使って entry point から user code、exit まで進む
- `AC-P2-10`: loader が immutable module object の hash を map 時に検証し、simulated backing source を import 後／delay-load 前に差し替えても実行 bytes が変化しない
- `AC-P2-11`: サクラノ詩の local load checkpoint を実行し、最初の import／bootstrap blocker と architecture assumption の見直し結果を evidence に残す。title の起動成功自体は合格条件にしない

現在のimport-binding／BGI firstからseventeenth-call ingressは`AC-P2-01`、`AC-P2-06`、`AC-P2-11`へのpartial evidenceである。BGI requesting entry chainから最初の17 API return、four ordered dynamic endpoint identities、one logical TLS index／thread value、two encoded-pointer tokens、次のendpoint encoding call identityまで処理するが、general API dispatch、ordinal fixture、export／forwarder／DLL graph、general bootstrap／executionを欠くため各criterionと`AC-P2`全体は未完了である。

### AC-P3: Win32 substrate and persistence complete

- `AC-P3-01`: handle／object、thread、critical section、interlocked、TLS slot／cleanup、event／mutex／semaphore／wait、clock／timer の採用 contract tests が通る
- `AC-P3-02`: User message queue の post／send／peek／get／dispatch、timer、callback の deterministic fixture が通る
- `AC-P3-03`: Windows path、case-insensitive lookup、overlay、sharing の採用範囲を differential test で満たす
- `AC-P3-04`: Windows-932／Unicode の filename、API string、registry value round-trip が通る
- `AC-P3-05`: save と virtual registry を commit、browser context close、reopen、load して同じ state hash を得る
- `AC-P3-06`: commit 中 kill、quota full、permission revoke 後も旧／新いずれかの整合した save を読める
- `AC-P3-08`: IUnknown ABI、vtable thunk、HRESULT、interface identity、reference count の共通 foundation が fixture を通る
- `AC-P3-09`: renderer／surface の ADR-007 と DirectSound／AudioWorklet／clock の ADR-008 が Accepted
- `AC-P3-10`: managed module snapshot を restart 後も同じ hash で参照でき、外部 source replacement、overlay、quota failure で executable／DLL が shadow されない
- `AC-P3-11`: write／flush／close／browser commit の durability point と hard-stop rollback を kill injection で検証し、最後に commit 済みの state を保持する
- `AC-P3-12`: 同じサクラノ詩 fingerprint の thread／message／file／registry checkpoint を実行し、最初の mismatch と scope decision を evidence に残す

### AC-P4: Interactive GDI slice complete

- `AC-P4-01`: VN-like T1／T2 fixture が logical window を作り、message loop、paint、keyboard／mouse 入力へ応答する
- `AC-P4-02`: DC／object lifetime、DIB、palette、BitBlt／StretchBlt、clip、text の採用 suite が semantic／golden 判定を通す
- `AC-P4-03`: CP932 text と採用 font mapping で missing glyph がなく、fixture の glyph advance／baseline／line break／clip が versioned threshold 内にある。超過は partial／unsupported と記録する
- `AC-P4-04`: resize、DPR、fullscreen、focus loss、graphics context loss の採用 browser E2E が復帰する
- `AC-P4-05`: reference scene が frame／main-thread responsiveness budget を満たし、差分 baseline が保存される
- `AC-P4-06`: サクラノ詩の window create から GDI／input checkpoint を実行し、GDI／font／message assumption の差を evidence に残す
- `AC-P4-07`: primary-reachable GDI+ startup、bitmap file／DIB create、LockBits／UnlockBits、load／save subset が semantic／pixel oracle を通る

### AC-P5: DirectDraw comparison complete [Non-blocking]

- `AC-P5-01`: COM ABI の IUnknown／vtable／HRESULT／reference count fixture が通る
- `AC-P5-02`: primary／offscreen surface、8 bit palette、採用 16／24／32 bit format、pitch、lock／unlock が exact memory oracle と一致する
- `AC-P5-03`: blit、color key、clip、flip、cooperative level、logical display mode の adopted suite が通る
- `AC-P5-04`: lost／restore、resize、context loss 後に surface state を再構成する
- `AC-P5-05`: VN-like scene を 60 分実行し、frame miss、memory growth、visual diff が確定 threshold 内である
- `AC-P5-06`: corpus が呼ぶ DirectDraw method が全件 status を持ち、adopted clipper、GetDC／ReleaseDC、mode enumeration、vertical blank／scanline fixture が Windows oracle または明示した unsupported 診断と一致する
- `AC-P5-07`: Nursery Rhyme の DirectDraw create から first present checkpoint を実行し、surface／method／format gap と scope decision を evidence に残す

### AC-P6: DirectSound complete

- `AC-P6-01`: DirectSound interface identity／reference count、device／primary／secondary buffer、PCM format、lock wrap、play／stop／loop の offline oracle が通る
- `AC-P6-02`: volume、pan、frequency、play／write cursor、event-handle notification／thread wake の adopted semantic suite が通る
- `AC-P6-03`: SetCooperativeLevel の valid／invalid／destroyed HWND、focus loss、primary format permission と、別 state machine である browser user activation／suspend／resume を adopted suite どおり扱う
- `AC-P6-04`: integrated graphics／audio fixture の 2 時間試験で warm-up 後 underrun counter が 0、drift と queue depth が versioned threshold 内であり、別途実機 listening evidence に audible defect がない
- `AC-P6-05`: buffer lost／Restore、AudioWorklet failure／device change 相当から復帰するか、title を data loss なく停止する
- `AC-P6-06`: サクラノ詩の DirectSound create から first audio checkpoint を実行し、HWND／cursor／media gap と scope decision を evidence に残す
- `AC-P6-07`: primary scenario で到達する `PlaySoundA`／`mciSendCommandA` command、flags、callback／notification、format が Windows oracle と一致し、未到達 command と区別される

### AC-P6D: Bounded D3D9 and DirectInput complete

- `AC-P6D-01`: 対象 fingerprint／scenario の D3D9 interface、vtable slot、CreateDevice parameters、format、state、draw、Reset／loss call manifest が再取得可能である
- `AC-P6D-02`: IDirect3D9／IDirect3DDevice9／target resource の IUnknown identity、vtable、HRESULT、reference count fixture が通り、未実装 slot は具体的に診断される
- `AC-P6D-03`: target-required device／surface／texture／buffer、format、pitch、Lock／Unlock、upload／readback が semantic／exact memory oracle と一致する
- `AC-P6D-04`: target-required fixed-function transform、render／texture-stage／sampler state、vertex format、blend、draw の reference scene が pixel／semantic golden を通る
- `AC-P6D-05`: synthetic scene とサクラノ詩 first-present checkpoint が selected backend で frame budget を満たし、60 分試験で unbounded resource growth／persistent visual divergence がない
- `AC-P6D-06`: DirectInput 8 の root／keyboard／mouse identity、data format、cooperative level、Acquire、GetDeviceState／GetDeviceData、focus loss／reacquire が Windows／DOM replay fixture と一致する
- `AC-P6D-07`: Reset、resize、focus transition、graphics context／device loss 後に復帰するか、primary runtime を保存状態を壊さず停止する

### AC-P7-DP: サクラノ詩 full-play target

- `AC-P7-DP-01`: fingerprint 済みサクラノ詩で import -> preflight -> boot -> title -> 全シナリオ／選択肢／ending -> User32／DirectInput input -> GDI／GDI+／D3D9 graphics -> DirectSound／到達 WinMM audio -> save -> stop -> reopen -> load が通り、利用者が意図する全編を不自由なく実プレイできる
- `AC-P7-DP-02`: 実行前後の source `.exe`／`.dll` hash が同一で、module は immutable source から load され、executable overlay shadowing／自己更新が拒否される
- `AC-P7-DP-03`: failure 時に最初の unsupported API／instruction／resource と guest call site を特定できる
- `AC-P7-DP-04`: preflight が executable／dependency inventory、既知の unsupported、静的には不明な項目を表示し、通過を compatible 保証として扱わない

### AC-P7-MVP: Multi-title expansion [Deferred]

- `AC-P7-MVP-01`: expansion 開始時に corpus、本数、runtime diversity、critical scenario、停止 budget を明示的に再決定する
- `AC-P7-MVP-02`: title ごとに 60 分以上の representative flow、複数 save slot、browser restart 後の load が通る
- `AC-P7-MVP-03`: API／CPU／PE／browser／title compatibility matrix と既知差分が runtime version に対応している
- `AC-P7-MVP-04`: title-specific code patch なしで動作し、必要差は declarative profile と一般 API fix に限定される
- `AC-P7-MVP-05`: support 外 title／browser／feature を compatible と誤表示しない
- `AC-P7-MVP-06`: 各 title の critical text scene で glyph 欠落、意味を失う clip／overlap、scenario を変える line break がなく、超過 title は partial／unsupported とする

### AC-P8: Public beta release [Deferred]

- `AC-P8-01`: parser／decoder／marshaller／VFS／graphics／audio fuzz gate が確定 budget と coverage を満たし、未解決 critical crash がない
- `AC-P8-02`: 8 時間 soak、browser matrix、performance regression、crash recovery、storage migration が release threshold を満たす
- `AC-P8-03`: resource exhaustion、Worker kill、log flood、storage exhaustion の stress test が停止 threshold と save consistency を満たす
- `AC-P8-04`: SBOM、dependency integrity、artifact scan、reproducible build evidence が揃う
- `AC-P8-05`: supported／unsupported matrix と data backup／delete が公開文書に反映される
- `AC-P8-06`: fresh browser profile の release artifact で end-to-end scenario が通り、rollback 手順と migration rollback が検証済み
- `AC-P8-07`: release approver が evidence manifest と residual risk を確認し、versioned release decision を記録する

## 11. Bug severity と release policy

| Severity | 例 | Release policy |
|---|---|---|
| Critical | 広範な save loss、停止不能な無限実行、再現性のある runtime state 破壊 | release 不可、既存 artifact の停止／撤回を検討 |
| High | supported title が起動不能、frequent save corruption、hang を停止不能 | beta／MVP release 不可 |
| Medium | workaround 可能な描画／音声差、一部 API mismatch、限定 browser failure | known issue と owner／期限があれば判断可能 |
| Low | 診断表示、非本質 UI、文書の軽微な差 | backlog 化可能 |

`supported` と宣言した scenario の regression と source file mutation は severity を一段下げて扱わない。

Critical／High finding は mitigation plan だけで waiver できない。再現と影響の新しい証拠に基づき Medium 以下へ正式に再分類した場合だけ release 判定へ進める。commit 済み save の広範な破損と停止不能な実行は non-waivable とする。

## 12. Definition of Done

個別 task の完了には、該当する次の項目を含める。

- behavior と非対象が API inventory／documentation に反映済み
- 実装境界を代表する automated contract test が 1 本あり、追加 test は確認済み regression または曖昧な semantics に対応している
- Windows oracle または model oracle との差分が説明済み
- trace／metric が必要な操作には observability がある
- guest input の size、time、memory limit がある
- browser support と fallback／unsupported error を確認済み
- CI が canonical command で成功し、flaky retry に依存しない
- acceptance evidence へ fixture／test ID を結び付けた

実装が存在するだけ、または一つの実 title で手動起動できるだけでは Done としない。
