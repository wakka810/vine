# 未確定事項とリスク台帳

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 現状: 1 人の個人開発、primary corpus、full-play goal、Rust + TypeScript、static dependency inventory、LLVM baseline、Windows trace v1／v2、selected IDirect3D9 root、W1／W2／W3、high-address memory、Rust production memory／PE loader ingress、BGI firstからsixth Win32 call returnは確定。TypeScript／Wasm browser boundary、device-level trace、general runtime／browser appは未確定

この文書は、不確実性を TODO のまま埋め込まず、実装を進める条件と見直す条件を明確にする。所有者欄の role は別人を意味せず、単一 maintainer が判断時に切り替える責任観点を示す。

## 2. 現時点の仮定

| ID | 仮定 | 破れた場合の影響 | 検証期限 |
|---|---|---|---|
| A-01 | 観測した 7 entry executable はすべて PE32 / i386 native code であり、primary executable に CLR／delay import／static TLS はない | dynamic DLL や runtime path に未対応 executable feature があれば active scope を再定義 | Phase 0-2 |
| A-02 | 少なくとも一つの代表 VN-like workload を interpreter 系で実用速度に近づけられる | CPU engine または対象 title を変更 | Phase 1 |
| A-03 | サクラノ詩は GDI／GDI+、bounded D3D9、DirectInput 8、DirectSound と限定した Win32／COM／WinMM subset で primary scenario を実行できる | D3D9 shader、video codec、広い COM class 等が必須なら Phase 6D と active scope を再設計 | Phase 0-6D |
| A-04 | title source は read-only、save／registry は別 overlay で成立する | installer semantics、self-update、同一 file 書換えへの追加設計 | Phase 3 |
| A-05 | browser の local storage と Worker で、必要容量と同期／永続性を満たせる | import 方式、support browser、storage backend を変更 | Phase 0-3 |
| A-06 | 必要な system API を synthetic DLL として実装できる | 技術 blocker。project scope を停止または変更 | Phase 0-2 |
| A-07 | fingerprint 済み local corpus と今後作る synthetic fixture を継続して利用できる | 自動回帰と実 title 検証が成立しない | 継続 |
| A-08 | full-play target は desktop browser のみに限定でき、mobile／tablet を初期範囲外にできる | memory、input、background、thermal の設計を前倒し | Resolved scope |

仮定は事実ではない。検証期限を過ぎた未検証の仮定は、対応 phase の完了を block する。

## 3. 未確定事項

### 3.1 判断規則

各 `Q-*` は ADR、decision note、または明示的な scope update で閉じる。単に実装が先行した状態を `Resolved` としない。

| Status | 意味 |
|---|---|
| Open | 調査／比較前 |
| Investigating | spike、測定中 |
| Resolved | 選択肢、根拠、影響、再検討条件を記録済み |
| Deferred | 決定期限を後ろへ移し、現在 phase への影響がないことを確認済み |

### 3.2 Product、scope、governance

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-001 | Resolved | 最初の compatibility corpus は何か | primary はサクラノ詩。Nursery Rhyme は同系統比較、残り 5 title は deferred corpus | [corpus-inventory.md](corpus-inventory.md) の PE／import／fingerprint inventory | Resolved | Product |
| Q-002 | Resolved | active completion を何で判定するか | fingerprint 済みサクラノ詩の全編を Chrome で不自由なくプレイできること。短い versioned scenario と machine-readable API coverage は段階的 evidence。multi-title MVP は active scope 外 | requirements と corpus scope decision | Resolved | Product |
| Q-004 | Investigating | 実行形態は local dev server、static local hosting、desktop wrapper のどれか | browser app を前提とし、個人利用なので外部 hosting は要件にしない | COOP／COEP、offline、OPFS、update 手順の local smoke | Phase 0 | Product + Web platform |
| Q-005 | Open | desktop browser の support Tier はどうするか | Chromium 系を最初の feasibility target として測り、Firefox を同じ phase で評価。Safari は証拠なしに約束しない | feature probe、E2E、性能、OPFS／audio／worker 差 | Phase 1 | Web platform |
| Q-006 | Deferred | mobile／tablet をいつ扱うか | サクラノ詩 full-play target の対象外 | active target 完了後に必要性を再評価 | Future | Product |
| Q-007 | Resolved | game installer の実行を初期範囲に含めるか | 既に展開／install 済みの game directory import のみを扱う | local corpus の配置確認 | Resolved | Product |

### 3.3 Core runtime

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-010 | Investigating | core の言語 ownership、workspace、Wasm toolchain は何か | Rust は CPU／memory／PE／ABI core、TypeScript は Windows API facade／browser backend／UI／toolingを担当する。root Rust workspaceは成立済みだが、Rust coreのWasm packagingとTypeScript／Worker boundaryは未固定 | [toolchain-spike.md](toolchain-spike.md)、最初のRust Wasm／TypeScript workspace build | Phase 0 | Runtime lead |
| Q-011 | Investigating | x86 engine は既存 engine、独自 interpreter、hybrid のどれか | v86を交換可能なworking baselineとして使う。fixed T1のcdecl／callbackとBGI 388-page first six API returnsは通った。具体的なblockerがない限り他候補の比較や自作をactive workにせず、最終固定はgeneral memory、browser Worker、ISA／performance spike後 | [x86-engine-scorecard.md](x86-engine-scorecard.md)、W1／BGI result、ISA tests、browser benchmark | Phase 0 | CPU lead |
| Q-012 | Open | optimized execution は threaded interpreter、IR、DBT、generated Wasm のどれか | Phase 1 は baseline interpreter。性能不足が測定された hot path だけを最適化 | cold／warm benchmark、CSP、cache invalidation、differential trace | Phase 1 gate | CPU lead |
| Q-013 | Investigating | 4 GiB guest VA をどう表現するか | high-address spike、controlled reflection、dirty-page `MemoryDelta` roundtrip、VGA MMIO holeを除外したBGI 388-page transfer、generation／owner付きone-page reconciliationにより、Vine region table＋bounded physical allocatorをv86 pagingへ反映する案をprovisional candidateとした。Wasm offsetとの1:1 mappingは避ける | [adr-003-guest-memory.md](adr-003-guest-memory.md)、[memory-delta-and-transactions.md](memory-delta-and-transactions.md)、[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)、automatic v86 writeback、mapped file、memory pressure／Worker spike | Phase 0 | Runtime lead |
| Q-014 | Investigating | page size、protection、guard、self-modifying code policy は何か | 4 KiB region／page metadataとtransactional invalidationをbaseline候補とする。controlled TLB／generated-code ackは成立したがcurrent v86にnative execute protectionがないためadapter fetch gateが必須 | [adr-003-guest-memory.md](adr-003-guest-memory.md)、execute／guard／write-to-code／snapshot fixture | Phase 1 | CPU + Memory owners |
| Q-015 | Open | guest thread を cooperative に multiplex するか、Worker へ分散するか | Runtime Worker 上の cooperative scheduler を baseline | wait／callback／message test、UI responsiveness、multi-thread corpus inventory | Phase 0 | Runtime lead |
| Q-016 | Open | SharedArrayBuffer と cross-origin isolation を必須にするか | optional fast path として検証。必要なら Tier 1 の deployment requirement にする | audio ring、thread、copy overhead、embed 制約、header smoke | Phase 1 gate | Web platform |
| Q-017 | Open | SEH、VEH、APC、TLS／TEB をどこまで実装するか | corpus と compiler fixture の使用経路を inventory 化し、silent stub はしない | Windows differential traces、title startup traces | Phase 1-2 | Windows ABI owner |
| Q-018 | Open | x87／MMX／SSE／SSE2 の初期範囲は何か | corpus binary scan と runtime trace の union を最低範囲にする | compiler 別 fixture、FPU precision／rounding oracle | Phase 1 | CPU lead |
| Q-019 | Investigating | PE32 bootstrap の PEB／TEB／FS／process parameters layout をどこまで保証するか | measured i386 profileとしてTEB self `+0x18`、PEB `FS:[0x30]`、process parameters `PEB+0x10`、command line `+0x40`、buffer `+0x04`だけをtyped ownershipへ採用した。supported compiler／CRT fixtureをsupport bandとし、内部 layout の全面互換を約束しない | [v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md)、[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)、compiler／CRT startup fixture | Phase 1 | Loader + Windows ABI |

### 3.4 PE、ABI、Windows API

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-020 | Open | PE feature の必須集合は何か | relocation、named／ordinal import、forwarder、TLS、resource、delay import を fixture 化 | corpus inventory、loader matrix、Windows behavior comparison | Phase 1 | Loader owner |
| Q-021 | Open | synthetic system DLL と同梱 DLL の名前解決順は何か | module class ごとの決定的な解決順を定義し、曖昧な置換を禁止 | title dependency graph、loader fixture | Phase 1 | Loader owner |
| Q-022 | Open | API metadata と marshaller をどう生成するか | hand-written raw pointer glue を避け、schema から validation と docs を生成 | callback、nested structure、union、size-versioned API prototype | Phase 1 | ABI owner |
| Q-023 | Open | CRT、COM、OLE、shell、common dialog の初期範囲は何か | import／dynamic lookup 順に support band を定義 | corpus trace、fixture、UI／compatibility impact | Phase 2 | Windows API owner |
| Q-024 | Open | direct `ntdll` call や syscall をどう扱うか | documented userspace boundary を優先し、direct syscall は unsupported として検出 | corpus binary scan、startup trace | Phase 2 | Windows API owner |
| Q-025 | Open | unsupported API の既定動作は fail-fast か error return か | API ごとの policy。成功を装う stub は禁止 | caller behavior fixture、diagnostic UX | Phase 1 | Windows API + Product |
| Q-026 | Open | Windows differential oracle をどう運用するか | 自作 fixture を Windows 環境で実行し、structured trace を比較 | automation、clock／font 差の normalize 方針 | Phase 0 | Test lead |
| Q-027 | Open | API-set／system module alias、SxS manifest／activation context、CRT redist の初期範囲は何か | corpus の import／resource／dynamic trace で必要範囲を決める | alias／manifest fixture、DLL search oracle | Phase 1 | Loader + Windows API |
| Q-028 | Open | varargs API をどの schema／format grammar まで扱うか | API ごとの bounded schema を使い、generic variadic bridge は unsupported | `wsprintf` 等の fixture、format-string fuzz、ABI oracle | Phase 1 | ABI + QA |

### 3.5 Storage、locale、browser features

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-030 | Open | import 形式は directory、file set、archive のどこまでか | directory／explicit file set を先行。archive は traversal／bomb 対策後 | browser API matrix、large title test、UX test | Phase 3 | Product + Storage |
| Q-031 | Open | OPFS、IndexedDB、File System Access の役割分担は何か | adapter を設け、source handle と writable managed storage を分離 | crash test、quota、eviction、permission、browser matrix | Phase 0 | Storage owner |
| Q-032 | Open | file sharing、mapping、timestamp、wildcard、short name をどこまで再現するか | title traces で必要度順。未対応差分を profile に記録 | filesystem differential suite | Phase 3 | Storage + Windows API |
| Q-033 | Open | save の atomicity、backup、migration format は何か | journal または copy-then-commit、versioned manifest、export 必須 | process kill、quota full、old-version migration test | Phase 3 | Storage owner |
| Q-034 | Open | Windows-932、locale、timezone の既定値は何か | title profile に明示し、host locale へ暗黙追従しない | Japanese text／filename／sort fixture | Phase 2 | I18N owner |
| Q-035 | Open | IME、clipboard、file drop を personal target に含めるか | keyboard/mouse と分離し、primary scenario で未使用なら deferred | corpus use、permission／lifecycle review | Phase 3 | Product |
| Q-036 | Open | Network API を完全拒否するか、限定実装を持つか | personal target では unsupported | primary scenario の runtime call evidence | Phase 2 | Product + Windows API |
| Q-037 | Open | immutable module snapshot を managed copy、browser `File` snapshot、再検証付き handle のどれで保持するか | content-addressed managed copy を baseline とし、quota／restart／TOCTOU を比較 | external replacement、delay load、restart、quota test | Phase 0 | Storage + Loader |
| Q-038 | Investigating | DirectInput、Raw Input、gamepad を初期 input band に含めるか | サクラノ詩の DirectInput 8 keyboard／mouse subset は active。Raw Input／gamepad は deferred | vtable method／device／data format dynamic trace、input fixture | Phase 6D | Input owner |
| Q-039 | Open | guest write 成功、flush／close、browser commit の durability contract は何か | acknowledged write と durable commit を区別し、hard stop は未commit state を rollback | kill injection、quota、browser transaction behavior | Phase 3 | Storage owner |

### 3.6 Graphics と audio

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-040 | Open | GDI／DirectDraw surface と D3D9 resource の共有境界はどこか | GDI／DDraw pixel storage は共有候補。D3D9 は lock staging、format conversion、presentation だけを共有 | 8／16／24／32 bit、pitch、lock、upload／readback benchmark | Phase 3 | Graphics lead |
| Q-041 | Open | renderer は Canvas 2D、WebGL2、WebGPU のどれか | D3D9 backend を満たす WebGL2／WebGPU を主比較し、Canvas 2D は GDI 補助候補 | palette、RGB565、text、readback、primary D3D9 state／draw、context loss | Phase 3 | Graphics lead + Web platform |
| Q-042 | Open | GDI text rasterization と font substitution をどうするか | font mapping を明示し、metric 差を測定 | CP932 glyph、kerning、baseline、line break golden | Phase 3 | Graphics + I18N |
| Q-043 | Deferred | DirectDraw cooperative／display mode をどう写像するか | logical display mode + canvas scale + optional Fullscreen API | Nursery Rhyme comparison を開始するときに測定 | Phase 5 branch | Graphics lead |
| Q-044 | Open | AudioWorklet と ring buffer の transport は何か | AudioWorklet consumer。SAB と copy queue を比較 | latency、underrun、support Tier、COOP/COEP impact | Phase 3 | Audio lead |
| Q-045 | Open | guest clock と audio clock の drift policy は何か | audio sample clock を観測し、bounded correction | 30／120 分 playback、background／resume test | Phase 6 | Audio + Runtime |
| Q-046 | Investigating | MCI、ACM、DirectShow、non-PCM codec をどう扱うか | サクラノ詩の `mciSendCommandA` 到達 command だけ active。その他は DirectSound と分離 | primary scenario の runtime calls、format、browser decode | Phase 6 | Audio owner |
| Q-047 | Investigating | サクラノ詩に必要な D3D9 support band は何か | import 数ではなく COM vtable method、format、render state、draw path の実行 trace で固定 | CreateDevice から first present／scene／reset の trace と fixture | Phase 6D entry | Graphics lead |

### 3.7 Quality と operations

| ID | Status | 問い | 暫定方針 | 解決に必要な証拠 | 期限 | Owner role |
|---|---|---|---|---|---|---|
| Q-050 | Open | 個人環境の基準端末、性能 SLO、bundle budget は何か | 実際に利用する desktop を primary reference とし、実測前に絶対値を固定しない | hardware／browser profile、vertical slice benchmark | Phase 1 gate | Performance owner + Product |
| Q-052 | Open | title fingerprint に何を含めるか | metadata + selected executable／DLL hash | collision、update／variant test | Phase 3 | Storage |
| Q-055 | Open | fuzz／soak の最低量と personal acceptance gate は何か | component coverage を先に計測し、Phase 1 で budget を固定 | baseline throughput、corpus size、CI cost | Phase 1 | QA owner |

## 4. リスク評価

### 4.1 尺度

| 値 | 発生可能性 | 影響 |
|---|---|---|
| Low | 現時点の証拠では起こりにくい | 一つの task／minor release 内で吸収可能 |
| Medium | 条件次第で十分に起こる | phase の再計画または support 制限が必要 |
| High | 未検証または既に典型的な障害要因 | personal target の成立、correctness、save data を脅かす |

`High / High` は、担当と検証 task が割り当てられるまで関連実装を開始しない。risk は phase review ごとに probability、impact、trigger、residual risk を更新する。

## 5. リスク台帳

### 5.1 Product と成立性

| ID | リスク | P | I | 早期指標／trigger | 緩和策 | contingency | Owner role |
|---|---|---|---|---|---|---|---|
| R-001 | 「サクラノ詩対応」が無制限の Win32／DirectX 対応へ膨張する | High | High | static import をすべて実装し始める、未到達 API が backlog を占める | primary scenario、runtime call evidence、bounded support band を固定 | 未到達 API と非 critical scenario を deferred に戻す | Product + Runtime lead |
| R-002 | fingerprint 済み corpus または再現 scenario を継続利用できない | Low | High | storage failure、version 置換、scenario 再現不能 | synthetic fixture と title corpus を分離し、hash／profile／checkpoint を固定 | fixture 開発を継続し、T3 acceptance を保留 | Product |
| R-003 | primary title を急ぐため title-specific conditional／patch が増える | Medium | High | EIP／hash 条件、fixture のない fix、BGI 内部 semantics の host 再実装 | API semantic test と gap closure rule、declarative profile の用途制限 | personal target を一時停止し、一般 Win32／DirectX contract へ戻して修正 | Architecture owner |
| R-004 | x64／D3D11／Unity や multi-title 一般化を早期に混ぜて personal target が遅延する | Medium | High | 64 bit、D3D11、公開配布 task が critical path に入る | future track と active target を分離 | expansion branch を凍結しサクラノ詩 evidence を優先 | Product |

### 5.2 CPU、memory、scheduler

| ID | リスク | P | I | 早期指標／trigger | 緩和策 | contingency | Owner role |
|---|---|---|---|---|---|---|---|
| R-010 | x86 flags、x87、exception の誤りが非決定的な title failure になる | High | High | oracle 差分、scene ごとの crash、save state divergence | ISA differential tests、compiler corpus、instruction trace bisection | engine 候補を変更、命令範囲を限定 | CPU lead |
| R-011 | interpreter が実用 frame／audio deadline を満たさない | High | High | Phase 1 hot path が provisional budget を大幅超過 | profile-driven cache／IR、既存 engine 比較、Worker isolation | target title／resolution を絞るか optimized engine へ gate 付き移行 | CPU + Performance |
| R-012 | browser 向け DBT／generated Wasm が compile pause、CSP、cache invalidation で成立しない | Medium | High | cold start spike、self-modifying code mismatch、strict CSP failure | interpreter fallback、small block cache、write invalidation test | DBT を非採用。support scope を baseline 性能内に縮小 | CPU lead |
| R-013 | guest 4 GiB VA と host memory 制約が衝突する | High | High | high-base fixtureと2 MiB内のBGI 388-page transferは通ったが、allocator quota／snapshot／Chrome WorkerでOOMまたはstale mapping | v86 paging＋bounded physical pages＋region owner、physical MMIO exclusion、memory ceiling test | checked sparse replacementを再評価、対応rangeをprofileに記録 | Memory owner |
| R-014 | sync Win32 と async Web API の橋が deadlock／reentrancy bug を生む | High | High | nested callback、SendMessage、file wait で停止 | explicit thread states、request IDs、callback depth、model tests | API／thread support band を縮小し unsupported stop | Runtime lead |
| R-015 | cooperative scheduling が timing-sensitive multi-thread title と合わない | Medium | High | wait starvation、audio producer miss、race oracle 差 | fairness、virtual clock、yield points、thread trace | SAB／multi-Worker spike、対象 title 除外 | Runtime lead |
| R-016 | background throttling や browser lifecycle で guest time が破綻する | High | Medium | resume 後 timer storm、audio desync、save corruption | freeze/resume policy、lifecycle event、bounded catch-up | background 実行を非対応にし pause を強制 | Web platform |

### 5.3 Windows API、graphics、audio

| ID | リスク | P | I | 早期指標／trigger | 緩和策 | contingency | Owner role |
|---|---|---|---|---|---|---|---|
| R-020 | undocumented／subtle Win32 semantics が loader や API を破る | High | High | ordinal／TLS／LastError／callback 順で Windows 差分 | small contract fixtures、structured Windows oracle、trace | corpus を既知 compiler/runtime band に限定 | Windows API lead |
| R-021 | silent stub が起動を進める一方、後で state を壊す | High | High | unsupported call 後に別 subsystem で failure | status taxonomy、stub side-effect test、first divergence trace | stub を error／stop に変更 | Windows API lead |
| R-022 | GDI／GDI+／DirectDraw の pixel、palette、pitch、ROP 差で画面が崩れる | High | High | golden diff、readback mismatch、surface lock corruption | common pixel tests、format-specific path、semantic + image oracle | support format／ROP を限定し title profile に明記 | Graphics lead |
| R-023 | font availability または metrics 差で日本語 text が切れる | High | High | missing glyph、line wrap／button layout 差 | font mapping、metric tests、title override | font を利用者指定にし、既知差分を表示 | Graphics + I18N |
| R-024 | browser graphics backend 差、context loss で表示不能になる | Medium | High | vendor-specific golden failure、device lost | backend abstraction、context restore、browser E2E | support browser／GPU を Tier 2 または unsupported にする | Graphics + Web platform |
| R-025 | DirectSound cursor と Web Audio scheduling 差で音切れ／同期ずれが起きる | High | High | underrun、loop gap、30 分 drift | AudioWorklet ring、single clock policy、offline + realtime test | latency buffer を増やし、対応 format を限定 | Audio lead |
| R-026 | MCI、codec、video 等が primary scenario の必須経路になる | High | High | `mciSendCommandA`／COM class call before menu、未対応 media format | startup／scenario dynamic trace を Phase 6 より前に取り、command／format を固定 | bounded media task を追加し、未証明の codec 一般化は行わない | Product + Audio |
| R-027 | D3D9 の実利用範囲が bounded fixed-function subset に収まらない | High | High | shader create、複数 swap chain、複雑な render target／readback、頻繁な Reset | CreateDevice から scene までの vtable／state trace、WebGL2／WebGPU spike | Phase 6D で architecture と scope を再決定し、成立性未証明のまま API 実装を広げない | Graphics lead |

### 5.4 Storage、browser、runtime robustness

| ID | リスク | P | I | 早期指標／trigger | 緩和策 | contingency | Owner role |
|---|---|---|---|---|---|---|---|
| R-030 | quota、eviction、permission loss、crash で save data を失う | Medium | High | commit 中 kill、quota-full test、browser eviction | journal／atomic replace、backup/export、preflight quota | 起動を read-only にし保存不能を明示 | Storage owner |
| R-031 | Windows path／case／sharing 差で asset または save を誤る | High | High | case collision、wildcard、delete/share oracle 差 | object-ID VFS、differential suite、source/overlay separation | unsupported path behavior を title profile で拒否 | Storage owner |
| R-031A | 外部 file 差替え、writable overlay、persistent self-update が executable／DLL bytes を変え、無改変保証を形骸化する | Medium | High | preflight／delay-load 間の hash 変化、module path write | stable snapshot、map hash verification、executable shadow deny | title を停止し、新 version は明示的な再 import のみ許可 | Loader + Storage |
| R-032 | malformed PE／archive／asset が parser memory bug を突く | High | High | fuzzer crash、OOB、unbounded allocation | memory-safe/bounded parsing、fuzz、size／depth limit | affected input feature を disableし regression fixture を追加 | Component + QA owner |
| R-033 | guest が CPU、memory、thread、surface、log を枯渇させ UI を停止する | High | High | watchdog miss、browser tab kill、log explosion | layered quota、rate limit、Worker termination、backpressure | title を停止し未commit write を rollback | Runtime + QA |
| R-035 | browser feature 差で同じ runtime revision の挙動が分岐する | High | Medium | OPFS／audio／worker E2E divergence | feature detection、Tier matrix、adapter contract | 一部 browser を Tier 2／unsupported に再分類 | Web platform |
| R-036 | COOP／COEP が hosting、embedding、third-party resource と衝突する | Medium | Medium | SAB が使えない、asset load が block | self-contained assets、header test、non-SAB fallback | Tier 1 を専用 hosting に限定 | Web platform |
### 5.5 Supply chain、delivery

| ID | リスク | P | I | 早期指標／trigger | 緩和策 | contingency | Owner role |
|---|---|---|---|---|---|---|---|
| R-044 | dependency／toolchain drift で再現不能または不正な runtime artifact が生成される | Medium | High | lock drift、unverified binary、manifest delta | pin、reproducible build、artifact hash、dependency review | known-good toolchain から rebuild | Release + Maintainer |
| R-045 | CPU、Windows、graphics、audio、browser を 1 人で扱うため、context switch と保守量が進捗を止める | High | High | 未完 branch／spike が増える、同時に複数 subsystem を変更する、長期に再開不能 | WIP 1、vertical checkpoint、短い ADR、synthetic test、active／deferred の厳格な分離 | active scenario を短縮し、一つの blocker だけを解消する iteration に戻す | Maintainer |
| R-046 | 目先の title 起動を優先して correctness gate が形骸化する | High | High | title demo を理由に fixture／oracle／diagnostic を skip | exit-criteria-based roadmap、各 gap の最小 regression fixture | milestone を compatibility experiment に戻し、欠けた evidence を先に補う | Product + Maintainer |

## 6. Feasibility の go / no-go 条件

### 6.1 Phase 0 から Phase 1 へ進む条件

- corpus と fixture の profile が記録されている
- x86 engine 候補に未処理の技術 blocker がない
- guest memory、scheduler、storage、renderer、audio、bounded D3D9 の spike plan と測定値が定義済み
- browser Tier 候補と deployment header を再現できる
- resource limit と cancellation rule の最小集合が承認済み

### 6.2 Phase 1 の技術 go 条件

- selected engine が必要 ISA／ABI fixture を正しく実行する
- guest callback、nested API、thread wait、periodic yield が deadlock しない
- high-address／sparse allocation が memory budget 内で動く
- baseline または bounded optimization prototype の representative CPU hot path が、基準端末と Tier 1 browser で Phase 0 の測定目標を満たす
- 30 分 stress で main thread stall、silent divergence がなく、memory／pending request／queue の増加率が versioned threshold 内にある

最適化計画だけでは技術 go と `AC-P1` を満たさない。測定未達でも限定調査を続ける場合は `Conditional go` とし、数値 gap、prototype、期限、撤退値、禁止する downstream 作業を記録する。少なくとも広い Win32 API 実装、title compatibility claim、Phase 2 完了宣言は、同じ benchmark の再実行で go 条件を満たすまで禁止する。

### 6.3 No-go または scope reduction 条件

次のいずれかが発生し、二つ以上の engine／architecture 案で解消できない場合、personal target の範囲または project premise を見直す。

- CPU 性能が audio／UI deadline から一桁以上離れ、許容できる最適化経路がない
- required guest VA を browser memory 制約内で表現できない
- sync／async と callback semantics を main thread 非 block で再現できない
- 必須 dependency が target browser／toolchain と両立しない
- representative title が非対象の driver／kernel／codec／online service を必須とする
- 再現可能な oracle／corpus を用意できず、correctness を判断できない

No-go は project 全体の即時中止だけを意味しない。critical scenario の短縮、別 architecture の検証、developer research runtime への位置付け変更も選択肢とする。ただし受け入れ条件を満たさないものを personal target 完了とは呼ばない。

### 6.4 後続 phase の scope reduction trigger

| Gate | Trigger | 必須判断 |
|---|---|---|
| Phase 3 storage | Tier 1 browser で module snapshot、kill-safe save、quota recovery、title isolation のいずれかが threshold を満たさない | browser／import 方式／title size を縮小するか storage architecture を再選定 |
| Phase 4 GDI | adopted font で glyph 欠落、critical text の clip／overlap、frame budget 超過を解消できない | font／renderer／title corpus を変更し partial support を明記 |
| Phase 5 DirectDraw branch | comparison corpus の format／clipper／GDI interop が threshold を満たさない | branch を deferred に戻す。サクラノ詩 gate は block しない |
| Phase 6 DirectSound | Tier 1 browser で cursor／notification／focus semantics、underrun／drift threshold を満たさない | SAB／hosting Tier、latency、audio format、title scope を再決定 |
| Phase 6D D3D9／DirectInput | shader／method／format scope または backend 性能が entry threshold を超える | renderer／support band／primary scenario を再設計し、Phase 7A へ進まない |
| Phase 7A | title-specific code path または binary patch が必要になる | target を差し替えず、failure を一般 contract で再現できるまで integration を停止 |
| Phase 7B | optional expansion が active target の保守性を損なう | expansion を deferred に戻す |

各 trigger の具体的 threshold は gate の Entry criteria で evidence record に固定する。未達のまま task を追加し続けることを黙示的な `Go` としない。

## 7. Phase review での更新項目

各 phase の開始と終了で、次を記録する。

- 新しく確認または否定された仮定
- closed／new `Q-*` と対応 ADR
- risk の P／I、owner、mitigation task、residual risk
- title／API／browser compatibility matrix の変化
- performance、memory、fuzz、soak の実測値
- dependency、fixture、artifact integrity の差分
- schedule range と critical path の見直し
- scope reduction／stop rule の発動有無

実装 task と risk mitigation task を別 backlog にせず、[roadmap.md](roadmap.md) の同じ phase に結び付ける。
