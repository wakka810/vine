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

upstream release／fallback artifact では `cpu_exception_hook` が compile out される。adapter patch は JS hook を追加せず、adapter 実行中の divide error、invalid opcode、general protection、page fault を `0x100 + vector` と error codeへ変換する。W1はfixed trap pageの4 owned addressだけを`#UD`とEIPで分類した。production Rust側のstable ID／symbol tableはcontrolled v86 entryでthunk ID `1`、fixed cdecl continuationでthunk ID `2`、separate control metadataでcallback return sentinel ID `1`とprocess-entry return sentinel ID `2`のexact EIPを分類した。outer return continuationはunmapped `0xDEADC0DE` fetchをreason `0x10E`、PFEC `4`、CR2／EIPで構造化した。one fixed service／callback／entry completionだけをdispatch済みで、general trap dispatch、process lifecycle、SEH delivery、breakpointはまだ実装していない。

### B4. Guest memory model

v86 の通常 RAM は contiguous guest physical memory である。一方、Vine は 4 GiB guest virtual address、reserve／commit／R／W／X、bounded physical memory を必要とする。2026-07-11の`ADR-003` spikeは次の2案を同じhigh-address mapで比較した。

- v86 の paging を使い、Vine region table を page table と bounded physical page allocator へ反映する
- v86 memory access を Vine の checked sparse memory interface へ差し替える

3,072-byteのfixed PE32 imageはimage base `0xF1000000`で、`.text` RX、`.rdata`／`.rprobe`／`.nxprobe` R、`.data` RWを持つ。2 MiB physical poolの15 pages／60 KiBをdirectory、3 page tables、11 mapped pagesへ割り当て、44 KiBのvirtual mappingからhigh EIP／stackを実行した。R write PFEC `7`、unmapped read／write PFEC `4`／`6`、cross-page writeのsecond-page fault／no partial write、PTE protect／remap後の`full_clear_tlb`効果はv86 pagingで成立した。

一方、fixed v86はinstruction fetchをread translationとして扱い、PEのnon-executable `.nxprobe`を実行した。unmapped fetchもPFEC `4`でinstruction-fetch bitを持たない。checked sparse referenceは同じpage metadataからnon-executable／unmapped fetchを`21`／`20`として拒否したがCPU未接続である。この結果と後続writeback evidenceから、Rust-owned Vine region／bytesをbounded v86 pagingへ反映し、adapter-owned execute gateを追加する案をAccepted baselineとする。snapshot、guard、Worker ceiling、performanceは後続gateであり、byte authority decisionとは分離する。詳細は[adr-003-guest-memory.md](adr-003-guest-memory.md)を正とする。

続くproduction Rust ingressはsparse owner／region／page state、bounded zeroed frames、logical execute seal、cross-page preflight、generation-tagged `MemoryDelta`を実装した。high fixtureの6 image pagesとBGI planの380 pagesを所有し、controlled imageの2 IATを`0x7F000000` arenaへatomic bindして両ownerをsealした。optional entry-return modeはseparate ownerのRX control pageへcallback／process-entry sentinelを置いた7 pagesをgeneration `9`でreflectし、actual invalidation後にone-level callback、outer cdecl return、exact entry completionまで進めた。初回snapshot後の通常data writeはdirty 1 pageだけをlive v86へ適用できる。逆方向は全writable PTE Dirty bitをslice後にscanし、binary multi-page batchをowner／generation単位でRust authorityへatomic reconcileする。general process mapping／snapshot、writable-page scan costは未接続／未判定である。詳細は[guest-memory-ingress.md](guest-memory-ingress.md)、[memory-delta-and-transactions.md](memory-delta-and-transactions.md)、[memory-writeback.md](memory-writeback.md)、[pe-import-binding.md](pe-import-binding.md)、[v86-entry-ingress.md](v86-entry-ingress.md)、[v86-cdecl-ingress.md](v86-cdecl-ingress.md)、[v86-callback-return-ingress.md](v86-callback-return-ingress.md)、[v86-outer-return-ingress.md](v86-outer-return-ingress.md)、[v86-entry-return-ingress.md](v86-entry-return-ingress.md)を正とする。

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

### B8. Production-memory entry: Verified ingress

`npm run check:runtime:entry`は同じlive Rust `GuestMemory`をprobe sessionに保持し、guest VA／owner／seal／R／W／X／4 KiB bytesだけをNode adapterへ渡す。Rust physical frame、v86 physical placement、private offsetはprotocolに含めない。Nodeは2 MiB poolの11 pagesへpage directory、3 tables、6 reflected pages、1 harness stackを配置し、state ABI restore、explicit TLB／generated-code clear後にgeneration `6`をacknowledgeする。

instruction budget `3`でentry `0x10001000`を実行し、2 execute stop／1 budget stopでproduction thunk ID `1`／`0x7F000000`のstructured `#UD`へ到達した。ESP `0x006FFFF0`、return `0x1000100A`、arguments `[19,23]`、bound IATを検証した。このentry-only reportではservice dispatch／resumeを行わない。詳細は[v86-entry-ingress.md](v86-entry-ingress.md)を正とする。

### B9. Production cdecl service: Verified ingress

`npm run check:runtime:cdecl`はordered production thunk metadataからexact thunk ID `1`を分類し、fixed frame `[return,19,23]`をchecked readする。version 1 CPU state ABIでEAX `42`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x1000100A`へresumeし、guest自身が2 argumentsをcleanupする。4 execute stop／2 budget stop後、exact thunk ID `2`／`0x7F000010`でreturn `0x10001019`、callback target `0x10001030`、value `42`を構造化した。targetはsealed executable pageに属し、callee-saved sentinelは保持された。

これはone fixed two-argument cdecl development serviceのevidenceである。general signature metadata、other calling conventions、LastError、callback frame／return sentinel、callback execution、outer service return、SEHは未実装である。詳細は[v86-cdecl-ingress.md](v86-cdecl-ingress.md)を正とする。

### B10. Production callback return: Verified ingress

`npm run check:runtime:callback`はseparate runtime owner `3`のRX control pageへsentinel ID `1`／`0x7F010000`を置き、generation `9`の7 production pagesをreflectする。frame ID `1`はparent thunk ID `2`、outer ESP／return、target／value、sentinel identityを保持する。checked stack ESP `0x006FFFE8`へ`[sentinel,42]`を積み、guest `add_one`を実行して6 execute stop／3 budget stop後にEAX `43`、ESP `0x006FFFEC`でexact sentinelへ戻った。

これはone-level callback entry／return trapのevidenceである。frame retirement、outer service return、general frame stack、nested／reentrant callback、adapter-to-Rust dirty synchronizationは未実装である。詳細は[v86-callback-return-ingress.md](v86-callback-return-ingress.md)を正とする。

### B11. Production outer return: Verified ingress

`npm run check:runtime:outer-return`はcallback frame ID `1`をretireし、outer cdecl thunk ID `2`からEAX `43`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x10001019`でguestへreturnする。guestは2 argumentsをcleanupし、`0x10003000 = 43`、entry EAX `0`を作り、final `RET`でESP `0x00700000`へ進める。9 execute stop／5 budget stop後、harness-only unmapped return `0xDEADC0DE`のfetchはreason `0x10E`、PFEC `4`、CR2／EIP／previous EIP `0xDEADC0DE`で停止した。

これはone fixed outer cdecl unwindとentry continuationのevidenceである。production-owned entry sentinel、process exit lifecycle、general frame stack、nested／reentrant callback、adapter-to-Rust dirty synchronizationは未実装である。詳細は[v86-outer-return-ingress.md](v86-outer-return-ingress.md)を正とする。

### B12. Production entry return: Verified ingress

`npm run check:runtime:entry-return`はcontrol owner `3`のslot `2`へprocess-entry sentinel ID `2`／`0x7F010010`を追加し、combined page SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`をgeneration `9`でsealする。initial ESP `0x006FFFFC`へexact addressを置き、9 execute stop／5 budget stop後にreason `0x106`、EIP／previous EIP `0x7F010010`、EAX `0`、ESP `0x00700000`、fixture result `43`でentry completionを分類した。outer modeのunmapped PFEC `4`は不変である。

これはtyped process-entry returnとbalanced fixed stackのevidenceである。initial stackはadapter-localで、Windows process／thread exit lifecycle、PEB／TEB、process parameters、general stack growth／guardは未実装である。詳細は[v86-entry-return-ingress.md](v86-entry-return-ingress.md)を正とする。

### B13. Production stack bootstrap: Verified ingress

`npm run check:runtime:stack-bootstrap`はanonymous owner `4`の`[0x006FF000,0x00700000)`をstack regionとしてRW commitし、initial ESPへentry sentinelをchecked writeしてgeneration `12`でsealする。8 reflected pagesのcontent SHA-256は`d7471bcc744e2f769861e1a40cc5520f5c0d452f2ea8539233a8573315b6f1cd`である。adapterは`harness-stack`をallocate／writeせず、page directory／3 tables／8 reflected pagesの12 pages／48 KiBで同じ9 execute stop／5 budget stop、EAX `0`、ESP `0x00700000`を通した。

これはcontrolled stack ownership handoffのevidenceである。fixed one-page stackはPE stack reserve／commit、guard／growth、multi-thread、teardown、v86-originated writeback、PEB／TEBの代用ではない。詳細は[v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md)を正とする。

### B14. Bootstrap FS first stop: Verified ingress

`npm run check:runtime:bootstrap-fs-first-stop`はnative WindowsでTEB self／PEB pointerを自己検証するimport-free fixtureをproduction Rustの7-page reflectionから実行する。FS selector／base `0`／`0`の最初の`FS:[0x18]`はreason `0x10E`、PFEC `4`、CR2 `0x18`で止まり、current／previous EIPは`0x10001007`／`0x10001000`となった。5,982-byte reportのSHA-256は`0276cc927a3d870b10a632cf95995e79ec96cd8af5558e964b7635d2d2b7941b`である。

これはnative FS offset contractとTEB未実装境界のevidenceであり、guest TEB／PEB、nonzero FS base、process parameters、TLS、LastErrorを実装しない。詳細は[v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md)を正とする。

### B15. Bootstrap FS completion: Verified ingress

`npm run check:runtime:bootstrap-fs`はtyped Rust TEB／PEB pagesをgeneration `15`でreflectし、versioned CPU stateのFS baseを`0x7FFDE000`へrestoreする。native-oracled 4 fieldsは一致し、9 execute stop／8 budget stop後にentry EAX `0`、ESP `0x00700000`でexact process-entry sentinelへ到達した。8,214-byte reportのSHA-256は`3567ff39474f72781010fdb810b793f58424483a7663b89f2cea4b73a28748b9`である。

これはminimum TEB／PEB field contractであり、process parameters、TLS、LastError、general selector／lifecycleを実装しない。詳細は[v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md)を正とする。

### B16. Process-parameters first stop: Verified ingress

`npm run check:runtime:bootstrap-params-first-stop`はseparate import-free fixtureをnative Windowsでself-oracleし、i386の`PEB+0x10`、`ProcessParameters+0x40`、`UNICODE_STRING.Buffer+0x04`からbounded command-line matchまでをexit code `0`で固定する。同じfixtureはminimum guest PEBのzero pointerを観測し、8 execute stop／7 budget stop後にentry EAX `2`、ESP `0x00700000`でexact process-entry sentinelへ到達した。8,351-byte reportのSHA-256は`b1c05c3e9df1529196276c8905f3b97aee306610c870461d458a857c80aadde5`である。

これはnative layoutとfirst unsupported pointのevidenceであり、process-parameters region、command-line bytes、broader PEB／process lifecycleを実装しない。詳細は[v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md)を正とする。

### B17. Process-parameters completion: Verified ingress

`npm run check:runtime:bootstrap-params`はtyped process-parameters／command-line pagesをgeneration `21`でreflectし、PEB pointer、Length `40`、MaximumLength `42`、Buffer、UTF-16 basename＋NULをack後にnormal readする。同じfixtureは94 execute stop／93 budget stop後にmatch index `0`、entry EAX `0`、ESP `0x00700000`でexact process-entry sentinelへ到達した。10,521-byte reportのSHA-256は`c1e76294d4bba6760cbc8d022e022ef66b557d370b567d9a498798b8b2f7ac66`である。

これはmeasured command-line support bandであり、general process parameters、loader／thread lifecycleを実装しない。BGI execution continuationは[v86-bgi-first-trap.md](v86-bgi-first-trap.md)で別に固定した。

`npm run check:runtime:bgi-first-trap`はopt-in 400-page Rust limit／8 MiB protocol-line limitでBGIの388 sealed pagesをgeneration `24`へreflectする。2 MiB adapter poolはv86のVGA MMIO hole `[0xA0000,0xC0000)`を除外し、directory 1、tables 4、reflected pages 388の393 allocationsを使う。4,904,602-byte offerのcontent SHA-256は`f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`である。

entry `0x100A383E`はinstruction budget `1`で全境界のlogical execute permissionを検査し、19 execute stop／18 budget stop／19命令でexact owned thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`／`0x7F000670`へ到達した。runnerはESP `0x006FFFD0`、return `0x100A88D2`、raw stack wordsを記録し、API dispatch／returnを行わない。9,592-byte reportのSHA-256は`4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840`である。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

### B18. MemoryDelta data roundtrip: Verified ingress

`npm run check:runtime:memory-delta`は初回4-page reflectionをacknowledgeした後、RustのRW／NX pageへ8 bytesを書き、generation `11`の1-page／4 KiB deltaだけを既存v86 mappingへ適用する。cache effectsは全てfalseで、guest x86はEDX:EAX `0x8877665544332211`を読んでexact `UD2`へ停止する。ack後のRust dirty pagesは0である。同じfixtureは実行可能page末尾からNX pageへ跨るinstructionを15-byte conservative gateで実行前に停止する。

これはhost-originated data outputを全snapshot再送なしでCPUへ返せるevidenceである。逆方向のautomatic writebackとbinary transportはB34で固定する。decode-aware execute span、general mapping lifecycleは未実装である。詳細は[memory-delta-and-transactions.md](memory-delta-and-transactions.md)を正とする。

### B19. First BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-first-win32-call`はID `104`のexact one-argument stdcall frame／IAT callを検査し、controlled UTC sampleを8-byte `FILETIME`として返す。outputがlive stack上にあるため、generation `24`／owner `4`のcurrent v86 pageをRustへreconcileしてからgeneration `25`のone-page deltaを発行する。11 guest-changed bytesを保持したfull page、zero mapping／cache effects、Rust acknowledgementを検査する。

stdcall return後4命令、entryから累計23命令でID `103`／`KERNEL32.dll!GetCurrentProcessId`へ到達する。14,540-byte reportのSHA-256は`7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16`である。これはone fixed page writeback／clock sample／API returnであり、general dirty tracking、clock、ABI gateway、process identityを実装しない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

### B20. Second BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-second-win32-call`はID `103`のzero-argument stdcall frame／IAT callをadapterとRustで検査し、owner `checkpoint-process-identity`のcontrolled PID `4096`をEAXへ返す。generation `25`のlive stack pageを再reconcileし、4 changed bytesをsemantic memoryへ取り込むが、host writeがないためgeneration `25`／dirty 0を保持する。

return後3命令、entryから累計26命令でID `149`／`KERNEL32.dll!GetCurrentThreadId`へ到達する。18,472-byte reportのSHA-256は`bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67`である。これはone controlled process identity／API returnであり、general process／handle／thread modelを実装しない。詳細は[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md)を正とする。

### B21. Third BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-third-win32-call`はID `149`のzero-argument stdcall frame／IAT callをadapterとRustで検査し、owner `checkpoint-thread-identity`のcontrolled TID `8192`をEAXへ返す。generation `25`のlive stack pageを再reconcileし、1 changed byteをsemantic memoryへ取り込むが、host writeがないためgeneration `25`／dirty 0を保持する。

return後3命令、entryから累計29命令でID `13`／`KERNEL32.dll!GetTickCount`へ到達する。22,383-byte reportのSHA-256は`d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa`である。これはone controlled thread identity／API returnであり、general process／thread／TEB client-ID／scheduler modelを実装しない。詳細は[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md)を正とする。

### B22. Fourth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-fourth-win32-call`はID `13`のzero-argument stdcall frame／IAT callをadapterとRustで検査し、owner `checkpoint-monotonic-clock`のcontrolled tick count `3,600,000`をEAXへ返す。sample ID `uptime-ms-3600000`をfixed wall-clock sampleとpairし、generation `25`のlive stack pageから1 changed byteをreconcileするが、host writeがないためgeneration `25`／dirty 0を保持する。

return後5命令、entryから累計34命令でID `157`／`KERNEL32.dll!QueryPerformanceCounter`へ到達する。26,558-byte reportのSHA-256は`79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111`である。これはone controlled monotonic sample／API returnであり、general clock／timer／scheduler／snapshot modelを実装しない。詳細は[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

### B23. Fifth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-fifth-win32-call`はID `157`のone-argument stdcall frame／IAT callをadapterとRustで検査し、controlled counter `12,345,678,901`をstack上の8-byte outputへ書く。generation `25`のlive pageから5 changed bytesをreconcileし、generation `26`のone-page delta／ack後にBOOL `1`を返す。

return後44命令、entryから累計78命令でID `83`／`KERNEL32.dll!GetStartupInfoW`へ到達する。31,427-byte reportのSHA-256は`eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1`である。これはone controlled performance-counter sample／API returnであり、frequency、general clock／timer／scheduler／snapshot modelを実装しない。詳細は[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md)を正とする。

### B24. Sixth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-sixth-win32-call`はnative BGIのtarget caller RVA `0xA36E7`で68-byte x86 `STARTUPINFOW`が全書込されることをfocused traceで確認する。runtime側はID `83`のone-argument stdcall frame／IAT callをadapterとRustで検査し、owner `checkpoint-process-startup`のprofile `startupinfow-v68-zero-optional-fields`を書き込む。generation `26`のlive pageから31 changed bytesをreconcileし、generation `27`のone-page delta／ack後にVOID policyでEAXを保持する。

return後9命令、entryから累計87命令でID `84`／`KERNEL32.dll!HeapSetInformation`へ到達する。39,031-byte reportのSHA-256は`208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c`である。これはone controlled startup-info profile／API returnであり、general process creation、desktop、image path、console／standard handlesを実装しない。詳細は[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md)を正とする。

### B25. Seventh BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-seventh-win32-call`はnative BGI target caller RVA `0xA36FC`でarguments `[NULL,1,NULL,0]`、BOOL `1`、LastError `87 -> 87`をfocused traceで確認する。runtime側はID `84`のfour-argument stdcall frame／IAT callをadapterとRustで検査し、owner `checkpoint-process-heap-policy`のexact termination-on-corruption variantだけを受け付ける。generation `27`のlive pageから12 changed bytesをreconcileするが、guest writeがないためgeneration `27`／dirty 0を保持してBOOL `1`を返す。

return後21命令、entryから累計108命令でID `102`／`KERNEL32.dll!HeapCreate`へ到達する。43,980-byte reportのSHA-256は`09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f`である。これはone controlled process heap-policy selection／API returnであり、general heap allocator、corruption termination、LastError modelを実装しない。詳細は[v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md)を正とする。

### B26. Eighth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-eighth-win32-call`はnative BGI target caller RVA `0xA8886`でarguments `[0,0x1000,0]`、nonzero host result、LastError `0 -> 0`、first allocation consumerをfocused heap-lifecycle traceで確認する。runtime側はID `102`のthree-argument stdcall frame／IAT callをadapterとRustで検査し、generation `27`のlive pageから8 changed bytesをreconcileする。owner `checkpoint-process-private-heaps`のbounded metadataへtyped logical handle `0x68010001`を作り、guest writeなしでgeneration `27`／dirty 0を保持してreturnする。

return後14命令、entryから累計122命令でID `72`／`KERNEL32.dll!GetModuleHandleW`へ到達する。49,699-byte reportのSHA-256は`f8c8ca1e678bb1e118248a6626825ddf585dc240aa59e1abc428429cbab3a1ad`である。これはone controlled logical private-heap creation／API returnであり、native pointer、heap backing／allocation、general loader、LastError modelを実装しない。詳細は[v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md)を正とする。

### B27. Ninth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-ninth-win32-call`はnative BGI target caller RVA `0xA5811`で`GetModuleHandleW(L"KERNEL32.DLL")`、nonzero host HMODULE、LastError `87 -> 87`、immediate `GetProcAddress(...,"FlsAlloc")` consumerをfocused traceで確認する。runtime側はID `72`のone-argument stdcall frame／IAT callとbounded UTF-16 nameをadapterとRustで検査し、generation `27`のlive pageから9 changed bytesをreconcileする。owner `checkpoint-loaded-modules`のbounded metadataへtyped logical handle `0x4D010001`を作り、guest writeなしでgeneration `27`／dirty 0を保持してreturnする。

return後9命令、entryから累計131命令でID `150`／`KERNEL32.dll!GetProcAddress`へ到達する。56,729-byte reportのSHA-256は`147a3599cf942af03034c7f410d333b35c0360812936e39a7473c43008d8d5be`である。これはone controlled logical loaded-module lookup／API returnであり、native HMODULE、guest system-DLL image、general loader／export／LastError modelを実装しない。詳細は[v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md)を正とする。

### B28. Tenth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-tenth-win32-call`はnative BGI caller RVA `0xA582F`の`GetProcAddress(nativeKernel32,"FlsAlloc")`と続く3件のFLS lookup、LastError `87 -> 87`をfocused traceで確認する。runtime側はseparate owner／arenaへID `1`／`0x7F020000`の`kernel32.dll!FlsAlloc` RX endpointをsealし、389-page generation `27` reflectionをadapterへ渡す。ID `150`のtwo-argument frame、logical module、bounded ASCII name、14-byte register-indirect IAT provenanceをadapterとRustで検査し、generation `30`のlive pageから15 changed bytesをreconcileする。

host pointerではなくidentity-preserving endpointを返した5命令後、entryから累計136命令でsame ID `150`／`GetProcAddress(...,"FlsGetValue")`へ到達する。65,858-byte reportのSHA-256は`54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`である。これはone controlled dynamic export lookup／API returnであり、general export／forwarder resolution、endpoint dispatch、FLS semantics、LastError modelを実装しない。詳細は[v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md)を正とする。

### B29. Eleventh BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-eleventh-win32-call`はordered `FlsAlloc`／`FlsGetValue` endpointsをsame RX pageへatomic installし、first endpointのID／address／identity／lifetimeを保持する。second ID `150`のpersistent ESI target、13-byte call sequence、bounded ASCII nameをadapterとRustで検査する。generation `30` stack pageとBGI writable image pageをone transactionでreconcileし、slot `0x100FEEBC`がfirst endpoint `0x7F020000`を保持することをrequired invariantとする。

ID `2`／`0x7F020010`を返し、BGIのsecond store後5命令、entryから累計141命令でsame ID `150`／`GetProcAddress(...,"FlsSetValue")`へ到達する。76,844-byte reportのSHA-256は`40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`である。これはsecond controlled dynamic lookup／two-page reconciliationであり、general dirty discovery、endpoint dispatch、FLS semanticsを実装しない。詳細は[v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md)を正とする。

### B30. Twelfth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-twelfth-win32-call`はordered pageへID `3`／`FlsSetValue`を追加し、first two endpointsを保持する。third ID `150`のpersistent ESI target、13-byte sequence、bounded ASCII nameをadapterとRustで検査する。generation `30`のstack／image pagesをone transactionでreconcileし、first／second guest storesが`0x7F020000`／`0x7F020010`を保持することをrequired invariantとする。

ID `3`／`0x7F020020`を返し、BGIのthird store後5命令、entryから累計146命令でsame ID `150`／`GetProcAddress(...,"FlsFree")`へ到達する。89,289-byte reportのSHA-256は`28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`である。これはthird controlled dynamic lookup／two-page reconciliationであり、general dirty discovery、endpoint dispatch、FLS semanticsを実装しない。詳細は[v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md)を正とする。

### B31. Thirteenth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-thirteenth-win32-call`はordered pageへID `4`／`FlsFree`を追加し、first three endpoints／storesを保持する。fourth ID `150`のpersistent ESI target、13-byte sequence、bounded ASCII name、generation `30`のstack／image pagesをadapterとRustで検査する。

ID `4`／`0x7F020030`を返してBGI storeを観測し、12命令後、entryから累計158命令でstatic unsupported ID `70`／`TlsAlloc`へ到達する。103,179-byte reportのSHA-256は`1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`である。これはfourth controlled lookupであり、TLS／FLS semanticsを実装しない。詳細は[v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-fourteenth-win32-call`はfocused TLS traceの`TlsAlloc()`／LastError preserve／immediate first consumerをoracleとする。exact ID `70`のzero-argument frame／IATとall four endpoint storesをchecked処理し、native index `0x1C`を捨ててnamespace `vine.logical-tls-index.v1`のindex `0`を返す。7命令後、累計165命令でID `68`／`TlsSetValue(0,0x7F020010)`へ到達する。110,236-byte reportのSHA-256は`4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`である。詳細は[v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md)を正とする。

`npm run check:runtime:bgi-fifteenth-win32-call`はID `68`のlive ESI call、logical index `0`、FlsGetValue endpoint ID `2`、initial thread ID `8192`をchecked処理する。stack／endpoint-store／TLS-index-storeの3 pagesをtransactional reconcileしてone thread-value metadataを作り、BOOL `1`を返す。9命令後、累計174命令でID `89`／`EncodePointer(NULL)`へ到達する。120,499-byte reportのSHA-256は`25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`である。詳細は[v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md)を正とする。

### B32. Sixteenth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-sixteenth-win32-call`はtwo focused pointer tracesのprocess-dependent native resultsをdiscardし、ID `89`のdirect-IAT frame `[0x100A5467,NULL]`、retained TLS slot／thread valueをchecked処理する。namespace `vine.logical-encoded-pointer.v1`のdeterministic token `0xEC010001`を返し、stack pageを13 bytes reconcileする。55命令後、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ到達する。129,530-byte reportのSHA-256は`310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`である。詳細は[v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md)を正とする。

### B33. Seventeenth BGI Win32 call return: Verified ingress

`npm run check:runtime:bgi-seventeenth-win32-call`はsame native tracesとstatic disassemblyからsource `0x100A9608`をexecutable process-initializer functionとして分類する。11-byte direct-IAT frame、17-byte source prefix、prior encoded-NULL／TLS stateをchecked処理し、token `0xEC010002`を返す。BGI global `0x100FF62C`へのstoreを観測し、9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ到達する。140,660-byte reportのSHA-256は`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`である。詳細は[v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md)を正とする。

### B34. Automatic binary CPU writeback: Verified ingress

`npm run check:runtime:memory-writeback`は9 pagesを`VINERF01` binary reflectionでv86へ渡し、bounded slice後に8 writable PTEをscanする。first sliceはcross-page stack／surface、`REP STOSD` heap、RWX targetのexact 7 dirty pagesを検出し、49 changed bytesをgeneration `18 -> 19`へatomic reconcileする。Rustのone-page `VINEDL01` host deltaで`19 -> 20`へ進めた後、second sliceは別のstack／heap／surface／RWX exact 4 pages、13 changed bytesだけを`20 -> 21`へ戻す。

Dirty bitは各scanでclearし、`full_clear_tlb()`後だけ次へ進む。CPU writebackは`VINEWB01` fixed-width controlとcontiguous page payloadを使い、Rustはbase generation、owner、mapping、write permission、duplicate、alignment、sizeをbatch全体でpreflightする。reportは4,578 bytes／SHA-256 `5eab2884b70f8b1a48d98dff8021b29a1690296c03dcae0183b5c9fdc10368b8`である。これはcorrectness baselineであり、O(writable-pages) scan／full TLB clearのtitle performance、Chrome cancellation、snapshotを証明しない。詳細は[memory-writeback.md](memory-writeback.md)を正とする。

### B35. Browser Worker: Verified vertical slice

zero-import Rust runtime coreとsource-v86 mirrorはChrome Runtime Worker内でcontrolled PE32を46 instructions／10 slices実行する。2 deferred host completions、depth-2 callback、automatic writeback、Canvas／Web Audio／IndexedDB、cancel／watchdog、exact entry returnを通した。30分stressは11,326 sessions、failure 0、heap slope 136 bytes/min、maximum heartbeat gap 16.28 msである。詳細は[browser-runtime-worker.md](browser-runtime-worker.md)を正とする。

### B36. Browser performance gate: Go

commit `a0d99e5`のclean worktree、Chrome `150.0.7871.115`でversion 1 benchmarkを実行した。integer 134.1M ips、x87 52.5M ips、SSE2 83.7M ips、adapter trap 254,518/s、production Rust dispatch＋host completion＋CPU resume 11,279/s、callback 13.653 us、active steady memory 14.58 MB、hidden-tab ratio 0.985、AudioWorklet max delivery gap 51.315 msで全thresholdを満たした。decisionは`go`、report SHA-256は`e50ad355361f91e01baae1dcb6f62fb813b2ddc1202466682c60eda7e959a1e7`である。詳細は[v86-browser-benchmark.md](v86-browser-benchmark.md)を正とする。

これはsynthetic performance gateであり、adopted ISA correctness、general title workload、full-play performanceを証明しない。

## 8. 次の adapter slice

現在の source adapter は exact-budget execution、exception detail、versioned CPU state、binary bidirectional page synchronizationを提供する。controlled reflectionはzero-base fault比較を保持し、typed Rust bootstrap、entry completion、BGIの389-page dynamic-endpoint mode、first seventeen checked stdcall returnsまで進んだ。同じRust／v86 boundaryはChrome Runtime Worker vertical slice、30分stress、version 1 performance gateを通し、v86継続を`go`とした。Step 9のtyped object／handle、process-owned module graph、controlled lifecycle／two-thread registry／one-object wait-queue foundationsもbrowser／BGI regressionを維持した。BGI continuationは凍結したまま、次のruntime checkpointはmessage-queue foundationである。

```text
classify general STOP_THUNK / STOP_CALLBACK_RETURN / STOP_FAULT
map/protect/fault guest pages through the selected memory boundary
invalidate code range
```

adapter fixture は次の順に増やす。

1. [Fixed baseline verified] 最小Win32 fixtureで複数のowned thunk IDとguest window callbackを通した。後続のproduction sliceでgenerated API metadata／callback schemaを使うgeneral dispatcherへ移す。
2. [Measured] unmapped／read-only accessとcross-page faultを区別した。non-executableはcurrent v86 gapとして固定した。
3. [Controlled production reflection verified] Rust region／page generationをprovisional v86 paging、physical backing、execute gateへ接続し、actual invalidation後にentryからowned thunk ID `1`へ到達する。
4. [Verified] fixed cdecl thunk ID `1`をdispatch／resumeし、thunk ID `2`のcallback ingressを構造化する。
5. [Verified] production-owned callback sentinel／frameで1段callbackを実行し、exact return stopを構造化する。
6. [Verified] frameをretireしてouter service returnを完了し、guest entry continuationを測定する。
7. [Verified] production-owned process-entry return sentinelでexact successful completionを分類する。
8. [Verified] controlled initial stackとbootstrap recordをRust-owned reflectionへ移す。
9. [Verified] separate fixtureでnative FS-based TEB self／PEB pointer contractとzero-base first stopを固定する。
10. [Verified] Rust-owned TEB／PEB pagesとnonzero FS baseで同じfixtureをentry returnまで通す。
11. [Verified] separate native oracleでi386 process-parameters／command-line pathを固定し、empty guest PEBをexplicit resultへ戻す。
12. [Verified] 測定済みprocess-parameters／command-line bytesをminimum Rust ownershipへ接続する。
13. [Verified] BGI static importsをidentity付きexplicit trap thunksへbindしてimageをsealする。
14. [Verified] sealed BGI imageをbounded large-image transferでadapterへ渡し、first reachable trapを測る。
15. [Verified] exact `GetSystemTimeAsFileTime` frameへcontrolled `FILETIME`を書き、guestへreturnしてnext trapを測る。
16. [Verified] exact `GetCurrentProcessId` frameへcontrolled process IDを返し、next trapを測る。
17. [Verified] exact `GetCurrentThreadId` frameへcontrolled thread IDを返し、next trapを測る。
18. [Verified] exact `GetTickCount` frameへcontrolled tick-count sampleを返し、next trapを測る。
19. [Verified] exact `QueryPerformanceCounter` frameへcontrolled 64-bit sampleを書き、next trapを測る。
20. [Verified] exact `GetStartupInfoW` frameへcontrolled x86 structを書き、next trapを測る。
21. [Verified] exact `HeapSetInformation` frameの到達variantをreturnし、next trapを測る。
22. [Verified] exact `HeapCreate` frameへtyped logical heap handleを返し、next trapを測る。
23. [Verified] exact `GetModuleHandleW` frameへtyped logical loaded-module handleを返し、next trapを測る。
24. [Verified] exact `GetProcAddress` frameへidentity-preserving callable `FlsAlloc` endpointを返し、next trapを測る。
25. [Verified] second `GetProcAddress` frameへdistinct `FlsGetValue` endpointを返し、next trapを測る。
26. [Verified] third `GetProcAddress` frameへdistinct `FlsSetValue` endpointを返し、fourth lookupを測る。
27. [Verified] fourth `GetProcAddress` frameへdistinct `FlsFree` endpointを返し、next trapを測る。
28. [Verified] `EncodePointer(NULL)`のnative behaviorを測定し、process-specific host resultをdiscardしたcontrolled returnからnext trapを測る。
29. [Verified] `EncodePointer(0x100A9608)`のnative behavior／source role／first consumerを測定し、observed process-initializer function variantだけをreturnする。
30. `EncodePointer(0x7F020000)`へcontrolled `FlsAlloc` endpoint tokenを返し、prior encoded result storeをreconcileする。
31. broader stdcall signature、thiscall、nested callbackを実到達fixtureから追加する。
32. [Verified] 同じfixtureをNodeとChromium Workerで実行する。
33. [Measured] version 1 synthetic CPU／ABI／memory／Worker workloadを測定し、go thresholdを通す。adopted ISA correctnessとtitle workloadは別taskに残す。

## 9. 採用 gate

すでに満たした gate:

- source から fallback adapter Wasm を再現 build できる
- caller-defined budget と structured stop reason が test で保証される
- release-size build で CPL3 fault stop が動く
- versioned CPU state が save／reset／restore／resume を通る
- fixed T1 fixture の exact owned API trap と cdecl guest callback が通る
- high-address mapping、R／W fault ordering、protect／remap invalidationのmeasurementがあり、native execute gapが明示される
- controlled production ownershipがsemantic reflection／actual invalidationを経てv86 entryからfirst thunkまで通る
- fixed production cdecl serviceがexact thunk metadataとstate ABIでguestへreturnし、second thunk ingressまで通る
- separate production callback sentinel／frameからone-level guest callbackがresult `43`でexact return trapまで通る
- v86-originated stack／heap／surface／RWX dirty pagesをAPI固有listなしでRust authorityへ戻せる
- Chromium Worker correctness、30分stress、versioned performance thresholdが通る

`ADR-002` の採用前に残る gate:

- general process heap／stack growth、guard、snapshotをv86 paging／fetchへ接続する
- target workloadに必要なISA／ABI／exception bandを説明できる
- fork の変更範囲と upstream追従手順を説明できる

一つでも根本的に満たせない場合は、Bochs CPU-only extraction を同じ adapter fixture で測る。Unicorn は browser／Wasm execution backend を再現できた時点で再評価する。libx86emu は integer-only reference fixture には利用できるが primary engine にはしない。
