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
これはCPU context／stack／TEB、guest `CreateThread`、wait queue、last-thread semantics、DLL thread
callbackの代用ではない。詳細は[runtime-thread-registry.md](runtime-thread-registry.md)を正とする。

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
1024x1024／one-level／format 22に対するusage／pool `0／2`、`0／0`、`0x200／0`のversion-12 allow-list、
three distinct 4-byte texture pointer publications、complete 22-slot `IDirect3DTexture9` catalog、各child-held
device reference、private logical descriptor／indexを検証する。Invalid／unsupported／unwritable／capacity／
stale pathsはguest、COM、graphics stateをpartial mutationせず、failureはidentity allocatorやdevice refcountを
消費しない。Final texture releaseはlogical textureをretireして必要ならdevice／rootへcascadeする。
Slot 18は2-word／HRESULT ABI、level zero、4-byte surface pointer publication、complete 17-slot
`IDirect3DSurface9` catalog、child-held texture reference、private logical descriptor／indexを検証する。
Source textureはversion-12 variant index 0のstatic SYSTEMMEM descriptorに限定し、二DEFAULT variantは
guest／COM／graphics mutation前のtyped unsupportedを検証する。
Repeat取得は同じsurface／IUnknown identityへAddRefし、invalid level／NULL／unwritable／capacity／stale pathは
guest、COM、graphics stateをpartial mutationしない。Final surface releaseはlogical surfaceをretireして
必要ならtexture／device／rootへcascadeする。Surface slot 15／16は1-word／HRESULT ABI、4-byte HDC writeと
exact surface／HDC／thread release relationを検証する。Repeat acquisition、wrong／repeated releaseは
`D3DERR_INVALIDCALL`かつfailure-atomicで、live HDC中のsurface COM retirementを拒否する。Reached
`GetStockObject(4)`／`FillRect`はtyped graphics-surface HDCを通り、GDI ownerとGraphics ownerのplanを両方
preflightして`1024x1024` surfaceへ`[0,0,800,600]`／COLORREF 0のbounded commandを一件だけcommitする。
Node／Chromeはsame ordered surface DC／fill／release、続く二texture creation、device refcount 4のsemanticsと
device slot 26 `CreateVertexBuffer` stopを比較し、browser renderer、texture／surface pixel storage、pitch、
Lock stagingが未生成であることを要求する。
COM structured-stop evidenceはRust ABI catalogの`explicitArgumentWords`をboundedなcapture幅とし、return
address、this pointer、全explicit argumentsを保持してprefix一致では済ませない。Native two-run oracleは
zero width／height／formatのresolution、remaining field preserve、non-NULL device、first reached device
method、nested AddRef relationに加え、focused surface callと直後の`GetDC` orderingを固定する。Controlled
i386 oracleはrepeat surface／IUnknown identity、textureとは別のIUnknown、`GetContainer` relation、native
invalid-level output zero化とNULL access violationを固定する。Vineは共通failure-atomicity contractを優先し、
invalid inputでoutputを変更しない。Surface DC oracleはrepeat／reacquire、surface reference-count不変、
wrong／repeated `ReleaseDC`、live DC中の`LockRect`拒否、NULL／read-only output exceptionを固定する。
Focused initial-texture traceは三callのexact caller RVA／arguments／S_OK、same device、distinct non-NULL output、
直後の`CreateVertexBuffer`をtwo-runで固定する。詳細は
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

最初のvertical sliceのsource halfは完了している。Rust modelはmount escape、Windows-932-aware case identity、share conflict、stale manifest plan、wrong／duplicate completion、cursor、guest-memory failure atomicity、pending close、process-exit cancellationを検証する。Public PE32 fixtureは`CreateFileA -> GetFileSize -> ReadFile -> SetFilePointer -> ReadFile -> CloseHandle`を自己検証し、clean-profile Chromeはsame-origin success、pending cancellation、watchdog cleanupを通す。Private targetはfingerprint済みasset一つを二つのfile lifetimeで全量readし、Node／Chromeのordered semanticsと実行前後hash不変を比較する。

一つのsave fileのCreate／Write／Seek／Close、browser／Worker／server restart、SQLite durability、kill injectionは未完了である。API breadth、複数save slot、general mapped fileもsource halfからclaimしない。詳細は[runtime-vfs.md](runtime-vfs.md)を正とする。

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

W3 representative browser contractは、このW2 harnessが成功後にemitする`vine.browser-present` version `1` eventをexactly 1件だけ受け取る。payloadはframe `2x2`、`rgba8` sparse pixel `(1,1) = [255,0,0,255]`である。one-shot Canvas 2D sinkはschema／version／payloadを検査して1回だけ`putImageData`し、clean temporary Chrome profileでpresent count `1`、Canvas `2x2`、`getImageData` readback `[255,0,0,255]`をassertする。これはbrowser-facing pixel boundaryの代表test 1本であり、guest execution／Worker protocol、production surface／queue／renderer、frame timing、visible monitor output、general pixel conversionを代用しない。

後続のChrome Runtime Worker contractはW3とは別に、controlled PE32のguest CPUをWorker内で実行し、generic trap、deferred host completion、nested callback、automatic writeback、Canvas／Web Audio／IndexedDB、immutable source range read、pending cancel、watchdog、thread／process return sentinelを通す。Source fixtureはRust-owned manifest／handle／cursorとsame-origin title／snapshot／object adapterを通り、二つのrange payloadとexit codeをguest自身とrunnerの両方で検証する。protocol version 7は`thread-switch`／`thread-exit`を含むclosed execution-state graphとper-thread opaque CPU context、callback completionから同depthのreplacement callbackへのtransition、Rust-owned logical window completion metadataに加え、summary／full evidence levelを明示する。通常のprivate gateはsummaryを使うが、全callを外部可観測fieldへ正規化し、予約sequence順の`sha256-canonical-json-v1`をNode／Chromeで独立計算して一致させる。Digest自身がnext sequenceとbounded pending mapでduplicateを拒否し、別のcompleted-sequence collectionをauthorityにしない。ASCII canonical fragmentのdirect SHA入力とnon-ASCII UTF-8 fallbackは同じalgorithm hashをUnicode／optional-field／guest-write projection testで固定する。Handler ID等の内部fieldはdigest対象外で、詳細call／writeback／host-delta列は明示的なfull診断だけに残す。Summaryでもsequence identity、call count、source completion、memory digest、terminal stopを検査し、Windows stateをTypeScriptへ複製しない。initial threadのhost wait中にsecond guest threadがmarker write／exitし、その後initial contextへ復帰すること、replacement時にretired frame identityを照合することを要求する。State contractはillegal transitionのmutation前拒否、wrong-thread／duplicate／cancelled-late completion、nested sync APIとcallback-resuming async policy、全terminal pathの`stopped`／pending request record zero、watchdogのthread／request／last-transition contextを検査する。Engine contractはactual WasmでCPU contextのmutation-free生成／保存／復元、structured stop、mapping operation、owner-tagged changed batch、cache effect、pinned revision／ABI／artifact mismatchを検査し、Runtime Worker sourceにPTE／private offset／v86 reasonがないことを固定する。Current measurementは[current-checkpoint.json](current-checkpoint.json)だけを正とする。これは`AC-P1-03`～`AC-P1-05`、`AC-P1-07`、`AC-P1-10`とimmutable-source／logical-window ingressへのpartial evidenceであり、general Win32 thread creation、wait-any／wait-all、timeout／fairness、writable persistence、full-play title behaviorを代用しない。

private BGI Worker contractはactive generic profileをChrome内Rust/Wasm authorityで実行する。Nodeは
oracle reportだけを生成し、Chrome側のprocess／module／heap／dispatcher／memory stateを所有しない。
Ordered completion、source-read request／cursor／payload identity、final memory content、next structured faultを比較し、cancel／watchdogでprocess exit、
module detach、pending work zeroを要求する。別のversioned workload gateは同じWorkerでsessionを反復し、
latency／slice／heartbeat distribution、main／Worker heap、backing storage、Rust／execution-engine linear
memory、Wasm crossingをthreshold判定する。必要sampleが取得不能ならfailし、current resultはmanifestだけへ
保存する。各sessionはcall countだけでなくdigest algorithmとsemantic SHAもmanifestへ照合する。Threshold
failureはretryやlimit変更で合格化せず、exact failure、artifact、試した切り分け、
保持するinvariant、必要権限、再開条件をstructured blockerとして同じmanifestへ残す。Host負荷を利用者の
許可なく停止せず、同じfixtureをisolated hostで再測定しても失敗する場合だけWorker profileとbounded
optimizationへ進む。Methodologyとnon-claimsは[bgi-browser-worker.md](bgi-browser-worker.md)および
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

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
