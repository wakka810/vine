# テスト戦略と受け入れ条件

## 1. 文書情報

- ステータス: Proposed
- 基準日: 2026-07-10
- 現状: static corpus inventory、documentation check、PE32 EXE／DLL fixture、Windows native callback oracle、native window／message／pixel oracle、LLVM／Wasm smoke、v86 bounded execution／CPU state contract、fixed T1 PE／ABI runtime contract、fixed W2 loader ingress contract、Windows trace summary contractは存在する。production runtime test framework、CI、benchmark、対応browserは未選定

この文書は、各実装を「動いたように見える」状態から、再現可能な互換性証拠へ進めるための基準を定義する。現在の canonical command は `npm run check` であり、source adapter の再現確認は必要な checkpoint だけ `npm run prepare:engine` を使う。

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

現在のW2 loader ingress representative contractは、SHA-256固定の`window-pixel.exe`をpreferred baseへchecked mapし、3 DLL／13 named importの全IATを一意なowned thunkへ解決する。budget `3`でentryを実行し、最初の`kernel32.dll!GetModuleHandleA`をguest call site RVA `0x1004`付きの`unsupported-host-service`として停止した場合だけ成功する。source hash、IAT terminator、thunk ownership、executable call encoding、engine stop、cycle上限も同時に検証する。

これはpreferred-base固定のNode.js harnessであり、relocation、ordinal／export／forwarder、final protection、module graph、source provider差替え、guest stack trace、host API実装を検証しない。`P1-ABI-01`／`P1-DIAG-01`／`P2-PE-01`／`P2-PE-02`／`P2-PE-04`／`P2-PE-10`／`P2-DIAG-01`への限定したpartial evidenceであって、各task、`AC-P2-06`、`AC-P2`全体の完了ではない。

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
