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
- Rust `vine-core`の64 KiB runtime-owned thunk arena／stable ID／supported-prefix IAT bindingと、controlled fixtureのowner seal／BGIの次unsupported import stop

v86 memory接続、general DLL／export lifecycle／bootstrap、Win32 API、graphics／audio は未実装である。controlled imageのIATは全bind／seal済みだがCPU未接続で、BGIは`Direct3DCreate9` 1件だけbindした未seal imageである。サクラノ詩のnative traceを引き続き実装順の根拠にする。
trace v1／v2、selected IDirect3D9 root、W1／W2／W3／W7 resultは [windows-native-evidence.md](windows-native-evidence.md) を正とし、W7 planは[pe-loader-ingress.md](pe-loader-ingress.md)、memory boundaryは[guest-memory-ingress.md](guest-memory-ingress.md)、mapped bytesは[pe-materialization.md](pe-materialization.md)、thunk ownershipは[pe-import-binding.md](pe-import-binding.md)を参照する。

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
8. `W7 Sakura startup` [PE32 load-plan／guest-memory／materialization／import-binding ingress verified]: immutable BGI.exeからplanを作り、`0x10000000`の380 pagesへsource copy／zero-fill／18,199 `HIGHLOW`を適用する。native-reachable `d3d9.dll!Direct3DCreate9`をproduction thunkへbindし、次の`DINPUT8.dll!DirectInput8Create`で停止する。

各 checkpoint の automated test は原則 1 本にする。追加 test は、実際の regression、calling convention／fault ordering の曖昧さ、または title failure を最小化した場合だけ作る。

## 6. First Windows session exit criteria

次が揃えば Windows 上で本実装を開始できる。

- `npm run check` が Windows で通る
- BGI.exe の fingerprint が corpus inventory と一致する
- startup JSONL が再取得できる
- startup／title resource bandのruntime-reachable module／API一覧がある
- 次の未対応 vertical slice が W2～W3 の具体的な一項に落ちている

初回 trace は title resource band、D3D9／DirectInput／DirectSound factory、GDI+ まで到達し、W0を閉じるのに必要な実装順を決めた。後続trace v2はcaller attributionとselected `IDirect3D9` root 11 methodsを観測したがdevice／resource methodsは未観測である。W1／W2／W3とhigh-address memory spikeに続き、W7はBGIのowned plan、380-page materialization、runtime-owned `Direct3DCreate9` thunkを通して次のunsupported importまで進んだ。次はsealed controlled image／thunk arena／bounded stackをprovisional v86 pagingへ反映し、entryからexact thunk ID `1`へ到達する。視覚的なtitle checkpointは旧schemaにmanual markerがないため`P0-COR-02`の未acceptanceのままである。

以後の title failure は Windows native trace／debugger で最小化し、browser runtime の実装と fixture は同じ Windows workspace で進める。
