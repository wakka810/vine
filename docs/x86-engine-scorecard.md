# x86 execution engine scorecard

## 1. 文書情報

- ステータス: Verified adapter feasibility。`ADR-002` は未決定
- 基準日: 2026-07-10
- 対応 task: `P0-ARCH-02`
- canonical commands: `npm run check:engine`、source build 後の `npm run check:engine:adapter`
- 現在の結論: v86 の bounded interpreter fork は成立。production engine としては未採用

## 2. 結論

4 候補のうち、現時点で browser／Wasm artifact をそのまま実行し、必要な i386、x87、SSE の最小経路を確認できたのは v86 だけである。v86 `0.5.424` の fallback Wasm を PC として boot せずに初期化し、制御した 32 bit 命令列を実行できた。debug Wasm では CPL3 の `UD2` を host exception hook へ trap できた。

ただし、release／fallback artifact には次の 2 機能がない。

1. caller が指定した命令 budget 以内で必ず戻る export
2. CPL3 exception を Win32 thunk dispatcher へ渡す production 用 hook

固定 source revisionへ bounded-execution patch と state-module overlay を適用し、Rust 1.97.0 から release fallback を再 build した。追加 import なしの `vine_execute_budget` により同一 page の無限 loop を 0、7、5 命令で正確に分割でき、CPL3 の `#DE`、`#UD`、`#GP`、`#PF` と CPL0 halt を structured stop reason として取得できた。さらに version 1、640 bytes の CPU state ABI で GPR、flags、segment／control state、x87、XMM／MXCSR を save／reset／restore／resume できた。

これにより bounded execution、release fault stop、thread state round trip の実装可能性は確認できた。続く fixed T1 slice では、release adapter上でruntime-owned `UD2` thunk、cdecl host service、1段guest callback、return sentinelも通った。それでも `ADR-002` は Accepted にしない。4 GiB virtual address／physical memory ownership、広いABI／exception、Chromium Worker、ISA correctness／性能が未解決だからである。

## 3. 評価条件

### 3.1 Phase 1 の必須条件

- browser の Wasm32 で動作する
- PE32／i386 user mode と CPL3 fault semantics を表現できる
- integer、flags、control flow、stack、string、x87 と必要な SSE 系を検証可能である
- instruction budget、trap、halt、fault の理由を host へ返して停止できる
- register、FPU／XMM、EIP、flags、segment／CPL state を thread ごとに save／restore できる
- 4 GiB guest virtual address と bounded physical memory の構成案を持てる
- API thunk、callback return sentinel、breakpoint を title code の変更なしで捕捉できる
- interpreter path を correctness baseline とし、optimized path を後から比較できる

### 3.2 この scorecard でまだ評価しないもの

- サクラノ詩 workload の実用速度
- 全 i386／x87／SSE 命令の正確性
- Windows exception、SEH、thread scheduling の完成度
- JIT の採用可否
- production language の最終決定

## 4. 調査 snapshot

| Candidate | Inspected revision／artifact | 観測した実装形態 |
|---|---|---|
| v86 | source `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f`、npm `0.5.424` | Rust CPU core、Wasm interpreter fallback、generated-Wasm JIT、browser frontend |
| Unicorn | `7c5db94191defc1e04a4f66f4eb1220903cba837` | QEMU TCG 系 core、C API、memory／code hook、bounded run API |
| Bochs | `254568cbcebeae6eeb99d5c7fe65f6098da0957c` | C++ interpreter、広い x86 ISA、full-PC device model、instrumentation |
| libx86emu | `ce81129c57fdfbf690bebc210a8db97d926cc5e7` | 小規模 C interpreter、memory／I/O hook、instruction hook、bounded run API |

revision は 2026-07-10 に shallow clone して読んだ source snapshot である。canonical check は clone を要求せず、lockfile で固定した v86 npm artifact だけを使う。

## 5. 比較結果

| Criterion | v86 | Unicorn | Bochs | libx86emu |
|---|---|---|---|---|
| Browser／Wasm evidence | Strong。prebuilt browser Wasm と frontend がある | Blocked。調査 revision に再現済み browser／Wasm backend がない | Partial。portable interpreter だが調査 revision に browser build がない | Partial。小規模 C で移植余地は高いが Wasm build は未実証 |
| i386 user-mode ISA | Strong candidate。protected mode、paging、CPL を持つ | Strong native API candidate | Strong accuracy candidate | Basic 32 bit と protected mode の一部 |
| x87／SSE | x87 と SSE smoke 成功。広い正確性 test は未実施 | QEMU backend は広いが Wasm path が未成立 | 広い x87／SSE family を構成可能 | upstream README が FPU／MMX／instruction extension 未対応と明記 |
| Bounded execution | Source adapter で exact budget を実証。upstream artifact の公開 `main_loop` には未搭載 | Strong。`uc_emu_start` に count／timeout がある | Partial。CPU loop へ adapter が必要 | Strong。max instruction、timeout、stop API がある |
| Memory adapter | Gap。通常 RAM は内部 contiguous buffer、MMIO のみ JS hook | Strong API。map／protection／invalid access hook がある | Partial。CPU と physical memory／device 層の分離作業が必要 | Strong API。permission、page mapping、全 memory access hook がある |
| API／exception trap | Source adapter の release buildでstructured fault stopを実証。fixed T1のexact owned thunk／1段callbackはVerified、SEH／broad ABIは未実装 | Strong。code／block／memory／invalid access hook がある | Partial。instrumentation はあるが adapter 化が必要 | Strong。per-instruction、interrupt、I/O hook がある |
| Thread state／snapshot | Verified adapter baseline。version 1、640-byte CPU-only state の round trip が通る | Strong。context save／restore API がある | Strong full-PC state、CPU-only schema は要抽出 | Partial。clone と公開 state があるが必要 ISA state が不足 |
| Browser integration cost | Lowest observed。ただし PC coupling の除去が必要 | Very high until Wasm execution backend is proven | High。C++ full-PC coupling と build size が課題 | Low codebase cost、high ISA implementation cost |
| Current disposition | Bounded adapter candidate。memory／broad ABI／browser gate 待ち | Parked | Fallback comparison | Not viable as primary engine |

Unicorn の API 形状は Vine の adapter contract に最も近い。しかし native で使いやすいことと browser Wasm で成立することは別であり、調査 revision からは TCG 実行経路を browser 向けに再現できていない。libx86emu は制御 API が良い一方、サクラノ詩に必要になり得る x87／SSE baseline を満たさない。Bochs は correctness の比較候補だが、最初の integration cost が v86 より大きい。

## 6. v86 の再現実験

### 6.1 Artifact contract

`npm run check:engine` は次を lockfile から解決して検査する。

| Artifact | Size | SHA-256 | Imports | Exports |
|---|---:|---|---:|---:|
| `v86-fallback.wasm` | 2,113,455 bytes | `0859dc359024ee2ac04a1eade930043065a86a1d041d9c5115669ab3eba7cbed` | 22 | 804 |
| `v86.wasm` | 2,084,417 bytes | `aec2c16bb0a1618aa641bb44d9c0fe14681f8c1459fa08c32e3e0562020884e8` | 22 | 804 |
| `v86-debug.wasm` | 11,325,416 bytes | `e5407e92a85fa7b5c4b2a97729aed863f7976539f0708eec0beb551f25598eda` | 25 | 807 |

fallback import には timer、halt、I/O port、MMIO、JIT callback がある。通常 RAM は imported memory callback ではなく、Wasm 内で確保した contiguous buffer を CPU core が直接読む。`memory`、`rust_init`、`allocate_memory`、`reset_cpu`、`update_state_flags`、`full_clear_tlb`、`main_loop` が export されている。

debug artifact だけが `cpu_exception_hook`、`log_from_wasm`、`dbg_trace_from_wasm` を追加 import する。この差を check で固定し、debug-only trap を production capability と誤認しないようにした。

### 6.2 PC boot を迂回した fallback smoke

check は 2 MiB の guest physical memory を確保し、BIOS、PCI、VGA、disk image を初期化せずに次を行う。

1. reset 後の state を 32 bit flat protected mode、CPL0 に構成する。
2. `0x1000` に 89 bytes の制御 program を置く。
3. integer arithmetic と store、stack、`call`／`ret`、x87、SSE を実行する。
4. test sentinel の `hlt` で終了する。
5. register、EIP、instruction count、guest memory、host import calls を照合する。

| Evidence | Expected／observed |
|---|---:|
| Executed instructions | 21 |
| Final EIP | `0x1050` |
| Final EAX | `54` |
| Final ESP | `0x1ff000` |
| Integer store | `0x12345679` |
| Guest call result | `54` |
| x87 result | `2` |
| SSE result bits | `0x40000000` (`2.0f`) |
| MMIO／port I/O calls | 0 |

この結果は v86 CPU core を full-PC boot なしで呼べることを示す。CPL0 と `hlt` は test の終了 sentinel であり、Win32 user mode の実行方式ではない。

### 6.3 CPL3 exception trap smoke

debug artifact では state を CPL3 に構成し、`mov eax, 42` と `UD2` を実行する。`UD2` の invalid-opcode exception `#6` が `cpu_exception_hook` に 1 回到達し、hook が stop state と次の EIP `0x1007` を設定する。2 命令で停止し、EAX は `42` のまま保持される。

これは synthetic import thunk と callback return sentinel を予約 address range の `UD2` で表現し、fault EIP から thunk ID を引ける可能性を示す。title の code を書き換える方式ではなく、PE loader が IAT を runtime-owned thunk address へ解決する設計と対応する。

### 6.4 Source-built bounded adapter

[`spikes/x86-engine/`](../spikes/x86-engine/README.md) に固定 revision 用 patch、toolchain、build command、contract check を保存した。patch は interpreter へ残り budget を渡し、adapter 実行中だけ exception を stop reason に変換する。通常の v86 path は adapter 外で同じ動作を保つ。

2026-07-10 の source build:

| Property | Observed |
|---|---|
| Rust | 1.97.0、`wasm32-unknown-unknown` |
| LLVM／Clang | 22.1.8 |
| Source overlay | 2 patches + 1 tracked Rust state module |
| Adapter Wasm size | 1,882,836 bytes |
| Adapter Wasm SHA-256 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Imports／exports | 22／809 |
| Source build | approximately 24 seconds (Rust compile: 21 seconds) |

`npm run check:engine:adapter` は次を機械検証する。

- budget `0` で 0 命令、budget `7` で 7 命令、その続きの budget `5` で累計 12 命令
- CPL3 `#DE = 0x100`、`#UD = 0x106`、`#GP = 0x10d`、`#PF = 0x10e`
- page fault error code `4` と fault EIP
- CPL0 halt reason `2`
- CPU state ABI version `1`、size `640` と invalid magic の拒否
- GPR、x87、XMM state の save／reset／restore／resume 1 sequence
- MMIO、port I/O、generated-code callback が 0 回

`UD2` thunk は exception EIP を runtime-owned thunk table で分類すればよく、release artifact が JS exception hookを importする必要はない。W1 fixed T1 sliceはstop reason `0x106`とexact owned EIPの両方を要求し、cdecl host call、callback／entry return sentinelまで通した。

### 6.5 生成 evidence

prebuilt check は `.work/x86-engine-spike/report.json`、source adapter check は `.work/v86-adapter-spike/report.json` に machine-readable result を出力する。`.work/` は生成物なので Git 管理しない。

## 7. 成立した条件と未解決 blocker

### B1. Source build: Verified

Rust 1.97.0、`wasm32-unknown-unknown`、Clang 22.1.8 で upstream fallback と adapter fallback の両方を build できた。patch は固定 revision の clean clone に `git apply --check` で再適用できる。compiler version 間の byte-for-byte hash 一致は要求せず、source revision、toolchain、Wasm contract、behavior report を固定する。

### B2. Caller-defined instruction budget: Verified baseline

upstream `main_loop` は内部 `LOOP_COUNTER = 100003` を使い、fallback interpreter の同一 page loopは `INTERPRETER_ITERATION_LIMIT = 100001` を持つ。adapter patch は残り budget を interpreter block へ渡し、同一 page の backward loop でも超過 0 で戻る。

この baseline は JIT を無効化し、既存 compiled cache があれば clear する。optimized path の budget contract は別 gate とする。

### B3. Release structured fault stop: Verified baseline

upstream release／fallback artifact では `cpu_exception_hook` が compile out される。adapter patch は JS hook を追加せず、adapter 実行中の divide error、invalid opcode、general protection、page fault を `0x100 + vector` と error codeへ変換する。W1はfixed trap pageの4 owned addressだけを`#UD`とEIPで分類した。production Rust側にはstable ID／symbolを持つthunk tableが追加されたがCPU未接続であり、production trap classification、SEH delivery、breakpointはまだ実装していない。

### B4. Guest memory model

v86 の通常 RAM は contiguous guest physical memory である。一方、Vine は 4 GiB guest virtual address、reserve／commit／R／W／X、bounded physical memory を必要とする。2026-07-11の`ADR-003` spikeは次の2案を同じhigh-address mapで比較した。

- v86 の paging を使い、Vine region table を page table と bounded physical page allocator へ反映する
- v86 memory access を Vine の checked sparse memory interface へ差し替える

3,072-byteのfixed PE32 imageはimage base `0xF1000000`で、`.text` RX、`.rdata`／`.rprobe`／`.nxprobe` R、`.data` RWを持つ。2 MiB physical poolの15 pages／60 KiBをdirectory、3 page tables、11 mapped pagesへ割り当て、44 KiBのvirtual mappingからhigh EIP／stackを実行した。R write PFEC `7`、unmapped read／write PFEC `4`／`6`、cross-page writeのsecond-page fault／no partial write、PTE protect／remap後の`full_clear_tlb`効果はv86 pagingで成立した。

一方、fixed v86はinstruction fetchをread translationとして扱い、PEのnon-executable `.nxprobe`を実行した。unmapped fetchもPFEC `4`でinstruction-fetch bitを持たない。checked sparse referenceは同じpage metadataからnon-executable／unmapped fetchを`21`／`20`として拒否したがCPU未接続である。この結果から、Vine region table＋bounded physical allocatorをv86 pagingへ反映し、adapter-owned execute gateを追加する案をprovisional candidateとする。snapshot、guard、generated-code invalidation、Worker ceilingを閉じるまで`ADR-003`はAcceptedではない。詳細は[adr-003-guest-memory.md](adr-003-guest-memory.md)を正とする。

続くproduction Rust ingressはsparse owner／region／page state、bounded zeroed frames、logical execute seal、cross-page preflight、generation-tagged invalidation requirementを実装した。high fixtureの6 image pagesとBGI planの380 pagesを所有するが、v86 page tables／physical RAM／fetch pathへは未接続である。ownership resultは[guest-memory-ingress.md](guest-memory-ingress.md)を正とする。さらにcontrolled imageの2 IATを`0x7F000000` arenaのowned `UD2` thunksへbindしてimage／runtime ownerをsealした。generation `4`はactual adapter invalidation前のpending requirementであり、CPU execution evidenceではない。詳細は[pe-import-binding.md](pe-import-binding.md)を正とする。

### B5. Stable state ABI: Verified baseline

adapter は `vine_cpu_state_version`、`size`、`ptr`、`save`、`restore` を export し、host-directed register／thread save／restore fieldsをversioned 640-byte bufferへ置いた。buffer は magic、version、word count、GPR、EIP／flags、segment、control／debug register、descriptor state、x87、XMM／MXCSR を持つ。RAM setupとdiagnostic instruction counterはまだprivate linear-memory offsetに依存する。

contract check は 5 命令実行後に state を保存し、CPU reset 後に復元して残り 7 命令を実行する一つの sequence を使う。EAX は `10` から `15`、x87 result は `2`、XMM result は `4.0f` となり、halt へ到達した。これは thread state round trip の baseline であり、全 state combination の correctness 証明ではない。

### B6. Fixed T1 thunk／callback: Verified baseline

`npm run check:runtime:abi` は SHA-256 固定の `abi-smoke.exe` を8 MiB contiguous memoryへmapし、IAT 2 slotをruntime-owned `UD2` thunkへ解決する。private v86 offsetを使うためadapter Wasmもsize／SHA-256／imports／exportsで固定し、`microtick`以外のhost function importは呼ばれた時点で失敗する。instruction budget `3`のloopでcdecl `fixture_add`、guest `add_one` callback、callback return、entry returnを通し、result `43`、final EAX `0`、ESP `0x00700000`、budget stop 5回、累積21命令を確認した。

host-directed resumeはversion 1 state bufferのEIPとPREVIOUS_EIPを同時に更新する。EBX、EBP、ESI、EDIのsentinel、各trapのreturn address／argument／ESPも検証する。private v86 global offsetはfixed harnessのinitial CPU／RAM setupとinstruction counter観測に限定し、host-directed register stateにはstate v1 bufferを使う。

これはcdecl、1 callback depth、single threadのbaselineである。stdcall／thiscall／fastcall、nested callback、exception／SEH、wait／wake、async host responseの証明ではない。

### B7. Fixed W2 loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flow: Verified baseline

`npm run check:runtime:window-loader`はSHA-256固定の`window-pixel.exe`を同じ8 MiB contiguous memoryへpreferred-base mapし、3 DLL／13 named importを一意なowned `UD2` thunkへ解決する。`GetModuleHandleA(NULL)`に続き、fixed `RegisterClassExA` variantの48-byte structureとbounded ASCII nameをchecked readし、host-owned logical class stateとATOM `0xC000`を登録してstdcall resumeする。fixed `CreateWindowExA` variantは12 argumentsとmodule-scoped class lookupを検査し、harness-local typed HWND `0x57010001`と48-byte `CREATESTRUCTA`を作り、WndProc `0x00401260`の`WM_CREATE` callback／return sentinelを往復する。fixed `showwindow-sw-show-hidden-to-visible`はcreated／hidden objectとarguments `[HWND, 5]`を検査し、visibilityをtrueへ遷移して`showCommand=5`を保持し、previous visibilityのBOOL `0`をstdcall returnする。

fixed `invalidaterect-whole-client-no-erase`はarguments `[HWND, NULL, FALSE]`を検査し、symbolic whole-client update region、erase false、paint state queuedをwindowへ保持して、owner guest threadのgenerated-paint laneへcandidate `[HWND, WM_PAINT, 0, 0]`を1件追加する。BOOL `1`をstdcall returnする。これは直前のinvalidation checkpointで`GetMessageA`を次failureとして19 budget stop／26 execute stop／70命令まで進めたevidenceを保持する。

fixed `getmessagea-unfiltered-generated-paint-zero-metadata`はarguments `[0x00403034, NULL, 0, 0]`と28-byte writable MSG spanを検査し、`[HWND, WM_PAINT, 0, 0, 0, 0, 0]`を全書込する。candidate laneを1件から0件へdequeueし、paint stateを`queued`から`in-flight`へ遷移してregion／erase／sequenceを保持し、BOOL `1`をstdcall returnする。6 completed partial returnsの後、fixed `dispatchmessagea-generated-wm-paint-callback`はfixtureの`CALL ESI`をIAT `0x004020D8`／thunk `0x00500050`へ帰属し、MSGをchecked readしてWndProc `0x00401260`へ`[HWND, WM_PAINT, 0, 0]`を積む。

直前のmessage-dispatch checkpointはDispatch API ID `3`とcallback ID `4`をpendingにし、guestが`BeginPaint(HWND, 0x0040308C)`へ到達するまでの28 budget stop／37 execute stop／101命令を固定した。今回のcontinuationはfixed `beginpaint-whole-client-harness-surface`でharness-only bounds `[0,0,2,2]`、typed HDC `0x48010001`、64-byte PAINTSTRUCT、active paint sessionを作る。fixed `setpixel-harness-surface-exact-point`はlogical pixel mapへ`(1,1) = 0x000000FF`を1件書き、fixed `endpaint-matching-active-session`はPAINTSTRUCTを再検査してsession／HDCをretireし、paint stateを`none`へ戻す。

fixed `destroywindow-synchronous-wm-destroy`はDispatch API `3` -> `WM_PAINT` callback `4` -> Destroy API `5` -> `WM_DESTROY` callback `6`の4 frameを作る。fixed `postquitmessage-thread-slot-exit-zero`がthread quit slotへexit `0`を置き、callback `6`、API `5`、callback `4`、API `3`の順にreturnする。2回目のfixed `GetMessageA`は`CALL EDI`をowned thunkへ帰属し、MSG `[NULL, WM_QUIT, 0, 0, 0, 0, 0]`を全書込してreturn `0`、quit slotをconsumeする。fixtureはentry sentinelへEAX `0`、ESP `0x00700000`、callee-saved復元で到達する。13 completed partial returns、6 callback events、66 budget stop、83 execute stop、17 owned `UD2`、231命令である。

これはmultiple thunk resolution、checked structure／ASCII string／MSG／PAINTSTRUCT、copied class／window semantic state、2 guest callback level／4 frame unwind、fixed paint／destroy／quit、full fixture entry returnのbaselineである。HWND／HDC、2x2 client、pixel map、paint session、thread quit slot、call ID／frame layoutはfixed harnessの選択であり、native geometryやproduction object／surface／queue layoutを固定しない。general BeginPaint／EndPaint／DC／SetPixel、common surface／format／pitch／orientation／dirty region、general DestroyWindow／WM_NCDESTROY、message ordering／wait、arbitrary callback nesting、DOM／Canvas presentation、relocation、protection、sparse memory、browser Workerは未検証であり、production loader／ABI gatewayを示さない。`SetPixel`はtest-onlyでBGIのGDI requirementへ昇格しない。

### B8. Browser Worker

今回の実行環境は Node.js 24 の WebAssembly runtime である。Chromium の Runtime Worker、yield、termination、source map、cold start、memory ceiling は未検証である。

### B9. Correctness と performance

W1の21命令とW2の231命令の成功は ISA correctness suite でも VN-like benchmark でもない。Windows oracle と独立 vector を使う instruction conformance、compiler fixture、hot loop、callback、30 分 stress が必要である。

## 8. 次の adapter slice

現在の source adapter は exact-budget execution、exception detail、versioned CPU state、fixed W1／W2 flowを提供し、W3 browser first pixelも完了した。high-address memory spikeはv86 pagingのpresent／R／W／fault ordering／invalidationを実証し、execute permission gapを分離した。BGI load plan、production Rust region／page ownership、source copy／zero-fill／`HIGHLOW` materialization、supported-prefix IAT bindingも成立した。次のadapter sliceはprivate offset、fixture page table、2 MiB poolをruntime全体へ拡散させず、sealed controlled image、thunk arena、bounded stackとRust memory generationをv86 mapping／invalidation／fetch gateへ接続する。

```text
classify general STOP_THUNK / STOP_CALLBACK_RETURN / STOP_FAULT
map/protect/fault guest pages through the selected memory boundary
invalidate code range
```

adapter fixture は次の順に増やす。

1. [Fixed baseline verified] 最小Win32 fixtureで複数のowned thunk IDとguest window callbackを通した。後続のproduction sliceでgenerated API metadata／callback schemaを使うgeneral dispatcherへ移す。
2. [Measured] unmapped／read-only accessとcross-page faultを区別した。non-executableはcurrent v86 gapとして固定した。
3. [Production image／thunk ownership verified／adapter pending] Rust region／page generationをprovisional v86 paging、physical backing、execute gateへ接続し、actual invalidation後にentryからowned thunk ID `1`へ到達する。
4. broader stdcall signature、thiscall、nested callbackを実到達fixtureから追加する。
5. 同じfixtureをNodeとChromium Workerで実行する。
6. adopted ISA fixtureとnormalized workloadを測定する。

## 9. 採用 gate

すでに満たした gate:

- source から fallback adapter Wasm を再現 build できる
- caller-defined budget と structured stop reason が test で保証される
- release-size build で CPL3 fault stop が動く
- versioned CPU state が save／reset／restore／resume を通る
- fixed T1 fixture の exact owned API trap と cdecl guest callback が通る
- high-address mapping、R／W fault ordering、protect／remap invalidationのmeasurementがあり、native execute gapが明示される

`ADR-002` の採用前に残る gate:

- production Rust ownershipをv86 paging／fetchへ反映し、snapshotとactual generated-code invalidationを接続する
- Chromium Worker で correctness smoke と provisional benchmark が通る
- target workloadに必要なISA／ABI／exception bandを説明できる
- fork の変更範囲と upstream追従手順を説明できる

一つでも根本的に満たせない場合は、Bochs CPU-only extraction を同じ adapter fixture で測る。Unicorn は browser／Wasm execution backend を再現できた時点で再評価する。libx86emu は integer-only reference fixture には利用できるが primary engine にはしない。
