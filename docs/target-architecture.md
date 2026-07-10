# 目標アーキテクチャ

## 1. 文書情報

- ステータス: Proposed
- 目的: 実装開始前に、交換可能な技術と壊してはいけない境界を分離する
- 注意: 記載するコンポーネントは設計案であり、リポジトリに実装は存在しない

## 2. アーキテクチャ原則

1. **保存元と実行状態を分ける。** 元の `.exe` / `.dll` の保存バイト列と hash は変えない。relocation、IAT 解決、section zero-fill 等の必須 loader 処理は guest memory 上だけで行い、title 固有の code patch は行わない。
2. **guest を未信頼とする。** PE parser、CPU、API marshalling、VFS のすべてで、guest address、length、handle、path を検査する。
3. **main thread を塞がない。** guest CPU と scheduler は Worker 側に置き、DOM event と rendering presentation だけを必要に応じて main thread へ渡す。
4. **同期 API を async host に漏らさない。** guest thread の suspend / resume を runtime scheduler が管理し、Web API の Promise を guest stack へ直接露出しない。
5. **compatibility をデータとして扱う。** CPU feature、PE feature、API status、title result を versioned metadata にする。
6. **正確性を先に測る。** interpreter を成立性の基準候補とし、DBT、generated Wasm、parallel execution は benchmark と differential test 後に選ぶ。
7. **縦断経路を早く通す。** 各 subsystem を広く作る前に、合成 PE から pixel、audio、save まで最小の一経路を完成させる。
8. **Rust と TypeScript の ownership を分ける。** Rust は CPU adapter、guest memory、PE loader、ABI gateway、TypeScript は Windows API facade、scheduler integration、browser backend、UI、diagnostics を担当する。

## 3. 論理構成

```text
User-selected title directory / package
                  |
                  v
+-------------------------------- Browser shell (main thread) -----------------------------+
| import UI | title selection | canvas presentation | input | lifecycle | diagnostics UI |
+-------------------------------+---------------------------+-------------------------------+
                                | typed messages / event queues
                                v
+------------------------------- Runtime Worker -------------------------------------------+
|  Runtime orchestrator / scheduler / watchdog / trace                                      |
|                                                                                           |
|  +-----------------+   +-----------------+   +--------------------+                       |
|  | PE/DLL loader   |-->| guest address   |<->| x86 execution      |                       |
|  | import resolver |   | space / memory  |   | engine adapter     |                       |
|  +--------+--------+   +--------+--------+   +----------+---------+                       |
|           |                     |                       |                                  |
|           +---------------------+-----------+-----------+                                  |
|                                             v                                              |
|                                  ABI gateway / API dispatcher                              |
|                                             |                                              |
|           +------------------+--------------+---------------------+                        |
|           v                  v                                    v                        |
|  synthetic Win32 DLLs   GDI / DDraw / D3D9 / DInput / DSound   VFS / registry             |
|           |                  |                                    |                        |
|           +------------------+--------------+---------------------+                        |
|                                             v                                              |
|                                        host services                                      |
+-------------------------+-------------------+-------------------+--------------------------+
                          |                   |                   |
                          v                   v                   v
                  Canvas/WebGL/WebGPU   AudioWorklet/Web Audio   OPFS/IndexedDB
```

この図は責務の境界を示す。実装が単一 Wasm module、複数 package、TypeScript、Rust、C/C++ のどれになるかは ADR で決める。

## 4. 実行フロー

### 4.1 Import から entry point まで

1. Browser shell が user gesture により選択された file handle または package stream を取得する。
2. Import service が file 数、容量、path、archive 展開率、PE signature を上限付きで検査する。
3. 元 file を read-only source volume に登録し、`.exe`／`.dll` は content-addressed managed blob 等の stable byte snapshot に固定して title fingerprint を作る。large asset の lazy source handle とは lifetime を分ける。
4. PE loader が main module を parse し、machine、subsystem、imports、CPU feature 候補を inventory にする。
5. Policy engine が unsupported architecture、driver、executable form、resource limit を分類する。静的 preflight 通過を互換性保証にしない。
6. Guest address space が image、stack、heap、PEB／TEB 相当領域を割り当て、flat segment、FS base、process parameters、initial register／stack を adopted bootstrap profile に従って初期化する。
7. Loader が section、relocation、imports、TLS、同梱 DLL を guest memory 内に配置する。
8. Synthetic DLL resolver が実装 export を API thunk address に結び付ける。
9. DLL attach、TLS callback を仕様順に呼び、最後に executable entry point へ制御を渡す。
10. Scheduler が命令 slice ごとに CPU を進め、event、timer、host response、watchdog を処理する。

### 4.2 API call

```text
guest CALL / JMP import thunk
  -> execution engine trap
  -> ABI schema lookup
  -> pointer / length / handle validation
  -> compatibility implementation
       -> immediate: result + LastError を返す
       -> deferred: guest thread を WAITING にし host request を発行
  -> host response / event
  -> guest thread を RUNNABLE に戻す
  -> return register / stack を ABI 規則どおり復元
```

API implementation は `Promise` を guest へ見せない。deferred operation の途中でも、別 guest thread、message、cancel、timeout を scheduler が扱える設計にする。

### 4.3 Host から guest callback

Window procedure、timer procedure、enumeration callback 等は host event から guest code へ戻る。DirectSound notification は callback ではなく、runtime の event handle を signal して wait 中の guest thread を wake する。

- callback target が executable page 内にあるかを検証する。
- calling convention と引数 schema を metadata から組み立てる。
- 現在の API call へ再入可能か、queue すべきかを API policy に記録する。
- callback 用 frame を guest stack に積み、sentinel return thunk まで CPU を実行する。
- nested callback の深さと命令 budget を制限する。
- exception、stack imbalance、destroyed handle は structured failure として trace する。

callback と再入は後付けしない。Phase 1 の vertical slice で実証する。

## 5. Browser execution topology

### 5.1 推奨 baseline

| Context | 責務 | 禁止事項 |
|---|---|---|
| Main thread | shell UI、DOM input、user activation、canvas presentation、permission prompt | guest 命令 loop、長い parse、同期 wait |
| Runtime Worker | PE parse、CPU、scheduler、API implementation、VFS cache、trace | DOM 直接操作 |
| AudioWorklet | deadline に沿った PCM consume、sample clock、underrun counter | guest API、file parse、重い allocation |
| Optional storage worker | backend が要求する同期 access handle、transaction／snapshot | guest pointer の直接参照 |

OffscreenCanvas、SharedArrayBuffer、Atomics、OPFS sync access 等の採用は browser matrix と cross-origin isolation のコストを測って決める。これらがない環境では、copy queue または support Tier の引き下げを選べるようにする。

### 5.2 Guest thread model

最初の候補は、複数の Windows thread を一つの Runtime Worker 上で cooperative に schedule する方式である。

- 各 thread は register set、FPU state、stack、TEB／FS、LastError、Win32 TLS slot、wait state を持つ。
- 命令 count、virtual time、API boundary、wait で yield する。
- Windows wait object と message queue を runtime 内で解決する。
- browser main thread や Promise の完了を、runtime event として enqueue する。
- priority、APC、alertable wait、true parallelism は corpus の必要性を測って拡張する。

この方式は data race の再現性を高める一方、multi-core 性能や一部 timing-sensitive code を制約する。SharedArrayBuffer を使う 1 guest thread / 1 Worker 方式は、Phase 1 の benchmark と semantic test を通った場合だけ採用候補にする。

### 5.3 Clock ownership

clock を browser の複数時刻源へ分散させない。

- monotonic guest clock: performance counter、timer、sleep、scheduler の基準
- audio sample clock: 音声 consume の実測。drift correction の観測源
- wall clock: date/time API 用。test では固定可能
- frame clock: presentation cadence。guest clock を勝手に進めない

background tab、debugger pause、audio suspend 後に catch-up するか freeze するかは title profile ではなく runtime policy として決め、trace に記録する。

## 6. Guest address space と memory

### 6.1 要求される性質

- 32 bit virtual address と unsigned wrap semantics
- page 単位の reserve / commit / protection
- low/high address、image base、stack guard、shared mapping
- unaligned access と little-endian load/store
- code page invalidation と self-modifying code の検知
- host metadata と guest bytes の分離
- guest pointer + length の overflow-safe な範囲検証
- snapshot、state hash、debug dump の取得

Wasm32 linear memory の offset と guest virtual address を同一と仮定しない。推奨案は sparse page table または region table を介し、CPU と marshaller が共通の checked memory interface を使う方式である。hot path の TLB/cache は profile 後に追加する。

### 6.2 Page state

```text
UNMAPPED -> RESERVED -> COMMITTED
                           |
                           +-> READ / WRITE / EXECUTE の論理 protection
                           +-> DIRTY / CODE / SHARED / GUARD metadata
```

host の実行権限とは無関係に、guest `EXECUTE` を論理検査する。write-to-code が発生した場合は decode/cache を無効化し、policy によって継続または診断停止する。

### 6.3 Heap と mapped file

初期 heap は Win32 contract を満たす runtime-owned allocator とし、guest memory 内に block を配置する。VFS mapped file、image section、anonymous allocation は同じ region abstraction で表現するが、flush、copy-on-write、sharing の semantics は別 metadata とする。

## 7. x86 execution engine

### 7.1 Adapter contract

CPU engine は次の interface の背後に置く。

- reset / load thread state / save thread state
- execute up to instruction or time budget
- checked memory read / write / fetch
- trap on API thunk、exception、breakpoint、yield、halt
- invalidate decoded code range
- deterministic trace hook
- instruction feature report

既存 engine を採用する場合も、Wasm portability、correctness corpus、callback trap、memory adapter、bundle size を評価する。decoder だけを再利用する場合と full emulator を再利用する場合を区別する。

### 7.1.1 Current feasibility candidate

2026-07-10 時点では v86 を adapter feasibility の先行候補とする。prebuilt fallback Wasm で full-PC boot を迂回した 32 bit integer／stack／x87／SSE program が動き、debug Wasm では CPL3 `#UD` を host hook へ返せた。詳細と再現 command は [x86-engine-scorecard.md](x86-engine-scorecard.md) に記録する。

固定 source overlay では、release fallback に追加 import を増やさず exact instruction budget、structured CPL3 fault stop、versioned 640-byte CPU state ABI を追加できた。fixed T1／W2 flowに続き、high-address PEを2 MiB physical poolから`0xF1000000`へmapし、present／R／W、cross-page fault ordering、protect／remap invalidationを測定した。v86はinstruction fetchをreadと区別せずnon-executable sectionを実行するため、Vine region table＋bounded allocator＋execute gateをv86 pagingへ接続する案を`ADR-003` provisional candidateとする。これは engine／memoryの採用決定ではない。広いcalling convention、SEH、snapshot、generated-code invalidation、browser Workerは未検証である。詳細は[adr-003-guest-memory.md](adr-003-guest-memory.md)を正とする。

### 7.2 段階的な命令範囲

1. i386 integer、flags、branch、stack、string、segment の必要部分
2. x87 と Windows compiler が生成する floating-point sequence
3. CPUID、RDTSC 等の環境／時間依存命令を仮想化
4. privileged、I/O、kernel／direct-system-call 命令を guest fault／unsupported として隔離
5. MMX、SSE、SSE2 を corpus inventory 順に追加
6. atomic、lock prefix、thread interaction
7. self-modifying code と最適化 engine の cache coherence

命令を「実装済み」とする条件は decode できることではなく、flags、fault ordering、NaN／rounding、memory side effect を fixture と oracle で確認できることである。

### 7.3 Optimization gate

Interpreter、threaded interpreter、IR interpreter、DBT、generated Wasm module を Phase 0-1 で比較する。最適化方式は以下を満たしてから既定にする。

- CSP と browser support Tier を満たす
- code cache が guest write、DLL unload、snapshot と整合する
- cold start と compilation pause が UX budget 内
- interpreter との differential trace が一致する
- malformed input で runtime state を破壊しない
- dependency と配布 size が release 方針に合う

## 8. PE / DLL loader

Loader は file parse、image mapping、module lifecycle、symbol resolution を分離する。

### 8.1 Parse layer

- bounded reader を使い、offset + length を overflow-safe に検査する
- DOS、COFF、optional header、data directory、section を immutable model にする
- warning と fatal error を taxonomy で区別する
- unsupported machine／feature を mapping 前に検出する

### 8.2 Mapping layer

- image base を選び section を map する
- zero-fill と alignment を適用する
- relocation を guest memory 内だけに書く
- section protection を最終状態に切り替える
- import／delay import の IAT を guest thunk address へ結ぶ

### 8.3 Module lifecycle

- module identity、reference count、load order、dependency graph を保持する
- export by name / ordinal、forwarder、dynamic lookup を扱う
- corpus が必要とする API-set／system module alias、manifest／activation context を policy 付きで解決する
- TLS template と callback、DLL process/thread attach/detach の順序を検証する
- unload 中の callback、stale function pointer、cycle を診断する

System DLL 名の解決順は policy で固定する。Vine synthetic DLL と title 同梱 DLL を曖昧に混ぜず、置換可否を module ごとに定義する。Microsoft system DLL を title directory から読み込むことを前提にしない。

Main executable と load 対象 DLL は、import 時に hash と stable snapshot を記録した immutable source object から map する。外部 process が元 file handle の内容を import 後や delay-load 前に差し替えても、実行中／再起動後の module bytes が変わってはならない。content-addressed managed copy を推奨 baseline とし、別方式を採る場合は map 直前の再検証と lifetime が同じ保証を満たすことを ADR で証明する。writable overlay が同じ path の `.exe`／`.dll` を shadow して module resolution を変えないようにする。guest が source file の persistent self-update を要求した場合は既定で拒否して診断し、新 version の取り込みは runtime 外の明示的な再 import として扱う。mapped guest image page への書き込みは別問題として page protection／code invalidation policy で扱う。

### 8.4 PE32 process bootstrap

Entry point へ渡す前に、compiler／CRT support band ごとに次を一貫して構築する。

- adopted flat CS／DS／ES／SS と TEB を指す FS selector／base
- stack base／limit、initial ESP、guard、return sentinel、initial register policy
- TEB の exception chain、self、PEB pointer、TLS slots、LastError 等の adopted field
- PEB の image base、loader module list、process heap／flags 等の adopted field
- process parameters の command line、environment、current directory、window／startup data の adopted field
- static PE TLS template／callback と Win32 TLS Alloc／Get／Set／Free の別々の lifecycle

PEB／TEB は undocumented field を含む全面互換とは表現せず、supported compiler／CRT startup fixture と direct-access trace で保証する layout を versioned profile にする。未採用 field の direct access は silent zero ではなく、可能な範囲で trace／fault へ結び付ける。

## 9. ABI gateway と API metadata

API 定義は、可能な限り単一の machine-readable metadata から次を生成する。

- DLL、export name、ordinal、calling convention
- parameter type、direction、optional、count／length relation
- structure size、packing、nested pointer
- ANSI / Unicode variant と code page
- return type、LastError policy、handle ownership
- callback schema と reentrancy policy
- implementation status と semantic test link
- resource cost と execution mode

Marshaller は scalar、pointer、string、array、structure、callback を checked guest memory interface 経由で読む。raw guest pointer は call boundary を越えて保持しない。API 実装後に output buffer を書く際は thread cancellation と lifetime を再検証する。

Handle は tagged table entry と generation を持ち、guest-visible integer から host object を直接推測できない形にする。close 後の handle、型違い、double close を一貫した error にする。

## 10. Synthetic Windows DLL layer

### 10.1 レイヤー

| Layer | 主な責務 |
|---|---|
| Native boundary | PEB／TEB／FS、LastError、exception、virtual memory、loader 内部契約 |
| Base services | process／thread、heap、file、time、sync、encoding、module API |
| USER | logical window、message queue、focus、input、timer |
| GDI | DC、object、bitmap、text、blit、clip |
| COM subset | IUnknown、class registry、apartment の必要範囲 |
| DirectX facade | DirectDraw、DirectSound。将来 D3D、DXGI、XAudio2 |

`stub` は成功を返すだけの意味にしない。status は次で区別する。

- `implemented`: contract test と必要 scenario を通る
- `partial`: 対応 variant と差分を明示する
- `diagnostic_stub`: 副作用なしで既知の error を返し trace する
- `unsupported`: call site を含む structured failure で停止する

## 11. VFS と registry

### 11.1 Volume model

```text
C:\Game\...      read-only source volume + title-local writable overlay
C:\Users\...     title-local writable profile
temp              quota 付き ephemeral volume
explicit mount    user consent を得た場合のみ
```

Path normalization は Windows semantics と mount resolution を同じ関数で混同しない。まず drive、separator、relative path、case-insensitive name、reserved name を guest path model で解決し、その後 mount 内の object ID へ変換する。

### 11.2 Persistence

- source と save を別 namespace／quota にする
- immutable module snapshot と large asset の lazy source handle を別 object／lifetime にする
- write は journal または copy-then-commit で crash consistency を持たせる
- guest API へ write 成功を返す時点、flush／close、browser transaction commit の durability を定義する
- watchdog／quota／runtime defect による hard stop では未commit transaction を rollback し、停止を理由に自動commit しない
- schema version と runtime version を保存する
- migration 前 snapshot、失敗時 rollback、明示 export を提供する
- browser eviction、quota、private mode、permission revocation を起動前に診断する
- title fingerprint ごとに storage を隔離する

Backend は OPFS、IndexedDB、memory adapter の共通 contract の背後に置く。同期 access が可能でも scheduler を無制限に block しない。

### 11.3 Registry

Registry は typed key-value tree とし、VFS file へ無理に模倣しない。HKCU と title-local classes を中心に、default view、case-insensitive key、value type、enumeration order、delete semantics を検証する。host browser setting や OS registry へ接続しない。

## 12. Window、message、input

Guest の HWND は logical object であり、DOM element を直接公開しない。最初は一つの top-level canvas に複数 logical window を composition する方式を候補とする。

- Runtime Worker が window tree、z-order、focus、capture、invalid region を所有する
- Main thread は DOM event を timestamp 付きで normalized input event にする
- User32 layer が virtual key、scan code、mouse coordinate、message sequence へ変換する
- synchronous send と asynchronous post を scheduler 上で区別する
- paint は invalidation、BeginPaint / EndPaint、presentation の順序を trace する
- browser shortcut と OS-reserved key は support matrix に差分を記録する
- DirectInput 8 frontend は同じ normalized keyboard／mouse state を読み、独立した DOM listener や別の入力時刻系を持たない
- Acquire／Unacquire、cooperative level、buffered event は logical HWND／focus state と明示的に接続する

IME、clipboard、file drop は personal target の keyboard path と分離して deferred とする。

## 13. Graphics

### 13.1 共通 surface model

GDI bitmap と DirectDraw surface が共有できる CPU-visible pixel storage abstraction を候補とする。D3D9 resource は同じ object model へ押し込まず、guest-visible lock staging、format conversion、presentation boundary だけを共有候補とする。

- width、height、pitch、orientation
- indexed 8 bit、RGB555／565、24 bit、32 bit 等の format
- palette、color key、alpha policy
- lock count、dirty region、owner、lost state
- CPU read/write と GPU upload/download の同期

GDI semantics と DirectDraw semantics を同一 object に押し込まず、storage と presentation backend だけを共有する。D3D9 の render target、texture、vertex buffer、device state は別 frontend が所有する。

### 13.2 GDI frontend

DC state、selected object、mapping、clip、ROP、font metric を runtime model に保持する。Canvas text の結果を Windows GDI と同一と仮定せず、glyph rasterization と font substitution の選択を ADR にする。

### 13.3 DirectDraw frontend

COM vtable と HRESULT の ABI を提供し、surface create、lock、blit、flip、palette、cooperative level を共通 surface command に落とす。fullscreen は browser Fullscreen API と canvas scaling で表現し、display mode の変更を host OS へ要求しない。

### 13.4 Bounded D3D9 frontend

サクラノ詩の runtime trace で観測した interface／method／format／state だけを versioned support band とする。

- `Direct3DCreate9` から IDirect3D9／IDirect3DDevice9 の COM identity と lifetime を構築する
- CreateDevice／Reset／cooperative state と logical HWND／canvas presentation を接続する
- guest-visible Lock／Unlock memory と backend texture／buffer allocation を分離する
- fixed-function state、vertex format、blend、sampler、draw を WebGL2 または WebGPU command へ変換する
- device loss／context loss 後は reconstruct 可能な resource と guest に再作成を要求する resource を区別する
- 未観測 method を成功扱いせず、call site、vtable slot、arguments を診断して停止する

shader model や一般 D3D9 application compatibility はこの frontend の初期要件ではない。runtime trace で shader path が必須と判明した場合は、Phase 6D の scope／feasibility を再審査する。

### 13.5 GDI+ image subset

GDI+ は一般的な drawing stack として再実装せず、サクラノ詩で到達する bitmap load／save、DIB conversion、LockBits／UnlockBits を GDI surface、VFS、browser image codec のどこで処理するか ADR-007 で決める。OS codec の暗黙差に依存せず、format と pixel result を fixture に記録する。

### 13.6 Backend selection

| 候補 | 評価項目 |
|---|---|
| Canvas 2D | GDI primitive／text の補助実装、pixel access cost、再現性。D3D9 backend にはしない |
| WebGL2 | D3D9 fixed-function translation、shader による blit／palette、readback cost、context loss |
| WebGPU | D3D9 state translation、compute／copy 性能、browser availability、device loss、bundle complexity |

最初の renderer は知名度で選ばず、8 bit palette、RGB565、color key、partial dirty update、text、readback に加え、primary trace の D3D9 state／draw workload を同じ候補 backend で測る。

## 14. Audio

DirectSound frontend は buffer object と Windows-visible cursor semantics を所有し、backend は PCM block と control event を受け取る。

```text
guest Lock/Unlock -> DirectSound buffer state -> mixer/resampler -> ring buffer
                                                               -> AudioWorklet
                                                               -> Web Audio output
```

- AudioWorklet を real-time consumer とし、重い変換や guest access を置かない
- play cursor と write cursor は browser callback の瞬間値ではなく定義した clock model から計算する
- wrap-around lock、loop、volume、pan、frequency、event-handle notification／thread wake を semantic test する
- logical HWND、SetCooperativeLevel、focus／deactivation、primary format permission、buffer lost／Restore を DirectSound state として扱う
- browser user activation、AudioContext suspend、background、output device change は別の state machine とし、DirectSound cooperative level の代用にしない
- underrun、overrun、queue depth、drift を常時計測可能にする

MCI、ACM、DirectShow、video codec は DirectSound と同じものとして暗黙対応しない。サクラノ詩が静的 import する `mciSendCommandA` は primary scenario の到達性を測り、到達する command subset だけを Phase 6 の明示 task とする。

## 15. Browser shell

Shell はランタイムの機能説明ページではなく、実行に必要な操作面を提供する。

- library／recent title と local import
- preflight result と必要 browser feature
- launch、pause、resume、stop、reset
- canvas、fullscreen、audio activation
- save storage の使用量、export、import、delete
- compatibility status と diagnostic export
- runtime update と migration status

Shell は file handle や persistent permission が browser ごとに異なることを明示的な状態として扱う。

## 16. Diagnostics と compatibility data

全 subsystem は共通 event envelope を使う。

| Field | 例 |
|---|---|
| runtime | version、build ID、schema version |
| guest | virtual timestamp、thread、EIP、module、symbol |
| event | category、operation、result、duration |
| API | DLL、name／ordinal、status、LastError／HRESULT |
| resource | handle type、guest address range、quota delta |

Trace level は off、error、API summary、debug、instruction のように段階化し、instruction trace を通常運用で有効にしない。

## 17. Runtime robustness

runtime は互換性 failure が browser tab 全体の停止や再現不能な state corruption へ広がらないよう、次を共通基盤として持つ。

- PE、DLL、resource、archive の bounded parser と checked arithmetic
- guest pointer、length、handle、callback lifetime の一貫した検査
- instruction、wall time、memory、file、thread、handle、surface、audio の上限
- Worker watchdog と明示的に停止できる lifecycle
- malformed／boundary corpus、fuzz、negative test
- dependency pin と build artifact hash

## 18. Error taxonomy

| Class | 例 | 既定動作 |
|---|---|---|
| User action required | audio activation、file permission | guest を pause し shell で要求 |
| Unsupported | API、instruction、pixel format | call site 付きで安全停止 |
| Guest fault | access violation、invalid opcode | guest exception。処理不能なら title のみ停止 |
| Invalid input | malformed PE、archive bomb | mapping 前に拒否 |
| Resource limit | instruction、memory、storage quota | title を pause／stop し、commit 済み state を保持して未commit transaction を rollback |
| Host transient | context loss、audio suspend | bounded retry または再初期化 |
| Runtime defect | invariant violation、marshaller mismatch | fail closed、diagnostic を生成し、該当 build を不合格 |

失敗時に shell 全体を reload するだけの実装を避け、title runtime、storage、UI の failure domain を分離する。

## 19. 将来拡張

### 19.1 PE32+ / x86_64

再利用を目指すのは parser framework、module graph、API metadata、VFS、handle、diagnostics、backend である。次は新規または大幅な再検証が必要になる。

- 64 bit guest address space と pointer marshalling
- x86_64 execution、SSE baseline、Win64 calling convention
- unwind metadata、exception、TEB／PEB layout
- memory64 または multi-region memory の browser 成立性
- 64 bit structure layout と handle／pointer truncation audit

### 19.2 D3D9 generalization / D3D11 / DXGI / XAudio2 / Unity

サクラノ詩向け bounded D3D9 subset の完了を D3D9 全体の互換性とは扱わない。shader translation、追加 resource state、swap chain、threading、media、managed runtime、JIT 等を含む拡張は、それぞれ feasibility milestone と技術 review を持つ。

## 20. 先行 ADR

実装開始前または Phase 1 中に、少なくとも以下を決定する。

| ADR | 決定対象 | 決定期限 |
|---|---|---|
| ADR-001 | Rust + TypeScript の workspace、ownership boundary、Wasm toolchain | Phase 0 |
| ADR-002 | x86 engine の自作／再利用と baseline execution mode | Phase 0 |
| ADR-003 | sparse guest memory、page size、protection、snapshot | Phase 0 |
| ADR-004 | Runtime Worker、guest scheduling、async suspension | Phase 0 |
| ADR-005 | API metadata、thunk、callback、marshaller の生成方式 | Phase 1 前半 |
| ADR-006 | VFS backend、immutable module snapshot、source／overlay、durability／hard-stop rollback | Phase 0 |
| ADR-007 | renderer、GDI／DirectDraw surface、D3D9 resource／presentation boundary | Phase 3 まで |
| ADR-008 | DirectSound mixer、AudioWorklet、clock ownership | Phase 3 まで |
| ADR-009 | browser support Tier、SAB、COOP / COEP | Phase 1 終了時 |
| ADR-011 | compatibility profile と API inventory schema | Phase 0 |

各選択肢と決定方法は [risks-and-open-questions.md](risks-and-open-questions.md) に記載する。
