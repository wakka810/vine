# Vine 詳細ロードマップ

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 現在地: Phase 0 In discovery／W1／W2／W3／W7 PE32 load／memory／entry return／stack bootstrap／bootstrap ingress／explicit import traps／BGI seventeenth Win32 call。Frozen BGI evidenceは最初の17 API returnで固定し、separate runtime-kernel profileは24 contracts／61 callsをcommon dispatcherからreturnして`GetCommandLineA()`へ到達した。同じcurrent sessionはChrome Runtime Worker内のRust/Wasm authorityへ移行し、Node oracleとのordered completion／memory digest一致、heartbeat、cancel／watchdog teardownを通した。timer clamp非依存yieldによる3分／1,579-session workload gateはsession／slice分布、main／Worker heapとbacking-storage slope、linear memory、Wasm crossing thresholdを通した。full-play steady-memory、visual critical scenarioとdevice-level COM continuationは未完了
- repository baseline: `main` の `4e8f29d` はルート `README.md` 1 行だけ。現在は docs、repository check、PE32 fixtures、v86 source adapter、W1／W2／W3、high-address memory、Rust W7 load／memory／stack／bootstrap、sealed BGI trap binding、388-page entry-to-trap execution、controlled Chrome Runtime Worker、version 1 v86 browser engine gate、mapped module／bounded PE32 export registry、controlled guest import／DLL process callback lifecycle、public CIがある。bound multi-module CPU execution、dynamic DLL lifecycle、general process integration、Win32 runtime、title browser appは未実装
- 対象: PE32 / i386、Win32、GDI／GDI+、bounded D3D9、DirectInput 8、DirectSound によるサクラノ詩 full-play runtime

このロードマップは、広範な Windows 互換を一度に実装する計画ではない。Windows native trace を根拠に、headless 実行、startup、first frame、input、audio、persistence の短い vertical checkpoint を広く浅く通す。各 subsystem の全面実装は後回しにし、各 phase は日付ではなく受け入れ条件を満たした時点で完了する。

関連文書:

- [requirements.md](requirements.md): 要件と personal target の定義
- [corpus-inventory.md](corpus-inventory.md): primary target、fingerprint、static dependency と調査待ち evidence
- [target-architecture.md](target-architecture.md): コンポーネント境界と先行 ADR
- [x86-engine-scorecard.md](x86-engine-scorecard.md): engine 候補、実行 evidence、adapter 採用 gate
- [risks-and-open-questions.md](risks-and-open-questions.md): `Q-*`、`R-*`、go / no-go 条件
- [testing-and-acceptance.md](testing-and-acceptance.md): test 資産、証跡、`AC-*`
- [windows-native-evidence.md](windows-native-evidence.md): Windows W0、BGI trace v1／v2、selected IDirect3D9 root、native ABI oracle、W1 runtime、W2 native window fixture／loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flow、W3 browser first pixel
- [adr-003-guest-memory.md](adr-003-guest-memory.md): Rust byte authority、v86 execution mirror、slice synchronization／failure policy
- [memory-writeback.md](memory-writeback.md): PTE Dirty discovery、binary multi-page writeback、continuous-slice evidence
- [pe-import-binding.md](pe-import-binding.md): all-import read-only plan、atomic thunk／IAT commit、rollback、unresolved retry
- [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md): optional identity trap policy、controlled fixture、BGI full binding／owner seal
- [v86-entry-ingress.md](v86-entry-ingress.md): semantic page reflection、actual invalidation acknowledgement、controlled entry-to-thunk evidence
- [v86-cdecl-ingress.md](v86-cdecl-ingress.md): exact thunk classification、fixed cdecl return、guest cleanup、callback-service ingress
- [v86-callback-return-ingress.md](v86-callback-return-ingress.md): separate callback-control owner、frame ID、checked stack、exact guest callback return evidence
- [v86-outer-return-ingress.md](v86-outer-return-ingress.md): frame retirement、outer cdecl return、guest result／entry continuation evidence
- [v86-entry-return-ingress.md](v86-entry-return-ingress.md): typed process-entry sentinel、exact completion、entry result／stack balance evidence
- [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md): Rust-owned stack region、bootstrap metadata、adapter-local stack removal evidence
- [v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md): native TEB／PEB offset oracle、import-free production reflection、zero-FS-base exact fault evidence
- [v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md): typed Rust TEB／PEB regions、FS-base restore、exact field／entry completion evidence
- [v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md): native process-parameters／command-line oracle、zero guest pointer、explicit result `2` evidence
- [v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md): typed process-parameters／command-line ownership、exact bytes、source-v86 completion evidence

## 2. 計画上の前提

### 2.1 Solo development model

開発者、利用者、maintainer は同一の 1 人である。文書内の owner role は別 team の存在を意味せず、設計判断時に明示的に切り替える責任観点を表す。

| Concern | 主責務 | 運用規則 |
|---|---|---|
| Runtime / CPU | x86、guest memory、scheduler、Wasm optimization | Phase 1 gate を越えるまで広い Win32 実装を始めない |
| Windows compatibility | PE、ABI、Kernel32／User32／GDI、contract oracle | title failure を最小 fixture へ還元してから修正する |
| Browser graphics／input | surface、GDI／GDI+、D3D9、DirectInput、presentation | primary trace で観測した範囲だけ active にする |
| Browser audio／storage | DirectSound、AudioWorklet、VFS、registry、save | data durability と clock contract を先に固定する |
| Quality / maintenance | fixture、fuzz、CI、ADR、dependency integrity | 実装 task と同じ change で最低限の evidence を追加する |

同時進行する primary implementation workstream は 1 本に限定する。別 subsystem の発見事項は backlog へ記録し、現在の phase checkpoint を再現可能にしてから切り替える。active endpoint は Phase 7A のサクラノ詩 full-play target であり、Phase 7B と Phase 8 は明示的に再開を決めるまで deferred とする。

### 2.2 Estimate の読み方

team 開発向けだった旧 elapsed range は個人開発へ換算できないため廃止する。現時点では phase gate と relative uncertainty だけを用い、calendar date は Phase 1 benchmark と primary dynamic trace が揃った後に一つ先の phase についてだけ見積もる。

| Phase | Active status | 不確実性 |
|---|---|---|
| Phase 0 Foundation | In discovery | High |
| Phase 1 Execution feasibility | Active next gate | Very high |
| Phase 2 Headless PE runtime | Planned | Very high |
| Phase 3 Win32 substrate / persistence | Planned | Very high |
| Phase 4 Window／GDI／GDI+ | Planned | High |
| Phase 5 DirectDraw comparison | Non-blocking branch | High |
| Phase 6 DirectSound／WinMM | Planned | High |
| Phase 6D bounded D3D9／DirectInput | Planned primary path | Very high |
| Phase 7A サクラノ詩 full-play target | Active endpoint | Very high |
| Phase 7B Multi-title expansion | Deferred | 未評価 |
| Phase 8 Public beta hardening | Deferred | 未評価 |

次の場合は直ちに再見積もりする。

- Phase 0 で browser、execution engine、primary critical scenario を確定したとき
- Phase 1 の normalized CPU benchmark と guest memory prototype が出たとき
- サクラノ詩の Win32／D3D9／DirectInput／audio dynamic trace を取得したとき
- GDI／font、D3D9 state、DSound／media の未知依存が判明したとき
- active target または利用可能時間が変わったとき

見積もりを完成期限として扱わない。no-go 時に受け入れ条件を暗黙に下げず、scenario、API band、architecture のどれを変えるか記録する。

## 3. Milestone と依存関係

```text
Phase 0  Foundation / corpus / decisions / test system
   |
   v
Phase 1  CPU + guest memory + ABI + scheduler feasibility        [GO / NO-GO]
   |
   v
Phase 2  bounds-safe PE/DLL loader + headless Win32 runtime
   |
   v
Phase 3  handles + threads + message + VFS + registry + persistence
   |
   +--------------------------+
   |                          |
   v                          v
Phase 4 window/GDI/GDI+   Phase 6 DirectSound/WinMM
   |                          |
   +--> Phase 5 DirectDraw    |  [non-blocking branch; no downstream gate]
   |                          |
   v                          |
Phase 6D bounded D3D9/DI      |
   |                          |
   +--------------------------+
                 |
                 v
       Phase 7A サクラノ詩 full-play target          [ACTIVE ENDPOINT]
                 |
                 v
       Phase 7B Multi-title expansion               [DEFERRED]
                 |
                 v
       Phase 8 Public beta hardening                [DEFERRED]
                 |
                 v
       Future x64 / D3D11 / DXGI / XAudio2
```

Phase 6 は Phase 3 の scheduler、clock、handle、browser topology が確定すれば開始できる。Phase 6D は Phase 4 の logical window／renderer を必要とする。Phase 7A の完了には Phase 6 と Phase 6D が必要だが、比較用の Phase 5 は必要条件ではない。solo 運用では図の並行可能性を同時実装の許可とは解釈せず、依存が少ない順に一つずつ進める。

## 4. Workstream

| ID | Workstream | 継続責務 |
|---|---|---|
| WS-GOV | Governance | personal scope、decision record、repository policy |
| WS-CORPUS | Compatibility research | corpus inventory、fixture、Windows oracle、support matrix |
| WS-CORE | Execution core | guest memory、x86、ABI、scheduler、exception |
| WS-PE | Loader / module | PE parse、map、DLL lifecycle、synthetic export |
| WS-WIN | Windows substrate | handle、thread、time、User32、encoding、COM subset |
| WS-STO | Storage | import、VFS、registry、persistence、migration |
| WS-GFX | Graphics | window bridge、surface、GDI、DirectDraw、font |
| WS-AUD | Audio | DirectSound、mixer、AudioWorklet、clock sync |
| WS-SHELL | Product shell | preflight、launch、lifecycle、save UI、diagnostics |
| WS-QA | Quality | CI、contract、differential、E2E、benchmark、compat data |

すべての phase で `WS-QA` を implementation task と同じ change に含めるが、最初は各境界を代表する automated contract 1 本に限定する。実 regression、ABI／fault ordering の曖昧さ、title failure の最小再現が生じた場合だけ test を追加する。

## 5. Task status と優先度

| 値 | 意味 |
|---|---|
| Not started | 未着手 |
| In discovery | 要件／spike／測定中 |
| In progress | 実装と test が進行中 |
| Blocked | blocker、owner、再確認条件を記録済み |
| Accepted | 対応 `AC-*` の evidence を milestone review が承認済み |

優先度は `P0` が phase gate に必須、`P1` が同 phase で強く推奨、`P2` が後続可能である。task ID の `P0` とは Phase 0 を意味し、優先度欄の `P0` とは別である。

各 task 表は計画上の scope を示すため Status 列を反復しない。現在の進捗は各 phase の冒頭と本書末尾の current queue に記録し、machine-readable traceability manifest を導入した後はそこから status view を生成する。本文の `～` や `*` は説明用の省略表記であり、機械的な traceability record では使用しない。

## 6. Phase 0: Foundation、scope、evidence system

### 6.1 目的

技術選択を暗黙に固定せず、corpus、test oracle、resource control、build／CI、主要 ADR を揃える。最初の code は product feature ではなく、execution、storage、graphics、audio 候補を比較する disposable spike とする。

現在 `P0-COR-01` は In discovery であり、7 entry executable の PE32/i386 判定、SHA-256、static import inventory は [corpus-inventory.md](corpus-inventory.md) に記録済みである。primary trace v1はmain window、D3D9／DirectInput 8／DirectSound factory、GDI+、title resource band、trace v2はcaller attributionとselected `IDirect3D9` root 11 methodsまで取得し、[windows-native-evidence.md](windows-native-evidence.md) に正規化した。device／resource COM methodとvisual title checkpointは未確定である。`P0-COR-03` の EXE／DLL／callback fixture と `P0-ARCH-01` の LLVM baseline は [toolchain-spike.md](toolchain-spike.md) で再現可能であり、Windows native callback oracleも通った。`P0-ARCH-02` は v86／Unicorn／Bochs／libx86emu の比較、v86 prebuilt Wasm の integer／x87／SSE smoke、固定 source adapter の exact budget／CPL3 fault stop／version 1 CPU state round trip、W1／W2 fixed flow、W3 browser first pixelまで進んだ。[x86-engine-scorecard.md](x86-engine-scorecard.md) に残る memory／broad ABI／Worker gateを記録した。production toolchainとengine最終選定は未確定である。

### 6.2 Entry criteria

- 単一 maintainer が project 概要、本ドキュメント群、repository policy を決定できる
- primary executable と comparison corpus の read-only inventory にアクセスできる

### 6.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P0-GOV-01 | P0 | personal target、非対象、用語、active／deferred scope を baseline 化 | accepted requirements、decision process | Q-002、R-001 |
| P0-GOV-02 | P1 | canonical command、dependency update、backup、ADR／review ownership を単一 maintainer 向けに決定 | repository policy | AC-P0-02 |
| P0-COR-01 | P0 | 7 title の static inventory を dynamic API／instruction／thread／file／registry evidence で補完 | corpus matrix | Q-001、Q-038、Q-047、A-01～03 |
| P0-COR-02 | P0 | サクラノ詩の critical scenario、checkpoint、対象 hash と、Nursery Rhyme の non-blocking comparison role を固定 | versioned title profile skeleton | R-002、R-003 |
| P0-COR-03 | P0 | T0／T1 fixture の source、builder、manifest と T2 候補を作る | fixture registry | Q-026、AC-P0-05 |
| P0-COR-04 | P0 | Windows differential oracle の event schema、runner、normalization を設計 | oracle prototype、sample trace | Q-026、R-020 |
| P0-ARCH-01 | P0 | Rust core／TypeScript host の workspace、ownership、Wasm build／debug境界を最小sliceで固定 | ADR-001 | Q-010 |
| P0-ARCH-02 | P0 | v86 working baseline のmemory hook、callback trap、Worker、ISA accuracy gateを閉じ、blocker時だけ代替engineを再評価 | ADR-002、engine scorecard | Q-011、R-010～012 |
| P0-ARCH-03 | P0 | sparse guest memory、4 GiB VA、page protection、snapshot を spike | ADR-003、memory measurements | Q-013～014、R-013 |
| P0-ARCH-04 | P0 | Runtime Worker、cooperative thread、async suspend、callback／reentrancy を model 化 | ADR-004、state machine | Q-015～016、R-014～016 |
| P0-ARCH-05 | P0 | VFS source／overlay、immutable module snapshot、backend、durability point、hard-stop rollback、quota を spike | ADR-006 | Q-030～033、Q-037、Q-039、R-030～031A |
| P0-ARCH-06 | P0 | WebGL2／WebGPU を中心に palette、RGB565、blit、text、readback と primary D3D9 state／draw workload を比較 | renderer spike data、ADR draft | Q-040～042、Q-047、R-022～024、R-027 |
| P0-ARCH-07 | P0 | AudioWorklet、SAB／copy queue、PCM loop、gesture と primary の WinMM／MCI 到達性を比較 | audio spike data、ADR draft | Q-044～046、R-025～026 |
| P0-ARCH-08 | P0 | API metadata、compatibility profile、trace／error schema を設計 | ADR-011、versioned schemas | Q-022、Q-025 |
| P0-WEB-01 | P0 | personal desktop の browser feature matrix、Tier 候補、COOP／COEP、offline／local hosting を検証 | support draft、local deployment smoke | Q-004～006、Q-016 |
| P0-TOOL-01 | P0 | workspace、toolchain pin、lockfile、format／lint／unit／Wasm build を scaffold | reproducible local build | NFR-MNT-02 |
| P0-TOOL-02 | P0 | CI、browser smoke、cache、artifact manifest を作る | required CI checks | AC-P0-04 |
| P0-QA-01 | P0 | test taxonomy、fixture ID、evidence manifest、golden update policy を実装 | test harness skeleton | AC-P0-05 |
| P0-QA-02 | P0 | 実際に使う desktop reference profile と CPU／frame／audio／memory metrics を決定 | performance plan | Q-050 |
| P0-QA-03 | P0 | 全 Must requirement を owner task と test／AC へ結ぶ完全展開 traceability manifest と検査を作る | versioned traceability data | NFR-MNT-01、AC-P0-08 |
| P0-QA-04 | P0 | Markdown lint の version／config を pin し、CJK table の line-length／table-style policy を決定 | documentation check | NFR-MNT-02、AC-P0-09 |
| P0-TOOL-03 | P0 | dependency pin、artifact hash、update check の baseline を作る | dependency integrity checks | R-044 |

### 6.4 Deliverables

- repository governance と canonical local／CI command
- selected toolchain と accepted ADR-001～004、006、011。ADR-009 は候補、測定条件、Phase 1 決定期限を記録
- fingerprint 済み primary／comparison corpus、critical scenario、fixture registry、Windows oracle prototype
- resource control と dependency integrity baseline
- browser／reference device matrix と performance measurement plan
- disposable D3D9 graphics／audio／storage／memory spike の比較結果
- Phase 1 backlog、owner、go / no-go threshold

### 6.5 Exit / acceptance

`AC-P0-01`～`AC-P0-09` をすべて満たす。特に次を milestone review で確認する。

- fresh clone の build／test／browser smoke が CI と一致する
- x86 engine 候補に Wasm portability または correctness の未処理 blocker がない
- primary scenario が driver／kernel／online service を critical path に含まない
- D3D9／DirectInput／WinMM の runtime-reachable support band と Phase 6D entry threshold が記録されている
- Phase 1 の性能、memory、callback、scheduler の測定条件と no-go 条件が数値または明確な判定規則になっている

### 6.6 明示的に行わないこと

- 広い Win32 API 実装
- title-specific patch
- PE32+／x64、D3D11、DXGI、Unity の prototype を critical path に入れること
- 実測なしの release 日付または互換性 claim

## 7. Phase 1: Execution feasibility vertical slice

### 7.1 目的

guest memory、x86 engine、ABI thunk、callback、scheduler、Worker 境界が browser で正しく、停止可能で、必要性能へ到達可能かを証明する。ここが最初の技術 go / no-go gate である。

Production PE loader を広く作る前に、固定された T1 image または loader-lite を用いて `guest instruction -> API trap -> host service -> guest callback -> yield` を貫通させる。

### 7.2 Entry criteria

- AC-P0 完了
- x86 engine／toolchain／memory／scheduler の候補と評価 threshold が承認済み
- T3 title に依存しない normalized hot-path workload がある

### 7.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P1-MEM-01 | P0 | guest address type、checked range、page／region table、reserve／commit／free を実装 | memory core + tests | REQ-MEM-01～02、R-013 |
| P1-MEM-02 | P0 | read／write／execute／guard、cross-page access、dirty／code tracking を実装 | protection state machine | Q-014 |
| P1-MEM-03 | P1 | snapshot、state hash、diagnostic memory view、physical page quota を追加 | deterministic evidence | NFR-COR-02、NFR-ROB-01 |
| P1-CPU-01 | P0 | selected engine を adapter contract に接続し checked memory／trap／budget を実装 | execution adapter | REQ-CPU-01、REQ-CPU-04 |
| P1-CPU-02 | P0 | i386 integer／flags／control／string の adopted conformance suite を通す | ISA result matrix | AC-P1-02、R-010 |
| P1-CPU-03 | P0 | x87／MMX／SSE／SSE2 候補を inventory し、adopted baseline を実装／検証 | feature matrix | Q-018 |
| P1-CPU-04 | P0 | invalid opcode、divide、memory fault、breakpoint、privileged／I/O／direct-system-call の trap taxonomy を実装 | exception events | REQ-CPU-03、REQ-CPU-07 |
| P1-ABI-01 | P0 | thunk address space と cdecl／stdcall／thiscall／adopted fastcall bridge を実装 | ABI gateway prototype | REQ-ABI-01 |
| P1-ABI-02 | P0 | scalar／pointer／string／structure の checked marshalling prototype を作る | metadata + generated/checking glue | Q-022、REQ-ABI-03 |
| P1-ABI-03 | P0 | host -> guest callback、sentinel return、nested callback、stack balance を実装 | callback harness | REQ-ABI-02、R-014 |
| P1-ABI-04 | P0 | API-specific varargs schema、bounded format parser、unsupported policy を prototype | variadic ABI policy | REQ-ABI-05、Q-028 |
| P1-SCH-01 | P0 | guest thread state、run queue、instruction slice、virtual clock、wait／wake を実装 | cooperative scheduler | Q-015、R-015 |
| P1-SCH-02 | P0 | deferred host request、suspend／resume、timeout、cancel、reentrancy state machine を実装 | async host bridge | CON-02、R-014 |
| P1-SCH-03 | P0 | Runtime Worker lifecycle、main-thread message protocol、watchdog／hard stop を実装 | worker vertical slice | NFR-ROB-01、NFR-PERF-04 |
| P1-VS-01 | P0 | controlled T1 image から pixel command、audio pulse、persistent byte write の host service を一度ずつ呼ぶ | local feasibility demo | AC-P1-10 |
| P1-DIAG-01 | P0 | instruction／API／scheduler event と first-failure report を追加 | trace prototype | REQ-DIAG-01、03 |
| P1-PERF-01 | P0 | [Measured v1] CPU、memory、thunk、callback、Workerのmicrobenchmarkを実施。SAB comparisonは必要時だけ追加 | baseline report | Q-012、Q-016、Q-050 |
| P1-PERF-02 | P0 | [Synthetic go / BGI workload v1 passed] normalized hot loopと30分fixture stressに加え、3分BGI session反復をreference desktop／Chromeで実施 | go/no-go evidence | R-011～016 |
| P1-QA-02 | P0 | instruction／wall time／memory／message／log limit と cancellation negative tests を実装 | resource control | REQ-CPU-04、NFR-ROB-01 |
| P1-QA-01 | P0 | ISA／ABI differential、deterministic replay、state hash を CI／scheduled job に接続 | core test pipeline | NFR-COR-01～02 |
| P1-ADR-01 | P0 | 実測から execution optimization、SAB／COOP/COEP、browser Tier を決定 | ADR-002 update、accepted ADR-009 | Q-012、Q-016 |
| P1-ADR-02 | P0 | API metadata、thunk、callback、marshaller の生成方式を prototype から確定 | accepted ADR-005 | Q-022、AC-P1-09 |

### 7.4 Deliverables

- browser Worker で動く bounded x86 execution core
- sparse guest memory と quota／watchdog
- ABI thunk、guest callback、deferred host call prototype
- deterministic core test、trace、state hash
- public T1 vertical slice と performance／stress report
- selected browser Tier と optimization decision

### 7.5 Exit / acceptance

`AC-P1-01`～`AC-P1-10` をすべて満たす。次の failure は exit 不可である。

- callback または async host response で guest stack／thread state が壊れる
- browser main thread を長時間 block し、stop で制御を回収できない
- high-address／sparse guest mapping が reference browser の memory budget に入らない
- ISA／ABI differential に説明できない silent mismatch が残る
- baseline または bounded optimization prototype の normalized workload が go threshold を満たさない

最適化計画だけの場合は `Conditional go` であり AC-P1 は未完了のままとする。pure PE parser、fixture、oracle 等の破棄しにくい調査は続行できるが、広い Win32 API 実装や Phase 2 完了宣言は benchmark 再合格まで行わない。

未達時は [risks-and-open-questions.md](risks-and-open-questions.md) の no-go 手順に従い、engine、scenario、personal target を再決定する。互換 API の実装量で問題を覆わない。

## 8. Phase 2: Headless PE32 / DLL runtime

### 8.1 目的

境界検査された PE32 loader、DLL lifecycle、synthetic base API、exception／TLS／LastError を統合し、synthetic EXE／DLL fixture を Windows entry point から `ExitProcess` 相当まで決定論的に実行する。

### 8.2 Entry criteria

- AC-P1 完了と技術 go decision
- API metadata、error taxonomy、module resolution policy が accepted
- loader normal／negative fixture と fuzz seed が用意済み

### 8.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P2-PE-01 | P0 | immutable bounded reader と DOS／COFF／optional header／section parser を実装 | PE model | REQ-PE-01、REQ-PE-06 |
| P2-PE-02 | P0 | section mapping、alignment、zero-fill、final protection を実装 | image mapper | REQ-PE-02 |
| P2-PE-03 | P0 | base relocation と preferred／alternate image base を実装 | relocation engine | REQ-PE-03 |
| P2-PE-04 | P0 | import by name／ordinal、export、forwarder、IAT thunk を実装 | symbol resolver | REQ-PE-03 |
| P2-PE-05 | P0 | module graph、search policy、reference count、load／unload order を実装 | DLL manager | REQ-PE-05、Q-021 |
| P2-PE-06 | P0 | TLS template／callback、DLL process／thread attach order を実装 | lifecycle trace | REQ-PE-04 |
| P2-PE-07 | P0 | resource と corpus が必要とする delay import を実装 | feature support | Q-020 |
| P2-PE-08 | P0 | unsupported PE32+、driver、managed indicator を preflight で分類 | structured errors | REQ-IMP-04 |
| P2-PE-09 | P0 | corpus が必要とする API-set／system module alias、manifest／activation context の解決と未対応診断を実装 | dependency resolution policy | REQ-PE-07、Q-027、AC-P2-08 |
| P2-PE-10 | P0 | immutable module object と map-time hash verification を loader contract にし、mutable backing provider の差替えを拒否 | stable module source | REQ-IMP-07、AC-P2-10 |
| P2-BOOT-01 | P0 | flat segment、FS base、initial register／stack／sentinel、process entry context を実装 | PE32 bootstrap context | REQ-BOOT-01 |
| P2-BOOT-02 | P0 | adopted PEB／TEB、loader list、process parameters、command line／environment／CWD、TLS slot layout を実装 | bootstrap support band | REQ-BOOT-02、Q-019 |
| P2-WIN-01 | P0 | per-thread LastError、process／thread identity と bootstrap structure を API／scheduler に接続 | native boundary | REQ-CPU-05、REQ-API-02 |
| P2-WIN-02 | P0 | VirtualAlloc／protect、heap、module、process exit の最小 synthetic ntdll／Kernel32 band を実装 | base DLL band 0 | REQ-API-01～06 |
| P2-WIN-03 | P0 | `LoadLibrary`／`GetProcAddress`／`FreeLibrary` 相当を module graph と接続 | dynamic loading | REQ-API-04 |
| P2-WIN-04 | P0 | guest exception dispatch と adopted SEH／unwind path を実装 | exception bridge | REQ-ABI-04、Q-017 |
| P2-ABI-01 | P0 | API schema から export／thunk／marshaller／status docs を生成 | generated synthetic DLL metadata | ADR-005 |
| P2-DIAG-01 | P0 | module map、load trace、unresolved import、guest stack trace を実装 | loader diagnostic | REQ-DIAG-03～04 |
| P2-QA-01 | P0 | valid PE feature matrix と Windows lifecycle oracle を CI へ接続 | loader contract suite | AC-P2-01～04 |
| P2-QA-02 | P0 | malformed PE corpus、parser／loader fuzz、allocation limit test を追加 | parser safety evidence | AC-P2-05 |
| P2-QA-03 | P0 | supported compiler／CRT startup と immutable source replacement／delay-load fixture を追加 | bootstrap／source contracts | AC-P2-09～10 |
| P2-VS-01 | P0 | EXE -> native guest DLL -> synthetic API -> callback -> exit fixture を完成 | headless public demo | AC-P2-03 |
| P2-COR-01 | P0 | サクラノ詩を local dev harness で load し、最初の unsupported import／bootstrap point までの trace を取得 | assumption checkpoint | AC-P2-11。合否は title 成功ではなく architecture review |

### 8.4 Deliverables

- PE32 parser／mapper と DLL module graph
- import／export／relocation／TLS／resource／必要な delay import
- generated API thunk と synthetic base DLL band 0
- guest exception、TEB／LastError、heap／virtual memory の最小 runtime
- adopted PEB／TEB／FS／process parameters と compiler／CRT startup profile
- immutable module object、map-time hash、external replacement rejection
- loader differential／fuzz suite と headless EXE／DLL demo

### 8.5 Exit / acceptance

`AC-P2-01`～`AC-P2-11` をすべて満たす。追加条件:

- relocation と IAT 書き込みは guest memory のみに行われ、source file hash が不変
- TLS callback、DLL attach、entry point の順序を trace と oracle で確認
- cdecl／stdcall 等の stack cleanup と per-thread LastError が DLL 境界を越えて正しい
- unsupported import／PE feature は hang／silent success ではなく、module と symbol を含む診断で停止
- malformed fixture が parser／marshaller の bounds check を通過しない

## 9. Phase 3: Win32 substrate、VFS、registry、persistence

### 9.1 目的

title が UI／graphics を初期化する前提となる handle、thread／sync、clock、message queue、encoding、file、registry を実装し、browser restart をまたぐ安全な save round-trip を成立させる。

### 9.2 Entry criteria

- AC-P2 完了
- VFS backend ADR、guest clock、handle schema、API support band 1 が accepted
- storage kill／quota／namespace test を実行できる browser harness がある

### 9.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P3-WIN-01 | P0 | generation 付き typed handle table と object lifetime／wait interface を実装 | object manager | REQ-API-02～03 |
| P3-WIN-02 | P0 | thread create／exit、event、mutex、semaphore、wait、sleep、timer を scheduler に接続 | base DLL band 1 | REQ-API-03、05 |
| P3-WIN-03 | P0 | message queue、Post／Send、Peek／Get、dispatch、timer callback の core を実装 | User32 headless core | REQ-UI-02、R-014 |
| P3-WIN-04 | P1 | APC／alertable wait／priority の corpus 必要範囲を inventory し、support／unsupported policy を実装 | threading profile | Q-017 |
| P3-WIN-05 | P0 | critical section、interlocked、TLS Alloc／Get／Set／Free、thread-exit cleanup を実装 | synchronization／TLS band | REQ-API-03、AC-P3-01 |
| P3-COM-01 | P0 | IUnknown ABI、vtable thunk、HRESULT、interface identity、reference count の共通 foundation を実装 | COM ABI foundation | REQ-COM-01、AC-P3-08 |
| P3-I18N-01 | P0 | UTF-16、Windows-932、ANSI／Unicode API glue、invalid sequence policy を実装 | encoding service | REQ-I18N-01 |
| P3-I18N-02 | P0 | title locale、timezone、code page、font mapping metadata を profile に追加 | locale profile | REQ-I18N-02、Q-034 |
| P3-STO-01 | P0 | Windows drive／CWD／path parser、case-insensitive object lookup、mount policy を実装 | VFS namespace | REQ-FS-01 |
| P3-STO-02 | P0 | read-only source + writable overlay、create／read／write／seek／enumerate／metadata を実装 | file API band 1 | REQ-FS-02～03 |
| P3-STO-03 | P0 | sharing、locking、delete／rename、mapped file の adopted semantics を実装 | file API band 2 | REQ-FS-04、Q-032 |
| P3-STO-04 | P0 | OPFS／IndexedDB 等の backend adapter、journal／atomic commit、quota handling を実装 | persistent volume | REQ-FS-05、R-030 |
| P3-STO-05 | P0 | typed virtual registry、HKCU／title-local classes、enumeration、persistence を実装 | registry service | REQ-REG-01～03 |
| P3-STO-06 | P0 | title namespace、schema version、snapshot、export／import／reset、migration skeleton を実装 | save lifecycle | REQ-SHELL-03 |
| P3-STO-07 | P0 | content-addressed module snapshot、restart lifetime、external replacement／overlay shadow rejection を browser backend に実装 | immutable module store | REQ-IMP-07、Q-037、AC-P3-10 |
| P3-STO-08 | P0 | write／flush／close／transaction の durability point と hard-stop rollback を実装 | durability contract | REQ-FS-05、Q-039、AC-P3-11 |
| P3-ARCH-01 | P0 | P0 spike と GDI／DDraw fixture から renderer と共通 surface 方針を確定 | accepted ADR-007 | Q-040～043 |
| P3-ARCH-02 | P0 | P0 spike と DirectSound fixture から mixer、AudioWorklet、clock 方針を確定 | accepted ADR-008 | Q-044～046 |
| P3-DIAG-01 | P0 | handle、thread、wait、file、registry、quota event を共通 trace schema に追加 | substrate diagnostics | REQ-DIAG-01 |
| P3-QA-01 | P0 | thread／wait／message／timer の model／Windows differential suite を追加 | scheduling contracts | AC-P3-01～02 |
| P3-QA-02 | P0 | path／case／sharing／CP932／registry differential suite を追加 | storage contracts | AC-P3-03～04 |
| P3-QA-03 | P0 | browser close、Worker kill、quota full、permission revoke を追加 | persistence evidence | AC-P3-05～06 |
| P3-QA-04 | P0 | external module replacement、restart hash、durability point ごとの kill injection を追加 | source／durability evidence | AC-P3-10～11 |
| P3-QA-05 | P0 | case collision、archive／file limit の必要な negative case を追加 | VFS robustness gate | R-031～033 |
| P3-VS-01 | P0 | headless fixture が message／timer／file／registry を使い、restart 後に同じ state hash へ戻る | persistence demo | AC-P3-05 |
| P3-COR-01 | P0 | サクラノ詩を bootstrap し、thread／message／file／registry の最初の mismatch を trace で確認 | assumption checkpoint | AC-P3-12。title 成功を phase gate にしない |

### 9.4 Deliverables

- handle／object、thread／wait、clock／timer、headless message queue
- DirectDraw／DirectSound が共有する IUnknown ABI／vtable／HRESULT foundation
- Windows-932／Unicode と title locale profile
- Windows path semantics を持つ source／overlay VFS
- restart-safe immutable module store と external replacement rejection
- crash-consistent browser persistence と virtual registry
- 明示した durability point と hard-stop rollback
- save export／import／reset／migration skeleton
- renderer／surface と AudioWorklet／clock の accepted ADR
- storage／scheduler differential、kill、quota tests

### 9.5 Exit / acceptance

`AC-P3-01`～`AC-P3-06` と `AC-P3-08`～`AC-P3-12` を満たす。追加条件:

- guest thread ごとの LastError、TLS、wait state が分離される
- synchronous Send／nested callback と asynchronous Post が trace で区別され、deadlock fixture を停止できる
- CP932 filename／string／registry が host locale に依存せず round-trip する
- source volume は書き換わらず、save／registry は title-local overlay のみに commit される
- commit interruption／hard stop 後も last good committed save へ復帰でき、未commit write を自動commit しない

## 10. Phase 4: Window、input、interactive GDI／GDI+ vertical slice

### 10.1 目的

logical Win32 window と browser canvas／input を接続し、VN-like fixture が message loop を通じて日本語 text と bitmap を描き、keyboard／mouse に反応する最初の interactive slice を完成させる。サクラノ詩で到達する GDI+ image subset もここで共通 surface／VFS に接続する。

### 10.2 Entry criteria

- AC-P3 完了
- renderer／surface／font ADR が accepted
- graphics golden を browser ごとに生成／比較できる
- main thread と Runtime Worker の presentation／input protocol が versioned schema を持つ

### 10.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P4-UI-01 | P0 | window class、logical HWND、create／show／size／focus／destroy、window tree を実装 | User32 window band | REQ-UI-01 |
| P4-UI-02 | P0 | paint invalidation、BeginPaint／EndPaint、Send／Post message と presentation を接続 | paint lifecycle | REQ-UI-02 |
| P4-UI-03 | P0 | DOM keyboard／mouse event を timestamp 付き normalized event へ変換 | main-thread input adapter | REQ-UI-03 |
| P4-UI-04 | P0 | virtual key、scan code、repeat、modifier、mouse coordinate／button／wheel、capture を実装 | User32 input band | REQ-UI-03 |
| P4-UI-05 | P0 | focus loss、stuck key、resize、DPR、aspect、fullscreen lifecycle を実装 | browser lifecycle | REQ-UI-04 |
| P4-GFX-01 | P0 | CPU-visible surface storage、pixel format、pitch、orientation、dirty region を実装 | common surface core | REQ-GFX-01、Q-040 |
| P4-GFX-02 | P0 | renderer backend と Runtime Worker -> presentation queue を実装 | accepted graphics backend | Q-041 |
| P4-GDI-01 | P0 | DC と bitmap／brush／pen／font／palette／region の typed object と selection rule を実装 | GDI object model | REQ-GDI-01 |
| P4-GDI-02 | P0 | DIB、pixel conversion、palette、BitBlt／StretchBlt、adopted ROP、clip を実装 | GDI raster band 1 | REQ-GDI-02～03 |
| P4-GDI-03 | P0 | text encoding、font selection、glyph／metric、basic TextOut 系を実装 | GDI text band 1 | REQ-GDI-02、REQ-I18N-03 |
| P4-GDI-04 | P0 | line／rectangle／fill 等、corpus が要求する primitive を実装 | GDI primitive band | REQ-GDI-02 |
| P4-GDIP-01 | P0 | runtime-reachable GDI+ startup、bitmap file／DIB create、LockBits／UnlockBits、load／save subset を実装 | GDI+ image band 1 | REQ-GDIP-01 |
| P4-I18N-01 | P0 | font candidate、title mapping、missing glyph／metric report を確定 | font profile + metric evidence | Q-042、R-023 |
| P4-DIAG-01 | P0 | window、message、GDI object、draw command、frame timing を trace に追加 | visual diagnostics | REQ-DIAG-01 |
| P4-QA-01 | P0 | window／message／input deterministic replay と browser E2E を追加 | interaction suite | AC-P4-01、04 |
| P4-QA-02 | P0 | exact pixel、semantic state、font tolerance golden suite を追加 | graphics oracle | AC-P4-02～03 |
| P4-QA-03 | P0 | context loss、resize、DPR、focus、fullscreen injection を追加 | recovery suite | NFR-REL-02 |
| P4-PERF-01 | P0 | upload／readback／dirty rect／text／frame cadence を profile し baseline 化 | graphics benchmark | NFR-PERF-02 |
| P4-QA-04 | P0 | image dimension、pitch、rectangle、object count、frame queue limit を検査 | graphics limits + fuzz | R-032～033 |
| P4-VS-01 | P0 | CP932 text／background／sprite を描き、keyboard／mouse で scene state が変わる T1／T2 fixture を完成 | interactive GDI demo | AC-P4-01 |
| P4-COR-01 | P0 | サクラノ詩の window 作成から GDI／GDI+ checkpoint までを local harness で試し、font／image mismatch を分類 | assumption checkpoint | AC-P4-06～07。title 成功を phase gate にしない |

### 10.4 Deliverables

- logical window／HWND と browser input／presentation bridge
- common CPU-visible surface と selected renderer
- GDI DC／object、DIB／palette／blit／clip／text と bounded GDI+ image band
- Japanese font substitution profile と metric diagnostics
- interactive synthetic VN-like fixture、golden、browser recovery tests
- graphics performance／resource baseline

### 10.5 Exit / acceptance

`AC-P4-01`～`AC-P4-07` をすべて満たす。追加条件:

- input event は focus と timestamp を保ち、browser shortcut で取得不能な key を support matrix に記載する
- GDI handle lifetime、selected object、delete failure、DC state が contract test と一致する
- bitmap の top-down／bottom-up、palette、pitch、clip で guest memory 外を読まない
- font 差を「見た目が近い」だけで合格させず、missing glyph、baseline、advance、line break、clip／overlap を versioned threshold で判定する
- resize／DPR／context loss 後も logical client size と guest coordinate が一貫する

## 11. Phase 5: DirectDraw comparison branch

### 11.1 目的

Nursery Rhyme と PURIPA が要求する DirectDraw COM contract と 2D surface semantics を共通 surface／renderer に接続し、palette、16 bit pixel、lock、blit、flip、logical fullscreen を正確に扱う。この branch は同系統 BGI の比較には有用だが、サクラノ詩 personal target の entry／exit gate ではない。

### 11.2 Entry criteria

- maintainer が comparison branch の開始を明示的に決定
- AC-P4 完了
- corpus の DirectDraw interface version、method、pixel format、cooperative mode inventory がある
- COM ABI／reference count の範囲と system DLL resolution policy が accepted

### 11.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P5-COM-01 | P0 | 共通 COM ABI 上に DirectDraw の QueryInterface、interface version、identity／reference count を実装 | DirectDraw COM bindings | REQ-COM-01、AC-P5-01 |
| P5-DD-01 | P0 | DirectDraw object、interface version、capability query、display mode model を実装 | DirectDraw root | REQ-DD-01～02 |
| P5-DD-02 | P0 | surface descriptor、primary／offscreen／backbuffer、flip chain を実装 | surface object graph | REQ-DD-01 |
| P5-DD-03 | P0 | 8 bit indexed palette、RGB555／565、adopted 24／32 bit format を実装 | pixel format band | REQ-GDI-03、REQ-DD-01 |
| P5-DD-04 | P0 | Lock／Unlock、rectangle、pitch、nested／busy policy、dirty upload を実装 | CPU surface access | REQ-DD-01 |
| P5-DD-05 | P0 | Blt／BltFast、fill、source／destination color key、clip の adopted flags を実装 | DDraw blitter band 1 | REQ-DD-01 |
| P5-DD-06 | P0 | Flip、presentation cadence、GetFlipStatus／GetBltStatus の adopted semantics を実装 | presentation path | REQ-DD-01 |
| P5-DD-07 | P0 | cooperative level、logical mode switch、browser fullscreen／focus policy を実装 | display policy | REQ-DD-02、Q-043 |
| P5-DD-08 | P0 | surface lost／restore、context loss、resize 後の resource reconstruction を実装 | recovery path | REQ-DD-03 |
| P5-DD-09 | P0 | corpus が呼ぶ全 vtable method を implemented／partial／unsupported に分類 | invoked-method closure | REQ-DD-04、AC-P5-06 |
| P5-DD-10 | P0 | adopted clipper、GetDC／ReleaseDC、mode enumeration、vertical blank／scanline を実装／診断 | window／GDI interop band | REQ-DD-04、P4-GDI-01 |
| P5-DIAG-01 | P0 | COM／surface lifecycle、format、lock、blit、flip timing を trace に追加 | DDraw diagnostics | REQ-DIAG-01 |
| P5-QA-01 | P0 | vtable／QueryInterface／reference count／HRESULT differential fixture を追加 | COM contracts | AC-P5-01 |
| P5-QA-02 | P0 | pitch／lock／palette／format の exact memory oracle を追加 | surface contracts | AC-P5-02 |
| P5-QA-03 | P0 | blit／color key／flip／mode／lost surface の semantic + golden suite を追加 | DDraw contracts | AC-P5-03～04 |
| P5-QA-04 | P0 | clipper／GetDC／ReleaseDC／mode enumeration／vblank の adopted oracle を追加 | DDraw interop contracts | AC-P5-06 |
| P5-PERF-01 | P0 | upload strategy、palette conversion、frame queue、readback を benchmark | DDraw baseline | R-022、R-024 |
| P5-QA-05 | P0 | surface count／dimension／pitch／lock duration／GPU allocation limit と fuzz を追加 | DDraw resource control | NFR-ROB-01 |
| P5-VS-01 | P0 | palette／16 bit scene、sprite color key、flip、input を使う VN-like fixture を完成 | playable visual demo | AC-P5-05 |
| P5-COR-01 | P0 | Nursery Rhyme の DirectDraw create から first present までを local harness で試し、surface／method gap を分類 | comparison checkpoint | AC-P5-07。title 成功を phase gate にしない |

### 11.4 Deliverables

- COM ABI band 0 と DirectDraw root／surface graph
- adopted pixel format、palette、lock、blit、color key、flip
- logical display mode／fullscreen と loss／restore path
- exact surface memory oracle、semantic／golden suite、60 分 visual soak

### 11.5 Exit / acceptance

`AC-P5-01`～`AC-P5-07` をすべて満たす。追加条件:

- guest-visible pitch、surface memory、lock rectangle と実 backend allocation を混同しない
- 8 bit palette と RGB555／565 の exact conversion を全値または十分な exhaustive vector で確認する
- COM reference count／interface identity と destroyed object behavior が deterministic
- browser fullscreen が拒否されても guest／shell が recover し、host display mode を変更しない
- surface loss／context loss で source／save state を壊さず、resource leak がない

Phase 5 が未着手、途中、または Deferred でも Phase 6D／7A へ進める。Phase 5 の code が共通 surface／COM ABI を変更する場合は active target の synthetic regression を通す。

## 12. Phase 6: DirectSound and Web Audio

Phase 6 の core audio task は AC-P3 完了後に Phase 4～5 と並行開始できる。ただし DirectSound の HWND／cooperative-level 統合と `AC-P6-04` の graphics + audio soak は Phase 4 の logical window／interactive fixture に依存し、Phase 6 の exit には AC-P4 完了が必要である。Phase 7A では graphics と audio の clock／lifecycle を再統合する。

### 12.1 目的

DirectSound buffer と cursor semantics を runtime clock に接続し、AudioWorklet を real-time consumer として BGM／voice／effect の PCM playback、loop、volume、pan、streaming を安定して提供する。

### 12.2 Entry criteria

- AC-P3 完了
- core 実装は Phase 4 と並行可能だが、phase exit 前に AC-P4 を満たす
- corpus の DirectSound interface、PCM format、buffer usage、notification／streaming inventory がある
- AudioWorklet transport、SAB／fallback、clock ownership ADR が accepted

### 12.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P6-AUD-01 | P0 | guest monotonic clock、audio sample clock、drift observation／correction policy を確定 | audio clock model | Q-045、R-025 |
| P6-COM-01 | P0 | 共通 COM ABI 上に DirectSound の interface version、identity／reference count を実装 | DirectSound COM bindings | REQ-COM-01、AC-P6-01 |
| P6-DS-01 | P0 | DirectSound device、capability query、HWND／cooperative level、primary／secondary buffer object を実装 | DirectSound object graph | REQ-DS-01、P4-UI-01 |
| P6-DS-02 | P0 | PCM 8／16 bit、mono／stereo、adopted sample rates、format validation を実装 | PCM format band | REQ-DS-02 |
| P6-DS-03 | P0 | buffer Lock／Unlock、wrap-around、dirty region、streaming producer を実装 | guest audio buffer | REQ-DS-03 |
| P6-DS-04 | P0 | Play／Stop／loop、volume、pan、frequency、status を実装 | playback controls | REQ-DS-01～02 |
| P6-DS-05 | P0 | play／write cursor、notification、thread wake と scheduler integration を実装 | timing semantics | REQ-DS-02～03 |
| P6-DS-06 | P0 | invalid／destroyed HWND、focus／deactivation、buffer lost／Restore、primary format permission を実装 | window／recovery semantics | REQ-DS-05～06 |
| P6-AUD-02 | P0 | mixer、resampler、clipping、queue／ring producer を実装 | runtime audio pipeline | REQ-AUD-01 |
| P6-AUD-03 | P0 | AudioWorklet consumer、queue depth、underrun／overrun counter を実装 | Web Audio backend | REQ-AUD-01 |
| P6-AUD-04 | P0 | user activation pending、AudioContext suspend／resume、background、device change policy を実装 | browser audio lifecycle | REQ-DS-04 |
| P6-AUD-05 | P0 | unsupported codec／MCI／ACM／DirectShow call を inventory、分類、診断 | media support boundary | REQ-AUD-02、Q-046 |
| P6-MCI-01 | P0 | primary scenario で到達する `PlaySoundA`／`mciSendCommandA` command、callback／notification、format subset を実装 | bounded WinMM／MCI band | REQ-AUD-03、Q-046 |
| P6-DIAG-01 | P0 | buffer state、cursor、queue depth、drift、underrun、worklet state を trace／metric に追加 | audio diagnostics | REQ-DIAG-01 |
| P6-QA-01 | P0 | buffer／mixer／cursor／notification の offline semantic oracle を追加 | DirectSound contracts | AC-P6-01～02 |
| P6-QA-02 | P0 | HWND／cooperative level／focus／activation／suspend／buffer-lost／recovery の browser E2E を追加 | lifecycle suite | AC-P6-03、05 |
| P6-QA-03 | P0 | graphics + audio integrated 2 時間 soak と drift／underrun measurement を追加 | realtime evidence | AC-P6-04 |
| P6-QA-04 | P0 | buffer size／count、sample rate、queue、worklet message、decode failure の limit／fuzz を追加 | audio resource control | NFR-ROB-01 |
| P6-VS-01 | P0 | loop BGM、streaming voice 相当、effect、pause／resume を使う VN-like fixture を完成 | playable media demo | AC-P6-04 |
| P6-COR-01 | P0 | サクラノ詩の DirectSound create から first audio checkpoint までを local harness で試し、HWND／cursor／MCI／media gap を分類 | assumption checkpoint | AC-P6-06～07。title 成功を phase gate にしない |

### 12.4 Deliverables

- DirectSound device／buffer、adopted PCM／control／cursor／notification
- mixer／resampler、runtime ring、AudioWorklet backend
- browser audio activation／lifecycle state machine
- offline audio oracle、realtime E2E、2 時間 graphics／audio soak
- media／codec の explicit unsupported inventory
- primary scenario で到達する WinMM／MCI command subset と fixture

### 12.5 Exit / acceptance

`AC-P6-01`～`AC-P6-07` をすべて満たす。追加条件:

- Lock の wrap、play／write cursor、notification order を browser callback timing へ直接依存させない
- DirectSound cooperative／focus state と browser user activation state を分離し、どちらかを他方の代用にしない
- user activation を得るまで guest を壊れた成功状態へ進めず、shell と runtime の待機状態が一致する
- AudioWorklet 内で guest memory parse、file I/O、unbounded allocation を行わない
- 2 時間試験の threshold は実測から確定し、underrun を平均値で隠さない
- codec／media dependency は DirectSound 対応と誤表示せず、unsupported call site を示す

## 12D. Phase 6D: Bounded D3D9 and DirectInput 8

### 12D.1 目的

サクラノ詩の runtime trace で観測した D3D9／DirectInput 8 subset を logical window、common presentation、normalized input へ接続する。一般的な D3D9 compatibility を作る phase ではなく、method、format、state、device path を versioned manifest で閉じる。

### 12D.2 Entry criteria

- AC-P3 と AC-P4 完了。Phase 6 audio は並行可能だが、Phase 7A entry 前に完了する
- 対象 hash、critical scenario、Windows trace の開始／終了 checkpoint が固定済み
- CreateDevice から first present／representative scene までの D3D9 vtable、format、render state inventory がある
- DirectInput 8 の device、data format、cooperative level、到達 method inventory がある
- ADR-007 が D3D9 resource／presentation boundary と WebGL2／WebGPU backend を含めて Accepted

### 12D.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P6D-COR-01 | P0 | サクラノ詩の D3D9 interface／vtable slot、CreateDevice parameters、format、state、draw、Reset／loss trace を取得 | versioned D3D9 call manifest | Q-047、R-027 |
| P6D-COR-02 | P0 | DirectInput 8 device、data format、cooperative level、Acquire／state／event call trace を取得 | versioned DirectInput call manifest | Q-038 |
| P6D-COM-01 | P0 | IDirect3D9／IDirect3DDevice9／resource interface の identity、QueryInterface、reference count、HRESULT、vtable thunk を実装 | D3D9 COM bindings | REQ-COM-01、REQ-D3D9-01 |
| P6D-D3D-01 | P0 | adapter／mode／caps の観測 query と CreateDevice／presentation parameters を実装 | D3D9 root／device band | REQ-D3D9-01 |
| P6D-D3D-02 | P0 | target-required surface、texture、vertex／index buffer、format、pool／usage state を実装 | bounded resource graph | REQ-D3D9-02 |
| P6D-D3D-03 | P0 | Lock／Unlock、pitch、rectangle、staging、upload／readback を guest memory contract に接続 | resource transfer path | REQ-D3D9-02 |
| P6D-D3D-04 | P0 | target-required transform、render／texture-stage／sampler state、vertex format、blend、draw を backend command へ変換 | bounded fixed-function path | REQ-D3D9-03 |
| P6D-D3D-05 | P0 | BeginScene／EndScene／Present、Reset、cooperative state、context／device loss を実装 | presentation／recovery state machine | REQ-D3D9-04 |
| P6D-INP-01 | P0 | DirectInput8 root、keyboard／mouse device、data format、cooperative level、Acquire lifecycle を実装 | DirectInput object band | REQ-DI-01～02 |
| P6D-INP-02 | P0 | GetDeviceState／GetDeviceData、buffering、focus loss、reacquire、overflow を normalized input queue へ接続 | DirectInput event path | REQ-DI-02～03 |
| P6D-DIAG-01 | P0 | COM slot、resource、format、state delta、draw、present、input acquisition を trace に追加 | D3D9／input diagnostics | REQ-DIAG-01、03 |
| P6D-QA-01 | P0 | COM／device／resource／state／draw の Windows semantic fixture と pixel golden を追加 | D3D9 contract suite | AC-P6D-01～05 |
| P6D-QA-02 | P0 | DirectInput state／buffer／focus／reacquire の Windows／DOM replay fixture を追加 | DirectInput contract suite | AC-P6D-06 |
| P6D-QA-03 | P0 | context loss、Reset、resize、focus、60 分 reference scene、resource growth を browser E2E で検証 | D3D9 stability evidence | AC-P6D-05、07 |
| P6D-QA-04 | P0 | resource dimension／count、lock range、draw count、state／command queue、GPU allocation limit と fuzz を追加 | graphics resource controls | R-027、NFR-ROB-01 |
| P6D-VS-01 | P0 | synthetic D3D9 scene とサクラノ詩の first-present checkpoint を実行 | primary graphics checkpoint | AC-P6D-05。title 完走は Phase 7A gate |

### 12D.4 Deliverables

- primary target の versioned D3D9／DirectInput call manifest
- bounded D3D9 COM／device／resource／fixed-function／presentation frontend
- DirectInput 8 keyboard／mouse frontend
- WebGL2 または WebGPU backend mapping と context／device-loss recovery
- semantic fixture、pixel golden、input replay、60 分 stability evidence

### 12D.5 Exit / acceptance

`AC-P6D-01`～`AC-P6D-07` をすべて満たす。追加条件:

- support status は import 名ではなく vtable slot、method variant、format、state、flag 単位で記録する
- guest-visible pitch／lock bytes と backend resource layout を同一と仮定しない
- 未観測／未実装 method は成功を返さず、call site と slot を診断する
- shader path または未計画の media／swap-chain dependency が primary critical path に現れた場合は Phase 7A へ進まず scope review を行う

## 13. Phase 7A: サクラノ詩 full-play target

### 13.1 目的

fingerprint 済みのサクラノ詩 `BGI.exe` を immutable snapshot から import し、boot、title、全シナリオ／選択肢／ending、User32／DirectInput input、GDI／GDI+／D3D9 graphics、DirectSound／到達 WinMM、save、browser restart、load まで通す。同時に、個人利用に必要な最小 browser shell と diagnostic を整える。

この phase では一つの title を動かすが、title-specific binary patch、asset decoder、scenario VM 再実装を許可しない。差分修正は一般化した API semantics、CPU／loader fix、または明示的で declarative な title profile に限定する。

### 13.2 Entry criteria

- AC-P6 と AC-P6D 完了。AC-P5 は不要
- サクラノ詩 profile、fingerprint、critical scenario、checkpoint が確定

### 13.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P7A-SHELL-01 | P0 | directory／file-set import、capacity／file-count check、title fingerprint を実装 | local import flow | REQ-IMP-01～02、05 |
| P7A-SHELL-02 | P0 | PE／DLL／import／CPU／graphics／audio preflight で既知の unsupported と静的には不明な項目を示し、compatible 保証と区別 | compatibility preflight | REQ-IMP-04、REQ-SHELL-02、AC-P7-DP-04 |
| P7A-SHELL-03 | P0 | library、launch、audio activation、pause／resume／stop／reset、fullscreen を実装 | runtime shell | REQ-SHELL-01 |
| P7A-SHELL-04 | P0 | save usage、export／import／backup／delete、permission／quota status UI を実装 | save management | REQ-FS-05、REQ-SHELL-03 |
| P7A-SHELL-05 | P0 | update／storage schema migration／rollback の user-visible flow を実装 | migration flow | NFR-REL-01 |
| P7A-COR-01 | P0 | static inventory 済みのサクラノ詩について、残る dynamic API／instruction／thread／file／registry／media trace gap を閉じる | gap report | Q-001、R-003、corpus inventory |
| P7A-COR-02 | P0 | gap を `general runtime defect`、`partial API`、`unsupported scope` に triage | compatibility backlog | R-001、R-021 |
| P7A-COR-03 | P0 | generalizable CPU／PE／API／graphics／audio gap を fixture 化して修正 | regression contracts | Definition of Done |
| P7A-COR-04 | P0 | fingerprint、locale、entry executable、drive mapping 等の declarative profile を作成 | サクラノ詩 profile | REQ-I18N-02 |
| P7A-DIAG-01 | P0 | first unsupported／mismatch、guest call stack、module、runtime／browser を report | compatibility diagnostic | REQ-DIAG-03～04 |
| P7A-DIAG-02 | P0 | diagnostic bundle preview／export を実装 | support artifact | REQ-DIAG-01、REQ-DIAG-04 |
| P7A-QA-03 | P0 | source snapshot／hash、external replacement、immutable module load、resource limit を title flow で再検証 | title consistency evidence | REQ-IMP-07、AC-P7-DP-02 |
| P7A-QA-01 | P0 | clean local profile で段階 checkpoint を記録し、最終的に利用者が意図する全編／route／ending の実プレイを完了する | full-play journal | AC-P7-DP-01 |
| P7A-QA-02 | P0 | 長時間 play、multi-save、restart／load、focus／audio resume を実施 | play-session evidence | NFR-REL-01～02 |
| P7A-UX-01 | P1 | shell の keyboard、focus、status／error announcement を検証 | host accessibility evidence | NFR-ACC-01 |

### 13.4 Gap closure rule

実 title で failure が出た場合、次の順に処理する。

1. 最小の T0／T1 fixture または trace replay で再現する。
2. Windows oracle、仕様、runtime invariant のどれに反するか分類する。
3. 同じ API／命令／format を使う既存 fixture への影響を確認する。
4. 一般 runtime fix と negative／regression test を実装する。
5. title profile が必要な場合、意味、適用条件、互換性への影響を文書化する。
6. driver／kernel、非対象 media が原因なら unsupported として分類する。

禁止する workaround:

- executable／DLL／asset の保存バイト列 patch
- guest EIP や title hash にだけ依存する code skip
- API success を返すだけで state を作らない silent stub
- title 固有の scenario／script／asset decoder

### 13.5 Exit / acceptance

`AC-P7-DP-01`～`AC-P7-DP-04` をすべて満たし、この fingerprint のサクラノ詩を全編不自由なくプレイできた時点を active roadmap の完了とする。これはサクラノ詩 1 build の full-play compatibility であり、一般の VN／Win32／D3D9 compatibility を意味しない。

## 14. Phase 7B: Optional multi-title expansion [Deferred]

### 14.1 目的

サクラノ詩で得た実装を別 title へ広げる場合の task template である。現在は active roadmap に含めず、固定本数、runtime 系統数、MVP claim を設定しない。

### 14.2 Entry criteria

- AC-P7-DP 完了
- maintainer が active target の完了後に expansion 開始を明示的に決定
- 対象 corpus の profile、scenario、本数、停止 budget を新たに確定
- first-title 由来の API／CPU／performance debt と residual risk が review 済み

### 14.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P7B-COR-01 | P0 | 各 title の static／dynamic inventory と critical scenario を version 管理 | corpus profiles | REQ-DIAG-04 |
| P7B-COR-02 | P0 | title 追加ごとに gap closure rule を適用し、一般 fixture／API contract を増やす | shared compatibility fixes | R-001、R-003 |
| P7B-COR-03 | P0 | API／PE／CPU feature の actual call coverage と partial behavior を集計 | coverage matrix | NFR-COMP-01 |
| P7B-PROF-01 | P0 | declarative profile schema、validation、migration、runtime version compatibility を完成 | profile system | REQ-IMP-05 |
| P7B-PROF-02 | P0 | profile option の濫用を監査し、hash-specific code path を禁止 | architecture conformance | AC-P7-MVP-04 |
| P7B-SHELL-01 | P0 | multiple title storage isolation、library state、profile update／reset を完成 | multi-title shell | REQ-SHELL-01～03 |
| P7B-QA-01 | P0 | title ごとの clean import -> boot -> scene -> input -> save -> restart -> load scenario を実行 | compatibility suite | AC-P7-MVP-01～02 |
| P7B-QA-02 | P0 | title／browser／runtime version の result history と regression diff を作る | compatibility history | NFR-COMP-01 |
| P7B-QA-03 | P0 | 60 分以上、複数 save slot、failure injection、resource usage を各 title で検証 | expansion evidence | AC-P7-MVP-02 |
| P7B-QA-04 | P0 | 各 title の critical text scene で glyph／clip／overlap／line break を検証 | title text evidence | REQ-I18N-03、AC-P7-MVP-06 |
| P7B-PERF-01 | P0 | title workload の CPU hot block、frame、audio、memory、startup を profile し budget を確定 | product performance baseline | NFR-PERF-01～04 |
| P7B-DOC-01 | P0 | supported／partial／unsupported の API、browser、title、media matrix を生成 | expansion compatibility report | AC-P7-MVP-03、05 |
| P7B-REV-01 | P0 | requirements、risk、estimate、future scope を evidence に基づき更新 | expansion milestone review | all Q/R |

### 14.4 Exit / acceptance

この phase と `AC-P7-MVP-*` は Deferred であり、現状の active completion には適用しない。再開時は corpus 本数と目的に合わせて acceptance を改訂してから task を開始する。旧 gate を再利用する場合の追加条件:

- corpus の本数だけでなく、engine/runtime 特性が 2 系統以上である
- 各 title の source hash が import 前後で一致し、module は immutable source から load され、save／registry は独立 namespace にある
- 一つの title の fix が synthetic fixture と他 title regression suite を通る
- unknown／untested を compatible と集計しない
- support title で重大な save loss、停止不能 hang、persistent audio／visual break がない

multi-title milestone を再開しても、任意の Windows 7／10 application が動くという意味ではない。

## 15. Phase 8: Public beta hardening [Deferred]

### 15.1 目的

第三者向け配布を決めた場合に、parser、marshaller、storage、browser lifecycle、performance、supply chain を release 品質へ引き上げる task template である。完全個人用という現 scope では実施しない。

### 15.2 Entry criteria

- maintainer が個人利用から第三者向け配布へ scope を変更
- Phase 7B の改訂済み acceptance 完了
- release target browser、hosting、artifact format、versioning／rollback policy が確定
- release reviewer が割り当て済み

### 15.3 Task breakdown

| ID | Pri | Task | 成果物 | 依存／対応 |
|---|---|---|---|---|
| P8-QA-02 | P0 | PE、decoder、marshaller、path、graphics、audio fuzz target の coverage／budget を release gate 化 | fuzz evidence | AC-P8-01 |
| P8-QA-03 | P0 | resource exhaustion、Worker kill、log flood、archive bomb、storage exhaustion を stress test | resource-limit evidence | R-033 |
| P8-WEB-01 | P0 | Tier 1 browser E2E、Tier 2 smoke、feature fallback、COOP／COEP header を固定 | browser matrix | NFR-PORT-01 |
| P8-WEB-02 | P0 | offline／update／cache invalidation、old tab／new runtime、service worker 採否を検証 | deployment lifecycle | Q-004 |
| P8-STO-01 | P0 | storage schema migration、backup、rollback、quota／eviction recovery、delete を全 supported browser で検証 | migration evidence | AC-P8-02、06 |
| P8-PERF-01 | P0 | startup、frame、audio、memory、save、bundle の regression budget を CI／scheduled job に固定 | performance gates | MET-* |
| P8-REL-01 | P0 | integrated 8 時間 soak、pause／resume、context／audio loss、periodic save／load を実行 | reliability evidence | AC-P8-02 |
| P8-SC-01 | P0 | lock、SBOM、reproducible build、artifact hash／attestation、dependency update policy を完成 | supply-chain evidence | R-044 |
| P8-SHELL-01 | P0 | first-run browser feature check、actionable error、backup／delete、supported scope を完成 | beta shell | REQ-SHELL-* |
| P8-ACC-01 | P0 | shell の keyboard、focus、name／role／status、contrast、zoom を検証 | accessibility evidence | NFR-ACC-01 |
| P8-DOC-01 | P0 | installation／hosting、support matrix、known limits、data management を公開 | release documentation | AC-P8-05 |
| P8-OPS-01 | P0 | issue template、compat report intake、artifact rollback runbook を作る | maintenance runbooks | NFR-SUP-01 |
| P8-QA-01 | P0 | clean release artifact で synthetic fixture と title corpus の full acceptance を再実行 | release candidate evidence | AC-P8-06 |
| P8-REV-01 | P0 | evidence manifest、residual risk、known issue、release claim を approver review | versioned release decision | AC-P8-07 |

### 15.4 Deliverables

- versioned beta artifact と reproducible build／SBOM／attestation
- browser／performance／fuzz／soak／migration evidence
- compatibility matrix、known limits、data management documentation
- support／rollback runbook
- release decision と residual risk register

### 15.5 Exit / acceptance

この phase と `AC-P8-*` は Deferred であり、active completion には適用しない。再開した場合は `AC-P8-01`～`AC-P8-07` を改訂した配布 scope に照らして満たす。

## 16. Future track

Future target は active endpoint の自動的な次 iteration ではなく、それぞれ discovery と feasibility gate を持つ。

### 16.1 Compatibility band expansion

最初に行う拡張は architecture の拡大ではなく、PE32 内の support band を段階的に増やすことである。

1. compatibility report から複数 title に共通する unsupported API／format を選ぶ。
2. synthetic fixture と Windows oracle を作る。
3. semantic complexity と resource cost を review する。
4. API implementation と title regression を追加する。
5. machine-readable support matrix を更新する。

候補例は COM／OLE の追加 class、common controls、MCI／ACM、IME、clipboard である。network は service dependency の別 review を必要とする。

### 16.2 D3D9 support band generalization

Phase 6D はサクラノ詩で観測した bounded subset だけを保証する。別 title や一般 D3D9 application へ広げる場合は次を改めて行う。

| Discovery task | 判定内容 |
|---|---|
| F-D3D9-01 | 追加 corpus と Phase 6D manifest の device、fixed function、shader model、texture／format、render state gap inventory |
| F-D3D9-02 | 追加 state／shader を含む D3D9 -> WebGL2／WebGPU translation、shader／state cache prototype |
| F-D3D9-03 | readback、device lost、swap chain、threading、performance benchmark |
| F-D3D9-04 | graphics API validation、shader input、GPU resource-limit review |
| F-D3D9-GATE | 追加 representative fixture の correctness／performance が threshold 内なら expansion roadmap 承認 |

### 16.3 PE32+ / x86_64 / Win64

| Discovery task | 判定内容 |
|---|---|
| F-X64-01 | x86_64 engine、SSE baseline、Win64 calling convention correctness／performance |
| F-X64-02 | 64 bit guest VA、Wasm memory64／multi-region、pointer marshalling、memory pressure |
| F-X64-03 | PE32+ loader、base relocation、unwind info、SEH、TEB／PEB、structure layout |
| F-X64-04 | API metadata の pointer-width audit と shared backend reuse |
| F-X64-GATE | representative PE32+ EXE／DLL vertical slice と no-go review 後に別 roadmap 作成 |

### 16.4 D3D11 / DXGI / XAudio2 / Unity

順序は原則として `D3D9 evidence -> DXGI/D3D11 feasibility -> XAudio2 -> runtime-specific dependencies` とする。Unity は単なる D3D11 title ではなく、managed runtime、native plugin、threading、media、JIT／code generation、filesystem layout を inventory してから target version band を決める。

Future track でも driver、kernel、Windows OS 全体の仮想化は非対象のままである。

## 17. Critical dependency

| Upstream | Downstream | 依存理由 | 先行してよい作業 |
|---|---|---|---|
| Corpus / oracle | 全 subsystem | 必要 feature と correctness 判定が決まる | 公開仕様調査、test harness skeleton |
| Guest memory | CPU、loader、marshaller、surface lock | 全 guest pointer と mapping の基準 | schema／fixture 作成 |
| CPU adapter | ABI、loader lifecycle、API callback | guest code を正しく進める必要 | PE parser の pure parse layer |
| ABI / callback | synthetic DLL、User32、COM | API と callback の stack／reentrancy 基準 | API inventory |
| Scheduler / clock | wait、message、storage async、audio | sync guest と async host の調停 | pure VFS／mixer model |
| PE loader | title execution | EXE／DLL entry と imports が必要 | Windows API contract fixture |
| Handle / object | file、thread、window、GDI、DirectX | lifetime と wait の共通契約 | backend spike |
| VFS persistence | title integration | save と source immutability が必要 | GDI／audio fixture |
| Window / presentation | GDI、GDI+、D3D9、DirectInput | logical HWND、canvas、focus、input state の基盤 | API／vtable inventory |
| GDI / CPU surface | DirectDraw comparison | shared pixel format／backend を使う | COM ABI implementation |
| D3D9 trace／ADR-007 | D3D9 frontend | method、format、state、resource boundary を bounded にする | COM schema／synthetic fixture |
| Normalized input／logical HWND | DirectInput 8 | device state、focus、Acquire の基準 | COM schema／input replay |
| Audio clock / scheduler | DirectSound | cursor／notification／wake の基準 | offline mixer |
| Bounded D3D9 + DirectInput + DirectSound | サクラノ詩 integration | primary target の画面、入力、音声 | shell／preflight／diagnostics |
| Diagnostics | compatibility closure | first mismatch を特定する必要 | UI polish |

依存前の mock は interface 検証に限定し、mock の成功を integration acceptance に数えない。

## 18. Solo execution plan

### 18.1 WIP rule

- primary implementation task は常に 1 件だけ In progress にする
- 調査中に見つけた別 subsystem の gap は fixture／trace ID とともに backlog 化し、その場で実装へ分岐しない
- 各 work session の終了時に canonical test、再現 command、次の最小 action を残す
- subsystem を切り替える checkpoint は、accepted ADR、green fixture、または記録済み blocker のいずれかとする
- documentation／test は primary task の一部であり、独立した後工程にしない

### 18.2 Recommended sequence

1. Phase 0 で primary scenario／dynamic trace と toolchain／engine／memory／scheduler decision を閉じる。
2. Phase 1 の execution vertical slice を go/no-go gate まで完了する。
3. Phase 2 の PE／ABI／minimal Kernel32 を entry／exit fixture で閉じる。
4. Phase 3 の handle／thread／message／VFS／registry をサクラノ詩 checkpoint へ接続する。
5. Phase 4 の window／GDI／GDI+ と normalized input を閉じる。
6. Phase 6 の DirectSound／到達 WinMM path を閉じる。
7. Phase 6D の bounded D3D9／DirectInput path を閉じる。
8. Phase 7A で subsystem gap を一つずつ synthetic fixture へ還元し、primary scenario を完走する。

Phase 5、7B、8 はこの順序へ自動挿入しない。DirectDraw comparison が active blocker の理解に直接必要な場合だけ、範囲と終了 checkpoint を決めて Phase 5 の一部を実行する。

## 19. Requirement / risk traceability

| 領域 | 主 task | Acceptance | 主 risk |
|---|---|---|---|
| Governance | P0-GOV-* | AC-P0 | R-045～046 |
| Corpus／oracle | P0-COR-*、P7*-COR-* | AC-P0-05、AC-P7-* | R-001～003、R-020 |
| Guest memory／CPU | P1-MEM-*、P1-CPU-* | AC-P1 | R-010～016 |
| PE／DLL／bootstrap／ABI | P1-ABI-*、P2-PE-*、P2-BOOT-*、P2-ABI-* | AC-P2 | R-020～021、R-031A |
| Win32／COM core／scheduler | P1-SCH-*、P2-WIN-*、P3-WIN-*、P3-COM-* | AC-P1～03 | R-014～016、R-020～021 |
| VFS／registry／module snapshot | P3-STO-* | AC-P3 | R-030～031A |
| GDI／GDI+／font／User32 input | P4-* | AC-P4 | R-022～024 |
| DirectDraw comparison | P5-* | AC-P5、non-blocking | R-022、R-024 |
| DirectSound | P6-* | AC-P6 | R-025～026 |
| Bounded D3D9／DirectInput | P6D-* | AC-P6D | R-024、R-027 |
| サクラノ詩 integration | P7A-* | AC-P7-DP | R-001～004 |
| Multi-title／public release | P7B-*、P8-* | Deferred | 再開時に再評価 |

## 20. Milestone review template

各 phase の終了 review は次の順で行う。

1. Entry criteria と scope change を確認する。
2. task の完了ではなく該当 `AC-*` evidence manifest を確認する。
3. test skip、accepted deviation、browser／title result を review する。
4. performance、memory、fuzz、soak を前 baseline と比較する。
5. `Q-*`、ADR、assumption、risk P／I、residual risk を更新する。
6. dependency／fixture／artifact integrity を確認する。
7. next phase の owner、capacity、critical path、estimate range を更新する。
8. `Go`、`Conditional go`、`Repeat`、`Scope reduction`、`No-go` の decision を記録する。

`Conditional go` は blocker を後回しにする一般許可ではない。condition、owner、期限、downstream で禁止される作業を明示する。

## 21. Backlog 運用規則

- task は最低一つの requirement、risk mitigation、acceptance のいずれかへ link する。
- title failure から直接 code patch を作らず、再現 fixture／trace と分類 task を先に置く。
- API backlog は import count だけでなく runtime call frequency、critical path、semantic complexity、resource cost で順序付ける。
- `partial` API は対応 variant と未対応 behavior を child task に分ける。
- test／diagnostic／resource limit／documentation を implementation task の acceptance に含める。
- P2 item が P0 gate を block している場合、優先度を上げるか phase scope から明示的に外す。
- Future target と Deferred phase の task を personal target の critical path に混ぜない。
- estimate 超過時は test gate を削らず、title／API／browser scope を見直す。

## 22. 現在地からの Windows task queue

static inventory、primary title選択、CPU adapter、Windows W0、trace v1／v2、W1／W2／W3、high-address memory、W7 Rust PE32 load／memory／entry return／stack bootstrap／bootstrap ingress／explicit unsupported trap binding／BGI first runtime trapに加え、MemoryDelta、changed-state transaction journal、atomic import binding、public E2E／CI、low-alignment rejection、planner fuzz／randomized memory test、cross-page execute gateは完了した。さらにBGIの最初の17 APIをchecked returnし、entryから累計238命令でsame ID `89`／`KERNEL32.dll!EncodePointer(0x7F020000)`へ到達した。review Step 2／3に続き、Step 4の13 API contract／17 call occurrenceをgeneric `ApiDispatcher`／authoritative `RuntimeState`へ移行し、17-call expectation manifestをNode／Rustで共有するgeneric session runnerへ統合して旧report／reflection digestを維持した。Step 5はtransaction開始時のfull cloneを除去し、4 MiB baseline／0-byte start、1／2 changed pageで4／8 KiBのtracked rollback payloadを固定した。Step 6はRust `GuestMemory`をbyte authority、v86 pagingをexecution mirrorとしてADR-003をAcceptedにし、binary reflection／delta／writeback、PTE Dirty multi-page discovery、stack／heap／surface／RWXの連続2-slice atomic reconciliationをgeneration `18 -> 21`で固定した。Step 7はcontrolled PE32をChrome Runtime Worker内で46命令実行し、generic dispatch、2 deferred completions、depth-2 callback、Canvas／Web Audio／IndexedDB、cancel／watchdog、clean exitを同じbrowser pathで通した。30分／11,326-session stressは失敗0、heap slope 136 bytes/min、最大heartbeat gap 16.28 msだった。Step 8はcommit `a0d99e5`のclean Chrome runでinteger／x87／SSE2、adapter trap、production generic API、callback、context、slice、reflection／writeback、invalidation、cold start／active memory、background、audio、Wasm crossingの全thresholdを通し、v86継続を`go`とした。Step 9 foundation 1はgeneration付きtyped object／handle table、separate object lifetime、close／stale detection、process／thread wait signal、module／heap close policyを`RuntimeState`へ入れた。foundation 2はprocess-owned module graph、canonical identity、reference ownership、dependency-first attach／reverse detach metadataを追加した。foundation 3はcontrolled process／initial-thread bootstrap、running、exit request／completion、atomic wait signal orderingを追加した。foundation 4はcontrolled second thread、thread-local LastError／TLS／callback／scheduler lane、explicit selection、TID昇順all-lane terminationを追加した。foundation 5はone-object wait registration、object-indexed FIFO、thread-exit signal／wake event、already-signaled completion、exit cancellationを追加した。foundation 6はbounded raw per-thread FIFO、empty block、first-message wake event、ordered dequeue、exit cleanupを追加した。foundation 7はtyped logical private heapを32 x 16 MiB controlled guest arena、8-byte first-fit、monotonic allocation ID、deterministic initialization、free／reuse、heap／process teardownへ接続した。foundation 8はsealed mapped image identityとbounded PE32 name／ordinal／forwarder resolutionをprocess module graphへ接続した。foundation 9はcontrolled EXE／guest DLLのdirect mapped exportとprocess-owned runtime catalog endpointをsame IAT planへ入れ、static dependency、attach failure unwind／retry、dependency-first attach／reverse detach callbackをRust authorityへ接続した。Step 10はfrozen 17-call workloadをprocess-owned 15-module graph、330 runtime endpoints、persistent Rust session、automatic binary v86 writebackへ戻し、four additional `EncodePointer` callsを含む21 callsまで進めた。22 writeback batches／38 changed pages／268 bytes、3 host deltas、generation `30 -> 52`を通し、instruction 266の`InitializeCriticalSectionAndSpinCount(0x100FF630, 4000)`をfirst unsupported capabilityとして構造化停止した。frozen reportを不変に保ち、separate 96,244-byte generic report／SHA-256 `a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa`を固定した。Step 11はnative 24-byte layout／LastError／recursive enter／leaveとBGI wrappersを測定し、process-owned critical-section stateと3 contractsをsame generic pathへ追加した。14 initializationsを含む35 calls、36 writeback batches／65 dirty pages／344 changed bytes、17 host deltas、final generation `80`を通し、instruction 575の`DecodePointer(0xEC010003)`へ進んだ。167,719-byte report／SHA-256 `cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69`を固定した。Step 12はnative inverse／LastError／immediate consumerを既存2 traceで固定し、registered Vine tokenだけをprocess-owned inverse mappingへ解決する`DecodePointer` contractを追加した。36 calls、37 writeback batches／66 dirty pages／348 changed bytes、17 host deltas、final generation `81`を通し、instruction 577のdynamic `FlsAlloc(0x100A5666)`へ進んだ。171,356-byte report／SHA-256 `5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e`を固定した。Step 13は2件のnative FLS traceでcallback RVA `0xA5666`、native index `9`、LastError preserve、immediate index storeを固定し、bounded process-owned logical FLS slot `0`とdynamic endpoint dispatchをsame runtimeへ追加した。37 calls、38 writeback batches／68 dirty pages／399 changed bytes、17 host deltas、final generation `82`を通し、instruction 618の`HeapAlloc(0x68010001,8,0x214)`へ進んだ。175,123-byte report／SHA-256 `26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c`を固定した。`LoadLibrary`／`FreeLibrary`、TLS／FLS value／free／thread callback、independent unload／retirement、per-thread CPU bootstrap、wait-any／wait-all／timeout、critical-section contention blocking／delete、User32 message semantics、guest HeapAlloc dispatch、snapshot、BGI browser execution、title-workload performanceは未実装／未判定であり、`AC-P1`／`AC-P2`等を完了しない。

上記の未判定一覧はStep 13時点の記録である。BGI Browser executionとtitle-workload performanceは
後続checkpointで実装／測定済みであり、現在の未完了範囲はfull-play、long-idle、title outputである。

Step 14はexisting heap traceで`HEAP_ZERO_MEMORY`／`0x214`／LastError preserveを固定し、
reached `HeapAlloc`をexisting transactionへ接続した。Guest pointer `0x20000000`、lazy
reserve／commit、generation `82 -> 85` host deltaを通し、19 contracts／39 calls、40
writeback batches／70 dirty pages／415 changed bytes、18 host deltas、final generation `87`
でinstruction 644のdynamic `FlsSetValue(0,0x20000000)`へ進んだ。183,102-byte report／
SHA-256 `d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4`
を固定した。

Step 15はexisting FLS tracesのBGI caller return RVA `0xA5955`でBOOL `1`と
LastError `87 -> 87`／`0 -> 0`を固定し、dynamic `FlsSetValue`をselected thread
`8192`のlogical slot `0`へ接続した。Existing second `GetModuleHandleW`とfirst
`EnterCriticalSection`を含む20 contracts／42 calls、43 writeback batches／75 dirty
pages／497 changed bytes、19 host deltas、final generation `91`を通し、instruction 707の
ID `124`／`InterlockedIncrement(0x100F08E0)`へ進んだ。195,821-byte report／SHA-256
`f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f`を固定した。

Step 16は2件のfocused native traceでcaller RVA `0xA5577`／`0xA7351`／`0xA73C7`の
`0 -> 1`／`1 -> 2`／`0 -> 1`、LastError `87 -> 87`とlater `-1 -> 0` wrappingを固定した。
Required aligned `InOut LONG`をcommon dispatcherのchecked read／one guest writeへ接続し、
3 callsをreturnした。Existing critical-section／thread／startup contractsを含む21 contracts／
50 calls、51 writeback batches／86 dirty pages／667 changed bytes、26 host deltas、final
generation `106`を通し、instruction 934の`HeapAlloc(0x68010001,8,0x800)`へ進んだ。
234,775-byte report／SHA-256
`4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643`を固定した。

Step 17はexisting heap-lifecycle pairのsecond `HeapAlloc(nativeHeap,0x8,0x800)`をversioned
first-two-reached contractへ固定した。Frozen v1はsame sizeをmutation-free rejectし、新profileだけが
first allocationを保持してID `2`／guest `0x20000218`／zeroed `0x800` blockを作る。
Unsealed heap ownerのdirty-only generation `106 -> 107` deltaをmirror owner identityへ接続し、
21 contracts／51 calls、52 writeback batches／89 dirty pages／906 changed bytes、27 host deltas、
final generation `108`を通した。instruction 1395の`GetStdHandle(0xFFFFFFF6)`へ進み、
239,789-byte report／SHA-256
`fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8`を固定した。

Step 18はfocused `standard-handle` trace 2件でBGI caller RVA `0xA87B6`のnonnull
`GetStdHandle(STD_INPUT_HANDLE)` result `0x208`／LastError `87 -> 87`を固定した。Immediate
consumerはINVALID／NULL check後にsame handleを`GetFileType`へ渡す。Native handleを破棄し、
process-owned logical stdin `0x53010001`をone-argument stdcall contractから返す。22 contracts／
52 calls、53 writeback batches／90 dirty pages／911 changed bytes、27 host deltas、final
generation `109`を通した。instruction 1403の`GetFileType(0x53010001)`へ進み、243,749-byte
report／SHA-256 `5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e`を固定した。

Step 19はfocused `file-type` trace 2件でstdinの`FILE_TYPE_CHAR (2)`／LastError
`87 -> 87`とconsumerを固定した。Registered logical stdinだけをclassifyし、existing 15th
critical-section initializationをreturnする。23 contracts／54 calls、55 writeback batches／
94 dirty pages／938 changed bytes、28 host deltas、final generation `112`を通した。
instruction 1439の`GetStdHandle(STD_OUTPUT_HANDLE)`へ進み、252,994-byte report／SHA-256
`dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf`を固定した。

Step 20はexisting `standard-handle` trace 2件でBGI caller RVA `0xA87B6`の
`GetStdHandle(STD_OUTPUT_HANDLE)` result `0x234`／LastError `87 -> 87`を固定した。
versioned input／output contractだけがlogical stdout `0x53010002`を返し、stdin-only profileは
不変に保つ。23 contracts／55 calls、56 writeback batches／95 dirty pages／943 changed
bytes、28 host deltas、final generation `113`を通した。instruction 1447の
`GetFileType(0x53010002)`へ進み、257,008-byte report／SHA-256
`0c6047e5c5007f244edaa5bd7ea963dd410283801d3bb572fa8a3aa82b10795d`を固定した。

後続architecture correctionでWin32 semanticsとcompatibility policyを分離した。55-call
semantic resultとnext stopは不変で、その時点のpolicy-bearing reportは257,475 bytes／SHA-256
`c158580fe7567b385ad28851181b7c865236510479040ec6fdc05fd4c4a2833a`である。旧hashは
capture時artifact identityとしてのみ保持する。後続Browser oracle integration後のcurrent replayは
257,878 bytes／SHA-256
`564c3425f01b35e86717b68335b18f8bcd3d30d996717a1d0848028104cf0a2b`である。

current 61-call sessionはChrome Runtime Worker内のRust/Wasm authorityへ移行し、Node oracleとの
ordered completion／memory digest一致、main heartbeat、cancel／watchdog teardownを通した。続く
3分／1,579-session BGI workload gateはfailure 0で、session／slice分布、main／Worker heapとbacking
storage slope、Rust／v86 linear memory、271 Wasm crossings/sessionの全version 1 thresholdを通した。
これはfull-play、long-idle、title outputの完了を意味しない。

1. [完了] `GetFileType(0x53010002)`のnative `FILE_TYPE_PIPE`／LastError／immediate consumerを固定し、registered logical stdoutだけをversioned file-type contractへ接続した。57 calls／generation `116`を通し、instruction 1484の`GetStdHandle(STD_ERROR_HANDLE)`へ進んだ。
2. [完了] `GetStdHandle(STD_ERROR_HANDLE)`のnative identity `0x238`／LastError `87 -> 87`／`GetFileType` consumerを固定し、logical stderr `0x53010003`だけを既存contractへ追加した。58 calls／generation `117`を通し、instruction 1492の`GetFileType(0x53010003)`へ進んだ。
3. [完了] `GetFileType(0x53010003)`のnative `FILE_TYPE_PIPE`／LastError／consumerを固定し、registered logical stderrだけを既存file-type contractへ追加した。60 calls／generation `120`を通し、instruction 1516の`SetHandleCount(32)`へ進んだ。
4. [完了] `SetHandleCount(32)`のnative return `32`／LastError `87 -> 87`／discarded consumerを固定し、policy-enabled synchronous contractだけを追加した。61 calls／generation `121`を通し、instruction 1526の`GetCommandLineA()`へ進んだ。
5. `GetCommandLineA()`のnative pointer relation／returned bytes／LastError／consumerを固定し、existing process-parameter authorityへANSI viewだけを接続する。Host pointer、broad environment、command-line mutationを同時に追加しない。

frozen third `EncodePointer(0x7F020000)`はhistorical trapとして保持し、dedicated eighteenth-call expansionはpauseを続ける。`SetHandleCount(32)`は完了し、現在はreached `GetCommandLineA()`を進める。この queue の各 checkpoint は、native evidence、実装、代表 automated test 1 本、既存evidenceとのsemantic同等性、短い結果記録で閉じる。

## 23. 完了の定義

active roadmap は、次をすべて満たしたとき完了とする。

- AC-P0、P1、P2、P3、P4、P6、P6D、P7-DP の evidence が同じ runtime revision と primary fingerprint に結び付いている。AC-P5、P7-MVP、P8 は不要
- 段階的な versioned checkpoint に加え、サクラノ詩の全編を clean local Chrome profile で不自由なくプレイできる
- 元 `.exe`／`.dll` の保存 hash が不変で、write は title-local overlay に限定される
- unsupported input が hang や silent success ではなく説明可能に停止する
- instruction／memory／queue の上限と stop／cancel が title flow で機能する
- CPU／ABI／PE／API／graphics／audio／storage に synthetic fixture と回帰 test がある
- browser／API／title compatibility matrix が tested／untested／partial を区別する
- save backup／restore と runtime revision rollback が個人環境で再現可能
- dependency／fixture／artifact の integrity が確認されている
- residual risk と既知制約が local compatibility profile に反映されている

サクラノ詩 full-play target の完了は、別 build／title、D3D9 support band 全体、PE32+、x86_64、D3D11、DXGI、XAudio2、Unity、full Windows compatibility の完了を意味しない。それらは個別の成立性 gate と roadmap を必要とする。
