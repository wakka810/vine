# テスト戦略と受け入れ条件

<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [`current-checkpoint.json`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
<!-- END CURRENT CHECKPOINT REFERENCE -->

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 現状: 実装済み範囲、active checkpoint、next stopは上のmanifest参照を正本とする

この文書は、各実装を「動いたように見える」状態から、再現可能な互換性証拠へ進めるための基準を定義する。Repository-wide regressionは`npm run check`で`check:fast`とpublic source-adapter経路の`check:fixture-e2e`を順に実行する。Active checkpointの再現commandはmanifest参照を正本とし、広いadapter checkは`check:adapter`、private BGIは`check:private-corpus`へ分離する。

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

representative raw paging measurementは、fixed PE32／LAA imageを`0xF1000000`へnon-identity mapし、high EIP／stack、R page write、unmapped read／write、cross-page write、protect／remap前後を一つのsource-built v86 runnerで検査する。PFEC `7`／`4`／`6`、second-page fault前のno partial write、explicit TLB invalidationをassertする。同じmetadataをchecked sparse referenceへ与え、execute-denied／unmapped fetchをPFEC相当`21`／`20`として検査する。このhistorical low-level runnerはadapter execute ABIを有効化せず、upstream v86がnon-executable sectionを実行するbaselineもgapとして保持する。

production execution-engine contractは別にadapter ABI version `1`を有効化し、Rust reflectionのexecute metadataをadapter-owned bitmapへ同期する。executable page末尾のone-byte instruction、次のNX pageでのopcode拒否、`mov eax, imm32`がNX pageへ跨ぐ拒否とEIP／EAX不変、両page executable時の成功をactual Wasmで検査する。拒否はreason `3`、PFEC相当`0x15`、denied byteのCR2を持つstructured `execute-protection` stopであり、固定15-byte windowは使わない。

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

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->

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

`npm run check:runtime:entry`は同じlive Rust memoryから6 sealed pagesをversion 1 semantic reflectionとしてsource-built v86へ渡す。2 MiB poolへpage directory／3 tables／6 reflected pages／1 harness stackを置き、state ABI restore、explicit TLB／generated-code clear後だけgeneration `6`をacknowledgeする。Rust normal read／fetch receipt、reflection metadata／content digest、adapter byte-consumption execute gate、entry `0x10001000`、instruction budget `3`、first thunk ID `1`、ESP／return／2 arguments、deterministic reportを1 sequenceで検査する。詳細は[v86-entry-ingress.md](v86-entry-ingress.md)を正とする。

`npm run check:runtime:cdecl`は同じsessionを継続し、offerのordered thunk metadata、exact thunk ID `1`、fixed frame `[return,19,23]`、EAX `42`／ESP `+4`／guest returnへのstate restore、guest側argument cleanup、exact thunk ID `2`、callback return／target／value、sealed executable target pageを1 sequenceで検査する。4 execute stop／2 budget stopと5,501-byte deterministic reportを固定するが、callback frameを構築または実行しない。詳細は[v86-cdecl-ingress.md](v86-cdecl-ingress.md)を正とする。

`npm run check:runtime:callback`はoptional Rust-owned control owner／RX sentinel page、generation `9` reflection、actual invalidation acknowledgement、frame ID `1`、checked writable stack `[sentinel,42]`、executable callback target、exact callback return trap、EAX `43`／ESP `0x006FFFEC`を1 sequenceで検査する。6 execute stop／3 budget stopと7,028-byte deterministic reportを固定するが、frame retirementとouter service returnを行わない。詳細は[v86-callback-return-ingress.md](v86-callback-return-ingress.md)を正とする。

`npm run check:runtime:outer-return`はそのexact stopからframe ID `1`をretireし、outer cdecl return state EAX `43`／ESP `0x006FFFF4`／EIP `0x10001019`を復元する。guest caller cleanup、result `0x10003000 = 43`、entry EAX `0`、final ESP `0x00700000`、unmapped harness return `0xDEADC0DE`のreason `0x10E`／PFEC `4`／CR2を1 sequenceで検査する。9 execute stop／5 budget stopと8,124-byte deterministic reportを固定するが、production-owned entry sentinelやprocess exitを実装しない。詳細は[v86-outer-return-ingress.md](v86-outer-return-ingress.md)を正とする。

`npm run check:runtime:entry-return`は同じcontrol ownerのslot `2`へtyped process-entry sentinel ID `2`／`0x7F010010`を追加し、combined page／7-page reflection digest、generation `9` acknowledgement、initial stack value、exact `#UD` classification、entry EAX `0`、ESP `0x00700000`、fixture result `43`を1 sequenceで検査する。9 execute stop／5 budget stopと8,674-byte deterministic reportを固定し、既存outer modeのunmapped `#PF`も保持するが、production stack／process lifecycleを実装しない。詳細は[v86-entry-return-ingress.md](v86-entry-return-ingress.md)を正とする。

`npm run check:runtime:stack-bootstrap`はRust owner `4`のone-page stack region、RW commit、checked sentinel initialization、page digest、generation `12` seal、8-page reflection、normal-read receiptを検査する。adapter allocationに`harness-stack`がなくowner `4` reflected pageが1件あること、同じ9 execute stop／5 budget stop、entry result／ESP／fixture resultを固定する。9,430-byte reportを検査するが、general reserve／commit、guard／growth、PEB／TEB、v86-originated writebackを実装しない。詳細は[v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md)を正とする。

`npm run check:runtime:bootstrap-fs-first-stop`はimport-free PE32 fixtureをnative実行し、`FS:[0x18]` TEB self／`FS:[0x30]` PEB pointerとTEB内fieldの一致をexit code `0`で検査する。同じfixtureの5 image pages、control、Rust-owned stackをgeneration `9`でreflectし、FS base zeroのfirst instructionがreason `0x10E`、PFEC `4`、CR2 `0x18`、previous EIP `0x10001000`で停止することと5,982-byte reportを固定する。guest TEB／PEBやnonzero FS baseは実装しない。詳細は[v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md)を正とする。

`npm run check:runtime:bootstrap-fs`はtyped TEB／PEB regions、checked zero／pointer initialization、page digest、generation `15`の9-page reflection、FS base restoreを検査する。同じfixtureの4 observation、entry EAX `0`、ESP `0x00700000`、exact process-entry sentinel、9 execute stop／8 budget stopと8,214-byte reportを固定する。process parameters、loader list、TLS、LastErrorは実装しない。詳細は[v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md)を正とする。

`npm run check:runtime:bootstrap-params-first-stop`はseparate import-free fixtureをnative実行し、i386の`PEB+0x10`、`ProcessParameters+0x40`、`UNICODE_STRING.Buffer+0x04`からbounded `bootstrap-params.exe` matchまでをexit code `0`で検査する。同じfixtureをexisting minimum TEB／PEBへ渡し、zero process-parameters pointer、explicit entry result `2`、ESP `0x00700000`、8 execute stop／7 budget stop、8,351-byte reportを固定する。process-parameters region／command-line bytesは実装しない。詳細は[v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md)を正とする。

`npm run check:runtime:bootstrap-params`はtyped owner `6`／`7`、RW／NX commit、PEB `+0x10` pointer、parameters `+0x40`のLength `40`／MaximumLength `42`／Buffer、20 UTF-16 units＋NUL、3 page digests、generation `21`の11-page reflectionを検査する。ack後のnormal read、match index `0`、entry EAX `0`、ESP `0x00700000`、94 execute stop／93 budget stop、10,521-byte reportを固定し、先行3 reportsのbyte identityも保持する。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-first-trap`はprimary fingerprintを前後照合し、explicit 400-page Rust limit／8 MiB protocol limit内で388 pagesをreflectする。generation `24`、reflection digest、owner別page digest、VGA MMIO holeを除外した2 MiB physical placement、PDE indices `[1,64,508,511]`、actual invalidation acknowledgementを検査する。entryから19命令でexact thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`へ到達し、EIP／ESP／return／raw stack wordsを固定する。API dispatch、output write、stdcall return、guest resumeは行わない。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-first-win32-call`は同じprimary fingerprint／388 pagesからID `104`をdeclared endpointとして検査する。fixed clock sample、one-argument stdcall frame、IAT call、RW／NX output、generation `24`／owner `4`の4 KiB stack reconciliation、11 changed bytes、generation `25`のone-page delta、full-page merge、zero cache effects、Rust acknowledgement、VOID EAX-preserve policy、ESP `+8`を1 sequenceで固定する。return後4命令、累計23命令でexact ID `103`／`GetCurrentProcessId`へ停止し、14,540-byte reportのSHA-256を検査する。general writeback、clock、ABI gateway、process identityは実装しない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-second-win32-call`はID `103`をdeclared endpointへ上げ、host PIDに依存しないowner／PID `4096`、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、4 changed bytes、generation／dirty不変、EAX result、ESP `+4`を固定する。return後3命令、累計26命令でexact ID `149`／`GetCurrentThreadId`へ停止し、18,472-byte reportのSHA-256を検査する。general process／handle／thread identityは実装しない。詳細は[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-third-win32-call`はID `149`をdeclared endpointへ上げ、host TIDに依存しないowner／TID `8192`、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、1 changed byte、generation／dirty不変、EAX result、ESP `+4`を固定する。return後3命令、累計29命令でexact ID `13`／`GetTickCount`へ停止し、22,383-byte reportのSHA-256を検査する。general process／thread／TEB client-ID／scheduler modelは実装しない。詳細は[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-fourth-win32-call`はID `13`をdeclared endpointへ上げ、host uptimeに依存しないowner／tick count `3,600,000`、paired wall-clock sample、zero-argument stdcall frame、IAT call、generation `25`のstack reconciliation、1 changed byte、generation／dirty不変、EAX result、ESP `+4`を固定する。return後5命令、累計34命令でexact ID `157`／`QueryPerformanceCounter`へ停止し、26,558-byte reportのSHA-256を検査する。general monotonic clock／timer／scheduler／snapshot modelは実装しない。詳細は[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-fifth-win32-call`はID `157`をdeclared endpointへ上げ、controlled counter `12,345,678,901`、one-argument stdcall frame、IAT call、generation `25`のstack reconciliation、5 changed bytes、8-byte output、generation `26` delta／ack、BOOL result、ESP `+8`を固定する。return後44命令、累計78命令でexact ID `83`／`GetStartupInfoW`へ停止し、31,427-byte reportのSHA-256を検査する。counter frequency／origin、general clock／timer／scheduler modelは実装しない。詳細は[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-sixth-win32-call`はfocused native traceでtarget callerの68-byte x86 layoutとfull overwriteを測定し、ID `83`をdeclared endpointへ上げる。controlled process-startup owner／profile、one-argument stdcall frame、IAT call、generation `26`のstack reconciliation、31 changed bytes、68-byte output、generation `27` delta／ack、VOID EAX-preserve policy、ESP `+8`を固定する。return後9命令、累計87命令でexact ID `84`／`HeapSetInformation`へ停止し、39,031-byte reportのSHA-256を検査する。general process creation、desktop、title path、console／standard handlesは実装しない。詳細は[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-seventh-win32-call`はfocused native traceでtarget callerのarguments `[NULL,1,NULL,0]`、BOOL `1`、LastError unchangedを測定し、ID `84`をdeclared endpointへ上げる。controlled process heap-policy owner／ID、four-argument stdcall frame、IAT call、generation `27`のstack reconciliation、12 changed bytes、generation／dirty不変、BOOL result、ESP `+20`を固定する。return後21命令、累計108命令でexact ID `102`／`HeapCreate`へ停止し、43,980-byte reportのSHA-256を検査する。general heap allocator、corruption detection／termination、per-thread LastErrorは実装しない。詳細は[v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-eighth-win32-call`はfocused heap-lifecycle traceで`HeapCreate(0,0x1000,0)`、nonzero native result、LastError `0 -> 0`、first `HeapAlloc(createdHeap,HEAP_ZERO_MEMORY,0x214)` consumerを測定し、ID `102`をdeclared endpointへ上げる。Rustはthree-argument stdcall frame／IAT call、generation `27` stack reconciliation、8 changed bytesを検査し、namespace `vine.logical-private-heap.v1`のtyped handle `0x68010001`とbounded metadata-only heap stateを返す。guest writeなしでgeneration／dirtyを保持し、14命令後、累計122命令でexact ID `72`／`GetModuleHandleW`へ停止する。49,699-byte reportのSHA-256を検査し、native pointer、heap backing、allocation semantics、general loader modelは実装しない。詳細は[v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-ninth-win32-call`はfocused module-handle traceで`GetModuleHandleW(L"KERNEL32.DLL")`、native HMODULE、LastError `87 -> 87`、immediate `GetProcAddress(...,"FlsAlloc")` consumerを測定し、ID `72`をdeclared endpointへ上げる。Rustはone-argument stdcall frame／IAT call、64-code-unit bounded UTF-16 read、case-insensitive canonical module identity、generation `27` stack reconciliation、9 changed bytesを検査し、namespace `vine.logical-loaded-module.v1`のtyped handle `0x4D010001`を返す。guest writeなしでgeneration／dirtyを保持し、9命令後、累計131命令でexact ID `150`／`GetProcAddress`へ停止する。56,729-byte reportのSHA-256を検査し、native HMODULE、guest system-DLL image、general loader／export modelは実装しない。詳細は[v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-tenth-win32-call`はfocused dynamic-export traceで4件のFLS lookup、native function addresses、LastError `87 -> 87`を測定し、ID `150`をdeclared endpointへ上げる。Rustはlogical KERNEL32 handle、two-argument stdcall frame、14-byte register-indirect IAT provenance、64-byte bounded ASCII／ordinal discriminationを検査する。separate owner／arenaのRX endpoint ID `1`／`0x7F020000`へ`kernel32.dll!FlsAlloc` identityを保持し、host pointerを返さない。tenth modeは389 pages／initial generation `27`、post-write generation `30`となり、15 changed stack bytesをreconcileする。5命令後、累計136命令でsame ID `150`／`FlsGetValue`へ停止する。65,858-byte reportのSHA-256 `54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`を検査し、general export／forwarder resolutionとFLS semanticsは実装しない。詳細は[v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-eleventh-win32-call`はsame native traceの`FlsGetValue` lookupを使い、ordered endpoint ID `1`／`2`をone RX pageへatomic installする。Rustはpersistent ESI call target、13-byte sequence、bounded ASCII nameを検査し、generation `30`のstack／writable image pagesをone transactionでreconcileする。BGI first endpoint store `0x100FEEBC = 0x7F020000`をrequired invariantとし、ID `2`／`0x7F020010`を返す。BGIのsecond store後5命令、累計141命令でsame ID `150`／`FlsSetValue`へ停止する。76,844-byte reportのSHA-256 `40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`を検査し、tenth report hashも不変に保つ。詳細は[v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-twelfth-win32-call`はsame native traceの`FlsSetValue` lookupを使い、ordered endpoint ID `1`～`3`をone RX pageへatomic installする。Rustはthird persistent ESI call、13-byte sequence、bounded ASCII name、generation `30`のstack／writable image pagesを検査する。BGI stores `0x100FEEBC = 0x7F020000`／`0x100FEEC0 = 0x7F020010`をrequired invariantsとし、ID `3`／`0x7F020020`を返す。third store後5命令、累計146命令でsame ID `150`／`FlsFree`へ停止する。89,289-byte reportのSHA-256 `28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`を検査し、eleventh report hashも不変に保つ。詳細は[v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-thirteenth-win32-call`はsame native traceの`FlsFree` lookupを使い、ordered endpoint ID `1`～`4`をone RX pageへatomic installする。Rustはfourth persistent ESI call、bounded ASCII name、generation `30` stack／image reconciliation、first three storesを検査し、ID `4`／`0x7F020030`を返す。fourth store後12命令、累計158命令でstatic unsupported ID `70`／`TlsAlloc`へ停止する。103,179-byte reportのSHA-256 `1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`を検査し、twelfth report hashも不変に保つ。詳細は[v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-fourteenth-win32-call`はtwo focused `tls-lifecycle` tracesで`TlsAlloc()`、native index `0x1C`、LastError preserve、immediate `TlsSetValue` consumerを測定する。ID `70`をdeclared endpointへ上げ、Rustはzero-argument frame／IAT、generation `30`のstack／image reconciliation、all four endpoint storesを検査する。native indexを捨ててnamespace `vine.logical-tls-index.v1`のindex `0`を返し、7命令後、累計165命令でunsupported ID `68`／`TlsSetValue(0,0x7F020010)`へ停止する。110,236-byte reportのSHA-256 `4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`を検査し、thirteenth report hashも不変に保つ。詳細は[v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-fifteenth-win32-call`はsame native evidenceのsuccessful `TlsSetValue` variantだけをID `68`へbindする。Rustはframe `[return,0,0x7F020010]`、live ESI／bound IAT、allocated slot、initial controlled thread、endpoint ID `2`、all four storesを検査する。stack／endpoint-store／TLS-index-storeの3 pagesをgeneration `30`でtransactional reconcileし、thread-value metadataを1件作ってBOOL `1`を返す。9命令後、累計174命令でunsupported ID `89`／`EncodePointer(NULL)`へ停止する。120,499-byte reportのSHA-256 `25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`を検査し、fourteenth report hashも不変に保つ。詳細は[v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-sixteenth-win32-call`はtwo focused `pointer-encoding` tracesの`EncodePointer(NULL)`、varying native result、LastError preserveをoracleとする。ID `89`をdeclared endpointへ上げ、Rustはframe `[0x100A5467,NULL]`、direct IAT call、retained TLS slot／initial-thread valueを検査する。native results／process cookieを捨ててnamespace `vine.logical-encoded-pointer.v1`のtoken `0xEC010001`を返し、stack pageを13 bytes reconcileする。55命令後、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ停止する。129,530-byte reportのSHA-256 `310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`を検査し、fifteenth report hashも不変に保つ。詳細は[v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi -- --checkpoint bgi-seventeenth-win32-call`はsame tracesのsecond `EncodePointer`とstatic disassemblyをoracleとする。Rustはframe `[0x100A9697,0x100A9608]`、11-byte direct IAT call、17-byte executable source prefix、prior encoded-NULL／TLS stateを検査する。native result／cookieを捨ててprocess-initializer function token `0xEC010002`を返し、stack pageを32 bytes reconcileする。BGI global `0x100FF62C`へのstoreを観測し、9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ停止する。capture時の140,660-byte report／SHA-256 `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`との一致はartifact observationとして記録する。詳細は[v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md)を正とする。

上記historical report bytes／SHA-256はcapture artifact identityであり、現在のrunnerは一致を観測するがsemantic hard gateにはしない。各checkpointのexact call／argument／result／write、instruction stop、next endpointがgateである。scheduler orderingは独立laneのinterleaveを許すpartial orderと禁止relationを分け、performanceはsample分布とversioned thresholdで判定する。分類の正本は[evidence-classification.md](evidence-classification.md)とする。

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
dependency-first attach、strict reverse detach、object-table registration failure before commitを1 graphで
固定する。`RuntimeState` contractはexisting typed handleを通す2-module orderを追加する。
これはmodule metadata orderのevidenceであり、DLL image mapping、export／forwarder、TLS／
`DllMain` callback、unloadを実装済みとしない。詳細は
[runtime-module-graph.md](runtime-module-graph.md)を正とする。

process／initial-thread lifecycleの代表Rust contractはbootstrap triple、explicit start、
callback＋pending host requestを持つexit request、request中のprocess／thread nonsignal、
thread／process completion、duplicate／invalid transition時のfull state不変、thread-only
host cancellationを1 sequenceで固定する。whole RuntimeStateをcloneせずcurrent thread／lane／
最大2 wait recordsだけをplanする。これはmulti-thread、last-thread semantics、DLL detach、
TLS cleanup、wait/wakeの代用ではない。詳細は
[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md)を正とする。

controlled thread registryの代表Rust contractはsecond TID／typed handle、duplicate registration時の
full state不変、LastError／TLS／callback／scheduler laneの分離、TLS／FLS／callback allocator ownership、
process／root serialization compatibility、explicit selection、thread-only exit、2 laneのpending work cleanup、
TID昇順thread signal＋process-last signalを固定する。
このcontrolled second-thread fixture単独はCPU context／stack／TEB、guest `CreateThread`の代用ではないが、
別のguest-thread contractがreached `CreateThread`のTID／typed object／stack／TEB／FS／entry frame／suspend-resumeを
全owner preflight後にpublishする。Per-thread stack contractはcurrent scheduler threadだけを対象に、non-present
guard faultをzeroed RW／NX commitとthread-local guard movementへ一括適用し、wrong-thread、present PFEC、stale guard、
unacknowledged memoryをobservable mutation前に拒否する。Node／ChromeはRustのTID／allocation／ownerをnon-owningに
correlateし、別のWindows stack authorityを持たない。Wait queue、last-thread semantics、DLL thread callback、stack
overflow／SEH deliveryはなお別contractである。詳細は[runtime-thread-registry.md](runtime-thread-registry.md)を正とする。

Scheduler／wait subsystem unit contractsはprivate lane／event／host-request／request ID allocator and
registration／object-index／registration ID allocator collections、mutation-free
start／completion／cancellation／wait-allocation plan、serialized schedulerとroot host-request／wait-ID
compatibility shape、FIFO、duplicate／overflow rejection、process-exitでexcludedとなるwake candidateの
cross-subsystem validationを固定する。Aggregate testはunknown completionとthread／process exitがhost
identityをpartial mutationしないこと、およびwait plan失敗がIDを消費しないことを固定し、inspectionは
immutable getterだけを使う。

Runtime registry subsystem unit contractsはprivate dynamic-endpoint table、encoded-pointer token／source
indexes、policy／ID allocator、mutation-free plan、targeted commit、identity reuse、collision／exhaustionの
mutation-free rejectionを固定する。Aggregate testはprocess address rangeを使うendpoint preflight、
endpoint-to-thread TLS classification、pointer inverse relation、existing root／nested serialization shapeを
固定し、host request authorityをregistryへ移さない。詳細は
[runtime-registries.md](runtime-registries.md)を正とする。

Heap subsystem unit contractsはprivate logical／backing collections／automatic slot allocatorのco-ownership、
mutation-free creation／allocation plan、live-slot skip、exhaustion rejection、explicit creation時のallocator
不変、targeted commitを固定する。Serialization contractは既存`process.heaps`とroot
`nextPrivateHeapSlot` shapeをborrowed compatibility viewとして固定する。Aggregate atomicity testは
object-table conflict時にheap collectionsとslot allocatorが両方不変であることを固定し、GuestMemory
integrationはcollision、allocation、free、destroy、process teardownをtransactionalに通す。

Module subsystem unit contractsはprivate process records／graph／image／import／export collectionとDLL
lifecycle operation ownership、mutation-free registration／lifecycle plan、既存`process.modules`／
`moduleGraph` serialization shape、duplicate／stale-plan rejection、owned recordsだけを通すgraph
operationを固定する。Aggregate atomicity testはtyped-object registration failure時のrecords／graph不変、
invalid sentinel／wrong frame時のlifecycle／callback stack不変、old frame pop＋new frame pushのatomic
replacementを固定する。Import binding contractsはplan／memory／metadata commit ordering、failed memory
work後のretryを維持する。Active DLL operationはgraph／image／binding mutationをsubsystem内で拒否する。

Runtime responsibility checkpoint S13のcontractは、`runtime.rs`がsubsystem宣言／re-exportだけのfacade、
`RuntimeKernel`がaggregate実体、`RuntimeState`がcompatibility aliasであることを固定する。Kernelのdirect
fieldはprocess state、address allocator、private subsystem ownerだけとし、direct scalar allocatorと
subsystem collection mutationを持たない。`RuntimeProcessState`のlifecycle／environment／standard-handle
collectionはprivateで、heap／module／synchronizationはindependent owner、従来の`process` JSONは
`RuntimeProcessView`のimmutable borrowed serializationで維持する。Mutex object planはkernelへopaqueで、
create／open／last close／teardownのstale commitが両ownerをpartial mutationしないことをunit／integration
testで固定する。Heap、module attach、thread exit、wait wakeの各operationはowner-local plan testと
cross-subsystem atomic testを両方要求する。Manifest validatorはこのboundary、representative operation、
compatibility view、saved next stopをcurrent-state invariantとして検査する。

COM foundationの代表Rust fixtureはread-only interface／vtable page、RX `UD2` endpoint、x86 stdcall
decode／callee cleanup、canonical IUnknown identity、supported／unknown IIDのHRESULT、shared
`QueryInterface`／`AddRef`／`Release` count、LastError preserve、final mapping／owner／allocation retirementを
固定する。Malformed stack／`this`／output memory、duplicate install、failed final releaseはCOM state、
guest memory、address allocatorをpartial mutationしない。Unsupported slotはinterface、IID、slot、method、
endpoint、callsite付きで停止する。Browser contractは独立IDの非再利用、immutable decimal-string
association、local releaseがRust refcountを変更しないことを固定する。詳細は
[com-iunknown-foundation.md](com-iunknown-foundation.md)を正とする。

Bounded D3D9 frontendのrepresentative fixtureは、version-neutral COM gatewayからversioned
D3D9 handlerへのdelegationを固定する。Slot 8は2-word／HRESULT ABI、`RuntimeDisplays`由来の
1920×1080 geometry、policy由来の60 Hz／`D3DFMT_X8R8G8B8`、16-byte full overwriteを検証する。
Slot 5は3-word／HRESULT ABI、policy-owned virtual identity、zero-filled 1,100-byte
`D3DADAPTER_IDENTIFIER9`のfull overwriteを検証する。Slot 14は3-word／HRESULT ABI、304-byte
x86 `D3DCAPS9` layout、policy-owned `READ_SCANLINE`／`DYNAMICTEXTURES`／PS 3.0 bandと残りzeroを
検証する。Slot 9は5-word／HRESULT ABIと、adapter `0`／HAL／format `22→22`に対するwindowed／
full-screen両predicateを検証し、zero／nonzero `BOOL`を正規化する。Slot 10は6-word／HRESULT ABIと、
adapter `0`／HAL／adapter format `22`／texture format `22`の
Usage `0`／`0x200` predicateを検証する。五queryともLastError／reference count不変で、invalid
adapter、device type、format、usage、resource type、NULL、read-only output、disabled／malformed
policy、missing／mismatched display authorityはguest memoryとruntime stateをpartial mutationしない。
Invalid identityは`D3DERR_INVALIDCALL`、format band外は`D3DERR_NOTAVAILABLE`となる。Identifierのunknown flagsは
`D3DERR_INVALIDCALL`、`D3DENUM_WHQL_LEVEL`はcertificate discoveryを行わずtyped unsupportedとなる。
Public PE fixtureはnativeのnonempty NUL-terminated identity／required capsとVineのexact virtual
identity／capsを分けてself-checkし、device-type両modeとdynamic texture formatも共通に確認する。
Historical D3D9 Node／Chrome profileは同じ3 predicate completionsと次の`LoadLibraryA` stopを比較し、
active profileはさらにlogical module load／export lookupを完了して`GetClientRect`へ進む。Native
traceはdisplay mode 4 calls、identifier 1 call、device caps 1 call、device type 5 calls、device format
13 callsを各2 runで再現する。Device typeのBGI direct 2／internal 3 calls、device formatのBGI direct
1／`CreateDevice` parented 3／その他internal 9 callsをidentityで分ける。Identifier consumerがHRESULT
signだけを読みhost GPU fieldを消費しないこと、caps consumerがHRESULT signとoffset 8／12／204だけを
読むこと、両predicate callerがHRESULT signを検査することを固定する。
Root query leavesが`RuntimeGraphics`／renderer resourceを生成しないことも固定する。Slot 16は6-word／
HRESULT ABI、56-byte x86 `D3DPRESENT_PARAMETERS`のstrict windowed HAL band、live HWNDからの
800×600 resolution、format 22へのnormalization、12-byte prefixと4-byte device pointerのatomic writeを
検証する。Complete 119-slot `IDirect3DDevice9` catalog、child-held root reference、logical graphics deviceの
private ownership、capacity／stale plan／failure atomicity、final device releaseによるgraphics retirementと
optional root cascadeもunit／aggregate fixtureで固定する。Node／Chromeはsame ordered completion、guest
write、COM identity、logical presentationを比較する。Slot 23は8-word／HRESULT ABI、exact
1024x1024／one-level／format 22に対するusage／pool `0／2`、`0／0`、`0x200／0`のversion-13 allow-list、
three distinct 4-byte texture pointer publications、complete 22-slot `IDirect3DTexture9` catalog、各child-held
device reference、private logical descriptor／indexを検証する。Invalid／unsupported／unwritable／capacity／
stale pathsはguest、COM、graphics stateをpartial mutationせず、failureはidentity allocatorやdevice refcountを
消費しない。Final texture releaseはlogical textureをretireして必要ならdevice／rootへcascadeする。
Slot 18は2-word／HRESULT ABI、level zero、4-byte surface pointer publication、complete 17-slot
`IDirect3DSurface9` catalog、child-held texture reference、private logical descriptor／indexを検証する。
Source textureはversion-13 variant index 0のstatic SYSTEMMEM descriptorに限定し、二DEFAULT variantは
guest／COM／graphics mutation前のtyped unsupportedを検証する。
Repeat取得は同じsurface／IUnknown identityへAddRefし、invalid level／NULL／unwritable／capacity／stale pathは
guest、COM、graphics stateをpartial mutationしない。Final surface releaseはlogical surfaceをretireして
必要ならtexture／device／rootへcascadeする。Surface slot 15／16は1-word／HRESULT ABI、4-byte HDC writeと
exact surface／HDC／thread release relationを検証する。Repeat acquisition、wrong／repeated releaseは
`D3DERR_INVALIDCALL`かつfailure-atomicで、live HDC中のsurface COM retirementを拒否する。Reached
`GetStockObject(4)`／`FillRect`はtyped graphics-surface HDCを通り、GDI ownerとGraphics ownerのplanを両方
preflightして`1024x1024` surfaceへ`[0,0,800,600]`／COLORREF 0のbounded commandを一件だけcommitする。
Node／Chromeはsame ordered surface DC／fill／releaseと続く二texture creationを比較する。Device slot 26は
6-word／HRESULT ABI、exact 112-byte／`D3DUSAGE_WRITEONLY`／FVF `0x144`／`D3DPOOL_MANAGED`／NULL shared
handle、complete 14-slot `IDirect3DVertexBuffer9`、child-held device reference、private logical descriptor／
backing relationを検証する。Createのguest pointer、COM identity、graphics ID、address allocation、zero-filled
read-only memoryは全ownerのcapacity／identity／spanをpreflightして一transactionでpublishする。Slot 11は
4-word／HRESULT ABIとfull-range `Lock(0,0,...,0)`だけを許可し、4-byte backing pointer writeとread／write
protectionをatomicにcommitする。Slot 12はzero-word／HRESULT ABIでactive lockを要求し、read-only protection、
lock clear、content generation incrementをatomicにcommitする。Duplicate／unsupported lock、unlock without
lock、unwritable output、COM／graphics／address／memory exhaustion、stale planは全authorityをpartial mutationせず、
final Releaseはresource mapping／owner／allocation／graphics associationをCOM backingと共にretireする。
Node／Chromeは三completion、112-byte content、device refcount 5、backing protection、device slot 57
`SetRenderState` stopを比較し、browser renderer、texture／surface pixel storage、GPU vertex bufferが未生成で
あることを要求する。
Reached `SetRenderState`はtwo-word／HRESULT ABI、exact order
`(22,1)／(7,0)／(137,0)／(27,1)`、zero guest writes、device refcount不変、LastError preserveを固定する。
`RuntimeGraphicsDevice`はreached valueだけをtyped stateとして所有し、Windows default全体を推測しない。
Plan／preflight／commitはstale、wrong device、generation exhaustion、unsupported tupleでstateとgenerationを
partial mutationせず、same-value reapplyはsuccess／zero generation deltaとする。Node／Chromeは四completion、
call digest、logical values、browser-owned render-state tableとrendererが未生成であることを比較する。続くslot 19
`GetRasterStatus`はtwo-word／HRESULT ABI、swap chain 0、exact eight-byte output
`0000000037040000`、LastError／device refcount／logical state不変を固定する。NULL／invalid swap chainは
`D3DERR_INVALIDCALL`、unwritable output、disabled policy、wrong／retired COM-to-graphics relation、
display-authority mismatchはguest bytesと全authorityをpartial mutationしない。Native focused evidenceは
非VBlank scanline 1200／189／834とVBlank／scanline 0、およびwrapperがnative lineをdisplay maxへclampし、
caller RVA `0x5B730`がline wrapまたは`ceil(max / 2)`到達でreturnすることを固定する。旧fixed sample 1を使う
1,024-cycle bounded probeは同じraster／timer-arm／timer-wait sequenceと不変memoryを再現するため、progress
oracleを満たさない。Hostの具体scanlineはartifact observationとしてpolicyへimportせず、version 29はvirtual
1920x1080 displayのlast visible line 1079をstatelessに返す。Node／Chromeはcall 17,866を同じ順序・outputで
完了し、post-sceneではsequence 127,376からinstruction 28,761,426の`Present` slot 17へ進むことを一致させる。
Reached slot 43 `Clear`はsix-word／HRESULT ABI、exact
`(Count=0, pRects=NULL, D3DCLEAR_TARGET, ARGB=0xFF000000, Z=1.0f, Stencil=0)`だけを許可する。
Rust owner testはpristineからuniform clearへのcontent generation、owner-local sequence、full presentation rect、
same-device／live-COM relation、stale／duplicate plan rejectionを固定する。別flags／color／depth／stencil、partial rect、
disabled policy、wrong／retired relationはguest bytes、COM、graphics、allocator、generationをpartial mutationしない。
Node／Chromeはdevice 3、COM object 21、presentation `1280x720`、back-buffer generation／sequence `1／1`、
device generation 44、zero guest writes、renderer resourceなしを一致させる。
Reached `QueryPerformanceFrequency`／progressing `QueryPerformanceCounter`はone-pointer／BOOL ABI、
required writable eight-byte `LARGE_INTEGER`、LastError preserveを固定する。Frequency queryは
process-owned nonzero valueをmutation-freeで返し、counter queryは`RuntimeProcessState`の
lifecycle-bound plan／stale-check／targeted commitでcurrent valueを返してnext valueだけを進める。
Dispatcher／marshaller fixtureは10 MHz output、NULL／read-only failure、1 ms policy quantum、連続sample、
policy-disabled historical fixed modeを検査し、owner modelはzero increment、signed range exhaustion、
stale lifecycle／counter planのfailure atomicityを固定する。Focused native evidenceはfrequencyが
3 sentinel callとtwo runsで10 MHzに安定し、counterが各runでstrictly increasingし、BGI callerが
frequency failure時だけ1000／`GetTickCount`へfallbackして成功時はcounterをfrequencyで除算することを
要求する。Node／Chromeはfrequency completion、301 counter completions、全outputの10,000-tick progression、
same semantic digest／memory／`GetMonitorInfoA` stop、clean cancel／watchdogを一致させる。Host QPC、
browser clock、Worker cadence、WinMM resolution、frame／audio clockをimportしない。
このgatewayはbrowser schedulingに到達するため、同じ180-second／17-threshold workloadの全通過もacceptance
条件とする。Performance-frequency stopの履歴runはSHA固定Chrome for Testingのvalid environmentで127 sessions、
semantic failure 0、heartbeat P99 6.055 ms／maximum 36.075 ms、slice P99 1.675 msを記録して17／17を通過した。
Heartbeat 25 ms、live heap／slope／backing storage、96 MiB capacityを含むversioned thresholdsは変更していない。

Reached `GetMonitorInfoA`はtwo-word／BOOL ABIとrequired writable 40-byte `MONITORINFO`を固定する。
`RuntimeDisplays`がlogical HMONITOR、monitor rect、contained work rectを一つのprofile identityとして所有し、
handlerはhandleをsizeより先に検証する。NULL／stale handleはFALSE／1461、live handleのnon-40 sizeは
FALSE／87、40-byte successはmonitor `(0,0)-(1920,1080)`、work `(0,0)-(1920,1032)`、primary flagを
全span preflight後にatomic publishしてLastErrorを保存する。Native 72-byte `MONITORINFOEXA`はoracleだけを
保持し、reached consumerがないためtyped unsupportedとする。Controlled oracleは11 casesでsize
`0／39／40／41／71／72／73`、NULL／invalid handle、failure buffer preservation、validation orderを固定する。
Owner modelはinvalid／non-contained work rect、duplicate configure、stale identityでdisplay stateとallocatorを
変更しない。Node／Chromeはsequence 18,439のexact 40 bytes、続く二metric、18,441-call digest、final memory、
`DirectInput8Create` stopを一致させる。

このcurrent workloadの180-second gateは、先行二runがsemantic failure 0かつ16／17 passでheartbeat P99だけ
33.480／33.865 msとなったため、failure report、environment qualification、Node三回A/B、authority／threshold
invariant、再開条件をstructured feasibility blockerへ保存した。その後、明示的にquiescentなhostで同じ
configuration、pinned Chrome、runtime artifact、17 thresholdsを変更せず再実行し、127 sessions、heartbeat P99
6.065 ms、17／17 passを得た。Accepted reportは2,033,566 bytes／SHA-256
`151750896f8b7e874acb67d3d30334e6b812fb4eb64efd28ebce39d445078b0d`であり、blockerを解除して
`DirectInput8Create`へ進む。先行failureはhistorical diagnosticsとして保持し、threshold変更や単純retry passの
根拠にはしない。

Reached `DirectInput8Create`はfive-word／HRESULT ABI、version `0x0800`、NULL outer、required writable
four-byte output、`IID_IDirectInput8A`／`IID_IUnknown`を固定する。NULL／read-only output、unknown IID、old／new
version、aggregation、COM capacity exhaustionはconcrete HRESULTでoutputとCOM allocator／identity／mapping／
generationをpartial mutationしない。Successは`RuntimeCom`のcanonical IUnknownと`IDirectInput8A` identity、
refcount 1、8 KiB backing、exact 11-slot vtableをguest outputと同じtransactionでpublishする。BrowserはCOM
stateを所有しない。Native non-NULL aggregationのS_OK／NULL outputはsilent successとして模倣せず、
`CLASS_E_NOAGGREGATION`をmutation前に返すbounded policy差を明示する。

Focused native two-run evidenceはfactory caller、exact arguments／IID、S_OK／non-NULL output、HRESULT consumerと、
直後のslot 4 `EnumDevices(4, callback, 0, 1)`を固定する。Controlled COFF-i386 oracleはvalid two-IID、unknown
IID、version `0x0700／0x0800／0x0900`、failure output clearingを検査する。Rust dispatcher testsはidentity／
vtable／Release retirement、all negative variants、IUnknown publication、resource exhaustion、catalog ABIを固定する。
Node／Chromeはsequence 18,442、root pointer／object／interface relation、atomic host delta、18,442-call digest、
final memory、`IDirectInput8A::EnumDevices` COM stop、clean cancel／watchdogを一致させる。

Current DirectInput root workloadの180-second gateは、同じversioned 17 thresholdsを変更せずvalid environmentで
128 sessions、semantic failure 0、session P95 1,445.050 ms、slice P99 2.170 ms、heartbeat P99 6.050 ms、
17／17 passを得た。Accepted reportは2,037,763 bytes／SHA-256
`a0c09c440c6cb66fab15cf4340a4588a73dabdef8d22d16c5aa054dfde065b0a`である。これはroot factory／COM
identityの成立性を受け入れるが、enumeration callback、device identity、acquire／state、DOM inputをclaimしない。

Reached `IDirectInput8A::EnumDevices`はfour-explicit-word COM ABI、live root identity、device class 4、non-NULL
executable callback、reference 0、flags 1を固定する。Focused native two-run evidenceは両runでS_OK／callback 0、
callerが空listを走査して成功することを確認する。Controlled COFF-i386 oracleはprimary class 4／attached-onlyが
callback 0、keyboard／pointer／all／device classが各callback 1、callback contextと580-byte
`DIDEVICEINSTANCEA`が渡ること、NULL callback／invalid class／invalid flagsが`E_INVALIDARG`かつcallback 0で
LastErrorを保存することを固定する。

Runtime policy version 1はexact primary queryのdevice count 0だけを許可する。Reached completionはS_OK、guest write
0、callback scheduling 0、COM refcount 1 unchanged、LastError／memory generation unchangedである。Invalid inputは
`E_INVALIDARG`、valid unreached variantはstructured unsupported、nonzero-device policyはvalidation faultとなり、いずれも
guest bytes、COM／input state、allocatorをpartial mutationしない。Node／Chromeはsequence 18,443を同じsemanticsで
完了し、その後のheap pathを含む18,452 calls、instruction 11,921,739、semantic SHA-256
`c4408e6ddb87129787656cb709d07081b59edfb7517aa15791b003d0a065214e`、final memory SHA-256
`07630b9b0a45bb1b97d2159e51bb9c635596ce8ff4067dc70370a7864ea78e23`、次の`DSOUND.dll` ordinal 11
(`DirectSoundCreate8`) stop、clean cancel／watchdogを一致させる。

Current enumeration workloadの180-second gateは127 sessions、semantic failure 0、session P95 1,458.825 ms、slice
P99 2.125 ms、heartbeat P99 6.060 ms、valid environment、17／17 passを得た。新たに到達した1 MiB heap pathで
execution mirrorは18,219,008 bytesとなるため、新fixtureだけに24 MiB ceilingを事前設定し、残る16 thresholdsは
維持した。Accepted reportは1,955,904 bytes／SHA-256
`b9bbd3d6ba8a8e5dbde7876f4913fb361f58e9972ab1d4077d20ddadbbffd507`である。Callback-producing enumeration、
device identity／create／acquire／state、DOM inputは受け入れていない。

Reached DirectSound initializationは`DirectSoundCreate8`、live top-level windowへのpriority
`SetCooperativeLevel`、primary buffer作成、stereo 44.1 kHz／16-bit PCM `SetFormat`、looping `Play`、
88,200-byte secondary buffer作成、`IDirectSoundBuffer8` QueryInterface／Releaseを順に固定する。Rust
`RuntimeAudio`がdevice／buffer descriptor／format／playback relation、`RuntimeCom`がinterface identityと
refcountを所有し、guest output publicationを含むcross-owner failureをatomicにする。Controlled x86 oracleは
NULL／bad-size／primary-format／secondary-size等のinvalid descriptor、aggregation、primary non-looping play、
secondary SetFormat、Buffer8 identityを検査する。Reached content transferはsecondary full-range
`Lock(0, 88200, ..., 0)`、callerによる全sample write、exact single-segment `Unlock`、looping secondary
`Play(..., DSBPLAY_LOOPING)`を追加する。初回LockはRust-owned address allocationと`GuestMemory` backing、4 output、
audio lock stateを一括publishし、Unlockはread-only protectionとcontent generationを一括commitする。Final
`Release`はCOM／audio／allocation／memory lifetimeをまとめてretireする。

Node／Chrome semantic gateは同じordered calls、Lock output、sample writeback、protection transition、terminal
memory、clean cancel／watchdogを要求し、続く`USER32.dll!SetTimer`でstructured stopする。Native oracleが示す
NULL outputの部分clearや任意segmentを受け入れるUnlockはprimary-reached contractへ一般化せず、Vineはexact
variant外をmutation前に拒否する。Partial／flagged／wrapping Lock、cursor、Stop／Restore、notification、browser
audio transport、audible primary output、WinMM audioはclaimしない。既存180-second observationはDirectSound
root／cooperative-level workloadのartifactで、buffer-content経路のcurrent performance evidenceとして扱わない。

Reached `SetTimer` gateはlive HWND、ID 1、20 ms、mapped executable TIMERPROC、same owner threadをexact profileとして
固定する。Focused native two-run evidenceはreturn 1／LastError preserveと、同じowner threadへの反復
`TIMERPROC(HWND, WM_TIMER, 1, tick)`を確認する。Controlled COFF-i386 oracleはsame-key replacementがtimer IDを
保持してold callbackを止めること、first／repeat `KillTimer`、invalid HWND error 1400、NULL HWND／zero ID allocationを
固定する。Runtime testはregistration／replacement／process teardown、4,096-record capacity rejection、invalid／
wrong-owner／non-executable callbackのfailure atomicityを検査する。Node／Chrome gateはsequence 18,577の登録、
Rust-owned timer snapshot、guest write 0、LastError preserve、同じterminal memory／semantic digestと、続く
`KERNEL32.dll!CreateThread` stopを要求する。Clock expiry、`WM_TIMER`／TIMERPROC delivery、`KillTimer`、NULL HWND、
allocated ID、general intervalsはこのcheckpointのacceptance claimに含めない。

Reached `CreateThread` gateはNULL security attributes、default stack、mapped executable start、opaque parameter、
optional DWORD output、flags `0`／`CREATE_SUSPENDED`を固定する。Focused native two-run evidenceはsuspended
creationのnon-NULL handle／nonzero output TID／LastError preserve、直後の`ResumeThread` previous count 1、workerへ
同じparameterが渡ること、callerがhandleを保存してresume result `0xFFFFFFFF`だけをfailure扱いすることを確認する。
Controlled COFF-i386 oracleはsuspended／immediate start、success LastError preserve、oversized stackのNULL／
ERROR_INVALID_PARAMETER／output preserve、invalid `ResumeThread`の`0xFFFFFFFF`／ERROR_INVALID_HANDLEを固定する。

Rust testsはTID／object／handle、stack reserve／commit／guard、TEB／PEB fields、entry frame、scheduler suspend／resume、
output writeを一つのtransactionとして検査し、unmapped outputとpage quota failureが全allocator／mapping／registryを
不変にすることを要求する。Dynamic entry returnはthread objectをsignalし、stack／TEB mappingとaddress allocationを
releaseし、同じaddressを再利用できなければならない。Node／Chrome gateはsequence 18,598のCreateThread、18,599の
ResumeThread、両CPU context registration、両threadの実行、同じsemantic digest／terminal memoryを要求する。
Synchronous resume自体はcallerを即時preemptせず、別runnable laneとの競合中に64 guest API boundariesが完了すると
Rust schedulerが一回switchする。競合前のAPI履歴はcounterへ含めず、callback中はswitchせず、stale selection planは
mutation前に拒否する。DLL thread notification、TLS／FLS cleanup callback、last-thread process exit、simultaneous guest
execution、instruction／wall-clock preemptionはclaimしない。

Reached `SetThreadPriority` gateは直前に作成したtyped thread handleと
`THREAD_PRIORITY_ABOVE_NORMAL` (`1`)をexact profileとして固定する。Focused native two-run evidenceは同じ
CreateThread handle relation、TRUE、LastError preserve、callerがEAXを読まずframeを復元してreturnすることを要求する。
Controlled COFF-i386 oracleはnormal default、above-normalへの変更とrepeat、normalへの復元、success LastError
preserve、unknown numeric priorityのFALSE／ERROR_INVALID_PARAMETER／state preserve、invalid／closed handleの
FALSE／ERROR_INVALID_HANDLEを固定する。

Rust testsはper-thread isolation、mutation-free plan、stale／duplicate commit rejection、invalid／closed／wrong-kind
handleとpolicy-disabled variantのfailure atomicityを検査する。Node／Chrome gateはsequence 18,600、logical thread
8193のabove-normal state、guest write 0、LastError preserve、同じsemantic digest／terminal memoryと、続く
`USER32.dll!GetDC(NULL)` stopを要求する。Logical priorityはcooperative lane selectionを変更せず、host scheduler class、
browser task priority、pseudo-handle variant、priority boostをclaimしない。

Reached `GetDC(NULL)` gateはconfigured primary monitorとcurrent guest threadへのnon-owning relationを持つdistinct
logical HDCを固定する。Focused native two-run evidenceはnon-NULL screen HDC、直後の同じHDCを使う
`EnumFontFamiliesExA`、`ReleaseDC(NULL, hdc)` TRUE、全success LastError preserveを要求する。In-process oracleは
concurrent二acquisitionがdistinctで、first／second releaseがTRUE、firstのrepeat releaseがFALSE、各callが異なる
LastError sentinelを保存することを固定する。

Rust model／dispatcher testsはnullable HWND ABI、policyとdisplay authority、monotonic identity、typed monitor／rect、
same-thread release、stale／duplicate plan、unknown／repeated release、capacity failureがsurface、allocator、HDC
collection、guest memoryを部分更新しないことを検査する。Node／Chrome gateはsequence 18,608、HDC
`0x47010003`、monitor `0x4f010001`、1920x1080 rect、guest write 0、LastError preserve、renderer resource 0と、
続く`GDI32.dll!EnumFontFamiliesExA` stopを要求する。Screen drawing、font callback／catalog／metrics、host HDC、
DOM screen topology、cross-thread releaseはclaimしない。

Reached `EnumFontFamiliesExA` gateはlive same-thread screen HDC、flags 0、`SHIFTJIS_CHARSET`、Windows-932
`ＭＳ ゴシック` filterをexact profileとして固定する。Native two-run evidenceはprimaryで一callback／outer result 0、
controlled stop／continue oracleで各一callbackとouter result 0／1、LPARAM round-trip、LastError preserve、input／guard
unchangedを要求する。Fixed ANSI fieldは最初のNULまでのsemantic bytesだけを比較し、native未初期化tailのraw equalityを
要求しない。

Rust dispatcher testsは188-byte `ENUMLOGFONTEXA`、96-byte `NEWTEXTMETRICEXA`、font type 4のcanonical payload、
callback result propagation、wrong charset／face／flags、invalid callback、stale／cross-thread HDC、callback scratch failureが
memory、frame allocator、HDC authorityを部分更新しないことを検査する。Node／Chrome gateはsequence 18,609の一callback、
callback result 0、続くsequence 18,610の`ReleaseDC(NULL, hdc)` TRUE、live DC 0、同じsemantic digest／terminal memoryと、
instruction 11,958,023の`GDI32.dll!CreateFontA`到達を要求する。

Reached `CreateFontA` gateは14-word x86 stdcallとexact CP932 requestを固定する。Native two-run evidenceはheight／width
64／32、weight 100、charset 128、output precision 4、quality 2、Windows-932 `ＭＳ ゴシック`、non-NULL result、
LastError preserveと、callerがhandleを保存／nonzero検査して`CreateCompatibleDC(NULL)`へ進むorderingを要求する。
Controlled oracleは`GetObjectA(font, 60)`のexact round-tripとguard不変、select／restore identity、first／repeat
`DeleteObject`のTRUE／FALSE、stale `GetObjectA`のoutput preserve、全success／failure LastError sentinel preserveを固定する。

Rust model／dispatcher testsはdistinct monotonic nonshared identity、exact 60-byte LOGFONTA、owner-local stale／duplicate
plan rejection、NULL／unterminated／cross-page face、policy-disabled／mismatched fields、capacity failureがobject collection、
allocators、guest memoryを部分更新しないことを検査する。Node／Chrome gateはsequence 18,611、handle `0x47020002`、
object ID 2、guest write 0、LastError preserve、browser font resource 0、同じsemantic digest／terminal memoryと、
instruction 11,960,140の`GDI32.dll!CreateCompatibleDC(NULL)` stopを要求する。このfont checkpoint単独では
selection／introspection／deletionをclaimせず、後述のDIB lifecycle checkpointで同じobject authorityへ追加する。

Reached compatible-memory-DC gateはone-word x86 stdcall `CreateCompatibleDC(NULL)`と`DeleteDC` lifecycleを固定する。
Native two-run oracleは三つのdistinct non-NULL HDC、object type 10、共有initial font type 6、共有initial bitmap type 7、
`ReleaseDC(NULL, memoryHdc)`の0／LastError 1425／HDC生存、`DeleteDC` first／repeat／independentの1／0／1、削除後
object type 0、全hook／system resultとLastError sentinelの一致を要求する。到達していない`DeleteObject(HDC)` variantは
nativeではretireするが、本contractのclaimやdispatch経路へ取り込まない。

Rust model／dispatcher testsは初回だけ二つの共有default objectを作ること、後続HDCがidentityを再利用すること、DCと
二objectのcapacity preflight、owner-local stale／duplicate plan rejection、policy-disabled／non-NULL source、unknown／
repeat／wrong-thread／non-memory deletionがcollection、allocator、LastError、guest memoryを部分更新しないことを検査する。
Node／Chrome gateはsequence 18,613、HDC `0x47010004`、DC ID 4、selected font／bitmap ID 3／4、guest write 0、
LastError preserve、browser DC resource 0、同じsemantic digest／terminal memoryと、instruction 11,960,150の
`GDI32.dll!CreateDIBSection` stopを要求する。このcompatible-DC checkpointではpixel storageを作らず、後続のDIB
lifecycleだけがbitmap backingとselection変更を追加する。

Reached DIB-section lifecycle gateはsix-word x86 stdcall `CreateDIBSection`、exact 24-byte DIB `GetObjectA`、
memory-DCのbitmap／font `SelectObject`、restore後のfont／DIB `DeleteObject` contractを固定する。Native two-run
evidenceは40-byte header、128 x -96 top-down 8-bpp `BI_RGB`、256-entry reverse-grayscale RGBQUAD、stride 128、
12,288 initial-zero pixel bytes、create bits pointerと`GetObjectA` bits fieldの一致、select／restore identity、
restore後delete 1／1、stale output preserve、全LastError preserveを要求する。Nativeのselected DIB deleteは1だが
deferred lifetimeをVineへimportせず、selected objectはmutation-freeにrejectする。Deleted HDCでのnative create成功も
primary未到達variantとしてclaimしない。

Rust model／dispatcher testsはHDC、descriptor／palette、GDI object、graphics-arena allocation、memory owner／mapping、
zero backing、guest outputの全preflight後publicationを検査する。Malformed header、unreached palette／format、stale HDC、
capacity／mapping failure、selected delete、stale `GetObjectA`がobject collection、allocator、guest bytes、outputを部分更新
しないことを要求する。Node／Chrome gateはcreate sequence 18,614、DIB handle `0x47020005`、bits `0x44001000`、
allocation ID 59／memory owner 29、4 dirty pages、`GetObjectA` sequence 18,616、select sequence 18,617／18,618、
同じsemantic digest／terminal memoryを要求する。このDIB checkpoint単独ではoutline metrics、text output、glyph
rasterization、host／browser font resolution、一般DIB formatをclaimしない。

Reached outline-text-metrics gateはthree-word x86 stdcall `GetOutlineTextMetricsA`のnullable size queryとbounded
ANSI outputを固定する。Native two-run evidenceはselected 64 x 32 CP932 `ＭＳ ゴシック`でrequired 269 bytes、fixed
212 bytes、family／face／style／full-name offsets 212／226／240／245を要求する。`cbData` 1..212はrequested bytesを
返してpartial outputの`otmSize`をrequested sizeへ合わせ、213..268はfixed 212 bytesだけを書いて0／LastError 1003、
269以上は269 bytesを書いて269を返す。SuccessはLastErrorを保存し、deleted HDCは0／LastError 6、前後guardは不変とする。
Native run間で異なるpadding offsets 57..59／71はsemantic valueとしてimportせず、zero-normalized 269-byte output
SHA-256 `d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d`をpolicy valueとする。

Rust dispatcher testsはlive same-thread memory HDC、selected nonshared FontA identity、exact profile、nullable output、
partial／incomplete／oversized buffer、cross-page output、stale HDC／stock font／disabled policyを検査し、failure時に
guest bytes、LastError以外のGDI state、allocatorを部分更新しないことを要求する。Node／Chrome gateはsequence
18,619／18,621、size queryと269-byte write、HDC `0x47010004`、font `0x47020002`、selection generation 2、same
semantic digest／terminal memoryを要求する。

Reached TextOut raster gateはfive-word x86 stdcall `TextOutA`のexact primary variantを固定する。Native two-run
evidenceはorigin 0／0、Windows-932 byte `0x61`、BOOL 1／LastError 0、zero-filled 128 x 96 top-down 8-bpp planeの
FNV-1a `0x52965dc5 -> 0xe6fc8461`、476 changed bytes、x 3..28／logical y 20..54を要求する。Repeat callと
zero-length callはpixelを変えず、zero-length successはseeded LastErrorを保存し、deleted HDC callは0／LastError 0とする。

Rust dispatcher／model testsはlive same-thread memory HDC、selected FontA／DIB、policy byte、selection／content
generation、expected before-image、writable backingをpreflightし、4,384-byte guest writeとDIB content generationを
failure-atomicにcommitすることを検査する。Node／Chrome gateはsequence 18,623、HDC `0x47010004`、font
`0x47020002`、DIB `0x47020005`、content generation 1、same 18,637-call semantic digest／terminal memory、restore
select sequence 18,624／18,625、DIB／font deletion 18,626／18,627、`DeleteDC` 18,628を要求する。このcheckpointの
次stopはinstruction 12,073,229の`gdiplus.dll!GdiplusStartup`であった。Other glyphs／fonts／positions／colors、general text
rasterization／substitution、host／browser font resource、browser bitmap resourceはclaimしない。

Reached GDI+ startup gateはthree-word x86 stdcall `GdiplusStartup`とone-word `GdiplusShutdown`のsession lifetimeを
固定する。Native two-run evidenceは16-byte input `[version=1, callback=NULL, suppress-background=0,
suppress-codecs=0]`、notification output NULL、initial token zero、Status 0、LastError 0、nonzero tokenを要求する。
Native tokenの数値はrun間で変わるためsemantic valueにせず、wrapperがEAXを分岐に使わず戻り、cleanupが保存tokenを
`GdiplusShutdown`へ渡してglobalをzeroにするconsumer relationだけを固定する。

Rust owner／dispatcher testsはpolicy header、exact input、logical token namespace、distinct session、exact shutdown、
process teardown、capacity、unsupported version、callback／notification variant、bad pointer、stale planを検査する。
Guest token bytesとowner stateは同じpreflight後transactionでpublishし、failure時にsession collection、allocator、
generation、guest bytesを部分更新しない。Node／Chrome gateはsequence 18,638、instruction 12,073,229、Status 0、
address `0x10151F8C`への`01 00 50 47` write、logical session／token ID 1、generation 1、live session 1を要求する。
18,640 callsのsemantic SHA-256は`bc58f9c9dd15a6a98d84597022e571611b24ca008dc7fb6aaf7224b630aa4779`、
terminal memory SHA-256は`dcc54fb1d9776de0ef5f3456cb080abc52955505e0169293546ca57ed1d2d164`で一致する。
次stopはinstruction 12,073,370の`kernel32.dll!VirtualAlloc(NULL, 0x06000000, 0x00102000, 4)`である。
`GdiplusShutdown`のprimary到達、bitmap／codec／LockBits／browser resourceはこのgateでclaimしない。

Reached VirtualAlloc gateはfour-word x86 stdcallのexact reserve-only variantを固定する。Native three-run evidenceは
NULL preferred address、96 MiB、`MEM_RESERVE | MEM_TOP_DOWN`、`PAGE_READWRITE`、non-NULLでrunごとに異なる
result、LastError 0を要求する。`VirtualQuery`相当のobservationはregion全体が`MEM_RESERVE`／`MEM_PRIVATE`、
current protection no-access、allocation protection read-writeであることを固定する。CallerはEAXをglobalへ保存して
returnし、focused trace中のcommit／decommit／releaseは未到達である。

Rust allocator／dispatcher testsはhighest aligned gap、stale plan、policy外variant、owner capacity、GuestMemory
overlapを検査し、allocator ID、owner ID、region、generationを部分更新しないことを要求する。成功はallocation ID 60、
memory owner 30、anonymous range `[0x79000000,0x7F000000)`、generation 36,584→36,585、reserve operation 1、
dirty／payload／physical page 0である。Node／Chrome full evidenceはsequence 18,641のmetadataを含む18,642 callsを
ordered比較する。Summary semantic SHA-256は`66b57e063b689b3bc09bedc200ccc47f6891a433a394db56c4f57abce4277d74`、
terminal memory SHA-256は`bf87be18f36bbe79f7338d2e809b4701b6953e92bda01123dd5990357a3abffa`で一致する。
このVirtualAlloc checkpointの次stopはinstruction 12,111,929の`imm32.dll!ImmGetDefaultIMEWnd(0x48010001)`だった。Commit、preferred-address、
VirtualFree、general protection、native/browser address identityはこのgateでclaimしない。

Reached default IME context gateはsequence 18,643の`ImmGetDefaultIMEWnd`でapp HWNDとはdistinctなthread-logical
IME HWNDを返し、18,644の`SendMessageA(WM_IME_CONTROL, IMC_CLOSESTATUSWINDOW)`でlogical status requestを更新し、
18,645の`ImmAssociateContext(app, NULL)`でprevious default HIMCを返してassociationだけをdetachする。HWND、HIMC、
associationはRust ownerが全capacity／lifecycleをpreflightしてからpublishし、native handle、native message dispatch、
browser resourceを取り込まない。Node／Chromeは18,680 calls、110 import endpoint contracts／108 API symbols、
semantic SHA-256 `6e0d7795057f9ed45a7e380e49758fdc7bf603a777353f74139dc759e0f65dcc`、terminal memory
SHA-256 `ef30be973462cb545e57a0153362a9f7e40bac0ede6c01cf56424958bcedb2c5`で一致した。両runtime threadを実行し、
context switchは1回である。BGI.gdbはgeneration 1のwritable profileとしてreadし、guest writeはまだ到達していない。
次stopはinstruction 11,973,553の`kernel32.dll!SetWaitableTimer(0x57010002, 0x500fff98, 0, 0, 0, 0)`である。
IME composition／candidate UI／DOM text input、cross-thread association、primary save／restart／loadはclaimしない。

COM structured-stop evidenceはRust ABI catalogの`explicitArgumentWords`をboundedなcapture幅とし、return
address、this pointer、全explicit argumentsを保持してprefix一致では済ませない。Native two-run oracleは
zero width／height／formatのresolution、remaining field preserve、non-NULL device、first reached device
method、nested AddRef relationに加え、focused surface callと直後の`GetDC` orderingを固定する。Controlled
i386 oracleはrepeat surface／IUnknown identity、textureとは別のIUnknown、`GetContainer` relation、native
invalid-level output zero化とNULL access violationを固定する。Vineは共通failure-atomicity contractを優先し、
invalid inputでoutputを変更しない。Surface DC oracleはrepeat／reacquire、surface reference-count不変、
wrong／repeated `ReleaseDC`、live DC中の`LockRect`拒否、NULL／read-only output exceptionを固定する。
Focused initial-texture traceは三callのexact caller RVA／arguments／S_OK、same device、distinct non-NULL output、
直後の`CreateVertexBuffer`をtwo-runで固定する。Focused vertex-buffer traceはCreate／Lock／112-byte caller
write／Unlockのexact caller RVA、arguments、initial zero content、written content、S_OK／LastError preserve、
直後の`SetRenderState(22,1)`をtwo-runで固定する。Focused render-state traceは四caller RVA、state／value、
contiguous same-device／thread relation、S_OK／LastError preserveをtwo-runで固定する。Focused raster-status
traceはslot 19、swap chain 0、full output overwrite、nonblank／VBlankのconsumer分岐をtwo-runで固定する。詳細は
[d3d9-frontend.md](d3d9-frontend.md)を正とする。

Reached `LoadLibraryA`のrepresentative contractsはone-word x86 stdcall、nullable logical module
handle、LastError from-handler、bounded ASCII basename readを固定する。Policy fixtureは
`d3dx9_43.dll!D3DXCreateEffect`だけをenableし、NULL／empty 87、missing basename 126、path／
non-ASCII typed unsupportedを独立に検査する。Rust subsystem／aggregate testsはfirst loadのmodule
record／graph／runtime-service and thread references／callback-free attach／typed object publication、repeat
identity、stale plan、object collision、export preflight failureを固定し、失敗時のgraph／allocator／object／
memory不変を検証する。Node／Chromeはsame `0x4D010010` return relationと続くlogical endpoint lookup、
same `GetClientRect` stop／memory digest、clean lifecycle teardownを比較する。Public Worker fixtureと
180-second repeated gateもcore boundaryのregressionとして実行する。Native image mapping、general search、
`DllMain`／TLS callback、unload、D3DX method semanticsはacceptance claimに含めない。詳細は
[runtime-module-graph.md](runtime-module-graph.md)と
[win32-compatibility-policy.md](win32-compatibility-policy.md)を正とする。

Reached COM initialization contractは別fixtureでone-argument x86 stdcall、typed HRESULT、
`S_OK`／`S_FALSE`／`RPC_E_CHANGED_MODE`、LastError preserve、NULL-only reserved policyを固定する。
Thread subsystem testはsame-model counted initialization、model conflictのno-mutation、per-thread isolation、
stale commit、count overflow、balanced uninitialize、duplicate rejection、thread-exit cleanupを固定する。
Node／Chrome primary gateはRust-owned apartment snapshot、same next stop、same memory digestを比較し、
TypeScriptがapartment authorityを持たないことを検証する。Native oracleとnon-claimsは
[bgi-co-initialize.md](bgi-co-initialize.md)を正とする。

Reached system-default language contractは別fixtureでzero-argument x86 stdcall、typed U16
zero-extension、policy-owned `0x0411`、LastError preserve、zero guest write／runtime mutationを
固定する。Policy-disabled、unsupported LANGID、missing locale layerはcompletion前に失敗し、native
EAX upper wordをsemantic valueとしてimportしない。Node／Chrome primary gateはsame AX consumer、
same next stop、same memory digestを比較する。Native oracleとnon-claimsは
[bgi-get-system-default-lang-id.md](bgi-get-system-default-lang-id.md)を正とする。

Reached drive-type contractはprocess-directory owner unit fixtureとdispatcher fixtureで、NULL
current drive、exact root aliases、other drive、non-root／relative／empty、unterminated、policy-disabled、
LastError preserve、zero guest write／runtime mutationを固定する。Policyはscan boundだけを持ち、drive
letter／typeは`RuntimeProcessDirectories`のimage/current identityから導出する。Node／Chrome primary
gateはA–Z 26 calls、virtual `C:\`だけのfixed result、same next stop／memory digestを比較し、host／browser
drive topologyをimportしない。Native oracleとnon-claimsは
[bgi-get-drive-type-a.md](bgi-get-drive-type-a.md)を正とする。

controlled wait queueの代表Rust contractはnonwaitable／duplicate rejection時のfull state不変、
one registration、`WaitingObject` lane、別threadのtwo-stage exit completionまでのnonsignal、exact
signal／runnable wake／one event、already-signaled immediate completion、process-exit cancellationを
1 testで固定する。これはevent／mutex／semaphore、multiple waiters、timeout、wait-any／wait-all、
alertable wait、message wake、guest APIの代用ではない。詳細は
[runtime-wait-queue.md](runtime-wait-queue.md)を正とする。

controlled message queueの代表Rust contractはempty receiveのblock、別threadからのIDs `1,2` enqueue、
first messageだけのwake event、owner FIFO dequeue、reblock後ID `3`によるrewake、exited-target rejection
時のfull state不変、process-exit message discardを1 testで固定する。これはHWND validation、filter、
Peek／Get、WM_QUIT、paint、input、timer、Send／dispatch、guest APIの代用ではない。詳細は
[runtime-message-queue.md](runtime-message-queue.md)を正とする。

Controlled private-heap backingの代表Rust contractはfixed arena collision時のowner／region／runtime
rollback、8-byte aligned IDs `1,2,3`、zero／`0xA5` initialization、`0xDD` free poison、exact
double-free／interior invalid-free rejection、lowest-address reuse、typed heap destroy、exit-request中の
backed／metadata-only heap teardown、final memory generation／ack `13`を1 testで固定する。Logical heapと
backingはprivate `RuntimeHeaps`が同時に所有し、aggregateはcollectionを直接mutateしない。これはguest
`HeapAlloc`／`HeapFree`／`HeapDestroy` dispatch、zero-size／realloc、concurrent allocator、LFH、title
capacity profileの代用ではない。詳細は[runtime-heap-backing.md](runtime-heap-backing.md)を正とする。

Exact 64 MiB private-heap variantの代表Rust contractはprocess-permanent
`[0x40000000,0x44000000)` address record、reserve-only allocation、one-page
transactional materialization、flags-zero `0xA5`／zero-memory `0x00`、new IDでの
slot reuse、free／process teardownを固定する。Second live、duplicate／stale page、outside、
protection、unsynchronized memory、physical quotaはruntime／memory mutation前にrejectする。
Native lifetime oracleは16,384 requested pagesのうち先頭30 pagesだけがwrite-firstでaccessされ、
16,354 pagesがfreeまでuntouchedであることをtwo capturesで再現する。Private BGI Node／Chromeは
1,358 calls、first materialization instruction 50,123、memory digest、instruction 51,614の
`CreateFileA` stopで一致する。Changed memory／Worker boundaryの180-second gateは326 sessions、
failure 0、17 thresholds passedである。これはmultiple／arbitrary large allocations、native pointer／
fill identity、full-play memory profileの代用ではない。詳細は
[bgi-large-heap-allocation.md](bgi-large-heap-allocation.md)を正とする。

Reached ordinary multi-page `HeapFree`のacceptanceは、exact live allocation
identityとlive-count transition、reserved range全体の`0xDD` poison、dirty-only
binary delta、mapping／cache effectなし、`TRUE`／LastError preserve／guest output
なしを一つのtransactionとして検証する。Nodeはpost-ack、Chrome Workerは
pre-ack snapshotを記録するため、absolute generationの一致ではなく同じ
one-generation advance、dirty-page range／owner、payload digest、final memory
content、および明示したack orderingを要求する。詳細は
[bgi-large-ordinary-heap-free.md](bgi-large-ordinary-heap-free.md)を正とする。

Reached private-heap allocation reuseのacceptanceは、adjacentなflags-zero
`HeapAlloc` batchを最初の別endpointまで一体として扱い、fresh allocation ID、
requested／reserved size、live-count progression、small backing reuse、および
直前にfreeした隣接rangeをまたぐordinary multi-page first-fitを検証する。各callは
dirty-only binary delta、mapping／cache effectなし、LastError preserve、guest output
なしでなければならない。Node post-ack／Chrome Worker pre-ack snapshotの違いを明示し、
one-generation advance、page range／owner、payload digest、final memory contentを一致させる。
詳細は[bgi-private-heap-allocation-reuse.md](bgi-private-heap-allocation-reuse.md)を正とする。

Reached FLS allocationの代表Rust contractはstatic／dynamic endpoint ID `1`のcoexistence、
unmapped callback rejection、executable／NULL callback、logical indices `0..127`、capacity
failure時のstate／memory／LastError不変を固定する。private BGI profileはnative index `9`を
破棄してlogical index `0`をreturnし、37 calls／instruction 618の
`HeapAlloc(0x68010001,8,0x214)`まで進む。これはFLS value／free／teardown callbackまたは
guest heap allocationの代用ではない。詳細は
[bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md)を正とする。

Historical reached private-heap allocationの代表Rust contractはthree-argument stdcall、logical heap
handle、schema-v1 `HEAP_ZERO_MEMORY`／exact-size gate、guest pointers `0x20000000`／
`0x20000218`、zeroed first／second blocks、reserve／commitとdirty-only deltas、LastError
preserveを固定する。Frozen v1は`0x800`をstate／memory mutation前にrejectし、extended
contractだけがfirst blockを保持してsecond blockを作る。Base private BGI profileは
19 contracts／39 calls、generation `87`まで進み、instruction 644のdynamic
`FlsSetValue(0,0x20000000)`で停止する。Active schema-v2 regressionは同じhandlerで
flags `0`／`HEAP_ZERO_MEMORY`、ordinary nonzero `1..=16 MiB`、explicit exact 64 MiBを許可し、
非列挙ordinary size `0xBC`のsuccess、32 MiB gap／64 MiB超過／未対応flagのmutation-free
rejectionを固定する。
Zero-size、failure return、other flags、destroy／realloc、FLS value semanticsの代用ではない。詳細は
[bgi-private-heap-allocation.md](bgi-private-heap-allocation.md)を正とする。

Reached FLS valueの代表Rust contractはtwo-argument stdcall、allocated logical index、
nullable opaque value、BOOL return、LastError preserve、two selected threadのvalue isolation、
unknown index rejection時のstate／memory不変を固定する。Private BGI profileは20 contracts／
42 calls、generation `91`まで進み、existing second `GetModuleHandleW`／first
`EnterCriticalSection`を通してinstruction 707の`InterlockedIncrement(0x100F08E0)`で
停止する。これはFLS get／free／callback teardownまたはinterlocked semanticsの代用ではない。
詳細は[bgi-fiber-local-storage-value.md](bgi-fiber-local-storage-value.md)を正とする。

Reached `InterlockedIncrement`の代表Rust contractはrequired aligned `InOut LONG`、
`0 -> 1`／wrapping `-1 -> 0`、one-argument stdcall、LastError preserve、exact one-write、
read-only／unmapped／page-crossing unaligned targetのmutation-free rejectionを固定する。
Private BGI profileは21 contracts／50 calls、generation `106`まで進み、3件のnative-matching
incrementとexisting critical-section／thread／startup callsを通してinstruction 934の
`HeapAlloc(0x68010001,8,0x800)`で停止する。これはadjacent interlocked APIs、cross-worker
atomicity、broad heap allocationの代用ではない。詳細は
[bgi-interlocked-increment.md](bgi-interlocked-increment.md)を正とする。

Reached second private-heap allocationのprivate BGI profileはallocation ID `1`を保持し、
ID `2`／guest `0x20000218`／zeroed `0x800`を作る。Unsealed heap ownerのgeneration
`106 -> 107` deltaはmapping identityとprotectionを保持してexisting v86 pageへ適用される。
21 contracts／51 calls、generation `108`まで進み、instruction 1395の
`GetStdHandle(STD_INPUT_HANDLE)`で停止する。詳細は
[bgi-private-heap-second-allocation.md](bgi-private-heap-second-allocation.md)を正とする。

Reached standard input handleの代表Rust contractはselector `0xFFFFFFF6`、process-owned
namespace `vine.logical-standard-handle.v1`、stable handle `0x53010001`、one-argument
stdcall、LastError preserve、memory不変、output selectorのmutation-free rejectionを固定する。
Private BGI profileは22 contracts／52 calls、generation `109`まで進み、instruction 1403の
`GetFileType(0x53010001)`で停止する。詳細は
[bgi-standard-input-handle.md](bgi-standard-input-handle.md)を正とする。

Reached stdin file typeの代表Rust contractはregistered logical handle lookup、
`FILE_TYPE_CHAR (2)`、one-argument stdcall、LastError preserve、memory不変、unknown logical
handleのmutation-free rejectionを固定する。Private BGI profileは23 contracts／54 calls、
generation `112`まで進み、instruction 1439の`GetStdHandle(STD_OUTPUT_HANDLE)`で停止する。
詳細は[bgi-file-type.md](bgi-file-type.md)を正とする。

Reached stdoutの代表Rust contractはversioned input／output selector profile、process-owned
logical handle `0x53010002`、identity reuse、LastError preserve、guest write 0、stderrと
output file typeのmutation-free rejectionを固定する。Private BGI profileは23 contracts／
55 calls、generation `113`まで進み、instruction 1447の
`GetFileType(0x53010002)`で停止する。詳細は
[bgi-standard-output-handle.md](bgi-standard-output-handle.md)を正とする。

Reached stdout file typeの代表Rust contractはregistered logical stdout
`0x53010002`、`FILE_TYPE_PIPE (3)`、one-argument stdcall、LastError preserve、memory不変、
default policyのmutation-free rejectionを固定する。Private BGI profileは23 contracts／57 calls、
generation `116`まで進み、instruction 1484の`GetStdHandle(STD_ERROR_HANDLE)`で停止する。詳細は
[bgi-standard-output-file-type.md](bgi-standard-output-file-type.md)を正とする。

Reached stderrの代表Rust contractはversioned input／output／error selector profile、
process-owned logical handle `0x53010003`、identity reuse、one-argument stdcall、LastError
preserve、guest write／memory mutation 0、unlisted selector rejectionを固定する。Private BGI
profileは23 contracts／58 calls、generation `117`まで進み、instruction 1492の
`GetFileType(0x53010003)`で停止する。詳細は
[bgi-standard-error-handle.md](bgi-standard-error-handle.md)を正とする。

Reached stderr file typeの代表Rust contractはregistered logical stderr
`0x53010003`、`FILE_TYPE_PIPE (3)`、one-argument stdcall、LastError preserve、
memory不変、default policyのmutation-free rejectionを固定する。Private BGI profileは
23 contracts／60 calls、generation `120`まで進み、instruction 1516の
`SetHandleCount(32)`で停止する。詳細は
[bgi-standard-error-file-type.md](bgi-standard-error-file-type.md)を正とする。

Reached `SetHandleCount`の代表Rust contractはpolicy-enabled count `32`、同値return、
one-argument stdcall、LastError preserve、guest write／runtime／memory mutation 0、default
policyのmutation-free rejectionを固定する。Private BGI profileは24 contracts／61 calls、
generation `121`まで進み、instruction 1526の`GetCommandLineA()`で停止する。詳細は
[bgi-set-handle-count.md](bgi-set-handle-count.md)を正とする。

Reached `GetCommandLineA`の代表Rust contractはexisting UTF-16 process-parameter authorityから
派生したprocess-owned ASCII pointer、two-call stability、distinct A/W relation、zero-argument stdcall、
LastError preserve、call中guest write／runtime／memory mutation 0を固定する。Private BGI profileは
25 contracts／62 calls、generation `122`まで進み、instruction 1536の
`GetEnvironmentStringsW()`で停止する。Chrome Workerもsame calls、memory digest、structured stopを
再現する。詳細は[bgi-command-line-a.md](bgi-command-line-a.md)を正とする。

Reached `GetEnvironmentStringsW`／paired `FreeEnvironmentStringsW`の代表Rust contractは
host environmentをimportしないUTF-16 double-NUL authority、distinct one-copy lease、zero／one-argument
stdcall、LastError preserve、exact-pointer free、concurrent acquisition／foreign／duplicate freeの
mutation-free rejectionを固定する。Private BGI profileは27 contracts／63 calls、generation `123`まで
進み、instruction 1558の`WideCharToMultiByte()`で停止する。BGIはconversion後にfreeするため、
paired freeはpublic semantic fixtureで先に固定する。Chrome Workerもsame calls、memory digest、
structured stopを再現する。詳細は
[bgi-environment-strings-w.md](bgi-environment-strings-w.md)を正とする。

Reached `WideCharToMultiByte`の代表Rust contractはpolicy-owned Windows-932 ACP、positive
UTF-16 count、size query／conversion、system-default substitution／used-default BOOL、complete-unit
partial prefix、LastError preserve／87／122、overlap rejection、unsupported code pageのmutation-free
stopを固定する。Variable destinationはbounded dynamic pointer schemaからdispatcher-owned atomic
writeへ入る。Private BGI profileは28 contracts／64 calls、generation `124`まで進み、instruction
1591の`HeapAlloc(0x68010001,0,1)`で停止する。Chrome Workerもsame calls、memory digest、
structured stopを再現する。詳細は
[bgi-wide-char-to-multi-byte.md](bgi-wide-char-to-multi-byte.md)を正とする。

Reached flags-zero `HeapAlloc`の代表Rust contractは既存
`kernel32.heap-alloc.v2`に対するpolicy pair `(0,1)`、allocation ID `3`、
requested／reserved length `1／8`、deterministic-uninitialized fill、dirty-only
delta、unsupported pairのmutation-free stopを固定する。Private BGI profileは
28 contracts／67 calls、generation `129`まで進み、instruction 1687の
`GetLastError()`で停止する。Chrome Workerもsame calls、memory digest、
structured stopを再現する。詳細は
[bgi-private-heap-size-1.md](bgi-private-heap-size-1.md)を正とする。

Reached `GetLastError`の代表Rust contractはpolicy-independentな
`kernel32.get-last-error.v1`、zero-argument stdcall、selected Rust runtime threadのfull
32-bit LastError return、LastError preserve、guest write／runtime／memory mutation 0を固定する。
Private BGI profileは29 contracts／68 calls、generation `130`まで進み、instruction 1695の
`TlsGetValue(0)`で停止する。Chrome Workerもsame calls、memory digest、structured stopを再現する。
詳細は[bgi-get-last-error.md](bgi-get-last-error.md)を正とする。

Reached `TlsGetValue`の代表Rust contractはpolicy-independentな
`kernel32.tls-get-value.v1`、one-argument stdcall、process-owned allocated-index
validation、selected-thread set／unset value、success LastError `0`、invalid-index
LastError `87`、TLS／guest-memory mutation 0を固定する。Native PE32 fixtureはmain／worker
thread isolationを通す。Private BGI profileは30 contracts／69 calls、generation `131`まで
進み、instruction 1703のdynamic `FlsGetValue(0)`で停止する。Chrome Workerもsame calls、
memory digest、structured stopを再現する。詳細は
[bgi-tls-get-value.md](bgi-tls-get-value.md)を正とする。

Reached `FlsGetValue`の代表Rust contractはpolicy-independentな
`kernel32.fls-get-value.v1`、one-argument stdcall、process-owned allocated-index
validation、selected-thread storage initialization／set／unset value、success LastError `0`、
invalid-index／uninitialized-thread LastError `87`、FLS／guest-memory read mutation 0を固定する。
Native PE32 fixtureはnew-thread uninitialized failureとmain／worker set-value isolationを通す。
Private BGI profileは31 contracts／70 calls、generation `132`まで進み、instruction 1709の
`SetLastError(87)`で停止する。Chrome Workerもsame calls、memory digest、structured stopを再現する。
詳細は[bgi-fls-get-value.md](bgi-fls-get-value.md)を正とする。

Reached `SetLastError`の代表Rust contractはpolicy-independentな
`kernel32.set-last-error.v1`、one-argument VOID stdcall、full 32-bit selected-thread
LastError write、EAX preserve policy、guest write／memory mutation 0を固定する。Native oracleは
`0`／`87`／`122`／`0x12345678`のcall後LastErrorを固定し、process-specific native EAXを
semantic valueから除外する。Private BGI profileは32 contracts／81 calls、generation `145`まで
進み、instruction 1936の`GetACP()`で停止する。Chrome Workerもsame calls、memory digest、
structured stopを再現する。詳細は[bgi-set-last-error.md](bgi-set-last-error.md)を正とする。

Reached `GetACP`の代表Rust contractはpolicy-backedな`kernel32.get-acp.v1`、
zero-argument stdcall、U32 return、LastError preserve、guest write／runtime／memory
mutation 0、policy-disabledのmutation前structured stopを固定する。Native oracleは
active code page `932`とsentinel LastError preserveを固定する。Private BGI profileは
33 contracts／82 calls、generation `146`まで進み、instruction 1978の
`HeapAlloc(0x68010001,0,0x220)`で停止する。Chrome Workerもsame calls、memory digest、
structured stopを再現する。詳細は[bgi-get-acp.md](bgi-get-acp.md)を正とする。

Reached flags-zero size-544 heap allocationの代表Rust contractはexisting
`kernel32.heap-alloc.v2`とversioned policy pairだけを使い、preceding policyでの
mutation-free rejection、allocation ID `4`／guest pointer `0x20000A20`、requested／reserved
length `544`、deterministic-uninitialized fill、LastError preserve、dirty-only deltaを固定する。
Native evidenceはrunごとに異なるinitial fingerprintとRVA `0xA71C5`のfull-buffer copy
consumerを分離する。Private BGI profileは33 contracts／87 calls、generation `152`まで進み、
instruction 4523の`GetCPInfo(932,0x006FF9C0)`で停止する。Chrome Workerもsame calls、
memory digest、structured stopを再現する。詳細は
[bgi-private-heap-size-544.md](bgi-private-heap-size-544.md)を正とする。

Reached `SwapMouseButton`の代表contractは`runtime/input.rs::RuntimeInput`を唯一の
mouse mapping authorityとし、version-2 User32 profileのconfigure-once、zero／nonzero BOOL
decode、previous-state BOOL return、LastError preserve、zero guest write、idempotent
generation、stale plan拒否を固定する。Dispatcher fixtureはinitial false／trueの両方で
`SwapMouseButton(FALSE)`後にfirst resultをsecond argumentへ渡しinitial mappingへ戻ること、
policy-disabled／missing／mismatched ownerがmemoryとinput stateを変えないことを検査する。
Primary evidenceはfocused native 2-call consumer、Node／Chrome memory equivalence、clean
cancel／watchdog、unchanged 17-threshold browser benchmarkを要求する。詳細は
[runtime-input-state.md](runtime-input-state.md)と
[bgi-swap-mouse-button.md](bgi-swap-mouse-button.md)を正とする。

Reached `timeGetDevCaps`の代表contractはversion-1 `WinmmCompatibilityPolicy`を唯一の
virtual timer-capability authorityとし、two-argument x86 stdcall、raw MMRESULT、LastError
preserve、8-byte `TIMECAPS` fixed-width output、query時clock非進行を固定する。Dispatcher
fixtureはsize 7の`TIMERR_NOCANDO(97)`／無書込み、size 8／12のexact 8-byte write、policy
disabled、NULL、truncated、read-only spanのfailure atomicityを検査する。Primary evidenceは
guarded native size `0/4/7/8/12` oracle、BGI consumerがminimumを10と比較して
`timeBeginPeriod(1)`へ進むこと、Node／Chrome memory equivalence、clean cancel／watchdog、
unchanged 17-threshold browser benchmarkを要求する。詳細は
[winmm-compatibility-policy.md](winmm-compatibility-policy.md)と
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md)を正とする。

Reached `timeBeginPeriod`の代表contractは`runtime/timers.rs::RuntimeTimers`を唯一の
timer-resolution request authorityとし、one-argument x86 stdcall、raw MMRESULT、LastError
preserve、zero guest writeを固定する。Owner model testはduplicate begin／matching ends、effective
minimum、stale plan、64-period capacity、generation、process teardownを検査する。Dispatcher fixtureは
out-of-policy beginとunmatched endが`TIMERR_NOCANDO`でruntime／memoryを変えないこと、disabled
policyがstructured faultになることを検査する。Primary evidenceは17-call native pairing oracle、
reached period 1、Node／Chrome timer snapshot／memory equivalence、clean cancel／watchdog、unchanged
17-threshold browser benchmarkを要求する。Browser timer、audio clock、scheduler granularityをrequest
authorityへ昇格させない。詳細は[winmm-compatibility-policy.md](winmm-compatibility-policy.md)と
[bgi-time-begin-period.md](bgi-time-begin-period.md)を正とする。

Reached `timeGetTime`の代表contractはzero-argument x86 stdcall、raw DWORD／U32、
LastError preserve、zero guest writeを固定し、Kernel32 `GetTickCount`と同じ
`RuntimeProcessState` tick plan／commitを使う。Dispatcher fixtureは連続する共有clock sample、
`timeBeginPeriod(1)`からの独立、disabled／zero-increment policyのmutation-free failureを検査し、
process-owner model testがwrapping progressionを固定する。Primary evidenceはLastError sentinel 3種とwrapping
`GetTickCount` bracketを使うtwo-run native oracle、BGIのwrap-aware consumer、Node／Chrome
ordered semantics／memory／`MessageBoxA` stop、clean cancel／watchdog、unchanged
17-threshold browser benchmarkを要求する。Host uptime、browser clock、scheduler／audio clockを
importしない。詳細は[winmm-compatibility-policy.md](winmm-compatibility-policy.md)と
[bgi-time-get-time.md](bgi-time-get-time.md)を正とする。

Heap reuse後に再到達した`timeGetTime` continuationは新しいWindows semanticsとして扱わず、
同じ`winmm.time-get-time.v1` handlerと`RuntimeProcessState` clock authorityを要求する。
Node／Chromeはreturned／next tickとpolicy increment、LastError preserve、zero guest write、
ordered semantic digest、terminal memoryを一致させ、最初の異種owned endpointである
`EnterCriticalSection`のargument／IAT／stack直前で停止する。TypeScript側のclock sampleや
WinMM state複製を禁止し、変動するexact valueは`current-checkpoint.json`だけを正本とする。

Post-clock critical-section continuationは、同一guest addressに対する連続した
`EnterCriticalSection`／`LeaveCriticalSection`を一つのbalanced roundtripとして検査する。
両callの実行thread、EAX／LastError preserve、24-byte locked／unlocked guest state、atomic
publication、ordered semantic digest、terminal memoryをNode／Chromeで一致させ、release後の
scheduler handoffとcall provenanceを混同しない。既存`RuntimeSynchronization`を唯一のauthority
とし、最初の異種owned endpointである`WaitForSingleObjectEx(handle,10,TRUE)`直前で停止する。
Cross-thread blockingやAPC一般化はclaimせず、変動するexact値は`current-checkpoint.json`を正とする。

Post-critical-section `WaitForSingleObjectEx` recurrenceは、既存のfinite alertable
waitable-timer transactionをそのまま再利用し、handle／timer／object／arm／registration／logical
clock／waiting thread／scheduler laneの再検証後にだけexpiration、signal、registration retirement、
`WAIT_OBJECT_0`をcommitする。Node／Chromeはtimer snapshot、zero guest write、LastError、ordered
semantic digest、terminal memory、次の`EnterCriticalSection` endpointを一致させる。Worker pacingは
一件のacknowledgementを追加してpending／stale resourceを残さず、Windows timer、wait result、thread
selectionのauthorityにならない。APC delivery、general timeout、cross-thread waiterはclaimしない。

Post-wait critical-section continuationは、次の異種owned endpointまでに到達する6 callを
3組のbalanced `EnterCriticalSection`／`LeaveCriticalSection` batchとして検査する。各pairは
同一threadと同一guest addressを要求し、EAX／LastError preserve、24-byte locked／unlocked
publication、pair終了時のbalanced state、ordered semantic digest、terminal memoryをNode／Chromeで
一致させる。2個のlogical critical-section identityを`RuntimeSynchronization`だけが所有し、Workerは
completion evidenceだけを保持する。次の`SetWaitableTimer`は6引数frame直前で停止し、このcheckpoint
内ではtimerを再armしない。Cross-thread contention、fairness、abandonmentは引き続きclaimしない。

Post-critical-section `SetWaitableTimer` recurrenceは、既存のrelative one-shot arm transactionを
再利用し、timer ID／logical handle／manual-reset mode／typed-object identity／close policy／open-handle
countを再検証してからtimer payloadと`nonsignaled` wait stateをjoint commitする。Node／Chromeは
BOOL／LastError、zero guest write、stack cleanup、arm generation、logical deadline、ordered semantic
digest、terminal memory、直後の`WaitForSingleObjectEx(handle,10,TRUE)` frameを一致させる。このprofileは
wait dispatch前で停止し、clock advance、expiration、signal、wait result、browser pacing resourceを
claimしない。既存handlerのreachability checkpointなので、新しいperformance baselineは要求しない。

Post-timer-arm `WaitForSingleObjectEx` recurrenceは、同じRust-owned finite alertable wait transactionで
timer／typed object／arm／registration／logical clock／owner thread／scheduler laneをpreflightし、logical
expiration、`signaled` state、registration retirement、`WAIT_OBJECT_0`を一括commitする。Node／Chromeは
arm generation、deadline、advanced clock generation、zero guest write、LastError、ordered semantic digest、
terminal memory、次の`EnterCriticalSection(0x1014c8d0)` frameを一致させる。Workerは一件のversioned pacing
requestをacknowledgeしてpending／stale callbackを残さず、Windows timer、clock、result、thread resumeの
authorityにならない。APC delivery、general timeout、cross-thread waiter、performance baselineは拡張しない。

その直後のcritical-section recurrenceは、異種owned endpointまでの6 callを同じ3組のbalanced batch
contractで再検査する。Node／Chromeはthread `8193`、2個のguest identity、call順序、EAX／LastError
preserve、24-byte locked／unlocked publication、最終balanced state、terminal memory、次の6引数
`SetWaitableTimer` frameを一致させる。Rust `RuntimeSynchronization`だけがWindows-visible stateを所有し、
Workerはnon-owning evidenceだけを保持する。次のtimer arm dispatch前で停止し、contention／fairness／
abandonment／APC semanticsや新しいperformance baselineを拡張しない。

Post-timer-rearm wait直後のcritical-section recurrenceも、同じ6 call／3 roundtrip contractで
再検査する。Node／Chromeはthread `8193`、2個のguest identity、call順序、EAX／LastError preserve、
各callのfailure-atomicな24-byte locked／unlocked publication、最終balanced state、terminal memory、
次の6引数`SetWaitableTimer` frameを一致させる。Rust `RuntimeSynchronization`だけがWindows-visible
stateを所有し、Workerはnon-owning evidenceだけを保持する。timer arm dispatch前で停止し、contention、
fairness、abandonment、APC semanticsや新しいperformance baselineを拡張しない。

その後の`SetWaitableTimer` recurrenceは既存relative one-shot transactionでframeを消費し、timer／typed
object／live generation／lifetimeのpreflight後にlogical deadlineと`nonsignaled` stateをjoint commitする。
Node／ChromeはBOOL／LastError、zero guest write、stack cleanup、ordered semantic digest、terminal memory、
次のalertable wait frameを一致させる。`primary-main-loop-recurring-timer-cycle-batch`は、そのwait completion、
6 call／3 roundtripのbalanced critical-section block、次のrearmを8-callの一単位として完了した。続く
`primary-main-loop-recurring-timer-multi-cycle-batch`は同じscenario unitを16周期のbounded windowへ拡張し、
全wait／rearmのidentity・generation・deadline連続性、48 balanced roundtrip、2周期で発生する
`8193 -> 8192 -> 8193` work interval、合計4 thread transition、terminal memoryをNode／Chromeで一致させる。
Browser pacingは引き続きnon-owning acknowledgementであり、Rustのtimer／wait／scheduler／synchronization
authorityを複製しない。

`primary-main-loop-recurring-timer-window-batch`は同じtimer／wait continuityを64周期、hard maximum 80の
bounded windowで再検査する。Acceptするのは64 wait／64 arm、first-to-last cycle間962 ordered completions、
448 critical-section completions（224 acquire／224 release）、cycle境界をまたぐ10 ownership state、
thread `8192`を含む6周期、14 participant transition、cycle 60後の64-call worker-only interval、balanced terminal
state、terminal memory、次のfinite alertable waitをNode／Chromeが一致させる場合だけである。Workerは
Windows-visible timer／wait／scheduler／synchronization stateを所有せず、pacing callbackはthread、request、
generation、lifetimeを検証してからacknowledgeする。次はtimer windowをさらに広げず、main loopが新しい
presentation workへ進むかをbounded gatewayとして判定する。unbounded loop、APC、general timeout、general
cross-thread contention、readable pixel、long-run安定性はまだclaimしない。Exact値とartifact identityは
`current-checkpoint.json`だけを正本とする。

`primary-main-loop-presentation-progress-gateway`は、同じledgerをhard maximum 256周期まで延長し、
third `Present`へ進んだ場合はその直後のmain timer rearmで停止し、進まない場合はhard boundで
`bounded-window-exhausted`として停止する。今回のacceptanceは後者であり、256 wait／256 matching arm、
3,856 ordered completions、1,792 critical-section completions（896 acquire／896 release）、4個の
inter-cycle interval、balanced terminal state、terminal memoryをNode／Chromeで一致させた。後半では
thread `8192`上の4回の`IDirect3DDevice9::TestCooperativeLevel`、それぞれに対応するtimer 1のarm／wait、
反復するzero-input scan／geometry／cursor／`PeekMessageA`／`IsIconic`を到達したが、third `Present`は
発生しなかった。timer 1とmain-loop timer 2は別laneとして検証し、1回のsynchronous logical waitを
browser pacing countへ混入させない。これはdeadlockや描画不能の証明ではなく、入力を与えないbounded
windowの分類である。次はwindowを再拡張せず、到達済みsteady-state loopへbounded inputを投入して
guest-visibleなinput／presentation progressを判定する。Exact値とartifact identityは
`current-checkpoint.json`だけを正本とする。

Post-timer-wait `SetWaitableTimer` recurrenceは、そのframeを同じrelative one-shot transactionで消費する。
Rustはtimer ID／logical handle／typed-object identity／manual-reset mode／live arm generation／object lifetimeを
preflightしてから、timer payloadと`nonsignaled` wait stateをjoint commitする。Node／ChromeはBOOL／LastError、
zero guest write、stack cleanup、新しいlogical deadline、ordered semantic digest、terminal memory、直後の
`WaitForSingleObjectEx(handle,10,TRUE)` frameを一致させる。WorkerはWindows timer stateを所有せず、この
checkpointではclock advance、expiration、wait result、pacing callbackを発生させない。次のwait recurrence、
exact値、artifact identityは`current-checkpoint.json`を正本とし、新しいperformance baselineは要求しない。

Post-timer-rearm `WaitForSingleObjectEx` recurrenceは、そのlive armを既存のfinite alertable wait
transactionで消費する。Rustはtimer／typed object／arm／registration／logical clock／owner thread／scheduler
laneをpreflightし、expiration、`signaled` state、registration retirement、`WAIT_OBJECT_0`を一括commitする。
Node／Chromeはzero guest write、LastError preserve、ordered semantic digest、terminal memory、次の
`EnterCriticalSection(0x1014c8d0)` frameを一致させる。Workerはversioned pacing requestを一件だけ
acknowledgeし、pending／cancelled／stale callbackを残さず、Windows timer、clock、wait result、thread resumeの
authorityにならない。APC delivery、general timeout、cross-thread waiter、performance baselineは拡張せず、
exact値とartifact identityは`current-checkpoint.json`だけを正本とする。

Reached `GetVersionExA`の代表contractはdynamic in/out pointerのminimum 4-byte
preflight、accepted 148／156-byte full-span read、policy-field merge、single atomic write、
BOOLとhandler-owned LastErrorを固定する。Dispatcher fixtureはbase／extended layout、native-unwritten
padding保持、invalid sizeのFALSE／122／無書込み、policy-disabled、NULL、cross-page truncation、
read-only outputを検査する。Policy testはnon-NT platformとoverlong service-pack labelを拒否する。
Primary evidenceはtwo balanced guarded native traces、BGIのplatform-ID consumer、Node／Chrome same
output／memory／next stop、clean cancel／watchdogを要求する。Host build／browser OSをpolicyへimportしない。
Worker transport、scheduler、memory topology、graphics、audio、lifecycleを変えないsemantic leafなので
performance gateは再実行せず、直前の`winmm-time-begin-period`観測をprofile付き履歴として保持する。
詳細は[bgi-get-version-ex-a.md](bgi-get-version-ex-a.md)を正とする。

Reached `CreateWaitableTimerA`の代表contractはthree-argument x86 stdcall、non-NULL logical
handle、handler-owned LastError 0、guest write zeroを固定する。Native oracleはmanual／auto resetの
unnamed timerが両方create成功し、initial zero-time waitが258、close成功であることと、BGIの
`NULL,TRUE,NULL` caller／direct handle storeをtwo balanced tracesで固定する。Active policyはreached
manual-reset unnamed variantだけを許可し、security attributes／name／auto reset／disabled policyを
mutation前に拒否する。

Subsystem testはtimer ID／capacity／stale plan／retirement、object-table testはwaitable-timer kind／
handle generation／batch teardown、cross-subsystem testはcreation capacity failure、last close、process
exitのpayload／object／handle atomicityを検査する。Dispatcher fixtureはlogical `0x57010001`、timer ID 1、
object ID 20、manual reset、nonsignaled stateとnegative variantsを固定する。Primary evidenceはNode／
Chrome same timer snapshot／memory／`LoadIconA` stop、clean cancel／watchdogを要求する。Object lifetimeと
process teardown boundaryを変更するため3分benchmarkを再実行し、17 thresholds全通過を要求する。
詳細は[bgi-create-waitable-timer-a.md](bgi-create-waitable-timer-a.md)を正とする。

Reached `SetWaitableTimer`のrepresentative contractはsix-argument x86 stdcall、readable signed
`LARGE_INTEGER` input、relative due-time magnitude 1..100,000、period 0、NULL completion routine／context、
resume FALSE、BOOL 1、LastError 0、zero guest writeを固定する。Native oracleはmanual timerの二回waitが
`WAIT_OBJECT_0`、auto timerが`WAIT_OBJECT_0`後に`WAIT_TIMEOUT`、invalid handleがFALSE／
`ERROR_INVALID_HANDLE` 6となることをhook／direct system callの一致で確認する。

Owner testはarm generation、rearm、zero／unknown timer／stale planのmutation-free rejectionを検査する。
Cross-subsystem testはtimer payloadとtyped-object wait stateをjoint preflightし、wrong kind、stale handle、
object preflight failure、policy rejection、unreadable due-timeで両ownerとguest bytesを保持することを要求する。
Dispatcher fixtureはhandle `0x57010002`、timer ID 2、object ID 24、due magnitude 5,000、generation 1、
`nonsignaled` resetを固定する。Node／Chromeはsame ordered call、timer／object snapshot、terminal memory、
`WaitForSingleObjectEx(0x57010002,10,TRUE)` stop、clean cancel／watchdogを一致させる。Host crossing、
browser clock、expiry scan、hot pathを変更しないsemantic leafなので、新しいperformance baselineは要求せず
直前のversioned observationを保持する。詳細は
[bgi-set-waitable-timer.md](bgi-set-waitable-timer.md)を正とする。

Reached `WaitForSingleObjectEx`のrepresentative contractはthree-argument x86 stdcall、timer-only
logical handle、finite timeout 0..10 ms、alertable TRUE、U32 wait result、success／timeout LastError
preserve、invalid handle `WAIT_FAILED`／6を固定する。Native oracleはarmed manual timerの
`WAIT_OBJECT_0`、repeat zero-timeの`WAIT_OBJECT_0`、nonsignaled zero-timeの`WAIT_TIMEOUT`、
sentinel LastError preservation、hook／direct system call一致を要求する。

Owner testはlogical clock monotonicity、exact arm generation expiration、manual／auto reset、
deadline-before-timeout／timeout-before-deadline、zero-time no-op、stale／duplicate plan rejectionを検査する。
Cross-subsystem testはtimer clock、timer payload、typed-object wait state、wait registration、scheduler lane、
host requestを全件preflightし、failure時にcollection／allocator／generationを保持することを要求する。
Positive delayはversion 1の`runtime.timer-delay.v1` payloadへfull-width request／registration／timer／clock／arm
identityを固定し、wrong ack、rearm、close、thread cancel、stale／duplicate completionをmutation-freeに拒否する。
Dispatcher fixtureは
unsupported timeout／alertable mode／object kindをmutation前に拒否し、runtime defectをinvalid-handleへ
maskしない。Primary Node／Chrome evidenceはthree re-arm／wait、clock `5,000/10,000/15,000`、same
semantic digest／memory、three browser pacing completion、normal capability completion、clean cancel／watchdogを
一致させる。Worker service modelはfreezeでremaining delay保持、resume rearm、cancel後late callbackのstale化、
lifecycle generation mismatch、one-per-task catch-upを固定する。Timer service中はsingle emulated CPUをholdし、
Rust completionが指定するowner laneを一度だけ復帰してwall timingによるguest order変化を防ぐ。APC、
`INFINITE`、general wait fairnessはclaimしない。詳細は
[bgi-wait-for-single-object-ex.md](bgi-wait-for-single-object-ex.md)を正とする。

Reached `LoadIconA`のrepresentative loader contractはSHA-256固定のpublic PE32 fixtureから
type／name／language resource treeとsix icon/cursor leavesをbounded parseし、UTF-16 name、malformed offset／
directory band／duplicate identity／count limitを検査する。Mapped-module contractはload-base lookup、
catalog leaf range、duplicate registration、failure atomicityを固定する。

User32 dispatcher contractはinteger resource 101からexact 32×32／highest-bpp DIBを選び、logical
HICON `0x49010001`、success LastError 0、guest write zero、same-resource identity reuse、process teardownを
検査する。Missing resource／invalid module／malformed DIB／system icon／string name／PNG variantは
mutation前に具体的failureとなる。Primary evidenceはtwo balanced native traces、BGI caller／consumer、
Node／Chrome same icon snapshot／memory／`LoadCursorA` stop、clean cancel／watchdogを要求する。
詳細は[bgi-load-icon-a.md](bgi-load-icon-a.md)を正とする。

Reached `LoadCursorA`のrepresentative contractはtwo-argument x86 stdcall、U32 HCURSOR、
handler-owned LastError、guest write zeroを固定する。Version-13 User32 policyは
`NULL,IDC_ARROW`とexact 32×32 bounded module resourceを許可する。System successはLastError preserve、
missing ID 65,535はNULL／1,814とする。Module successはLastError 0、missing／wrong-typeはNULL／1,813、
malformed dataはNULL／1,812、invalid moduleはNULL／6とする。Other predefined、string、disabled、
unmatched-size、PNG variantsはidentity mutation前にstructured faultとなる。

Owner model testはcursor ID／logical handle／source index、system／module repeat identity、icon commit後の
stale cursor plan、exact capacity、allocator、combined process teardownを検査する。Controlled PE fixtureは
group cursor／payload catalog relation、DIB dimensions、hotspot、malformed group／size／hotspotを固定する。
Dispatcher fixtureはlogical `0x43010001`／`0x43010002`、module metadata、二つのLastError policy、memory
generation不変、teardown後のstale handleを検査する。Primary evidenceはtwo balanced native traces、
system／module shared identity、BGIのfour callsitesとconsumer、Node／Chrome same cursor snapshot／memory／
`Direct3DCreate9` stop、clean cancel／watchdogを要求する。Worker protocol、transport、performance boundaryを
変更した場合だけ3分benchmarkを更新する。詳細は
[bgi-load-cursor-a.md](bgi-load-cursor-a.md)を正とする。

Reached `RegisterClassExA`のgateway contractはrequired 48-byte `WNDCLASSEXA*`一引数、x86
stdcall、U16 zero-extended return、handler-owned LastError、guest write zeroを固定する。Policyは
64-byte以内のNUL終端ASCII name、styles `0x28`／`0x3020`、zero extras、NULL brush／menu／small
icon、mapped local module、same-module executable WndProc、live logical HICON／HCURSORだけを許可する。
SuccessはLastError preserve、duplicate module/nameはzero／1,410、bad `cbSize`はzero／87とする。

Owner model testはmonotonic class ID／ATOM `0xC000`、case-folded module/name index、ATOM index、
copy-owned record、non-owning icon/cursor reference、stale／duplicate plan、invalid reference／metadata、
ATOM exhaustion、class-first process teardownを検査する。Dispatcher fixtureはbad structure pointer／
size、wrong module、non-executable WndProc、unknown icon／cursor、unsupported field／style／name、disabled
policyのfailure atomicityと、failure後もfirst ATOMを消費しないことを固定する。Primary evidenceはtwo
balanced native traces、guarded identity/lifetime oracle、BGI main／child registrations、Node／Chrome
same immutable class snapshot／memory／`GetSystemMetrics` stop、clean cancel／watchdogを要求する。
Worker payloadとstateful lifetime変更後の3分benchmarkは既存17 thresholds全通過を要求する。詳細は
[bgi-register-class-ex-a.md](bgi-register-class-ex-a.md)を正とする。

Reached `GetSystemMetrics`のrepresentative contractはone-argument x86 stdcall、U32 register
result、LastError preserve、guest write／callback／runtime mutation zeroを固定する。Screen width／
heightは`RuntimeDisplays`のprimary RECTから導出し、caption／fixed-frame値だけをversioned User32
policyから読む。Dispatcher fixtureはreached five-index mapping、stack cleanup、success LastError、
memory generation不変に加え、unlisted index、disabled policy、missing／mismatched display authorityが
無変更でspecific faultになることを検査する。Primary evidenceはtwo balanced native traces、distinct
LastError sentinel oracle、BGIのframe／aspect consumer、Node／Chrome same ordered values／memory／
structured stop、clean cancel／watchdogを要求する。Read-only semantic leafなのでWorker transport、
scheduler、memory topology、graphics／audio、lifecycleのperformance gateは再実行せず、previous
observationをcurrent artifact measurementと扱わない。詳細は
[bgi-get-system-metrics.md](bgi-get-system-metrics.md)を正とする。

Private primary targetのcheckpoint gateは`npm run check:private-target`を正とする。Exact
implementation commit／tree、全dirty path、固定source fingerprint、current profile schema、
Node／Chrome semantic outcome、Rust／LLVM／v86 identity、next structured stop、memory digest、
pending work、artifact hashをversion 1 evidenceへまとめる。Runtime入力のdirty state、source
mismatch、semantic mismatchはhard failureとし、明示許可した無関係pathだけを結果へ残せる。
Performanceは別のversioned observationであり、semantic passへ混ぜない。詳細は
[private-target-gate.md](private-target-gate.md)を正とする。

<!-- END FROZEN HISTORICAL EVIDENCE -->

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

production callback scratch contractはscalar argumentとscratch-relative pointerを区別し、argument／scratch bound、checked x86 frame arithmetic、callback／sentinel execute access、全writable bandをmutation前に検証する。Valid caseはscratch padding、sentinel、全argumentsをone contiguous writeで固定し、exact top frameだけをretireしてtyped outer resultへ変換する。Negative caseはunmapped stack、invalid scratch offset、wrong／duplicate frame IDでmemory generation、callback depth、allocatorが不変であることを要求する。Callback中に既存API trapが到達するcaseはouter sequenceをentry時に予約し、nested callを処理してからouter completionを同じslotへcommitする。現在のrepresentativeはone-level stdcall BOOL callbackとone nested immediate APIであり、arbitrary depth、cross-thread callback、SEHの代用にはしない。

production outer-return representative contractはそのfixed frameをretireし、callee `RET`相当でouter return addressだけをconsumeしてresult `43`をguest cdecl callerへ返す。guest自身の2 argument cleanup、result write、entry comparison、stack balanceを検証するが、process-entry return sentinel、general entry lifecycle、nested／reentrant frame unwind、callback fault／cancellationの代用にはしない。

production entry-return representative contractはtyped sentinel ID `2`をinitial stackへ置き、guest entry result `0`とbalanced ESPをexact `#UD`で分類する。別modeでunmapped returnがPFEC `4`のままであることも検査するが、adapter-local one-page stackをproduction stack、process／thread exit、PEB／TEB、process parametersの代用にはしない。

production stack-bootstrap representative contractは同じone-page bytesとbootstrap metadataをRust owner／region／page reflectionから供給し、adapter-local map／writeを0件にする。これはownership handoffのevidenceであり、PE headerの任意stack size、guard／growth、multi-thread、teardown、mutation writebackの代用にはしない。

bootstrap FS first-stop representative contractはnative self-oracleとguest exact-faultを同じimport-free imageで結ぶ。FS base `0`からCR2 `0x18`になることを要求し、TEBがあるように見せるsilent zero-page mappingを許さない。これはoffset／fault baselineであり、PEB／TEB layout全体、process parameters、TLS、LastErrorの代用にはしない。

bootstrap FS ingress representative contractはfirst-stopと同じimageへ2 typed pagesだけを追加し、TEB self／PEB pointerのdirect／indirect observationとentry returnを要求する。fixed addresses、selector `0`、empty PEBをgeneral Windows process environmentの代用にはしない。

process-parameters first-stop representative contractはnative command-line self-oracleとguest zero-pointer resultを同じimport-free imageで結ぶ。fixture-defined result `2`とexact entry returnを要求し、empty PEBをsilent successさせない。測定した3 offsetをgeneral `RTL_USER_PROCESS_PARAMETERS`、current directory、environment、loader／process lifecycleの代用にはしない。

process-parameters ingress representative contractはfirst-stopと同じimageへ2 typed pagesだけを追加し、PEB pointer、8-byte `UNICODE_STRING`、42 command-line bytesのexact readbackとentry result `0`を要求する。fixed addresses、exact basename、zero-filled unmeasured fieldsをgeneral process parametersやBGI bootstrapの代用にはしない。

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
W2のcreate／invalidation ingress segmentは`GetModuleHandleA(NULL)`とfixed `RegisterClassExA`をstdcall resumeし、fixed `CreateWindowExA`では48-byte argument bandを保持したまま1段のWndProc stdcall callbackを往復する。callback frameはreturn sentinelを含む20 bytesで、WndProcの`ret 16`後にouter ESP `0x006FFFC0`へ戻る。outer service完了時はreturn addressと12 argumentsの52 bytesをpopし、EAX `0x57010001`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x0040109B`とする。fixed `ShowWindow`は2 DWORD argumentsをpopし、previous visibilityのBOOL `0`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010AE`でresumeする。fixed `InvalidateRect`は3 DWORD argumentsをpopし、BOOL `1`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010B9`でresumeする。host boundaryのcallee-saved registerとentry prologueのsaved ESI／EDIを保持する。後続のpaint／quit flowはfixed 2-level callbackとfixture entry returnまで通すが、これらをgeneral stdcall、invalid stack、thiscall／fastcall、arbitrary nested callback／reentrant service、callback fault／failure、general process entry lifecycleの代用にはしない。
<!-- END FROZEN HISTORICAL EVIDENCE -->

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

- same-origin localhost serverが登録済みsource rootとtitle-local profileだけを公開し、raw host path／wrong title／wrong snapshotを拒否する
- source manifest登録時に全objectのbyte length／SHA-256を検証し、HTTP request／responseのtitle fingerprint、snapshot、source、object、offset、requested／transferred lengthを照合する
- drive、absolute／relative、`.`／`..`、separator、case、trailing dot／space の policy
- case collision、enumeration、wildcard、rename、replace
- read-only source と writable overlay の通常 file shadowing、および executable／DLL shadowing の拒否
- sharing mode、delete-on-close、locking、mapped view の採用範囲
- CP932／Unicode filename、invalid sequence、long path の採用範囲
- SQLite base generation、duplicate commit、quota full、permission revoke、browser／Worker／server kill、commit interruption
- registry key case、value type、enumeration、delete、default value
- runtime schema upgrade／downgrade refusal、backup／restore
- import 後／delay-load 前の host source replacement と、managed module snapshot の restart 後 hash

最初のvertical sliceはimmutable sourceに加えてbounded writable profileまで成立している。Rust model／integrationはmount escape、Windows-932-aware case identity、source／profile path overlap、share conflict、create／truncate、sparse write、quota、cursor、guest-memory failure atomicity、durable close、failed-close retry、wrong／duplicate completion、process-exit discard、metadata-only profileのhash-bound atomic hydrationを検証する。Version 3 profileではfileと別namespaceのallowlisted directory identity／generationを持ち、host-first durable `CreateDirectoryA`、duplicate、missing parent、file conflict、stale／malformed completion、cancelのfailure atomicityも検証する。SQLite modelはnew profile、restart、base generation CAS、idempotent duplicate、request-key conflict、quota、transaction rollback、commit後response loss、epoch lease revoke／replacement、late commit拒否、SQLite lock、4 MiB profileのbounded binary range、same-origin profile-v3 range／commit／directory createを検証する。Recoverable storage failureはRustへWin32 failure completionを返し、dirty handle／staged bytesまたはuncommitted directory planをcontractに従って保持または破棄する。Public PE32 fixtureはimmutable `CreateFileA -> GetFileSize -> ReadFile -> SetFilePointer -> ReadFile -> CloseHandle`を自己検証し、clean-profile Chromeはsame-origin success、pending cancellation、watchdog cleanupを通す。Private targetはBGI.gdbをprofile objectとしてSQLiteから読み、run-local SQLiteへ`UserData` directoryを作成し、Node／Chromeのordered semantics、terminal commit barrier／lease revoke、配布fileとshared profile不変を比較する。

Controlled guestによるChrome上のWrite／durable Close、browser／Worker／server停止後の再load、各commit段階のkill injection、permission revokeは未完了である。Primary titleのsave UI、複数slot、registry、general mapped fileもこのstorage foundationからclaimしない。詳細は[runtime-vfs.md](runtime-vfs.md)を正とする。

Crash consistency test は SQLite transactionのstage前、stage後、commit中、publish後でprocess／Worker／server killを模擬し、再起動後に旧generationまたは新generationのどちらか一方が完全に読めることを確認する。

### 5.7 Window、message、input tests

現在のW2 native oracleは関連eventだけを`WM_CREATE -> WM_PAINT -> WM_DESTROY`としてexactに記録し、queued `WM_PAINT`の`GetMessageA`／`DispatchMessageA`経由、BeginPaint／SetPixel／EndPaint、destroy／quit、exit code `0`を自己検証する。可変な補助messageは順序contractに含めない。これはWindows oracleであり、Vineのlogical HWND、host surface、browser presentation、`AC-P4`の証明ではない。

現在のW2 class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flowはfixed valid caseだけを持つ。48-byte `WNDCLASSEXA`、executable guest WndProc、main-module HINSTANCE、ASCII nameをhost-owned class stateへcopyし、local class keyとATOM `0xC000`の双方向lookupを検証する。`CreateWindowExA`ではmodule identity、class key／ATOM／copied name、copied title、styles、semantic `CW_USEDEFAULT` position、requested outer size、null parent／menu、`creating -> created` lifecycleをhost-owned logical windowへ保持する。`ShowWindow`は同じlive objectを`visible=false -> true`へ遷移し、`showCommand=5`を追加する。native oracleとの比較はclass registration／window createがnonzeroで成功し、関連eventの先頭が1回の`WM_CREATE`でcallbackが0を返すsemanticまでとする。native fixtureは`ShowWindow` returnを無視するため、previous visibilityのBOOL `0`はfixed harness contractでありnative goldenではない。ATOM／HWND数値、raw補助message、`CREATESTRUCTA` bytes、callback stack／sentinel、物理的な表示もnative比較に含めない。

同じlive visible objectへ、`InvalidateRect`はsymbolic whole-client update region、erase false、paint state queuedを保持し、owner guest threadへgenerated-paint candidate 1件を追加する。native fixtureは`InvalidateRect`のnonzero returnをbranchで要求し、最終的に関連`WM_PAINT`が1回dispatchされることを自己検証するが、Vineのregion表現、thread lane、candidate sequence、coalescingをgoldenにしない。今回のfixture callは1回だけなのでhidden windowとrepeat coalescingは未検証である。

fixed `GetMessageA`はcandidateをzero metadataのMSGへmaterialize／dequeueし、fixed `DispatchMessageA`は同じMSGからregistered WndProcの`WM_PAINT` branchへ入る。fixed paint／quit continuationはBeginPaint／SetPixel／EndPaint、nested WM_DESTROY／PostQuit、strict unwind、WM_QUIT、entry returnまで進み、native oracleの関連event／counter／COLORREF／quit code／exit `0`と一致する。native oracleはVineの2x2 bounds、PAINTSTRUCT bytes、HDC／HWND数値、pixel map、zero metadata、call ID／stack addressをgoldenにしない。Vine側の1 pixelはlogical stateであり、物理的な表示は未検証である。

Primary window gatewayは別のproduction contractとして、Rust `RuntimeUser32`がclass relation、copied
title、geometry、owner thread、logical HWND、`creating -> created` lifecycleを所有する。
`CreateWindowExA`は`WM_NCCREATE -> WM_NCCALCSIZE -> WM_CREATE -> WM_SIZE -> WM_MOVE`を同じdepthの
callback replacementで完了し、`WM_NCCALCSIZE`だけが16-byte RECTを書き換える。`WM_SIZE`／`WM_MOVE`の
`LPARAM`はpointerでなくpacked scalarとして扱う。Dedicated callback scratch arenaが下降stackと重ならず、
preflight failure、callback rejection、stale replacementはwindow identity、allocator、memory、frame stackを
部分更新しない。Focused native oracleはmessage順、default result、RECT、menu failure chainとcallerの
nonzero HWND consumerを比較するが、native HWND数値、一般menu、visibility、queued message、DOM window、
rendererをgoldenまたは実装済みとしない。

Primary synchronous-message contractは、このcreation gatewayで`created`になったsame-thread HWNDだけを
対象に、`SendMessageA(WM_SETICON, ICON_SMALL, shared 16x16 HICON)`をregistered class WndProcへstdcall
4-argument callbackとして再入する。Focused Rust contractはcallback target／frame／callee cleanup、
LastError preserve、WndProcのu32 `LRESULT`返却を固定する。Nested `DefWindowProcA`はsmall-icon relationを
owner-local planでcommitし、再設定ではprevious HICONを返す。Wrong／stale HWND、creating lifecycle、wrong
owner thread、other message／`WPARAM`、NULL／wrong-type／stale／wrong-size HICON、callback memory preflight、
stale icon planではwindow／class／icon、callback allocator、guest memoryを部分更新しない。Native oracleは
WndProcがSendのdirect child／same threadで4 argumentsをそのまま受け、result 0とLastError preserveを返し、
callerが`LRESULT`を読まずwindow-position setupへ進むことを比較する。Cross-thread／queued send、arbitrary
message、unbounded reentrancy、timeout、DOM dispatchは実装済みまたはgoldenとしない。

Primary window-position contractはversioned initial show／hideとsame-geometry activation profileを扱う。Showは
`SetWindowPos(HWND_NOTOPMOST, 15360, 8640, 0, 0, 0x559)`をlogical display／non-client geometryへ解釈し、
`WM_WINDOWPOSCHANGED -> nested WM_MOVE -> same-depth WM_GETICON(ICON_SMALL2)`を完了してexpected shared
small-icon identityを確認した後だけgeometry、visible、topmostをcommitする。Hideはpositionを保持したまま
`WM_WINDOWPOSCHANGED`完了後だけvisibleをfalseへcommitする。

Activation caseはvisibleなowner-thread windowに
`SetWindowPos(HWND_NOTOPMOST,0,0,806,629,0x10A)`を呼び、geometry／style／visibilityを変えずに
`WM_WINDOWPOSCHANGING`二回、`WM_ACTIVATEAPP`、`WM_NCACTIVATE`、`WM_ACTIVATE`を順に送る。
`WM_ACTIVATE` default processing内は`WM_IME_SETCONTEXT -> nested WM_IME_NOTIFY -> replacement WM_SETFOCUS`、
maximum callback depth 3とする。最初のWINDOWPOS callback前はactive／focusなし、foregroundはexternal、
二回目ではactive／foregroundがtargetでfocusなし、IME／focus messageでは三relationともtargetであることを
logical identityとして比較する。Native z-order HWNDとexternal thread IDの数値はgoldenにしない。

Focused Rust testsはoperation／relation generation、callback order／depth／replacement、packed move coordinates、
WINDOWPOS scratch、wrong owner／lifecycle／flags／icon result、stale／duplicate plan、callback rejectionのfailure
atomicityを固定する。Native oracleはshow／hideの二BOOLに加え、activation success／LastError preserve、invalid
HWNDの0／1,400、message／result順、relation transition、absence of `WM_WINDOWPOSCHANGED`／`WM_SIZE`／`WM_MOVE`、
caller consumerを二つのbalanced traceで比較する。Node／Chrome equivalenceはRust authorityのread-only final
window snapshot、8 callback completions、active／focus／foreground relation、same memory digest、next stopを
比較する。General resize／z-order、cross-thread／multiple-window activation、queued paint、DOM positioningは
実装済みまたはgoldenとしない。

Already-active repeat caseは同じHWND、`HWND_NOTOPMOST`、current `1286x749` outer geometry、flags `0x10A`を
受け、`WM_WINDOWPOSCHANGING`一回だけをWndProc／`DefWindowProcA`へ送る。Focused model／dispatcher testsは
callback result不一致とstale relation／window planがwindow、active／focus／foreground、generation、guest bytesを
部分更新しないこと、success completionも全`RuntimeUser32` snapshotを不変に保つことを比較する。Native二runは
BOOL 1、LastError 0、geometryと三relation不変、activation／resize／changed／move message不在を固定する。
Node／Chromeはsequence 108,718、frame 25、scratch `0x00501000`、validate-only disposition、zero writes、
protocol v25 predecessorの`MessageBoxA` stopまでを同じsemantic／memory digestで再現する。General idempotent positioning、z-order変更、
cross-thread／multi-window activationは実装済みまたはgoldenとしない。

Reached D3D9 recreated-texture bandは、native二runでRVA `0x5A715`／`0x5A7B7`／`0x5A7DC`からの
`2048x1024`、level 1、format 22、system-memory static／default static／default dynamicの三callを固定する。
各callは`S_OK`、LastError 0、4-byte output publication、non-NULL distinct texture、HRESULT sign consumerを要求する。
Rust model／dispatcher testsはversion 19の六descriptor allow-list、system-memory variant indices `0,3`だけの
`GetSurfaceLevel(0)`、capacity／bad pointer／unsupported descriptorのfailure atomicity、COM／graphics／guest-memoryの
cross-owner preflightを比較する。Node／Chrome protocol v26はsequence 108,735–108,743の
`CreateTexture -> GetSurfaceLevel -> GetDC -> GetStockObject -> FillRect -> ReleaseDC -> Release -> CreateTexture -> CreateTexture`
を完全一致させ、surface `2048x1024`、fill `[0,0,1280,720]`、renderer resourceなし、semantic／memory digest、
次の`SetWindowPos(HWND_TOP,317,154,0,0,0x25)` structured stopを比較する。General texture dimensions、mip chain、
device loss、GPU allocation／upload／presentはこのbandのacceptanceへ含めない。

続くframe-change move leafは、native二runのcaller RVA `0x938C2`、BOOL 1／LastError preserve、invalid HWNDの
FALSE／1,400、outer origin `15360,8640 -> 317,154`、client origin `15363,8666 -> 320,180`を固定する。
Native message orderは`WM_WINDOWPOSCHANGING -> WM_NCCALCSIZE -> WM_NCPAINT -> WM_ERASEBKGND ->
WM_WINDOWPOSCHANGED -> WM_MOVE`である。Runtime acceptanceはpaint／eraseをnon-claimとして除外し、frames
26–28の`WM_WINDOWPOSCHANGING -> WM_NCCALCSIZE -> WM_WINDOWPOSCHANGED`とnested frame 29の`WM_MOVE`
だけをモデル化する。Focused model／dispatcher testsはNCCALCSIZE completion前のgeometry不変、completion時の
targeted commit、callback result mismatch、stale／duplicate plan、wrong geometry／insert-after／HWNDのfailure
atomicity、GDI surfaceとactive／focus／foreground relation不変を比較する。Node／Chrome protocol v27はsequence
127,346、nested sequence 127,349、zero guest writes、同じsemantic／memory digest、次の
`ShowWindow(hwnd,SW_SHOWNORMAL)` structured stopまでを一致させる。General z-order、repaint、multi-window move、
DOM positioningは実装済みまたはgoldenとしない。

続くalready-visible `ShowWindow(SW_SHOWNORMAL)` leafは、native resultのopaque nonzero 24をBOOL 1へ正規化し、
LastError、geometry、visibility、active／focus／foreground relation、update regionを不変に保つ。Invalid HWNDは
FALSE／1,400である。Focused Rust testsはunsupported command、wrong lifecycle／relation、disabled policyを
mutation前に拒否することを固定する。Node／Chromeはsequence 127,351、zero guest writes、callback 0、browser
resource 0を一致させる。

`UpdateWindow` paint leafは、pending full-client regionがあるvisible same-thread windowだけを対象に
`WM_PAINT` frame 30を同期開始する。`BeginPaint`はregionを消費し、one logical paint HDC、paint generation 1、
64-byte PAINTSTRUCTをUser32／GDI／GuestMemory joint transactionでpublishする。`EndPaint`はsame HWND／thread／
PAINTSTRUCT bytesだけを受理し、sessionとHDCをretireする。Focused model／dispatcher testsはcallback entry前の
region不変、Begin後のsession／HDC／bytes、generic `ReleaseDC` rejection、corrupted structure、wrong callback result、
invalid HWND、missing region、disabled policy、stale／duplicate planのfailure atomicityを比較する。Node／Chrome
protocol v28はBegin sequence 127,353、End sequence 127,354、full-client rect、erase 1、zero live session／HDC after
Endを一致させる。Outer UpdateWindowは次の`IDirect3DDevice9::Clear` slot 43 stop時点でcallback-activeなので、
completed-call countへ含めない。General invalidation、nested paint、clip／background erase、queued dispatch、Canvas／
WebGL presentationはこのleafのacceptanceへ含めない。

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
続くD3D9 Clear leafはnative二runの402／404 callsすべてがcaller RVA `0x5AF9E`、explicit arguments
`0,0,1,0xFF000000,0x3F800000,0`、`S_OK`、LastError 0であることを固定する。Focused Rust
model／dispatcher testsはslot 43 catalog、exact profile、logical back-buffer owner、plan／preflight／targeted commit、
invalid variantとstale／duplicate planのfailure atomicityを比較する。Node／Chrome protocol v29はsequence
127,355、instruction 28,761,128、device reference count 5、back-buffer rect `[0,0,1280,720]`、zero guest writes、
同じsemantic／terminal-memory digestを一致させる。次stopは29 instructions後の
`USER32.dll!SetRect(0x006FF29C,0,0,1280,720)`である。Partial rectangle、depth／stencil clear、pixel backing、
draw／Present、device loss、Canvas／WebGL／WebGPU outputはこのleafのacceptanceへ含めない。
<!-- END FROZEN HISTORICAL EVIDENCE -->

続く`SetRect` leafは、focused native二runが各498 balanced API pairs／diagnostics 0で、primary call
`SetRect(rect,0,0,1280,720)`をcaller RVA `0x5B00B`から観測し、TRUE、LastError preserve、16-byte
`RECT [0,0,1280,720]`全書込を一致させることをoracleとする。同じrun内のcontrolled oracleはsigned
coordinates `[-7,9,123,-456]`をexact `LONG32`として書き、success時の二つのLastError sentinelを保存し、
NULL outputではFALSEを返して別のsentinelを保存する。CallerはBOOLを分岐に使わず、同じRECT pointerを直後の
`IDirect3DTexture9::AddDirtyRect`へ渡す。Focused Rust testsはfive-word stdcall ABI、nullable 16-byte output、
signed encoding、NULL path、unmapped outputの全byte failure atomicityを固定する。

Canonical Node／Chrome evidenceはtyped arguments、output address／bytes、normalized BOOL、LastError preserve、
stdcall cleanup、ordered semantic／terminal-memory equality、manifested next stopを一致させる。Rust `GuestMemory`だけが
bytesのauthorityであり、User32またはTypeScriptにpersistent RECT stateを作らない。Exact protocol、sequence、
instruction、report artifact、next-stop identityはcurrent manifestだけを正本とする。RECTの意味をtexture
dirty-regionへ反映すること、NULL dirty rect、texture generation、upload／draw／Presentはこのleafのacceptanceへ
含めない。

続く`IDirect3DTexture9::AddDirtyRect` checkpointは、focused native二runが各8 callsを観測し、primary
`RECT [0,0,1280,720]`、interior `[1,2,17,19]`、NULLが`D3D_OK`、empty／inverted／negative／
out-of-boundsが`D3DERR_INVALIDCALL`、全LastError sentinelがpreserveされることをoracleとする。Primary
caller RVAは`0x5B01D`、source creation RVAは`0x5A715`、immediate consumerはdevice slot 31
`UpdateTexture`である。Focused Rust model／dispatcher testsはsystem-memory source profile、initial
whole-texture dirty state、partial bounding union、NULL／full canonicalization、idempotent generation、wrong-owner／
disabled-policy／invalid RECT／state-changing stale／duplicate plan／generation exhaustionのfailure atomicityを固定する。

Protocol v32のNode／Chrome acceptanceはsequence 127,357、instruction 28,761,165、source texture 7／COM 22、
device 3、descriptor `2048x1024`／one level／static X8R8G8B8 SYSTEMMEM、dirty region whole、texture
generation 41、content generation 1、fill sequence 3、zero guest writes、reference count 1、renderer resourceなしを
一致させる。AddDirtyRect単体profileのcanonical stopは10 instructions後の
`IDirect3DDevice9::UpdateTexture(0x01020004,0x01030004)` slot 31である。

続く`UpdateTexture` gatewayは、focused native二runがprimary static SYSTEMMEM→static DEFAULTを`D3D_OK`、
controlled non-NULL oracleがrepeat／dynamic DEFAULT successとreversed pool／DEFAULT source／same-resource
`D3DERR_INVALIDCALL`、全LastError preserveを一致させることをoracleとする。Active policyはprimaryで到達した
static destinationだけを許可し、dynamic destinationと安全なnative oracleを得ていないNULL variantはclaimしない。
Focused Rust testsはtexture-owned content、surface retirement後のfill存続、二texture plan／preflight／targeted commit、
clean-source repeat、stale／wrong-role／partial-dirty failure atomicityを固定する。Node／Chromeはsequence 127,358、
instruction 28,761,175、source texture 7／COM 22／generation 42、destination texture 8／COM 24／generation 45、
content generation 1、fill sequence 3、source dirty consumption、zero guest writes、renderer resourceなしを一致させる。
Canonical stopは6 instructions後の`IDirect3DDevice9::BeginScene()` slot 41である。Texel backing、partial-dirty transfer、
dynamic destination、browser upload、scene／draw／Presentはこのcheckpointのacceptanceへ含めない。

後続のtexture `LockRect`／`UnlockRect` checkpointは、focused native二runでprimary SYSTEMMEM textureのlevel 0、
`RECT [0,0,1280,720]`、flags `0x8000`が`D3D_OK`、pitch 8,192を返し、nested lockが
`D3DERR_INVALIDCALL`かつoutput不変、unlockとrepeated unlockが`D3D_OK`、全LastError sentinel preserveとなることを
oracleとする。Raw traceはprimary 2,636 events／1,278 balanced pairs、confirmation 2,538 events／1,229 balanced pairsで、
最大API depth 3、diagnostics 0、source executable hash不変を固定する。

Focused Rust model／dispatcher testsはlazy 8,388,608-byte graphics allocation、GuestMemory byte authority、pitch／bitsの
atomic output、read-write／read-only protection transition、single active lock、nested／stale／wrong-owner／unwritable-output／
capacity failure atomicity、source allocation retirement guard、unlock-without-lock no-opを比較する。Node／Chromeはlock
sequence 148,198／instruction 147,769,351、output `0020000000100044`、allocation 77／owner 100、unlock sequence
148,203／instruction 148,730,506、content generation 2を一致させる。

同じ機能群のpartial `UpdateTexture` acceptanceはsequence 148,272／instruction 148,744,059、exact dirty rectangle
`[0,0,1280,720]`、source／destination generation 46、content generation 2、update sequence 2を一致させる。
Destinationがsourceの直前generationを表すprecondition、full authoritative backing publication、source dirty consumption、
two-texture failure atomicityを固定し、arbitrary partial historyとdynamic destinationはclaimしない。Workerはdraw時にのみ
execution mirrorからchecked byte copyし、Node／Chromeは二回目のscene／draw／Presentまで同順序で完了して、次の
`IDirect3DDevice9::TestCooperativeLevel()` slot 3へ進む。

同じ機能群のcooperative-level acceptanceは、generic native traceの998 callがすべて`D3D_OK`でLastErrorを保持し、
callerが`D3DERR_DEVICELOST`／`D3DERR_DEVICENOTRESET`を実際に分岐判定することを固定する。Focused二runは
最大API depth 3かつbalancedであり、controlled live-device oracleはLastError sentinel `0x11223344`／`0x55667788`を
保持して`D3D_OK`を返す。Rust model／dispatcher testはlive graphics-device associationをauthorityとし、成功時の
graphics／COM／guest memory／runtime generation不変と、missing／retired／wrong-kind failure atomicityを固定する。
Node／Chromeはsequence 148,293／instruction 148,744,443、device `0x01010004`、`D3D_OK`、reference count 5、
zero guest writesを一致させる。Device lost／not-reset transition、Reset、browser context loss連携、recoveryはclaimせず、
その後のUser32 message-queue subsystem gateway identityはcurrent manifestだけを正本とする。

続く`BeginScene` leafは、focused primary trace二runがcaller RVA `0x5B048`、zero explicit arguments、`D3D_OK`、
LastError 0 preserve、直後の`test eax,eax; js` consumerを一致させることをoracleとする。Controlled PE32/i386
lifecycle oracleは二回の同一runでvalid begin／end cycleを`D3D_OK`、outside／repeated EndSceneとnested BeginSceneを
`D3DERR_INVALIDCALL`、全callのLastError sentinel `0x5A5A` preserve、device reference count不変として固定する。
Focused Rust model／dispatcher testsはdevice-local scene authority、sequence／generation preflight、inactive-to-active
targeted commit、nested／NULL／unknown／stale／duplicate failure atomicityを比較する。

Node／Chrome protocol v33はsequence 127,359、instruction 28,761,181、device 3／COM 21、reference count 5、
presentation `1280x720`、back-buffer clear generation 1、active scene sequence 1、device generation 45、zero guest writes、
renderer resourceなし、ordered semantic／terminal-memory equalityを一致させる。Canonical stopは14 instructions後の
`IDirect3DDevice9::SetSamplerState(0,6,2)` slot 69である。EndScene、sampler state、draw／Present、device loss、
texel backing、browser upload、visible outputはこのcheckpointのacceptanceへ含めない。

続く`SetSamplerState` leafは、focused primary trace二runがcaller RVA `0x5B18A`／`0x5B1A0`、sampler 0、
MINFILTER 6／MAGFILTER 5、LINEAR 2の順序、`D3D_OK`、LastError 0 preserve、戻り値をbranchせず次callへ進むことを
一致させる。Controlled PE32/i386 readback oracleは初期MIN point、MIN／MAG linearへの遷移、同値repeat success、
全callのLastError sentinel `0x5A5A` preserve、device reference count不変を固定する。Native driverがtested invalid
sampler／state／filterにも`D3D_OK`を返す事実は記録するが、active runtime policyはprimaryの二callだけへ狭め、
profile外variantを`D3DERR_NOTAVAILABLE`かつmutation-freeに拒否する。

Focused Rust model／dispatcher testsはdevice-local sparse typed sampler authority、enum validation、generation
preflight、targeted／idempotent commit、NULL／unknown／wrong-class／stale／duplicate／unsupported／exhaustionのfailure
atomicityを固定する。Node／Chrome protocol v34はsequence 127,360／127,361、instruction 28,761,195／28,761,204、
device 3／COM 21、reference count 5、MIN then MIN+MAG linear、device generation 46／47、zero guest writes、renderer
resourceなしを一致させる。続くunprofiled `SetRenderState(171,1)`、`(20,6)`、`(19,5)`は三件とも
`D3DERR_NOTAVAILABLE`となりcallerが結果を無視することを明示的に一致させる。Canonical stopはinstruction
28,761,237の`IDirect3DDevice9::SetTextureStageState(0,1,4)` slot 67である。他sampler／state／filter、三render
state、texture-stage state、EndScene、draw／Present、pixel backing、browser rendererはacceptanceへ含めない。

続く`SetTextureStageState` leafは、focused primary trace二runがstage 0、state 1–6、value
`4,2,0,2,2,0`の連続call、`D3D_OK`、LastError 0 preserve、最初の五callから次state setupへ進み最後のcallから
`SetTexture`準備へ進むconsumerを一致させる。Controlled PE32/i386 readback oracleは初期値
`4,2,1,2,2,1`から到達band適用後の`4,2,0,2,2,0`への遷移、同値repeat、LastError sentinel `0x5A5A`
preserve、device reference count不変を固定する。Native driverがtested invalid stage／state／valueにも`D3D_OK`を
返す事実はdriver evidenceとしてのみ記録し、active policyは到達した六tupleだけを許可する。

Focused Rust model／dispatcher testsはdevice-local sparse typed texture-stage authority、enum validation、generation
preflight、targeted／idempotent commit、NULL／unknown／wrong-class／stale／duplicate／unsupported／exhaustionのfailure
atomicityを固定する。Node／Chrome protocol v35はsequence 127,365–127,370、instruction
28,761,237–28,761,282、device 3／COM 21、reference count 5、cumulative color／alpha state、device generation
48–53、zero guest writes、renderer resourceなしを一致させる。Canonical stopはinstruction 28,761,291の
`IDirect3DDevice9::SetTexture(0,0x01030004)` slot 65である。他stage／state／value、texture binding、EndScene、
draw／Present、pixel backing、browser rendererはacceptanceへ含めない。

続く`SetTexture` leafは、focused primary trace二runが同じdevice、stage 0、non-NULL texture、caller RVA
`0x5B27C`、`D3D_OK`、LastError 0 preserve、直後の`SetFVF(0x144)` consumerを一致させる。Controlled
PE32/i386 oracleはbind／repeat／switch／unbind、LastError sentinel `0x5A5A` preserve、SetTextureによる
reference count不変とGetTextureの一時AddRefを固定する。Native driverがwrong-device textureやstage 16も
受理する事実はdriver evidenceとしてのみ記録し、active policyはsame-deviceのstatic DEFAULT variant 4と
stage 0／NULL unbindだけへ狭める。

Focused Rust model／dispatcher testsはdevice-owned non-owning binding、same-device validation、generation
preflight、targeted／idempotent commit、texture retirementと同じcommitでのbinding clear、NULL／unknown／
wrong-class／wrong-owner／unsupported／stale／duplicate／exhaustionのfailure atomicityを固定する。Node／Chrome
protocol v36はsequence 127,371、instruction 28,761,291、device 3／COM 21、texture 8／COM 24、stage 0、
device generation 54、reference count 5、zero guest writes、renderer resourceなしを一致させる。Canonical stopは
instruction 28,761,298の`IDirect3DDevice9::SetFVF(0x144)` slot 89である。他stage／resource binding、vertex
format、EndScene、draw／Present、pixel backing、browser rendererはacceptanceへ含めない。

続く`SetFVF` leafは、focused primary trace二runが同じdevice、FVF `0x144`、caller RVA
`0x5B291`、`D3D_OK`、LastError 0、結果を分岐へ使わず直後に
`SetStreamSource(0,vertexBuffer,0,28)`を準備するconsumerを一致させる。Controlled PE32/i386 oracleは
初期値0と`0x144`／`2`／`0`／`0xFFFFFFFF`のnative set／get roundtrip、LastError sentinel `0x5A5A`
preserve、device reference count不変を固定する。Nativeの`GetFVF(NULL)`はprocess faultとなるため安全な
oracleから除外し、NULL output semanticsをclaimしない。Active policyはnative permissivenessを一般化せず
到達値`0x144`だけへ狭める。

Focused Rust model／dispatcher testsはdevice-owned typed FVF authority、generation preflight、targeted／
idempotent commit、unknown／wrong-class／wrong-owner／unsupported／stale／duplicate／exhaustionのfailure
atomicity、zero guest writes、LastError preserveを固定する。Node／Chrome protocol v37はsequence 127,372、
instruction 28,761,298、device 3／COM 21、device generation 55、reference count 5、typed
`xyz-rhw-diffuse-texture1`、renderer resourceなしを一致させる。Canonical stopはinstruction 28,761,315の
`IDirect3DDevice9::SetStreamSource(0,0x01050004,0,28)` slot 100である。他FVF、stream binding、EndScene、
draw／Present、pixel backing、browser rendererはacceptanceへ含めない。

続く`SetStreamSource` leafは、focused primary trace二runが同じdevice、stream 0、non-NULL vertex buffer、offset 0、
stride 28、caller RVA `0x5B3EC`、`D3D_OK`、LastError 0、結果を分岐へ使わず直後に
`DrawPrimitive(D3DPT_TRIANGLESTRIP,0,2)`を準備するconsumerを一致させる。Controlled PE32/i386 oracleはinitial
NULL binding、target／repeat、offset 4、stride 0、stream 1、NULL unbindのnative set／get roundtrip、LastError
sentinel `0x5A5A` preserve、SetStreamSourceによるvertex-buffer reference count不変、GetStreamSourceの一時AddRefを
固定する。Active policyはnative permissivenessを一般化せず、到達したstream 0／same-device vertex buffer／offset 0／
stride 28だけへ狭め、NULL unbindを含む他variantをmutation前に拒否する。

Focused Rust model／dispatcher testsはdevice-owned non-owning stream binding、same-device ownership、generation
preflight、targeted／idempotent commit、vertex-buffer retirementと同じcommitでのbinding clear、NULL／unknown／
wrong-class／wrong-owner／unsupported／stale／duplicate／exhaustionのfailure atomicity、zero guest writes、LastError
preserveを固定する。Node／Chrome protocol v38はsequence 127,373、instruction 28,761,315、device 3／COM 21、
logical vertex buffer 3、offset 0、stride 28、device generation 56、reference count 5、renderer resourceなしを一致させる。
Canonical stopはinstruction 28,761,324の`IDirect3DDevice9::DrawPrimitive(5,0,2)` slot 81である。他stream、offset、
stride、NULL unbind、draw semantics、EndScene、Present、pixel backing、browser rendererはacceptanceへ含めない。

続く`DrawPrimitive` gatewayは、focused primary trace二runが同じdevice、primitive type 5、start vertex 0、primitive
count 2、caller RVA `0x5B402`、`D3D_OK`、LastError 0、結果を分岐へ使わずslot-42 `EndScene`へ進むconsumerを
一致させる。Controlled PE32/i386 oracleはscene外、required state不足、stream不足の`D3DERR_INVALIDCALL`、target／
repeat success、LastError sentinel `0x5A5A` preserve、device／vertex-buffer reference count不変を固定する。Nativeが
受理したzero-count、out-of-range、triangle-list、scene外-after variantは到達policyへ一般化しない。

Focused Rust model／dispatcher testsはactive sceneとexact typed stateのpreflight、checked `GuestMemory` read、112-byte
owning immutable vertex snapshot、texture content snapshot、4,096 submission／1 MiB vertex-byte limit、stale／capacity／
wrong-owner／uninitialized／locked／unsupported failure atomicity、resource retirement後のsnapshot保持、Clearによるpending
submission破棄を固定する。Node／Chrome protocol v39はsequence 127,374、instruction 28,761,324、device 3／COM 21、
device generation 57、back-buffer generation 2、submission／scene sequence 1、vertex buffer 3、texture 8、vertex hash
`405bea74541d2aece445d0dac693da55fb404e777c91ff81d87d4c6272581362`、zero guest writes、renderer resource／visible
pixelsなしを一致させる。Canonical stopはinstruction 28,761,332の`IDirect3DDevice9::EndScene` slot 42である。他draw、
`EndScene` semantics、Present、pixel backing、browser renderer／visible outputはacceptanceへ含めない。

続く`EndScene` gatewayは、focused primary trace二runが同じdevice／thread、caller RVA `0x5B43B`、引数なし、
`D3D_OK`、LastError 0、HRESULTを分岐へ使わないconsumerを一致させる。共有controlled PE32/i386 scene-lifecycle
oracleはfirst begin／endとsecond cycleのsuccess、nested begin、outside／repeated endの`D3DERR_INVALIDCALL`、
LastError sentinel `0x5A5A` preserve、device reference count不変を固定する。

Focused Rust model／dispatcher testsはactive-scene identity、device／graphics generation preflight、active sceneだけを
clearするtargeted commit、draw／clear content保持、stale／duplicate／unknown／inactive failure atomicity、zero guest writes、
LastError／COM reference preserveを固定する。Node／Chrome protocol v40はsequence 127,375、instruction 28,761,332、
device 3／COM 21、device generation 58、active sceneなし、back-buffer generation 2、draw submission保持、reference count 5、
renderer resource／visible pixelsなしを一致させる。Capability checkpointはinstruction 28,761,389で完了し、次のowned
endpointを`IDirect3DDevice9::GetRasterStatus` slot 19／`0x01011160`として固定する。続く通常wait loopをunsupported stopとは
扱わない。他scene variant、renderer translation、Present、pixel backing、browser upload／visible outputはacceptanceへ含めない。

続くpost-scene raster gatewayはprotocol v43／browser report schema 11で検証する。Node／Chromeは
`GetRasterStatus(0,0x006FF280)`をsequence 127,376／instruction 28,761,389で完了し、output
`0000000037040000`、scanline 1079、COM refcount 5、zero graphics／reference mutation、LastError preserve、
atomic eight-byte writeを一致させる。Capability completionはinstruction 28,761,426、semantic digest
`0d078f5f58c953ff8d8c952a33497e308c5e7632c6c03437c21ca04a85f711d6`、memory digest
`c8e437febb7fd0b6b79ad5822f8185cc336e16b532c980aa39815479590f204e`を固定し、次のowned endpointを
`IDirect3DDevice9::Present` slot 17／`0x01011140`として記録する。Host／browser scanout import、post-scene
timer cycle、Present execution、renderer resource、visible pixelsはこのcheckpointのacceptanceへ含めない。

続くfirst-Present gatewayはprotocol v44／browser report schema 12で検証する。Focused native trace二runは
slot 17、caller RVA `0x5B7B1`、四つのoptional argumentが全てNULL、`D3D_OK`、LastError 0 preserve、
`test eax,eax`後のnegative-HRESULT branchを一致させる。Rust model／dispatcher testsはclosed scene、clear／draw
content、device／content／Present generationをplan／preflightし、content generation 2をimmutable frame sequence 1へ
publishした後にempty discard generation 3を作るtargeted commitを固定する。Active scene、missing content、non-NULL
argument、stale／duplicate plan、generation exhaustionはdevice、allocator、frame sequence、back buffer、guest bytes、COM
refcountを不変にする。

Node／Chromeはsequence 127,377／instruction 28,761,426、device 3／COM 21、device generation 59、zero guest writes、
`D3D_OK`を一致させる。Chromeはguest execution前にtransferred `OffscreenCanvas`のWebGL2 context／shader、exact GPU
storage、bounded CPU stagingをpreflightし、complete fixed-function snapshotの検証後だけresource `browser-resource:1`を
non-owningでbindする。1280x720、texture upload 8,388,608 bytes、vertex upload 112 bytes、draw 1、center RGBA
`[0,0,0,255]`、WebGL error 0、guest resume前renderを検証する。Nodeはbrowser resourceなしで同じRust semanticsを検証する。両者の
semantic digestは`792c9f0f6f620fdce04445f5d6138b317656bed87a392196fae2ed0d09bc3398`、memory digestは
`bff1bcdfd09d14143797630f57222aaf9a94ca08e29593a85010690da7e804b9`で、checkpoint completionはinstruction
28,761,433、次endpointは`WINMM.dll!timeGetTime` ID 8／`0x7F000070`である。Readable title content、partial／redirected／
dirty-region Present、second frame、resize、context／device lossとrecoveryはacceptanceへ含めない。

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
続くpost-Present continuationはprotocol v45／browser report schema 13で一つのcohesive acceptanceにする。
Node／Chromeは`Present`後の`timeGetTime`をsequence 127,378／instruction 28,761,433で完了し、shared
process tick 3,601,007、次tick 3,601,008、LastError preserve、zero guest writesを一致させる。同じ
`WM_PAINT` frame 30がinstruction 28,761,544でresult 0を返した後、`GetForegroundWindow`をsequence
127,379／instruction 28,761,549で完了し、Rust-owned foreground relationのlogical HWND
`0x48010001`、zero-argument stdcall cleanup、LastError preserve、zero mutationを一致させる。NULL foreground
relationもRust dispatcher testで0を返し、logical window／LastError／guest memoryを変更しないことを固定する。
TypeScriptはWindows foreground identityを所有せず、browser resourceを作成しない。

Canonical stopは127,690 supported calls／29,325,480 instructions、semantic digest
`ac1eee34c5e1f3e4746600db145ac70a2d0a023949a9094729572c7a04c41617`、memory digest
`6ba9e12a6946dd09a1d673d76f7dab2a11436ad1f7fe5989d2c43a6698b82321`で、次endpointは
`USER32.dll!GetAsyncKeyState(0x11)` ID 231／`0x7F000E60`である。このcheckpointはinputを未消費とし、
keyboard event transport、focus／foreground gating、pressed-since-last-query bit、repeat、multi-key stateをclaimしない。
<!-- END FROZEN HISTORICAL EVIDENCE -->

Async keyboard input continuationはWorker message protocol 47／browser BGI report schema 15で検証する。
一件のfocused `ControlLeft` key-downだけを投入し、side-specific `VK_LCONTROL`からgeneric
`VK_CONTROL`へのaggregation、最初のsigned-SHORT `0xFFFF8001`、追加inputなしの後続pollで
`0xFFFF8000`を要求する。これによりqueried generic keyのpressed-since-query bitだけが一度消費され、
current-down bitは保持されることをNode／Chromeのordered semantics、LastError preserve、zero guest write、
最終memory contentで一致させる。Continuation中に到達するsource object 3の三つのimmutable readもoffset／length／
payload identityまで一致させるが、zero-input counterfactualがないためinputとの因果やvisible gameplay responseは
acceptanceへ含めない。Key-up、全38-key initial scan、message translation、DirectInput keyboard、arbitrary live inputは
次のscenario evidenceとする。Exact occurrence、instruction、digest、artifact identity、次endpointは変動値の正本である
`docs/current-checkpoint.json`だけから検証する。

Primary system-menu gatewayは、初期window-creation中のNULL failure chainと、created main windowの
`GetSystemMenu(FALSE) -> EnableMenuItem(SC_CLOSE, MF_BYCOMMAND | MF_ENABLED)` success chainを分離して検証する。
`runtime::user32::tests::system_menu_identity_is_window_owned_reentrant_and_failure_atomic`はwindow-owned identity、
repeat reuse、initial close-item state、idempotent enable、専用menu generation、capacity／handle exhaustion、process
teardownを固定し、menu allocation後にも事前作成したwindow mutation planがcommitできることを要求する。
`user32_dispatcher::live_system_menu_identity_is_window_owned_reused_and_failure_atomic`とmenu contract／negative testsは
x86 stdcall cleanup、logical-handle namespace、success LastError preserve、NULL／unknown handle failure、unsupported
revert／command／flagsのmutation-free rejectionを固定する。Focused native evidenceは二つのbalanced runで、nonzero
native HMENUが次のEnable callへそのまま渡され、previous state 0とLastError 0が返るrelationだけをgoldenにする。
Native handle数値は比較しない。Primary Node／Chromeは外側resizeの全4 callback return、active callback depth 0、
二つのlive menu completion、zero guest writes、同じordered semantic digestと`Sleep(0)` structured stopを一致させる。
TRUE revert、arbitrary menu tree／item state、owner-destroy lifecycle、DOM／browser menu renderingは実装済みまたは
acceptance済みとしない。

Reached `DragAcceptFiles` leafは、live `created` HWNDのeffective extended styleにある
`WS_EX_ACCEPTFILES` bitだけを更新する。Rust model／dispatcher testsはTRUE／FALSE、idempotent generation、
unknown／wrong-lifecycle handle、stale／duplicate plan、LastError ordering、VOID／EAX preserve、callee cleanupを
固定する。Focused native evidenceはBGIのFALSE callとlive-window TRUE／FALSE oracleを分離し、
`GWL_EXSTYLE` transitionとWindowsがsentinel LastErrorを0へclearすることを比較する。Node／Chromeはprotocol
version 20のread-only window snapshot、zero guest writes、semantic／memory digest、次endpointを一致させる。
Browser drag listener、`HDROP`、`WM_DROPFILES`、file permission、VFS importはこのleafのacceptanceへ含めない。

Reached `Sleep(0)` leafはone-argument x86 stdcall、VOID／EAX preserve、LastError preserve、guest write zeroと、
Rust-owned `yield-current-thread` execution boundaryを固定する。Scheduler model testsはrunnable competitorがある時の
immediate round-robin selectionとquantum reset、競合なしのcurrent-lane継続、callback non-interleaving、stale／duplicate／
runnable-set mismatch rejectionを検査する。Dispatcher negative testsはnonzero intervalとdisabled policyをruntime／memory／
scheduler mutation前にspecific faultへする。Focused x86 native oracleはmain／workerを同じCPUへpinし、ready handshake後の
8 callすべてでworker counterが進み、8個のsentinel LastErrorが保存されることを独立2 runで要求する。Primary callerは既存の
二つのraster-status trace summaryがouter loopの`Sleep(0)` yieldに一致することをconsumer evidenceとする。Node／Chromeは
240 completionsと240 scheduler selections、first／last sequence・instruction、`8192 -> 8193` relation、host wait zero、semantic／
memory digest、次の`GlobalMemoryStatus` stopを一致させる。Positive interval、wall-clock delay、no-competitor timing、priority-weighted
selection、preemptive fairnessは実装済みとしない。Worker transport、memory topology、browser-resource lifetime、graphics／audioを
変えないbounded scheduler leafなので、直前のversioned archive workloadをrisk evidenceとして保持し、current Sleep workloadの
performance measurementへ読み替えない。

Reached `GlobalMemoryStatus` leafはone-argument x86 stdcall、32-byte `MEMORYSTATUS`の一括write、
VOID／EAX preserve、LastError preserveを固定する。Policy／dispatcher testsはnative legacy saturation fields、
`systemInfo` application spanからのtotal virtual導出、NULL／read-only／cross-page／disabled／malformed policyの
mutation-free rejectionを検査する。Focused native evidenceは二つのbalanced run、前後16-byte guard、sentinel
LastError `0`／`87`／`0x12345678`、callerがoffset 8／12のphysical fieldsだけを読みsigned maximumへclampする
relationをgoldenにする。Host依存のmemory load／available virtualとVOID後のEAX数値は比較しない。Node／Chromeは
一completionのidentity、exact output、policy／virtual-span authority、host／browser memory non-import、semantic／memory
digest、次の`GetFileAttributesA` stopを一致させる。`GlobalMemoryStatusEx`、dynamic pressure、runtime allocator quota、
Wasm linear-memory capacityは実装済みまたはacceptance済みとしない。

Reached `GetFileAttributesA` leafはone-argument x86 stdcall、policy-owned regular `0x20`／directory `0x30`、
success LastError preserve、missing final componentの`0xFFFFFFFF`／2、missing parentまたはmount escapeの
`0xFFFFFFFF`／3、guest write zeroを固定する。RuntimeFiles model testsはWindows-932／ASCII-case-insensitive path、
current-directory relative resolution、registered source／existing writable file、mount root／registered-path ancestor
directory、absent writable objectを検査する。Dispatcher testsはNULL／malformed／disabled policyとresolution failureで
VFS snapshot、allocator、guest bytes、LastErrorが部分更新されないことを要求する。Focused native evidenceは独立2 runの
111 calls、regular-file／directory oracle、success sentinel preserve、primary callerのinvalid-result後のarchive／directory bit
testをgoldenにする。Node／Chromeは23 completions、3 missing-file／20 missing-path classifications、first／last identity、
Rust VFS authority、host／browser filesystem non-import、semantic／memory digest、次の`CreateDirectoryA` stopを一致させる。
Wildcard、reparse point、general DOS attributes、host filesystem enumeration、general directory mutationは実装済みとしない。

Reached `CreateDirectoryA` leafはtwo-argument x86 stdcall、NULL security attributes、一件のallowlisted initially-absent
writable-profile directory、success BOOL／LastError preserve、guest write zeroを固定する。Native oracleは独立2 runで
success、duplicate `ERROR_ALREADY_EXISTS` 183、missing parent `ERROR_PATH_NOT_FOUND` 3、regular-file conflict 183、
cleanup、source hash不変を検査する。RuntimeFiles model／dispatcher testsはdirectory identity／generation、parent／file
classification、mutation-free plan、host-first SQLite publication、stale／duplicate／malformed completion、storage failure、
cancel、process exitでのfailure atomicityを要求する。SQLite／Worker testsはprofile schema version 3、epoch lease、CAS、
idempotency ledger、transaction rollback、terminal drainを検査する。Node／Chromeは一completionのrequest／directory／
generation identity、BOOL／LastError、zero guest writes、Rust／SQLite authority、semantic／memory digestを一致させる。
その後の`SetWindowPos` repeat-activation leafは別contractとして完了済みである。General directory create／enumeration／delete／rename、ACL、
host filesystem mutation、primary save/loadは実装済みとしない。

W3 representative browser contractは、このW2 harnessが成功後にemitする`vine.browser-present` version `1` eventをexactly 1件だけ受け取る。payloadはframe `2x2`、`rgba8` sparse pixel `(1,1) = [255,0,0,255]`である。one-shot Canvas 2D sinkはschema／version／payloadを検査して1回だけ`putImageData`し、clean temporary Chrome profileでpresent count `1`、Canvas `2x2`、`getImageData` readback `[255,0,0,255]`をassertする。これはbrowser-facing pixel boundaryの代表test 1本であり、guest execution／Worker protocol、production surface／queue／renderer、frame timing、visible monitor output、general pixel conversionを代用しない。

後続のChrome Runtime Worker contractはW3とは別に、controlled PE32のguest CPUをWorker内で実行し、generic trap、
deferred host completion、nested callback、automatic writeback、Canvas／Web Audio／IndexedDB、immutable source range read、
writable profile read／commit transport、pending cancel、watchdog、thread／process return sentinelを通す。Source fixtureは
Rust-owned manifest／handle／cursorとsame-origin title／snapshot／object adapterを通り、二つのrange payloadとexit codeを
guest自身とrunnerの両方で検証する。Versioned protocolは`thread-switch`／`thread-exit`を含むclosed
execution-state graphとper-thread opaque CPU context、callback completionから同depthのreplacement callbackへのtransition、
Rust-owned logical windowとGDI+ session completion metadata、versioned writable commit completion、normal capability completion、
versioned waitable-timer pacing／page lifecycle、read-only `DragAcceptFiles` window snapshot、Rust-selected immediate API
execution-boundary disposition、typed `GlobalMemoryStatus`／`GetFileAttributesA` snapshots、writable-profile
version 3 directory-create request／completionに加え、
compact／summary／full evidence levelを明示する。

Capability completionはrequired／observed count、last sequence／instruction、次のowned endpoint、final memory digestを持ち、
`firstUnsupported`とは相互排他的である。通常のprivate gateはcompactまたはsummaryを使うが、全callを外部可観測fieldへ
正規化し、予約sequence順の`sha256-canonical-json-v1`をNode／Chromeで独立計算して一致させる。Digest自身がnext sequenceと
bounded pending mapでduplicateを拒否し、別のcompleted-sequence collectionをauthorityにしない。ASCII canonical fragmentの
direct SHA入力とnon-ASCII UTF-8 fallbackは同じalgorithm hashをUnicode／optional-field／guest-write projection testで固定する。
Handler ID等の内部fieldはdigest対象外で、詳細call／writeback／host-delta列は明示的なfull診断だけに残す。Compact／summaryでも
sequence identity、call count、storage completion、memory digest、terminal evidenceを検査し、Windows stateをTypeScriptへ
複製しない。

Initial threadの通常host wait中にsecond guest threadがmarker write／exitし、その後initial contextへ復帰すること、timer pacing中は
single emulated CPUをholdしてRust completion ownerを一度だけ復帰すること、replacement時にretired frame identityを照合することを
要求する。State contractはillegal transitionのmutation前拒否、wrong-thread／duplicate／cancelled-late completion、nested sync APIと
callback-resuming async policy、全terminal pathの`stopped`／pending request record zero、watchdogのthread／request／last-transition
contextを検査する。Engine contractはactual WasmでCPU contextのmutation-free生成／保存／復元、structured stop、mapping operation、
owner-tagged changed batch、cache effect、pinned revision／ABI／artifact mismatchを検査し、Runtime Worker sourceにPTE／private offset／
v86 reasonがないことを固定する。Current measurementは[current-checkpoint.json](current-checkpoint.json)だけを正とする。これは
`AC-P1-03`～`AC-P1-05`、`AC-P1-07`、`AC-P1-10`とstorage／logical-window ingressへのpartial evidenceであり、general Win32
thread creation、wait-any／wait-all、general host-elapsed pacing／fairness、primary save/restart/load、full-play title behaviorを代用しない。

private BGI Worker contractはactive generic profileをChrome内Rust/Wasm authorityで実行する。Nodeは
oracle reportだけを生成し、Chrome側のprocess／module／heap／dispatcher／memory stateを所有しない。
Ordered completion、source-read request／cursor／payload identity、final memory content、next structured faultを比較し、cancel／watchdogでprocess exit、
module detach、pending work zeroを要求する。別のversioned workload gateは同じWorkerでsessionを反復し、
latency／slice／heartbeat distribution、main／Worker heap、backing storage、Rust／execution-engine linear
memory、Wasm crossingをthreshold判定する。必要sampleが取得不能ならfailし、current resultはmanifestだけへ
保存する。各sessionはcall countだけでなくdigest algorithmとsemantic SHAもmanifestへ照合する。Threshold
failureまたはunchanged confirmationでも再発するenvironment invalidationはretryやlimit変更で合格化せず、
exact failure、artifact、試した切り分け、
保持するinvariant、必要権限、再開条件をstructured blockerとして同じmanifestへ残す。Host負荷を利用者の
許可なく停止せず、同じfixtureをisolated hostで再測定しても失敗する場合だけWorker profileとbounded
optimizationへ進む。Methodologyとnon-claimsは[bgi-browser-worker.md](bgi-browser-worker.md)および
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

Trapless continuation profileはprofile全体のinstruction／wall-clock上限を持ち、Rustがservice可能なpage faultを
同じwindow内で処理して次のruntime-owned endpointまで再開する。Node／Chromeのslice sizeとreconciliation
generationは同一でなくてよいが、旧bounded stopを越えたこと、terminal instruction、next endpoint、ordered call
digest、final memory digestはexactに一致しなければならない。Chromeのprogress statusは全observed countを残しつつ
先頭と最新を含む最大4,096件へbounded化し、summary reportの4 MiB上限を緩めない。Window exhaustion、
unserviceable fault、cancel、watchdogはterminal stateとpending work zeroを検証する。

Timer-heavy profileのbrowser scheduling remediationは、guest-visible logical deadline／timer／thread authorityを
変えない場合だけ認める。Task driver、yield coalescing、Wasm dispatch fast pathを変更した時はNode／Chromeの
ordered semantic digestとfinal memory digestをexactに再確認し、cancel／watchdog／pending host waitの即時境界を
保持する。Valid environmentでminimum sessionsまたはsession P95がversioned thresholdを外れた場合、他の
thresholdが通過しても`threshold-blocked`とする。Zero-delay diagnosticやinvalid-environment runはacceptance
evidenceへ昇格させない。

このprivate 180秒gateのbrowserは、public acceptanceと同じChrome for Testing
`151.0.7922.34`を使い、versionだけでなくofficial archiveとexecutableのSHA-256をtracked manifestで
固定する。実行前sampleから終了まで、別processがUpdater lifecycle、system／Chrome-tree／external CPU、
physical disk throughput／busy、Chrome PIDごとのpriority class／priority boost／power throttlingを監視する。
Environment observation schema version 2はexternal processをPID＋creation timeで追跡し、CPU delta上位8件と
process counterで説明できない残差を各sampleへ保存する。Benchmark reportは実行したmonitor scriptのbytes／
SHA-256も固定し、aggregate external CPUだけを残す再現不能な診断を禁止する。
監視欠測、Updater、versionedな持続CPU／disk excursion、同一processのQoS変化はworkload threshold failureと
混ぜず`invalid-environment`として破棄する。Heartbeat P99 25 msを含む17 thresholdは不変である。
先行するwaitable-timer pacing profileは同一fixture／thresholdでworkloadとenvironment qualificationを
通過し、main-thread releaseまでのsemantic checkpointを閉じた。後続continuationはRust schedulerの
logical thread selection、critical-section lifecycle、per-thread stack growthを保持したままbounded guest
executionを次のowned endpointまで進める。旧instruction-budget stopを越えるprofileでもbudget自体を
無制限化せず、page-fault serviceとterminal endpointのorderingをNode／Chromeで一致させる。
Exact current値、report identity、attempted options、次endpointはmanifestだけを正本とする。

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
 -> same-origin localhost server start
 -> local fixture / actual asset import
 -> source hash verification
 -> launch
 -> canvas first frame
 -> input replay
 -> audio activation / output counters
 -> save
 -> runtime stop, browser context close, and server restart
 -> reopen from committed generation
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

Performance acceptance時は測定対象外の継続CPU／GPU workloadも記録し、versioned workloadが固定thresholdを
外れたhostをclean referenceとして扱わない。外部workloadを停止する権限がない場合はstructured blockerとし、
同一duration／thresholdのisolated rerunを再開条件にする。

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

Current public CI implements this split as static semantic validation, Rust tests,
one reproducible fixture／adapter producer, core fixture E2E, browser E2E, performance
observation, fuzz smoke, and a public Windows native-oracle job. Required browser
acceptance is fixed to Chrome for Testing `151.0.7922.34`; moving Chrome stable is a
separate non-blocking compatibility observation. The fuzz smoke uses three bounded
input targets for PE planning, CPU writeback decoding, and source-read response
decoding, while the weekly workflow gives each target a separate 30-minute campaign.
Private title execution and native trace capture remain local because the corpus is
not distributable.

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

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
現在のproduction memory／entry／return／PE header由来initial stack reserve／commit／guard growth／minimum TEB・PEB／process-parameters／bidirectional binary memory sync／controlled private-heap backing／BGI firstからseventeenth-call ingress、controlled Chrome Worker、version 1 browser performance gateは`AC-P1-01`～`AC-P1-08`、`AC-P1-10`へのpartial evidenceである。frozen controlled CPUはprimary titleの17 API return、separate runtime-kernel profileは46 contracts／284 callsまで進み、Node／Chromeは4回のstack growth、CP_ACP NUL終端変換、process current-directory generation、named mutex metadata、instruction 25,256の`GetCurrentThread` stopでsemantic一致した。Public fixtureはbrowser callback serviceとpixel／audio／persistence、synthetic CPU／ABI／memory thresholdを通したが、additional-thread stack／overflow、adopted ISA suite、general process／thread／blocking-wait lifecycle、broader PEB・TEB、host timeout、broad guest heap APIとtitle capacity／system-DLL image／snapshot、SEH dispatch／filter invocation、full title browser workloadを欠くため各criterionと`AC-P1`全体は未完了である。
<!-- END FROZEN HISTORICAL EVIDENCE -->

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

<!-- BEGIN FROZEN HISTORICAL EVIDENCE -->
現在のimport-binding／BGI ingressは`AC-P2-01`、`AC-P2-06`、`AC-P2-11`へのpartial evidenceである。Frozen startup semanticsとprocess-owned dynamic endpoint identitiesを保持し、generic dispatcherは到達済みheap／TLS／FLS／LastError／NLS／handle／synchronization／file／window／COM／D3D9 operationsを同じRust authorityへ接続する。`LoadLibraryA`はその境界へpolicy-bounded callback-free runtime-system moduleを追加し、logical module identity、typed object、thread reference、attach state、export lookupを一つのpreflight／commit pathで処理する。Browserはordered completion metadataを観測するだけで、Windows module stateを所有しない。ただしadditional-thread stack／overflow、broad heap／critical-section／mutex blocking・abandonment・guest consumer APIs、SEH dispatch／filter invocation、general relative-path／VFS state、general DLL search／guest image load／unload、broad API coverage、ordinal fixture、full export／forwarder／DLL graph、general bootstrap／executionを欠くため各criterionと`AC-P2`全体は未完了である。
<!-- END FROZEN HISTORICAL EVIDENCE -->

### AC-P3: Win32 substrate and persistence complete

- `AC-P3-01`: handle／object、thread、critical section、interlocked、TLS slot／cleanup、event／mutex／semaphore／wait、clock／timer の採用 contract tests が通る
- `AC-P3-02`: User message queue の post／send／peek／get／dispatch、timer、callback の deterministic fixture が通る
- `AC-P3-03`: Windows path、case-insensitive lookup、overlay、sharing の採用範囲を differential test で満たす
- `AC-P3-04`: Windows-932／Unicode の filename、API string、registry value round-trip が通る
- `AC-P3-05`: save と virtual registry をlocalhost SQLiteへcommitし、browser／Worker／serverを停止、再起動、loadして同じ state hash を得る
- `AC-P3-06`: SQLite commit中のbrowser／Worker／server kill、quota full、permission revoke後も旧／新いずれかの整合したsave generationを読める
- `AC-P3-07`: 実title asset一つをread-only snapshotからOpen／Read／Seek／Closeし、同じsessionでsave一つをCreate／Write／Close、再起動、Open／Readでき、source hashとauthority境界が不変である
- `AC-P3-08`: IUnknown ABI、vtable thunk、HRESULT、interface identity、reference count の共通 foundation が fixture を通る
- `AC-P3-09`: renderer／surface の ADR-007 と DirectSound／AudioWorklet／clock の ADR-008 が Accepted
- `AC-P3-10`: managed module snapshot を restart 後も同じ hash で参照でき、外部 source replacement、overlay、quota failure で executable／DLL が shadow されない
- `AC-P3-11`: write／flush／close／SQLite commit の durability point と hard-stop rollback を kill injection で検証し、最後に commit 済みの state を保持する
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
