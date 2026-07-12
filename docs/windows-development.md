# Windows development

## 1. Current checkpoint

準備段階で次を固定済みである。

- 7 title の PE32／i386 static inventory と fingerprint
- LLVM 22 による PE32 EXE／DLL／callback fixture build
- v86 `0.5.424` prebuilt Wasm の integer／x87／SSE smoke
- v86 source revision `2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f` の reproducible adapter build
- exact instruction budget、CPL3 fault stop、version 1 CPU state round trip
- Windows W0 environment、BGI fingerprint、canonical check
- Frida 17.15.4 による60秒startup traceとvalidated local summary
- native `abi-smoke.exe` の DLL callback oracle
- source-built v86 上の fixed PE32 map、IAT thunk、cdecl host service、1段 guest callback、budget yield、return sentinel
- native `window-pixel.exe` のstdcall WndProc、queued message、paint、pixel、destroy／quit oracle
- source-built v86 上の`window-pixel.exe` preferred-base map、13 owned thunk、`GetModuleHandleA(NULL)`、fixed class semantic state／ATOM、harness-local logical HWND／HDC／2x2 client、`WM_CREATE`／nested `WM_PAINT`／`WM_DESTROY` callback、paint／destroy／quit、entry return sentinel
- fixed W2 resultのversion 1 browser present eventと、Chrome Canvas 2Dのone-shot present／pixel readback
- trace v2のcaller module＋RVA、call ID／parent call ID、leave LastError／durationと、explicit selected `IDirect3D9` root 11-method evidence
- high-memory PE32の`0xF1000000` map、present／R／W／cross-page fault、TLB invalidationとv86 execute-protection gap
- Rust `vine-core`のimmutable snapshot／bounded PE32 section・import・relocation planと、BGIのW7 process-bootstrap stop
- Rust `vine-core`のsparse owner／region／page table、bounded zeroed frame allocator、logical execute seal／invalidation protocolと、BGIの380-page memory-ingress stop
- Rust `vine-core`のsource copy／explicit zero-fill／`HIGHLOW` materializationと、BGIの330-slot original IAT verification／first unbound import stop
- Rust `vine-core`のall-import read-only plan、64 KiB runtime-owned thunk arena／stable ID、transactional IAT commitと、controlled fixtureのowner seal／BGI 329 unresolved identityのmemory不変stop
- Rust semantic reflection／ack sessionによるcontrolled 6 pagesのv86 mapping、actual invalidation、bounded stack／entry-to-thunk ID `1`
- ordered production thunk metadataとversioned CPU stateによるfixed cdecl result `42`、guest cleanup、thunk ID `2`のcallback-service ingress
- separate Rust runtime ownerのcallback return sentinel、frame ID `1`、checked stack、guest callback result `43`のexact return stop
- frame ID `1`のretirement、outer cdecl result `43`、guest result／entry continuation、balanced ESPとharness-only unmapped returnのstructured `#PF`
- 同じcontrol pageのtyped process-entry sentinel ID `2`、exact entry result `0`／ESP `0x00700000` completionとunmapped-fault regression
- Rust owner `4`のone-page stack region／bootstrap metadata、generation `12` reflection、adapter-local stack allocationなしのexact completion
- import-free bootstrap fixtureのnative `FS:[0x18]` TEB self／`FS:[0x30]` PEB oracleと、FS base zeroのsource-v86 CR2 `0x18`／PFEC `4` first stop
- typed Rust TEB／PEB regions、generation `15` reflection、CPU state FS-base restore、4 exact field observationとentry result `0`
- separate import-free fixtureのnative `PEB+0x10` process-parameters／`+0x40` command-line oracleと、empty guest PEBのexplicit result `2`
- typed process-parameters／command-line regions、generation `21` reflection、exact UTF-16 observationとentry result `0`
- BGIの388-page bounded reflection、v86 VGA MMIO physical hole回避、generation `24` acknowledgement、entryから19命令後のexact `KERNEL32.dll!GetSystemTimeAsFileTime` trap
- controlled clock、generation／owner付きstack reconciliation、generation `25` `FILETIME` delta、ID `104` stdcall return、ID `103`／`GetCurrentProcessId` next trap
- controlled process identity／PID `4096`、generation `25` reconciliation、ID `103` zero-argument stdcall return、ID `149`／`GetCurrentThreadId` next trap
- controlled thread identity／TID `8192`、generation `25` reconciliation、ID `149` zero-argument stdcall return、ID `13`／`GetTickCount` next trap
- controlled monotonic clock／tick count `3,600,000`、generation `25` reconciliation、ID `13` zero-argument stdcall return、ID `157`／`QueryPerformanceCounter` next trap
- controlled performance counter `12,345,678,901`、generation `26` delta、ID `157` one-argument stdcall／BOOL return、ID `83`／`GetStartupInfoW` next trap
- focused native `GetStartupInfoW` x86 observation、controlled 68-byte startup profile、generation `27` delta、ID `83` one-argument VOID stdcall return、ID `84`／`HeapSetInformation` next trap
- focused native `HeapSetInformation` arguments／BOOL／LastError observation、controlled termination-on-corruption policy、generation `27` preserving reconciliation、ID `84` four-argument stdcall return、ID `102`／`HeapCreate` next trap
- focused native heap lifecycle observation、typed logical private-heap handle `0x68010001`／metadata、generation `27` preserving reconciliation、ID `102` three-argument stdcall return、ID `72`／`GetModuleHandleW` next trap
- focused native module-handle observation、bounded UTF-16 `KERNEL32.DLL` lookup、typed logical loaded-module handle `0x4D010001`、generation `27` preserving reconciliation、ID `72` one-argument stdcall return、ID `150`／`GetProcAddress` next trap
- focused native dynamic-export observation、separate RX endpoint `0x7F020000`、register-indirect IAT provenance、bounded ASCII `FlsAlloc`、generation `30` preserving reconciliation、ID `150` two-argument stdcall return、second `GetProcAddress`／`FlsGetValue` next trap
- ordered `FlsAlloc`／`FlsGetValue` endpoints、persistent ESI provenance、generation `30` stack＋image-page transactional reconciliation、first endpoint store verification、endpoint `0x7F020010` return、third `GetProcAddress`／`FlsSetValue` next trap
- ordered `FlsAlloc`／`FlsGetValue`／`FlsSetValue` endpoints、first two guest-store verification、endpoint `0x7F020020` return、fourth `GetProcAddress`／`FlsFree` next trap
- ordered four FLS endpoints／guest stores、`FlsFree = 0x7F020030` return、static ID `70`／`TlsAlloc` next trap
- focused TLS lifecycle evidence、typed logical TLS index `0` allocation、all four store preservation、static ID `68`／`TlsSetValue(0,0x7F020010)` next trap
- initial-thread TLS endpoint metadata、three-page reconciliation、BOOL `TlsSetValue` return、static ID `89`／`EncodePointer(NULL)` next trap
- focused pointer-encoding evidence、controlled encoded-NULL token `0xEC010001`、TLS state preservation、same ID `89`／`EncodePointer(0x100A9608)` next call
- executable process-initializer source validation、token `0xEC010002`／guest global store、same ID `89`／`EncodePointer(0x7F020000)` next call
- 初回full reflection後のdirty-page-only `MemoryDelta`、通常RW／NX 8-byte writeのlive v86 readback、generation単位acknowledgement
- Rust byte authority／v86 execution mirror、binary reflection／delta／writeback、PTE Dirty scanによるstack／heap／surface／RWXの連続2-slice multi-page reconciliation
- process environment／PE materialization／import bindingのsnapshot transaction、failpoint rollback、same-memory retry
- `check:fast`／`check:fixture-e2e`／`check:adapter`／`check:private-corpus`、Linux public CI、PE planner fuzz／randomized memory contract
- valid low-alignment PEのstructured unsupported stopと、cross-page NX instructionのconservative execute gate

general process／thread integration、DLL／export lifecycle、Win32 API、graphics／audio は未実装である。controlled imageはminimum TEB／PEB、native-oracled command-line bytes、controlled startup-info／heap-policy／private-heap／loaded-module、four dynamic-export variants、one logical TLS index／thread value、two encoded-pointer tokensまで進んだ。BGIは全330 importsをowned trapsへbindし、runtime-reachable APIを17回returnしたが18件目以降とgeneral TLS／FLS／pointer encoding semanticsは未実装である。詳細は [windows-native-evidence.md](windows-native-evidence.md) と [v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md) を正とする。

## 2. Windows toolchain

PowerShell から使える状態にする最小構成:

| Tool | Purpose |
|---|---|
| Windows 11 Pro 25H2 | primary development／native oracle environment |
| Chrome stable | primary browser。evidence に実 version を記録 |
| Git for Windows | repository と source revision 管理 |
| Node.js 22+／npm | repository checks、runtime tooling |
| LLVM／Clang 22 | PE32 fixture、Wasm object、PE inspection、`llvm-dlltool`によるi386 import library生成 |
| Python 3 | Frida runner |
| Frida tools | BGI.exe の module／API dynamic trace |
| MSYS2 `make` | v86 upstream Makefile の実行 |
| Rust 1.97.0 | root Cargo workspaceとv86 adapter Wasm build。rootは`rust-toolchain.toml`、adapterは`prepare:engine`で固定 |
| WinDbg | crash／exception の native oracle。必要時だけ使用 |

repository は `V:\src\vine`、primary executable は `V:\softs\サクラノ詩\BGI.exe` に置く。Rust は CPU／memory／PE／ABI core、TypeScript は Windows API facade／browser backend／UI／toolingを担当する。

Frida の導入:

```powershell
py -m pip install --upgrade frida-tools
```

MSYS2 では `make` と `sh` を導入し、`C:\msys64\usr\bin` を user `PATH` に置く。Visual Studio 全体は最初の trace には不要である。

## 3. Repository bring-up

PowerShell で repository root から実行する。

```powershell
npm ci
npm run check:fast
npm run check:fixture-e2e
npm run check
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows/check-environment.ps1 -BgiExe "V:\softs\サクラノ詩\BGI.exe"
```

v86 adapter を Windows 上でも再 build する場合は、source adapter 用の toolchain を確認してから PowerShell で実行する。Windows build は Rust 同梱の `rust-lld.exe` を直接使用する。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/windows/check-environment.ps1 -SourceAdapter
npm run check:toolchain
npm run prepare:engine
npm run check:runtime:abi
npm run check:runtime:memory
npm run check:runtime:window-loader
npm run check:runtime:window-browser
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
```

最初の Windows checkpoint は 2026-07-10 に成立した。全 tool の version、canonical check、BGI fingerprint は [windows-native-evidence.md](windows-native-evidence.md) に記録している。

## 4. First native trace

focused Frida runner は module load と起動経路の高信号 API だけを JSONL に記録する。

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

利用者がタイトル画面まで手動操作した後、`Ctrl+C` で detach する。時間で区切る場合は `--seconds 60` 等を指定する。trace 終了時に game も停止する必要がある場合だけ `--kill-after` を加える。runnerとsummarizerは既存outputを拒否する。意図的に置き換える調査だけ`--force`を使う。`--vtable idirect3d9`はroot interfaceを選んだcaptureだけに加える。省略時は既存のfocused export bandからvtableへ広がらない。

最初の trace から次だけを確定する。

- 実際に load された DLL
- タイトル画面までに到達した file、window、D3D9、DirectInput、DirectSound／WinMM、COM entry
- process start から first window／first graphics entry までの順序
- BGI direct callerとOS／driver内部caller、nested wrapper／COM callのownership
- Frida で観測できず WinDbg breakpoint が必要な箇所

広い API hook は追加しない。未観測の API を先回りして実装対象にしない。

## 5. Development order

一つの subsystem を完成させてから次へ進まず、次の縦断 checkpoint を順に通す。

1. `W0 Native evidence`: tool version／fingerprintと、W1選択に十分なstartup resource-band traceを固定する。manual marker付きvisual titleは`P0-COR-02`で継続する。
2. `W1 Headless execution` [Verified]: controlled synthetic PE32をmapし、runtime-owned import thunk、guest callback、entry return sentinelまで走らせる。
3. `W2 Startup substrate` [Native fixture／loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flow verified]: trace 先頭に必要な Kernel32／User32 の最小経路で logical window を作る。
4. `W3 First frame` [browser-facing 1-pixel boundary verified]: fixed logical window／message／pixel resultをversion 1 eventへ変換し、Chromeの最小Canvas sinkで1回present／readbackした。guest execution、production surface、renderer backendは未検証である。
5. `W4 First interaction`: DirectInput または message input を一つ通す。
6. `W5 First audio`: DirectSound または到達した WinMM 経路で一つの buffer を再生する。
7. `W6 Persistence`: title-local file open／read と save write／restart read を一つ通す。
8. `W7 Sakura startup` [PE32 load-plan／guest-memory／materialization／full trap binding／BGI firstからseventeenth Win32 call verified; controlled fixture callback return verified]: immutable BGI.exeからplanを作り、`0x10000000`の380 pagesへsource copy／zero-fill／18,199 `HIGHLOW`を適用する。330 importsをowned thunksへbindし、base 388-page bootstrapにone dynamic-export pageを加えてsource-v86へ渡す。controlled clock／identity／startup-info／heap-policy／private-heap／loaded-module／four dynamic-export lookups／TLS allocation／initial value／two encoded-pointer tokensを17件returnし、third `EncodePointer(0x7F020000)` callまで実行する。18件目のAPI returnは未実装である。

各 checkpoint の automated test は原則 1 本にする。追加 test は、実際の regression、calling convention／fault ordering の曖昧さ、または title failure を最小化した場合だけ作る。

## 6. First Windows session exit criteria

次が揃えば Windows 上で本実装を開始できる。

- `npm run check` が Windows で通る
- BGI.exe の fingerprint が corpus inventory と一致する
- startup JSONL が再取得できる
- startup／title resource bandのruntime-reachable module／API一覧がある
- 次の未対応 vertical slice が W2～W3 の具体的な一項に落ちている

初回 trace は title resource bandまで到達し、後続trace v2はselected `IDirect3D9` root 11 methodsを観測した。W7 controlled imageのentry completion、Rust-owned stack、minimum bootstrap、BGI full trap binding、389-page endpoint mode、first seventeen Kernel32 returns、automatic binary multi-page writebackまで固定した。BGIのexact `EncodePointer(0x7F020000)` continuationは保持して凍結し、次はRust／v86 boundaryをChrome Runtime Workerへpackageする。視覚的なtitle checkpointは`P0-COR-02`の未acceptanceのままである。

以後の title failure は Windows native trace／debugger で最小化し、browser runtime の実装と fixture は同じ Windows workspace で進める。
