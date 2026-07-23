# Windows native evidence

## 1. 文書情報

- ステータス: W0／trace v2 attribution／selected IDirect3D9 root／W1／W2／W3／W7 PE32 load／memory／entry return／stack／bootstrap／explicit import traps／BGI firstからseventeenth Win32 callはVerified。visual title／device-level method coverageはIn discovery
- 取得日: 2026-07-10／2026-07-11／2026-07-12
- runtime checkpoint更新日: 2026-07-12
- 完了 checkpoint: `W0`、`trace v2 attribution ingress`、`selected IDirect3D9 root capture`、`W1`、`W2 runtime flow`、`W3 browser first-pixel boundary`、`W7 PE32 load／memory／entry return ingress`、`production stack bootstrap ingress`、`bootstrap FS first-stop ingress`、`bootstrap FS ingress`、`process-parameters first stop`、`process-parameters ingress`、`explicit unsupported trap binding`、`BGI first runtime trap`、`BGI first Win32 call return`、`BGI second Win32 call return`、`BGI third Win32 call return`、`BGI fourth Win32 call return`、`BGI fifth Win32 call return`、`BGI sixth Win32 call return`、`BGI seventh Win32 call return`、`BGI eighth Win32 call return`、`BGI ninth Win32 call return`、`BGI tenth Win32 call return`、`BGI eleventh Win32 call return`、`BGI twelfth Win32 call return`、`BGI thirteenth Win32 call return`、`BGI fourteenth Win32 call return`、`BGI fifteenth Win32 call return`、`BGI sixteenth Win32 call return`、`BGI seventeenth Win32 call return`、`P0-COR-04` の最初のnative sample
- partial evidence: `P0-COR-01`、`P0-COR-02`、`P0-COR-03`、`P0-ARCH-03`、`P1-MEM-01`、`P1-MEM-02`、`P1-MEM-03`、`P1-ABI-01`、`P1-ABI-02`、`P1-ABI-03`、`P1-DIAG-01`、`P1-SCH-01`、`P1-QA-01`、`P2-PE-01`、`P2-PE-02`、`P2-PE-03`、`P2-PE-04`、`P2-PE-10`、`P2-WIN-02`、`P2-DIAG-01`、`P2-COR-01`、`P3-WIN-01`、`P3-WIN-03`、`P4-UI-01`、`P4-UI-02`、`P4-GDI-01`、`P6D-COR-01`、`P6D-COR-02`
- primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

raw JSONL、local summary、game file、absolute trace pathはGitに追加しない。summaryはabsolute title／system pathとPIDを保持するlocal-sensitive artifactであり、共有用に正規化されたmanifestではない。本書は同じraw traceから再生成できる小さなredacted resultだけを記録する。

## 2. W0 environment

Windows host で `npm ci`、canonical check、environment checker、Chrome version 取得を行った。

| Component | Observed |
|---|---|
| Windows | Windows 11 25H2、`10.0.26200.8655`、x64 host |
| Chrome stable | `150.0.7871.115` |
| Git | `2.55.0.windows.2` |
| Node.js／npm | `24.18.0`／`11.16.0` |
| Python | `3.14.6` |
| Frida／frida-tools | `17.15.4`／`14.10.4` |
| LLVM／Clang／LLD | `22.1.8` |
| BGI.exe | `1,193,472` bytes、fingerprint matched |

再現 command:

```powershell
npm ci
npm run check
powershell -NoProfile -ExecutionPolicy Bypass `
  -File tools/windows/check-environment.ps1 `
  -BgiExe "V:\softs\サクラノ詩\BGI.exe"
```

全 command は成功した。Chrome executable の version は取得済みだが、browser runtime がまだ存在しないため Chrome execution smoke の acceptance ではない。

project profileはWindows 11 Pro 25H2としているが、今回の`Win32_OperatingSystem`はWindows 11 Homeを返した。build `10.0.26200.8655` とx64は実測値として使い、edition差はruntime checkpointの合否に使わず、環境台帳上の未解決差として残す。

## 3. Sakura startup trace

### 3.1 Provenance

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\sakura-startup.jsonl" `
  --seconds 60 `
  --kill-after

node tools/summarize-windows-trace.mjs `
  --input ".work\windows-traces\sakura-startup.jsonl" `
  --output ".work\windows-traces\sakura-startup.summary.json"
```

既存summaryを意図的に再生成する場合だけsummarizerへ`--force`を加える。

| Field | Result |
|---|---|
| Capture source commit | `231775d8349d0af86975d1dfed38e06760172b41` |
| Capture dirty state | tracked／untrackedともclean。`.work`と`node_modules`はignore |
| `package-lock.json` SHA-256 | `e8b3191d2eec2384524b2b0d3bf46de23a6800dbbdd2d6ff4d76ee833d016814` |
| Trace schema | `vine.win32-trace.v1` |
| Summary schema | `vine.win32-trace-summary.v1` |
| Capture UTC | `2026-07-10T07:53:30.377309Z` ～ `2026-07-10T07:54:31.480395Z` |
| Duration | `61.103` seconds |
| Raw size | `1,675,818` bytes、`8,343` JSONL records |
| Raw SHA-256 | `c5130aeba6cb489c97c68f566b3612b154286bf93b452758603c6121b2fb7cf2` |
| Agent target | `ia32`、pointer size `4` |
| Agent sequence | `0` ～ `8340`、contiguous |
| API pairing | enter `4,135`、leave `4,135`、thread-local max depth `3` |
| Module events | `70` loads、case-fold 後 `64` unique names |
| Diagnostics | hook error `0`、agent message `0`、cleanup error `0` |

capture時点の`trace-start`はtarget pathだけを持ち、hashを埋め込まない。表のprimary fingerprintはcapture直前のenvironment checkとcapture直後の`Get-FileHash`が同値だったことを手動で結合したevidenceである。runnerはこのcapture後にtarget size／hashをheaderへ追加したため、次回からこの制約はない。

capture 開始時に利用者へタイトル画面までの操作を依頼した。trace には `SGTitle*` と `bgm040` の asset search が残り、title 用 resource load band まで進んだ。一方、この revision の schema は手動 checkpoint label と screenshot を持たないため、「タイトル画面が視覚的に表示された」こと自体は machine-verifiable acceptance に数えない。

### 3.2 Observed process module set

次は case-fold で重複 load をまとめた process-wide module set である。driver／OS 内部 load を含むため、この一覧だけから guest requirement または direct import と判定しない。

```text
BGI.exe, ntdll.dll, kernel32.dll, kernelbase.dll, imm32.dll, win32u.dll,
user32.dll, gdi32.dll, d3d9.dll, winmm.dll, dinput8.dll, gdi32full.dll,
ucrtbase.dll, msvcrt.dll, msvcp_win.dll, advapi32.dll, sechost.dll,
comdlg32.dll, rpcrt4.dll, combase.dll, shcore.dll, shlwapi.dll, version.dll,
dwmapi.dll, shell32.dll, comctl32.dll, ole32.dll, dxcore.dll, gdiplus.dll,
wininet.dll, dsound.dll, resampledmo.dll, powrprof.dll, winmmbase.dll,
oleaut32.dll, msdmo.dll, umpdc.dll, ws2_32.dll, crypt32.dll, iphlpapi.dll,
dnsapi.dll, psapi.dll, bcrypt.dll, dsparse.dll, nsi.dll, inputhost.dll,
kernel.appcore.dll, uxtheme.dll, atidx9loader32.dll, amdxn32.dll, dxgi.dll,
bcryptprimitives.dll, amdihk32.dll, resourcepolicyclient.dll, d3dx9_43.dll,
msctf.dll, hid.dll, setupapi.dll, devobj.dll, wintrust.dll, mmdevapi.dll,
audioses.dll, windows.ui.dll, avrt.dll
```

### 3.3 First-reach entries

sequence は ASLR address や wall clock より安定した順序 evidence として使う。call count は 60 秒 capture 固有の診断値であり、compatibility contract ではない。

| Sequence | Entry | Observation |
|---:|---|---|
| 56 | `ole32!CoInitialize` | COM initialization。続いて `CoInitializeEx` |
| 58 | `user32!CreateWindowExW` | OLE hidden window。title main window ではない |
| 63 | `gdi32!BitBlt` | 最初の hooked graphics entry |
| 65 | `kernel32!CreateFileA` | `${TITLE_ROOT}\BGI.gdb` |
| 73 | `user32!CreateWindowExA` | class `BGI - Main window`、Ethornell version title |
| 111 | `d3d9!Direct3DCreate9` | SDK version `0x20`。capture 中 4 calls |
| 241／244 | `LoadLibraryA`／`GetProcAddress` | `d3dx9_43.dll` と `D3DXCreateEffect` を動的解決。method call 自体は未観測 |
| 255 | `gdi32!StretchDIBits` | capture 中 2 calls |
| 457 | `dinput8!DirectInput8Create` | DirectInput 8 factory entry |
| 675 | `dsound!DirectSoundCreate8` | static ordinal import の OS 上の named entry |
| 680 | `ole32!CoCreateInstance` | capture 中 43 calls。CLSID／IID は現 schema では未取得 |
| 811 | `gdiplus!GdiplusStartup` | GDI+ startup |

`winmm.dll` は初期 module set にあるが、hook 対象の MCI／waveOut entry は 0 calls だった。D3D9、DirectInput、DirectSound は factory 到達だけが確定し、COM vtable method、format、state、Acquire、buffer、Present は未観測である。

### 3.4 File behavior

`CreateFileA` から内部 `CreateFileW` への nested wrapper を thread-local call stack で一つにまとめた。

| Metric | Result |
|---|---:|
| Raw A／W hook entries | 3,702 |
| Logical file calls | 1,887 |
| Case-insensitive unique paths | 1,481 |
| Title-relative logical calls | 1,813 |
| Title-relative successful／failed calls | 239／1,574 |
| Title-relative unique attempted paths | 1,462 |
| Successful title-relative paths | 31 |

成功した path:

```text
BGI.exe
BGI.gdb
data01099.arc, data01100.arc, data01101.arc, data01102.arc, data01103.arc
data01104.arc, data01105.arc, data01106.arc, data01107.arc, data01108.arc
data01109.arc, data01999.arc, data02099.arc, data02100.arc, data02300.arc
data02301.arc, data02302.arc, data02303.arc, data02304.arc, data02305.arc
data02306.arc, data02500.arc, data02501.arc, data02502.arc, data02503.arc
data05000.arc
sysgrp.arc, sysprg.arc, system.arc
```

失敗 probe は title root と `ActionCode`、`BitmapsForAnimation`、`Graphic`、`GroupedImage`、`MapData`、`Script`、`Sound`、`Stream`、`Voice`、`実験リソース` の各 search root に分布した。VFS は成功 path だけでなく、この case-insensitive search／not-found behavior を再現する必要がある。ただし現 hook は desired access、share、creation disposition、flags、last error を取らないため、read／write intent はまだ確定しない。

## 4. Native ABI oracle

Windows で deterministic PE32 fixture を直接実行した。

| Artifact | SHA-256 |
|---|---|
| `abi-smoke.exe` | `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| `vine_fixture.dll` | `a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878` |

`abi-smoke.exe` は `fixture_add(19, 23) == 42`、DLL から EXE の `add_one` callback、結果 `43` の store を通った場合だけ entry code `0` を返す。`Start-Process -Wait -PassThru` で exit code `0` を確認した。さらに Windows 上の `npm run check:toolchain` は同じ EXE を毎回実行し、report の `nativeAbiExitCode` に `0` を要求する。Windows 以外ではこの field は `null` であり、native oracle 済みとは扱わない。

## 5. Selected W1 slice

W1 primary implementation checkpoint は、同じ `abi-smoke.exe` を固定 T1 image として使った。

| Item | Fixed value／behavior |
|---|---|
| EXE SHA-256 | `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| Image base | `0x00400000` |
| Entry | `0x00401000` |
| IAT `fixture_add` | `0x00402034` |
| IAT `fixture_call_callback` | `0x00402038` |
| Guest callback `add_one` | `0x00401030` |
| Result storage | `0x00403000` |
| T1 guest RAM | 8 MiB identity mapping、`[0x00000000, 0x00800000)` |
| Runtime trap page | `[0x00500000, 0x00501000)` |
| Thunks | add `0x00500000`、callback `0x00500010`、各先頭は `UD2` |
| Sentinels | callback return `0x00500100`、entry return `0x00500110`、各先頭は `UD2` |
| Stack | `[0x00600000, 0x00700000)`、initial ESP `0x006FFFFC` |
| Execution model | single guest thread、instruction budget `3`、host call／callback／return sentinel のみ |

代表 contract test は次を一度に通す。

```text
guest entry
  -> fixture_add(19, 23)
  -> host result 42
  -> fixture_call_callback(add_one, 42)
  -> guest callback result 43
  -> callback return sentinel
  -> [0x00403000] = 43
  -> entry return sentinel with EAX = 0
```

この checkpoint は low-address contiguous mapping を使ってよいが、sparse 4 GiB memory、page ownership、production DLL loader、stdcall／thiscall、nested callback、async scheduler、Worker の decision にはしない。所有 thunk 外の `#UD` は通常の guest fault のままにし、title EIP 固有 skip は行わない。

trap dispatch は stop reason `0x106` と exact owned EIP の両方が一致した場合だけ行う。trap page の各 owned slot は `UD2` で始め、その他の byte は CPL3 `HLT` で埋めて accidental entry を structured `#GP` にする。host が resume addressを設定するときは version 1 CPU state の EIP と PREVIOUS_EIP を同じ値へ更新する。

entry開始時は `[0x006FFFFC] = 0x00500110`、EIP／PREVIOUS_EIP `0x00401000` とする。import thunk到達時の cdecl stack は `[ESP] = guest return`、argumentは `[ESP+4...]` である。host service完了時はreturn addressの4 bytesだけをpopし、argument cleanupはguest callerに残す。

expected import traps:

```text
fixture_add:
  ESP=0x006FFFF0
  [ESP]=0x0040100A, [ESP+4]=19, [ESP+8]=23

fixture_call_callback:
  ESP=0x006FFFF0
  [ESP]=0x00401019, [ESP+4]=0x00401030, [ESP+8]=42
```

callback thunkではouter ESPを保存し、`[outerESP-8] = 0x00500100`、`[outerESP-4] = 42`、ESP `outerESP-8`、EIP／PREVIOUS_EIP `0x00401030` とする。callback return trapはESP `outerESP-4`、EAX `43` で到達する。outer callを完了するときは保存したreturn addressへ戻り、ESPを`outerESP+4`へ進める。entry return sentinelではESP `0x00700000`、EAX `0` を要求する。budget `3` を固定し、少なくとも1回のbudget stopをcontractに含める。

loader-liteは実行前にEXE hashを検証し、`.data` のvirtual size 4 bytes／raw size 0をzero-fillし、IATをpatchしてから実行する。section permissionをmetadataとして保持し、CPU enforcementを加える場合はIAT patch後に適用する。このsliceだけのためにfull page protectionを実装しない。W1はsource-built adapter artifactを使うため、再現prerequisiteは次とする。

```powershell
npm run check:toolchain
npm run prepare:engine
npm run check:engine:adapter
npm run check:runtime:abi
npm run check:runtime:window-loader
```

## 6. W1 runtime result

`tools/run-runtime-abi-slice.mjs` は EXE を一度だけ immutable buffer へ読み、size と SHA-256 を検証してから DOS／COFF／PE32 optional header、section table、import descriptor、name、IAT を parse する。preferred base 以外の relocation、ordinal import、追加 DLL、追加 import はこの fixed loader-lite の範囲外であり、hash または manifest が異なる image は実行しない。private v86 setup／instruction-counter offsetへ触る前に、adapter Wasmもsize `1,882,836`、SHA-256 `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26`、imports／exports `22`／`809`へ固定する。

mapper は 8 MiB の contiguous v86 physical memory を identity guest address として使い、header と 4 section を `0x00400000` へ配置した。raw size `0` の `.data` 4 bytes は zero-fill され、IAT 2 slot は mapping 後に owned thunk へ置換された。trap page は `HLT` で初期化し、owned 4 address の先頭だけを `UD2` にした。section protection は metadata として parse するだけで、CPU enforcement はまだない。

実行 command と結果:

```text
npm run check:runtime:abi
runtime ABI slice passed: result=43, EAX=0, ESP=0x700000,
budget stops=5, instructions=21
```

contract は次を同時に検証した。

- stop reason `0x106` と exact owned EIP の両方による 2 import thunk と 2 return sentinel の分類
- `fixture_add` と `fixture_call_callback` 到達時の return address、argument、ESP
- callback frame、callback result `43`、outer cdecl return、guest caller による argument cleanup
- EBX、EBP、ESI、EDI の sentinel preservation
- entry result storage `43`、final EAX `0`、final ESP `0x00700000`
- instruction budget `3` による 5 回の budget stop と、64 stop cycle の harness 上限
- host-directed resume ごとの version 1 state EIP／PREVIOUS_EIP 同時更新
- `microtick` 以外の v86 host function import は default failure。MMIO、port I/O、halt、random、generated-code callback は 0 回

Windows native oracle の process exit `0` と W1 guest entry return EAX `0` は一致し、guest memory の result `43` が内部の callback 経路も通ったことを補強する。これは `P1-ABI-01`／`P1-ABI-03`／`P1-SCH-01`／`P1-QA-01` の限定した partial evidenceであり、各 task または `AC-P1` 全体の完了ではない。

memory measurement は、image `[0x00400000, 0x00405000)`、trap page `[0x00500000, 0x00501000)`、stack `[0x00600000, 0x00700000)` が 8 MiB contiguous allocationへ収まったことだけを示す。high-address PE、reserve／commit、protection、guard、page ownership、sparse 4 GiB、browser memory ceilingは未検証であり、`P0-ARCH-03` は未完了である。

W1 は Node.js 24 の WebAssembly runtime 上の fixed harness である。production Rust／TypeScript workspace、DLL lifecycle、stdcall／thiscall、nested callback、SEH、wait／wake、async scheduler、Runtime Worker、Chrome main-thread responsivenessを証明しない。

## 7. W2 native window／message／pixel oracle

`fixtures/pe32/window-pixel/` はWindows SDK header／libraryを使わないfreestanding i386 fixtureである。tracked DEFから`llvm-dlltool -m i386 -k`でimport libraryを生成し、Clang／LLDだけでGUI EXEへlinkする。使用した`llvm-dlltool.exe`のSHA-256は`80934e8f208a0cc2a87a6057f871d0f492461952b8672464749a6c3dff34109c`で、LLVM suiteは22.1.8である。

| Property | Fixed result |
|---|---|
| Artifact | `window-pixel.exe`、4,096 bytes |
| SHA-256 | `ad8f4a0bddd759ac52b18119e60838c3ad13d29994449f38b0393fdedde454e9` |
| Format | PE32／i386、GUI subsystem、preferred base `0x00400000` |
| Image | entry `0x00401000`、WndProc `0x00401260`、size `0x5000`、headers `0x400` |
| Import／IAT | directory RVA `0x2030`、IAT RVA `0x20C0`、size `0x40` |
| Sections | `.text` RVA `0x1000`／VS `0x409`、`.rdata` `0x2000`／`0x1F8`、`.data` `0x3000`／`0xCC`、`.reloc` `0x4000`／`0xB8` |

exact import manifest:

```text
kernel32.dll: GetModuleHandleA
user32.dll: BeginPaint, CreateWindowExA, DefWindowProcA, DestroyWindow,
            DispatchMessageA, EndPaint, GetMessageA, InvalidateRect,
            PostQuitMessage, RegisterClassExA, ShowWindow
gdi32.dll:   SetPixel
```

このうち`SetPixel`以外の12 symbolはprimary fingerprintのBGI.exeにもstatic importとして存在する。`SetPixel`はfirst pixel commandを明確にするtest-only APIであり、このfixtureからBGIのGDI requirementへ昇格しない。

fixtureはx86 scalar、pointer、`WNDCLASSEXA`、`MSG`、`PAINTSTRUCT`のsize／field offsetをcompile-timeに検査する。160x120 windowを作り、補助messageを無視して関連eventだけを次の順で記録する。

```text
WM_CREATE
  -> queued WM_PAINT via GetMessageA / DispatchMessageA
  -> BeginPaint -> SetPixel(1, 1, red) -> EndPaint
  -> DestroyWindow -> WM_DESTROY -> PostQuitMessage(0)
  -> GetMessageA returns 0 with WM_QUIT
```

各event／API count、pixel return、event順、quit codeが一致した場合だけentryが`0`を返す。runnerはnative processを10秒で停止し、paintを抑止する`windowsHide`を使わない。`npm run check:toolchain`はnative exit code `0`をreportへ記録し、追加の5回連続実行もすべてexit code `0`、初回約100ms、以降約41～43msだった。

このoracleはWindows上のstdcall WndProc、queued message、GDI pixel call、cleanup順を固定する。このoracle単独ではVine内のstdcall gateway、logical HWND／DC、message scheduler、host surface、browser presentation、物理的に見えるpixelを検証しない。後続のfixed W2 harnessがlogical HWND、`WM_CREATE` callback round trip、fixed logical visibility／invalidation／MSG materialization、paint／nested destroy／quit／entry returnを別途検証するが、W2 production runtime、W3 first frame、`P4-*`、`AC-P4`の完了とは扱わない。

## 8. W2 loader／module／class／window-create／visibility／invalidation／message-dispatch ingress／paint／quit flow result

`tools/run-runtime-window-loader-slice.mjs`は`window-pixel.exe`を一度だけimmutable bufferへ読み、size `4,096`とSHA-256 `ad8f4a0bddd759ac52b18119e60838c3ad13d29994449f38b0393fdedde454e9`をadapter初期化前に固定する。DOS／COFF／PE32 header、4 section、3 import descriptor、13 named import、IAT directoryをchecked rangeでparseし、map後にもsource hashが不変であることを検証する。adapter WasmはW1と同じsize、hash、import／export countへ固定する。

8 MiB contiguous identity memoryのpreferred base `0x00400000`へheaderとsectionをmapし、13 IAT slotを`0x00500000`からstride `0x10`の一意なowned `UD2` thunkへ全件解決した。各thunkはmodule、symbol、IAT RVA／VAを逆引きでき、3 descriptorのIAT terminator `0x004020C4`、`0x004020F4`、`0x004020FC`はzeroのまま保持した。trap page外へのthunk配置、entry return sentinelとの衝突、unowned `#UD`は失敗する。

実行commandと結果:

```text
npm run check:runtime:window-loader
runtime window paint and quit flow passed: imports=13,
completed-variants=13, callback-events=6, hwnd=0x57010001,
hdc=0x48010001, pixel=0xff, budget stops=66,
execute stops=83, instructions=231
```

entry `0x00401000`からinstruction budget `3`で実行すると、`push edi`、`push esi`、`push 0`の後、call直前`0x00401004`／ESP `0x006FFFF0`で1回budget stopする。次のsliceはIAT `0x004020C0`経由でthunk `0x00500000`へ入り、stop reason `0x106`、error code `-1`、ESP `0x006FFFEC`、return `0x0040100A`、argument `NULL`を得る。

fixed main module identityはname、preferred-base address、image size、source hashを持ち、partial variant `GetModuleHandleA(NULL)`はそのbase `0x00400000`をEAXへ返す。1 DWORD argumentのstdcallとしてreturn addressとargumentの8 bytesをpopし、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x0040100A`へstateをrestoreする。restore直後のstateを再saveしてEBX／EBP／ESI／EDI不変と更新したEAX／ESP／EIPを検証し、その後guest自身がESIへmodule baseを書き込む。

resume後はbudget stop `0x00401014`、`0x0040102D`、owned thunk `0x005000A0`を経て`RegisterClassExA`へ到達する。fixed partial variant `wndclassexa-v48-local-ascii`はargument `0x00403004`の48 bytesをguest range検査後に読み、`cbSize=48`、WndProc `0x00401260`がexecutable image section内、hInstance `0x00400000`がmain module、その他のfixture fieldがzeroであることを検証する。class-name pointer `0x00402000`はinteger atom formを拒否し、64-byte上限でnonempty NUL-terminated ASCII `VineWindowPixelFixture`を読む。

logical class keyはmodule hash／base、local scope、A encoding、copied class nameから作る。guestの`WNDCLASSEXA` addressとclass-name pointerをrecordに保持せず、decoded name、module identity、WndProc callback descriptorのentry RVA／calling convention／argument bytes、style、class／window extra byte countをhost-owned frozen objectへcopyし、key lookupとATOM lookupの両方へ同一objectを登録する。最初のdeterministic local class ATOMは`0xC000`であり、native ATOM値との数値一致ではなくnonzero／一意性をcontractとする。`RegisterClassExA`はEAX `0x0000C000`を返し、1 DWORD stdcallとしてESP `0x006FFFEC`から`0x006FFFF4`、EIP／PREVIOUS_EIP `0x00401042`へresumeする。

その後budget stop `0x0040104E`、`0x0040105E`、`0x00401076`、`0x0040108E`を経て、累積30命令で`CreateWindowExA`のowned thunk `0x00500020`へ到達する。stop reason `0x106`とowned EIPの両方が一致した時だけserviceとして分類し、call site `0x00401095`、return `0x0040109B`、ESP `0x006FFFC0`を得る。12 raw argumentsは次である。

```text
extended style: 0
class / title:  0x00402000 / 0x00402000
style:          0x00CF0000
x / y:          0x80000000 / 0x80000000
width / height: 160 / 120
parent / menu:  0 / 0
instance / arg: 0x00400000 / 0
```

fixed partial variant `createwindowexa-v12-local-ascii-wm-create`は上の12 wordsをすべてchecked readし、class／title pointer `0x00402000`をそれぞれ64-byte上限のnonempty NUL-terminated ASCIIとして読み、`VineWindowPixelFixture`へcopyする。module identityを含むlocal class keyで登録済みclassを引き、ATOM mapのobject identityとWndProc callback target `0x00401260`を再検証する。

最初のlogical HWNDは`0x57010001`である。host-owned sealed window recordはmodule identity、class key／ATOM／copied name、copied title、extended／normal style、semantic `CW_USEDEFAULT` position、requested outer size `160 x 120`、null parent／menu、lifecycle、visibilityを保持する。作成開始時は`creating`／hiddenで、callback成功後に`created`／hiddenとなる。上位byteのtype tag、generation `1`、slot `1`を含む数値layoutはこのharnessでtyped lookup／object identityを検証するためのdeterministic choiceであり、native HWNDとの数値一致やproduction handle encoding、reuse／stale detection／lifetimeを決定しない。

trap pageとは別のruntime-owned scratch `0x00501000`へ、次の48-byte x86 `CREATESTRUCTA`を書き込む。これは一つの同期callback用固定rangeであり、general scratch allocatorやnested lifetime modelではない。

```text
lpCreateParams / hInstance: 0 / 0x00400000
hMenu / hwndParent:         0 / 0
cy / cx:                    120 / 160
y / x:                      0x80000000 / 0x80000000
style:                      0x00CF0000
lpszName / lpszClass:       0x00402000 / 0x00402000
dwExStyle:                  0
```

outer `CreateWindowExA` trapのESPは`0x006FFFC0`である。そこから20 bytes下のESP `0x006FFFAC`へcallback frameを作り、次をguest memoryからread backして検証する。

```text
[ESP +  0]  callback return sentinel 0x00500E00
[ESP +  4]  HWND                     0x57010001
[ESP +  8]  WM_CREATE                1
[ESP + 12]  wParam                   0
[ESP + 16]  CREATESTRUCTA pointer    0x00501000
```

WndProc `0x00401260`をresumeするとguest自身が`create_count=1`、`event_count=1`、最初のevent `WM_CREATE`、`failure_code=0`を記録してEAX `0`で`ret 16`する。callback return sentinelはESP `0x006FFFC0`で到達し、outer frameと12 argumentsが保持されていることを再検証する。host call ID／parent call IDを対応付け、logical windowを`created`へ遷移させた後、outer stdcallをEAX `0x57010001`、ESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x0040109B`でresumeする。

guestはHWNDをnonzeroとして受理し、`ShowWindow` owned thunkへ到達する。fixed partial variant `showwindow-sw-show-hidden-to-visible`はcall site `0x004010A8`、return `0x004010AE`、IAT `0x004020F0`、thunk `0x005000B0`、ESP `0x006FFFE8`を検証し、2 arguments `[0x57010001, 5]`をchecked readする。HWNDが同じcreated／hidden live objectを指し、commandが`SW_SHOW`である場合だけ、visibilityをfalseからtrueへ遷移して`showCommand=5`を保持する。

Win32 `ShowWindow`のBOOLはsuccessではなく呼出し前のvisibilityを表す。このfixed caseはhiddenだったため正常にEAX `0`を返し、return addressと2 argumentsの12 bytesをpopしてESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010AE`でresumeする。ESIはHWNDのままである。guestはreturnを分岐に使わず、`push FALSE`、`push NULL`、`push HWND`の3命令後にcall site `0x004010B3`へ到達する。各call siteはreturn addressから無条件に逆算せず、直前6 bytesが`FF 15 <absolute IAT>`であり、operandが到達thunkを指すIAT slotと一致し、requester imageのexecutable section内にあることを検証する。

fixed partial variant `invalidaterect-whole-client-no-erase`はcall site `0x004010B3`、return `0x004010B9`、IAT `0x004020E4`、thunk `0x00500080`、ESP `0x006FFFE4`を検証し、3 arguments `[0x57010001, NULL, FALSE]`をchecked readする。HWNDが同じcreated／visible live objectを指し、RECT pointerがNULL、eraseがFALSEである場合だけ、windowにsymbolic `{ kind: "whole-client" }` update regionと`erasePending=false`を保持する。同じowner guest threadのgenerated-paint laneへ、sequence `1`、kind `generated-paint`、source `window-update-region`、`[HWND, WM_PAINT, 0, 0]`のfrozen candidateを1件追加し、paint stateを`queued`へ遷移する。

`InvalidateRect`はBOOL `1`をEAXへ返し、return addressと3 argumentsの16 bytesをpopしてESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010B9`でresumeする。ESIはHWNDのままである。直前のinvalidation checkpointは、続くguest codeがMSG `0x00403034`とfilter argumentsを積み、call site `0x004010CC`の`GetMessageA`へ到達し、zero MSGとqueued candidateを保持して停止するところまでを証明した。

message-dispatch checkpointで追加したfixed partial variant `getmessagea-unfiltered-generated-paint-zero-metadata`はcall／return `0x004010CC`／`0x004010D2`、IAT／thunk `0x004020E0`／`0x00500070`、ESP `0x006FFFE0`を検証し、4 arguments `[0x00403034, NULL, 0, 0]`をchecked readする。28-byte writable rangeへ`[0x57010001, WM_PAINT, 0, 0, 0, 0, 0]`を全書込し、generated-paint laneからsequence `1`のcandidateをdequeueして1件から0件へ減らす。windowのpaint stateを`queued`から`in-flight`へ遷移し、whole-client update region、erase false、sequence `1`を保持する。BOOL `1`をEAXへ返し、return addressと4 argumentsの20 bytesをpopしてESP `0x006FFFF4`、EIP／PREVIOUS_EIP `0x004010D2`でresumeする。

guestはdispatch countとdispatched-paint countを各1へ進め、IAT `0x004020D8`からESIへowned thunk `0x00500050`をloadした後、bytes `FF D6`の`CALL ESI`をcall site `0x004010F5`で実行する。fixed partial variant `dispatchmessagea-generated-wm-paint-callback`はreturn `0x004010F7`、outer ESP `0x006FFFEC`、argument `[0x00403034]`、7-DWORD MSG copy、in-flight candidate identity、live HWND／class／WndProcを検証する。API call ID `3`を作り、その内側のESP `0x006FFFD8`へcallback ID `4`、sentinel `0x00500E00`、arguments `[HWND, WM_PAINT, 0, 0]`を積んでWndProc `0x00401260`へresumeする。

message-dispatch checkpoint時点では、guest WndProcがpaint count `1`、event count `2`、second event `WM_PAINT`、BeginPaint count `1`を記録した。callback内でcall／return `0x0040133D`／`0x00401343`、IAT／thunk `0x004020C8`／`0x00500010`、ESP `0x006FFFC8`、arguments `[HWND, 0x0040308C]`の`BeginPaint`へ到達し、64-byte `PAINTSTRUCT`は未書込でzero、failure codeは0、EndPaint／SetPixel／destroy／quit countは0だった。6件のcompleted service return、3件のcallback event、pending frame 2件と当時のdiagnosticは次である。

```text
kind:                  host-service-return
module / symbol:       kernel32.dll / GetModuleHandleA
variant / status:      module-name-null / partial
call / return VA:      0x00401004 / 0x0040100A
argument / result:     NULL / 0x00400000
calling convention:    stdcall, argument bytes 4
stack before / after:  0x006FFFEC / 0x006FFFF4

kind:                  host-service-return
module / symbol:       user32.dll / RegisterClassExA
variant / status:      wndclassexa-v48-local-ascii / partial
call / return VA:      0x0040103C / 0x00401042
argument / result:     0x00403004 / 0x0000C000
class name / WndProc:  VineWindowPixelFixture / 0x00401260
stack before / after:  0x006FFFEC / 0x006FFFF4

kind:                  host-service-return
module / symbol:       user32.dll / CreateWindowExA
variant / status:      createwindowexa-v12-local-ascii-wm-create / partial
call / return VA:      0x00401095 / 0x0040109B
argument bytes:        48
result:                0x57010001
callback target:       0x00401260, WM_CREATE, result 0
stack before / after:  0x006FFFC0 / 0x006FFFF4

kind:                  host-service-return
module / symbol:       user32.dll / ShowWindow
variant / status:      showwindow-sw-show-hidden-to-visible / partial
call / return VA:      0x004010A8 / 0x004010AE
argument bytes:        8
arguments / result:    [0x57010001, 5] / 0
visibility:            false -> true, showCommand 5
stack before / after:  0x006FFFE8 / 0x006FFFF4

kind:                  host-service-return
module / symbol:       user32.dll / InvalidateRect
variant / status:      invalidaterect-whole-client-no-erase / partial
call / return VA:      0x004010B3 / 0x004010B9
argument bytes:        12
arguments / result:    [0x57010001, NULL, FALSE] / 1
update / erase:        whole-client / false
paint candidate:       sequence 1, generated-paint, queued
stack before / after:  0x006FFFE4 / 0x006FFFF4

kind:                  host-service-return
module / symbol:       user32.dll / GetMessageA
variant / status:      getmessagea-unfiltered-generated-paint-zero-metadata / partial
call / return VA:      0x004010CC / 0x004010D2
IAT / thunk VA:        0x004020E0 / 0x00500070
argument bytes:        16
arguments / result:    [0x00403034, NULL, 0, 0] / 1
MSG words:             [0x57010001, 0x000F, 0, 0, 0, 0, 0]
paint / lane depth:    queued -> in-flight / 1 -> 0
stack before / after:  0x006FFFE0 / 0x006FFFF4

kind:                  pending-api-frame
call ID:               3
module / symbol:       user32.dll / DispatchMessageA
variant / status:      dispatchmessagea-generated-wm-paint-callback / partial
call encoding:         CALL ESI (FF D6), register-indirect-esi
call / return VA:      0x004010F5 / 0x004010F7
IAT / thunk VA:        0x004020D8 / 0x00500050
stack / argument:      0x006FFFEC / [0x00403034]
phase / resume ESP:    callback / 0x006FFFF4

kind:                  guest-callback-enter
call / parent ID:      4 / 3
role / target:         window-procedure-wm-paint / 0x00401260
return sentinel:       0x00500E00
stack / arguments:     0x006FFFD8 / [0x57010001, 0x000F, 0, 0]

kind:                  unsupported-reentrant-host-service
parent / root call ID: 4 / 3
root service:          user32.dll!DispatchMessageA
root variant:          dispatchmessagea-generated-wm-paint-callback
callback role:         window-procedure-wm-paint
module / symbol:       user32.dll / BeginPaint
guest call VA / RVA:   0x0040133D / 0x133D
return VA / RVA:       0x00401343 / 0x1343
IAT VA / RVA:          0x004020C8 / 0x20C8
thunk VA:              0x00500010
stack / arguments:     0x006FFFC8 / [0x57010001, 0x0040308C]
engine stop / error:   0x106 / -1
```

直前のinvalidation checkpointは19 budget／26 execute stop／70命令、message-dispatch checkpointは28 budget／37 execute stop／101命令で`BeginPaint` ingressまでを固定した。今回のcontinuationは同じinstruction budget `3`で次を通す。

```text
BeginPaint:     variant beginpaint-whole-client-harness-surface
                call/return 0x0040133D/0x00401343, IAT/thunk 0x004020C8/0x00500010
                ESP 0x006FFFC8, args [0x57010001, 0x0040308C]
                result HDC 0x48010001, paint in-flight -> painting
SetPixel:       variant setpixel-harness-surface-exact-point
                call/return 0x00401357/0x0040135D, IAT/thunk 0x004020F8/0x005000C0
                args [0x48010001, 1, 1, 0x000000FF], result 0x000000FF
EndPaint:       variant endpaint-matching-active-session
                call/return 0x0040138D/0x00401393, IAT/thunk 0x004020DC/0x00500060
                result BOOL 1, paint painting -> none, HDC/session retired
DestroyWindow:  variant destroywindow-synchronous-wm-destroy, call ID 5
                call/return 0x004013D6/0x004013DC, IAT/thunk 0x004020D4/0x00500040
                nested WM_DESTROY callback ID 6, result BOOL 1
PostQuitMessage: variant postquitmessage-thread-slot-exit-zero
                 call/return 0x004013FD/0x00401403, IAT/thunk 0x004020E8/0x00500090
                 exit 0 -> thread quit sequence 2
GetMessageA:    variant getmessagea-unfiltered-thread-quit-zero-metadata
                CALL EDI, call/return 0x00401102/0x00401104
                MSG [NULL, WM_QUIT, 0, 0, 0, 0, 0], result 0
completion:     entry sentinel 0x00500F00, EAX 0, ESP 0x00700000
```

`BeginPaint`はwhole-client updateをsessionへsnapshotしてwindowからclearし、harness-only logical client bounds `[0,0,2,2]`と64-byte PAINTSTRUCT `[HDC, 0, 0, 0, 2, 2, 0 ...]`を作る。requested outer size `160x120`とは明示的に異なる。logical pixel mapは`(1,1) = 0x000000FF`を1件だけ持つ。EndPaint後はactive HDC／sessionなし、Destroy後はwindowが`destroyed`／hiddenとなりretired tableへ移る。

call ownershipはDispatch API `3` -> `WM_PAINT` callback `4` -> Destroy API `5` -> `WM_DESTROY` callback `6`である。`PostQuitMessage(0)`後、callback `6`は147命令、callback `4`は155命令でsentinel returnし、各APIもstrict reverse orderでreturnする。2回目のGetMessageは161命令からWM_QUITをconsumeし、最終231命令でentry sentinelへ到達する。host frame depth、active HDC／session、paint candidate、in-flight paint、quit slotはすべて0／NULLである。completed partial service returnは13件、callback eventは6件、66 budget stop、83 execute stop、17 owned `UD2`である。

native fixtureは関連event `WM_CREATE -> WM_PAINT -> WM_DESTROY`、各API／event count `1`、pixel return `0x000000FF`、quit code／entry result `0`を自己検証する。fixed W2 harnessはこのsemantic sequenceとcounterを一致させるが、native client geometry、PAINTSTRUCT bytes、HWND／HDC数値、pixel storage、zero metadata、call ID／stack addressをgoldenにしない。Windowsの補助messageや`WM_NCCREATE`／`WM_NCDESTROY`もfixed Vine sequenceには含めない。

既存invalidation／message-dispatch evidenceを保持した上で、paint／quit checkpointの新しいdeltaは`P1-ABI-01`／`P1-ABI-02`／`P1-ABI-03`／`P1-DIAG-01`／`P1-SCH-01`／`P3-WIN-01`／`P3-WIN-03`／`P4-UI-01`／`P4-UI-02`／`P4-GDI-01`への限定したpartial evidenceである。対応するrequirementは`REQ-ABI-01`／`REQ-ABI-02`／`REQ-ABI-03`、`REQ-API-01`／`REQ-API-02`、`REQ-UI-01`／`REQ-UI-02`、`REQ-GDI-01`、`REQ-DIAG-01`に限る。`P4-GDI-01`はBeginPaintからEndPaintまでのtyped HDC lifetimeだけである。`P4-GFX-01`、`P4-GDI-04`、`REQ-GDI-02`はclaimせず、成功完走による`REQ-API-06`／`REQ-DIAG-03`の新しいdeltaもない。`AC-P1-03`、`AC-P3-02`、`AC-P4-01`、`AC-P4-02`を含むacceptance criterionは一つも完了しない。

general BeginPaint region／client geometry／clip／erase／background／nesting、general DC state、SetPixelの別座標／clip／format／failure、paint中のreinvalidation、mismatched／double EndPaintは未検証である。DestroyWindowのchild／owner／focus／capture／timer／cross-thread／repeat、`WM_CLOSE`／`WM_NCDESTROY`／resource cleanup、arbitrary callback nesting／SEH／LastErrorも未検証である。PostQuit／GetMessageの別code／multiple／ordering／filter／block／wake／error／他message、process teardown／DLL detach、common surface／format／pitch／orientation／dirty region、general DOM／Canvas presentation、production object／surface／queue layoutも未実装または未検証である。次節のW3はfixed 1-pixel boundaryだけを例外として検証する。`SetPixel`はtest-onlyでBGIのGDI requirementへ昇格しない。

## 9. W3 browser first-pixel result

`npm run check:runtime:window-browser`はfixed W2 harness全体を実行する。fixed `EndPaint` completionでlogical bounds `[0,0,2,2]`とpixel `(1,1) = COLORREF 0x000000FF`を次のbrowser-facing eventへ変換し、entry sentinel、final register／stack、logical window／paint session、counter、pixelを含む残りのW2 assertionがすべて成功した後だけexactly 1件を外へemitする。

```json
{
  "schema": "vine.browser-present",
  "schemaVersion": 1,
  "kind": "pixel-present",
  "presentSequence": 1,
  "source": { "checkpoint": "fixed-w2-window-pixel", "paintSequence": 1 },
  "frame": { "width": 2, "height": 2, "pixelEncoding": "rgba8" },
  "pixels": [{ "x": 1, "y": 1, "rgba": [255, 0, 0, 255] }]
}
```

runnerはこのeventをstdout transportからexactly 1件だけ受け取り、一時loopback HTTP serverでtracked pageとone-shot Canvas 2D sinkへ渡す。sinkはschema／version／fixed payloadを検査し、2回目のpresentを拒否する。clean temporary profileのChrome `150.0.7871.115`をheadless起動した結果、present count `1`、Canvas size `2x2`、`getImageData(1,1,1,1)`のreadback `[255,0,0,255]`で完了した。Win32 `COLORREF`のlow byte redからbrowser `rgba8`への変換がこの境界で明示される。

これは`P4-UI-02`のpresentation boundaryへの限定したpartial evidenceである。Chrome内でguest CPU、PE loader、Win32 lifecycle、Runtime Workerを実行せず、main-thread protocol、frame queue、common surface、pitch／orientation／dirty region、renderer backend、cadence、resize／DPR／context lossを決定または検証しない。sparse eventはW3 checkpoint-localであり、`P4-GFX-01`／`P4-GFX-02`、`AC-P4-01`／`AC-P4-02`を完了しない。

## 10. Trace v2 attribution and selected IDirect3D9 root

commit `e4548ec41e28f354c49d9a1433adf877c120b905`のclean worktreeから、primary fingerprintを対象に次を実行した。vtable optionを省略したtraceは従来のfocused export bandだけをhookし、このcaptureだけ`IDirect3D9` rootを明示選択する。

```powershell
$trace = ".work\windows-traces\sakura-idirect3d9-v2-clean-20260711-035657.jsonl"
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output $trace `
  --seconds 15 `
  --kill-after `
  --vtable idirect3d9

node tools/summarize-windows-trace.mjs `
  --input $trace `
  --output "$trace.summary.json"
```

| Field | Result |
|---|---|
| Trace／summary schema | `vine.win32-trace.v2`／`vine.win32-trace-summary.v2` |
| Capture | 2026-07-11 `03:56:57.484 +09:00`～`03:57:13.544 +09:00`、`16.060` seconds、`timeout-kill` |
| Target | `1,193,472` bytes、SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1` |
| Raw | `2,267,972` bytes、`7,133` JSONL records、SHA-256 `cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898` |
| Local summary | `58,021` bytes、SHA-256 `3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9` |
| Agent sequence | `0`～`7,130`、contiguous |
| API pairing | enter／leave／pair各`3,529`、max depth `3`、call ID contract balanced |
| Selected vtable | `IDirect3D9` 1 table、17 named slots。`IDirect3DDevice9`その他は未選択 |
| Diagnostics | hook／vtable／call-stack／agent／cleanup errorすべて`0` |

trace v2では各`api-enter`／`api-leave`が同じglobal call ID、thread-local parent call ID、caller absolute address／module／RVAを持つ。leaveはreturn直後のunsigned LastErrorとinteger millisecond durationを持つ。summarizerはcall IDのcontiguity、thread stack上のparent、enter／leaveのcaller／module／API／interface／slot／object identity、LastError／duration rangeを検査する。historical trace v1はlegacy thread-stack pairingとして引き続き入力できる。

| Slot | IDirect3D9 method | Calls | BGI direct caller RVA |
|---:|---|---:|---|
| 1 | `AddRef` | 3 | なし。3 callsとも`d3d9.dll`内部 |
| 2 | `Release` | 5 | `0x5BB0E`で3 calls |
| 4 | `GetAdapterCount` | 6 | `0x59D36`で6 calls |
| 5 | `GetAdapterIdentifier` | 1 | `0x5B91C`で1 call |
| 8 | `GetAdapterDisplayMode` | 4 | `0x59DA5`で3 calls、`0x5B8D6`で1 call |
| 9 | `CheckDeviceType` | 5 | `0x5B9D6`／`0x5BA1D`で各1 call。残り3 callsは内部 |
| 10 | `CheckDeviceFormat` | 13 | `0x5BA69`で1 call。残り12 callsは内部 |
| 11 | `CheckDeviceMultiSampleType` | 3 | なし。3 callsとも`CreateDevice`内部 |
| 14 | `GetDeviceCaps` | 1 | `0x5B949`で1 call |
| 15 | `GetAdapterMonitor` | 9 | `0x59D5E`で6 calls。残り3 callsは内部 |
| 16 | `CreateDevice` | 3 | `0x5ACA5`で3 calls |

factory `Direct3DCreate9`は4 callsで、BGI callerは`0x5AB8C`が3回、`0x5B892`が1回である。root methodは11 unique／53 total calls、そのうちBGI directは27 callsである。3回の`CreateDevice`はAdapter `0`、DeviceType `1`、BehaviorFlags `0x44`、HRESULT `0`で、各内部root callはCreateDeviceのparent call IDへ帰属した。focus HWND、structure／output pointerの数値はnondeterministicでgoldenにしない。slot `0`、`3`、`6`、`7`、`12`、`13`はこのcaptureで未到達である。

LastErrorはAPIが成功時にclearする保証を意味せず、例えば内部`AddRef` leaveは値`122`を保持した。これをHRESULT／return valueの代わりに成功判定へ使わない。今回のcaptureは`P6D-COR-01`のroot-interface prerequisiteへのpartial evidenceであり、presentation parametersの内容、`IDirect3DDevice9`／resource vtable、format／state／draw／Present／Reset／loss、DirectInput／DirectSound methodを観測または確定しない。

### 10.1 GetAdapterDisplayMode output and consumer evidence

2026-07-18にprimary fingerprintへ同じselected vtable captureを2回実行し、trace agent／summarizerへ追加した`D3DDISPLAYMODE`のcall前後16 bytesを検証した。raw traceとsummaryは`.work/windows-traces/`に置き、Gitへ含めない。

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work\windows-traces\bgi-d3d9-display-mode-20260718-r1.jsonl `
  --seconds 5 `
  --kill-after `
  --vtable idirect3d9

node tools/summarize-windows-trace.mjs `
  --input .work\windows-traces\bgi-d3d9-display-mode-20260718-r1.jsonl `
  --output .work\windows-traces\bgi-d3d9-display-mode-20260718-r1.summary.json
```

| Field | Primary | Confirmation |
|---|---:|---:|
| Raw bytes／lines | `2,192,729`／`6,886` | `2,249,188`／`7,068` |
| Raw SHA-256 | `6b9bd17809ef235f1817fb08ccd4bfb92f27f3cd424bd0f15500470d73b05208` | `fac93ab62effd0228dc8c8a88b617f3aced5fcff9429c9e90c9347979c660f92` |
| Balanced API pairs | `3,405` | `3,496` |
| Diagnostics | `0` | `0` |
| Successful `GetAdapterDisplayMode` calls | `4` | `4` |

両captureともAdapter `0`、HRESULT `0`で、outputは毎回Width `1920`、Height `1080`、RefreshRate `60`、Format `22`（`D3DFMT_X8R8G8B8`）、bytes `80070000380400003c00000016000000`だった。最初のcallはzeroed 16-byte inputを全域上書きする。BGI direct callerはRVA `0x5B8D6`が1回、`0x59DA5`が3回で再現した。前者の直後はHRESULTのsignを検査し、後続処理はWidth／Height／RefreshRate／Formatの全fieldを読む。

これはbounded primary display profileを選ぶoracleであり、host display topologyをbrowserへimportする要求ではない。複数adapter、mode enumeration、fullscreen切替、format一般化、`CreateDevice`、renderer resourceを確定しない。source executable SHA-256は既存fingerprint `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のまま不変だった。

### 10.2 GetAdapterIdentifier output and consumer evidence

2026-07-18に同じprimary fingerprintへselected vtable captureを再実行し、trace
agent／summarizerが`D3DADAPTER_IDENTIFIER9`のcall前後1,100 bytes、decoded fields、hash、
pointer relationを検証した。最終evidenceはr3／r4で、raw traceとsummaryは
`.work/windows-traces/`に置きGitへ含めない。

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work\windows-traces\bgi-d3d9-adapter-identifier-20260718-r3.jsonl `
  --seconds 5 `
  --kill-after `
  --vtable idirect3d9

node tools/summarize-windows-trace.mjs `
  --input .work\windows-traces\bgi-d3d9-adapter-identifier-20260718-r3.jsonl `
  --output .work\windows-traces\bgi-d3d9-adapter-identifier-20260718-r3.jsonl.summary.json
```

| Field | Primary r3 | Confirmation r4 |
|---|---:|---:|
| Raw bytes／lines | `2,207,106`／`6,888` | `2,263,088`／`7,068` |
| Raw SHA-256 | `c635bde508a727e5c42dd0d17b75d3e82e30d62f77ad06bf497133cb4f8bd7d3` | `d13217c0bc87789d61da46af87339e63bb0a92ac11dd406698a6b75982574da5` |
| Balanced API pairs | `3,406` | `3,496` |
| Diagnostics | `0` | `0` |
| Successful `GetAdapterIdentifier` calls | `1` | `1` |

両captureはcaller RVA `0x5B91C`、Adapter `0`、Flags `0`、HRESULT `0`で一致する。
Call前は1,100 zero bytes／SHA-256
`a805fd6f7ad225e289321ba6e2b079e92c3accc1675a7ba6626101da38f727f2`、native
call後はSHA-256
`773a5d417dceeab588a8724b951f9926543e39b42fad71b58d1ba6efb6a8497c`で、169 bytesが
変化した。128-instruction consumer captureはHRESULTのsignだけを検査して
`GetDeviceCaps`へ進み、output fieldを読まない。BGI image内のidentifier保存先参照もcall／failure
logging相当の2箇所だけだった。

Native outputはcapture hostのAMD identityであり、guest-visible semantic goldenではない。Vineは
host GPU値をimportせず、同じlogical primary displayへ結び付いたversioned virtual identityを返す。
Ignoredのbounded x64 oracleではinvalid adapterとunknown flagsが
`D3DERR_INVALIDCALL`かつsentinel不変だった。NULL outputはこのhostでaccess violationとなったが、
Vineはguest memory safetyを優先し、NULLをmutation-free `D3DERR_INVALIDCALL`、unwritable rangeを
typed memory faultとして扱う。`D3DENUM_WHQL_LEVEL`、certificate discovery、adapter enumeration、
capability、device／renderer stateはこのevidenceから確定しない。Source executableのsize／SHA-256は
既存fingerprintのまま不変だった。

### 10.3 GetDeviceCaps output and consumer evidence

2026-07-19に同じprimary fingerprintへselected vtable captureを再実行し、trace
agent／summarizerが`D3DCAPS9`のcall前後304 bytes、全76 field offset／type、hash、caller stack
relation、256-instruction consumerを検証した。x86 layoutはlocal clang 22の
`-fdump-record-layouts-complete`でもsize `304`、alignment `4`、末尾field offset `300`として独立に
確認した。最終evidenceはe／fで、raw traceとsummaryは`.work/windows-traces/`に置きGitへ含めない。

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e.jsonl `
  --seconds 5 `
  --kill-after `
  --vtable idirect3d9

node tools/summarize-windows-trace.mjs `
  --input .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e.jsonl `
  --output .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e-summary.json
```

| Field | Primary e | Confirmation f |
|---|---:|---:|
| Raw bytes／lines | `2,274,737`／`7,066` | `2,282,110`／`7,068` |
| Raw SHA-256 | `69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361` | `56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f` |
| Balanced API pairs | `3,495` | `3,496` |
| Diagnostics | `0` | `0` |
| Successful `GetDeviceCaps` calls | `1` | `1` |

両captureはcaller RVA `0x5B949`、Adapter `0`、DeviceType `1`、HRESULT `0`で一致した。Call前の
stack-local bytes／changed-byte countはrunごとに異なるが、call後304 bytesは同じSHA-256
`5c48557292c6b254d6e9eb9612e3b24aae72f183bcbbcaf803d7a5ac16b41803`になった。Output pointerは
callee return後のcaller ESPと同一である。Success pathはHRESULT signを検査し、同じstack baseから
`Caps` offset `8`、`Caps2` offset `12`、`PixelShaderVersion` offset `204`だけを読む。Native値は順に
`0x00020000`、`0xe0010000`、`0xffff0300`だった。他のcaps fieldはこのcallerで消費されない。

これはprimary pathが要求するcapability semanticsのoracleであり、capture hostの全GPU capabilityを
guestへ複製するgoldenではない。Vineは`D3DCAPS_READ_SCANLINE`、
`D3DCAPS2_DYNAMICTEXTURES`、PS 3.0だけをversioned virtual policyへ採用し、残りfieldをzeroにする。
このroot queryはdevice／renderer stateを生成せず、shader translationの実装完了も意味しない。
Invalid adapter／device type／NULL、unwritable range、`CreateDevice`、device loss、resource／draw／Present
semanticsはmodel testまたは後続gatewayで別に判定する。Source executableのsize／SHA-256は既存
fingerprintのまま不変だった。

### 10.4 CheckDeviceType predicates and caller evidence

`GetDeviceCaps`の最終e／f captureには後続`CheckDeviceType`も完全なcall ID／parent ID付きで含まれる。
2026-07-19にsummarizerへ5つのtyped arguments、raw／normalized Windowed値、HRESULTを追加し、同じraw
artifactからsummaryを再生成した。Raw identityは10.3と同一であり、新しいcaptureを別のoracleとして
水増ししない。

```powershell
node tools/summarize-windows-trace.mjs `
  --input .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e.jsonl `
  --output .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e-summary.json `
  --force

llvm-objdump -d --x86-asm-syntax=intel `
  --start-address=0x45b9af --stop-address=0x45ba70 `
  "V:\softs\サクラノ詩\BGI.exe"
```

両traceは5 callsすべてAdapter `0`、DeviceType `1`、AdapterFormat／BackBufferFormat `22`、HRESULT
`0`で一致した。BGI direct callsはRVA `0x5B9D6`のWindowed `1`とRVA `0x5BA1D`のWindowed `0`で、
parent callはNULLである。残り3 callsはWindowed `1`で、`d3d9.dll`内部RVAから`CreateDevice` parentへ
帰属する。Immutable PEのdisassemblyと既存device-caps consumer snapshotはいずれも、各direct return
直後の`test eax,eax`／`jns`を示す。最初が成功するとfull-screen queryへ進み、二つ目が成功すると
`CheckDeviceFormat`へ進む。

MicrosoftのD3D9 contractでは、adapter範囲外または存在しないdevice typeは`D3DERR_INVALIDCALL`、
requested back-buffer format／hardware acceleration band不成立は`D3DERR_NOTAVAILABLE`である。Native
primary callsとpublic PE self-oracleは到達したformat `22→22`のwindowed／full-screenを`S_OK`として
固定し、Rust model testはこのfailure分類とnonzero `BOOL`正規化を固定する。このevidenceは一般format
conversion、別adapter／device、`CheckDeviceFormat`、`CreateDevice`、renderer／shader成立を主張しない。
Source executableのsize／SHA-256は既存fingerprintのまま不変だった。

### 10.5 CheckDeviceFormat texture predicates and caller evidence

10.3／10.4と同じe／f raw captureから`CheckDeviceFormat`の6 typed arguments、call／parent
identity、HRESULTを抽出した。2026-07-19にsummarizer contractを追加して再生成したsummaryは、eが
`164,638` bytes／SHA-256
`27cecfe4c1b476da563ebc43abd331e0038af3bbeff04e86347c432e992b1027`、fが`164,636`
bytes／SHA-256 `a0ff5bf1b896e2510c20c7553bdf3b6893d63606a1c70ca7275d8f5b5278724a`である。
Raw trace identityは10.3と同じであり、別captureとして数えない。

```powershell
node tools/summarize-windows-trace.mjs `
  --input .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e.jsonl `
  --output .work\windows-traces\bgi-d3d9-device-caps-20260719-012335-e-summary.json `
  --force

llvm-objdump -d --x86-asm-syntax=intel `
  --start-address=0x45ba4d --stop-address=0x45bab5 `
  "V:\softs\サクラノ詩\BGI.exe"
```

両runは各13 callsで一致した。全callがAdapter `0`、DeviceType `1`、AdapterFormat `22`、
RType `3` (`D3DRTYPE_TEXTURE`)である。CheckFormat `22`はUsage `0`で6 calls、Usage `0x200`
(`D3DUSAGE_DYNAMIC`)で4 callsが`S_OK`となった。CheckFormat `118`／Usage `0`の3 callsは
`D3DERR_NOTAVAILABLE`で、いずれも`CreateDevice` parentに属する。BGI direct callはRVA `0x5BA69`
のdynamic format `22` 1 callだけで、残り9 non-parented callsは`d3d9.dll`内部callerである。

Immutable PEはdirect return直後に`test eax,eax`／`jns`を行う。失敗時は初期化を中断し、成功時は
`LoadLibraryA("d3dx9_43.dll")`へ進む。Microsoftの`CheckDeviceFormat` contractどおり、adapter範囲外／
unsupported device typeは`D3DERR_INVALIDCALL`、requested usageでformatが不適合なら
`D3DERR_NOTAVAILABLE`としてmodel testへ固定した。このevidenceは一般format conversion、別resource
type／usage、dynamic DLL load、D3DX effect、`CreateDevice`、renderer／shader成立を主張しない。Source
executableのsize／SHA-256は既存fingerprintのまま不変だった。

### 10.6 LoadLibraryA runtime-system module and D3DX export

The title directory does not contain `d3dx9_43.dll`. The x86 system artifact
used by native Windows is `SysWOW64\d3dx9_43.dll`, 1,998,168 bytes with SHA-256
`0b28546be22c71834501f7d7185ede5d79742457331c7ee09efc14490dd64f5f`.
PE inspection shows entry-point RVA `0x001BEC0D` and imports from
`msvcrt.dll`, `GDI32.dll`, `KERNEL32.dll`, and `ADVAPI32.dll`. Those properties
are feasibility evidence for rejecting host-image mapping: importing this file
would also require native dependency loading, entry-point／rollback semantics,
host ABI coupling, and distribution authority.

Native BGI calls `LoadLibraryA("d3dx9_43.dll")` and receives a non-NULL result
while preserving LastError `0`. It immediately stores that opaque HMODULE,
tests it for zero, and passes it with `"D3DXCreateEffect"` to
`GetProcAddress`. That lookup is also non-NULL and preserves LastError, after
which the function pointer is stored globally. Immutable PE disassembly fixes
the return RVAs at `0x0005BAB0` and `0x0005BAC5`; the pointer is not invoked in
the interval before the next reached User32 call. Independent failure probes
return LastError 87 for NULL／empty input and 126 for a missing bare DLL name.
Native pointer values are discarded.

Vine therefore supplies a callback-free logical `runtime-system-module` rather
than mapping the Windows binary. Policy predeclares only
`d3dx9_43.dll!D3DXCreateEffect`. Rust preflights that dynamic endpoint, then
atomically publishes module handle `0x4D010010`, graph module ID 16,
process-attach sequence 16, object ID 23, one runtime-service reference, and one
dynamic reference owned by thread 8192. These numbers identify this artifact;
the tested semantics are same-handle reuse, reference ownership, attached
state, and failure atomicity. The export resolves to logical endpoint ID 5／
`0x7F020040`; no D3DX implementation is claimed or invoked.

Node completes `LoadLibraryA`／`GetProcAddress` as calls 17,823／17,824 and stops
at instruction 11,806,582 on
`USER32.dll!GetClientRect(0x48010001,0x006FFDC4)`. It records 17,871
writeback batches／36,014 dirty pages／1,807,003 changed bytes, generation
34,523, final memory SHA-256
`6769882742b0dd41d61c577cb4e1c042ea987706e165b795d65323983f24deba`,
and a 207,128,649-byte report with SHA-256
`1dfe9dc9dc5e2b514fd6c7e36b2765ecbc16135ee66a5e5d5c972e79aa00e09a`.
Chrome reproduces the same calls／stop／memory digest with 40,270 slices,
40,264 writebacks／87,871 pages, generation 56,917, 74,701 Wasm crossings,
and clean cancel／watchdog teardown. Its 24,188,713-byte report SHA-256 is
`ca7ea29ad003856955c475fcc62aa1e5cfad9bb75eda93ce6c9234179c276616`.

Because module lifecycle and Worker crossings changed, a new unchanged-threshold
180-second workload ran 29 sessions with zero failures and passed all 17
versioned gates. Session p95 was 6,383.355 ms and slice p99 was 2.615 ms; the
930,534-byte report SHA-256 is
`a81b99df457675c7855cc86e3551371e89b5277e3767f53b0e7c53a19eb26063`.
General DLL search, path／Windows-932 variants, native or guest DLL mapping,
dependency loading, `DllMain`／TLS callbacks, unload, and D3DX effect semantics
remain unsupported.

## 11. W7 PE32 load-plan ingress

Rust `vine-core`は、owned immutable bytesとimport-time SHA-256からversion 1のchecked header／section／import／base-relocation planを作る。representative `abi-smoke.exe` contractは4 sections、1 module／2 named imports、1 relocation block／6 wordsを検査し、BGI.exeには同じboundaryを次のcommandで適用した。

```powershell
npm run check:toolchain
npm run check:runtime:pe
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --output .work\pe-loader\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

BGI planは5 sections、14 modules／330 imports、237 relocation blocks／18,308 wordsを返した。named importは329、ordinal importは`DSOUND.dll` ordinal 11の1件、relocationは109 `ABSOLUTE`／18,199 `HIGHLOW`である。2回のlocal reportは1,976,261 bytes、SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be`で一致し、元BGI.exeのfingerprintも不変だった。

first stopは`process-bootstrap`／`peb-teb-process-parameters-unimplemented`である。guest memory materialization、IAT binding、PEB／TEB／process parameters、initial stack、entry executionはnot attemptedであり、silent import successを返さない。詳細なlimits、fixture、section plan、non-claimsは[pe-loader-ingress.md](pe-loader-ingress.md)を正とする。

このowned planをADR-003のregion／page owner、bounded allocator、execute gateへ接続する次checkpointは完了した。W7 report自体のstopとnon-claimsは変更しない。

## 12. Production guest-memory ingress

Rust `vine-core`は、4 GiB guest VAを4 KiB owner／region／page metadataとしてsparseに所有し、committed pageだけをbounded zeroed frameへ割り当てる。representative contractは次で再現する。

```powershell
npm run check:toolchain
npm run check:runtime:memory-core
```

fixed `high-memory.exe`はbase `0xF1000000`の6 image pagesを準備する。single sequenceはpending delta中のaccess拒否、unsealed execute PFEC `21`、seal後のRX fetch、NX execute PFEC `21`、R write PFEC `7`、RW data delta、second-page unmapped write PFEC `6`／no partial write、protect、7-page exact quota、decommit／frame reuse、release／owner teardownを検査する。final stateは1 sealed image owner／6 physical pages、generation `13`、acknowledged、dirty 0である。

fingerprint済みBGI.exeには次を適用した。

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 380 `
  --output .work\guest-memory\bgi-layout.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

BGI image rangeは`0x00400000`～`0x0057C000`、380 reserved／committed／physical pagesで、380 readable、100 writable、208 executable pagesとなった。ownerは未seal、generation `2`のreserve／commit deltaとdata TLB／fetch／generated-code effectsがpendingである。version 1 reportは1,831 bytes、SHA-256 `c46e40fdeb0301de1d852b4532bc0be0c4033bc5fd5f79479ab695663f7a119e`で一致した。379-page quotaはexit `1`でoutputを作らず、default W7 reportは1,976,261 bytes／SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be`のまま変わらない。元BGI.exeも1,193,472 bytes／primary SHA-256を保持した。

structured stopは`image-materialization`／`image-bytes-relocations-import-binding-unimplemented`である。pagesはallocator初期化のzeroだけを持ち、header／section source copy、PE zero-fill verification、`HIGHLOW`、IAT binding、BGI owner seal、v86 page table／TLB／generated-code invalidation、guest executionを行わない。詳細は[guest-memory-ingress.md](guest-memory-ingress.md)を正とする。

同じowned source／planからheader／section bytesをcopyし、zero-fillを検証して`HIGHLOW`を適用する次checkpointは完了した。memory-ingress report自体のstopとnon-claimsは変更しない。

## 13. Production PE32 image materialization

`npm run check:runtime:materialization`はfixed `abi-smoke.exe`をpreferred `0x00400000`から`0x10000000`へrelocateする。4 source ranges／2,560 bytesをcopyし、5 mapped tails／17,920 bytesをexplicit zero-fillして全byteをread backする。5 `HIGHLOW` target、2 original IAT slots、entry `0x10001000`を検査し、20,480 mapped bytesのSHA-256 `1cfa1ba4ef560433b38e2598ba291ae6cbd0f8139408274c2192c30e4a185110`を返した。first stopは`vine_fixture.dll!fixture_add`の`import-binding`／`iat-binding-unimplemented`である。

fingerprint済みBGI.exeには次を適用した。

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 380 `
  --materialize-image-base 0x10000000 `
  --output .work\materialization\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

BGI imageは`[0x10000000, 0x1017C000)`、entry `0x100A383E`となった。6 ranges／1,193,472 source bytes、5 ranges／363,008 zero bytes、18,199 `HIGHLOW`、330 original IAT slotsを検証した。mapped 1,556,480 bytesのSHA-256は`d9dc33d121ea2da6ba5e459a9da13b70bf8bfc1c668593a80c6b3689d841199b`である。first unbound importはIAT `0x100D14F4`の`d3d9.dll!Direct3DCreate9`だった。

fixture／BGI reportを各2回生成し、それぞれ2,436 bytes／SHA-256 `5ad5e461d0de7c40578239ee875fc84eed69ccc88af50c266706b75e4b552c68`、2,481 bytes／SHA-256 `862a57027a5e009ecef86f701c6edf13b173fb242dea4df3c629ebe0575426ee`で一致した。元BGI.exeは1,193,472 bytes／primary SHA-256を保持した。

ownerは未seal、generation `2` deltaはpendingである。IAT write、runtime-owned thunk、DLL／export resolution、discardable unmap、v86 page table／invalidation、PEB／TEB／stack、guest executionはnot attemptedである。詳細は[pe-materialization.md](pe-materialization.md)を正とする。controlled fixtureの2 importをproduction thunkへbindする後続checkpointは次節で完了したが、このmaterialization report自体のstopとnon-claimsは変更しない。

## 14. Production PE32 import binding

`npm run check:runtime:imports`はmaterialized `abi-smoke.exe`の2 named importsを明示的runtime export catalogへ照合し、`[0x7F000000, 0x7F010000)`の64 KiB arenaから16-byte strideのowned thunksを割り当てる。ID `1`／`2`は`0x7F000000`／`0x7F000010`、IATは`0x10002034`／`0x10002038`である。physical quotaは5 image pages＋1 thunk page、committed arenaは先に`HLT`で埋め、owned slot先頭だけを`UD2`へ変える。runtime／image ownerをsealし、actual adapter acknowledgement前のgeneration `6` invalidationを保持する。

acknowledgement後は両IAT pointerと`UD2`／`HLT` bytesをnormal guest accessでread／fetchし、未所有slotがall `HLT`、sealed IATへのwriteがPFEC `7`になることを検証した。arena SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`である。probe reportを2回生成し、4,483 bytes／SHA-256 `bdcc35c73320d5c628cd917b6ab0b0c9a2d2c3aefad7673ca7dc03e169a9b661`で一致した。stopは`process-bootstrap`／`peb-teb-process-parameters-unimplemented`である。

fingerprint済みBGI.exeにはnative traceで到達済みのfactoryだけを宣言した。

```powershell
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --prepare-memory-pages 381 `
  --materialize-image-base 0x10000000 `
  --bind-name d3d9.dll Direct3DCreate9 `
  --output .work\import-binding\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

BGI declared-only policyは全330 importsをread-only planし、`Direct3DCreate9`をstable ID `1`／`0x7F000000`へ割り当てる一方、残る329 identitiesを全て報告する。未解決があるためarenaは作らず、IATは全てoriginal value、image ownerはunsealed、generationはmaterialization時の`2`のままである。first unresolvedは`DINPUT8.dll!DirectInput8Create`／IAT `0x100D1050`、lastは`DSOUND.dll` ordinal `11`で、stopは`import-resolution`／`import-symbols-unimplemented`である。2 reportは86,073 bytes／SHA-256 `6887944898fc8cecb132d63774573e6823e4ade25aba8aee6de98b31d4b6a69f`で一致し、catalog追加後のsame-image retryも成功した。元BGI.exeは1,193,472 bytes／primary SHA-256を保持した。

このcheckpointはtrap address ownershipとIAT mutationを検証するだけで、`Direct3DCreate9`のservice behavior、general DLL／export／forwarder resolution、PEB／TEB／production stack、BGI entry executionを実装しない。詳細は[pe-import-binding.md](pe-import-binding.md)を正とする。controlled imageをv86へ反映する後続checkpointは次節で完了したが、このbinding report自体のstopとnon-claimsは変更しない。

## 15. Production-memory v86 entry ingress

`npm run check:runtime:entry`はsource-built v86と`vine-pe-probe --v86-reflection-session`を接続する。probeは同じlive `GuestMemory`を保持したまま、sealed image owner 1の5 pagesとruntime owner 2の1 thunk pageをascending VAでemitする。reflection SHA-256は`3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e`で、Rust physical frame ID、host offset、v86 physical addressはprotocolへ出さない。

Node adapterは2 MiB poolへpage directory 1、directory index `64`／`508`／`1`のpage table 3、reflected page 6、harness-only stack 1の計11 pages／44 KiBを配置した。CPU stateはversion 1／640-byte ABIで復元し、restore内`full_clear_tlb`、explicit `full_clear_tlb`、`jit_clear_cache_js`を完了した後にだけgeneration `6`のackを返す。probeは`acknowledge_delta(6)`後、pendingがNULL、dirty pagesが0であることとIAT／entry／thunkのnormal read／fetchを検証してreceiptを返した。

stackは`[0x006FF000, 0x00700000)`の1 page、initial ESP `0x006FFFFC`、entry sentinel `0xDEADC0DE`である。entry `0x10001000`をinstruction budget `3`で実行し、2 execute stop／1 budget stop後にstructured `#UD` reason `0x106`／error `-1`を得た。EIP／previous EIPはthunk ID `1`の`0x7F000000`、ESP `0x006FFFF0`、return `0x1000100A`、arguments `[19,23]`、IAT `0x10002034 -> 0x7F000000`である。

version 1 reportは4,831 bytes／SHA-256 `c254b3be79eccbfd944dbae3d00376e9db81597eaccdec3488670408611a817c`で一致した。これはcontrolled production page ownership、actual invalidation order、bounded execution、exact trap identityのevidenceである。one-page stack／flat CPL3／physical placementはharness-onlyで、service dispatch、callback、PEB／TEB、guard／growth、snapshot、general dirty-page synchronization、BGI executionを実装しない。詳細は[v86-entry-ingress.md](v86-entry-ingress.md)を正とする。fixed cdecl continuationは次節で完了したが、このentry-only reportとstopは変更しない。

## 16. Production cdecl service ingress

`npm run check:runtime:cdecl`は同じreflection／acknowledgement sessionを使い、Rust offerにplan順のowned thunk metadataを加える。first thunk ID `1`／`0x7F000000`でframe `[return 0x1000100A, 19, 23]`をchecked readし、fixed `fixture_add` result `42`をversion 1 CPU state ABIへ書く。calleeの`RET`相当としてESPを`0x006FFFF0`から`0x006FFFF4`へ進め、EIP／PREVIOUS_EIPをguest return `0x1000100A`へ揃える。

guestはcdecl arguments 2件をcleanupし、IAT `0x10002038`経由でsecond thunk ID `2`／`0x7F000010`へ入る。全体は4 execute stop／2 budget stopで、second frameはreturn `0x10001019`、callback target `0x10001030`、value `42`、EAX `42`、ESP `0x006FFFF0`である。callback targetはsealed executable reflected pageに属し、両trapはaddress rangeではなくexact `ImportThunk` recordで分類する。EBX／EBP／ESI／EDI sentinelは保持され、Wasm host importは`microtick` 1回だけである。

version 1 reportは5,501 bytes／SHA-256 `199923cb0e0b03e0fdc042db095a22f3ef697d78010834280dca1eb2b8116c7f`で一致し、entry-only modeの4,831-byte reportも不変である。このcheckpointはone fixed two-argument cdecl serviceだけを扱い、general ABI metadata、LastError、callback frame／return sentinel、callback execution、outer service return、SEH、production stack／bootstrap、BGI executionを実装しない。詳細は[v86-cdecl-ingress.md](v86-cdecl-ingress.md)を正とする。callback return continuationは次節で完了したが、このcdecl-only reportとstopは変更しない。

## 17. Production callback return ingress

`npm run check:runtime:callback`はreflection probeへcallback-control installationを明示し、既存import arenaの直後にseparate runtime owner `3`の`[0x7F010000,0x7F020000)`をreserveする。先頭1 pageをRXでcommitし、sentinel ID `1`／`0x7F010000`へ`UD2`、残りへ`HLT`を置いてsealする。page SHA-256は`4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059`である。

callback modeのreflectionは7 production pages、generation `9`、content SHA-256 `45d980f7ab5aa3c98942777693438a9c780f5d6a4f0e86f2f8db63211a3af406`となる。Node adapterはpage directory 1、page table 3、reflected page 7、harness stack 1の計12 pages／48 KiBを2 MiB poolへ置く。actual state restore／TLB／generated-code clear後にgeneration `9`をacknowledgeし、Rust normal fetchとNode readbackの両方でsentinel bytesを検証した。optional fieldを省略するentry／cdecl modeのreport hashは不変である。

thunk ID `2`到達時のouter ESPは`0x006FFFF0`である。frame ID `1`はparent thunk ID `2`、outer return `0x10001019`、target `0x10001030`、value `42`を保持する。checked writable stackのESP `0x006FFFE8`へ`[0x7F010000,42]`を置き、targetを実行した。6 execute stop／3 budget stop後、EIP／previous EIP `0x7F010000`、EAX `43`、ESP `0x006FFFEC`でexact callback-return trapへ到達した。callee-saved registerは保持され、host importは`microtick` 1回である。

version 1 reportは7,028 bytes／SHA-256 `29ede77d61369e560197dedb35311e662e0d3c56a3c6ce5dadc9743c02c10771`で一致した。control range／owner／IDはcheckpoint-localで、adapter stack mutationはRust memoryへ戻らない。frame ID `1`はactiveのままで、outer service return、nested callback、reentrancy、exception／SEH、production stack／bootstrap、BGI executionを実装しない。詳細は[v86-callback-return-ingress.md](v86-callback-return-ingress.md)を正とする。outer continuationは次節で完了したが、このcallback-only reportとstopは変更しない。

## 18. Production outer return ingress

`npm run check:runtime:outer-return`はcallback-return checkpointと同じgeneration `9`／7 production pagesを使い、exact sentinel stateからframe ID `1`をretireする。outer thunk ID `2`のfixed cdecl returnとしてEAX `43`、ESP `0x006FFFF4`、EIP／previous EIP `0x10001019`を復元した。callee `RET`相当はouter return addressだけをconsumeし、2 argumentsはguest callerに残す。

guestは`ADD ESP,8`でargumentsをcleanupし、`0x10003000`へ`43`を書いた。続くcomparisonはentry EAX `0`を作り、final `RET`はinitial stackのharness-only `0xDEADC0DE`をconsumeしてESP `0x00700000`へ進めた。このaddressはunmappedなので、合計9 execute stop／5 budget stop後にreason `0x10E`、PFEC `4`、CR2／EIP／previous EIP `0xDEADC0DE`のstructured `#PF`となった。callee-saved registerは保持され、host importは`microtick` 1回である。

version 1 reportは8,124 bytes／SHA-256 `de6ce28c1aa6f990fe211abd81c71415a0c908fb7a47ba6651ad0aa355cdd841`で一致し、entry／cdecl／callback-only reportはbyte-identicalである。これはfixed outer cdecl unwind、guest result、entry result、stack balanceのevidenceであり、page faultをprocess exitとは扱わない。production-owned entry sentinel、general frame stack、nested／reentrant callback、exception／SEH、PEB／TEB、production stack／bootstrap、BGI executionは未実装である。詳細は[v86-outer-return-ingress.md](v86-outer-return-ingress.md)を正とする。exact entry completionは次節で完了したが、このouter-only fault reportは変更しない。

## 19. Production entry return ingress

`npm run check:runtime:entry-return`はcallback-control owner `3`をsealする前にslot `2`へprocess-entry return sentinel ID `2`／`0x7F010010`を追加する。slot `1`のcallback sentinelと同じtyped control pageを共有し、combined page SHA-256は`bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`、7 reflected pagesのcontent SHA-256は`b184b076d28abbf59401e35113aaeb30ca1f29b8cee590ff1d71897313bfb9db`である。generation `9`、7 production pages、12 adapter pages／48 KiBは変わらない。

initial ESP `0x006FFFFC`へprocess-entry sentinel addressを置いた。同じfixed service／callback／outer return flowを9 execute stop／5 budget stopで実行し、fixture result `0x10003000 = 43`、entry EAX `0`、ESP `0x00700000`、EIP／previous EIP `0x7F010010`でstructured `#UD`へ到達した。trapはcontrol rangeではなくtyped sentinel ID `2`のexact addressで分類する。outer-only modeは引き続きunmapped `0xDEADC0DE`をreason `0x10E`／PFEC `4`／CR2で停止し、unknown returnをsuccessにしない。

version 1 reportは8,674 bytes／SHA-256 `c5edbb527fad5fc0907d8784cd22df9d0c5a93bd26aa63fd413c0a6c5b75ce59`で一致し、先行4 reportはbyte-identicalである。これはfixed entry resultとstack balanceのevidenceであり、Windows process exitではない。initial stackはadapter-localで、process／thread lifecycle、PEB／TEB、process parameters、stack growth／guard、general callback stack、exception／SEH、BGI executionは未実装である。詳細は[v86-entry-return-ingress.md](v86-entry-return-ingress.md)を正とする。Rust-owned stack continuationは次節で完了したが、このadapter-stack reportは変更しない。

## 20. Production stack bootstrap ingress

`npm run check:runtime:stack-bootstrap`はanonymous owner `4`へstack region `[0x006FF000,0x00700000)`をreserveし、1 pageをRW commitする。initial ESP `0x006FFFFC`へprocess-entry sentinel address `0x7F010010`をchecked initializationし、page SHA-256 `3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079`でownerをsealする。stack commit／sealによりfinal invalidationはgeneration `12`となる。

reflectionは8 production pages／content SHA-256 `d7471bcc744e2f769861e1a40cc5520f5c0d452f2ea8539233a8573315b6f1cd`である。Rust statsは4 owners／regions／sealed owners、38 reserved pages、8 committed／physical／dirty pagesとなった。adapterはpage directory、directory index `1`／`64`／`508`の3 tables、8 reflected pagesの12 pages／48 KiBを使う。`harness-stack` allocationは0件で、owner `4` pageをRW／NXとしてmapし、top 16 bytesをnormal readで検証してstack write helperを呼ばない。

同じfixed flowは9 execute stop／5 budget stopでfixture result `43`、entry EAX `0`、ESP `0x00700000`、process-entry sentinel ID `2`へ到達した。version 1 reportは9,430 bytes／SHA-256 `a4e0fbb559f98e32218d5c49595ed47a113f6dd18b9f3bd4411e26efcc8592c2`で一致し、先行5 reportはbyte-identicalである。これはcontrolled stack ownershipのevidenceであり、PE stack reserve／commit一般、guard／growth、multi-thread、teardown、v86-originated writeback、PEB／TEB、process parameters、BGI executionを実装しない。詳細は[v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md)を正とする。FS baselineは次節で完了した。

## 21. Bootstrap FS native oracle and first stop

`bootstrap-fs.exe`は2,560 bytes／SHA-256 `5712377ff8a8bd2be292f8678902d19d3b4b2f1c1919922cce9d373f0696637c`のimport-free PE32 fixtureである。native Windowsでは`FS:[0x18]`のTEB self pointer、`FS:[0x30]`のPEB pointer、`TEB + 0x18`／`TEB + 0x30`の一致を自己検証し、exit code `0`となった。

`npm run check:runtime:bootstrap-fs-first-stop`はfixtureを`0x10000000`へrelocateし、5 image pages、control owner `2`、stack owner `3`をgeneration `9`の7 pagesとしてreflectする。content SHA-256は`bb476b06336e93654df562ce78054e0927d5b2527ac12b54241d84a1d50b0696`で、adapterはpage directory／3 tables／7 reflected pagesの11 pages／44 KiBを使う。import thunkとadapter-local stackはない。

version 1 CPU stateのFS selector／baseを`0`／`0`としてentry `0x10001000`をbudget `3`で実行すると、最初の`mov edx, fs:[0x18]`は1 execute stopでreason `0x10E`、PFEC `4`、CR2 `0x18`となった。current EIPはdecoded next instruction `0x10001007`、previous EIPはfaulting `0x10001000`、ESPは`0x006FFFFC`のままである。5,982-byte reportのSHA-256は`0276cc927a3d870b10a632cf95995e79ec96cd8af5558e964b7635d2d2b7941b`で2回一致した。

これはnative offset oracleとTEB未実装時のexact boundaryであり、guest TEB／PEB page、nonzero FS base、process parameters、loader list、TLS、LastError、process lifecycleを実装しない。詳細は[v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md)を正とする。Rust-owned continuationは次節で完了した。

## 22. Bootstrap FS ingress

`npm run check:runtime:bootstrap-fs`は`thread-environment-block` owner `4`の`0x7FFDE000` pageと`process-environment-block` owner `5`の`0x7FFDF000` pageをRW／NXでcommitする。両pageを明示zero-fillし、TEB `+0x18`へself、`+0x30`へPEB pointerを書いてsealする。TEB／PEB page SHA-256は`5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e`／`ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7`である。

generation `15`の9-page reflectionはSHA-256 `14a986f7f10229da07f54d11fd5eda29be02e0639f06aba6151307e35f0206f3`となった。Rust statsは5 owners／regions、24 reserved pages、9 committed／physical／dirty pagesで、adapterはpage directory／4 tables／9 reflected pagesの14 pages／56 KiBを使う。CPU state ABIはFS selector `0`を保持してbaseだけを`0x7FFDE000`へrestoreする。

同じfixtureは9 execute stop／8 budget stop後にprocess-entry sentinel ID `2`／`0x7F010010`へEAX `0`、ESP `0x00700000`で到達した。`FS:[0x18]`、`FS:[0x30]`、`*(TEB+0x18)`、`*(TEB+0x30)`は順にTEB、PEB、TEB、PEB addressと一致する。8,214-byte reportのSHA-256は`3567ff39474f72781010fdb810b793f58424483a7663b89f2cea4b73a28748b9`で2回一致し、first-stop reportもbyte-identicalである。

これはminimum field ownershipのevidenceであり、fixed addresses、selector `0`、empty PEBをgeneral Windows layoutとしない。process parameters、loader lists、TLS、LastError、stack bounds、multi-thread lifecycle、mutation writebackは未実装である。詳細は[v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md)を正とする。native process-parameters oracleは次節で完了した。

## 23. Process-parameters native oracle and first stop

`npm run check:runtime:bootstrap-params-first-stop`は2,560-byte／SHA-256 `50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40`のseparate import-free fixtureを使う。native Windowsは`FS:[0x30]`からPEB、`PEB+0x10`から`RTL_USER_PROCESS_PARAMETERS`、`ProcessParameters+0x40`からcommand-line `UNICODE_STRING`、その`+0x04`からUTF-16 bufferを読み、32 KiB以内で`bootstrap-params.exe`をcase-insensitive ASCII basename matchしてexit code `0`となった。

同じfixtureをexisting Rust-owned minimum TEB／PEBへ反映すると、PEB DWORD `+0x10`はexact zeroである。fixtureはPEBとzero pointerを`.data`へ記録し、logical result EAX `2`を返して8 execute stop／7 budget stop後にprocess-entry sentinel ID `2`／`0x7F010010`へESP `0x00700000`で到達した。generation `15`の9-page reflection SHA-256は`51357b546920ec78aa535488ce8d280a8027805e1d1d24e138b85264fbab9c39`、8,351-byte reportのSHA-256は`b1c05c3e9df1529196276c8905f3b97aee306610c870461d458a857c80aadde5`である。zero-FS first-stopとminimum-environment reportsもbyte-identicalだった。

これはmeasured i386 layoutとfirst unsupported pointのevidenceである。Vineはまだprocess-parameters region、UTF-16 command-line buffer、current directory、environment、image path、loader list、TLS、LastError、general process lifecycleを所有しない。詳細は[v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md)を正とする。次は測定済みcommand-line fieldだけをRust ownershipへ移す。

## 24. Process-parameters ingress

`npm run check:runtime:bootstrap-params`は`process-parameters` owner `6`／`0x7FFDC000`と`command-line` owner `7`／`0x7FFDD000`をRW／NXでcommitする。PEB `+0x10`へparameters pointer、parameters `+0x40`へLength `40`／MaximumLength `42`／Buffer `0x7FFDD000`、command-line pageへ20 UTF-16 unitsの`bootstrap-params.exe`とNULを初期化する。PEB／parameters／command-line page SHA-256は`780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39`／`5fb4eed3eb10240c39a428c43621e7198ccf0c3089ffff6e6aea233759533f75`／`f6e0abfe79cb7fafa42be5f1450e91539b3d0386f7140523e44109249f1e19bd`である。

generation `21`の11-page reflectionはSHA-256 `f9a59953a06999030e40e962a007f1ec5f0ecb73b077c4df760e212ba3d3acbb`となった。Rust statsは7 owners／regions、26 reserved pages、11 committed／physical／dirty pagesで、adapterはpage directory／4 tables／11 reflected pagesの16 pages／64 KiBを使う。ack後にPEB 64 bytes、parameters 72 bytes、command line 42 bytesをnormal readで一致させた。

同じfixtureは94 execute stop／93 budget stop後にprocess-entry sentinel ID `2`／`0x7F010010`へEAX `0`、ESP `0x00700000`で到達した。observationsはPEB `0x7FFDF000`、parameters `0x7FFDC000`、Length／MaximumLength `40`／`42`、Buffer `0x7FFDD000`、match index `0`である。10,521-byte reportのSHA-256は`c1e76294d4bba6760cbc8d022e022ef66b557d370b567d9a498798b8b2f7ac66`で2回一致し、先行zero-FS／minimum-environment／zero-parameters reportsもbyte-identicalである。

これは測定済みcommand-line pathだけのownership evidenceである。fixed addresses／textとzero-filled unmeasured bytesをgeneral `RTL_USER_PROCESS_PARAMETERS`、BGI bootstrap、loader list、TLS、LastError、process lifecycleの代用にはしない。詳細は[v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md)を正とする。explicit trap continuationは次節で完了した。

## 25. Explicit unsupported import trap binding

optional `--trap-unsupported-imports` policyはcatalog-missing identityへ`unsupported-trap` dispositionを付け、同じread-only planとatomic commitでowned `UD2` slotsへbindする。controlled fixtureでは`fixture_add`だけをdeclared endpointとし、`fixture_call_callback`をID `2`／IAT `0x10002038`のunsupported trapへbindした。両ownerはgeneration `6`でsealされ、ack後のIAT／trap bytesと`thunk-dispatch`／`unsupported-import-trap-dispatch-pending` stopを検証した。declared-only reportは4,483 bytes／SHA-256 `bdcc35c73320d5c628cd917b6ab0b0c9a2d2c3aefad7673ca7dc03e169a9b661`、controlled trap reportは4,487 bytes／SHA-256 `298797b19f15d4a1a95c048e24820b1af9fd1aaedd7833f0fa1560a46cc429bf`である。

BGI replayは382 physical pagesを使い、1 declared `d3d9.dll!Direct3DCreate9`と329 unsupported identitiesを全330 IAT slotsへbindした。ID `1`は`0x7F000000`／IAT `0x100D14F4`、first unsupported ID `2`は`DINPUT8.dll!DirectInput8Create`／`0x7F000010`／IAT `0x100D1050`、last ID `330`は`DSOUND.dll` ordinal `11`／`0x7F001490`／IAT `0x100D1058`である。2-page arena SHA-256は`88f1b31a1ed2f724903f9ddaf16c789240b2010077537b9203faa44a63c20dde`、image／runtime ownerはgeneration `6`でsealされた。

2回のBGI reportは126,633 bytes／SHA-256 `7f9d96c502ae1b5cacd27da7a71cad87ada56a9fcf31ea9bc68b73bdf6ff54dc`で一致し、sourceは1,193,472 bytes／primary SHA-256のままだった。これはstatic identityとowner sealのevidenceであり、このreport自体はAPI実装、return、DLL lifecycle、runtime reachability、BGI entry executionを示さない。詳細は[pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md)を正とする。bounded execution continuationは次節で完了した。

## 26. BGI bounded reflection and first runtime trap

`npm run check:runtime:bgi -- --checkpoint bgi-first-trap`はexisting reflection schemaをopt-in上限400 pagesへ拡張し、dedicated runnerで8 MiB protocol-line上限を適用する。BGI image 380 pages、import arena 2、control 1、stack 1、TEB／PEB 2、process parameters／command line 2の合計388 pagesをgeneration `24`でreflectした。4,904,602-byte offerのcontent SHA-256は`f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`である。

2 MiB adapter poolではphysical VGA MMIO hole `[0xA0000,0xC0000)`を除外した。directory 1、page tables 4、reflected pages 388の393 allocationsとPDE indices `[1,64,508,511]`を使い、MMIO callbackから値を返さずに配置した。actual TLB／fetch／generated-code invalidation後だけgeneration `24`をacknowledgeした。

entry `0x100A383E`はinstruction budget `1`で各logical execute boundaryを検査し、19 execute stop／18 budget stop／19命令でexact owned thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`／`0x7F000670`へ到達した。IATは`0x100D1248`、ESPは`0x006FFFD0`、returnは`0x100A88D2`、raw stack wordsは`[0x100A88D2,0x006FFFEC,0,0,0,0,0,0]`である。API dispatch／write／returnは行わない。9,592-byte reportのSHA-256は`4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840`で2回一致し、source hashは前後不変だった。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

## 27. BGI first supported Win32 call return

`npm run check:runtime:bgi -- --checkpoint bgi-first-win32-call`はID `104`を2番目のdeclared endpointとし、`GetSystemTimeAsFileTime`のcall site `0x100A88CC`、IAT `0x100D1248`、one-argument stdcall frame `[0x100A88D2,0x006FFFEC]`をchecked処理する。host clock owner `checkpoint-host-clock`はUTC `2026-07-11T00:00:00Z`をWindows ticks `134282016000000000`／bytes `[00,40,67,37,C8,10,DD,01]`として供給する。

outputがv86-mutated stack page上にあるため、current page `0x006FF000`／owner `4`をgeneration `24`と照合してRustへreconcileした。initial pageとの差は11 bytes、page SHA-256は`46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785`である。続く8-byte writeはgeneration `25`、dirty page 1／4 KiB、mapping operations 0、cache effects 0、delta SHA-256 `dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840`となった。full dirty pageがcurrent v86 page＋exact outputだけであることを検査し、live mapping適用／Rust acknowledgement後はdirty pages 0である。

stdcall returnはESPを`0x006FFFD0`から`0x006FFFD8`へ進め、VOID APIのcheckpoint policyとしてEAXを保持した。4命令後、entryから累計23命令でexact thunk ID `103`／`KERNEL32.dll!GetCurrentProcessId`／`0x7F000660`へ到達した。IATは`0x100D1244`、ESPは`0x006FFFD4`、returnは`0x100A88DE`である。14,540-byte version 1 reportのSHA-256は`7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16`、source hashは前後不変だった。これはone fixed clock sample／page reconciliation／API returnであり、general clock、dirty tracking、process identity、ABI gatewayを実装しない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

## 28. BGI second supported Win32 call return

`npm run check:runtime:bgi -- --checkpoint bgi-second-win32-call`はID `103`を3番目のdeclared endpointとし、`GetCurrentProcessId`のcall site `0x100A88D8`、IAT `0x100D1244`、zero-argument stdcall frame `[0x100A88DE]`をadapterとRustの両方で検査する。process identity owner `checkpoint-process-identity`はhost PIDに依存しないcontrolled PID `4096`を供給する。

guest call frameの4 changed bytesを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`0x006FFFEC`から`4096`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

3命令後、entryから累計26命令でexact thunk ID `149`／`KERNEL32.dll!GetCurrentThreadId`／`0x7F000940`へ到達した。IATは`0x100D12FC`、ESPは`0x006FFFD4`、returnは`0x100A88E6`である。18,472-byte reportのSHA-256は`bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67`で、source hashは不変だった。これはone controlled PID／page reconciliation／API returnであり、general process／handle／thread modelを実装しない。詳細は[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md)を正とする。

## 29. BGI third supported Win32 call return

`npm run check:runtime:bgi -- --checkpoint bgi-third-win32-call`はID `149`を4番目のdeclared endpointとし、`GetCurrentThreadId`のcall site `0x100A88E0`、IAT `0x100D12FC`、zero-argument stdcall frame `[0x100A88E6]`をadapterとRustの両方で検査する。thread identity owner `checkpoint-thread-identity`はhost TIDに依存しないcontrolled TID `8192`を供給する。

guest call frameの1 changed byteを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`4096`から`8192`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

3命令後、entryから累計29命令でexact thunk ID `13`／`KERNEL32.dll!GetTickCount`／`0x7F0000C0`へ到達した。IATは`0x100D10DC`、ESPは`0x006FFFD4`、returnは`0x100A88EE`である。22,383-byte reportのSHA-256は`d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa`で、source hashは不変だった。これはone controlled TID／page reconciliation／API returnであり、general process／thread／TEB client-ID／scheduler modelを実装しない。詳細は[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md)を正とする。

## 30. BGI fourth supported Win32 call return

`npm run check:runtime:bgi -- --checkpoint bgi-fourth-win32-call`はID `13`を5番目のdeclared endpointとし、`GetTickCount`のcall site `0x100A88E8`、IAT `0x100D10DC`、zero-argument stdcall frame `[0x100A88EE]`をadapterとRustの両方で検査する。monotonic clock owner `checkpoint-monotonic-clock`はhost uptimeに依存しないsample ID `uptime-ms-3600000`／tick count `3,600,000`を既存fixed wall-clock sampleとpairする。

guest call frameの1 changed byteを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`8192`から`3,600,000`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

5命令後、entryから累計34命令でexact thunk ID `157`／`KERNEL32.dll!QueryPerformanceCounter`／`0x7F0009C0`へ到達した。IATは`0x100D131C`、ESPは`0x006FFFD0`、returnは`0x100A88FA`、output pointerは`0x006FFFE4`である。26,558-byte reportのSHA-256は`79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111`で、source hashは不変だった。これはone controlled tick-count sample／page reconciliation／API returnであり、general clock／timer／scheduler／snapshot modelを実装しない。詳細は[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

## 31. BGI fifth supported Win32 call return

`npm run check:runtime:bgi -- --checkpoint bgi-fifth-win32-call`はID `157`を6番目のdeclared endpointとし、`QueryPerformanceCounter`のcall site `0x100A88F4`、IAT `0x100D131C`、frame `[0x100A88FA,0x006FFFE4]`をadapterとRustの両方で検査する。同じmonotonic ownerのsample ID `qpc-ticks-12345678901`は64-bit counter `12,345,678,901`をtick sampleとpairするが、frequency／originを確定しない。

guest call frameを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。5 changed bytes、page SHA-256 `a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc`を保持し、zeroだったoutputへbytes `[35,1C,DC,DF,02,00,00,00]`を書いた。generation `26` deltaはdirty page 1／4 KiB、mapping／cache effects 0、SHA-256 `e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951`である。ack後にEAX `1`、ESP `0x006FFFD8`でreturnした。

44命令後、entryから累計78命令でexact thunk ID `83`／`KERNEL32.dll!GetStartupInfoW`／`0x7F000520`へ到達した。IATは`0x100D11F4`、ESPは`0x006FFF78`、returnは`0x100A36E7`、output pointerは`0x006FFF90`である。31,427-byte reportのSHA-256は`eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1`で、source hashは不変だった。これはone controlled counter／page reconciliation／delta／API returnであり、general clock、frequency、timer、scheduler、snapshotを実装しない。詳細は[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md)を正とする。

## 32. BGI sixth supported Win32 call return

opt-in `--observe startup-info` traceを2回取得し、target callがBGI caller RVA `0xA36E7`へreturnすること、x86 output sizeが68 bytesであること、未初期化inputが全書込されることを確認した。native outputは`cb=68`、empty reserved／desktop strings、absolute image-path title、flags `0x100`、3 nonzero inherited standard handlesを持つ。pointer／handle valuesはlaunchごとに変わるため、runtimeのcontrolled stateへコピーしない。

`npm run check:runtime:bgi -- --checkpoint bgi-sixth-win32-call`はID `83`を7番目のdeclared endpointとし、call／return `0x100A36E1`／`0x100A36E7`、IAT `0x100D11F4`、frame `[0x100A36E7,0x006FFF90]`をadapterとRustで検査する。owner `checkpoint-process-startup`のprofile `startupinfow-v68-zero-optional-fields`は`cb=68`だけを定義し、残る64 bytesをzeroにする。generation `26` stack pageとの差は31 bytes、page SHA-256は`75e2211e3e9a79f92313319d9897bb1e868d74bb15f0dc513cc9cb393c9bb843`である。

68-byte writeはgeneration `27`、dirty 1 page／4 KiB、mapping／cache effects 0、delta SHA-256 `a2c82c133e41916b0b9af10314376ece9a281a8503db4b811031e5ce52919de8`となった。ack後はVOID policyでEAX `0x006FFF90`を保持し、ESP `0x006FFF80`へreturnする。9命令後、entryから累計87命令でexact ID `84`／`KERNEL32.dll!HeapSetInformation`／`0x7F000530`へ到達した。frameは`[return,NULL,1,NULL,0]`、39,031-byte reportのSHA-256は`208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c`で、source hashと先行report hashesは不変だった。これはone controlled startup-info profileであり、general process creation、desktop、title path、console／standard handlesを実装しない。詳細は[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md)を正とする。

## 33. BGI seventh supported Win32 call return

opt-in `--observe heap-set-information` traceを2回取得し、target callがBGI caller RVA `0xA36FC`へreturnすること、argumentsが`[NULL,1,NULL,0]`、BOOL resultが`1`であることを確認した。LastErrorはenter／leaveとも`87`で、成功時に変更されないstale thread stateだった。

`npm run check:runtime:bgi -- --checkpoint bgi-seventh-win32-call`はID `84`を8番目のdeclared endpointとし、call／return `0x100A36F6`／`0x100A36FC`、IAT `0x100D11F8`、frame `[return,NULL,1,NULL,0]`をadapterとRustで検査する。owner `checkpoint-process-heap-policy`のpolicy `heap-enable-termination-on-corruption`だけを受け付け、generation `27` stack pageとの差12 bytes／SHA-256 `d121a898c549de8cf2eea5d76912375a993cac9a7dfb41d101ef2eb2b68fa3da`をreconcileする。guest writeはなくgeneration `27`／dirty 0を保持し、LastErrorを変更せずBOOL `1`、ESP `0x006FFF80`でreturnする。

21命令後、entryから累計108命令でexact ID `102`／`KERNEL32.dll!HeapCreate`／`0x7F000650`へ到達した。frameは`[return,0,0x1000,0]`、43,980-byte reportのSHA-256は`09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f`で、source hashと先行report hashesは不変だった。これはone controlled heap-policy selectionであり、general heap allocator、corruption termination、LastError modelを実装しない。詳細は[v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md)を正とする。

## 34. BGI eighth supported Win32 call return

opt-in `--observe heap-lifecycle` traceは6,870,828 bytes／SHA-256 `9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682`である。target `HeapCreate`はBGI caller RVA `0xA8886`、arguments `[0,0x1000,0]`、native result `0x04A40000`、LastError `0 -> 0`だった。このresultはhost process addressとして破棄する。最初のBGI-owned consumerはRVA `0xAC3CC`の`HeapAlloc(createdHeap,0x8,0x214)`で、その次はsize `0x800`だった。

`npm run check:runtime:bgi -- --checkpoint bgi-eighth-win32-call`はID `102`を9番目のdeclared endpointとし、call／return `0x100A8880`／`0x100A8886`、IAT `0x100D1240`、frame `[return,0,0x1000,0]`をadapterとRustで検査する。owner `checkpoint-process-private-heaps`へvariant `heapcreate-growable-initial-4096`のone live recordを作り、namespace `vine.logical-private-heap.v1`、type tag `0x68`、generation／slot `1`／`1`のlogical handle `0x68010001`を返す。backingはfirst reached heap operationまでdeferし、live allocationは0である。

generation `27` stack pageとの差8 bytes／SHA-256 `48f30a675492096d7110c249c8f8bc35da1b58ab3db07df9553e168e56e05d93`をreconcileする。guest writeはなくgeneration `27`／dirty 0を保持し、ESP `0x006FFF7C`でreturnする。14命令後、entryから累計122命令でexact ID `72`／`KERNEL32.dll!GetModuleHandleW`／`0x7F000470`へ到達した。IATは`0x100D11C8`、frameは`[0x100A5811,0x100D24D0]`で、argumentはimmutable image内のUTF-16 `KERNEL32.DLL`である。49,699-byte reportのSHA-256は`f8c8ca1e678bb1e118248a6626825ddf585dc240aa59e1abc428429cbab3a1ad`で、source hashと先行report hashesは不変だった。これはone controlled logical heap creationであり、general heap backing／allocator、module loader、LastError modelを実装しない。詳細は[v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md)を正とする。

## 35. BGI ninth supported Win32 call return

opt-in `--observe module-handle` traceは347,850 bytes／SHA-256 `bac37acf086ff691ba436b2959143c76894f20f5e271667813cefebeb5ee3557`である。target `GetModuleHandleW`はBGI caller RVA `0xA5811`、argument `KERNEL32.DLL`、native result `0x758B0000`、LastError `87 -> 87`だった。このhost system-module baseは破棄する。直後のBGI-owned consumerはRVA `0xA582F`の`GetProcAddress(nativeKernel32,"FlsAlloc")`で、続けて3件のFLS exportをlookupする。

`npm run check:runtime:bgi -- --checkpoint bgi-ninth-win32-call`はID `72`を10番目のdeclared endpointとし、call／return `0x100A580B`／`0x100A5811`、IAT `0x100D11C8`、frame `[return,0x100D24D0]`をadapterとRustで検査する。64-code-unit boundでguest UTF-16 `KERNEL32.DLL`を読み、case-insensitive canonical identity `kernel32.dll`へ照合する。owner `checkpoint-loaded-modules`へvariant `getmodulehandlew-kernel32-loaded-runtime-module`のone live recordを作り、namespace `vine.logical-loaded-module.v1`、type tag `0x4D`、generation／slot `1`／`1`のlogical handle `0x4D010001`を返す。system-DLL guest imageとexport resolutionは未materializeである。

generation `27` stack pageとの差9 bytes／SHA-256 `2d08da9d16a6782982e4b2b9e4617909c4ec2a65fbc1702479972ad4618f4b2e`をreconcileする。guest writeはなくgeneration `27`／dirty 0を保持し、ESP `0x006FFF78`でreturnする。9命令後、entryから累計131命令でexact ID `150`／`KERNEL32.dll!GetProcAddress`／`0x7F000950`へ到達した。IATは`0x100D1300`、frameは`[0x100A582F,0x4D010001,0x100D250C]`で、second argumentはASCII `FlsAlloc`である。56,729-byte reportのSHA-256は`147a3599cf942af03034c7f410d333b35c0360812936e39a7473c43008d8d5be`で、source hashと先行report hashesは不変だった。これはone controlled logical module lookupであり、general loader list／DLL image／export／forwarder／LastError modelを実装しない。詳細は[v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md)を正とする。

## 36. BGI tenth supported Win32 call return

opt-in `--observe dynamic-export` traceは320,917 bytes／1,046 lines／SHA-256 `5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe`である。486 call pairsはbalanced、max depth `3`、diagnostics `0`だった。BGI caller RVA `0xA582F`／`0xA583C`／`0xA5849`／`0xA5856`から`FlsAlloc`／`FlsGetValue`／`FlsSetValue`／`FlsFree`をlookupし、native resultsは`0x758CD300`／`0x758D33F0`／`0x758CB2E0`／`0x758E0AA0`、全callのLastErrorは`87 -> 87`だった。host addressesは全て破棄する。

`npm run check:runtime:bgi -- --checkpoint bgi-tenth-win32-call`はID `150`を11番目のdeclared static endpointとする。BGIのcall variantは`MOV ESI,[0x100D1300]`、`PUSH 0x100D250C`、`PUSH EDI`、`CALL ESI`であり、Rustは14-byte sequence、bound IAT `0x7F000950`、frame `[0x100A582F,0x4D010001,0x100D250C]`をchecked処理する。second argumentはname lookupと分類し、64-byte boundでASCII `FlsAlloc\0`を読む。separate runtime owner `9`のarena `[0x7F020000,0x7F030000)`へnamespace `vine.dynamic-export-endpoint.v1`、ID `1`、address `0x7F020000`、identity `kernel32.dll!FlsAlloc`、lifetime `process-runtime`のRX endpointをsealする。page SHA-256は`4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059`であり、host addressを格納しない。

tenth modeだけinitial reflectionは389 pages／generation `27`／SHA-256 `e7a3cc956e167db7e078feefb30eb1d9f2204c8e17ca008f72c4359cce752456`となり、既存write generationは`28`／`29`／`30`へshiftする。generation `30` stack pageとの差15 bytes／SHA-256 `0ec9e7bed9bb310bdab5ae447581656314c744cf4d8fe2104a36d9fe297215c7`をreconcileし、guest writeなしでgeneration／dirty 0を保持してendpointを返す。5命令後、entryから累計136命令でsame ID `150`／`GetProcAddress`へ到達する。frameは`[0x100A583C,0x4D010001,0x100D2500]`、nameは`FlsGetValue`である。65,858-byte reportのSHA-256は`54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf`で2回一致し、sourceと先行report hashesは不変だった。これはone controlled dynamic lookup／endpoint allocationであり、general export／forwarder resolution、endpoint dispatch、FLS semantics、LastError modelを実装しない。詳細は[v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md)を正とする。

## 37. BGI eleventh supported Win32 call return

same dynamic-export traceのsecond BGI-owned lookupはcaller RVA `0xA583C`、symbol `FlsGetValue`、native result `0x758D33F0`、LastError `87 -> 87`である。runtimeはID `1`／`0x7F020000`／`FlsAlloc`とID `2`／`0x7F020010`／`FlsGetValue`をone transactionでsame owner／arenaへinstallし、shared RX page SHA-256 `bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc`でsealする。first endpointのidentity／address／lifetimeは不変であり、host pointerは保持しない。

`npm run check:runtime:bgi -- --checkpoint bgi-eleventh-win32-call`はframe `[0x100A583C,0x4D010001,0x100D2500]`、live ESI `0x7F000950`、`PUSH name／PUSH EDI／STORE prior EAX／CALL ESI`の13-byte sequence、bounded ASCII `FlsGetValue\0`をadapterとRustで検査する。generation `30`のstack pageとimage data page `0x100FE000`をone transactionでreconcileし、各changed bytesは`2`、SHA-256は`5a4cdc83d919e8a3dfc1f0617bde34c9fc0f7eb3cee2a6597aa9f22e8e7e67ef`／`098e6d79168e51989225bbab5e1c59d889363412c53b29d7478a0ad4174101f6`である。RustはBGI slot `0x100FEEBC`がfirst endpoint `0x7F020000`を保持することをrequired invariantとする。

ID `2`を返し、BGIが`0x100FEEC0`へ`0x7F020010`をstoreした5命令後、entryから累計141命令でsame ID `150`／`GetProcAddress`へ到達する。frameは`[0x100A5849,0x4D010001,0x100D24F4]`、nameは`FlsSetValue`である。eleventh reflectionは389 pages／generation `27`／SHA-256 `de382ad25a1487a1c8b038f48eace933d0270cf745820419b776689c87a50eeb`、76,844-byte reportのSHA-256は`40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5`で2回一致した。tenth report hashも不変である。これはsecond controlled lookupとbounded two-page reconciliationであり、general dirty discovery、endpoint dispatch、FLS semantics、export／forwarder resolutionを実装しない。詳細は[v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md)を正とする。

## 38. BGI twelfth supported Win32 call return

same dynamic-export traceのthird BGI-owned lookupはcaller RVA `0xA5849`、symbol `FlsSetValue`、native result `0x758CB2E0`、LastError `87 -> 87`である。runtimeはID `1`／`FlsAlloc`、ID `2`／`FlsGetValue`、ID `3`／`FlsSetValue`をone transactionでsame owner／arenaへinstallし、shared RX page SHA-256 `3b6345284f0929d025fc45cb9ae5f1d720c7575ec110cb41e7fbcaaa2fad2439`でsealする。first two endpoint identities／addresses／lifetimesは不変であり、host pointerは保持しない。

`npm run check:runtime:bgi -- --checkpoint bgi-twelfth-win32-call`はframe `[0x100A5849,0x4D010001,0x100D24F4]`、live ESI `0x7F000950`、13-byte persistent-register sequence、bounded ASCII `FlsSetValue\0`をadapterとRustで検査する。generation `30`のstack／image pagesをone transactionでreconcileし、各changed bytesは`3`、SHA-256は`c4f6754b2889228cfd3f7382adff164a4c9bb8c9722ac1f5f0a38e03471a9286`／`1cb318f989df8cbf93f59a84f1f5ab9b62d57f3237f7e7162c8df79456ca9d68`である。RustはBGI slots `0x100FEEBC = 0x7F020000`／`0x100FEEC0 = 0x7F020010`をrequired invariantsとする。

ID `3`を返し、BGIが`0x100FEEC4`へ`0x7F020020`をstoreした5命令後、entryから累計146命令でsame ID `150`／`GetProcAddress`へ到達する。frameは`[0x100A5856,0x4D010001,0x100D24EC]`、nameは`FlsFree`である。reflectionは389 pages／generation `27`／SHA-256 `8025131a9a8687ec01f68969f6cda2c102eaff9258a45bfcd29c5c5c3ae4b344`、89,289-byte reportのSHA-256は`28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4`で2回一致した。eleventh report hashも不変である。これはthird controlled lookupとbounded two-page reconciliationであり、general dirty discovery、endpoint dispatch、FLS semantics、export／forwarder resolutionを実装しない。詳細は[v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md)を正とする。

## 39. BGI thirteenth supported Win32 call return

same dynamic-export traceのfourth lookupはcaller RVA `0xA5856`、symbol `FlsFree`、native result `0x758E0AA0`、LastError `87 -> 87`である。runtimeはfour ordered FLS identitiesをsame owner／arenaへinstallし、shared RX page SHA-256 `82ff4f595efb66795e9086158d940bdfb0f8e30a73d6d16879d599eb5afab615`でsealする。first three endpoint identities／addresses／lifetimesは不変であり、host pointerは保持しない。

`npm run check:runtime:bgi -- --checkpoint bgi-thirteenth-win32-call`はframe `[0x100A5856,0x4D010001,0x100D24EC]`、live ESI `0x7F000950`、13-byte sequence、bounded ASCII `FlsFree\0`を検査する。generation `30`のstack／image pagesをone transactionでreconcileし、changed bytesは`2`／`3`、SHA-256は`ecbad1e057c0e424b1bdc8c863bd9a4ae85d77f6bee312ce7ea02704160dedf8`／`c166a29b61c167001046de381b7dd5ba238ef5e5694021a152d626c9024ca7d0`である。Rustはfirst three endpoint storesをrequired invariantsとする。

ID `4`／`0x7F020030`を返し、BGI store `0x100FEEC8`を観測した12命令後、entryから累計158命令でstatic ID `70`／`KERNEL32.dll!TlsAlloc`／`0x7F000450`へ到達する。zero-argument frameのreturnは`0x100A58AA`である。reflection SHA-256は`0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`、103,179-byte reportのSHA-256は`1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4`で2回一致した。twelfth report hashも不変である。`TlsAlloc`はdispatchしておらず、native TLS evidenceとsemanticsは次checkpointで取得する。詳細は[v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md)を正とする。

## 40. BGI fourteenth supported Win32 call return

opt-in `--observe tls-lifecycle` traceは6,138,442 bytes／19,816 lines／SHA-256 `214ca138648d6674860ba3fc43e18c017470f0b31e023f1e7e9e4afa88dae08b`で、62,273-byte／223-line confirmation captureのSHA-256は`fd23eb23fca34953216b0a1cf11c5ce4d88959618c5bc1173b58eab5199ad30e`である。両方でBGI caller RVA `0xA58AA`の`TlsAlloc()`はnative index `0x1C`、LastError `87 -> 87`を返す。直後のRVA `0xA58C1`は`TlsSetValue(0x1C,native FlsGetValue address)`を呼ぶ。native index／pointerは破棄する。

`npm run check:runtime:bgi -- --checkpoint bgi-fourteenth-win32-call`はID `70`を12番目のdeclared static endpointとし、zero-argument frame `[0x100A58AA]`、six-byte IAT call、IAT `0x100D11C0`、four endpoint storesをadapterとRustで検査する。owner `checkpoint-thread-local-storage`はnamespace `vine.logical-tls-index.v1`のlogical index `0`をlifecycle `allocated`／initialized thread values `0`として所有する。`TLS_OUT_OF_INDEXES = 0xFFFFFFFF`をfailure sentinelとして記録するが、failure pathは未到達である。

generation `30`のstack／image pagesをtransactional reconcileし、各changed bytesは`3`、SHA-256は`3595f775ff05d8b86348f1ddc3f931cc11b2bd9e769af5f012919daa888f91da`／`705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8`である。index `0`を返した7命令後、累計165命令でID `68`／`KERNEL32.dll!TlsSetValue`／`0x7F000430`へ到達し、frameは`[0x100A58C1,0,0x7F020010]`となる。110,236-byte reportのSHA-256は`4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb`で2回一致し、thirteenth report hashも不変である。これはone successful logical allocationであり、`TlsSetValue`、thread values、general TLS／FLS、LastError modelを実装しない。詳細は[v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md)を正とする。

## 41. BGI fifteenth supported Win32 call return

same TLS capturesのimmediate consumerはBGI caller RVA `0xA58C1`の`TlsSetValue(nativeIndex,nativeFlsGetValueAddress)`、BOOL `1`、LastError `87 -> 87`である。runtimeはnative argumentsを破棄し、logical index `0`とdynamic endpoint ID `2`／`0x7F020010`へ置き換える。

`npm run check:runtime:bgi -- --checkpoint bgi-fifteenth-win32-call`はID `68`を13番目のdeclared endpointとし、frame `[0x100A58C1,0,0x7F020010]`、live ESI／`CALL ESI`、bound IATをadapterとRustで検査する。allocation metadataを同一Rust sessionで保持し、initial controlled thread ID `8192`へFlsGetValue endpoint identityをone value recordとして関連付ける。

generation `30`のstack／endpoint-store／TLS-index-store pagesをtransactional reconcileし、changed bytesは`8`／`0`／`12`、SHA-256は`0f62e02ebf62129f93e1bbad8e4b2f2739783536fd70af2df31a7cd5ef3accec`／`705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8`／`b25cb15bb316bf0e716d70f9503a982c60ed5189eeb903b29d602141896d3c2c`である。BGI global `0x100F0764 = 0`をRust memoryへ取り込む。BOOL `1`を返した9命令後、累計174命令でID `89`／`KERNEL32.dll!EncodePointer`／`0x7F000580`へ到達し、frameは`[0x100A5467,NULL]`となる。120,499-byte reportのSHA-256は`25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f`で2回一致し、fourteenth report hashも不変である。これはone initial-thread valueであり、general TLS／FLSやpointer encodingを実装しない。詳細は[v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md)を正とする。

## 42. BGI sixteenth supported Win32 call return

opt-in `--observe pointer-encoding` profileのprimary captureは2,213,858 bytes／6,979 lines／SHA-256 `71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3`、confirmationは312,314 bytes／1,040 lines／SHA-256 `4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c`である。caller RVA `0xA5467`の`EncodePointer(NULL)`はそれぞれ`0x6F713C58`／`0x765B8B70`を返し、LastErrorは`0 -> 0`／`87 -> 87`である。process-cookie-dependent native valuesは破棄する。

`npm run check:runtime:bgi -- --checkpoint bgi-sixteenth-win32-call`はID `89`を14番目のdeclared endpointとし、frame `[0x100A5467,NULL]`、`PUSH 0`、direct `CALL [0x100D120C]`、bound IATをadapterとRustで検査する。same Rust sessionのlogical TLS index `0`、initial thread ID `8192`、FlsGetValue endpoint ID `2`をrequired invariantsとして保持する。owner `checkpoint-pointer-encoding`／namespace `vine.logical-encoded-pointer.v1`のcontrolled token `0xEC010001`を返し、native result／process cookieはimportしない。

generation `30`のstack pageを13 bytes reconcileし、SHA-256は`5f103e07657780d9f8a17053498cf300c1edee9a17272a20455ffd453fdff42a`である。55命令後、累計229命令でsame ID `89`／`EncodePointer(0x100A9608)`へ到達し、return addressは`0x100A9697`となる。129,530-byte reportのSHA-256は`310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b`で2回一致し、fifteenth report hashも不変である。これはencoded-NULL sentinelだけであり、general pointer encoding／decoding、process cookie、LastError modelを実装しない。詳細は[v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md)を正とする。

## 43. BGI seventeenth supported Win32 call return

same pointer tracesのsecond BGI callはcaller RVA `0xA9697`、source RVA `0xA9608`、native results `0x6F766E99`／`0x71094A70`、LastError `0 -> 0`／`87 -> 87`である。static disassemblyはsourceをprocess-initializer guest function、first consumerをglobal RVA `0xFF62C`へのstore、その後のconsumerを`DecodePointer`後のindirect callと分類する。native encoded valuesは破棄する。

`npm run check:runtime:bgi -- --checkpoint bgi-seventeenth-win32-call`はframe `[0x100A9697,0x100A9608]`、11-byte `PUSH imm32`／direct IAT call、source page owner／execute protection、17-byte function prefixをadapterとRustで検査する。prior encoded-NULL object、logical TLS slot／initial-thread endpoint valueをrequired invariantsとして保持し、namespace `vine.logical-encoded-pointer.v1`のprocess-initializer token `0xEC010002`を返す。

generation `30`のstack pageを32 bytes reconcileし、SHA-256は`3091514821863563f1c42861b1835da0639a0aecf15286732f71dec102615893`である。BGI store `0x100FF62C: 0 -> 0xEC010002`を観測し、9命令後、累計238命令でsame ID `89`／`EncodePointer(0x7F020000)`へ到達する。140,660-byte reportのSHA-256は`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`で2回一致し、sixteenth report hashも不変である。これはone executable guest-function mappingであり、general pointer encoding／decodingやendpoint invocationを実装しない。詳細は[v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md)を正とする。

## 44. BGI critical-section synchronization

opt-in `--observe critical-section` captureは8,094,273 bytes／14,542 lines／
SHA-256 `c668e8e6bd831442d4f0f5e3e2937395b7cd2bc003aa96a67b8872417cc53567`
で、Frida `17.15.4`、Windows build `26200`、diagnostics／oracle errors `0`だった。
BGI caller RVA `0xAB5E7`はspin count `4000`で14個の24-byte objectsをnative
address `0x0033F630..0x0033F768`へ連続初期化し、全callがBOOL `1`を返した。
image RVAは`0xFF630..0xFF768`なのでruntime mappingは
`0x100FF630..0x100FF768`となる。initialized fieldsは
`[-1,-1,0,0,0,4000]`である。

Frida-owned controlled oracleはnonzero LastError `87..92`がinitialize、recursive
enter／leave、deleteの全てで保持されることを測定した。first enterは
`LockCount -2`／recursion `1`／native TID、recursive enterはrecursion `2`、final
leaveはunlocked bytes、deleteは24 bytes zeroとなる。BGI側のimmediate consumerは
RVA `0xAB763`／`EnterCriticalSection`と`0xAB66F`／`LeaveCriticalSection`であり、
capture中にRVA `0xFF6C0`／`0xFF6F0`／`0xFF708`へ5／16／18 balanced pairsを
観測した。

Rustはinitialization、same-thread recursion、releaseの3 contractsとprocess-owned
stateを実装し、controlled TID `8192`をguest owner fieldへ書く。cross-thread
contentionはscheduler blocking未実装として`critical-section-contention-unimplemented`
でmutation前に停止する。generic BGI profileは14 initializationsを含む35 calls、36
writeback batches／65 dirty pages／344 changed bytes、17 host deltas、final generation
`80`を通し、instruction 575の`DecodePointer(0xEC010003)`へ進んだ。167,719-byte
reportのSHA-256は`cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69`
である。詳細は[bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md)
を正とする。

## 45. BGI pointer decoding

existing `--observe pointer-encoding` primary／confirmation tracesは
2,213,858 bytes／6,979 lines／SHA-256
`71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3`と
312,314 bytes／1,040 lines／SHA-256
`4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c`
である。BGI caller RVA `0xA5924`の`DecodePointer`はprocessごとに異なる
`0x61C0A638`／`0xC7C1EB7E`を同じnative `FlsAlloc` address
`0x758CD300`へ戻し、LastError `0 -> 0`／`87 -> 87`を保持した。native token、
address、process cookieは破棄する。

Rustはconfigured namespace、nonzero token ID、token record、reverse source indexを
checked処理し、registered Vine tokenだけをoriginal sourceへ戻す。controlled
`0xEC010003`はdynamic endpoint ID `1`／`0x7F020000`へdecodeされ、guest writeと
LastError変更はない。unknown／malformed tokenはmutation前にstructured invalid-call
となる。

`npm run check:runtime:bgi -- --profile kernel32-pointer-decoding`は36 calls、37 writeback batches／66 dirty
pages／348 changed bytes、17 host deltas、final generation `81`を通す。instruction
`577`でdynamic `FlsAlloc(0x100A5666)`へ到達し、未実装contractを
`dynamic-export-contract-missing`として停止する。171,356-byte reportのSHA-256は
`5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e`で2回一致した。
詳細は[bgi-pointer-decoding.md](bgi-pointer-decoding.md)を正とする。

## 46. BGI fiber-local-storage allocation

opt-in `--observe fls-lifecycle` tracesは441,487 bytes／1,459 lines／SHA-256
`2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281`と
453,151 bytes／1,498 lines／SHA-256
`d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186`
である。summaryは694／713 balanced API pairs、max depth `3`、contiguous
sequence、diagnostics `0`を返した。summarizerのobservation allowlistもtracerと
揃え、`fls-lifecycle`と既存`critical-section`をpublic contractで受け付ける。

BGI call instruction RVA `0xA5924`の`FlsAlloc`はcallback RVA `0xA5666`を受け、
両captureでnative index `9`を返した。LastErrorは`87 -> 87`／`0 -> 0`である。
native indexはprocess-local allocation orderに依存するため破棄する。直後のRVA
`0xA5926`はresultをRVA `0xF0760`へstoreし、`0xFFFFFFFF`と比較する。

Rustはnon-NULL callbackのexecute accessをchecked処理し、128-slot bounded
process tableへnamespace `vine.logical-fls-index.v1`のindex `0`とcallback
`0x100A5666`を保持する。dynamic endpoint ID `1`はbinding-scoped identityにより
same IDのstatic importと共存し、common `ApiDispatcher`からone-argument stdcallで
returnする。LastError、guest memory、host deltaはdispatch中に変えない。

`npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-allocation`は18 contracts／37 calls、
38 writeback batches／68 dirty pages／399 changed bytes、17 host deltas、final
generation `82`を通す。instruction `618`でID `75`／
`HeapAlloc(0x68010001,8,0x214)`へ到達し、`kernel32-private-heap-allocation`で
structured stopする。175,123-byte reportのSHA-256は
`26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c`で2回一致し、
先行4 reportsはbyte-identicalである。詳細は
[bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md)を正とする。

## 47. BGI private-heap allocation

existing opt-in heap-lifecycle traceは6,870,828 bytes／21,822 lines／SHA-256
`9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682`
である。BGI caller return RVA `0xAC3CC`のfirst paired callは
`HeapAlloc(nativeHeap,0x8,0x214)`、native result `0x04A405C0`、LastError
`0 -> 0`である。直後のpaired callはsame caller／flags、size `0x800`、result
`0x04A407E0`であり、later variantとしてdeferする。Native handle／resultsは破棄する。

Rust common dispatcherはlogical heap `0x68010001`、exact
`HEAP_ZERO_MEMORY`／`0x214`だけをexisting `ExecutionSession` transactionへ渡す。
Allocation ID `1`、guest address `0x20000000`、aligned length `0x218`を返し、16 MiB
reserveとone-page RW／NX commitをgeneration `82 -> 85`のbinary host deltaとして
v86 mirrorへ適用する。Other flags／sizesはmutation前に
`heap-alloc-variant-unsupported`となる。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-allocation`は19 contracts／39 calls、40
writeback batches／70 dirty pages／415 changed bytes、18 host deltas、final generation
`87`を通す。second `DecodePointer(0xEC010005)`がdynamic endpoint ID `3`／
`0x7F020020`を返し、instruction `644`で`FlsSetValue(0,0x20000000)`へ到達する。
183,102-byte reportのSHA-256は
`d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4`
である。詳細は[bgi-private-heap-allocation.md](bgi-private-heap-allocation.md)を正とする。

## 48. BGI fiber-local-storage value

existing opt-in FLS lifecycle tracesは441,487 bytes／1,459 lines／SHA-256
`2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281`と
453,151 bytes／1,498 lines／SHA-256
`d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186`
である。BGI caller return RVA `0xA5955`の`FlsSetValue`はnative index `9`、process-specific
heap pointer、BOOL `1`を返し、LastError `87 -> 87`／`0 -> 0`を保持する。

Rust common dispatcherはlogical index `0`とopaque value `0x20000000`をchecked
two-argument stdcallで受け、selected thread `8192`のseparate FLS value mapへ保存する。
Process-owned slot／callbackは共有し、thread-local valueはTLS mapと分離する。Unknown index／
inactive laneはmutation前にstructured faultとなる。

`npm run check:runtime:bgi -- --profile kernel32-fiber-local-storage-value`は20 contracts／42 calls、43
writeback batches／75 dirty pages／497 changed bytes、19 host deltas、final generation `91`
を通す。Existing second `GetModuleHandleW`／first `EnterCriticalSection`をreturnし、
instruction `707`でID `124`／`InterlockedIncrement(0x100F08E0)`へ到達する。
195,821-byte reportのSHA-256は
`f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f`
である。詳細は[bgi-fiber-local-storage-value.md](bgi-fiber-local-storage-value.md)を正とする。

## 49. BGI InterlockedIncrement

opt-in `interlocked-increment` tracesは272,588 bytes／875 lines／SHA-256
`917f73f6ad71737c406b2be2324c94abd7e6c59135f771ffe23f7664178706e5`と
335,188 bytes／1,054 lines／SHA-256
`719ad5fd2f36ad10897c414bb528a8140ae7256e3981a29010fe0971b6b36c3a`
である。両captureでBGI caller RVA `0xA5577`／`0xA7351`／`0xA73C7`はそれぞれ
LONG `0 -> 1`／`1 -> 2`／`0 -> 1`、同じresult、LastError `87 -> 87`を返す。
Later `0xA5577` pairの`-1 -> 0`はsigned wrappingを固定する。

Rust common dispatcherはrequired aligned `InOut LONG`をread／write preflightし、
`i32::wrapping_add(1)`のone guest writeとstdcall returnを同じimmediate completionへ入れる。
Current single Runtime Worker lane内ではguest executionを挟まず更新するが、general host-thread／
cross-worker atomicityはclaimしない。

`npm run check:runtime:bgi -- --profile kernel32-interlocked-increment`は21 contracts／50 calls、51 writeback
batches／86 dirty pages／667 changed bytes、26 host deltas、final generation `106`を通す。
3 targetのwrite generationは`92`／`98`／`100`である。instruction `934`でID `75`／
`HeapAlloc(0x68010001,8,0x800)`を`heap-alloc-variant-unsupported`として停止する。
234,775-byte reportのSHA-256は
`4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643`
である。詳細は[bgi-interlocked-increment.md](bgi-interlocked-increment.md)を正とする。

## 50. BGI second private-heap allocation

existing heap-lifecycle traceのcaller RVA `0xAC3CC`でcall IDs `28`／`29`はsame native
heap、flags `0x8`、sizes `0x214`／`0x800`、nonnull results `0x04A405C0`／
`0x04A407E0`、LastError `0 -> 0`を返す。Next same-caller sizeは`0xBC`でありdeferする。

このcapture時の旧implementation IDはartifact identityとして保持する。Current runtimeは
single `kernel32.heap-alloc.v2` handlerへ`kernel32-heap-two-v1` policyを渡す。First allocation ID `1`を保持し、
second ID `2`／guest `0x20000218`／zeroed `0x800`をsame bounded heapへ作る。
Generation `106 -> 107` deltaはunsealed owner `10`のexisting page `0x20000000`を更新し、
mapping operation／cache effectは0である。Mirror mappingはowner-sealed bitを保持してlater
deltaのidentityを検証する。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-second-allocation`は21 contracts／51 calls、52
writeback batches／89 dirty pages／906 changed bytes、27 host deltas、final generation `108`
を通す。instruction `1395`でID `94`／`GetStdHandle(STD_INPUT_HANDLE)`へ到達する。
239,789-byte reportのSHA-256は
`fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8`
である。詳細は[bgi-private-heap-second-allocation.md](bgi-private-heap-second-allocation.md)を正とする。

## 51. BGI standard input handle

opt-in `standard-handle` tracesは279,633 bytes／928 lines／SHA-256
`ececb814fe84a5f1495f3c6fa1fd41801fed12deee8b2699f961db4418f36de9`と
246,261 bytes／820 lines／SHA-256
`e5979f91d1f77977fc93d9785679190bafefd9afb3b82ec83c27eba9fa42766e`
である。両captureでBGI caller return RVA `0xA87B6`の
`GetStdHandle(STD_INPUT_HANDLE)`はnative result `0x208`、LastError `87 -> 87`を返す。
Resultは`INVALID_HANDLE_VALUE`／NULL check後、unchangedのままIAT `0x004D123C`／
`GetFileType`へ渡される。Native handle値はprocess-specificなのでdiscardする。

Rust common dispatcherはselector `0xFFFFFFF6`だけを受け、process-owned namespace
`vine.logical-standard-handle.v1`へlogical stdin `0x53010001`を遅延登録する。One-argument
stdcall、LastError preserve、guest write／host delta 0でreturnする。Standard output／errorは
mutation前にstructured unsupportedとなる。

`npm run check:runtime:bgi -- --profile kernel32-standard-input-handle`は22 contracts／52 calls、53 writeback
batches／90 dirty pages／911 changed bytes、27 host deltas、final generation `109`を通す。
instruction `1403`でID `101`／`GetFileType(0x53010001)`へ到達する。243,749-byte
reportのSHA-256は
`5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e`
である。詳細は[bgi-standard-input-handle.md](bgi-standard-input-handle.md)を正とする。

## 52. BGI standard input file type

opt-in `file-type` tracesは246,422 bytes／819 lines／SHA-256
`480e2e9e7901535f88cd6dd87994bc7da97c47cd90ee0eb77dbef1149bfb7a9f`と
261,838 bytes／872 lines／SHA-256
`9986a349350e6875c779f35582347dbc485618492f7a68afa3716064ba1f3f02`
である。BGI caller return RVA `0xA87C8`のstdin handles `0x208`／`0x210`はいずれも
`FILE_TYPE_CHAR (2)`、LastError `87 -> 87`を返す。Consumerはzero check、low-byte
classification、handle storeの後にcritical-section initializationへ進む。Native handle値はdiscardする。

Rust common dispatcherはregistered logical stdin `0x53010001`だけをresolveして`2`を返す。
Unknown logical handleはmutation前にstructured faultとなる。`npm run check:runtime:bgi -- --profile kernel32-file-type`は
23 contracts／54 calls、55 writeback batches／94 dirty pages／938 changed bytes、28 host deltas、
final generation `112`を通す。Existing 15th critical-section initializationをreturnし、instruction
`1439`でID `94`／`GetStdHandle(STD_OUTPUT_HANDLE)`をinput-only variant faultとして停止する。
252,994-byte reportのSHA-256は
`dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf`
である。詳細は[bgi-file-type.md](bgi-file-type.md)を正とする。

## 53. BGI standard output handle

existing opt-in `standard-handle` tracesは279,633 bytes／928 lines／SHA-256
`ececb814fe84a5f1495f3c6fa1fd41801fed12deee8b2699f961db4418f36de9`と246,261
bytes／820 lines／SHA-256
`e5979f91d1f77977fc93d9785679190bafefd9afb3b82ec83c27eba9fa42766e`である。BGI
caller return RVA `0xA87B6`の`STD_OUTPUT_HANDLE`は両captureで`0x234`、
LastError `87 -> 87`を返す。file-type capturesはoutput handle `0x234`／`0x23C`を
`GetFileType`へ渡すことも固定する。Native handle値はdiscardする。

Current Rust common dispatcherはsingle `kernel32.get-std-handle.v2`へ
`kernel32-standard-output-v1` policyを渡し、process-owned logical stdout
`0x53010002`を返す。Input-only profileと先行11 reportsは不変である。
`npm run check:runtime:bgi -- --profile kernel32-standard-output-handle`は23 contracts／55 calls、56
writeback batches／95 dirty pages／943 changed bytes、28 host deltas、final generation `113`を
通す。instruction `1447`でID `101`／`GetFileType(0x53010002)`を
`file-type-standard-handle-unsupported`として停止する。Current policy-bearing
257,878-byte reportのSHA-256は`564c3425f01b35e86717b68335b18f8bcd3d30d996717a1d0848028104cf0a2b`
である。詳細は[bgi-standard-output-handle.md](bgi-standard-output-handle.md)を正とする。

## 54. BGI standard output file type

same opt-in `file-type` tracesのoutput handles `0x234`／`0x23C`は、BGI caller return RVA
`0xA87C8`でいずれも`FILE_TYPE_PIPE (3)`、LastError `87 -> 87`を返す。Consumerはlow byteを
分類し、handleを保持した後、16番目のcritical sectionを初期化して`STD_ERROR_HANDLE`を要求する。
Native handle値はprocess-specificなのでdiscardする。

Rust common dispatcherはregistered logical stdout `0x53010002`だけをexisting
`kernel32.get-file-type.v2`でresolveし、versioned
`kernel32-standard-output-file-type-v1` policyから`3`を返す。Default policy、unknown handle、
standard errorはmutation前にstructured unsupportedとなる。
`npm run check:runtime:bgi -- --profile kernel32-standard-output-file-type`は23 contracts／57 calls、58 writeback
batches／99 dirty pages／966 changed bytes、29 host deltas、final generation `116`を通す。
instruction `1484`でID `94`／`GetStdHandle(STD_ERROR_HANDLE)`を
`standard-handle-selector-unsupported`として停止する。267,223-byte reportのSHA-256は
`0bc3770b1da0af519e4065021c11cc423427b9855d9ceedac6d9b00053837301`である。

Chrome Runtime Workerも57 ordered calls、same final memory SHA-256
`909cca3e66d023f7bc2a6b9c8ab9f2435b17338fa850a14823983f513b1c1b68`、same structured stopを
再現する。slice-boundary yieldを`setTimeout(0)`からlocal `MessageChannel` taskへ移した後、
unchanged 3分gateは1,750 sessions／failure 0、p95 `97.010 ms`で全thresholdを通した。
cancel 8 instructions、watchdog 4 instructions、pending work 0は不変である。詳細は
[bgi-standard-output-file-type.md](bgi-standard-output-file-type.md)と
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

## 55. BGI standard error handle

same opt-in `standard-handle` tracesはBGI caller return RVA `0xA87B6`の
`STD_ERROR_HANDLE`に対して、両captureでnative result `0x238`、LastError `87 -> 87`を返す。
Resultは`INVALID_HANDLE_VALUE`／NULL check後、unchangedのままIAT `0x004D123C`／
`GetFileType`へ渡される。Separate file-type capturesのstderr handles `0x238`／`0x240`は
ともに`FILE_TYPE_PIPE (3)`を返すが、このcheckpointではclassificationを実装しない。Native
handle値はdiscardする。

Rust common dispatcherはsingle `kernel32.get-std-handle.v2`へ
`kernel32-standard-error-v1` policyを渡し、process-owned logical stderr
`0x53010003`を返す。先行policyとreportsは不変で、stderr file typeはmutation前にstructured
unsupportedとなる。`npm run check:runtime:bgi -- --profile kernel32-standard-error-handle`は23 contracts／58 calls、
59 writeback batches／100 dirty pages／971 changed bytes、29 host deltas、final generation `117`を
通す。instruction `1492`でID `101`／`GetFileType(0x53010003)`を
`file-type-standard-handle-unsupported`として停止する。271,184-byte reportのSHA-256は
`bfa1da3badbbd96ed5c282bd21c3bb0041fa06834dbad38fe0af32e116432c2f`である。

Chrome Runtime Workerも58 ordered calls、same final memory SHA-256
`a97f019fb34b6ab90d9cbf76f35b194674296de83088cb1736cbdbad090b5b12`、same structured stopを
再現する。Unchanged 3分gateは1,771 sessions／failure 0、p95 `96.485 ms`、262 Wasm
crossings/sessionで全thresholdを通した。cancel 8 instructions、watchdog 4 instructions、
pending work 0は不変である。詳細は
[bgi-standard-error-handle.md](bgi-standard-error-handle.md)と
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

## 56. BGI standard error file type

same opt-in `file-type` tracesはBGI caller return RVA `0xA87C8`のstderr
handles `0x238`／`0x240`に対して、いずれも`FILE_TYPE_PIPE (3)`、LastError
`87 -> 87`を返す。Consumerはzero checkとlow-byte classification後にhandleを保持し、
17番目のcritical sectionを初期化して`SetHandleCount(32)`へ進む。Native handle値は
process-specificなのでdiscardする。

Rust common dispatcherはregistered logical stderr `0x53010003`だけをexisting
`kernel32.get-file-type.v2`でresolveし、versioned
`kernel32-standard-error-file-type-v1` policyから`3`を返す。Default policyとunknown
handleはmutation前にstructured unsupportedとなる。
`npm run check:runtime:bgi -- --profile kernel32-standard-error-file-type`は23 contracts／60 calls、61
writeback batches／104 dirty pages／991 changed bytes、30 host deltas、final generation
`120`を通す。instruction `1516`でID `99`／`SetHandleCount(32)`を
`endpoint-contract-missing`として停止する。280,475-byte reportのSHA-256は
`ce9d4271db746b1b7d377bb3e42a861f9c611367e5fcfefba0f19b35fca5aee1`である。

Chrome Runtime Workerも60 ordered calls、same final memory SHA-256
`5bb58fba2f503a84550a71e0ba35f11049e77ebb9038461db91f04d0dfb3bb61`、same
structured stopを再現する。Unchanged 3分gateは1,807 sessions／failure 0、p95
`95.215 ms`、269 Wasm crossings/sessionで全thresholdを通した。cancel 8
instructions、watchdog 4 instructions、pending work 0は不変である。詳細は
[bgi-standard-error-file-type.md](bgi-standard-error-file-type.md)と
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

## 57. BGI SetHandleCount

opt-in `set-handle-count` tracesは233,779 bytes／779 lines／SHA-256
`ce0ced493ed1af598d5f4b3e6b3aad3ab811b84991cc69fbfe4bf5d98b649a71`と
234,327 bytes／779 lines／SHA-256
`8f09c81e20837926d02d8584954641db726932eb7d7f1aa5eebb6ab9e152f4e8`
である。両captureともBGI caller return RVA `0xA881F`、argument `32`、result `32`、
LastError `87 -> 87`を返す。CallerはIAT call直後の`xor eax,eax`でresultを破棄して
function return `0`を作る。Native handle-table stateはimportしない。

Rust common dispatcherはsingle `kernel32.set-handle-count.v1`へversioned
`kernel32-set-handle-count-v1` policyを渡し、enabled count `32`を同値で返す。
Historical policiesとunlisted countはruntime／memory mutation前にstructured unsupportedとなる。
`npm run check:runtime:bgi -- --profile kernel32-set-handle-count`は24 contracts／61 calls、62 writeback
batches／105 dirty pages／992 changed bytes、30 host deltas、final generation `121`を通す。
instruction `1526`でID `85`／`GetCommandLineA()`を`endpoint-contract-missing`として
停止する。284,215-byte reportのSHA-256は
`91be62c168dd5296fa549a8bf553975853dfef506bcb3ce7670a7b8843f82f3b`である。

Chrome Runtime Workerも61 ordered calls、same final memory SHA-256
`3e0e3dc6fc0420af4cf1bc4233d8fc267311f3926abc18b91ce839ce2ff083b3`、same
structured stopを再現する。Unchanged 3分gateは2,220 sessions／failure 0、p95
`74.555 ms`、271 Wasm crossings/sessionで全thresholdを通した。cancel 8
instructions、watchdog 4 instructions、pending work 0は不変である。詳細は
[bgi-set-handle-count.md](bgi-set-handle-count.md)と
[bgi-browser-benchmark.md](bgi-browser-benchmark.md)を正とする。

## 58. BGI GetCommandLineA

opt-in `command-line-a` tracesは2,252,782 bytes／7,070 lines／SHA-256
`ad7babe19c454f8c2888cc97da47e830f527d8761904c02cd9dd180ce8c7fd84`と
251,449 bytes／812 lines／SHA-256
`9172c5a03ad171f54d5dc1586267f7e5f578aa0e620e5c295db60747400668ee`
である。BGI caller return RVA `0xA3786`は30-byte NUL-terminated CP932
`"V:\softs\サクラノ詩\BGI.exe"`を返し、LastErrorはそれぞれ`0 -> 0`／
`87 -> 87`である。Oracleの反復A/W callsはencodingごとにstable pointerを返し、A/W pointerは
distinctだがdecoded stringは同一である。Absolute pointerとhost pathはimportしない。

Callerはresultをglobal `0x00553520`へ保存する。RVA `0xA81D2`以降はそのpointerを再読し、NUL、
space、double quoteを走査してexecutable tokenをskipする。RVA `0xA84C6`以降も同じpointerから
argc／argvを構築するため、process-lifetime stable readable pointerが必要である。

Current runtimeは既存process-parameter authority `BGI.exe`からsame-page ASCII view
`0x7FFDD010`を作り、four process-environment pagesをpermanent allocator recordへ登録する。
`kernel32.get-command-line-a.v1`はzero-argument stdcall、LastError preserve、call中guest write 0で
pointerを返す。`npm run check:runtime:bgi -- --profile kernel32-command-line-a`は25 contracts／62 calls、
63 writebacks／107 dirty pages／1,001 changed bytes、30 host deltas、final generation `122`を通す。
instruction `1536`でID `98`／`GetEnvironmentStringsW()`へ停止する。288,041-byte reportの
SHA-256は`efa02444d94353422ff0d9a842045025a89a0adb9cf7f90f6d3a87a616b84668`である。

Chrome Runtime Workerも62 ordered calls、same memory SHA-256
`33986edc39e1f5ecb9f050add5e9f534d2e1d81cc31d003ccd5060e1d30806f2`、same stopを再現する。
218 slices、155 budget yields、274 Wasm crossings、final generation `227`で、cancel／watchdog後も
pending work 0である。詳細は[bgi-command-line-a.md](bgi-command-line-a.md)を正とする。

## 59. BGI GetEnvironmentStringsW / FreeEnvironmentStringsW

final `environment-strings-w` observerの2 capturesは263,415 bytes／819 lines／SHA-256
`c091345340239bb1bb450bac172d79c47a0d1f0123e2ba8e62ceac38d1f411a9`と
264,022 bytes／820 lines／SHA-256
`4c94f4efe5fee18a752dbafd4577b36d88b408530ca383b7d076cf135a69cba9`
である。Observerは環境名／値を保存せず、block shapeとlifetime relationだけを記録する。
Native launch blockは46 entries／double-NUL込み7,274 bytesだったが、このcontentはruntimeへ
importしない。

Oracleのtwo simultaneous acquisitionsは異なるcopy pointerを返し、PEB process-parameter
environment pointerとも異なる。Shape/content comparisonは同一で、LastError `0 -> 0`／
`87 -> 87`を保つ。両pointerの`FreeEnvironmentStringsW`はBOOL `1`を返し、sentinel
`88`／`89`を保つ。BGI callもreturn RVA `0xA855F`でLastError `0`／`87`を保ち、
RVA `0xA85D3`でsame pointerをfreeして`1`を得る。

CallerはUTF-16 double NULまで数え、whole blockを`WideCharToMultiByte`でsize query／convertした後に
freeする。Runtimeはhost entriesを使わず、`RTL_USER_PROCESS_PARAMETERS.Environment`のempty
double-NUL authority `0x7FFDD020`とdistinct copy `0x7FFDD030`を既存process-environment allocationへ
置く。`kernel32.get-environment-strings-w.v1`／
`kernel32.free-environment-strings-w.v1`はone-active-copy lifetime、exact free pointer、LastError
preserve、guest write 0を実装する。

`npm run check:runtime:bgi -- --profile kernel32-environment-strings-w`は27 contracts／63 calls、
64 writebacks／108 dirty pages／1,015 changed bytes、30 host deltas、final generation `123`を通す。
instruction `1558`でID `132`／`WideCharToMultiByte`へ停止する。Memory SHA-256は
`20109e02acad2ff74cc74520f10b4bba715479f87a1fec2ae000be3f3e179481`、291,793-byte reportの
SHA-256は`96cb98f09a1d2733442d38ce2736c15c38fd87f3f4fa082e020d623c3fdb6013`である。

Chrome Runtime Workerも63 ordered callsとsame stop／memoryを再現し、221 slices、157 yields、
278 Wasm crossings、final generation `230`、cancel／watchdog後pending work 0を通す。3分gateは
2,205 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-environment-strings-w.md](bgi-environment-strings-w.md)を正とする。

## 60. BGI WideCharToMultiByte

final `wide-char-to-multi-byte` observerの2 capturesは307,869 bytes／882 lines／SHA-256
`36244d5e122a3a6f6433c4013522946af680b2bd0867c7389daec8394176e299`と
308,089 bytes／883 lines／SHA-256
`cef9895a09956273c0c9ac3a678ce05faa0c04ad6f713c6a175b3a05423f4d16`
である。Observerはprivate environment contentを保存せず、bounded source／destination shapeと
comparison hashだけを記録する。Native active code pageは932である。

BGIはreturn RVA `0xA8599`でCP_ACP／flags 0のsize queryを行い、3,637 UTF-16 unitsに対して
3,637 bytesを得る。RVA `0xA85BC`のconversionも3,637を返し、source／destination shapeは
一致する。両callともLastError `87 -> 87`を保つ。Controlled x86 oracleはempty input result `1`、
`A`／U+3042／NULの`41 82 A0 00`、unmappable surrogate unitsの`3F 3F 00`とused-default
BOOL `1`、one-byte short bufferのpartial prefix／result `0`／LastError `122`、zero source countの
result `0`／LastError `87`を固定する。

Runtimeはhost localeを参照せず、policy-owned CP_ACP `932`と65,536-unit source boundを使う。
`kernel32.wide-char-to-multi-byte.v1`はpositive／`-1` input、query／conversion、system-default `?`、
optional used-default BOOL、complete encoded-unit prefix、source／destination overlap rejectionを
実装する。Variable outputはbounded `ApiDynamicPointerConstraint`からgeneric dispatcherの
preflight／atomic guest-write pathを通る。未対応code page／flags／explicit default pointerは
mutation前にstructured unsupportedとなる。

`npm run check:runtime:bgi -- --profile kernel32-wide-char-to-multi-byte`は28 contracts／64 calls、
65 writebacks／109 dirty pages／1,057 changed bytes、30 host deltas、final generation `124`を通す。
instruction `1591`でID `75`／`HeapAlloc(0x68010001,0,1)`を
`heap-alloc-variant-unsupported`として停止する。Memory SHA-256は
`9b2b6b23255736f59b3c02c75bb3dd5adf811fb99e93ce40bc31a601ba55fddb`、295,762-byte
reportのSHA-256は`ea0eb0f6a14869ca4e75643f01bd42c8c2e7360584c893e7578f6c21423b56cf`である。

Chrome Runtime Workerも64 ordered callsとsame stop／memoryを再現し、226 slices、161 yields、
283 Wasm crossings、final generation `234`、cancel／watchdog後pending work 0を通す。3分gateは
2,182 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-wide-char-to-multi-byte.md](bgi-wide-char-to-multi-byte.md)を正とする。

## 61. BGI private-heap one-byte allocation

combined heap／wide-environment observerのcomplete capturesは4,390,685
bytes／13,830 lines／SHA-256
`6d0c372dc3fde306798ddf2e5dc98c656aecf96e72db0424c465abb9976be693`と
4,211,371 bytes／13,262 lines／SHA-256
`4825636bcbfc9c923caaac28c96980b60d60fc4856acb9a2b4258b461bb9fe96`
である。Summariesは6,882／6,598 balanced API pairsとdiagnostics zeroを保持する。
Timeout中にheap pairが途切れた2 capturesはunbalanced evidenceとして除外した。

Native launchは`WideCharToMultiByte` queryで3,637 bytesを得て、BGI return RVA
`0xA4A33`の`HeapAlloc(nativeHeap,0,3637)`へ渡す。Non-null allocation pointerは
return RVA `0xA85BC`のconversion destinationと一致し、同じpointerがlater return RVA
`0xA48CD`の`HeapFree`へ渡されてresult `1`となる。Allocation sampleはLastError
`0 -> 0`を保つ。Initial bytesは2 capturesで一致しないため、pointer value／contentを
semantic oracleにせず、size dataflow、nonnull、consumer、lifetime relationだけを採用する。

Controlled empty environmentのquery resultは1なので、policy
`kernel32-private-heap-size-1-v1`はexisting `kernel32.heap-alloc.v2`へ
`(flags=0,size=1)`だけを許可する。Runtimeはallocation ID `3`／guest
`0x20000A18`／requested 1／reserved 8／deterministic-uninitializedを保持し、
following conversionのNUL writeとsource-copy freeまで進む。Native initial bytesのexact
再現やbroad heap semanticsは主張しない。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-size-1`は28 contracts／67 calls、
68 writebacks／114 dirty pages／1,166 changed bytes、32 host deltas、final generation `129`を通す。
instruction `1687`でID `129`／`GetLastError()`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`db234b2791244b2ca04dc20c957d3636a04b8a75ab506194d08ee03065bed7f2`、308,984-byte
reportのSHA-256は`bfecc42fc7b32d21fc86580f81174548e6a26ff2b96f9dee05de51a9e7b4e26e`である。

Chrome Runtime Workerも67 ordered callsとsame stop／memoryを再現し、240 slices、172 yields、
299 Wasm crossings、final generation `247`、cancel／watchdog後pending work 0を通す。3分gateは
2,162 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-private-heap-size-1.md](bgi-private-heap-size-1.md)を正とする。

## 62. BGI GetLastError

focused `get-last-error` observerのcomplete capturesは8,083,335
bytes／19,771 lines／SHA-256
`871e44f554e8a889cc77bf547e2898a0edb6434cec7aec8cdc529305946af325`と
8,062,489 bytes／19,757 lines／SHA-256
`1f29ca901ab1791ed3ed50a9cf27e5b9f47b9191b3984c4cfbaabd43d6560b57`
である。Summariesは9,848／9,841 balanced API pairsとdiagnostics zeroを保持する。

両runの最初のBGI callはreturn RVA `0xA55DD`でLastError `87`を読み、result `87`を返し、
return後もLastError `87`を保つ。Immediate consumerは`push dword ptr [global]`、
`mov edi,eax`、local helper call、indirect callの4命令であり、EAX resultはEDIへ保持される。
Controlled x86 oracleでLastError sentinel `0`、`87`、`122`、`0x12345678`を注入すると、
全callがfull 32-bit sentinelを返して同じ値を保つ。Native absolute address、thread ID、
hot-call count、timingはrun identityでありsemantic oracleにしない。

Runtimeの`kernel32.get-last-error.v1`はpolicy-independentなzero-argument stdcall contractである。
Selected `RuntimeThread`が所有するLastErrorを読み、full 32-bit returnを作り、
`LastErrorPolicy::Preserve`、guest write 0、runtime／memory mutation 0を保つ。Active compatibility
policyは以前のheap／process-view／code-page variantsだけを構成し、BGI observed valueをhandlerへ
埋め込まない。

`npm run check:runtime:bgi -- --profile kernel32-get-last-error`は29 contracts／68 calls、
69 writebacks／115 dirty pages／1,179 changed bytes、32 host deltas、final generation `130`を通す。
instruction `1695`でID `69`／`TlsGetValue(0)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`ea204befffc2c6eebbf5aed3a6e11f792644a35bce5c54a23c996678f2ccb909`、312,644-byte
reportのSHA-256は`29b3517baa9bc71d3b34b523f0a57c0cb17a0ff09e1dba9a842a0fc1339516b6`である。

Chrome Runtime Workerも68 ordered callsとsame stop／memoryを再現し、241 slices、172 yields、
301 Wasm crossings、final generation `248`、cancel／watchdog後pending work 0を通す。3分gateは
2,151 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-get-last-error.md](bgi-get-last-error.md)を正とする。

## 63. BGI TlsGetValue

focused `tls-lifecycle` observerのaccepted complete capturesは5,867,455
bytes／18,489 lines／SHA-256
`d9a85cadcd1d1891480e5a3c3285a14b0f111c9f805770faf4866733c24beaa5`と
6,331,907 bytes／19,955 lines／SHA-256
`78b951d688dce1ffcaa5a243b1e71e3a062af346d711794ba7f6be8f9afb4ba6`
である。Summariesは9,207／9,940 balanced API pairsとdiagnostics zeroを保持する。
Frida-created worker callbackでearly terminationした2 experimental capturesは除外し、上の
complete main-process capturesで置き換えた。

両runの最初のBGI callはreturn RVA `0xA54A0`で、preceding `TlsAlloc`が返したindexと
same-thread `TlsSetValue` valueを使う。Resultはstored valueと一致し、LastErrorは`87 -> 0`と
なる。Immediate consumerは`mov esi,eax`、`test esi,esi`、conditional success branch、
fallback `DecodePointer` callであり、nonnull resultをESIへ保持して後続indirect callへ渡す。

Controlled x86 oracleはset value `0x12345678`をsame value／LastError `0`として読み、
allocated-unset indexをNULL／LastError `0`、invalid `0xFFFFFFFF`をNULL／LastError `87`として
固定する。Separate 3,072-byte PE32/i386 fixtureのSHA-256は
`83bdaf9f52b246af3924106280feadce6419e1de78b266ea6b36b469cea7e6c3`で、native exit code
`0`を返す。Main thread value `0x12345678`、worker initial NULL／set value `0x23456789`、
main value不変を通し、per-thread relationをFrida callback timingに依存せず固定する。

Runtimeの`kernel32.tls-get-value.v1`はpolicy-independentなone-argument stdcall contractである。
Process-owned allocated-index registryを検査してselected `RuntimeThread`だけを読み、set／unset
successはLastError `0`、unallocated indexはLastError `87`を返す。TLS mapとguest memoryを変更せず、
BGI reached index／value／callerはhandlerへ埋め込まない。Missing static IAT contractは従来の
`endpoint-contract-missing`、process-owned dynamic exportは
`dynamic-export-contract-missing`としてendpoint identityを保持する。

`npm run check:runtime:bgi -- --profile kernel32-tls-get-value`は30 contracts／69 calls、
70 writebacks／116 dirty pages／1,180 changed bytes、32 host deltas、final generation `131`を通す。
instruction `1703`でdynamic endpoint ID `2`／address `0x7F020010`の`FlsGetValue(0)`を
`dynamic-export-contract-missing`として停止する。Memory SHA-256は
`3b2a4d71de4d796d93b92b16a2cb0ab1b62b45cab0cdaf173c5d6f09895736ba`、316,360-byte
reportのSHA-256は`f14401c55cac1c5247050aad297cc19e35cdd2b4f772ccb842568e0de21ee2d9`である。

Chrome Runtime Workerも69 ordered callsとsame stop／memoryを再現し、242 slices、172 yields、
303 Wasm crossings、final generation `249`、cancel／watchdog後pending work 0を通す。3分gateは
2,145 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-tls-get-value.md](bgi-tls-get-value.md)を正とする。

## 64. BGI FlsGetValue

focused `fls-lifecycle` observerのaccepted complete capturesは441,333
bytes／1,441 lines／SHA-256
`318c76a6ce0da8c7c47d335957876a343e74d2b4bf8f82e013407671e615471e`と
434,981 bytes／1,421 lines／SHA-256
`0eb6161706e16a84f97325b35021ea2db0084893a8d14e55b929f3b146cf69d2`
である。Summariesは685／675 balanced API pairsとdiagnostics zeroを保持する。Timeout時に
unrelated `CreateThread` hookがopenだった中間captureは除外した。

両runの最初のBGI callはreturn RVA `0xA55EC`で、preceding `FlsAlloc`のindexと
same-thread `FlsSetValue`のvalueを使う。Resultはstored valueと一致し、LastErrorは`0 -> 0`を
保つ。Immediate consumerは`mov esi,eax`、`test esi,esi`、nonnull success branchであり、NULL時だけ
`(flags=1,size=0x214)`のlocal private-heap helperへfallbackする。

Controlled oracleはset value `0x12345678`をsame value／LastError `0`、allocated-unsetを
NULL／LastError `0`、invalid `0xFFFFFFFF`をNULL／LastError `87`として固定する。Separate
3,072-byte PE32/i386 fixtureのSHA-256は
`82b7074bb21b71a81af6520888bd7f3d4a930174b2afecec0289a31d91b64f66`で、native exit code `0`を返す。
New workerはstorage未初期化のためinitial NULL／LastError `87`となり、set後はown value
`0x23456789`／LastError `0`を返す。Main thread value `0x12345678`は不変である。

Runtimeの`kernel32.fls-get-value.v1`はpolicy-independentなone-argument stdcall contractである。
Process-owned allocated-index registryとselected-thread storage initializationを検査し、set／unset
successはLastError `0`、unallocated indexまたはuninitialized thread storageはLastError `87`を返す。
FLS mapとguest memoryを変更せず、BGI reached index／value／callerはhandlerへ埋め込まない。

`npm run check:runtime:bgi -- --profile kernel32-fls-get-value`は31 contracts／70 calls、71
writebacks／117 dirty pages／1,183 changed bytes、32 host deltas、final generation `132`を通す。
instruction `1709`でID `154`／`SetLastError(87)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`de703069e1242705785faa744b240ab843cf6ee1aa7fa78ddf4ea835e2c4e811`、320,070-byte reportの
SHA-256は`f7672697a5f95f709fc242ce1c31b8f95039152a0d06f516aa042c8dafcacbcf`である。

Chrome Runtime Workerも70 ordered callsとsame stop／memoryを再現し、243 slices、172 yields、
305 Wasm crossings、final generation `250`、cancel／watchdog後pending work 0を通す。3分gateは
2,181 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-fls-get-value.md](bgi-fls-get-value.md)を正とする。

## 65. BGI SetLastError

focused `set-last-error` observerのcomplete capturesは1,889,260
bytes／5,671 lines／SHA-256
`aff200fc1e726988d2fbaee436959835572eef5fb5d99e50fe8d2d5a2afd1022`と
1,892,095 bytes／5,680 lines／SHA-256
`59c64d74d12510264e50c282b7f4caaffc47b29c8928dd969137a9dd5fd00841`
である。Summariesは2,800／2,804 balanced API pairs、max depth `4`、diagnostics zeroを保持する。

最初のBGI callはreturn RVA `0xA5647`、argument `87`でLastErrorを`0 -> 87`へ変更する。
Immediate consumerは`pop edi`、`mov eax,esi`、`pop esi`、`ret`でhelperをunwindする。
Native EAXはcallでclobberされ、absolute entry／exit値もrun間で異なるが、consumerがESIから
直ちに上書きするためsemantic oracleにしない。

Controlled x86 oracleは`0`、`87`、`122`、`0x12345678`の全sentinelでhook argument、
hook leave LastError、`SystemFunction` captureがfull 32-bit値と一致することを固定する。
Runtimeの`kernel32.set-last-error.v1`はpolicy-independentなone-argument VOID stdcall contractで、
generic dispatcherの`FromHandler` policyからselected-thread LastErrorだけを更新する。Guest writeと
memory generation changeはなく、VOID EAX preserveはnative resultの再現を主張しない。

`npm run check:runtime:bgi -- --profile kernel32-set-last-error`は32 contracts／81 calls、82
writebacks／132 dirty pages／1,370 changed bytes、34 host deltas、final generation `145`を通す。
`SetLastError(87)`はinstruction `1709`／`1777`／`1897`の3回をreturnし、instruction `1936`で
ID `156`／`GetACP()`を`endpoint-contract-missing`として停止する。Memory SHA-256は
`3f212a5faa9d76896ed9f464e344aafe7c1fae60baae03856815a4c89cb417fe`、363,609-byte reportの
SHA-256は`a915d7b91d65e6b1d4805bc4f0ab62d6444573693402bed9fd473518f56f9657`である。

Chrome Runtime Workerも81 ordered callsとsame stop／memoryを再現し、274 slices、192 yields、
344 Wasm crossings、final generation `278`、cancel／watchdog後pending work 0を通す。3分gateは
2,086 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-set-last-error.md](bgi-set-last-error.md)を正とする。

## 66. BGI GetACP

focused `get-acp` observerのaccepted complete capturesは269,228 bytes／875
lines／SHA-256
`91030f4ca8afb53e285739bdacadc8d3e8bef7ee35730eb3b2d605529e6363f3`と269,229
bytes／875 lines／SHA-256
`a5f3c00132344a43c51467ef4f3e83309edd49140fd6f2391891c84aea54b2b4`
である。Summariesはどちらも401 balanced API pairs、max depth `3`、one oracle、
diagnostics zeroを保持する。

BGI callはreturn RVA `0xA6F72`、argumentなし、result `932`、LastError
`87 -> 87`である。Local cleanup RVA `0xA6F4F`はEAXを保ってreturnし、
outer frame caller RVA `0xA71AF`は`mov [ebp+8],eax`で値を保存した後、
`cmp eax,[ebx+4]`でcached active code pageと比較する。

Controlled x86 oracleはLastError sentinel `0`、`87`、`122`、`0x12345678`の
全callでresult `932`が安定し、hookと`SystemFunction`のLastErrorが各sentinelを
保つことを固定する。Runtimeの`kernel32.get-acp.v1`はzero-argument U32 stdcall
contractで、versioned compatibility policyが所有するactive ANSI code pageだけを読む。
LastError／guest memory／runtime stateは変更せず、policy-disabled時はmutation前に停止する。

`npm run check:runtime:bgi -- --profile kernel32-get-acp`は33 contracts／82 calls、83
writebacks／134 dirty pages／1,430 changed bytes、34 host deltas、final generation `146`を通す。
instruction `1978`でID `75`／`HeapAlloc(0x68010001,0,0x220)`を
`heap-alloc-variant-unsupported`として停止する。Memory SHA-256は
`7ed5262e4eef60c3f665765d5741872b595205c5022193ec4633194e5ffa25ea`、367,384-byte
reportのSHA-256は`b897324ff6b85383b2b51be1505e84868e53d9fa5488e5d8f0f4da0ed36b476d`である。

Chrome Runtime Workerも82 ordered callsとsame stop／memoryを再現し、280 slices、197 yields、
351 Wasm crossings、final generation `284`、cancel／watchdog後pending work 0を通す。3分gateは
2,066 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-get-acp.md](bgi-get-acp.md)を正とする。

## 67. BGI private-heap 544-byte allocation

focused `heap-allocation` observerのaccepted capturesは361,127 bytes／169
lines／SHA-256
`da2eb2df7be81130ccb52c37be0573a6b04f5d4563fefe18d22f458951195b08`と
361,343 bytes／169 lines／SHA-256
`1daa8f2f1ca3766c6cdc078aee3557cb5aa7f37b27d28e1b004ec83279e001b9`
である。どちらも59 balanced API pairs、diagnostics zeroを保持する。Observerはmain
executable direct callerだけを残し、bounded returned-buffer fingerprint、x86 EBP caller
chain、tracked allocationから`HeapFree`へのrelationを記録する。2秒の中間captureは終了時に
unrelated later `HeapAlloc`がopenだったため除外した。

BGI return RVA `0xA4A33`の`HeapAlloc(nativeHeap,0,544)`は両runでnonnull resultを返し、
LastError `0 -> 0`を保つ。Returned 544 bytesはいずれも519 NUL bytesだがfingerprintは
`0xDDE20A18`／`0xC8C82863`で一致しないため、exact native initializationをsemantic oracleに
しない。Frame depth 1のconsumer RVA `0xA71C5`はresultをnonnull検査し、`ECX=0x88`の
`rep movsd`で544 bytes全体を上書きし、first DWORDをclearしてsame pointerを次関数へ渡す。
Accepted capture内にrelated `HeapFree`はなく、consumer中のlivenessだけを採用する。

Policy `kernel32-private-heap-size-544-v1`はexisting `kernel32.heap-alloc.v2`へ
`(flags=0,size=544)`だけを追加する。Runtimeはallocation ID `4`／guest `0x20000A20`／
requested／reserved 544／deterministic-uninitializedを保持する。Allocation delta sequence
`83`はgeneration `146 -> 147`のdirty-only pageであり、following full-buffer consumerは
writeback sequence `84`／generation `147 -> 148`でstack／heap 617 changed bytesをreconcileする。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-size-544`は33 contracts／87 calls、
88 writebacks／142 dirty pages／2,300 changed bytes、35 host deltas、final generation `152`を通す。
instruction `4523`でID `155`／`GetCPInfo(932,0x006FF9C0)`を
`endpoint-contract-missing`として停止する。Memory SHA-256は
`c420169199f66f0bec6dc619b5d694e1f4987ea1990ae98f4d6a7773c63fcb26`、386,921-byte
reportのSHA-256は`355c6c2dfc089c8d6f1ca30ea6cbe3b697f4e7afdfa6ed1ec4f2b0ea15593781`である。

Chrome Runtime Workerも87 ordered callsとsame stop／memoryを再現し、599 slices、511 yields、
663 Wasm crossings、final generation `589`、cancel／watchdog後pending work 0を通す。3分gateは
1,723 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-private-heap-size-544.md](bgi-private-heap-size-544.md)を正とする。

## 68. BGI GetCPInfo

focused `get-cp-info` observerのaccepted capturesは29,775 bytes／68 lines／SHA-256
`5689d985a0d780efed7d449144347b73421125aec337e8570f7c8463dc795f46`と
29,849 bytes／68 lines／SHA-256
`b930cdafb0443759a3a6db3cbaf6b549ba327de2d85725cf35456c49ea4fc0ef`
である。どちらも8 balanced API pairs、diagnostics zeroを保持する。Observerはdirect BGI
callとguarded x86 self-oracleだけを残し、32 raw bytes、LastError、return continuation、four
bounded EBP framesを記録する。

BGI return RVA `0xA6D13`の`GetCPInfo(932,stack)`はresult `1`、LastError `87 -> 87`である。
Guarded oracleはsentinel `0`／`87`／`0x12345678`の全callでsuccessとLastError preserveを
固定し、先頭18 bytesだけが`02 00 00 00 3F 00 81 9F E0 FC 00 00 00 00 00 00 00 00`へ
書かれ、following 14 bytesと両guardが不変であることを示す。Measured fieldsは
MaxCharSize `2`、DefaultChar `[0x3F,0]`、lead ranges `0x81..0x9F`／`0xE0..0xFC`である。
BGI consumerはBOOLを検査し、256-byte tableをclearしてこのinclusive rangeを展開する。

Policy `kernel32-get-cp-info-v1`はactive Windows-932 metadataだけを保持し、
`kernel32.get-cp-info.v1`はexplicit page一致、4-byte alignment、18-byte writable spanを
preflightしてTRUEを返す。Other page、policy-disabled、cross-region outputはmutation前に停止する。
Output host delta sequence `88`はgeneration `152 -> 153`のone stack page、consumer
writeback sequence `89`はgeneration `153 -> 154`／307 changed bytesである。

`npm run check:runtime:bgi -- --profile kernel32-get-cp-info`は34 contracts／92 calls、93
writebacks／148 dirty pages／2,690 changed bytes、36 host deltas、final generation `158`を通す。
instruction `5793`でID `130`／
`MultiByteToWideChar(932,1,0x006FFDD4,256,0,0,256)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は`1d0a6694d155aed6f9b12989c7d78ba0f6017c556749f218b9b131ac199eb389`、
407,009-byte reportのSHA-256は
`da95600680cdb922a2952a7045ac5ace1b6f2510f5f553773343854e2f2310a1`である。

Chrome Runtime Workerも92 ordered callsとsame stop／memoryを再現し、759 slices、666 yields、
822 Wasm crossings、final generation `741`、cancel／watchdog後pending work 0を通す。3分gateは
1,587 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-get-cp-info.md](bgi-get-cp-info.md)を正とする。

## 69. BGI MultiByteToWideChar

focused `multi-byte-to-wide-char` observerのaccepted capturesは50,395
bytes／84 lines／SHA-256
`99d1e7d520a13f840cc5d21885beb392c3ef018a62036c4af2573a8866192275`と
50,419 bytes／84 lines／SHA-256
`15c9640167a33d93146cd0b014eddf2efa78793c53c334d391c8d5a2f721cf49`
である。どちらも16 balanced API pairs、diagnostics zeroを保持する。APIはsix-argument
x86 stdcallであり、preceding unsupported stack snapshotのseventh wordは引数ではない。

BGI RVA `0xAD691`のqueryは`(932,1,source,256,NULL,0)`で256を返し、RVA
`0xAD6FB`のconversionはsame sourceと256-unit destinationへ256を返す。Sourceは
FNV-1a `0x80472A65`のstable 256 bytes、outputは512 bytes／SHA-256
`4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`
である。成功はrunごとのLastError `0`／`87`を保つ。Native Windows-932はsingle
`0xA0`を`U+F8F0`、`0xFD..0xFF`を`U+F8F1..U+F8F3`へ写し、WHATWG replacementと
異なる。OracleはJapanese pair、NUL、success preserve、insufficient-buffer prefix＋`122`、
zero count＋`87`を固定する。

Policy `kernel32-multi-byte-to-wide-char-v1`はallowed flags `[1]`と65,536-byte boundだけを
追加する。`kernel32.multi-byte-to-wide-char.v1`はchecked source／destination、query／conversion、
overlap、partial buffer、Windows private-use mappingsをsemantic contractとして処理し、targetの
pointer／256-byte value／callerをpolicyへ含めない。

`npm run check:runtime:bgi -- --profile kernel32-multi-byte-to-wide-char`は35 contracts／94
calls、95 writebacks／150 dirty pages／2,741 changed bytes、37 host deltas、final generation
`161`を通す。instruction `5894`でID `108`／
`GetStringTypeW(1,0x006FF738,256,0x006FF9D4)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は`eb77024dd956d49c9a1a9f89a0e248220cd6ab6850203978134aeb93b8c4e811`、
423,814-byte reportのSHA-256は
`97655782894eca5f4e97a095d1ee3b64619e4f5ecf076c2be91d9b79ca045718`である。

Chrome Runtime Workerも94 ordered callsとsame stop／memoryを再現し、772 slices、677 yields、
835 Wasm crossings、final generation `751`、cancel／watchdog後pending work 0を通す。3分gateは
1,482 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-multi-byte-to-wide-char.md](bgi-multi-byte-to-wide-char.md)を正とする。

## 70. BGI GetStringTypeW

focused `get-string-type-w` observerのaccepted capturesは54,930 bytes／76 lines／SHA-256
`579835aa3610b6ac7550d98598eeb465930ae4c71633fdcdd84401dee0b494de`と
54,934 bytes／76 lines／SHA-256
`ffca1c677d9c6a1b38af85c36e2bc6f98f922ec57a53935ac77bf613a1a14e2`
である。どちらも12 balanced API pairs、diagnostics zeroを保持する。Observerはdirect BGI
callとguarded x86 self-oracleだけを残し、bounded UTF-16 source、WORD output、LastError、
return continuation、four bounded EBP framesを記録する。

BGI return RVA `0xAD70D`の`GetStringTypeW(1,source,256,output)`はresult `1`で、runごとの
LastError `0`／`87`を保存する。Sourceは188 ASCII＋68 non-ASCII BMP units、NUL／surrogate
zeroで、FNV-1a `0x0F08DD60`／SHA-256
`4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`である。512-byte
outputはFNV-1a `0x1AB401A9`／SHA-256
`8eea87edb52710ebeee687bb46f65962ffaf7ad306d9d0e98bc81929b1af6d84`である。
ConsumerはBOOLをstore／testし、RVA `0xA71ED`付近でsuccess continuationへbranchする。

OracleはASCII case／digit／hex、control／whitespace、punctuation、Japanese／CJK、
fullwidth／halfwidth、Windows-932 private-use unitのrepresentative CTYPE1 wordsを固定する。
SuccessはLastError `0`／`87`／`0x12345678`を保存する。Zero countはFALSE＋`87`、invalid
information typeはFALSE＋`1004`で、いずれもoutput／guard不変である。Nativeはnegative
countをNUL-terminated inputとして受理するがtarget未到達のため実装へ昇格しない。

Policy `kernel32-get-string-type-w-v1`はallowed information type `[1]`と65,536-unit boundだけを
追加する。`kernel32.get-string-type-w.v1`はfour-argument stdcall、checked source／output、
positive explicit length、CTYPE1 classification、success LastError preserve、zero-count failureを
semantic contractとして処理する。Negative length、other info type、overlap、unimplemented unitは
mutation前にstructured stopし、target pointer／256-unit value／callerをpolicyへ含めない。

`npm run check:runtime:bgi -- --profile kernel32-get-string-type-w`は36 contracts／101 calls、
102 writebacks／159 dirty pages／2,958 changed bytes、39 host deltas、final generation `170`を通す。
instruction `7675`でID `96`／`LCMapStringW(1041,256,0x006FF708,256,0,0)`を
`endpoint-contract-missing`として停止する。Memory SHA-256は
`3c26a9fa98427249ef0f37546f25592d18d18e9ade8c07c4ebbd10e2ce6b9aa2`、468,010-byte reportの
SHA-256は`00facfca5960a6e0e233deab536aa5d01e190d6e7f72e6be7caffb2e8997c425`である。

Chrome Runtime Workerも101 ordered callsとsame stop／memoryを再現し、998 slices、896 yields、
870 Wasm crossings、final generation `778`、cancel／watchdog後pending work 0を通す。3分gateは
1,507 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-get-string-type-w.md](bgi-get-string-type-w.md)を正とする。

## 71. BGI LCMapStringW

focused `lc-map-string-w` observerのaccepted capturesは80,889 bytes／84 lines／SHA-256
`f6688a93a53991e866782e6cd9ae4cb8f6cf086a8b5ba42325eeea4376203d02`と
80,851 bytes／84 lines／SHA-256
`eb6507ae0d35080711a2ecd95472138b224d882c8b6d45c35aa5686f4bd3f61c`
である。どちらも16 balanced API pairs、diagnostics zeroを保持する。Observerはdirect BGI
callとguarded x86 self-oracleだけを残し、six arguments、bounded UTF-16 source／output、
LastError、guards、two return continuations、bounded EBP framesを記録する。

BGIはlocale `1041`、explicit length `256`でlowercase (`0x100`)／uppercase (`0x200`)の
queryとconversionを各1回呼び、全callが256を返してLastError `87`を保存する。Query return
RVAは`0xA7F81`、conversion return RVAは`0xA8020`である。Sourceは188 ASCII＋68
non-ASCII BMP units、NUL／surrogate zeroで、FNV-1a `0x0F08DD60`／SHA-256
`4684daa1a9839ae6e158ace6c2fcf905ff97e52a684d3f0e808cce56fddf6b00`である。
Lowercase outputはFNV-1a `0x16715DE0`／SHA-256
`2dbfb1ab3f867c8f7cc233c63a3749f303dbf5b7471e5ddf888bb074b043242c`、uppercase
outputはFNV-1a `0x4B0046E0`／SHA-256
`34c6ffd61beff429b3655fa26536beb9f81f0baaca883c24ea5dfb4443c8f7d9`である。

OracleはASCII、Japanese、fullwidth caseのquery／conversion、NUL-terminated input、
negative count `-2`、success LastError preserveを固定する。Insufficient destinationは0＋`122`、
zero countとNULL destination＋nonzero capacityは0＋`87`、flags 0は0＋`1004`で、source、
output、guardsはmutation-freeである。Nativeはnegative countをNUL-terminatedとして受理するが、
target未到達のため実装へ昇格しない。Query consumerはresult／capacityを検査してdestinationを選び、
conversion consumerはresultを検査してmapped bufferを`WideCharToMultiByte`へ渡す。

Policy `kernel32-lc-map-string-w-v1`はlocale `[1041]`、exact flags `[256,512]`、
65,536-unit boundだけを追加する。`kernel32.lc-map-string-w.v1`はsix-argument stdcall、checked
source／destination、explicit positive length、query／conversion、host-locale-independent reached
case mapping、success LastError preserve、insufficient／invalid shape failureをsemantic contractとして
処理する。Negative count、other locale／flags、overlap、unimplemented unitはmutation前にstructured
stopし、target pointer／256-unit value／callerをpolicyへ含めない。

`npm run check:runtime:bgi -- --profile kernel32-lc-map-string-w`は37 contracts／113 calls、
114 writebacks／174 dirty pages／3,350 changed bytes、44 host deltas、final generation `187`を通す。
instruction `12137`でID `123`／`InterlockedDecrement(0x100F08E0)`を
`endpoint-contract-missing`として停止する。Memory SHA-256は
`00d38b3de81df69fb459a3ff8ad7f0b53e1e50317f1fc6b546efabab19f20b55`、550,988-byte
reportのSHA-256は`d56849eac369c5ad5b946c06b27acba844b779e548795efcc464a67bc2c7a889`
である。

Chrome Runtime Workerも113 ordered callsとsame stop／memoryを再現し、1,560 slices、1,446 yields、
1,207 Wasm crossings、final generation `887`、cancel／watchdog後pending work 0を通す。3分gateは
1,200 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-lc-map-string-w.md](bgi-lc-map-string-w.md)を正とする。

## 72. BGI InterlockedDecrement

focused `interlocked-decrement` observerのaccepted capturesは51,155 bytes／70 lines／
SHA-256 `870cd630131c437bc65ef0818bacb87b4599e01c87bf19ed93d6d9a071c66be9`と
51,148 bytes／70 lines／SHA-256
`5bc3f4b68f74964e55def6bf33a31fbf4bd6fc87134c5d3f29a135a6a24d9c25`
である。どちらも9 balanced API pairs、contiguous sequence、diagnostics zeroを保持する。
Observerはtwo direct BGI calls、three guarded x86 self-oracle calls、four incidental
`GetProcAddress` pairsだけを残し、pointed LONG before／after、signed result、LastError、
alignment、guards、two return continuationsとimmediate consumersを記録する。

BGI targetはguest `0x100F08E0`／RVA `0xF08E0`で4-byte alignedである。RVA
`0xA7201`へ戻るfirst callは`1 -> 0`／result `0`、RVA `0xA72B8`へ戻るsecond
callは`0 -> -1`／result `-1`で、LastError `87`または`0`を保存する。First
consumerはEAX zeroを検査してretained process pointerのrelease／replaceを選び、second
consumerはEAXと後続`-1`比較からrelease／error pathを選ぶ。

Guarded oracleはaligned targetについて`1 -> 0`、`0 -> -1`、`INT_MIN -> INT_MAX`
と同じresultを固定し、LastError `0`、`87`、`0x12345678`を保存する。全leading／
trailing 16-byte guardsは不変である。Undefinedなnative unaligned behaviorはruntime
contractへ昇格しない。

`kernel32.interlocked-decrement.v1`はone-argument stdcall、required aligned `InOut
LONG`、wrapping decrement、new LONG result、success LastError preserveをpolicy-independent
semantic contractとして処理する。Increment／decrementはshared checked LONG helperを使う。
NULL、unaligned、unmapped、read-only targetはguest bytes、runtime state、register、LastErrorの
mutation前にstructured stopする。Current single cooperative Worker lane内のindivisible updateは
保証するが、cross-Worker shared-memory atomicityはclaimしない。

`npm run check:runtime:bgi -- --profile kernel32-interlocked-decrement`は38 contracts／
119 calls、120 writebacks／187 dirty pages／3,640 changed bytes、50 host deltas、final
generation `199`を通す。instruction `15863`でID `34`／
`GetModuleFileNameA(NULL,0x100FF520,260)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`83a1a4fd36f1f94efc7c0dcd83f1ff12f35729a00158ef7b48746f51896228f3`、
580,772-byte reportのSHA-256は
`6c905456197834909eb965cfa950c05a4587a91486b7b9c72f55253340bb9ff4`である。

Chrome Runtime Workerも119 ordered callsとsame stop／memoryを再現し、2,029 slices、
1,909 yields、1,687 Wasm crossings、final generation `1,361`、cancel／watchdog後pending
work 0を通す。3分gateは1,004 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-interlocked-decrement.md](bgi-interlocked-decrement.md)を正とする。

## 73. BGI GetModuleFileNameA

focused `get-module-file-name-a` observerのaccepted capturesは116,515 bytes／80 lines／
SHA-256 `dbea36944098cc0ade8c4d76e916447aa7563c7b97d53026c3601cb91edd5cae`と
116,457 bytes／80 lines／SHA-256
`5e52ed288cb692914ecfc542b904b7b0efc8d638968934a027d80cbaca8cee88`
である。どちらも14 balanced API pairs、sequence `0..77` contiguous、diagnostics zeroを
保持する。Observerはtwo direct BGI calls、eight guarded x86 self-oracle pairs、four
incidental `GetProcAddress` pairsだけを残す。

BGI callsはNULL module、capacity `260`／`780`を渡し、RVA `0xA84C6`／`0x60219`
へ戻る。双方result `27`／LastError `0 -> 0`で、Windows-932 path
`V:\softs\サクラノ詩\BGI.exe\0`を返す。Path bytesのSHA-256は
`72472c80161f0c142cee81058bcdfa1a43c25576788b35e0cb9fd0991a041b3c`である。
First destinationはimage RVA `0xFF520`、secondはthread stackで、両consumerはpathを
後続処理へ保持する。

Guarded oracleはgenerous／exact success、capacity `27`／`1`／`0`、NULL output、explicit
KERNEL32、invalid handleを固定する。Modern truncationはresultをcapacityとし、最終byteを
NUL化してLastError `122`を返す。Explicit KERNEL32は32-byte path、invalid handleはresult
`0`／LastError `126`／output不変であり、全guardsも不変である。Explicit-module behaviorは
BGI未到達のためcurrent runtime policyへ昇格しない。

`kernel32.get-module-file-name-a.v1`はthree-argument stdcall、main-module-only policy、
32 KiB capacity bound、full output-span preflight、length／termination／LastErrorを処理する。
Rust bootstrapが`windows-932-v1`のvirtual path `C:\Vine\BGI.exe\0`を所有し、host install
pathをsemantic inputにしない。Zero capacityはoutputを参照せず`0`／`122`、positive capacity
のNULL outputは`0`／`87`を返す。Explicit module、capacity limit、invalid guest spanは
mutation前にstopする。

`npm run check:runtime:bgi -- --profile kernel32-get-module-file-name-a`は39 contracts／
152 calls、153 writebacks／229 dirty pages／3,920 changed bytes、51 host deltas、final
generation `233`を通す。Reached call sequence `120`は16-byte pathを`0x100FF520`へ書き、
generation `199 -> 200`へ進む。instruction `16965`でID `75`／
`HeapAlloc(0x68010001,0,16)`を`heap-alloc-variant-unsupported`として停止する。Memory
SHA-256は`b233490f03ec30f568aa6ddf9868da4f41804150f4bbccfff8a20f2e9d01e943`、
704,797-byte reportのSHA-256は
`3bbeda2ffff7720afc73c7729b52f7168b2989758baf6c7cc7e6a45b870e15e7`である。

Chrome Runtime Workerも152 ordered callsとsame stop／memoryを再現し、2,174 slices、
2,021 yields、1,826 Wasm crossings、final generation `1,445`、cancel／watchdog後pending
work 0を通す。3分gateは961 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-get-module-file-name-a.md](bgi-get-module-file-name-a.md)を正とする。

## 74. BGI private-heap 16-byte allocation

combined `get-module-file-name-a`／`heap-allocation` observerのaccepted capturesは
464,225 bytes／190 lines／SHA-256
`6f0ed9c6a2b26576f1d2ba4d291ae6b7b5b862fa71e67a2f2780734bebe786ce`と
464,304 bytes／190 lines／SHA-256
`551e58fabb2e2ad97cb8b977c76945eef7bf636b7b84dfee3180ab8a77aa8cc1`
である。どちらも188 contiguous events、69 balanced API pairs、diagnostics zeroを保持し、
sequence `83`の`GetModuleFileNameA` leaveに続いてsequence `84`／`85`でdirect BGI
`HeapAlloc` enter／leaveを記録する。

Native install pathは日本語5文字を含むため、BGI return RVA `0xA4A33`のcallは
`HeapAlloc(nativeHeap,0,36)`であり、non-NULLを返してLastError `0 -> 0`を保存する。
Allocation return RVA `0xA8517`ではresultをESIへ保持し、NULL test後にcomputed upper boundと
ともにpath conversionへ渡す。36-byte initial bufferは両runとも30 NUL bytesを含むが、
FNV-1aは`0x1B0CEF1D`／`0x5128B3FD`で異なる。100 ms capture内にrelated
`HeapFree`はない。従ってnative path由来size 36と不安定なinitial bytesはruntime policyへ
輸入せず、controlled ASCII path `C:\Vine\BGI.exe\0`から到達するsize 16だけを有効化する。

Policy `kernel32-private-heap-size-16-v1`はexisting `kernel32.heap-alloc.v2`のflags-zero
listを`[1,16,544]`へ拡張する。Reached allocationはID `5`／block `0x20000C40`、
requested／reserved 16 bytes、`deterministic-uninitialized`である。Host delta sequence
`153`はgeneration `233 -> 234`のdirty-only one-page update、following consumer writeback
sequence `154`はinstruction `17061`／generation `234 -> 235`／55 changed bytesである。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-size-16`は39 contracts／
185 calls、186 writebacks／272 dirty pages／4,178 changed bytes、52 host deltas、final
generation `267`を通す。instruction `18143`でID `75`／
`HeapAlloc(0x68010001,8,4)`を`heap-alloc-variant-unsupported`として停止する。Memory
SHA-256は`b648580e9ea1840e7c2703115d144e3b7014040f74a6670a14136808ee2782ff`、
828,414-byte reportのSHA-256は
`887a74c11a1466cfbbfdeebf126ecc6138588c4b1f9e89c39cfdbb290a2ee8e6`である。

Chrome Runtime Workerも185 ordered callsとsame stop／memoryを再現し、2,325 slices、
2,139 yields、1,969 Wasm crossings、final generation `1,538`、cancel／watchdog後pending
work 0を通す。3分gateは918 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-private-heap-size-16.md](bgi-private-heap-size-16.md)を正とする。

## 75. BGI private-heap 4-byte allocation

focused `heap-allocation` observerのaccepted capturesは361,115 bytes／168 lines／
SHA-256 `9ab295457772c0a71f93cafb32f054f45dd0b64a8d07c21fb72f0aff30f15ed6`と
360,997 bytes／168 lines／SHA-256
`2836f8c51c84cab7690e0f66a4ec953ddeae9802669710981f0ff2d3f6c12c47`
である。どちらも166 contiguous events、59 balanced API pairs、diagnostics zeroを保持する。
Sequence `64`／`65`のpath用flags-zero 36-byte allocation直後、sequence `66`／`67`で
environment pointer tableのdirect BGI allocationを記録する。

Native callはRVA `0xAC3CC`へ戻る`HeapAlloc(nativeHeap,8,188)`で、non-NULLを返して
LastError `0 -> 0`を保存する。全188 bytesは両runでNUL、FNV-1a `0x67691275`である。
RVA `0xA821E`のcallerはANSI environment blockを走査し、先頭`=`でない46 entriesを数え、
NULL terminatorを1 slot加えてfour-byte pointer widthとchecked multiplyする。従ってnative
requestは`(46 + 1) * 4 = 188`である。Controlled empty environmentはzero entriesなので、
same guest codeのrequestは`(0 + 1) * 4 = 4`となる。Host environmentとsize 188はruntimeへ
importしない。

Consumer RVA `0xA8261`はresultをglobalへstore／NULL testし、各native entryのcopy pointerを
four-byte slotへ書き、最後のDWORDをNULL化する。Failure pathだけはtableをfreeしてglobalを
clearする。Accepted capture内にsuccessful tableとrelatedな`HeapFree`はない。

Policy `kernel32-private-heap-size-4-v1`はexisting `kernel32.heap-alloc.v2`のzeroed listへ
size `4`だけを追加する。Reached allocationはID `6`／block `0x20000C50`、requested 4／
reserved 8 bytes、zeroedである。Host delta sequence `186`はgeneration `267 -> 268`の
dirty-only one-page update、consumer writeback sequence `187`はinstruction `18180`／
generation `268 -> 269`／16 changed bytesである。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-size-4`は39 contracts／
186 calls、187 writebacks／274 dirty pages／4,194 changed bytes、53 host deltas、final
generation `269`を通す。instruction `18180`でID `76`／
`HeapFree(0x68010001,0,0x20000A18)`を`endpoint-contract-missing`として停止する。Memory
SHA-256は`d9abc780ef3d253810695351cb66296b1aeb727e7e8590f73fe20e2c1c73cd2c`、
833,007-byte reportのSHA-256は
`80f28db63bf8400b7ea7d1db5d7552303dec05dc71addb1ac2ed1ecea29eefa1`である。

Chrome Runtime Workerも186 ordered callsとsame stop／memoryを再現し、2,330 slices、
2,143 yields、1,974 Wasm crossings、final generation `1,542`、cancel／watchdog後pending
work 0を通す。3分gateは627 sessions／failure 0で全16 thresholdを通した。詳細は
[bgi-private-heap-size-4.md](bgi-private-heap-size-4.md)を正とする。

## 76. BGI private-heap one-byte allocation free

同じ2件のaccepted focused heap capturesは、sequence `60/61`のcall ID `7`で
`HeapAlloc(nativeHeap,0,3637)`を記録し、sequence `160/161`のcall ID `57`で
exact same heap／pointerを`HeapFree(...,0,pointer)`へ渡す。Free observerの
`relatedAllocationCallId`は両runとも`7`、requested bytesは`3637`、resultは`1`、
LastErrorは`0 -> 0`である。Allocation直後のuninitialized fingerprintは
`0x02DDAABF`／`0x0406D700`と異なるが、free直前は両runでFNV-1a
`0xA8AD592C`／NUL 47 bytesへ収束する。Native heap／pointer数値とhost-environment
由来sizeはruntimeへimportしない。

Fingerprint済みimageのRVA `0xA48CD`は`test eax,eax; jne success`であり、failure
branchだけが`GetLastError`を読む。Policy
`kernel32-private-heap-free-size-1-v1`はHeapFree flags `0`だけを有効化し、
`kernel32.heap-free.v1`はallocation ID `3`／`0x20000A18`／reserved 8 bytesを
existing transactional heap freeへ接続する。SuccessはBOOL `TRUE`、LastError preserve、
free delta sequence `187`／generation `269 -> 270`／dirty-only owner 10 page
`0x20000000`／payload SHA-256
`93cb85efb43c4586535cabdedf112e538d7fbe792245ed8954482b9815aa092a`
である。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-free-size-1`は40
contracts／197 calls、198 writebacks／300 dirty pages／4,423 changed bytes、54 host
deltas、final generation `281`を通す。Ten existing `EncodePointer` calls後、instruction
`18653`でID `75`／`HeapAlloc(0x68010001,8,128)`を
`heap-alloc-variant-unsupported`として停止する。Memory SHA-256は
`f248e7dafa44a4a036822bd2eda199959e01c5d39090f67fba5f9f15c0de0186`、
875,554-byte reportのSHA-256は
`e0b255e0112fb7d0cbcef12afcd91be63877049fed56fc30b62ce5123f1acd96`である。

Chrome Runtime Workerも197 ordered callsとsame stop／memoryを再現し、2,392 slices、
2,194 yields、2,027 Wasm crossings、final generation `1,582`、cancel／watchdog後
pending work 0を通す。3分gateは615 sessions／failure 0で全16 thresholdを通した。
NULL／interior／foreign／double-freeのmutation-free rejection、freed byte `0xDD`、reuse
identityはpublic semantic testで固定する。詳細は
[bgi-private-heap-free-size-1.md](bgi-private-heap-free-size-1.md)を正とする。

## 77. BGI private-heap 128-byte allocation

同じ2件のaccepted focused heap capturesは、直前のfreeに続くsequence `162/163`、
call ID `58`で`HeapAlloc(sameNativeHeap,8,128)`を記録する。Return RVAは
`0xAC3CC`、resultは両runともnon-NULL、LastErrorは`0 -> 0`、全128 bytesはNUL、
FNV-1aは`0xA7B537C5`である。Native resultはsequence `160/161`でfreeした
3,637-byte blockのaddressを再利用するが、Vineでfree済みなのはreserved 8 bytes
なので、このhost allocator reuseはsemantic inputへimportしない。

Fingerprint済みimageのRVA `0xA1DC3`は`push 4; push 0x20; call 0xA4EAC`で、
RVA `0xAC383`のchecked multiply/allocation helperへ32 elements * 4 bytesを渡す。
RVA `0xA1DCF`のconsumerはresultをESIへ保持し、`EncodePointer`へ渡してtwo
globalsへstoreし、non-NULL success pathでfirst DWORDをzeroにする。Accepted
capture内にこのallocationとrelatedなlater `HeapFree`はない。

Policy `kernel32-private-heap-size-128-v1`はexisting
`kernel32.heap-alloc.v2`へ`(flags=8,size=128)`だけを追加する。Reached
allocationはID `7`／`0x20000C58`、requested／reserved 128 bytes、zeroed、
live allocations `5 -> 6`である。Host delta sequence `198`はgeneration
`281 -> 282`のdirty-only owner-10 page updateで、supported call `199`の
`EncodePointer`後、consumer writeback sequence `200`はinstruction `18693`／
generation `283 -> 284`／14 changed bytesを固定する。

`npm run check:runtime:bgi -- --profile kernel32-private-heap-size-128`は40
contracts／199 calls、200 writebacks／304 dirty pages／4,444 changed bytes、55
host deltas、final generation `284`を通す。instruction `18693`でID `74`／
`IsProcessorFeaturePresent(10)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`5262246193dbe482a09781cac162f4f23f4eabaa4981c5f50ec2c9460ce96a89`、
883,995-byte reportのSHA-256は
`51382513962b57159e178966b16aac2010513a3c58cccaf9b9d6abbfc857d073`である。

Chrome Runtime Workerも199 ordered callsとsame stop／memoryを再現し、2,398
slices、2,198 yields、2,033 Wasm crossings、final generation `1,586`、
cancel／watchdog後pending work 0を通す。3分gateは591 sessions／failure 0で
全16 thresholdを通した。詳細は
[bgi-private-heap-size-128.md](bgi-private-heap-size-128.md)を正とする。

## 78. BGI processor-feature presence query

Focused `processor-feature` captures r3／r4は329,977 bytes／1,043 lines／
SHA-256 `1c0d15bc9a6325cd2a5d3eb8a12b5b901424eb954d1433977de75f4884cef054`と
332,449 bytes／1,055 lines／SHA-256
`819c0266ad0f98f59f05574049f6bdf207dafa413d6d875c21fce1fadd1fdbf5`
である。484／490 API pairsはbalanced、diagnosticsはzeroで、processor-feature
semantic projectionは一致する。

BGIはindex `10`をreturn RVA `0xA4D87`と`0xA7E7C`の2箇所から呼び、どちらも
BOOL `1`、LastError `0 -> 0`を観測する。直後のconsumerはEAXをimage RVA
`0x153518`／`0x153500`へstoreし、zeroを返す。Oracle profile
`x86-processor-feature-query-v1`はindex 10をLastError sentinel `0`／`87`／
`0x12345678`で反復し、全てresult 1とsentinel preserveを確認する。
`0xFFFFFFFF`はresult 0、sentinel 122 preserveである。

Policy `kernel32-is-processor-feature-present-v1`はexplicit
`{index:10,present:true}`だけを所有する。`kernel32.is-processor-feature-present.v1`
はone DWORD stdcall／BOOL32／LastError preserve／mutation-free contractで、未登録
indexは`processor-feature-index-unsupported`として停止する。Public fixtureは別の
explicit false indexもsame handlerで検証し、host CPU discoveryをimportしない。

`npm run check:runtime:bgi -- --profile kernel32-is-processor-feature-present`は41
contracts／202 calls、203 writebacks／311 dirty pages／4,561 changed bytes、56 host
deltas、final generation `289`を通す。2 feature calls後、existing policy-enabled
`HeapAlloc(0x68010001,8,2048)`がallocation ID 8／`0x20000CD8`を返し、one mapping
operationを含むsecond heap-page deltaを適用する。Instruction `18993`でID `78`／
`SetUnhandledExceptionFilter(0x100A816F)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`b4e25674b53cc1e159ee02cff2c00610dfbd3499d716c932ee2cd64fc99362e1`、
896,534-byte reportのSHA-256は
`c173fd9d2ae2763f15ddafa0b487bddeba9b934a978ddd840766cfbad870cdb5`である。

Chrome Runtime Workerも202 ordered callsとsame stop／memoryを再現し、2,437
slices、2,234 yields、2,066 Wasm crossings、final generation `1,614`、
cancel／watchdog後pending work 0を通す。改善後の3分gateは405 sessions／failure 0、
heartbeat P99 14.970 ms／maximum 37.225 msを含む全17 thresholdを通した。詳細は
[bgi-is-processor-feature-present.md](bgi-is-processor-feature-present.md)を正とする。

## 79. BGI unhandled-exception filter replacement

Focused `unhandled-exception-filter` captures r1／r2は2,238,917 bytes／7,031
lines／SHA-256
`d97a220135ce703dd0cfb7be263d264319073d0de3165b5ab6217b6ee2e7664b`と
2,240,926 bytes／7,037 lines／SHA-256
`ebc23df0b5d621490dcf8be5232d6a3510e9c87cab19ab36f428a7daea8a26df`
である。3,478／3,481 API pairsはbalanced、diagnosticsはzeroで、filter replacement
semantic projectionは一致する。

BGIはfilter RVA `0xA816F`をreturn RVA `0xA81BC`から一度登録する。Nativeは
previous filter `NULL`を返し、LastError `0 -> 0`を保存する。wrapperはzeroを返し、
outer consumer RVA `0xA5060`は`test eax,eax`でnonzero previous filterだけを分岐する。
Oracle profile `x86-process-filter-replacement-v1`はcallback AをBへ置換し、`NULL`で
解除してoriginalを復元する。return chain `original, A, B, NULL`、original `NULL`、
LastError sentinels `0`／`87`／`0x12345678`／`122`の保存を両captureで確認した。

`kernel32.set-unhandled-exception-filter.v1`はnullable pointerをone-argument stdcallで
受け、non-NULL guest execute accessをmutation前にpreflightする。filterはRust
`RuntimeProcessState`がprocess-owned stateとして保持し、previous pointerを返して
LastErrorを保存する。Host SEH／JavaScript exceptionへguest callbackを登録せず、exit
request中は保持し、process exit completion／direct terminationで退役する。このvalid
semantic domainにBGI-specific selectorは不要なため、active compatibility policyは
`kernel32-is-processor-feature-present-v1`のままである。

`npm run check:runtime:bgi -- --profile kernel32-set-unhandled-exception-filter`は42
contracts／206 calls、207 writebacks／316 dirty pages／4,690 changed bytes、57 host
deltas、final generation `294`を通す。Call 203でfilterを登録後、existing
`EnterCriticalSection`と2回の`DecodePointer`を完了する。Instruction `19096`でID
`73`／`HeapSize(0x68010001,0,0x20000C58)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`563d391b69ea3dcad1669d68f167bb35fad5b4d48adbd153a9f95ad1fb072f97`、
912,806-byte reportのSHA-256は
`92caa9ef24a63fc1abbac0bfc063361b2c7b9998cd14d2d2ee02e2d5c999defa`である。

Chrome Runtime Workerも206 ordered callsとsame stop／memoryを再現し、2,452
slices、2,245 yields、2,084 Wasm crossings、final generation `1,628`、
cancel／watchdog後pending work 0を通す。3分gateは411 sessions／failure 0、session
P95 526.395 ms、heartbeat P99 16.140 ms／maximum 62.570 msを含む全17 thresholdを
通した。詳細は
[bgi-set-unhandled-exception-filter.md](bgi-set-unhandled-exception-filter.md)を正とする。

## 80. BGI private-heap size query

Focused `heap-size` captures r1／r2は2,248,521 bytes／7,038 lines／SHA-256
`705979af74515682f36f6b5149394a393c9c7099ec211524075efd6052f40952`と
2,247,585 bytes／7,035 lines／SHA-256
`c74e88eb1a51c8e2e1e05d99cdb49510077be0960f0de4eef9ea930fb68d65fa`
である。3,482／3,480 API pairsはbalanced、diagnosticsはzeroで、heap-size
semantic projectionは一致する。

BGIは各captureでreturn RVA `0xA4F77`から5回同じlive allocationを問い合わせる。
Flags 0、earlier requested size 128、result 128、LastError `0 -> 0`が全callで一致する。
Caller RVA `0xA1D4D`はEAXをEBXへ保存し、current capacityと比較して不足時は最低
2,048 bytesのgrowth sizeを計算する。Oracle profile
`x86-private-heap-size-query-v1`はseparate private heapの128-byte allocationでflags
0／`HEAP_NO_SERIALIZE`が同じ128を返してLastErrorを保存すること、interior／freed
pointerが`0xFFFFFFFF`とLastError 87を返すことを両captureで確認した。

Policy `kernel32-heap-size-v1`はflags 0と`HEAP_NO_SERIALIZE`だけを追加する。
`kernel32.heap-size.v1`はRust process-owned heap backingからexact live allocationの
requested byte lengthを返し、aligned reservation／committed capacity／host heapを参照しない。
NULL／interior／foreign／freed pointerはfailure sentinel／87、unknown logical heapと
unlisted flagsはmutation前のstructured stopである。Public fixtureはBGI値と異なる
requested 129／reserved 136-byte allocationで同じhandlerを検証する。

`npm run check:runtime:bgi -- --profile kernel32-heap-size`は43 contracts／210
calls、211 writebacks／324 dirty pages／4,757 changed bytes、58 host deltas、final
generation `299`を通す。Call 207が128を返し、calls 208／209の`EncodePointer`とcall
210の`LeaveCriticalSection`を完了する。Instruction `19185`でID `147`／
`InitializeCriticalSection(0x100FD9CC)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`90c245267e933a01c1e54fa0305bec1d54a16180a3e7465b6a3b6b3c8d3dd0b8`、
929,421-byte reportのSHA-256は
`058ca051ad9b4118ce8854333eafd35aab117dc70da860b28188a2e1afff8fac`である。

Chrome Runtime Workerも210 ordered callsとsame stop／memoryを再現し、2,465
slices、2,254 yields、2,100 Wasm crossings、final generation `1,640`、
cancel／watchdog後pending work 0を通す。3分gateは438 sessions／failure 0、session
P95 536.710 ms、heartbeat P99 9.935 ms／maximum 37.930 msを含む全17 thresholdを
通した。詳細は[bgi-heap-size.md](bgi-heap-size.md)を正とする。

## 81. BGI one-argument critical-section initialization

Focused `critical-section` captures r4／r5は295,301 bytes／524 lines／SHA-256
`62758a647a37e270c4d2f969055c814d7559d1c398f0324ed0243b29135bc689`と
294,495 bytes／524 lines／SHA-256
`ecc73e513eefdee295dc324bd0bff8a699ea8819bc99b6cad7694b336599ebc4`
である。両方236 API pairsはbalanced、diagnosticsはzeroで、default
initializationのsemantic projectionは一致する。

BGI callはreturn RVA `0x9B2C3`からimage RVA `0xFD9CC`の24-byte zero storageを
初期化する。Return後は
`FFFFFFFFFFFFFFFF000000000000000000000000D0070002`、raw SpinCount fieldは
`0x020007D0`である。LastError `0 -> 0`、exceptionなし、controlled sentinel 86も
保存される。Direct wrapperは`mov eax,esi; pop esi; ret`で独自returnを作り、outer
consumer RVA `0xA37BE`はそのwrapper EAXをESIと比較するため、VOID API自身のEAXは
consumerへ渡らない。

`kernel32.initialize-critical-section.v1`はone-argument stdcall／VOID／EAX preserve／
LastError preserveとchecked 24-byte outputを実装する。Existing Rust process-owned
critical-section subsystemを使い、missing policy／duplicate initializationはmutation前に
停止する。Private raw fieldはgeneral semanticsへ埋め込まず、policy
`kernel32-initialize-critical-section-v1`が`33556432`を所有する。Public semantic
fixtureはdistinct `0x02001234`で同じhandlerを検証する。

`npm run check:runtime:bgi -- --profile kernel32-initialize-critical-section`は44
contracts／267 calls、268 writebacks／406 dirty pages／5,855 changed bytes、67 host
deltas、final generation `365`を通す。Call 211はinstruction `19185`で
`InitializeCriticalSection(0x100FD9CC)`をreturnし、generation `299 -> 300`の
page `0x100FD000` deltaを書く。Instruction `22754`でendpointなしのpage fault
`0x006FE000`／PFEC `4`へ到達し、clusterは`initial-thread-stack-growth`である。
Memory SHA-256は
`5a7b35e013f2ac108f5d7394dc908ba91a76aaa77dc4e5288d3ee827460c146c`、
1,154,575-byte reportのSHA-256は
`c396de1128554bad61efabc52e1e10f29896a14e923a75d349751cde2eda7686`である。

Chrome Runtime Workerも267 ordered calls、same endpoint-null fault／memoryを再現し、
2,926 slices、2,658 yields、2,585 Wasm crossings、final generation `1,994`、
cancel／watchdog後pending work 0を通す。3分gateは347 sessions／failure 0、session
P95 619.620 ms、slice P99 1.845 ms、heartbeat P99 15.570 ms／maximum 53.280 msを
含む全17 thresholdを通した。詳細は
[bgi-initialize-critical-section.md](bgi-initialize-critical-section.md)を正とする。

## 82. BGI initial-thread stack growth

PE headerの`SizeOfStackReserve = 1,048,576`、`SizeOfStackCommit = 4,096`を
Rust-owned range `[0x00600000,0x00700000)`へ接続した。Initial commitは
`0x006FF000`、movable guardは`0x006FE000`、process address allocation IDは
`37`である。Page faultの含有pageをmutation前にguard identityと比較し、zeroed
RW/NX pageを一枚ずつcommitする。

Node／Chromeはinstruction `22754`／fault `0x006FE000`、`22782`／
`0x006FDD94`、`22812`／`0x006FC000`、`22817`／`0x006FB000`の4 growthを
同じ順で適用する。Second faultがpage baseでないため、raw fault address equalityを
growth semanticsにはしない。Final guardは`0x006FA000`である。

Call 268のsecond `GetModuleFileNameA`をreturnした後、instruction `22843`で
`MultiByteToWideChar(0,0,0x006FE0A4,-1,0x006FD150,780)`へ到達する。Existing
policyはexplicit code page 932／flags 1だけを許可するため、fault code
`multi-byte-to-wide-char-variant-unsupported`でmutation前に停止する。Nodeは273
writebacks／412 dirty pages／5,929 changed bytes／72 host deltas／generation `374`、
Chromeは2,939 slices／2,666 yields／2,606 Wasm crossings／generation `2,010`を
記録し、memory SHA-256
`ca1d082833b4349640451886843761aafecc0a5e63e5771694e09e5c1906c0aa`で一致する。
3分gateは740 sessions／failure 0で全17 thresholdを通した。詳細は
[bgi-initial-thread-stack-growth.md](bgi-initial-thread-stack-growth.md)を正とする。

## 83. BGI CP_ACP NUL-terminated multibyte conversion

Focused `multi-byte-to-wide-char` capturesは80,561 bytes／88 lines／SHA-256
`a9f7c88d9ee5aa91c5a046bc9e84feea67682c3927611f6c6e1ea5903e0ca600`と
80,592 bytes／88 lines／SHA-256
`b66f55eddf91e399a0333fbfb43df64b6f5025f818a9bc0ef9f0d202b1891fb1`
である。両方18 API pairsはbalanced、sequenceはcontiguous、diagnosticsはzeroである。

BGI return RVA `0x2BB13`のcallは`CP_ACP`、flags 0、source count `-1`、capacity
780を渡す。Native install pathは28 Windows-932 bytes／23 UTF-16 unitsで、ともに
terminating NULを含む。Resultは23、LastErrorは`0 -> 0`である。Controlled oracleの
`A 82 A0 00` query／conversionはどちらもNUL込み3 unitsを返し、sentinel 92／93を
保存する。Immediate continuationはEAXを使わず五つのpath componentを準備し、nonnull
componentを`WideCharToMultiByte`へ渡す。Native path、pointer、780、caller RVAは
policy／handlerへimportしない。

`kernel32.multi-byte-to-wide-char.v1`はsingle semantic handlerのまま、active policy
`kernel32-multi-byte-to-wide-char-cp-acp-v1`が`allowActiveCodePageSelector: true`と
flags `[0,1]`を追加する。Historical policyはselectorを許可しない。`-1` sourceは
65,536-byte bound内でchecked NUL scanし、terminatorをdecode resultへ含める。Public
fixtureはexplicit 932／flags 1、CP_ACP／flags 0 query-conversion、LastError、partial
buffer、invalid shape、mutation-free unsupportedを同じhandlerで検証する。

Nodeは44 contracts／279 calls、284 writebacks／431 dirty pages／6,270 changed bytes、
75 host deltas、generation `388`を通す。Call 269はcontrolled `C:\Vine\BGI.exe\0`を
16 UTF-16 unitsへ変換し、calls 270／271がpath componentを既存
`WideCharToMultiByte`でconsumeする。Instruction `24294`でID `25`／
`SetCurrentDirectoryA(0x10104048)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`dfb9d4fae57c56d3c518b522669c129377a2de98e64cd7de9e5dfdad42e6113d`、
1,225,836-byte reportのSHA-256は
`80073d04b9881acff731109960be7be52b19074c7775dd3c5ef93f14d178deea`である。

Chrome Runtime Workerも279 ordered calls、same stop／memoryを再現し、3,123 slices、
2,839 yields、2,747 Wasm crossings、final generation `2,126`、cancel／watchdog後
pending work 0を通す。3分gateは607 sessions／failure 0、session P95 326.935 ms、
slice P99 1.230 ms、heartbeat P99 8.530 ms／maximum 23.800 msを含む全17 thresholdを
通した。詳細は
[bgi-multi-byte-to-wide-char-cp-acp.md](bgi-multi-byte-to-wide-char-cp-acp.md)を正とする。

## 84. BGI process current directory

Focused `set-current-directory-a` capturesは42,686 bytes／74 lines／SHA-256
`b6ea50c32a22c4beb28ce71fabc3329ec64553a74a1f56e96dc343003a94eee1`と
42,764 bytes／74 lines／SHA-256
`9a60c2e7562f5d3889c392e267bca332caf4031afdc0fe34404e7f0b54d873ed`
である。両方11 API pairsはbalanced、sequence `0..71`はcontiguous、diagnosticsは
zeroである。

Return RVA `0x6084A`のBGI callは21 Windows-932 bytes including NULの
`V:\softs\サクラノ詩\`を渡す。Path SHA-256は
`8c0d0e2de4dc6bacd126c540e2c5ade2c95d2f60aed46130cee309b1dc8dade4`、
FNV-1aは`0x6BEEB8DB`である。BOOLは1、LastErrorは`0 -> 0`、input bytesは
不変で、process current directoryはtrailing separatorを除くsame normalized identityを
保持する。Immediate continuationはEAXをdiscardし、source globalから別のprocess-global
bufferへpathをcopyする。302,028-byte／922-line extended capture（SHA-256
`b0a3a972029cc86e21fd7738d4331ea5d51a7fae7723100883715347cef4f43d`）では、
later `CreateFileA`がそのrootを持つabsolute `...\BGI.gdb`等を使う。

Controlled oracleはrelative `..`／`.` successがLastError 87／88を保存し、return後の
caller buffer mutationがprocess stateを変えないことを固定する。Existing file、missing
directory、empty stringはそれぞれBOOL 0／LastError 267、2、123を返す。Original directory
restoreはsuccessしLastError 92を保存する。

Rustはprivate `RuntimeProcessDirectories`へWindows-932 normalized identityとgenerationを
置く。Active policyはregistered process-image directoryと32 KiB scanだけを許可し、
`kernel32.set-current-directory-a.v1`はguest pointerを保持せず
`plan -> lifecycle preflight -> commit`する。Null／empty failureはWin32 result、validだが
unsupportedなrelative／other absolute pathとunterminated bound超過はmutation前のstructured
stopである。VFS/backendは追加していない。

Nodeは45 contracts／280 calls、285 writebacks／435 dirty pages／6,307 changed bytes、
75 host deltas、generation `389`を通す。Call 280はcontrolled `C:\Vine\0`をprocess-owned
identityへcopyしgeneration `1`を記録する。Instruction `24496`でID `59`／
`CreateMutexA(0,1,0x100E1CAC)`を`endpoint-contract-missing`として停止する。Memory
SHA-256は`7b95978468db1f5664799d9621b990ace2c06332b4e251bf6b377fa6c4e15bb2`、
1,230,591-byte reportのSHA-256は
`a90733ca1dc9b8017263c5c5439e55570403927420f1a535ff61e2774278fd68`である。

Chrome Runtime Workerも280 ordered calls、initial／post-call current-directory metadata、
same stop／memoryを再現し、3,149 slices、2,864 yields、2,771 Wasm crossings、final
generation `2,136`、cancel／watchdog後pending work 0を通す。3分gateは566 sessions／
failure 0、session P95 351.465 ms、slice P99 1.460 ms、heartbeat P99 16.135 ms／
maximum 20.905 msを含む全17 thresholdを通した。詳細は
[bgi-set-current-directory-a.md](bgi-set-current-directory-a.md)を正とする。

## 85. BGI named mutex creation

Focused `create-mutex-a` capturesは58,888 bytes／94 lines／SHA-256
`56931882497c18b6126a3dd93d535df07c4e717f2f3c800e78045bf56cb24904`と
58,776 bytes／94 lines／SHA-256
`d83862ad0d0effaafd273e3e9624363a6cbd87e1cf59f1c6d49afbd51b6a77cd`
である。両方21 API pairsはbalanced、sequence `0..91`はcontiguous、diagnosticsは
zeroである。Second captureはfirst BGI instanceがnameを保持した状態で別processを起動した。

Return RVA `0x87300`のBGI callはnull security attributes、initial owner TRUE、57 ANSI
bytes including NULの`Buriko General Interpreter for Sakuranouta is executing.`を渡す。
Name SHA-256は
`3f423adb8974d8f8cb0748e49b073d2f8f23d97d45fbe7d88b6d0f91cad196e5`、
FNV-1aは`0x0C6AC3D2`である。New instanceはnonnull opaque handle／LastError 0、
existing instanceはdistinct nonnull handle／LastError 183を返す。Continuationはhandleを
process-globalへstoreし、直ちに`GetLastError`を呼んで183と比較する。

Independent `x86-named-copy-owner-handle-lifetime-v1` oracleはcaller buffer mutation前の
name copy、distinct handles／same named object、first initial-owner grant、existing createの
initial-owner ignore、recursive zero-time wait／release、unowned releaseのERROR_NOT_OWNER 288、
remaining-handle lifetime、last-close name retirementを固定する。Native handle numbersとoracle
PID suffixはsemantic contractへimportしない。

Rust `RuntimeSynchronization`はANSI bytes／Windows-932 UTF-16 identity、owner thread、recursion、
named indexをprivateに所有する。Generic object tableは`RuntimeObjectKind::Mutex`、type tag
`0x58`、handle slots、object lifetimeだけを所有する。Create／open／last close／process teardownでは
synchronization ownerがmutation-free plan、object ownerがopaque mutation planを作る。`RuntimeKernel`は
lifecycle／capacity preflight後に両ownerのcommitだけを呼び、subsystem rootをclone／置換しない。Unowned／same-thread waitは
acquire／recursive acquireし、other-thread contentionは`mutex-contended-unsupported`でmutation前に
停止する。`kernel32.create-mutex-a.v1`はactive policyのnamed／32 KiB boundだけを使い、guest
pointerを保持しない。New objectはLastError 0、existing nameは183を設定する。

Nodeは46 contracts／284 calls、289 writebacks／443 dirty pages／6,633 changed bytes、
77 host deltas、generation `395`を通す。Call 281はlogical handle `0x58010001`、mutex ID `1`、
typed object ID `19`、owner thread `8192`、recursion `1`を返す。Calls 282–284は
`GetLastError`、flags-zero 16-byte `HeapAlloc`、`HeapFree`をreturnする。Instruction `25256`で
ID `40`／`GetCurrentThread()`を`endpoint-contract-missing`として停止する。Memory SHA-256は
`338da6511938aedf1ec46c5838499f159da7fcbf86218459821e385cc6cd3add`、
1,250,520-byte reportのSHA-256は
`0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`である。

Chrome Runtime Workerも284 ordered calls、same mutex metadata／stop／memoryを再現し、
3,246 slices、2,957 yields、2,850 Wasm crossings、final generation `2,207`、
cancel／watchdog後pending work 0を通す。3分gateは435 sessions／failure 0、session P95
476.945 ms、slice P99 2.110 ms、heartbeat P99 16.325 ms／maximum 28.480 msを含む全17
thresholdを通した。詳細は[bgi-create-mutex-a.md](bgi-create-mutex-a.md)を正とする。

## 86. BGI logical thread affinity and tick calibration

Focused `set-thread-affinity-mask` capturesは147,512 bytes／435 lines／SHA-256
`f55699275c2e3f7edcf8b7d414d7fe1e8bbabb8e701b04fa45acf097b4de0d5a`と
141,205 bytes／416 lines／SHA-256
`c3807adb646982fe1dccf82c80fd385d1402bd745d094a36c6dc9998e135e563`
である。185／176 API pairsはbalancedで、diagnosticsはzeroである。

Measured native process／system maskは`0x0000FFFF`である。BGIはcurrent-thread
pseudo-handleへmask `1`を設定しprevious `0x0000FFFF`を保存する。Return RVA
`0x6999A`から`GetTickCount` calibrationへ入り、約1,000 ms後にsaved maskを
restoreしてRVA `0x699F0`へreturnする。Successはprevious maskを返しLastErrorを
保存する。Independent oracleはzero maskをERROR_INVALID_PARAMETER 87、null handleを
ERROR_INVALID_HANDLE 6として固定する。Native mask／handle／CPU topologyはruntimeへ
importしない。

Calibration aggregate r8／r9は133,327 bytes／383 lines／SHA-256
`d485f0aa38f178ab12de96604f373a97e7d5ae8811a138a858dc84019d45898b`と
149,701 bytes／436 lines／SHA-256
`90e6649bd8405d58724c3ca7b6e8b0e4c3f18a9b00b03b62ecdd27830f22f077`
である。154,477／149,406 tick reads、各64 value changes、regression 0、elapsed
1,016 msを記録し、consumerがfixed sampleでは進まないことを示す。

Rust `RuntimeThreads`はpolicy-owned process maskに対するper-thread logical maskを
mutation-free plan／stale-rejecting commitで所有する。Active maskは`1`なのでBGIの
set／restoreはいずれも`SetThreadAffinityMask(0xFFFFFFFE,1) -> 1`となる。
`RuntimeProcessState`はactive policyのreadごと`+1 ms`でtick DWORDをsample／commitし、
historical policyはfixed sampleを保持する。Host time／affinityは参照しない。

Nodeは48 contracts／1,290 calls、1,295 writebacks／2,453 dirty pages、77 host deltas、
generation `1,401`を通す。Calls 286／1,290がaffinity、calls 287..1,289が1,003-read
calibrationである。Instruction `45420`でID `120`／
`GetSystemInfo(0x006FFED8)`を`endpoint-contract-missing`として停止する。Memory SHA-256は
`ca8193a48aa4293f31a9cd95bf516ff777e50256e3d1c53c440ae02b0d6b4bb5`、
5,260,396-byte reportのSHA-256は
`ffaf389e5ed377d07aefe71ad8ad3380209fcb603fa70b27b99c9ddf8f6ff2db`である。

Chrome Runtime Workerもsame ordered calls／affinity／tick values／stop／memoryを再現し、
6,269 slices、4,974 yields、5,872 Wasm crossings、final generation `3,223`、
cancel／watchdog後pending work 0を通す。3分gateは421 sessions／failure 0、session P95
430.865 ms、slice P99 2.190 ms、heartbeat P99 6.055 ms／maximum 25.555 msを含む全17
thresholdを通した。詳細は
[bgi-set-thread-affinity-mask.md](bgi-set-thread-affinity-mask.md)を正とする。

## 87. BGI x86 virtual system information

Focused `get-system-info` capturesは256,120 bytes／784 lines／SHA-256
`a64745f92c00a1523133e9b9ae38b434073008c7fe3f7a0d01f32208af5ed2c8`と
221,171 bytes／667 lines／SHA-256
`95f2df7426f70a9d91e6d94b1b29bceb697de6730fdcc3178162046ea82fc022`
である。357／299 API pairsはbalancedでdiagnosticsはzeroである。Independent oracleは
x86 `SYSTEM_INFO`のexact 36-byte write、前後guard不変、3回のoutput安定、LastError
sentinel `0`／`87`／`0x12345678`保存を固定する。

Native hostはarchitecture 0、page size 4096、application range
`0x00010000..0x7FFEFFFF`、active mask `0x0000FFFF`、processor count 16、type 586、
granularity 65536、level 25、revision `0x7502`を返す。BGI return RVA `0x69F0A`は
先頭DWORDだけをcopyしてEAXを1で上書きする。Host mask／count／level／revisionは
capture identityでありruntimeへimportしない。

Rustはschema-v2 policyのbounded PE32 profileを36 bytesへserializeする。Active maskは
既存のvirtual process affinity mask `1`、countはそのpopcount `1`から導出し、authorityを
重複させない。Public fixtureはsynthetic multi-bit maskとdistinct level／revisionを使い、
NULL／read-only／cross-page-invalid outputがpartial writeなしで停止することを固定する。

Nodeは49 contracts／1,291 calls、1,296 writebacks／2,455 dirty pages／7,808 changed
bytes、78 host deltas、generation `1,403`を通す。Call 1,291は`0x006FFED8`へ36 bytesを
writeする。Instruction `45614`でID `189`／
`EnumDisplayMonitors(0,0,0x1006A700,0)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`651b3336011adb55b98e8bb13329e4bb368993a73d59ab878c65294636df92f5`、
5,266,052-byte reportのSHA-256は
`ddcc44d2f9f21c11f6fe919a6c7a51fb018db01cc74e3b177f919811939c3c1c`である。

Chrome Runtime Workerもsame ordered calls／output／stop／memoryを再現し、6,294 slices、
4,998 yields、5,894 Wasm crossings、final generation `3,241`、cancel／watchdog後pending
work 0を通す。3分gateは415 sessions／failure 0、session P95 440.220 ms、slice P99
1.935 ms、heartbeat P99 6.045 ms／maximum 18.535 msを含む全17 thresholdを通した。
詳細は[bgi-get-system-info.md](bgi-get-system-info.md)を正とする。

## 88. BGI logical monitor callback enumeration

Focused `enum-display-monitors` capturesは208,419 bytes／643 lines／SHA-256
`3b3f673eaf46dcc902483dc2bededa2eeea595ff4149f7743bd3bacf16275f0e`と
153,699 bytes／465 lines／SHA-256
`8753e255371e544a10c118f17f60e06ec59d80b75bf546c8c3d92a65f43a172c`
である。287／200 API pairsはbalancedでdiagnosticsはzeroである。

BGIは`EnumDisplayMonitors(NULL,NULL,0x1006A700,0)`をRVA `0x6A781`から呼ぶ。
Callbackは1回、native opaque HMONITOR `0x10001`、NULL HDC、
`RECT(0,0,1920,1080)`、LPARAM 0を受けてTRUEを返し、outer resultもTRUE、
LastErrorも0のままである。Independent TRUE／FALSE oracleはdistinct LPARAMと
LastError sentinelsをround-tripし、FALSEがouter FALSEを返してfirst callback後に
停止することを固定する。Native handle値はruntime semanticsへimportしない。

Rust `RuntimeDisplays`はlogical namespace `vine.logical-monitor.v1`、handle
`0x4F010001`、fixed 1920x1080 geometryを所有する。Generic callback preparerは
16-byte RECT、return sentinel、4 argumentsをone 36-byte writable bandとして
preflight／commitし、zero／nonzero callback resultをouter BOOLへ正規化する。
BGI callback内の既存`HeapAlloc(...,24)` trapをouter call suspension中に処理し、
frame ID／stdcall ESP／LastError／call-entry orderを保持する。

Nodeは50 contracts／1,293 calls、1,299 writebacks／2,459 dirty pages／7,883
changed bytes、80 host deltas、generation `1,408`を通す。Chrome Runtime Workerも
same ordered calls／callback／stop／memoryを再現し、6,304 slices、5,005 yields、
5,906 Wasm crossings、final generation `3,250`、terminal pending work 0を通す。
Instruction `45685`でID `301`／`OLE32.dll!CoInitialize(NULL)`を
`endpoint-contract-missing`として停止する。Memory SHA-256は
`8ec8022f983af336f4e1b015d9f1760c6db01c606e678059cd6996cf4b403753`である。
3分gateは198 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-enum-display-monitors.md](bgi-enum-display-monitors.md)を正とする。

## 89. BGI COM apartment initialization

Focused `com-initialization` capturesは154,928 bytes／457 lines／SHA-256
`a85aea76149237c60508993ec1dd3e421ba69fc89bb2d4cfb6c3b59dae65e70e`と
60,989 bytes／151 lines／SHA-256
`b47ad8ae39f7451852bac6e2c59945a01599b94af970cbb450d9d10af8ed52cb`
である。196／49 API pairsはbalanced、diagnosticsはzeroである。Kill時にunrelated
D3D／dynamic-export callsがopenだった二つのshort captureはevidenceから除外した。

BGIは`CoInitialize(NULL)`を呼びRVA `0x87399`へreturnする。Immediate consumerは
`test eax,eax; js failure`でHRESULTのnegativeだけをfailureとし、LastErrorを読まない。
Independent same-thread oracleはfirst STA／repeat STA／STA-to-MTA conflict／count-one
conflict／first MTA／repeat MTA／MTA-to-STA conflict／balanced first STAを順に実行し、
results `S_OK,S_FALSE,RPC_E_CHANGED_MODE,RPC_E_CHANGED_MODE,S_OK,S_FALSE,
RPC_E_CHANGED_MODE,S_OK`を固定する。Successful callsはすべて`CoUninitialize`でbalance
される。Repeat／conflictはLastError sentinelを保存するが、first successはzeroとsentinel
preserveの両方を観測したためcontract外とする。

Rust `RuntimeThreads`はper-thread model／successful initialization countを所有する。
Same-model repeatはchecked increment、changed modeはno mutation、uninitializeはzeroでstateを
retireし、thread exitもそのthreadのstateだけをclearする。COM object／interface／refcount authority
`RuntimeCom`とは分離する。`ole32.co-initialize.v1`はNULL-only STA、typed HRESULT、stdcall、
LastError preserveを実装し、non-NULL reservedをmutation前に拒否する。

Nodeは51 contracts／1,294 calls、1,300 writebacks／2,460 dirty pages／7,890 changed
bytes、80 host deltas、generation `1,409`を通す。Chrome Runtime Workerもsame ordered
calls／apartment／stop／memoryを再現し、6,305 slices、5,005 yields、5,908 Wasm crossings、
final generation `3,251`、terminal pending work 0を通す。Instruction `45691`でID `39`／
`KERNEL32.dll!GetSystemDefaultLangID()`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`b7e5005417aa49c642ded6fdb078be8fe1750e9853cbbcf19e1996d9fd4fad4b`である。
3分gateは198 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-co-initialize.md](bgi-co-initialize.md)を正とする。

## 90. BGI system-default LANGID

Focused `system-default-lang-id` capturesは81,022 bytes／266 lines／SHA-256
`b7837df03fd27e34c5839116874d1b0858874c5179d3b92bfa94f99a570f0efa`と
79,185 bytes／260 lines／SHA-256
`01e67e62be63be76293fbf6d59e8d3173e88f4c1597e5571002cc817406f79e0`
である。103／100 API pairsはbalanced、diagnosticsはzeroである。Unrelated
`Direct3DCreate9`がopenだったr1／r3／r6と、WORD observer修正前のr2／r4はfinal evidenceから
除外した。

Four-call oracleはLastError `0`／`87`／`122`／`0x12345678`をすべて保存し、low WORD
`0x0411`を返す。Native raw EAXはaccepted runsで`0x01390411`／`0x00CD0411`と異なるため、
upper 16 bitsはunspecified artifact identityである。BGI return RVA `0x6A556`はAXを
`0x03FF`でmaskしprimary language `0x11`と比較する。LastErrorは読まない。

Rust policyはsystem-default LANGIDを`LCMapStringW.allowed_locales`と別fieldで所有し、Japanese
locale layerとの整合だけをvalidationする。`kernel32.get-system-default-lang-id.v1`はtyped U16を
EAXへzero-extendし、LastError保存、zero guest write／runtime mutationを固定する。Host localeと
native upper EAX bitsはimportしない。

Nodeは52 contracts／1,295 calls、1,301 writebacks／2,461 dirty pages／8,577 changed
bytes、80 host deltas、generation `1,410`を通す。Chrome Runtime Workerもsame ordered
calls／result／stop／memoryを再現し、6,324 slices、5,023 yields、5,923 Wasm crossings、
final generation `3,263`、terminal pending work 0を通す。Instruction `45840`でID `19`／
`KERNEL32.dll!GetDriveTypeA(0x006FFC4C)`を`endpoint-contract-missing`として停止する。
Memory SHA-256は
`05bf22ce52303ba9b0849352ecd4e00460449ac3bbf84fd03e30cac146d8dc0c`である。
3分gateは205 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-get-system-default-lang-id.md](bgi-get-system-default-lang-id.md)を正とする。

## 91. BGI drive-type scan

Focused `drive-type` capturesは68,100 bytes／128 lines／SHA-256
`d42ff6b0032bf52bf134c712886abd6a56c8b833156a6dbd5d92794e7a4d4919`と
68,042 bytes／128 lines／SHA-256
`08fa728b429fb36ae6fc6a063048ba0a227fdc8235d2f69f7be8d77e6b14dd7a`
である。両方37 API pairsはbalanced、diagnosticsはzeroである。Observer completion eventはBGIの
26th `Z:\` leave後にguest threadを停止し、terminationが後続graphics／window APIをopenのまま
残さない。

BGI return RVA `0x602B2`は`A:\`から`Z:\`を列挙し、resultと`DRIVE_FIXED(3)`でないflagを
二つのtableへ保存する。Native hostでは`C:\`とsource所在の`V:\`が3、他は
`DRIVE_NO_ROOT_DIR(1)`である。Four-call oracleはcurrent root／NULLを3、existing non-root／`.`を1とし、
LastError `0`／`87`／`122`／`0x12345678`を保存する。

Runtimeはnative drive listをimportせず、private `RuntimeProcessDirectories`のprocess-image identityから
virtual fixed driveを導出する。Controlled `C:\Vine` profileではoccurrence 3だけが3、他25 callsは1で、
LastError 0、guest write zeroである。Nodeは53 contracts／1,321 calls、1,327 writebacks／2,513 dirty
pages／8,669 changed bytes、80 host deltas、generation `1,436`を通す。Chrome Runtime Workerもsame
ordered calls／results／stop／memoryを再現し、6,378 slices、5,051 yields、6,002 Wasm crossings、final
generation `3,316`、terminal pending work 0を通す。Instruction `46215`でID `166`／
`USER32.dll!SwapMouseButton(0)`を`endpoint-contract-missing`として停止する。Memory SHA-256は
`070ccf6100b1a0537a43fa250942b23f9e293cad712d78b1d2a3c9d9a3ad0433`である。3分gateは193
sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-get-drive-type-a.md](bgi-get-drive-type-a.md)を正とする。

## 92. BGI mouse-button mapping restore

Focused `swap-mouse-button` capturesは31,879 bytes／72 lines／SHA-256
`eca20bd9a661ffd1b04b9dad776711c38002f18ab130e65c166fd58d4033d459`と
31,930 bytes／72 lines／SHA-256
`1dea669569a73ddc7eb5a9cf49068114865bf206c905204cec6733071f630a34`
である。両方9 API pairsはbalanced、agent sequenceはcontiguous、diagnosticsはzeroである。

BGIはRVA `0x88A3B`へreturnする`SwapMouseButton(FALSE)`のEAXをprocess globalへ保存し、
同じ値を直後のsecond callへ渡してRVA `0x88A43`へreturnする。Accepted hostはinitial／final
ともunswappedでarguments／resultsは`[0,0]`、LastErrorも`[0,0]`だった。Observerは最初の
native mappingをevidence metadataとして読み、2 calls完了後に同じstateへ戻ったことを確認してから
captureを停止する。Host mapping値はruntimeへimportしない。

Rust `RuntimeInput`はprofile、initial／current mapping、generationを所有し、User32 handlerは
previous BOOLを返す。Nodeは54 contracts／1,333 calls、generation 1,450まで進み、Chromeもsame
memory SHA-256
`f6030b2d43bbbc6fe43a2ede52b1ecc6e4962822e39235032f25ca560a58d0e4`
を再現する。Instruction `49731`でID `3`／
`WINMM.dll!timeGetDevCaps(0x1014C8E8,8)`へstructured stopする。詳細は
[bgi-swap-mouse-button.md](bgi-swap-mouse-button.md)を正とする。

## 93. BGI WinMM timer capabilities

Focused `time-get-dev-caps` accepted capturesは284,947 bytes／773 lines／SHA-256
`4c14edc2f9609c48dd4a0589e2e608a23364abc4eaab5231a5942c9ba760c6c2`と
284,903 bytes／775 lines／SHA-256
`57282a7c58b83082554a3f2dbe2f87653ccd92dfdaf0b26ea70414971ff11c2b`
である。351／352 API pairsはbalanced、agent sequenceはcontiguous、diagnosticsはzeroで、
BGI `timeBeginPeriod` leave後250 msのhook quiescenceも確認した。Open `CreateFile`を残した
short captureと、final period hook追加前のcaptureはauthoritative evidenceから除外した。

Guarded x86 oracleはsize `0/4/7`でresult `TIMERR_NOCANDO(97)`、output／guard不変、size
`8/12`でresult 0、先頭8 bytesだけ`01 00 00 00 40 42 0F 00`へ変化することを固定する。
これはminimum 1 ms／maximum 1,000,000 msである。各callは独立LastError sentinelを保存し、
unhooked system callと同じresult／bytesを返す。

BGI return RVA `0x91E6D`はMMRESULTを捨ててminimumを読み、10未満ならその値を
`timeBeginPeriod`へ渡す。Observed argumentは1、return RVAは`0x91E7E`、result／LastErrorは
0である。Rust `WinmmCompatibilityPolicy`はvirtual capability rangeを所有し、queryはclockを
進めずhost／browser timer characteristicをimportしない。

Nodeは55 contracts／1,334 calls、1,340 writebacks／2,531 dirty pages／8,971 changed bytes、
83 host deltas、generation 1,452を通す。Chromeもsame ordered calls／output／stop／memoryを
再現し、6,821 slices、5,481 yields、6,378 Wasm crossings、final generation 3,588、terminal
pending work 0を通す。Memory SHA-256は
`065650f1edeb4dccdc65d37b972956c38a098acd2e85c910a532853f6bd97966`である。3分gateは
358 sessions／failure 0で17 thresholdsを通した。Instruction `49737`でID `4`／
`WINMM.dll!timeBeginPeriod(1)`へstructured stopする。詳細は
[bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md)を正とする。

## 94. BGI WinMM timer-resolution request

Focused period-lifecycle accepted capturesは310,319 bytes／830 lines／SHA-256
`25e7c69f714c70d1918db6fea0b033419afa2308f89076a81ecb039cb69ba210`と
310,070 bytes／829 lines／SHA-256
`85cd6dd52528f1268773ae3031d7d18e22b571cec8ec3dc0d2e99514cb7c84ce`
である。両方379 API pairsはbalanced、agent sequenceはcontiguous、diagnosticsはzeroで、BGI
period leave後250 msのquiescenceを通す。`onLeave` scope defectで14 agent messagesを出したr1は
rejected evidenceである。

17-call x86 oracleはunmatched `timeEndPeriod(1)`とzero begin／endを
`TIMERR_NOCANDO(97)`、period 2／1,000,001／`UINT_MAX`のbegin／end pairを0とする。Duplicate
begin(1)はmatching endを2回要求し、period 1だけactiveな間のmismatched end(2)とthird end(1)は
97である。全callが独立LastError sentinelを保存し、hooked／unhooked resultは一致する。Nativeの
broad nonzero acceptanceはactive policyへimportせず、reached period 1だけを許可する。

BGI callはargument 1、return RVA `0x91E7E`、result／LastError 0である。Rust
`RuntimeTimers`がcount／effective minimum／generationを所有し、begin／endはstale-check plan、
unmatched endはmutation-free、process exitはoutstanding requestsを一括retireする。Browser scheduler、
audio、host timer resolutionはguest-visible stateへimportしない。

Nodeは57 contracts／1,336 calls、1,342 writebacks／2,536 dirty pages／9,099 changed bytes、84 host
deltas、generation 1,455を通す。Chromeもsame timer snapshot／ordered calls／stop／memoryを再現し、
6,841 slices、5,499 yields、6,397 Wasm crossings、final generation 3,603、terminal pending work 0を
通す。Memory SHA-256は
`b38b7030fc29769c220a4095891aa1f2bd2d71ea71584f5c3349097d7b084f6f`である。3分gateは367
sessions／failure 0で17 thresholdsを通した。Instruction `49890`でID `117`／
`KERNEL32.dll!GetVersionExA(0x006FFE9C)`へstructured stopする。詳細は
[bgi-time-begin-period.md](bgi-time-begin-period.md)を正とする。

## 95. BGI ANSI operating-system version

Focused `get-version-ex-a` capturesは287,884 bytes／791 lines／SHA-256
`ee021e4fb44315a342746bfd5908b36b5a2f319d2cd1c588155e115eed15ddc7`と
290,851 bytes／800 lines／SHA-256
`0ae028967ca9e815e15d6ce109310e8b20d57a19bc09fded832a7a2c3a59a90b`
である。360／364 API pairsはbalanced、agent sequenceはcontiguous、diagnosticsはzeroで、
BGI call後250 msのquiescenceを通す。

Guarded x86 oracleはdeclared size 0／147をFALSE、LastError 122、output／guard不変とし、
148／156をTRUE、LastError preserveとする。148-byte形はoffset 4..20、156-byte形はさらに
148..155だけを変更する。Native host fieldsは10.0 build 26200、platform 2、empty service pack、
suite `0x0300`、workstation type 1であるが、host identityとしてruntimeへimportしない。

BGIはzeroed 148-byte structureを渡してRVA `0x50526`へreturnし、`dwPlatformId == 2`だけを
分岐に使う。Rust schema-v2 policyはcontrolled NT 10.0 build 19045／suite `0x0100`／workstationを
所有する。Handlerはfull declared spanをreadしpolicy fieldsをbefore-imageへmergeしてsingle writeする。
Invalid sizeはFALSE／122／no write、NULL／truncated／read-only spanはmutation前に停止する。

Nodeは58 contracts／1,337 calls、1,343 writebacks／2,537 dirty pages／9,110 changed bytes、
85 host deltas、generation `1,457`を通す。Chromeもsame ordered calls／output／stop／memoryを再現し、
6,842 slices、5,499 yields、6,400 Wasm crossings、final generation `3,605`、terminal pending work 0を
通す。Instruction `49897`でID `116`／
`CreateWaitableTimerA(NULL,TRUE,NULL)`を`endpoint-contract-missing`として停止する。Memory
SHA-256は`6d5a550c0f16b5097465e1af2f3ffe0c97097d9223b8d2991757ba73a87b0280`である。
Performance gateはsemantic leafのため再実行せず、直前profileの観測を履歴として保持する。詳細は
[bgi-get-version-ex-a.md](bgi-get-version-ex-a.md)を正とする。

## 96. BGI waitable-timer creation

Focused `create-waitable-timer-a` capturesは256,897 bytes／779 lines／SHA-256
`1debfadd3238adb16d5607779fdce297d6420350f85323f07b15f047230551af`と
255,564 bytes／777 lines／SHA-256
`07e769e5e27d7495ea4fbc3f09bebb55cbc6de2c4b6779e37c1e796977eb475c`
である。354／353 API pairsはbalanced、agent sequenceはcontiguous、diagnosticsはzeroで、BGI call後
250 msのquiescenceを通す。

Controlled x86 oracleはunnamed manual-reset／auto-reset timerを各一件createし、どちらもnon-NULL、
success LastError 0、initial `WaitForSingleObject(handle,0)` result 258、close result 1を返す。
Hooked result／LastErrorはdirect system callと一致する。Native handle valueはartifact identityであり、
runtimeへimportしない。

BGIは`CreateWaitableTimerA(NULL,TRUE,NULL)`からRVA `0x5053B`へreturnし、EAXを直接`[esi+4]`へ
保存する。Rust `RuntimeTimers`はtimer ID／manual-reset payload、`RuntimeObjectTable`はobject ID／
typed handle／nonsignaled wait stateを所有する。Creation、last close、process teardownは全ownerを
preflightしたtargeted commitであり、TypeScriptはcompletion snapshotだけを運ぶ。

Nodeは59 contracts／1,338 calls、1,344 writebacks／2,541 dirty pages／9,178 changed bytes、85 host
deltas、generation 1,458を通す。Chromeもsame timer snapshot／ordered calls／stop／memoryを再現し、
6,848 slices、5,504 yields、6,407 Wasm crossings、final generation 3,611、terminal pending work 0を
通す。Instruction `49942`でID `192`／`USER32.dll!LoadIconA(0x10000000,0x65)`へstructured
stopする。Memory SHA-256は
`de0f2de11612c4792c7f87daa195c3a0b95d519069eefa7f93edc959cd4ce26a`である。Lifecycle boundary
変更後の3分gateは267 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-create-waitable-timer-a.md](bgi-create-waitable-timer-a.md)を正とする。

## 97. BGI module icon loading

Focused `load-icon-a` capturesは285,147 bytes／779 lines／SHA-256
`668a89b250c796eadce88ea8eec767220c8671b4a63b541327ad6969bdc8c49d`と
2,257,599 bytes／6,952 lines／SHA-256
`7b6fdc36e757566b7bb71cbfa21960e3b7297c5b2ee413ff7abfa25f5252a098`
である。354／3,438 API pairsはbalancedでdiagnosticsはzeroである。

Guarded x86 oracleはmain moduleのinteger resource 101を2回loadし、同じnon-NULL HICON、
success LastError 0を返す。`GetIconInfo`のcolor bitmapは32×32、1 plane、32 bppで、large／small
system metricsは32×32／16×16だった。Missing ID 65,535はNULL／LastError 1,813である。

BGI return RVAは`0x67CF3`／`0x67D7C`で、両resultは`WNDCLASSEXA.hIcon`へ入り、
`hIconSm`はNULLである。Resource treeの`RT_GROUP_ICON` 101／language 1,041は
256／48／32／16-pixel entriesを持ち、Windowsは4,264-byte `RT_ICON` 3の32-pixel DIBを選ぶ。
Rust policyもこのexact 32×32 variantを選ぶが、native HICON／bitmap handleはimportしない。

Nodeは60 contracts／1,339 calls、generation 1,459まで進み、Chromeもsame ordered icon
metadata／memory digestを再現する。Instruction 49,949で
`LoadCursorA(NULL,IDC_ARROW)`へstructured stopする。3分gateは254 sessions／failure 0で
17 thresholdsを通した。詳細は
[bgi-load-icon-a.md](bgi-load-icon-a.md)を正とする。

## 98. BGI system arrow cursor loading

Focused `load-cursor-a` capturesは333,790 bytes／806 lines／SHA-256
`252195f84e3d3d7e24d8f34b5531d248e738a7600ca9c2b1241663d31fb21382`と
333,385 bytes／805 lines／SHA-256
`30d6ec4272b6700a7d481276205ce25cbce7527e5ba22ff5820024a7c1cc844c`
である。367／366 API pairsはbalancedでdiagnosticsはzeroである。

Guarded x86 oracleは`LoadCursorA(NULL,IDC_ARROW)`を2回呼び、同じnon-NULL HCURSORを返し、
seeded success LastErrorを保持する。Cursor system metricsは32×32で、missing predefined ID
65,535はNULL／LastError 1,814を返す。Hooked result／LastErrorはdirect system callと一致する。

BGIはreturn RVA `0x67D09`／`0x67D8C`／`0x67FD2`でsystem arrowをrequestし、同じnative
identityを受け取る。RVA `0x67FE1`はmodule resource 106をrequestしてnon-NULLを得る。最初の
system resultは`WNDCLASSEXA.hCursor`へ入り、直後の`RegisterClassExA`がconsumerとなる。
Runtimeはcurrent reachで必要な最初の`NULL,IDC_ARROW`だけをpolicy-enableし、native HCURSORは
importしない。

Nodeは61 contracts／1,340 calls、1,346 writebacks／2,543 dirty pages／9,208 changed bytes、
85 host deltas、generation 1,460を通す。Chromeもsame ordered cursor metadata／memory digestを再現し、
6,850 slices、5,504 yields、6,411 Wasm crossings、final generation 3,613、terminal pending work 0を
通す。Instruction 49,957でID 191／`USER32.dll!RegisterClassExA(0x006FFB08)`へstructured stopする。
Memory SHA-256は
`8c580b84b365739f602e1e2c2307b87ecbd605d8bf28a52590e844f203af728f`である。User32 lifetimeと
Worker payload変更後の3分gateは376 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-load-cursor-a.md](bgi-load-cursor-a.md)を正とする。

## 99. BGI process-local window class registration

Focused `register-class-ex-a` capturesは326,543 bytes／804 lines／SHA-256
`003cfc9d9c9de92f9b0dad3421393d757fc7641a5c2661b27898c28122df7a97`と
327,416 bytes／805 lines／SHA-256
`0cd19660c72f5ec35a83ed25f59486f814ca908783a412c5d13af14e8951345b`
である。各366 API pairsはbalancedでdiagnosticsはzeroである。

Guarded x86 oracleはvalid 48-byte inputのsuccess／seeded LastError preserve、same module/nameの
zero／`ERROR_CLASS_ALREADY_EXISTS` 1,410、`cbSize=47`のzero／`ERROR_INVALID_PARAMETER` 87を
固定する。Invalid sizeを48へ修正した後のregistrationは成功し、failureがidentityを予約しない。
Successful ATOMはdistinctで、name／ATOM双方の`GetClassInfoExA`が成功する。二classを
`UnregisterClassA`した後のqueryは1,411で失敗する。全callで48 input bytesは不変であり、hookと
direct system result／LastErrorは一致する。Native ATOM値はopaque artifact identityである。

BGIはreturn RVA `0x67D28`／`0x67DAB`でmain／child classを登録する。両inputは48 bytes、mainは
style `0x28`／WndProc RVA `0x92550`／name `BGI - Main window`、childはstyle `0x3020`／WndProc
RVA `0x2E6E0`／name `BGI - Child window`である。HINSTANCEはmain module、HICON／HCURSORは同じ
shared identities、extrasはzero、brush／menu／small iconはNULLで、両callはnonzero／LastError 0、
input preservedとなる。

Nodeは62 contracts／1,344 calls、1,350 writebacks／2,547 dirty pages／9,236 changed bytes、85 host
deltas、generation 1,464を通す。Logical ATOMは`0xC000`／`0xC001`で、intervening second icon／cursor
loadsはidentity 1をreuseする。Chromeもsame ordered immutable class metadata／memory digestを再現し、
6,855 slices、5,505 yields、6,420 Wasm crossings、final generation 3,618、terminal pending work 0を
通す。Instruction 49,990でID 203／`USER32.dll!GetSystemMetrics(7)`へstructured stopする。Memory
SHA-256は`1aa4139c9a3a70a3f2090a6381212846a3171f40cfb93954867a2dc7efe6011c`である。Stateful User32
lifetimeとWorker payload変更後の3分gateは363 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-register-class-ex-a.md](bgi-register-class-ex-a.md)を正とする。

## 100. BGI initial system metrics

Focused `get-system-metrics` capturesは55,901 bytes／99 lines／SHA-256
`368d62d858ca8b26102b330d805ca1b7671deda64f7d9d6159ef08985f717d4f`と
55,839 bytes／99 lines／SHA-256
`32884be3566ecb1b15d25fc91b42a44eab223b1cd932a3ff3396fd1882036f08`
である。各22 API pairsはbalancedでdiagnosticsはzero、source executable hashは不変である。

Guarded x86 oracleはindices `7,8,4,0,1`へdistinct LastError sentinelをseedし、結果
`3,3,23,1920,1080`と全success LastError preserveを固定する。Hook result／LastErrorはdirect
system callと一致する。BGIも同じindex順と値をreturn RVA `0x67DDB`／`0x67DE6`／`0x67DEE`／
`0x67E26`／`0x67E2E`で受け取る。Immediate consumerはfixed-frame widthを2倍して6、heightを
2倍してcaptionを加え29とし、screen width／height ratioもinitial window geometryへ使う。

Runtimeはscreen dimensionsを`RuntimeDisplays` primary RECTから、caption／frameをversion-6
User32 policyから返し、host／browser metricsをimportしない。Nodeはfive callsを含むcontrolled
sessionを通し、Chromeもsame ordered results、memory digest、next structured stopを再現する。
Read-only semantic leafなので3分gateは再実行せず、manifestはprevious window-class observationを
current artifactと分離して保持する。詳細は
[bgi-get-system-metrics.md](bgi-get-system-metrics.md)を正とする。

## 101. BGI 64 MiB private-heap lifetime and sparse access

Focused `large-heap-allocation` capturesは419,374 bytes／204 lines／SHA-256
`5800d1cbeb3bffffed54e7d3abee1cb9e1262dd468f3190e8354dcf70e9610c3`と
419,375 bytes／204 lines／SHA-256
`0d2250f012b5795318c5d86bbd47876555720a2d16d78e8e906384dda540914e`
である。各75 API pairsはbalanced、diagnosticsはzero、source executable hashは不変である。

両captureでmain-module callerはflags 0／67,108,864 bytesをHeapAllocし、non-NULL pointerを得て
LastError 0を保持する。Matching HeapFreeはflags 0でBOOL 1／LastError 0となる。Native pointer値は
runごとに異なるopaque artifact identityであり、runtime address policyへimportしない。

Guarded lifetime monitorは16,384 requested pagesのうち先頭30 pagesだけがfree前にaccessされ、
16,354 pagesがuntouchedであることを再現する。Accessed runはpage 0..29／122,880 bytesで、各pageの
first accessはwrite、read／executeはzeroである。First writeはBGI RVA `0x00061AEF`／offset 0、
最後のnew pageはRVA `0x000A3F37`／offset 122,848である。このevidenceはsparse early useとlifetimeを
示すが、Windowsのcommit timing、uninitialized bytes、pointer reuseをsemantic oracleにしない。

Runtimeは32 ordinary heap slotsを維持し、`[0x40000000,0x44000000)`のprocess-permanent exact
64 MiB slotを追加する。Allocationは全rangeをreserveするだけでphysical pageを増やさず、generic
page-fault serviceがRust `RuntimeHeaps` planとGuestMemory transactionを通して4 KiBずつmaterializeする。
Primaryではinstruction 50,123のPFEC 6でfirst pageを`0xA5`初期化し、Node／Chromeはsame allocation／
page identity、memory digest、instruction 51,614の`CreateFileA` stopを再現する。Memory／Worker boundary
変更後の3分gateは326 sessions／failure 0で17 thresholdsを通した。詳細は
[bgi-large-heap-allocation.md](bgi-large-heap-allocation.md)を正とする。

## 102. Immutable source-file gateway

Historical Windows startup traceは最初のtitle-local `CreateFileA`が`${TITLE_ROOT}\BGI.gdb`を開き、main `CreateWindowExA`より前に到達することを示す。旧hookはdesired access／share／creation／flags／LastErrorを保持しないため、そのtraceだけからfile contractを一般化しない。Source manifestはinstalled `BGI.gdb`をobject ID 1、125,461 bytes、SHA-256 `e14899243887737b5fd1bddcf35d9fb3e74dc5f7ff0eea7198e1821f9a4d0f92`としてfingerprintし、private pathやbytesをGitへ入れない。

Public Windows oracleは新しい2,560-byte PE32 fixtureである。SHA-256は`292d298cb77a8223cb0f6fb66333457b3fab9b8e4f0c1643cf01936b769a1d63`、native exitは0である。Fixture自身が`CreateFileA -> GetFileSize -> ReadFile(8) -> SetFilePointer(50) -> ReadFile(11) -> CloseHandle`を実行し、94-byte public source objectのsizeと二つのpayloadを比較する。これによりx86 stdcall ABI、caller-visible BOOL／DWORD output、cursor consumerをWindows self-oracleとして固定するが、Vineのlogical handle数値やasync transportをnative goldenにしない。

Rust `RuntimeFiles`はmanifest／Windows-932 path／share／typed file payload／cursor／pending requestをprivateに所有する。Wrong mount、share conflict、stale manifest plan、wrong／duplicate completion、guest-memory failure、pending close、process exitはobservable stateを部分更新しない。Workerはprepared manifestへRust requestをbindingし、title fingerprint／snapshot／source／object／rangeだけでsame-origin serviceを呼ぶ。

Private Node evidenceは二つのfile lifetimeで同じassetをoffset 0から全量readし、各payload hashがmanifestと一致する。Source executableは実行前後で1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。Chromeもordered calls、二つのsource completion、guest writes、memory digest、next `CreateWindowExA` stopを再現し、pending cancel／watchdog後にRust／Worker recordを残さない。3分source-read workloadは全versioned thresholdを通した。

このevidenceは一objectのread-only Open／Size／Read／Seek／Closeだけを成立させる。Directory enumeration、write、truncate、mapped file、replacement recovery、large-corpus streaming、writable overlay、save／restart／load、SQLite durabilityは未検証である。Stable contractは[runtime-vfs.md](runtime-vfs.md)、backend decisionは[ADR-006](adr-006-vfs-local-server.md)を正とする。

## 103. BGI synchronous logical-window creation

Focused `CreateWindowExA` capturesは次のlocal raw artifactsである。Raw JSONLとprivate absolute pathは
Gitへ追加しない。

| Capture | Bytes／lines | SHA-256 |
|---|---:|---|
| primary | 316,971／793 | `c9c10d854613ff75f3af31485ac00b496a186c0c4de76193e31a6413c3f5e97e` |
| confirmation | 316,932／791 | `371643a615a1b45afbdb2e259653519c0c97f3275d6db78efdb7e1901eedc7fe` |
| callback payload | 319,849／794 | `ee8e544934f6b2c32b91d894bf727d00844abdff9a875fa7b6e6e7b9e09c8be9` |
| invalid-menu consumer | 330,210／794 | `948d2b703e08b94e657dfdf4c695177936f80419b7a54b4cd78f8d0d5508ce2e` |
| default-procedure confirmation | 342,059／833 | `8e1804c23f0c092fb489ed6b55d9e587bc98537f2565a7a3cfb460f73ba32863` |

Main-module `CreateWindowExA`はextended style `0x00040000`、class
`BGI - Main window`、Ethornell title、style `0x80CA0000`、position `(317,154)`、outer size
`806 x 629`、NULL parent／menu／creation parameterを受ける。Registered WndProcはBGI RVA
`0x92550`で、同期callbackは次の順とresultを持つ。

| Message | Input／output | Result |
|---|---|---:|
| `WM_NCCREATE` (`0x0081`) | 48-byte `CREATESTRUCTA` | `1` |
| `WM_NCCALCSIZE` (`0x0083`) | RECT `[317,154,1123,783] -> [320,180,1120,780]` | `0` |
| `WM_CREATE` (`0x0001`) | same creation fields | `0` |
| `WM_SIZE` (`0x0005`) | packed client `800 x 600`, `LPARAM=0x02580320` | `0` |
| `WM_MOVE` (`0x0003`) | packed client origin `(320,180)`, `LPARAM=0x00B40140` | `0` |

WndProcの同じcaller RVA `0x933BF`は`WM_NCCREATE`／`WM_NCCALCSIZE`／`WM_SIZE`／`WM_MOVE`で
`DefWindowProcA`を呼び、結果はそれぞれ`1,0,0,0`である。後二つのpacked `LPARAM`をpointerとして
derefしない。`WM_SIZE`中のBGI RVA `0x9274C`は`GetSystemMenu(NULL,FALSE)`を呼び、NULLと
`ERROR_INVALID_WINDOW_HANDLE` 1,400を受ける。その結果をそのまま渡すRVA `0x92753`の
`EnableMenuItem(NULL,0xF060,0)`は`0xFFFFFFFF`と`ERROR_INVALID_MENU_HANDLE` 1,401を返す。
Callerはmenu resultをbranchせずglobal flagを1にして進む。Native `CreateWindowExA` resultはnon-NULLとして
保存／branchされるが、数値identityはgoldenにしない。

Rust `RuntimeUser32`はclass relation、copied title、geometry、owner thread、logical HWND
`0x48010001`、creation lifecycleを所有する。Callbackはprocess-permanent
`[0x00501000,0x00511000)`を4 KiB depth slotへ分けたscratch arenaを使い、下降stackと分離する。
Scratch mapping、relocation、stack bytes、frame replacement、window mutationはpreflight後にcommitし、
callback rejection／stale replacement／memory failureでwindow identityやallocatorを部分更新しない。
`DefWindowProcA`の`LPARAM` ABIはhandlerがmessage tagを検証するbounded scalar／pointer unionであり、
`WM_NCCALCSIZE`だけが16 guest bytesを書き換える。

Private Node profileは1,394 calls、five creation callbacks、active callback depth 0を通し、外側
`CreateWindowExA`からlogical HWNDを返す。次のstructured stopはinstruction 11,476,518の
`GetSystemMetrics(50)`である。このcheckpointは一般menu identity／mutation、arbitrary window style／parent／
child、visibility、message queue、input、DOM surface、renderingを実装済みとしない。

## 104. BGI small-icon height metric

Focused `GetSystemMetrics` capturesは次のlocal raw artifactsである。Raw JSONLとprivate absolute
pathはGitへ追加しない。

| Capture | Bytes／lines | SHA-256 |
|---|---:|---|
| primary | 372,414／865 | `96a6c3b5ae5496a0f01fde57f39cc9213f68718a840e1d0ef9d7aa54d9b6e08f` |
| confirmation | 372,020／864 | `88b560977c9f07347c638466d5fb8939890119c9d8debcc16cf4c1b16f286dbd` |

両captureでBGIは`SM_CYSMICON`（index 50）と`SM_CXSMICON`（index 49）をこの順に呼び、
どちらも16を受け取る。Return RVAはそれぞれ`0x00067F2D`／`0x00067F32`で、両resultは
`LoadImageA(module,101,IMAGE_ICON,width,height,LR_SHARED)`のheight／widthへ渡される。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Nativeの両metric callはentry時点のLastError 1,401を保存する。Vineのindex 50 callは
LastError 0を保存するが、これは直前のouter `CreateWindowExA` bounded success contractが0へ
戻すためであり、`GetSystemMetrics` handler自身はentry値を変更しない。Callerはこの区間で
LastErrorを読まないため、nativeの1,401をpolicyへimportせず、preserve semanticsとconsumerだけを
oracleにする。

Runtimeはversion-8 User32 policyの`small_icon_height`からindex 50へ16を返し、host／browser
metricをimportしない。Nodeは1,395 calls、generation 1,645、zero guest writeを通し、Chromeも
same ordered result、memory digest、instruction 11,476,522のindex 49 stopを再現する。Index 49は
次checkpointとして未実装のままである。性能／memory／Worker crossingのboundaryを変えない
read-only semantic leafなので、3分gateは再実行せず直前のlogical-window observationを履歴として
保持する。

## 105. BGI small-icon width metric

Section 104のprimary／confirmation capturesは連続するindex 50／49 callsと両consumerを一つの
区間で記録しているため、width checkpointでも同じraw artifactsをoracleとして再利用する。
`SM_CXSMICON`（index 49）は両runで16、return RVAは`0x00067F32`、entry LastError 1,401を
保存し、resultは直後の`LoadImageA`のwidthへ渡る。CallerはLastErrorを読まず、source
executableのsize／SHA-256も不変である。

Runtimeはversion-9 User32 policyの`small_icon_width`から16を返す。Index 50／49は同じ
16×16 profileに属するが、各fieldは独立にdisabledにできるため、過去height checkpointのwidth
stopを再現できる。Handlerはguest memoryとdisplay stateを変更せずentry LastErrorを保存し、
host／browserのDPIやsystem metricsをimportしない。

Nodeは1,396 calls、1,438 writebacks／2,983 dirty pages／1,450,371 changed bytes、generation
1,646を通す。Chromeもsame ordered callsとmemory digest
`e9cedb57025ff173db3692c358ec55b6806a823b9748d88c7f40d83a72886a99`を再現し、46,125
slices、46,113 writebacks／106,461 pages、generation 46,319、47,698 Wasm crossings、terminal
pending work zeroを通す。Instruction 11,476,529でendpoint 193／
`LoadImageA(0x10000000,101,IMAGE_ICON,16,16,LR_SHARED)`へ`endpoint-contract-missing`として
structured stopする。

このcheckpointはsmall-icon width以外の未到達metric、host theme／DPI、`LoadImageA` semantics、
任意image type／resource／size／flagsを実装済みとしない。性能／memory／Worker crossingの
boundaryを変えないread-only semantic leafなので、3分gateは再実行せず直前のlogical-window
observationを履歴として保持する。

## 106. BGI LoadImageA small icon

Focused `load-image-a` capturesは次のlocal raw artifactsである。Raw JSONLとprivate absolute
pathはGitへ追加しない。

| Capture | Bytes／lines | SHA-256 |
|---|---:|---|
| primary | 295,589／803 | `c2196488e32c345f2d74f9ab15cd14774810b3988751439c0232a2d705947cf4` |
| confirmation | 295,572／803 | `239ddf4372885a699af8d8167d6c1eb6af97f320a2b0486697ac1b25a6d253c8` |

両captureは365 balanced API pairs、contiguous sequence、diagnostics zeroである。BGIは
`LoadImageA(main module,101,IMAGE_ICON,16,16,LR_SHARED)`をreturn RVA
`0x00067F43`と`0x000929E9`から二回呼び、同じnon-NULL HICONを受け取る。First callは
LastErrorを1,401から0へし、second callは0を保存する。First resultは直後の
`SendMessageA(hwnd,WM_SETICON,ICON_SMALL,hIcon)`へ渡り、second resultはNULL-checkされる。

独立oracleは同じmodule resourceから16×16、1 plane、32 bppのcolor DIBを選び、success
LastError 0とshared identityを再現する。Missing integer resource 65,535はNULL／LastError
1,813である。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-10 User32 policyで到達済み`IMAGE_ICON`、16×16、`LR_SHARED`だけを許可し、
既存PE resource catalogと`RuntimeUser32` shared icon ownerからresource ID 4／1,128 bytesを
選ぶ。32×32 large icon `0x49010001`と16×16 small icon `0x49010002`は別identityであり、同じ
small resourceの再要求は後者を再利用する。Unknown moduleとmissing resourceは具体的な
Win32 failure、wrong type／size／flags／string resourceはmutation前のstructured stopになる。

Nodeは1,397 calls、1,439 writebacks／2,984 dirty pages／1,450,383 changed bytes、generation
1,647を通す。Chromeもsame ordered callsとmemory digest
`4d9e0c04a72d9a598933803bff7cc40b558416a8a089fba6dccc34faaf81e274`を再現し、46,126
slices、46,114 writebacks／106,462 pages、generation 46,320、47,700 Wasm crossings、terminal
pending work zeroを通す。Instruction 11,476,536でendpoint 229／
`SendMessageA(0x48010001,WM_SETICON,ICON_SMALL,0x49010002)`へ`endpoint-contract-missing`として
structured stopする。

このcheckpointはbitmap／cursor image、default size、file load、copy/delete icon、arbitrary
resource／size／flags、DOM icon、一般message dispatchを実装済みとしない。既存resource／identity
boundary内のsemantic leafなので、3分gateは再実行せず直前の36-session observationを保持する。

## 107. BGI SendMessageA small-icon synchronous callback

Focused `send-message-a` capturesは次のlocal raw artifactsである。Raw JSONL、private absolute path、
native HWND／HICON値はGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 320,348／808 | 367 | `eb88a0376f7863446fadbd9a8d9ab877a90bb966d6ff3b85aee13605479da525` |
| confirmation | 320,408／806 | 366 | `be2a1bd09d8f2e321d117016b5e1f5bbd1d499a4704d915263a7539be0a7719c` |

両captureはcontiguous sequence、diagnostics zeroである。BGI caller RVA `0x00067F58`は直前の
shared small HICONを`SendMessageA(hwnd, WM_SETICON, ICON_SMALL, hIcon)`へ渡す。両runでresultは0、
LastErrorは`0 -> 0`である。Main WndProc RVA `0x00092550`はSend callのdirect childかつsame threadで、
HWND／message 128／`WPARAM` 0／HICONの4 argumentsがouter callと一致する。WndProcもresult 0、
LastError `0 -> 0`を返す。Return直後のcallerは`LRESULT`を読まず、flags `0x559`と二つの
`GetSystemMetrics` resultから次のwindow-position argumentsを組み立てる。Callback後のsecond
`LoadImageA`は同じshared resourceを再要求し、source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-11 User32 policyでこの一variantだけを有効にする。`RuntimeUser32`はmutation-freeに
live HWND、`created` lifecycle、selected threadとowner thread、registered class／WndProc、live shared
16×16 HICONを検証する。既存callback schedulerがscratch 0／20-byte stdcall frameを作り、WndProcの
u32 resultをouter `LRESULT`へ戻す。WndProc内のreached
`DefWindowProcA(hwnd, WM_SETICON, ICON_SMALL, hIcon)`は同じidentity relationを再検証して0を返すが、
logical window iconはまだ変更しない。Wrong／stale identity、wrong lifecycle／thread／variant、callback
preflight failureはguest memory、window／class／icon state、callback allocatorを部分更新しない。

Nodeは74 contracts／1,401 calls、callback frame 7、active depth 0、1,444 writebacks／2,989 dirty pages／
1,450,452 changed bytes、139 host deltas、generation 1,653を通す。Reportは10,378,226 bytes／SHA-256
`1c7a0ee4a0e02830eef104877bdb2bb8c65f684d45744472adc9bf557fbe4b36`、final memory SHA-256は
`0c3a154256959533654179ffed975f89a41ffa983f031a0254c3468514c76b07`である。Chrome Runtime Workerも
same 1,401 calls／11,476,612 instructions／memory digestを再現し、generation 46,326、46,119
writebacks／106,467 pages、139 host deltas、pending work zeroを通す。Browser reportは6,304,008 bytes／
SHA-256 `0dcd59cd61b1900ba6785382490c7e55462250330e152c4a6ba7ac1db58580b3`である。

Instruction 11,476,612でendpoint 202／
`SetWindowPos(0x48010001, 0xFFFFFFFE, 15360, 8640, 0, 0, 0x559)`へ
`endpoint-contract-missing`としてstructured stopする。一般message routing／queue、cross-thread send、
window icon mutation、arbitrary WndProc reentrancy、timeout／cancel、DOM icon／dispatchは未実装である。
既存User32 ownerとcallback scheduler内のbounded gatewayなので、3分gateは再実行せず直前の
36-session observationを保持する。

## 108. BGI SetWindowPos initial placement

Focused `set-window-pos` capturesは次のlocal raw artifactsである。Raw JSONL、private absolute path、
native HWND／HICON値はGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 95,895／132 | 39 | `82c8e13758feb0d498bdf3faa64304d6df9dc36ad50e5661152c3590596eeab7` |
| confirmation | 95,912／132 | 39 | `3601c319492ae04f485c15a00d43787362fe9b7a1ecc923cca859a857a0a8193` |

両captureはcontiguous sequence、diagnostics zeroである。最初のcaller RVA `0x00067F88`は
`SetWindowPos(hwnd, HWND_NOTOPMOST, 15360, 8640, 0, 0, 0x559)`を呼び、BOOL 1と
LastError `0 -> 0`を得る。WndProcは`WM_WINDOWPOSCHANGED`のWINDOWPOS
`[15360,8640,806,629]`／flags `0x10000D59`、packed `WM_MOVE` LPARAM `0x21DA3C03`、
`WM_GETICON(ICON_SMALL2)`をこの順で受ける。`WM_GETICON`は直前のshared 16×16
`LoadImageA`と同じHICON identityを返す。二番目のcaller RVA `0x00067F9F`は
`SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, 0x59B)`を呼び、BOOL 1、LastError
`0 -> 0`、WINDOWPOS flags `0x00001D9F`を得る。Callerは両BOOLをbranchに使わず二番目の
return後も進む。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-12 User32 policyでこのinitial show／hide pairだけを有効にする。先行するnested
`DefWindowProcA(WM_SETICON)`はwindowのsmall-icon relationをowner-local planでcommitし、previous
HICONを返す。Show operationはlogical 1,920×1,080 displayとnon-client policyからouter／client geometryを
planし、`WM_WINDOWPOSCHANGED -> nested WM_MOVE -> same-depth WM_GETICON`を完了してexpected icon
identityを確認した後だけgeometry、visible、topmostをcommitする。Hideもfinal
`WM_WINDOWPOSCHANGED`完了後だけvisibleをfalseへし、show geometryを保持する。Wrong owner／lifecycle／
flags／icon result、callback rejection、stale／duplicate operationではwindow、callback stack、scratch memory、
allocatorを部分更新しない。Runtime WorkerはCreateWindowとSetWindowPos completionのwindow snapshotを
read-only metadataとして記録し、logical window stateを所有しない。

Nodeは75 contracts／5,500 calls、5,547 writebacks／11,243 dirty pages／1,543,704 changed bytes、
4,234 host deltas、generation 9,853を通す。Reportは59,494,402 bytes／SHA-256
`5950440a334c611d25da9c9f79f50b92202faba1f5aa5971fcda6dc1214f2fb6`、final memory SHA-256は
`c74cac1c619df0782a3115a399aac990f4416faf2d16b49c06738caca89c4140`である。Chrome Runtime Workerも
same 5,500 calls／11,583,945 instructions／memory digestを再現し、callback entries 11、replacement 5、
maximum depth 2、logical-window completions 3、generation 54,651、50,347 writebacks／114,969 pages、
4,234 mapping deltas、60,137 Wasm crossings、terminal pending work zeroを通す。Browser reportは
10,793,133 bytes／SHA-256 `f5bb64c3944fc4a0039e0d262abc2eb8b815c6a2e78dff9a647ba9ce3436939b`である。

Workload lengthとcallback／Worker boundaryが変わったため、unchanged 17 thresholdsを持つ新しい
SetWindowPos version-1 configurationで180秒gateを再実行した。29 sessions／failure 0、session p95
6,558.045 ms、slice p99 2.770 msで17/17 thresholdsを通す。Benchmark reportは947,246 bytes／
SHA-256 `18587fccb08508132f74868fc0725755243372174cbafc3ee9891d5887d46e04`である。

Instruction 11,583,945では4,096件のlive critical sectionを保持した状態で、endpoint 147／
`InitializeCriticalSection(0x2004A190)`の次occurrenceを`critical-section-limit-exceeded`として
mutation前に停止する。General z-order／resize／activation、arbitrary flags／insert-after、multiple
windows／monitors、queued paint、DOM positioningは未実装であり、capacity adequacyは次checkpointで扱う。

## 109. BGI critical-section primary capacity

Primary BGI callerだけを対象にしたcritical-section traceはraw JSONLを`.work/`へ保持し、Gitへ追加しない。
Source executableは全captureの前後で1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

| Capture | Duration | Bytes／lines | Max／final live | SHA-256 |
|---|---:|---:|---:|---|
| primary | 10 s | 66,177,215／116,186 | 16,534／16,534 | `119b818f617206c9abcb63c2777279aaf8596f1b522b094c99b5f65960358b45` |
| repeat 1 | 2 s | 8,290,676／14,895 | 5,325／5,325 | `92d293cdfc587237fbe7e733111bc69264f1af287c34c1ac65285691c39f4fda` |
| repeat 2 | 2 s | 8,094,273／14,542 | 5,147／5,147 | `c668e8e6bd831442d4f0f5e3e2937395b7cd2bc003aa96a67b8872417cc53567` |

Primary 10-second captureは`InitializeCriticalSectionAndSpinCount` 17件、plain
`InitializeCriticalSection` 16,517件、`DeleteCriticalSection` 0件、enter 4,156件、leave 4,066件を
記録した。最後のinitializeはmain module loadから8,966 ms後である。したがって4,096／8,192は
primary startupに不十分であり、native最大値を上回る次のpower-of-two 32,768をbounded ceilingとする。
これはtitle callsiteで分岐するpolicyでも、unbounded host allocationでもない。

続くmodule-cursor oracle v2はsystem arrowのLastError preserve／missing 1,814に加え、main-module
resource 106のrepeat shared identity、success LastError 0、missing ID 65,535とwrong-type ID 101の
NULL／1,813を二つのbalanced captureで再現した。Primaryは336,703 bytes／812 lines／370 pairs／
SHA-256 `db9b2bbb24723939dfff8f86ce65c8c784a81b108df459c75007a86014321c5a`、confirmationは
338,341 bytes／815 lines／371 pairs／SHA-256
`f7855d7240e4523b77d65571a0926e28cc4ee63f421a6840c71f86f6c5940ae7`で、diagnosticは0である。
Immutable resource catalogは`RT_GROUP_CURSOR(12)` 106／lang 1,041／RVA `0x0016E9F4`／20 bytesから
`RT_CURSOR(1)` 5／RVA `0x00154F88`／2,220 bytesを選び、32×32／1 plane／8 bpp／hotspot (0,0)を示す。

Rustはmodule source metadataを`RuntimeCursorSource`へcopyし、module registry catalogとgroup／payload
leafをpreflightした後、logical cursor ID 2／HCURSOR `0x43010002`をtargeted commitする。System cursor、
module cursor、iconは別identityで、repeat、stale plan、capacity 256、malformed resource、process teardownの
failure atomicityをmodel／controlled PE testが固定する。

Node primary continuationは17,815 calls／17,862 writebacks／36,000 dirty pages／1,806,863 changed bytes／
16,547 host deltas、generation 34,481へ進む。Reportは207,005,816 bytes／SHA-256
`015528f1469aebec9959f7eba37c7c0593443fbbf3e1daf33da1dac174aa0731`、final memory SHA-256は
`a99b92dda8434553d23213a18d4e468ce784d02da969917c47529ece24e7c71f`である。

Chrome protocol v7のfull evidenceもsame ordered calls／11,806,445 instructions／memory digestを再現する。
512-instruction budgetで40,261 slices、22,399 budget yields、40,255 writebacks／87,857 pages、generation
56,875、74,673 Wasm crossings、terminal pending work zeroを通す。Browser reportは24,156,023 bytes／
SHA-256 `87710b68c70f96b621bb4b8e3bd57dc9a5daa92dd2bd15c0bb9cd83b512b8dc8`である。Cursor leafは
Worker protocol／performance boundaryを変えないため、直前の180秒critical-section gate（30 sessions、
failure 0、17/17 thresholds）を性能正本として保持する。

次stopはinstruction 11,806,445、endpoint 1／`d3d9.dll!Direct3DCreate9(32)`／IAT `0x100D14F4`で、
typed D3D9 frontendがないため`endpoint-contract-missing`を返す。これはroot／device COM identity、capability
query、presentation parameters、logical graphics resource、browser rendererを導入するsubsystem gatewayで
あり、factory returnだけのstubでは閉じない。Cursor checkpointはdecoded bitmap、CSS／DOM cursor、
animation、active cursor selection、cursor destructionを実装済みとしない。

## 110. BGI logical client rectangle

Focused `GetClientRect` capturesは次のlocal raw artifactsである。Raw JSONL、native HWND、
private absolute pathはGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 98,812／259 | 99 | `f1fc35c2afb505d39ed8514c8bf4a969d78bc013ee2fb15c890677ad8afe42a1` |
| confirmation | 98,817／259 | 99 | `151395f4a93715778a75d0262627cf63958a532cf3b26a478dc2c47ce408f05a` |

両captureはcontiguous sequence、diagnostics zeroである。BGI caller RVA `0x0005C997`はlive main
windowへ一回だけ`GetClientRect`を呼び、BOOL 1、LastError `0 -> 0`、RECT
`[0,0,800,600]`を得る。Return後はBOOLをbranchせず、同じHWNDを直ちに`GetDC`へ渡す。
独立oracleはsuccess時にsentinel LastErrorを保存して同じRECTを書き、NULL／invalid HWNDでは
BOOL 0、`ERROR_INVALID_WINDOW_HANDLE` 1,400を返して16-byte sentinel outputを変更しない。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtime handlerは`RuntimeUser32`が所有するlive windowのclient width／heightだけをimmutableに読み、
originをzeroとしてx86 RECTを一つのchecked guest writeで返す。成功はentry LastErrorを保存し、stale／
unknown HWNDはoutputをpreflightせずWin32 failureにできる。Live HWNDとunmapped outputのfailureは
window state、guest memory、allocator、generationを部分更新しない。Browserはlogical HWND、geometry、
LastErrorの第二の正本を持たず、host window size／DPIもimportしない。

Nodeは78 contracts／17,825 calls、17,872 writebacks／36,015 dirty pages／1,807,011 changed bytes、
16,552 host deltas、generation 34,525を通す。Reportは207,140,387 bytes／SHA-256
`e522b40e305bb31ff4d794b4c5ad3eeec3a30853dde5cb72cd5623bdc0ebff56`、final memory SHA-256は
`26693d59c268922fc4cc8a240c6bb21b9c96da51993a077a13c40cbab149c9a2`である。Chrome Runtime Workerも
same ordered calls／11,806,586 instructions／memory digestを再現し、40,271 slices、40,265
writebacks／87,872 pages、16,552 mapping deltas、generation 56,919、74,704 Wasm crossings、terminal
pending work zeroを通す。Browser reportは24,189,645 bytes／SHA-256
`43351ea6a278f010587ab80afe4fe4e4f9ca5397f08de2bd04e2b9f0271ec82d`である。

次stopはendpoint 239／`GetDC(0x48010001)`、IAT `0x100D1494`、return `0x1005C9A4`の
`endpoint-contract-missing`である。このcheckpointはgeneral screen／client coordinate conversion、
child／multiple window、DPI virtualization、DC identity／lifetime、GDI surface、browser renderingを
実装済みとしない。既存window authority内のread-only semantic leafなので3分gateは再実行せず、直前の
runtime-system module lifecycle observationを性能正本として保持する。

## 111. BGI window DC fill slice

Focused `window-dc` capturesは次のlocal raw artifactsである。Raw JSONL、native HWND／HDC／
HGDIOBJ値、private absolute pathはGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 63,019／95 | 17 | `7a793d995671f44e083df46ce77da221600f3cce546ee21b03c3a4e163fbbdae` |
| confirmation | 63,231／96 | 17 | `9c554473a4fda0abc78f4bf4cdb65170db0ea6c3972d76b64f95b47478dd4d23` |

両captureはcontiguous sequence、diagnostics zeroである。Main callerはRVA `0x0005C9A4`で
`GetDC(HWND)`を呼びnon-NULL HDC、RVA `0x0005C9AE`で`GetStockObject(4)`を呼びshared
`BLACK_BRUSH`、RVA `0x0005C9BB`で`FillRect(HDC,[0,0,800,600],brush)`を呼び1、RVA
`0x0005C9CB`で`ReleaseDC(HWND,HDC)`を呼び1を得る。成功callはentry LastErrorを保存する。
Callerは取得identityをそのまま次callへ渡し、fill／release result後も初期化を継続する。

独立negative oracleはinvalid HWNDの`GetDC`をNULL／`ERROR_INVALID_WINDOW_HANDLE` 1,400、invalid
HDCの`FillRect`を0／`ERROR_INVALID_HANDLE` 6、invalid HDCの`ReleaseDC`を0／1,425と固定する。
`GetStockObject(-1)`はNULLでLastErrorを保存する。別identity oracleではclass style `0x28`の同じ
windowに対する同時`GetDC`がdistinct HDCを返す。Release後の数値HDC再利用はcapture間で変動するため、
semantic contractとしてimportしない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Rust `RuntimeGdi`はDC、stock object、surface、index、allocator、generationをprivateに所有する。
Window relationはnon-owningで、kernelが`RuntimeUser32`のlive HWND／owner thread／client geometryを
再検証してからopaque targetを渡す。HDC namespaceは`vine.logical-gdi-dc.v1`、first handle
`0x47010001`で、acquisitionごとにdistinct identityを割り当てる。BLACK_BRUSHは
`vine.logical-gdi-object.v1`／`0x47020001`のprocess-shared objectである。一つの800×600 logical
surfaceはrelease後も残り、full-client black fill一件とsurface generation 1を保持する。Releaseは
matching acquisitionだけを退役させ、process teardownはGDIをUser32より先にpreflight／commitする。
Plan時にsurface次generationまで確定するため、commit開始後にcapacity／overflow failureは起きない。

Nodeは82 contracts／17,829 calls、17,876 writebacks／36,021 dirty pages／1,807,053 changed bytes、
16,552 host deltas、generation 34,529を通す。Reportは207,184,829 bytes／SHA-256
`1c82c43eddd4b4391c1862f4bffa42cdbb686a8aa24c924c115601ab922efd6b`、final memory SHA-256は
`09faccb143347196523dddf962774664e650cd8dc9e08d935d8cf084a5807b78`である。Chrome Runtime Workerも
4 callのarguments／resultsに加え、HDC／object／surface identity、fill、release後live countのimmutable
GDI snapshotまで一致させる。Same 17,829 calls／11,806,624 instructions／memory digest、40,275 slices、
40,269 writebacks／87,878 pages、16,552 mapping deltas、generation 56,923、74,712 Wasm crossings、
terminal pending work zeroを通す。Browser reportは24,193,899 bytes／SHA-256
`dbe371c7278e8129abed1812b01bfd8c83d3a8ecd9c451f990f224a8a24db554`である。

次stopはinstruction 11,806,624、endpoint 212／`SetWindowLongA(0x48010001,-16,0x90CA0000)`、
IAT `0x100D1428`、return `0x1005CAE3`の`endpoint-contract-missing`である。今回のsurfaceはlogical
stateであり、browser pixel buffer、renderer resource、`RuntimeGraphics`、Direct3D deviceを作らない。
GDIとD3D8／9／将来D3D10／11はguest frontendを分離し、共有可能なtranslation seamはvalidated
logical resource／commandより下へ置く。Synchronous logical stateとWorker crossingの性質は変えないため、
直前の180秒runtime-system module lifecycle observationを性能正本として保持する。

## 112. BGI effective window style transition

Focused `set-window-long-a` capturesは次のlocal raw artifactsである。Raw JSONL、native HWND、private
absolute pathはGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 67,283／88 | 6 | `c7ec66fdcce747246943c4cc98b1edfebb9f5b8748ccfd023916a8d4ed1c6283` |
| confirmation | 67,579／89 | 6 | `1b7d6816599493a0db1b5b7d4ff716a5fdc6192ed96f3c2fe51856f81d0c18a6` |

両captureはcontiguous sequence、diagnostics zeroである。BGI caller RVA `0x0005CAE3`はlive main
windowへ`SetWindowLongA(hwnd,GWL_STYLE,0x90CA0000)`を呼ぶ。Creation requested styleは
`0x80CA0000`だが、call直前のeffective styleはWindowsが保持した`WS_CLIPSIBLINGS`を含む
`0x84CA0000`である。First callはprevious effective style `0x84CA0000`を返し、effective styleを
`0x94CA0000`、visibilityをfalseからtrueへ変更する。続く同callerの二回は`0x94CA0000`を返し、
style／visibilityを変更しない。全success callはLastError `0 -> 0`を保存する。Return後のcallerはEAXを
読まず、global windowの有無から分岐して次の`SetWindowPos` argumentsを組み立てる。

独立oracleはlive windowへの同値style設定がprevious `0x94CA0000`を返してsentinel LastErrorを保存し、
stateを変更しないことを固定する。Invalid HWNDは0／`ERROR_INVALID_WINDOW_HANDLE` 1,400、live HWNDの
index -100は0／`ERROR_INVALID_INDEX` 1,413で、いずれもstyle／visibilityを変更しない。Source
executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-14 User32 policyでreached GWL_STYLE variantだけを有効にする。Window creation時の
requested `style`はcallback再現用に保持し、`effectiveStyle`とvisibilityは`RuntimeUser32`が同じlogical
window stateとして所有する。Ownerはlive identity／created lifecycle／thread／icon relationを検証して
opaque planを作り、stale planを拒否してtargeted commitする。同値更新はgenerationを進めず、invalid
window／index、unsupported value、stale planはwindow state、allocator、guest memoryを部分更新しない。
既存`SetWindowPos` show／hideも同じeffective `WS_VISIBLE` bitとvisibilityを同期する。WorkerはRustから
渡されたimmutable snapshotをordered completion検証にだけ使い、Windows styleの第二の正本を持たない。

Nodeは83 contracts／17,830 calls、17,877 writebacks／36,023 dirty pages／1,807,080 changed bytes、
16,552 host deltas、generation 34,530を通す。Reportは207,196,971 bytes／SHA-256
`731d0f1b8b72942a27ba8ed9c785b8e554eaa540eed1a9a7bb911852855d5668`、final memory SHA-256は
`f3f9914e7eb27778890180688a761b95491ed0367b7a43bb3efe7cf334093b7f`である。Chrome Runtime Workerも
same ordered calls／11,806,645 instructions／memory digestを再現し、40,276 slices、40,270
writebacks／87,880 pages、16,552 mapping deltas、generation 56,924、74,714 Wasm crossings、terminal
pending work zeroを通す。Browser reportは24,196,083 bytes／SHA-256
`b9d64968352ec30a7f0ffb5746fe287ecbdff80e3300cb4d0d06d37802e2b091`である。

次stopはinstruction 11,806,645、endpoint 202／
`SetWindowPos(0x48010001,0xFFFFFFFE,0,0,806,629,0x10A)`、IAT `0x100D1400`、return
`0x1005CB4B`の`set-window-pos-size-unsupported`である。General window-long index／value、frame
recalculation、style-dependent non-client geometry、parent／child window、async notification、arbitrary sized
positioningを実装済みとしない。今回のleafはgraphics frontend／logical resource／browser renderer境界を
変更せず、D3D8／9／将来D3D10／11のguest frontend分離方針にも新しい例外を作らないため、直前の180秒
runtime-system module lifecycle observationを性能正本として保持する。

## 113. BGI same-geometry window activation

Focused `set-window-pos-size` capturesは次のlocal raw artifactsである。Raw JSONL、native HWND／z-order
HWND／thread ID、private absolute pathはGitへ追加せずsemantic goldenにも使わない。

| Capture | Bytes／lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---|
| primary | 83,322／97 | 17 | `ef72cfa7b0677d81a302e4201cb6e516a93c1e274741de70809e0ff730a67401` |
| confirmation | 83,309／97 | 17 | `ae19ec55c88ee577ef4a32167b14f47f91f7fffc169de003634adde462d27753` |

両captureはcontiguous sequence、diagnostics zeroである。BGI caller RVA `0x0005CB4B`はvisible main
windowへ`SetWindowPos(hwnd,HWND_NOTOPMOST,0,0,806,629,0x10A)`を一回呼び、BOOL 1、LastError
`0 -> 0`を得る。Callback message／resultは
`WM_WINDOWPOSCHANGING(0)`二回、`WM_ACTIVATEAPP(0)`、`WM_NCACTIVATE(1)`、`WM_ACTIVATE(0)`、
`WM_IME_SETCONTEXT(0)`、`WM_IME_NOTIFY(0)`、`WM_SETFOCUS(0)`の順である。最初のWINDOWPOS entryでは
active／focusがNULL、foregroundがexternal、二回目ではactive／foregroundがtargetでfocusがNULL、IME／
focus messageでは三relationともtargetとなる。Outer／client geometry、effective style、visibilityは不変で、
`WM_WINDOWPOSCHANGED`／`WM_SIZE`／`WM_MOVE`は発生しない。Invalid HWND oracleは0／
`ERROR_INVALID_WINDOW_HANDLE` 1,400を返す。CallerはBOOLを直後のhelper callで上書きし、直接branchしない。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-15 User32 policyのsame-owner-thread／single-window activation variantだけを追加する。
`RuntimeUser32`はthread別active／focus relationとprocess foreground relationをprivateに所有し、window recordへ
booleanを重複させない。最初のWINDOWPOS callback完了後だけactive／foregroundをtargeted commitし、
`DefWindowProcA(WM_ACTIVATE)`はnested IME callbackをpreflightした後にfocusをcommitする。Callback frame 12～19、
same-depth replacement 5回、maximum depth 3を既存scheduler contractで処理する。Wrong owner／lifecycle／
geometry／flags、unexpected result、stale／duplicate callback／focus planではrelation、window、callback stack、
scratch memory、allocatorを部分更新しない。Native z-order HWNDとexternal thread IDはlogical identityへimportせず、
Workerはimmutable completion snapshotだけを検証する。

Nodeは83 contracts／17,841 calls、17,896 writebacks／36,046 dirty pages／1,807,316 changed bytes、
16,562 host deltas、generation 34,590を通す。Reportは207,405,914 bytes／SHA-256
`5d08b53e4c381c76f26af58bd4aee4aceafe8fb17df7b2b204a395e46422b202`、final memory SHA-256は
`13e6603659b09600ef4e43984e8f6dbda97cc274cc9d1bec95adb507de1c7474`である。Chrome Runtime Workerも
same ordered calls／11,807,189 instructions／memory digestを再現し、40,295 slices、22,399 budget yields、
40,289 writebacks／87,903 pages、16,562 mapping deltas、callback entries 19／replacement 10／maximum depth 3、
logical-window completions 5、generation 56,984、74,763 Wasm crossings、terminal pending work zeroを通す。
Browser reportは24,204,712 bytes／SHA-256
`94cd112b26c613f699ca0f827c1fa6984bae21cba946ff405fe8af4c3b212e90`である。

次stopはinstruction 11,807,189、endpoint 206／
`MonitorFromWindow(0x48010001,0x00000001)`、IAT `0x100D1410`、return `0x10059D19`の
`endpoint-contract-missing`である。General resize、z-order、cross-thread／multi-window activation、IME
composition、DOM focusは実装済みとしない。このwindow leafはgraphics frontend／logical resource／browser
renderer境界を変更せず、D3D8／9／将来D3D10／11 frontendの分離方針にも例外を作らない。Worker crossingと
resource topologyを変えるcheckpointではないため、直前の180秒runtime-system module lifecycle observationを
性能正本として保持する。

## 114. BGI logical monitor lookup

独立したCOFF-i386 oracleはhidden top-level window、desktop window、invalid HWND、NULLに対して
`MonitorFromWindow`を呼び、各call前にLastError `0x5A5A`を設定した。Oracle sourceは1,920 bytes／
SHA-256 `9443dcc8744ff0a39ee22485f7ad3f15a3413a1c51306361f117605bb82d271a`、x86 executableは
78,848 bytes／SHA-256 `c499b514839719a8536b663b02f09d98ed098c66cc27b6a8bd8979f53e1cdda9`
で、raw source／binaryは`.work/windows-traces/`に保持しGitへ追加しない。

| Target | Flags | Result | LastError |
|---|---|---|---|
| invalid HWND | `0`／`1`／`2` | NULL | 3 callsとも`ERROR_INVALID_WINDOW_HANDLE` 1,400 |
| hidden window | `0`／`1`／`2` | 同じnon-NULL monitor | sentinel `0x5A5A`を保存 |
| desktop window | `0`／`1`／`2` | hidden windowと同じmonitor | sentinel `0x5A5A`を保存 |
| NULL | `0` | NULL | sentinel `0x5A5A`を保存 |
| NULL | `1`／`2` | 同じprimary monitor | sentinel `0x5A5A`を保存 |
| hidden window | `3` | NULL | `ERROR_INVALID_FLAGS` 1,004 |

Immutable PE disassemblyではRVA `0x00059D13`がglobal main HWNDとflagを渡し、return RVA
`0x00059D19`からHMONITORをそのまま返す。Primary continuationはflag `1`
（`MONITOR_DEFAULTTOPRIMARY`）を一回到達する。RVA `0x00059D22`のcallerは結果をEBXへ保持し、
`IDirect3D9::GetAdapterCount`後のloopでslot 15 `GetAdapterMonitor(adapter)`の返値と比較する。
したがって必要なsemanticsは単なるnon-NULLではなく、D3D9 frontendが後で返すlogical monitorとの
identity一致である。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-16 User32 policyでlive logical HWND＋flag `1`だけを有効にする。Window validityは
`RuntimeUser32`、primary monitor identity／profile／rectangleは`RuntimeDisplays`をauthorityとし、
logical HMONITOR `0x4F010001`を返す。成功はLastErrorを保存し、unknown／stale HWNDはNULL／1,400、
unreached flagはmutation前の`monitor-from-window-flags-unsupported`とする。Display profile mismatch、
missing primary monitor、invalid HWND、unsupported flagではwindow、display、allocator、guest memory、
generationを部分更新しない。Browser display topologyやhost HMONITORをimportせず、Workerはordered
completionのimmutable metadataだけを検証する。

Node summaryは17,842 calls／11,807,197 instructions、17,897 writebacks／36,047 dirty pages／
1,807,324 changed bytes、16,562 host deltas、generation 34,591を通す。Reportは55,357 bytes／
SHA-256 `4a057195fa1855ce98e9f4c57009e391dcae6a560a91005d6c14f28834518cb8`、final memory
SHA-256は`98bd3eae9bde19766e317a90693e7aa6b7fc119b1208070650412ff6bfb37325`である。Chrome
Runtime Workerもsame ordered calls／instructions／memory digestを再現し、40,296 slices、40,290
writebacks／87,904 pages、16,562 mapping deltas、generation 56,985、74,766 Wasm crossings、terminal
pending work zeroを通す。Browser reportは193,673 bytes／SHA-256
`e95b09a2d9e6d9526d57233d2a9877f7306954152e837fe632b346961098c195`である。

このread-only semantic leafはWorker protocol、resource topology、memory ownershipを変えないため、
直前のpassing 17,841-call／32-session observationを性能正本として保持する。確認のため同じ17 thresholdsで
current workloadを180秒ずつ二回測定したが、unrelatedなuser-owned VM／application workloadが約3 CPU
coresを継続消費し、空きmemoryが約2.5 GiBのhostでは、19 sessions／p95 10,738.750 msと
18 sessions／p95 11,625.325 msとなった。両runともsemantic failureは0で、failed reportはそれぞれ
940,024 bytes／SHA-256 `678c3316c55867d0123421b76fa59f59ca0cdc0611871a23da8a2b5f74133221`、
914,066 bytes／SHA-256 `364cb93d7986307debd233f4b79dd5222018d60068b6ccd7ffcf15893d3a79e5`
として`.work/`へ保持する。First runだけWorker heap capacityも69,636,096 bytesで失敗し、second runは
63,606,784 bytesで通過した。Thresholdを緩和せず、これらをpassing evidenceへ昇格しない。

同じhost load下のNode A/B各3回では直前profileが平均41.031秒、新profileが41.466秒で、差は各runの
分散内だった。これによりMonitorFromWindow一call／8 instructions固有の退行ではなく、clean performance
oracleを満たさないambient loadと分類する。NULL、flags `0`／`2`、multiple monitor、nearest-monitor
geometry、host／browser topologyはnative観測済みでもreached contractへ昇格しない。

次stopはinstruction 11,807,197、`IDirect3D9` slot 4／`GetAdapterCount()`、this
`0x01000004`、return `0x10059D36`の`com-method-unsupported`である。`GetAdapterCount`、
`GetAdapterMonitor`、multi-adapter enumeration、host GPU topology、graphics resource、rendererは今回の
checkpointで実装済みとしない。

## 115. BGI D3D9 logical adapter count

既存のselected-`IDirect3D9` trace v2
`.work/windows-traces/sakura-idirect3d9-v2-clean-20260711-035657.jsonl`を
adapter-count oracleとして再利用する。Raw traceは2,267,972 bytes／7,133 records／SHA-256
`cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898`、summaryは
58,021 bytes／SHA-256
`3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9`で、diagnosticsは0である。
BGI direct caller RVA `0x00059D36`からslot 4 `GetAdapterCount()`へ6 calls到達し、すべてunsigned
result `1`、observed LastError `0`である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

Immutable PE disassemblyではreturn valueをEDIへ保存し、adapter indexを`[0, count)`で反復する。
各iterationはslot 15 `GetAdapterMonitor(index)`を呼び、返ったHMONITORを直前の
`MonitorFromWindow` resultと比較する。したがってreached contractは一つのlogical adapterと、そのadapterが
primary logical monitorへ対応する後続identity relationである。Host adapter countやGPU topologyはgoldenにしない。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-6 D3D9 policyの`single-logical-adapter-v1`だけを有効にし、unsigned `1`を返す。
Countは`D3d9CompatibilityPolicy`、profile存在性は`RuntimeDisplays`がauthorityを持つ。Successはguest memory、
LastError、COM reference count、object lifetime、generationを変更しない。Disabled／malformed policy、missing／
mismatched display profileはcompletion前のtyped faultとなり、allocator、COM state、display stateを部分更新しない。
`RuntimeGraphics`やbrowser renderer resourceは作らず、TypeScript側へWindows adapter stateを複製しない。

Node summaryは17,843 calls／11,807,216 instructions、17,898 writebacks／36,048 dirty pages／
1,807,333 changed bytes、16,562 host deltas、generation 34,592を通す。Reportは55,353 bytes／
SHA-256 `63783b504b76f1dbb6b17d6014c9bbb75c60436bed3630e779b8fa76606b54c1`、semantic digestは
`fc823022523c04a1fc8c7f6b6f751c4aaaac532040ec084fd366b119cf429d30`、final memory SHA-256は
`3690b976c824226903ee7905169b12fe7e4fa715b3154f1976b368573bb5aed4`である。Chrome Runtime Workerも
same ordered calls／instructions／digestを再現し、40,297 slices、40,291 writebacks／87,905 pages、
16,562 mapping deltas、generation 56,986、74,769 Wasm crossings、terminal pending work zeroを通す。
Browser reportは193,690 bytes／SHA-256
`4b66547957a4732d294f01d22e0cc819836022d9a625e21e3b09ac72ec3a3b45`である。

このread-only semantic leafはWorker protocol、resource topology、memory ownership、performance-sensitive
translation pathを変更しないため、直前のpassing 32-session observationを性能正本として保持する。
Multi-adapter、mode enumeration、host／browser GPU topology、`RuntimeGraphics` device、rendererは未実装である。
次stopはinstruction 11,807,216、`IDirect3D9` slot 15／`GetAdapterMonitor(0)`、this
`0x01000004`、endpoint `0x01001120`、return `0x10059D5E`の`com-method-unsupported`である。

## 116. BGI D3D9 logical adapter monitor

Section 115と同じselected-`IDirect3D9` trace v2をadapter-monitor oracleとして再利用する。Raw traceは
2,267,972 bytes／7,133 records／SHA-256
`cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898`、summaryは
58,021 bytes／SHA-256
`3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9`、diagnosticsは0である。
BGI direct caller RVA `0x00059D5E`からslot 15 `GetAdapterMonitor(0)`へ6 calls到達し、すべてnative
opaque non-NULL result、observed LastError `0`である。Native numeric HMONITOR `0x00010001`はartifact
identityであり、guest semantic valueにはしない。

Boundary caseは新しいCOFF-i386 oracleで固定した。Sourceは5,089 bytes／SHA-256
`337ddf83ec68ba80a25d70f1f9a9d0361a83833657ca7c852075043d16b57c67`、executableは3,584 bytes／
SHA-256 `1d2681032fad258d9930c1a4a88ac6d4cf67d53b7d26ff447a17d5129bb436d5`である。独立した2 runsは
同じ結果となり、adapter count `1`、adapter `0`はprimary identityと一致するnon-NULL、adapter `1`と
`UINT_MAX`はNULL、三callすべてsentinel LastError `0x5A5A`を保存した。Oracle source／binaryとraw
outputは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-7 D3D9 policyの`primary-adapter-monitor-v1`でadapter `0`だけを
`single-monitor-1920x1080-work-1920x1032-v2`へ対応付ける。Mappingは`D3d9CompatibilityPolicy`、HMONITOR identityは
`RuntimeDisplays`がauthorityを持つ。Valid adapterはlogical primary handle `0x4F010001`、invalid adapterは
NULLを通常完了として返す。いずれもLastError、guest memory、COM reference count、object lifetime、
runtime generationを変更しない。Disabled／malformed policy、missing monitor、display-profile mismatchは
mutation前のtyped faultとなる。Host monitor identity、browser topology、`RuntimeGraphics`、renderer resourceは
import／生成しない。

Nodeはsequence 17,844／instruction 11,807,216とsequence 17,848／instruction 11,807,374の二callで
同じ`0x4F010001`を返す。間には既存`GetAdapterDisplayMode`、二回目の`MonitorFromWindow`、二回目の
`GetAdapterCount`が順序どおり到達する。Summaryは17,848 calls／17,903 writebacks／36,054 dirty pages／
1,807,411 changed bytes、16,563 host deltas、generation 34,598を通す。Reportは55,347 bytes／SHA-256
`8525d1e2a8ca0f0435754d54904cb5cd0b05c6f49b6c636b8cb787d2a7e6c252`、semantic digestは
`72aaa892443446bb565b11908c6f01697bcd58110f1f15e281934eaf1eca2460`、final memory SHA-256は
`b09f1911d1718c3a0e8400e7499f5e86bcc223ba9d790f54b17c8d933427fd05`である。

Chrome Runtime Workerもsame ordered calls／instructions／digestを再現し、40,302 slices、40,296
writebacks／87,911 pages、16,563 mapping deltas、generation 56,992、74,784 Wasm crossings、terminal
pending work zeroを通す。Browser reportは193,686 bytes／SHA-256
`e2a46cc75fa1e043a0a69bf6164a84feabe9caf368f4c44a77a96b0794bc6aef`である。Controlled PE fixtureも
adapter `0` non-NULL／past-end NULLを含めて1,966 instructions、exit `0`を通し、Canvas pixel、audio、
storage、cancel／watchdog regressionを維持する。

次stopはinstruction 11,807,387、`IDirect3D9` slot 16／`CreateDevice`、this `0x01000004`、adapter `0`、
device type `1`、focus window `0x48010001`、behavior flags `0x44`、presentation parameters
`0x101038F0`、output pointer `0x101517C4`、endpoint `0x01001130`、return `0x1005ACA5`の
`com-method-unsupported`である。これは最初のlogical device、presentation state、graphics resource owner、
browser renderer transaction boundaryを必要とするsubsystem gatewayであり、API単体leafとして実装しない。

## 117. BGI D3D9 logical device creation

Focused `d3d9-create-device` observationを独立に2回取得した。Primary raw traceは2,333,677 bytes／
7,075 records／3,498 balanced API pairs／SHA-256
`df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071`、summaryは275,843 bytes／
SHA-256 `b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60`である。Confirmation
raw traceは2,355,307 bytes／7,121 records／3,521 balanced pairs／SHA-256
`2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3`、summaryは275,855 bytes／
SHA-256 `a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b`で、両summaryともdiagnosticsは0である。
Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceにはsuccessful `IDirect3D9::CreateDevice`が3 callsある。Adapter `0`、`D3DDEVTYPE_HAL`、behavior
flags `0x44`で、input `BackBufferWidth／Height／Format`は全て`0／0／0`、他のpresentation fieldsは
`BackBufferCount 1`、multisample `0／0`、`D3DSWAPEFFECT_DISCARD`、Windowed `1`、auto depth-stencil
disabled、format `D3DFMT_D16`、残りzeroである。Nativeは一callを`800x600／format 22`、二callを
`1280x720／format 22`へ解決し、remaining fieldsを保存してnon-NULL deviceと`S_OK`を返した。Observed
LastErrorは0で、BGI direct caller RVA `0x0005ACA5`はHRESULT signを消費する。新しいfull 119-slot
`IDirect3DDevice9` vtable observationでは最初の到達methodがslot 23 `CreateTexture(1024, 1024, 1, 0,
22, 2, output, 0)`であり、その内側のdevice `AddRef`は`2`を返す。Native HWND、COM pointer、vtable、
resolved desktop geometryはartifact identityであり、Vine guest identityへimportしない。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-8 `primary-windowed-hal-device-v1`だけを実装した。Sequence 17,849／instruction
11,807,387でroot `0x01000004`から呼ばれ、live HWND `0x48010001`のclient geometryを使ってpresentation
prefixへ`200300005802000016000000`を書き、device pointer `0x01010004`をoutputへ書いて`S_OK`を返す。
`RuntimeCom`はobject ID 3／interface ID 6のchildをreference count 1でpublishし、root countはretained
referenceにより2となる。`RuntimeGraphics`はlogical device ID 1とpresentation `800x600／format 22／
one buffer／one sample／discard／windowed`を所有する。Guest 16 bytes、COM identity、graphics associationは
全owner preflight後にpublishされ、final device releaseはgraphics associationをretireして必要ならroot
retirementへcascadeする。Browser renderer resourceは生成しない。

Node summaryは17,849 calls／11,807,772 instructions、17,904 writebacks／36,055 dirty pages／
1,807,469 changed bytes、16,564 host deltas、generation 34,733を通す。Semantic SHA-256は
`d062bba8eef984451f149034d3bb1f27b2d92a81184be5ffcfa4fcddb8f74f14`、final memory SHA-256は
`5317e97d117c7599c9900bb903c4ab184ab4f97db3cad6d9c6f951ba1941dbd1`、reportは82,645 bytes／
SHA-256 `0ea001699dc1de9669657eef81f3679e78be6b4981e84127d86b5d376f72f7e8`である。

Chrome Runtime Workerもsame ordered completion／instructions／semantic digestを再現し、29,082 slices、
11,178 budget yields、29,080 writebacks／61,961 pages、16,564 mapping deltas、17,755 mapping dirty pages、
generation 27→45,911、63,571 Wasm crossings、87,212 success execution transitions、terminal pending work zeroを
通す。Elapsedは5,692.240 ms、maximum sliceは3.660 ms、1,141 heartbeat samplesのmaximum gapは
31.035 msである。Browser reportは116,599 bytes／SHA-256
`94a2bd202dfab3bfab47f0b70f7c75d1195f4efbbde80996c7a4414422a31601`である。Wasm artifactは
3,420,622 bytes／SHA-256 `6dd86d69ad90ccf2bbe705ad08aaec42fc3555280bda8e721c5b2bf0c7118e5a`である。

次stopはinstruction 11,807,772、`IDirect3DDevice9` slot 23／`CreateTexture`、this `0x01010004`、
arguments `1024, 1024, 1, 0, 22, 2, 0x101517CC, 0`、endpoint `0x010111A0`、return
`0x1005A715`の`com-method-unsupported`である。Texture identity／storage／lock、renderer、draw、Present、
Reset、device loss、first visible frameはこのcheckpointで実装済みとしない。

## 122. BGI D3D9 initial texture sequence

Focused `d3d9-initial-textures` observationを独立に2回取得した。Primary raw traceは261,943 bytes／
505 records／219 balanced API pairs／SHA-256
`2fe16a38d7a8c8ee18b673d06f759ef3bfa3d412d3f688689692edb4d6d55ad4`、summaryは328,531 bytes／
SHA-256 `85db2bcea0fb895c1481a3af7330a3c79b430cbed997761cdb23d30e0fbf875d`である。Confirmation raw
traceは262,766 bytes／508 records／220 balanced pairs／SHA-256
`cdf646718a1a187e72129d9ae5562d44b4c5432c7c9c60ee1c217d30ae90179f`、summaryは328,698 bytes／
SHA-256 `20eb05e775a84075aec1c48b2137c9d237f684e2e19d580b1aac5671a7039837`で、両summaryともdiagnosticsは
0である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceは同じnative deviceからBGI caller RVA `0x0005A715／0x0005A7B7／0x0005A7DC`の
`IDirect3DDevice9::CreateTexture`を三回直接完了する。三variantは順に
`1024×1024／level 1／usage 0／format 22／pool 2`、同shapeの`pool 0`、
`usage 0x200／pool 0`で、shared handleは全てNULL、resultは全て`S_OK`、observed LastErrorは0である。
Output addressは`0x00D117CC／0x00D117D0／0x00D117D4`、native texture pointerは全てnon-NULLかつ
trace内でdistinctである。Pointer値はopaque artifact identityでありVine guest identityへimportしない。
三call成功後の次methodはdevice slot 26 `CreateVertexBuffer(112, 8, 0x144, 1, ..., NULL)`である。
Caller inspectionと実行traceにより、従来の`MessageBoxA`は第二textureの失敗branchであり、独立した
primary capability stopではないと分類した。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-12 `primary-initial-x8r8g8b8-textures-v1` allow-listを実装した。Sequence
17,850／17,857／17,858、instruction 11,807,772／840／856で同じdevice `0x01010004`から呼ばれ、
guest output `0x101517CC／0x101517D0／0x101517D4`へdistinct texture pointer
`0x01020004／0x01030004／0x01040004`を各4 bytes publishして`S_OK`を返す。`RuntimeGraphics`は
one system-memory static、one default-pool static、one default-pool dynamic descriptorをlogical texture
ID 1～3として所有する。`RuntimeCom`は三childからdevice referenceを一つずつretainし、terminal device
reference countは4である。各guest write、COM identity、graphics associationは全owner preflight後にだけ
publishされ、unsupported variant、unwritable output、capacity、stale planはguest bytes、allocator、COM
refcount、graphics index／generationをpartial mutationしない。DEFAULT poolもlogical descriptorだけで、
pixel storage、GPU allocation、browser renderer resourceは生成しない。`GetSurfaceLevel`はnativeで到達した
variant index 0のsystem-memory textureだけにboundし、二DEFAULT textureへはmutation-free typed unsupportedで
止まる。

Node summaryは17,858 calls／85 contracts／11,807,897 terminal instructions、17,913 writebacks／36,064
dirty pages／1,807,590 changed bytes、16,570 host deltas、generation 34,883を通す。Semantic SHA-256は
`1ced14993c500a6adff24ccba93fbb4db2b72e697cbb0efab5096ff2e2dfe53d`、final memory SHA-256は
`3ff9351545b113f8cdd109d8dfcbfda92547a7eedddb22cc2fe1e02d50799d78`、reportは82,645 bytes／
SHA-256 `0f6d63a5994607aea9947d4500efd580c74b99e05959ab7f860acdcfe2376286`である。

Chrome Runtime Workerもsame ordered semantics／instructions／digestを再現し、29,089 writebacks／61,970
pages、16,570 mapping deltas／17,768 dirty pages、generation 27→46,061、63,602 Wasm crossings、87,239
success transitions、terminal pending work zeroを通す。Elapsedは3,232.830 ms、maximum sliceは2.710 ms、
648 heartbeat samplesのmaximum gapは20.725 msである。Browser reportは116,647 bytes／SHA-256
`fb42845fe87bc550a83e41b0c97d15d4af524334fa998de367af142512bef9b5`、Wasm artifactは3,578,969
bytes／SHA-256 `ad905bca78caf37d78f1feecc43bd34b6af7440a44c8260f8e0aa5343cf25f01`である。

同じ180-second fixtureと既存17 thresholdsのbenchmarkは63 sessions、semantic failure 0、session P95
2,950.410 ms、slice P99 2.900 ms、heartbeat P99／maximum 5.995／18.655 msで17／17を通した。
Reportは946,980 bytes／SHA-256
`ce4884b30af587bda891ed611022980620f64be802f9c1172be67620140069a0`であり、threshold変更やretry
acceptanceは行っていない。

次stopはinstruction 11,807,897、`IDirect3DDevice9` slot 26／`CreateVertexBuffer`、this
`0x01010004`、arguments `0x70, 0x8, 0x144, 0x1, 0x101517DC, 0`、endpoint `0x010111D0`、
return `0x1005A890`の`com-method-unsupported`である。Vertex-buffer identity／storage、texture pixel
storage、Lock／Unlock、renderer、draw、Present、Reset、device loss、first visible frameはこのcheckpointで
実装済みとしない。

## 123. BGI D3D9 initial vertex buffer

Focused `x86-bgi-initial-vertex-buffer-v1` observationを独立に2回取得した。Primary raw traceは
286,932 bytes／512 records／222 balanced API pairs／SHA-256
`0a1719739f86eaa42353e0c762e802119225e30975a318a29669e9aea2ea5c30`、summaryは282,623 bytes／
SHA-256 `68133eb819c3055d58580f2bc63dff5b56c116aff6d5ff9762c4428108e1c849`である。Confirmation raw
traceは285,889 bytes／509 records／220 balanced pairs／SHA-256
`e2f7c1cc1d9c073d9325bf05d7f46a44e80bbbce54b0f8ffe08684f6bf393ed8`、summaryは282,874 bytes／
SHA-256 `7835288860d359c695d98bb7cbc19ecbc004f1d05ec6f54d384eaa7356ec3700`で、diagnosticsは両runとも
0である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceは同じnative device／vertex bufferに対し、BGI caller RVA
`0x0005A890／0x0005AA96／0x0005AAB2`からslot 26 `CreateVertexBuffer`、slot 11 `Lock`、slot 12
`Unlock`を順に完了する。Create argumentsは`112, 8, 0x144, 1`、shared handleはNULL、Lock argumentsは
offset／size／flagsすべて0で、三resultは`S_OK`、observed LastErrorは0である。Create後の112 bytesは全て
zeroでSHA-256 `b5fdab78d8947eacc864bfeecb4d2100780e5afe1cd8efafb124887913ac49fa`、callerがcopyした112-byte
payloadはSHA-256 `d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07`である。Native pointerは
opaque artifact identityとして扱う。直後はdevice slot 57 `SetRenderState(22, 1)`へ進む。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-13 `primary-initial-xyzw-diffuse-tex1-quad-v1`を実装した。Sequence
17,859／17,860／17,861、instruction 11,807,897／11,808,429／11,808,439でlogical device
`0x01010004`からvertex buffer `0x01050004`を作り、Lock output `0x006FFD54`へRust-owned backing
`0x44000000`をpublishし、Unlock後はread-onlyへ戻す。`RuntimeGraphics`はlogical vertex-buffer ID 1、
device relation、112-byte descriptor、single active lock、content generation 1を所有し、byte authorityは
`GuestMemory`に置く。Terminal device reference countは5、vertex-buffer reference countは1である。
CreateはCOM／address-space／graphics／memoryを全てpreflightしてからpublishし、Lock／Unlockもguest write、
protection、graphics stateをfailure-atomicにcommitする。Final ReleaseはCOM backing、resource backing／owner、
address records、graphics associationをまとめてretireする。Browser renderer／GPU resourceは作らない。

Node summaryは17,861 calls／11,808,458 terminal instructions、17,916 writebacks／36,070 dirty pages／
1,807,769 changed bytes、16,573 host deltas、generation 34,920を通す。Semantic SHA-256は
`6d4e217e8f8737c41c279ed3da18ea4b079f7bec1a5a6cf09ec420513c5f0ff9`、final memory SHA-256は
`c25cc143829b020bdfb3cfebc393046a1f5ae77f6b3e5197c3697aad37d13fa5`、reportは82,585 bytes／
SHA-256 `7c943a93c7d58c7b12854aa3cdc4e42eceacfbec6fb0c93953c03717ef61ac6b`である。

Chrome Runtime Workerもsame ordered semantics／instructions／digestを再現し、29,092 writebacks／61,976
pages、16,573 mapping deltas／17,773 dirty pages、generation 27→46,098、63,614 Wasm crossings、87,248
success transitions、terminal pending work zeroを通す。Browser reportは116,641 bytes／SHA-256
`9eb47d12e8a8a47cabd5bcfde05a940bae07b362fb7e2cb9b254ad6f6d627b8c`、Rust core Wasmは3,654,703
bytes／SHA-256 `b37c6f49d0954d49c73a6ca0a76a75913384a797b0749b17f9a96118a5f5bc4d`である。

同じ180-second fixtureと変更しない17 thresholdsのbenchmarkは64 sessions、semantic failure 0、session P95
2,914.615 ms、slice P99 2.585 ms、heartbeat P99／maximum 5.995／20.855 msで17／17を通した。Reportは
955,155 bytes／SHA-256 `c80155da249b02dc5aab121732bcb96a0853577cbcffe63b68f1acd5f7ce50d0`であり、threshold変更、
retry acceptanceは行っていない。

次stopはinstruction 11,808,458、`IDirect3DDevice9` slot 57／`SetRenderState`、this `0x01010004`、
arguments `22, 1`、endpoint `0x010113C0`、return `0x1005AD13`の`com-method-unsupported`である。
Render-state authority、draw、Present、Reset、device loss、first visible frameはこのcheckpointで実装済みとしない。

## 124. BGI D3D9 initial render states

Focused `x86-bgi-initial-render-states-v1` observationを独立に2回取得した。Primary raw traceは
208,326 bytes／511 records／222 balanced API pairs／SHA-256
`db86df8455db6ed3e2c36b13618730b7d40b6750012136308148a7fc302d86e9`、summaryは180,986 bytes／
SHA-256 `31202c168e9f22f0cec2570d41cdadd95ef9eab602e3bab3e1c8b409f2bafa4b`である。Confirmation raw
traceは208,999 bytes／512 records／222 balanced pairs／SHA-256
`66e7b34f2900bbf5470c6433bcd0d7ea0818e87396cc2108d5ee72a163b8a704`、summaryは181,209 bytes／
SHA-256 `1d04f1ec63c1cf3769f97267bf06bc6133e21a08aedd9173fa6af5a59c934100`である。両runともtrace
diagnosticは0、sequenceはcontiguous、API pairingはbalancedである。

両runは同じdevice／thread内でcaller RVA `0x0005AD13／0x0005AD26／0x0005AD3C／0x0005AD50`
からslot 57 `SetRenderState`を4回連続して完了する。State／valueは順に`22／1`、`7／0`、
`137／0`、`27／1`、全resultは`S_OK`、observed LastErrorは0である。Device pointerとthread IDはrun間で
異なりopaque artifact identityとして扱う。Source executableはbefore／afterとも1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`であり変更していない。

このevidenceはexact startup clusterとorderingを固定する。未到達render-state variant、Windows default state
全体、GPU renderer、draw、Present、Reset、device lossを一般化しない。Runtime／Node／Chromeのsemantic
evidenceと次の`GetRasterStatus` stopはcurrent checkpoint manifestを正本とする。

## 125. BGI D3D9 raster status

Focused `x86-bgi-first-raster-status-v1` observationを独立に2回取得した。Primary raw traceは
261,147 bytes／505 records／SHA-256
`31ae84a21f5a70c4ae25f3234f52bf229b08e06ca309bbac1567509e7573362a`、summaryは179,845 bytes／
SHA-256 `2d22afec1516f9cb1c2637eb641c6dc54c99f9d9fb3817add90d6fd40f76c724`である。Confirmation raw
traceは263,079 bytes／510 records／SHA-256
`491b6e252c957cc676b8d6dda981b15e74b5796ad9f7b327fbe9bd861b5eea7d`、summaryは180,079 bytes／
SHA-256 `e72d2b789c77f2005408dd4517179858939ac02559679fd11e5b20301f428069`である。両runともtrace
diagnosticは0、source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のまま変更していない。

両runはcaller RVA `0x0005AEDD`からdevice slot 19 `GetRasterStatus(0, output)`を一回完了し、`S_OK`、
observed LastError 0、8-byte full output overwriteを示す。Primaryは`InVBlank=FALSE／ScanLine=1200`、
confirmationは`InVBlank=TRUE／ScanLine=0`であり、具体値はhost scanoutに依存するartifact observationである。
Immediate consumerはHRESULTを検査し、`InVBlank`で分岐してscanlineを上限比較へ使い、VBlank時は
`0x81000000`を返す。Outer consumer RVA `0x0006A263`は`S_OK`とそのsentinelの両方を受理し、再sampleして
`Sleep(0)`でyieldする。このためVineはcaller／hashで分岐せず、versioned policyの安定したnonblank
`ScanLine=1`を返せる。Native device／output pointerはopaque identityとして扱う。

Node／Chromeはcall sequence 17,866、instruction 11,808,586、arguments
`0x01010004／0／0x006FFCF0`、return `0x1005AEDD`、output
`0000000001000000`を一致させる。次stopはinstruction 11,808,626の
`kernel32.dll!QueryPerformanceFrequency(0x006FFCF0)`、endpoint ID 66／`0x7F000410`、IAT
`0x100D11B0`、return `0x10091FD7`、reason `endpoint-contract-missing`である。Renderer、draw、Present、
host／browser scanout importはこのcheckpointで実装済みとしない。

## 126. BGI high-resolution performance clock

`query-performance-frequency` focused observationを独立に2回取得した。Primary raw traceは
186,666 bytes／469 records／SHA-256
`d8d40e8dece88280ab1d35283b143359a447d3608b4abc5bf80e705198d86790`、summaryは36,690 bytes／
SHA-256 `5a7b742f190a59cd9d50df2deca1283e3f88356fc98de88119815e97964177ab`である。Confirmation
raw traceは185,056 bytes／464 records／SHA-256
`1d3b500b87d2482ffbce59f0363c1506e81683b89aba476adc53184fad637a03`、summaryは36,866 bytes／
SHA-256 `5e144ad4a2b037ddc1ebc15a9151134df430b0d9fff18237cdeaa060c45fc40d`である。両summaryの
diagnosticは0、source executableは前後とも1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`で変更していない。

`x86-stable-performance-frequency-v1` oracleはLastError sentinel `0／87／0x12345678`で三回ずつ
`QueryPerformanceFrequency`を呼ぶ。全callがBOOL 1、guard保持、LastError保持、full eight-byte
`10,000,000`を返し、call間／run間で一致した。BGI callはcaller RVA `0x00091FD7`から一回到達し、
同じ10 MHzを得る。RVA `0x00091FD0`のwrapperは失敗時だけfrequencyを1000へ置換し、RVA
`0x00092050`のconsumerはfrequencyとcounterをx87 `fild`で読み、counterをfrequencyで除算する。

`query-performance-counter` progression observationも独立に2回取得した。Primary raw traceは
241,192 bytes／477 records／SHA-256
`976b08a862fb847f55460ad0c4c6fa00b50c71dcd250beda60310c5fd60f0398`、summaryは38,763 bytes／
SHA-256 `9d13cdace9dc15366319fa8570efc451d66481a81779acbf3f733515623cedf0`である。Confirmation
raw traceは284,763 bytes／477 records／SHA-256
`7f77b57c662d2bcacb3c5db417bb7de07412df1388eea390eb1a9a5b1c1f71d9`、summaryは39,582 bytes／
SHA-256 `0fd05c7688e7330b9644ad9d908e2e1ecddb15a4f7513e57a3e5f8cd9134e32a`である。Four-call
`x86-monotonic-performance-counter-v1` oracleの連続差分はprimary `4,396／4,020／3,984`、
confirmation `3,791／3,117／3,349`で全て正であり、全callがBOOL 1、LastErrorとguardを保持した。
Concrete counter値はhost-dependent artifact observationで、Vine policyへimportしない。

BGI initial caller RVA `0x000A88FA`とtarget caller RVA `0x00092017`の両方へ到達した。Confirmationで
target callerの連続sample差は17,953 ticks、10 MHz換算1.7953 msである。RVA `0x00092017`の
wrapperは成功時に64-bit outputをそのまま使い、失敗時だけ`GetTickCount`へfallbackする。このconsumer
evidenceから、Vineはcaller／occurrence分岐やhost sample copyではなく、process-owned frequencyと
deterministic progressionを実装する。

Runtimeはfrequency 10 MHzとcounterを`RuntimeProcessState`の単一clock authorityに置く。Frequency
queryはmutation-free、counter queryはactive policyの10,000-tick incrementをmutation-free planで
preflightし、running lifecycle、previous counter、signed `LARGE_INTEGER`上限をstale-checkしてtargeted
commitする。Zero increment、range exhaustion、stale plan、invalid outputはobservable clock／memoryを
部分更新しない。Host QPC、browser clock、Worker slice、WinMM resolution、frame／audio clockは取り込まない。

Node semantic runはfrequencyをsequence 17,867／instruction 11,808,626で完了し、既存initial sampleを含む
301 counter callsを`12,345,678,901`から10,000ずつ進める。18,438 supported calls、18,493
writebacks／36,650 dirty pages／1,816,818 changed bytes、17,145 host deltas、generation 36,069を通し、
semantic SHA-256は`95628a4df3257ac8eee45d7cb1f6cd7af87bf7e5dbeb8b60e64f309920c74151`、
final memory SHA-256は`7028e80eae6c2bfcfd7ffaed6c7ce9fa8ae98e7fbe338e3db98aa5b79ce3573d`である。
Summary reportは50,097 bytes／SHA-256
`6983198d0fbd6fbcab21b8ef2f9df6de6f98aa04e54b39d166ea06712605e829`である。次stopはinstruction
11,824,580の`USER32.dll!GetMonitorInfoA(0x4F010001,0x006FFD44)`、endpoint ID 200／
`0x7F000C70`、IAT `0x100D13F8`、return `0x1005C913`、reason `endpoint-contract-missing`である。
Chrome Runtime Workerもsame semantic digest／memory／stopを再現し、19,186 writebacks／38,262 pages、
17,145 mapping deltas、generation 27→36,764、55,131 Wasm crossings、57,530 execution transitions、
terminal pending work 0を通した。Browser reportは43,931 bytes／SHA-256
`fd7bb3358736c8e723f875411bee56c35141e5104450e2c2762c1a803df712e0`である。同じ180-second／
17-threshold fixtureは127 sessions、semantic failure 0、session P95 1,471.575 ms、slice P99 1.675 ms、
heartbeat P99 6.055 msで17／17を通した。Benchmark reportは1,958,180 bytes／SHA-256
`333ae2125e48e5d63c1be7622b11bd7395566f97c01b81344bd268b0ad0bc932`である。Current artifact
identityとcanonical reproductionはcurrent checkpoint manifestを正本とする。

## 127. BGI logical monitor information

Focused `get-monitor-info-a` observationを独立に2回取得した。Primary raw traceは412,244 bytes／
816 records／371 balanced API pairs／SHA-256
`01cfd734a384bf3678263af7312e3354688194e79ef21ee019b4fd527ec64577`、summaryは70,362 bytes／
SHA-256 `39ebd444eb1bc674bf71c7a2a869cf8af42a6f948d8c33be3ccaa46949da0c0b`である。
Confirmation raw traceは370,979 bytes／677 records／303 balanced API pairs／SHA-256
`f13219e019c4eecc5b383d8c2a79093338226a2ae19e00d0828d28fab8ef1a08`、summaryは65,244 bytes／
SHA-256 `f4c0c49198b5b8aceaf282871c29f215611e584a272a23111c0d01f0e5367c13`である。両summaryとも
diagnosticは0、hook completion後1秒のAPI quiescenceまで確認した。Raw trace／summaryは`.work/`に保持し、
Gitへ追加しない。Source executableは前後とも1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`で変更していない。

BGI callはcaller RVA `0x0005C913`、frame caller RVA `0x0009384C`から一回到達する。Live primary
HMONITORと40-byte `MONITORINFO`でBOOL 1、LastError preserve、monitor rect
`(0,0)-(1920,1080)`、work rect `(0,0)-(1920,1032)`、flags `MONITORINFOF_PRIMARY`を返す。
Callerはmonitor left／topをwindow placementへ使い、work rectはこの到達pathでは消費しない。Native
HMONITORとtaskbar／desktop topologyはartifact observationでありVineへimportしない。

Controlled x86 oracleは11 casesを一runで固定した。Live handleのsize 40／72は成功してLastErrorを保存する。
Live handleのsize `0／39／41／71／73`はFALSE／87、NULL／invalid handleはsize 40と0の両方で
FALSE／1461となり、handle validationがsize validationより先であることを示す。全failureで80-byte guard
bufferは不変である。72-byte `MONITORINFOEXA`は同じrect／flagsとdevice `\\.\DISPLAY1`を返すが、
primary callerは40-byte variantだけを要求する。

Runtimeは`RuntimeDisplays`をmonitor identity／monitor rect／contained work rectの単一authorityとし、version-17
User32 policyの`logical-primary-monitor-info-40-v1`だけを実装する。Logical handleを先にtyped lookupし、
invalid handleは1461、live handleのnon-40 sizeは87、40-byte successは全spanをpreflightして一回だけ
`28000000000000000000000080070000380400000000000000000000800700000804000001000000`をpublishする。
Native 72-byte variantはsilent truncationせず`get-monitor-info-a-ex-variant-unsupported`となる。Failureはguest
bytes、display identity、generation、LastErrorを部分更新しない。

Node summaryはsequence 18,439／instruction 11,824,580でcallを完了し、その後の
`GetSystemMetrics(23／3)`をsequence 18,440／18,441で完了する。18,441 supported calls、18,496
writebacks／36,660 dirty pages／1,816,970 changed bytes、17,146 host deltas、generation 36,073を通し、
semantic SHA-256は`cd147b58fb5c0f2ca0d8ffea5a38c47f586b9f63ce7b33ddc6e3eef79e55a98d`、
final memory SHA-256は`f9df61ebd98d5563ffd912b494322b69ce9520c712c3bf6553f52fdd51c46c38`である。
Summary reportは50,161 bytes／SHA-256
`ac57d5a0619b1688499be4bac30cacab1b261c38b4d092c3b78d7622c29e7790`である。Chrome Runtime Workerも
same ordered semantics／digest／memory／stopを再現し、19,191 slices、19,189 writebacks／38,272 pages、
17,146 mapping deltas／18,346 dirty pages、generation 27→36,768、55,138 Wasm crossings、57,539
execution transitions、terminal pending work 0を通した。Browser reportは43,930 bytes／SHA-256
`42f42b4e1eb57413b8fa5c61faad1e11a0002056130a0d772fe12623cfbd6572`である。

同じ180-second／17-threshold gateの先行二runはsemantic failure 0、16／17 passで、Heartbeat P99だけが
33.480／33.865 msとなった。旧performance-frequency stopとのsame-host Node三回比較は新path -0.747%で、
追加API costはshiftを説明しなかった。Thresholdを変更せず、retryをacceptanceに使わず、exact reports／A/B／
resume conditionをstructured feasibility blockerへ保存した。その後、明示的にquiescentなhostで同じfixture、
pinned Chrome、runtime artifact、17 thresholdsを変更せず127 sessions／semantic failure 0／17-of-17 passを得て
blockerを解除した。Accepted reportは2,033,566 bytes／SHA-256
`151750896f8b7e874acb67d3d30334e6b812fb4eb64efd28ebce39d445078b0d`である。これによりinstruction
11,825,212の`dinput8.dll!DirectInput8Create`へ進んだ。この時点ではDirectInput identity／device behaviorを
実装済みとしなかった。

## 128. BGI DirectInput 8 root factory and identity

Focused `dinput8-root` observationを独立に2回取得した。Primary raw traceは234,028 bytes／698 records／
SHA-256 `3fc837713e87b990da6576e19e9bd13e965581e2ab971437fab995a1e9fffe84`、confirmationは
232,973 bytes／695 records／SHA-256
`5216fa3642e2324fec4412579b9847a244585c3a4301c321a3a860e1ddac6c5c`である。両runともprofile
`x86-bgi-direct-input-8-root-v1`でfactoryと直後のfirst methodを一回ずつ捕捉した。Raw traceは`.work/`に
保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のまま変更していない。

Factoryはcaller RVA `0x0005C234`からmain HINSTANCE、version `0x0800`、
IID bytes `30 80 79 BF 3A 48 A2 4D AA 99 5D 64 ED 36 97 00` (`IID_IDirectInput8A`)、
non-NULL output、NULL outerで呼ばれる。NativeはS_OK／non-NULL rootを返し、LastErrorはentry 0から122へ
変わる。CallerはHRESULTのsignを検査してhelperへ進むため、opaque pointer値やLastError 122はpolicyへ
importしない。最初のroot callはcaller RVA `0x0005C1CE`のslot 4 `EnumDevices(4,0x1005BEF0,0,1)`で、
S_OK／LastError 0を返す。

Controlled x86／COFF-i386 oracle sourceは6,196 bytes／SHA-256
`718ebcf0e4fda1985c0ed77b5f4e3f3c8aa4bcbe40ba86743fb1cd8094587595`、executableは5,120 bytes／
SHA-256 `f1b98b895b09193b186e52377e8c7dceb91995a61c6998fc4fa8e59a2d06d5c9`である。Version `0x0800`の
`IID_IDirectInput8A`／`IID_IUnknown`はS_OK、unknown IIDは`E_NOINTERFACE`、`0x0700`は
`0x80070481`、`0x0900`は`0x8007047E`となり、failure outputはNULLである。Native non-NULL aggregationは
S_OK／NULL outputという矛盾した形を観測したため、Vineはsilent successを採らず
`CLASS_E_NOAGGREGATION`をmutation前に返す。Primary outerはNULLであり、このbounded policy差は到達pathを
変えない。Vine successはcaller LastErrorを保存する。

Runtimeはfive-word stdcall／HRESULT catalogを`dinput8.rs`に置き、`RuntimeCom`をroot object、canonical
IUnknown、`IDirectInput8A` interface、refcount、8 KiB backing、11-slot vtableの単一authorityとする。
`ExecutionSession`はCOM installとguest outputを全span／capacity preflight後に一transactionでcommitし、
failure時はguest bytes、COM allocator、identity、mapping、generationを部分更新しない。IUnknown three slotsは
shared implementationを使い、残るslotsはexact argument widthを持つtyped unsupported endpointである。
BrowserはCOM state、input state、device identityを所有しない。

Nodeはfactoryをsequence 18,442／instruction 11,825,212で完了し、output `0x01060004`、object ID 9、
interface ID 18、refcount 1、backing `0x01060000`／8,192 bytesをpublishした。18,442 supported calls、
18,497 writebacks／36,661 dirty pages／1,816,996 changed bytes、17,147 host deltas、generation 36,100、
semantic SHA-256 `a7b449fc05fa1740d568fe199913239d9544d55bb1b644138b852fe45866da08`、final memory
SHA-256 `442d3f326e8f247cdb82e9cf7efd625ef4d7732e90c04827bd69a860d620e6c1`を固定した。Summary reportは
53,845 bytes／SHA-256 `298349d459d4d4cd2d92ad807fc6cc261c3719b49d1ddc86273b7abf364e5db3`である。
Chrome Runtime Workerも同じdigest／memory／stopを再現し、19,192 slices、19,190 writebacks／38,273 pages、
17,147 mapping deltas／18,349 dirty pages、generation 27→36,795、55,142 Wasm crossings、57,542 execution
transitions、clean cancel／watchdogを通した。Browser reportは44,578 bytes／SHA-256
`518d55eb7cd2c77c9a61d71074bdc01dc61e0cafeabbd9bcfcb556bb27c77519`である。

同じ180-second／17-threshold gateはvalid environmentで128 sessions、semantic failure 0、session P95
1,445.050 ms、slice P99 2.170 ms、heartbeat P99 6.050 ms、17-of-17 passを得た。Reportは2,037,763
bytes／SHA-256 `a0c09c440c6cb66fab15cf4340a4588a73dabdef8d22d16c5aa054dfde065b0a`である。次stopはinstruction
11,825,229、endpoint `0x01061070`、return `0x1005C1CE`の
`IDirectInput8A::EnumDevices(this=0x01060004,4,0x1005BEF0,0,1)`、reason `com-method-unsupported`である。
Device enumeration／callback／device create／acquire／state、DOM inputはまだ実装済みとしない。

## 129. BGI DirectInput 8 empty game-controller enumeration

Focused `dinput8-enum-devices` observationを独立に2回取得した。Primary raw traceは263,408 bytes／700 records／
SHA-256 `f4a04103c290ccd4f3ec8e19150d9480be60214f1904cc4ba082bb47784d29b1`、confirmationは
261,749 bytes／697 records／SHA-256
`2c859ebf886caede34331c0ddfedd4c7591e9b51ad873038e810d88f765a037d`である。両runともprofile
`x86-bgi-direct-input-8-enum-devices-v1`で、factory caller RVA `0x0005C234`に続くslot 4 callをcaller RVA
`0x0005C1CE`から一回捕捉した。Argumentsはdevice class 4 (`DI8DEVCLASS_GAMECTRL`)、callback
`0x1005BEF0`、reference 0、flags 1 (`DIEDFL_ATTACHEDONLY`)で、S_OK／LastError 0、callback count 0である。
Caller callbackはdeviceがあればentryをallocate／linkするが、primary pathは空listをそのまま走査して成功する。
Raw traceは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／primary SHA-256を保持した。

Controlled COFF-i386 oracle sourceは10,528 bytes／SHA-256
`2a255635c7f5f5b27cae442cead6eb050f4f63e4306237ad13b1001b2aa4d908`、executableは10,752 bytes／
SHA-256 `ef3eabd2738fb4723b3c96f78798e52c73e556a9ec73c8e12dc7953d133c46da`で、二回の出力は一致した。
Primary class 4／flags 1はS_OK／callback 0、keyboard class 3、pointer class 2、all class 0、device class 1は
各callback 1だった。Reached callbacksはcaller referenceを転送し、最初のinstance sizeは580 bytesである。NULL
callback、class 99、unknown high flagは`0x80070057`／callback 0となり、sentinel LastErrorを保存した。

Runtimeは`vine.dinput8-compatibility-policy` version 1の
`empty-attached-game-controller-enumeration-v1`だけを実装する。Exact primary callはsequence 18,443／instruction
11,825,229でS_OKを返し、guest write／callback／device publication／state mutationは0、root refcountは1のままである。
Invalid argumentは`E_INVALIDARG`、valid unreached class／flagはtyped structured unsupported、callback-producing device
countを含むpolicyはdispatch前に拒否する。BrowserはCOM／input／device stateを所有せず、DOM device catalogを
importしない。

Nodeは続くheap pathを実行して18,452 supported calls、18,507 writebacks／36,964 dirty pages／2,931,700 changed
bytes、17,156 host deltas、generation 36,120、semantic SHA-256
`c4408e6ddb87129787656cb709d07081b59edfb7517aa15791b003d0a065214e`、final memory SHA-256
`07630b9b0a45bb1b97d2159e51bb9c635596ce8ff4067dc70370a7864ea78e23`を固定した。Summary reportは50,056
bytes／SHA-256 `4a9a079d1b7b8728d4cba82e649e0fd48b9476390765a0dec7e6306028bafff3`である。Chrome Runtime Workerも
same digest／memory／stopを再現し、19,207 slices、19,205 writebacks／38,582 pages、17,156 mapping deltas／
18,637 dirty pages、generation 27→36,820、55,176 Wasm crossings、57,587 execution transitions、clean
cancel／watchdogを通した。Browser reportは43,892 bytes／SHA-256
`12639faa2c11a5f45c476425d498581f38c4013001ca2e41565bce91d11a1c9f`である。

Versioned 180-second gateは127 sessions、semantic failure 0、session P95 1,458.825 ms、slice P99 2.125 ms、
heartbeat P99 6.060 ms、valid environment、17-of-17 passを得た。新たな1 MiB heap pathのsingle-run observation
18,219,008 bytesを根拠に、このversionだけexecution-engine ceilingを24 MiBとし、残る16 thresholdsは維持した。
Reportは1,955,904 bytes／SHA-256
`b9bbd3d6ba8a8e5dbde7876f4913fb361f58e9972ab1d4077d20ddadbbffd507`である。次stopはinstruction
11,921,739、endpoint ID 330／`0x7F001490`、IAT `0x100D1058`、return `0x1009CCCC`の`DSOUND.dll` ordinal 11。
Local SysWOW64 export tableはこれを`DirectSoundCreate8`へ解決する。ArgumentsはNULL device GUID、output
`0x100FEB10`、NULL outer、reasonは`endpoint-contract-missing`である。DirectSound object／buffer／browser audio
transport、callback-producing DirectInput variants、device create／acquire／stateはまだ実装済みとしない。

## 121. BGI shared multimedia tick sample

Focused `time-get-time` observationを独立に2回取得した。Primary raw traceは304,054 bytes／
854 records／390 balanced API pairs／SHA-256
`8560ea8389627cfc2c85b4a98f30f835ab53ff3ec13038ac55b25ad736790a29`、summaryは51,478 bytes／
SHA-256 `5fa32c69a605e1cd173eed042522669f253534a1d993d943ee239b12d16fc777`である。Confirmation
raw traceは303,051 bytes／851 records／388 balanced pairs／SHA-256
`8f3a8847793282b4eadb4f4e45c6a142610b9f3ea1f3e4437e0b8349b314cc5e`、summaryは51,652 bytes／
SHA-256 `83d7a4ca65a6b109379b796838cc1d97718083a28657bc70d395d636ef9604c3`で、両summaryとも
diagnosticsは0である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

`x86-time-get-time-shared-tick-epoch-v1` oracleはLastError sentinel `0／87／0x12345678`ごとに
`timeGetTime`を一回呼ぶ。Hook resultと`SystemFunction` resultは各callで一致し、LastErrorを保存し、
直前／直後の`GetTickCount` sampleによるunsigned wrapping bracket内にある。Concrete native tick値は
artifact identityでありVine policyへimportしない。BGIはcaller RVA `0x00091EBF`から一回呼び、LastError
0のまま戻る。RVA `0x00083144`をrootとするconsumerはEAXをunsigned wrapping elapsed-time operandとして
使う`wrap-aware-eax-delta-v1` flowであり、runtime handlerはcaller／hash／occurrenceを分岐条件にしない。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeは`winmm.time-get-time.v1`をzero-argument x86 stdcall／U32／LastError preserve／zero guest writeとして
実装した。Version-1 WinMM policyの`shared-process-tick-count-increment-1-v1`はincrement 1だけを選び、
`runtime/process.rs::RuntimeProcessState`がKernel32 `GetTickCount`と共有するtick datumを同じ
mutation-free plan／wrapping commitでsampleする。Sequence 17,858／instruction 11,807,928の結果は
3,601,004、次sampleは3,601,005である。`RuntimeTimers`のresolution-request tableはこのclockを変更せず、
disabled／zero-increment policyはruntime／memory mutation前のtyped faultとなる。Host／browser clock、
scheduler granularity、audio clockはimportしない。

Node summaryは17,858 calls／85 contracts／11,807,974 terminal instructions、17,913 writebacks／
36,066 dirty pages／1,807,627 changed bytes、16,568 host deltas、generation 34,811を通す。Semantic SHA-256は
`8bc885cebc50634d583f8ee8cfc7f22995fa7c177b2f4b94e83cea55695b563b`、final memory SHA-256は
`a540914cb267680fa2b9d6a07ab7d3c02853703073b4969d1fb448bb0291d3b0`、reportは49,985 bytes／
SHA-256 `dff9c0799ff4b6e3d583a52f3d2bbef1db02ebe132ffc8191dcd693f2f556a79`である。

Chrome Runtime Workerもsame ordered completion／instructions／semantic digest／terminal memoryを再現し、
29,089 writebacks／61,972 pages、16,568 mapping deltas、generation 27→45,989、63,598 Wasm crossings、
87,239 success execution transitions、terminal pending work zeroを通す。Browser reportは115,932 bytes／
SHA-256 `41bcd53626661df496cab3a8bf1b1f9bf41eb135ee091561d05c87886f2a6f0a`、Wasm artifactは
3,571,126 bytes／SHA-256 `d3bcdffc0e5a84b55fba2f5ad79163361954db5476a169c620cb2cb23a825843`
である。同じ180-second fixtureと既存17 thresholdsのbenchmarkは63 sessions、semantic failure 0、session
P95 2,952.075 ms、slice P99 2.665 ms、heartbeat P99 5.995 msで17／17を通した。Reportは944,587 bytes／
SHA-256 `cb8af93eb085ac01e74ec0bbf76aba7d019933a5ce4679fd9a79a0ff93de85bc`である。

次stopはinstruction 11,807,974、runtime-system endpoint ID 221、
`USER32.dll!MessageBoxA(0x48010001,0x100DCBA0,0x100DCBE0,0x00001010)`、endpoint
`0x7F000DC0`、IAT `0x100D144C`、return `0x10066B0D`の`endpoint-contract-missing`である。
このcheckpointはelapsed host time、native／browser millisecond精度、sleep／wait、multimedia callback、
frame cadence、audio deliveryを実装済みとしない。

## 120. BGI D3D9 surface DC and logical fill

Focused `d3d9-surface-get-dc` observationを独立に2回取得した。Primary raw traceは259,841 bytes／
519 records／225 balanced API pairs／SHA-256
`3d0010cace0f0e13e3f0c6918b57d3aaac46afb8b2fb399d16ec2197de41acef`、summaryは235,461 bytes／
SHA-256 `0a08225c4a15cd82f15d5401a8247b2b301bad1e787551fcb1f34e34c26b4192`である。Confirmation raw
traceは258,887 bytes／518 records／224 balanced pairs／SHA-256
`8e5109f42a539e04421bf2ac33fa85a98b27a084b52e2b2f2738f1861393c03c`、summaryは235,576 bytes／
SHA-256 `2b8e16261cdc17afc3b8779b94ecae3de9a4265277105aba0dad82277872010c`で、両summaryともdiagnosticsは
0である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceはBGI caller RVA `0x0005A746／0x0005A76C／0x0005A77D／0x0005A78E／0x0005A79A`から
`IDirect3DSurface9::GetDC -> GetStockObject(4) -> FillRect([0,0,800,600]) ->
IDirect3DSurface9::ReleaseDC -> IDirect3DSurface9::Release`をこの順で一回ずつ完了する。GetDC／
ReleaseDCは`S_OK`、FillRectは`1`、surface Releaseは`1`、HDC／BLACK_BRUSHはnon-NULL、全observed
LastErrorは0である。Native HDC／brush／COM pointer値はartifact identityであり、Vine guest identityへimportしない。
Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Controlled COFF-i386 oracleはsource 16,434 bytes／SHA-256
`33d79e003ff050f8e22787629b375d60c1650a331148d7e7ef40ed68654ad685`、executable 8,704 bytes／
SHA-256 `056d6fd36e5cfc14840b63137219e8e3fa36bcfb396104d1d1cba6faf7e6bd00`である。一つのsurfaceで最初の
GetDCは`S_OK`、repeatは`D3DERR_INVALIDCALL`かつoutput preserve、正しいReleaseDC後のreacquireは`S_OK`で、
surface reference countは`2`のままである。Wrong／repeated ReleaseDCは`D3DERR_INVALIDCALL`、正しいreleaseは
`S_OK`となる。Native `LockRect`はlive DC中に`D3DERR_INVALIDCALL`、release後に`S_OK`となる。NULL／read-only
outputはaccess violation `0xC0000005`で、各fixture SHA-256は
`f533018f6f7619ce6d67b27ebee5e0b2f9ca8401f7e4c5abf1a7585e70d68e6f`／
`ad7b34ea2ddd5206d050cd8550b0fb8074f9ceafacbc9819d330590c0759f6eb`である。SuccessはLastErrorを保存する。
Oracle source／binaryは`.work/`に保持しGitへ追加しない。

Runtimeはversion-11 `primary-surface-gdi-dc-v1`を実装した。Sequence 17,852から17,856、instruction
11,807,793／805／812／820／826でsurface `0x01030004`からHDC `0x47010002`／logical DC ID 2を取得し、
shared brush `0x47020001`でlogical graphics surface ID 1へCOLORREF 0のfill commandを一件記録し、HDCと
surfaceをretireする。`RuntimeGdi`はsurface／COM／threadへのnon-owning HDC relationを所有し、
`RuntimeGraphics`は`1024x1024` B8G8R8X8 surface、bounded fill history、content generation 1を所有する。
Guest HDC writeとGDI publicationは一transactionであり、repeat／wrong-owner／capacity／stale／unwritable pathは
guest bytes、identity allocator、GDI／graphics generationをpartial mutationしない。HDCはCOM refを増やさないが、
live中のsurface retirementを拒否する。ReleaseDC後のsurface Releaseはreference count 0となり、logical surfaceも
retireする。Pixel storageとbrowser renderer resourceは生成しない。

Node summaryは17,857 calls／11,807,928 terminal instructions、17,912 writebacks／36,064 dirty pages／
1,807,607 changed bytes、16,568 host deltas、generation 34,810を通す。Semantic SHA-256は
`7ae02cafb6169f8c2fab4ef007a8a6feaa98321d2b8334c37cb7bc7f687ac65a`、final memory SHA-256は
`070d7ec3e62b64ec3076a5fa9cde8311f6937aa022594c8dbae690a626d42135`、reportは49,961 bytes／
SHA-256 `13662b82392008a417a44457e1fd83abb88673c64527cac0a00ce4a791e9f104`である。

Chrome Runtime Workerもsame ordered semantics／instructions／digestを再現し、29,088 writebacks／61,970 pages、
16,568 mapping deltas、generation 27→45,988、63,596 Wasm crossings、87,236 success transitions、terminal
pending work zeroを通す。Elapsedは3,149.935 ms、maximum sliceは2.515 ms、632 heartbeat samplesのmaximum
gapは18.015 msである。Browser reportは115,926 bytes／SHA-256
`b0d86b3f844f2d035798adbada78129d1b5d3538aa1b3a1678637d3457d1e9dd`、Wasm artifactは3,568,357 bytes／
SHA-256 `ada8b39eaf1535d92c2c5a6552b8c5d2fafec3f48d2894196f0a72e46207630b`である。同じ180-second
fixtureは63 sessions、semantic failure 0、17／17 unchanged thresholdsを通した。Benchmark reportは956,407
bytes／SHA-256 `8382db48a785f6d36dce74d6e68fd16d0696efeeb5b157177548ff3b2aefa685`である。

次stopはinstruction 11,807,928、runtime-system endpoint ID 8、`WINMM.dll!timeGetTime`、endpoint
`0x7F000070`、IAT `0x100D14E4`、return `0x10091EBF`の`endpoint-contract-missing`である。Logical fillは
pixel outputではなく、surface storage、pitch、Lock／Unlock、renderer、draw、Present、Reset、device loss、
first visible frameはこのcheckpointで実装済みとしない。

## 118. BGI D3D9 logical texture creation

Section 117のfocused `d3d9-create-device` observationをtexture oracleとして再利用する。Primary raw traceは
2,333,677 bytes／7,075 records／3,498 balanced API pairs／SHA-256
`df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071`、summaryは275,843 bytes／
SHA-256 `b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60`である。Confirmation raw traceは
2,355,307 bytes／7,121 records／3,521 balanced pairs／SHA-256
`2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3`、summaryは275,855 bytes／
SHA-256 `a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b`で、両summaryともdiagnosticsは0である。
Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceにはBGI caller RVA `0x0005A715`からsuccessful `IDirect3DDevice9::CreateTexture`が一callあり、
argumentsはwidth／height `1024／1024`、levels `1`、usage `0`、format `22`、pool `2`、shared handle NULL、
result `S_OK`、observed LastError `0`である。CallerはHRESULT signを消費し、直後に返されたtextureのslot 18
`GetSurfaceLevel(0, output)`を呼ぶ。Focused hookはtexture output snapshotを取得していないため、native pointer値、
vtable bytes、reference count、surface outputをsemantic goldenにしない。Source executableは1,193,472 bytes／
SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Runtimeはversion-9 `primary-system-memory-x8r8g8b8-texture-v1`だけを実装した。Sequence 17,850／instruction
11,807,772でdevice `0x01010004`から呼ばれ、output `0x101517CC`へtexture pointer `0x01020004`を書いて
`S_OK`を返す。`RuntimeCom`はobject ID 4／interface ID 8の22-slot childをreference count 1でpublishし、
device countはretained referenceにより2となる。`RuntimeGraphics`はlogical texture ID 1、device ID 1との
relation、descriptor `1024／1024／1／usage none／B8G8R8X8／system-memory`を所有する。Guest 4 bytes、COM
identity、graphics associationは全owner preflight後にpublishされ、texture final releaseはassociationをretireして
必要ならdevice／root retirementへcascadeする。Texel storageとbrowser renderer resourceは生成しない。

Node summaryは17,850 calls／11,807,783 terminal instructions、17,905 writebacks／36,056 dirty pages／
1,807,483 changed bytes、16,565 host deltas、generation 34,770を通す。Semantic SHA-256は
`bfaf1165757018add907c7777315a9c814b2caacef29b5fe06704be542450e58`、final memory SHA-256は
`6d4f6d179c38596d280df1cc940c137b4e16cf1293eced104b2db60ee36ad944`、reportは56,558 bytes／
SHA-256 `0874e21b79c357eb3544a48171f5056809cf8e2314ac15995538877907dd58c5`である。

Chrome Runtime Workerもsame ordered completion／instructions／semantic digestを再現し、29,081 writebacks／
61,962 pages、16,565 mapping deltas／17,758 dirty pages、generation 27→45,948、63,575 Wasm crossings、terminal
pending work zeroを通す。Browser reportは116,634 bytes／SHA-256
`f52c5b25de5d278171d0321a6042f26903924b2b09e83bba116ccf91aa895aa8`である。最初の180-second benchmarkは
26 semantic sessionsと16／17 thresholdsを通したが、session P95 8,437.465 msが8,000 ms ceilingを超えた。
外部CPU workloadを利用者権限なしに停止せずstructured blockerへ記録した後、利用者が該当workloadを停止した。
同一fixture／duration／thresholdのisolated rerunは45 sessions、semantic failure 0、session P95
4,131.915 ms、17／17 thresholds passとなり、blockerを解消した。Passing reportは946,295 bytes／SHA-256
`3819bd12367772a3b48f83a85ba48b2272b57a7187723a3bb9a683ff437080d4`である。

次stopはinstruction 11,807,783、`IDirect3DTexture9` slot 18／`GetSurfaceLevel`、this `0x01020004`、
arguments `0, 0x006FFD38`、endpoint `0x01021150`、return `0x1005A731`の`com-method-unsupported`である。
Surface identity／storage／pitch／Lock、renderer、draw、Present、Reset、device loss、first visible frameはこの
checkpointで実装済みとしない。

## 119. BGI D3D9 level-zero surface retrieval

Focused `d3d9-get-surface-level` observationを独立に2回取得した。Primary raw traceは228,637 bytes／
514 records／222 balanced API pairs／SHA-256
`54aa56bade26d675f864162bb7a7a2f8b4fc30f8df8dbd0835681cc09f9eced2`、summaryは230,104 bytes／
SHA-256 `c68ef403d864c89ad466ed3bda3cef750065d7555b815fb23f3ad99a43697c8e`である。Confirmation
raw traceは229,203 bytes／514 records／222 balanced pairs／SHA-256
`e3f6d853293cfa8f395695c2de21cb4675cc71f27efa4b3715f2171575ad1422`、summaryは230,131 bytes／
SHA-256 `1ed3b5f247a8b6dd82efd2cdaa7cb0393e68d7db9639d07eaa1afa2244b4a81f`で、両summaryとも
diagnosticsは0である。Raw trace／summaryは`.work/`に保持しGitへ追加しない。

各traceにはBGI caller RVA `0x0005A731`からsuccessful `IDirect3DTexture9::GetSurfaceLevel`が一callあり、
level `0`、result `S_OK`、observed LastError `0`、surface output non-NULLである。CallerはHRESULT signを
消費し、直後にsurface slot 15 `GetDC`をRVA `0x0005A746`から呼ぶ。Native pointer値、vtable bytes、
reference countはartifact identityであり、Vine guest identityへimportしない。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
のままである。

別のcontrolled COFF i386 oracleはsource 13,977 bytes／SHA-256
`1ec0c191bb9783abcbfae7b92ba6d7136018111c4b1da7c860d7dd343daca562`、executable 12,288 bytes／
SHA-256 `7ca28333d06b7352b032b0af9ceb22a016e03d5830f17a74231569ef2b2fee9e`である。Repeat level-zero
取得は同じsurface pointerとIUnknown identityを返し、surface IUnknownはtexture IUnknownと異なる。
Surface `GetContainer(IID_IDirect3DTexture9)`は元textureを返す。Native invalid levelは
`D3DERR_INVALIDCALL`とoutput NULLを書き、NULL outputは`0xC0000005` access violationとなる。Success時は
LastErrorを保存する。Oracle source／binaryは`.work/`に保持しGitへ追加しない。

Runtimeはversion-10 `primary-texture-level-zero-surface-v1`だけを実装した。Sequence 17,851／instruction
11,807,783でtexture `0x01020004`からarguments `0, 0x006FFD38`で呼ばれ、outputへsurface pointer
`0x01030004`を書いて`S_OK`を返す。`RuntimeCom`はobject ID 5／interface ID 10の17-slot childをreference
count 1でpublishし、texture countはretained referenceにより2となる。`RuntimeGraphics`はlogical surface
ID 1、texture ID 1とのrelation、descriptor `level 0／1024x1024／format 22`を所有する。Guest 4 bytes、
COM identity、graphics associationは全owner preflight後にpublishされる。Repeat取得は同じsurface identityへ
AddRefし、final surface releaseはassociationをretireして必要ならtexture／device／root retirementへcascadeする。
Vineはrepository共通のfailure atomicityを優先し、invalid level／NULL outputへ
`D3DERR_INVALIDCALL`を返してoutputを変更しない。Unwritable output、capacity、stale planもguest bytes、
COM count、graphics index、allocator、generationを変更しない。Pixel storage、pitch、DC、lock staging、
browser renderer resourceは生成しない。

Node summaryは17,851 calls／11,807,793 terminal instructions、17,906 writebacks／36,057 dirty pages／
1,807,491 changed bytes、16,566 host deltas、generation 34,802を通す。Semantic SHA-256は
`7c43c22e49cd62568797ded4382ceacba5659be3c42d4a3460186702247c08bf`、final memory SHA-256は
`d4c3bed5fe4ab0ebd2bacc0c32023a679cc1c6d55f7c0701cb981eeec368f3e2`、reportは55,216 bytes／
SHA-256 `31fac5d5f3884fc6caa88e4c4073e791da1b132052fb3b1255b348999f620045`である。

Chrome Runtime Workerもsame ordered completion／instructions／semantic digestを再現し、29,084 slices、
29,082 writebacks／61,963 pages、16,566 mapping deltas／17,761 dirty pages、generation 27→45,980、
63,579 Wasm crossings、87,218 success execution transitions、terminal pending work zeroを通す。Elapsedは
4,426.825 ms、maximum sliceは2.845 ms、888 heartbeat samplesのmaximum gapは24.055 msである。
Browser reportは116,622 bytes／SHA-256
`c51183ef250d39ab79929a39d58965c54b33e6bf93b530708899bd459c696fb7`である。Wasm artifactは
3,527,856 bytes／SHA-256 `2498908a70fac8a2404f39346791186803906f79c482f995c8711fa1f10feb12`である。

同じ180-second fixtureと既存17 thresholdsのbenchmarkは46 sessions、semantic failure 0、session P95
4,037.135 ms、slice P99 2.535 ms、heartbeat P99／maximum 6.005／34.145 msで17／17を通した。
Reportは947,266 bytes／SHA-256
`d563064d738dcf927c167d2d4808420c12ec8d55253c4a658c7a6187f592021b`であり、threshold変更やretry acceptanceは
行っていない。

次stopはinstruction 11,807,793、`IDirect3DSurface9` slot 15／`GetDC`、this `0x01030004`、argument
`0x006FFD3C`、endpoint `0x01031120`、return `0x1005A746`の`com-method-unsupported`である。Logical
surface identityは実装済みだが、HDC identity／lifetime、pixel storage、pitch、Lock、renderer、draw、Present、
Reset、device loss、first visible frameはこのcheckpointで実装済みとしない。

## 130. BGI DirectSound initial buffers

Focused `dsound-create-sound-buffer` observationを独立に2回取得した。Primary raw traceは382,086 bytes／
801 records／SHA-256 `6d924745fb6b61efa7992ec4ccda6cd553bab625348b0734876216e12af37f33`、
confirmationは382,124 bytes／801 records／SHA-256
`adb5fc01b96004ca5b571fa7dbfff5008f79c02d6ae34d37d5068fc830be669c`で、diagnosticsは0、
address以外のsemanticsは一致した。Raw traceは`.work/`に保持しGitへ追加しない。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
のままである。

各traceはBGI caller RVA `0x0009C916／0x0009C988／0x0009C99B／0x0009CA37／0x0009CA82／
0x0009CA8D／0x0009CAB2`から、primary `CreateSoundBuffer`、primary `SetFormat`、looping `Play`、secondary
`CreateSoundBuffer`、`QueryInterface(IID_IDirectSoundBuffer8)`、`Release`、`Lock`をこの順で一回ずつ
完了する。Primary descriptorは36 bytes／flags 1／buffer bytes 0／format NULL、formatはPCM／stereo／
44,100 samples/s／176,400 average bytes/s／block align 4／16 bits、play flagsは1である。Secondaryはflags
`0x000080A8`／88,200 bytes／同じPCM formatである。QueryInterfaceはsame pointer、Releaseは1を返す。
Lockはoffset 0／88,200 bytes／flags 0で、first segmentはnon-NULL／88,200 bytes、second segmentはNULL／0、
resultはS_OKである。Native pointerとLastErrorはartifact identityでありVine semanticsへimportしない。

Controlled COFF-i386 oracleはsource 11,546 bytes／SHA-256
`2a4a5a2607d48f62ac56b317164a55bf6db4e74d871f537353d926c54dd7d462`、executable 6,656 bytes／
SHA-256 `52b332822134f18dcfd8e0ecb5329187eccddcf96082a9c383311370a69b71d2`である。NULL descriptor／
output、bad size、invalid primary bytes／format、zero secondary bytesは`E_INVALIDARG`かつoutput preserve、
aggregationは`CLASS_E_NOAGGREGATION`かつoutput preserveとなる。Primary SetFormat／looping PlayはS_OK、
non-looping Playは`E_INVALIDARG`、secondary SetFormatは`DSERR_INVALIDCALL`、Buffer8 QueryInterfaceは
same pointerである。

Runtimeはprimary／secondary descriptor、PCM format、looping playback state、COM child identityを
Rust-owned stateとして実装した。Nodeは18,460 supported calls、18,515 writebacks／36,973 dirty pages／
2,931,873 changed bytes、17,160 host deltas、generation 36,232、semantic SHA-256
`d20eb73d871ff9952d17ab436413846bc1af47134496c398a61d5f700b27e2fd`を通した。Chrome Workerも
same semantics／terminal memoryを再現し、19,215 slices、19,213 writebacks／38,591 pages、17,160 mapping
deltas／18,647 dirty pages、generation 27→36,932、55,204 Wasm crossings、57,611 success transitions、clean
cancel／watchdogを通した。次stopはinstruction 11,921,896、`IDirectSoundBuffer8::Lock` slot 11、this
`0x01090004`、endpoint `0x010910E0`、return `0x1009CAB2`の`com-method-unsupported`である。Sample backing、
Lock／Unlock content、cursor、secondary playback、browser audio transport、audible primary outputは未実装である。

## 131. BGI DirectSound secondary buffer transfer

Focused `dsound-buffer-lock-unlock-play` observationを独立に2回取得した。Primary raw traceは425,777 bytes／
847 records／SHA-256 `c0993be13360eb7515c6ceac90b228b58943c818d5973d53131929c51fe6d72d`、
confirmationは424,704 bytes／848 records／SHA-256
`76fab2a79d5ca148209f6ce9df060650f67d9f6ab7f79de88d6ea6d3eb60dbda`で、diagnosticsは0、
address以外のsemanticsは一致した。Raw traceは`.work/`に保持しGitへ追加しない。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
のままである。

BGI callerはRVA `0x0009CAB2`からsecondary `Lock(0, 88200, outputs, 0)`を呼び、first segmentはnon-NULL／
88,200 bytes、second segmentはNULL／0、resultはS_OKとなる。Callerは公開された全rangeをzero-fillし、
RVA `0x0009CAF3`からexact segmentで`Unlock`してS_OKを得る。88,200 bytesはすべてzeroで、FNV-1a 32-bitは
`0xCB8D2B65`である。直後にRVA `0x0009CB06`から`Play(0, 0, DSBPLAY_LOOPING)`を呼びS_OKを得る。
Native sample pointerはartifact identityであり、Vineのguest addressへimportしない。

Controlled COFF-i386 oracleはsource 11,532 bytes／SHA-256
`84596e1660f8c8e8701e5b94d4d9ab8683688ae81ced4de987d3982fdbcaf287`、executable 6,656 bytes／
SHA-256 `1615936f0f42ec6352c0c54a9a0373da0775b7837312eac3a0479678a0717d19`である。Valid full Lockは
S_OK／single full segment、double Lockは`E_INVALIDARG`となる。NULL first-data outputは`E_INVALIDARG`かつ
outputs preserve、NULL first-byte-count outputは`E_INVALIDARG`だがnativeはfirst-data outputだけをzero化する。
Unlock-before-LockのNULL segmentsとactive Lockに一致しないUnlockはnativeでS_OKとなり、後者はlockを消費して
続くexact／repeat Unlockを`E_INVALIDARG`にする。これらのpermissive segment acceptanceとpartial output mutationは
primary callerが消費せず、Vineのcross-owner failure atomicityを破るためbounded compatibility policyへ採用しない。
Vineはreached full Lock／exact Unlockだけを実装し、variant外をobservable mutation前にstructured faultへ分類する。

Runtimeはaudio-resource arenaからsample backingをlazy allocationし、Lock publication、caller write、Unlock時の
read-only protection、content generation、looping playback、final COM release時のallocation／memory retirementを
Rust authorityで一貫させる。NodeとChromeは同じordered calls、terminal guest bytes、mapping operations、clean
cancel／watchdogを通し、TypeScript側にDirectSound stateやbrowser audio resourceを作らない。Current report identityと
変動するaggregate値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。次stopはinstruction
11,945,256、`USER32.dll!SetTimer`、argumentsはlive HWND `0x48010001`、timer ID 1、20 ms、guest procedure
`0x10099600`で、`api-endpoint-contract-missing`である。Timer registration、callback delivery、browser clock、audible
outputはこのcheckpointで実装済みとしない。

## 132. BGI logical window timer registration

Focused `set-timer` observationを独立に2回取得した。Primary raw traceは1,610,985 bytes／5,007 records／
SHA-256 `d848db2ee4c8b9a356baa1f77325614ca8047c8d8e84738a81f3cc4d60230636`、confirmationは
1,611,913 bytes／5,010 records／SHA-256
`4fa263adc08521a7f82df1b8e586843a8eef4e338cfe5f9d2bdc00f906c1f1ab`で、diagnostics／hook errorsは0である。
Raw traceは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

BGI callerはRVA `0x00099D4B`へ戻るcallで、live top-level HWND、timer ID 1、20 ms、BGI RVA
`0x00099600`のTIMERPROCを渡す。`SetTimer`は1を返し、LastError 0を保存する。Caller wrapperは直後にEAXをzero化して
戻るためreturn valueを消費しない。各bounded traceは同じwindow／owner threadへ16回の
`TIMERPROC(HWND, WM_TIMER=0x0113, 1, GetTickCount-like DWORD)` entryを確認した。Callback timestampとnative HWNDは
artifact identityでありVineのlogical deadline／handleへimportしない。Observation期間内に`KillTimer`は到達しない。

Controlled COFF-i386 oracleはsource 11,122 bytes／SHA-256
`27dab91a5f2d579a4e491f628aae78f420a9920d700a2dd06f0596a7e3bdd894`、executable 8,192 bytes／SHA-256
`896cb0ea2adc8f49984a2e8e6faced2977bbec5439ed3c96970f993e85c4fa2f`である。Live HWND／ID 1／20 msは1を返して
LastError sentinel `0x5A5A`を保存する。同じkeyの再登録はIDを保持し、old callbackを止め、new callbackを同じowner
threadへ配送する。First `KillTimer`はTRUE、repeatはFALSEで、どちらもLastErrorを保存する。Invalid HWNDは0／
ERROR_INVALID_WINDOW_HANDLE 1400となる。NULL HWNDではrequested ID 0／`0x1234`のどちらもsystem-assigned IDを返し、
live HWND／ID 0もallocated IDを返す。これらのallocation／kill variantsはoracleとして保持するが、primaryのexact
registration sliceへは一般化しない。

Runtimeは`RuntimeTimers`にinternal ID、logical HWND／timer ID key、owner TID、requested／effective interval、
TIMERPROC、registration generationを所有させる。`RuntimeUser32`のlive-window／same-owner relationとexecutable guest
addressを全てpreflightしてからopaque timer planをcommitする。同じkeyのreplacementはinternal identityを保持し、
capacity 4,096、wrong owner、invalid callback、stale／duplicate planではtimer map、key index、allocator、generationを
変更しない。Process exitはtimer-resolution request、waitable timer payload、logical window timerを一つのtimer-owner
teardownでretireする。

Nodeは18,597 supported calls、18,652 writebacks／37,551 dirty pages／2,945,186 changed bytes、17,288 host
deltas、generation 36,502、semantic SHA-256
`df0edbfbc34388c1837bc93223787f5ba8faabac7c4c30b015ff3b1d2d8121ee`、terminal memory SHA-256
`4e54af4735dfaab564f6eed4779693851ab8ca3bd8f234ea97a1f771906b397c`を通した。Node reportは50,127 bytes／
SHA-256 `a1f767a7c74a0cdcbaba8ba881d15b3f33ed10119334ea41afe986c8f99bd4f8`である。Chrome Runtime Workerも
same ordered semantics／terminal memoryを再現し、19,352 slices、19,350 writebacks／39,169 pages、17,288 mapping
deltas／18,800 dirty pages、generation 27→37,202、55,608 Wasm crossings、58,022 success transitions、clean
cancel／watchdogを通した。Browser reportは43,910 bytes／SHA-256
`e90b9db5a030160d4c845d258e216dc7c9e1857951638de3f099a40bd3d35aae`である。

次stopはinstruction 11,945,954、`KERNEL32.dll!CreateThread` endpoint ID 50／address `0x7F000310`／IAT
`0x100D1170`／return `0x100A1FAD`の`api-endpoint-contract-missing`である。Captured stack wordsは
`0, 0, 0x100A1EE8, 0x20139940, 4, 0x20139940, 1`で、API contract確定前の最後のwordはtrailing observationとして
扱う。Logical clock expiry、`WM_TIMER`／TIMERPROC delivery、`KillTimer`、NULL-window／allocated-ID variant、browser
task timerはこのcheckpointで実装済みとしない。

## 133. BGI guest thread creation and resume

Focused `create-thread` observationを独立に2回取得した。Primary raw traceは1,780,985 bytes／886 records／
SHA-256 `326bdfe6341df2596ae0ce888cc051fdd957805d30a2a477295c1d3fe3339261`、confirmationは
1,780,897 bytes／889 records／SHA-256
`dc2f8fa5f4138ef04640a92a1cc9134d8b3fc0bba4e88939e8a76550ef393792`で、diagnostics／hook errorsは0である。
Raw traceは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

BGI callerはRVA `0x000A1FAD`へ戻るcallで、NULL security attributes、stack size 0、start RVA
`0x000A1EE8`、parameter `0x20139940`、`CREATE_SUSPENDED`、thread-ID output `0x20139940`を渡す。
`CreateThread`はnon-NULL handleを返し、同じoutputへnonzero native TIDを書き、LastError 0を保存する。
Callerはhandleを保存してnonzeroを検査し、RVA `0x000A1FBD`へ戻る`ResumeThread(handle)`を直ちに呼ぶ。
Resume resultは1、LastErrorは0で、callerは`0xFFFFFFFF`だけをfailureとして検査する。Worker start routineは
同じparameter pointerを受け取り、そのpointed bytesからcreated TIDとhandleのrelationを観測する。Native handle／
TIDと実行開始時刻はartifact identityであり、Vineのlogical identity／scheduler orderingへimportしない。

Controlled COFF-i386 oracleはsource 8,552 bytes／SHA-256
`8a983c11d492872926a4197462e524e4f0b154832655e5ed15a4f9ba430647e7`、executable 8,192 bytes／SHA-256
`1172ed1294e8d39327540052722f81fd24a2868dcadad0ff1056ec52dde2b014`である。Suspended creationはresume前に
worker effectを生まず、`ResumeThread`は1を返してworkerを開始する。Flags 0のcreationもworkerを開始する。
両success pathはLastError sentinel `0x5A5A`を保存する。Oversized stack requestはNULL／
ERROR_INVALID_PARAMETER 87となりoutputを保存し、invalid handleの`ResumeThread`は`0xFFFFFFFF`／
ERROR_INVALID_HANDLE 6となる。

Runtimeはmonotonic logical TID、thread object／handle、one-MiB stack reserve／one-page commit／guard、one-page
TEB、process PEB relation、entry／parameter／ESP／FS bootstrap、scheduler suspended lane、optional outputを一つの
cross-owner transactionでpublishする。`ResumeThread`はRust-owned suspend countを1から0へ進める。Workerはcompletion
snapshotからopaque v86 CPU contextを登録するだけでWindows thread stateを所有しない。Synchronous resume後は
callerを即時preemptせず、blockまたはexplicit scheduler switchまでchild contextをreadyに保持する。Dynamic entry
returnはthreadをsignalし、stack／TEB mappingとaddress allocationをgeneric release deltaでretireする。

Nodeは18,599 supported calls、18,654 writebacks／37,555 dirty pages／2,945,206 changed bytes、17,289 host
deltas、generation 36,513、semantic SHA-256
`bfc8c1e4811d3587474e2bbad777d89ab182a5417bd963cc1229b86d7b590fdc`、terminal memory SHA-256
`ae690b5dc75ce9ae4c63dd9a0dc169bb88121dd12838c9227b10f74c96f0cd79`を通した。Node reportは55,093 bytes／
SHA-256 `d12d10919c4016bb2645bc92d85fd51d1459c3d94f0e690d2ab3dc45b2e02d6c`である。Chrome Runtime Workerもsame
ordered semantics／terminal memoryを再現し、19,354 slices、19,352 writebacks／39,173 pages、17,289 mapping
deltas／18,803 dirty pages、generation 27→37,213、55,613 Wasm crossings、58,028 success transitions、clean
cancel／watchdogを通した。Browser reportは43,943 bytes／SHA-256
`86388766ffd07511b95dd31ef0227041329eef2edb84a43e963ce5db90c8f66a`である。

次stopはinstruction 11,945,977、`KERNEL32.dll!SetThreadPriority` endpoint ID 139／address `0x7F0008A0`／IAT
`0x100D12D4`／return `0x10091B77`の`api-endpoint-contract-missing`である。Argumentsはcreated logical handle
`0x54010002`とpriority `1`である。Thread priority、DLL thread attach／detach、TLS／FLS cleanup callbacks、natural
last-thread process exit、simultaneous multi-core guest executionはこのcheckpointで実装済みとしない。

## 134. BGI logical thread priority

Focused `set-thread-priority` observationを独立に2回取得した。Primary raw traceは2,773,117 bytes／953 records／
SHA-256 `dd98137e23b5c9cdf34dff09eb14e39b15d653086c0334a557768f5008e7caac`、confirmationは
2,774,186 bytes／953 records／SHA-256
`9370aec65046b6269eb27a77c37c40ae31c9dd9e5b34aaeeed63a8210d87cd2f`で、diagnostics／hook errorsは0である。
Raw traceは`.work/`に保持しGitへ追加しない。Source executableのhashは変更していない。

各traceには`SetThreadPriority`が31 callあるが、reached targetは1 callだけである。BGI callerはRVA
`0x00091B77`へ戻るcallで、直前の`CreateThread`が返した同じhandleと
`THREAD_PRIORITY_ABOVE_NORMAL` (`1`)を渡す。ResultはTRUE、LastErrorは0のままである。CallerはEAXを読まず、
SEH／frameを復元してreturnする。他の30 callはpriority 0の別thread initializationであり、このtarget profileへ
一般化しない。Native handle値やhost scheduling effectはartifact identityで、Vineへimportしない。

Controlled COFF-i386 oracleはsource 7,253 bytes／SHA-256
`5a79901aa8a5c77d17d9ba9d916c418dc204c52504118118225e3eddbfffa79c`、executable 8,704 bytes／SHA-256
`a181fe64363e2970393123a2389719fcaee689db89dc3c3132662fd8aa08f76f`である。New threadのinitial priorityは
normal (`0`)で、above-normalへの変更と同値repeat、normalへの復元はいずれもTRUEかつLastError sentinel
`0x5A5A`を保存する。Unknown numeric value `3`はFALSE／ERROR_INVALID_PARAMETER 87でpriorityを保存し、invalid
handleとclosed handleはFALSE／ERROR_INVALID_HANDLE 6となる。

RuntimeはWin32 priority enumをlogical valueへ変換し、typed live thread handleからTID 8193を解決する。
`RuntimeThreads`がnormal default、optional stored value、mutation-free plan、complete-record stale check、targeted
commitを所有する。Active schema-v2 kernel32 policyはpriority `1`だけを許可する。Unknown numeric value、
policy-disabled known value、invalid／closed／wrong-kind handleはthread record、LastError以外のstate、guest memoryを
部分更新しない。Successはguest write 0でLastErrorを保存する。Cooperative lane selectionは変えず、host scheduler
classとbrowser task priorityも取り込まない。

Nodeは18,607 supported calls、18,662 writebacks／37,576 dirty pages／2,946,750 changed bytes、17,295 host
deltas、generation 36,527、semantic SHA-256
`0348d8d9dd42b0999d8536fa85a897e3da5348768d218ecf0932e1c74f6a9199`、terminal memory SHA-256
`d9013c8f67bf8522affc7b11041d969a9dadf432e92b1bdf5eaea01a5aede2fa`を通した。Node reportは55,874 bytes／
SHA-256 `46ac2be20bce1059ddb57b5892f8607c11087a8c9ac0219abea97de0e041ab6d`である。Chrome Runtime Workerもsame
ordered semantics／terminal memoryを再現し、19,362 slices、19,360 writebacks／39,194 pages、17,295 mapping
deltas／18,810 dirty pages、generation 27→37,227、55,635 Wasm crossings、58,052 success transitions、clean
cancel／watchdogを通した。Browser reportは43,905 bytes／SHA-256
`17583f364d2e89bc8137314c8f5e6d012dfc71fb6b6fd6770f551b9f0c96620a`である。

次stopはinstruction 11,957,736、`USER32.dll!GetDC` endpoint ID 239／address `0x7F000EE0`／IAT
`0x100D1494`／return `0x1002C155`で、argumentはNULL HWNDである。既存window-only `GetDC` contractがNULLを
`null-logical-handle-forbidden`としてmutation前に止めるため、次checkpointはscreen DCをGDI subsystem gateway
として分類する。Priority-weighted scheduling、pseudo-thread-handle variant、priority boost、DLL thread
attach／detach、TLS／FLS cleanup callbacks、natural last-thread process exitはこのcheckpointでclaimしない。

## 135. BGI primary screen DC acquisition

Focused `screen-dc` observationを独立に2回取得した。Primary raw traceは46,976 bytes／91 records／SHA-256
`874c9744b015213963164d37073bc9edbfed25301501febcbccf0f3c4409ff80`、confirmationは46,849 bytes／91
records／SHA-256 `b06ec143734cb4d93fb5df3f5fe9849ddf95a4e804ba1a23e8648181ab32ee59`で、diagnostics／hook
errorsは0である。Profileは`x86-bgi-screen-dc-font-enumeration-lifetime-v1`、raw traceは`.work/`に保持し
Gitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

両traceでBGIはRVA `0x0002C155`へ戻る`GetDC(NULL)`からnon-NULL HDCを取得し、LastError 0を保存する。
直後のRVA `0x0002C16A`へ戻る`EnumFontFamiliesExA`は同じHDC、60-byte `LOGFONTA` pointer、callback RVA
`0x0002D2E0`、LPARAM RVA `0x000DCE7C`、flags 0を受け、0を返してLastError 0を保存する。Stable request bytesは
charset `0x80`とWindows-932 face `ＭＳ ゴシック`であり、未初期化stack由来の他fieldをpolicyへimportしない。
Callerはenum result 0を`neg`／`sbb`／`lea`でBOOL TRUEへ写像する。その後RVA `0x0002C177`へ戻る
`ReleaseDC(NULL, same HDC)`が1を返し、LastError 0を保存する。Native numeric HDCとstack addressはrunごとに
異なるartifact identityである。

同じtrace内の`x86-screen-dc-lifetime-v1` oracleは異なるsentinelを設定して
`GetDC(NULL)`を二回実行し、両方non-NULLかつdistinctであることを確認する。First releaseは1、同じHDCのrepeat
releaseは0、second releaseは1で、全五callが各LastError sentinelを保存しhook／SystemFunction結果が一致した。
Vineはこれをmonotonic logical acquisition identityとsame-thread exact release relationとして実装し、screen HDCに
window surface、pixel storage、browser renderer resourceを割り当てない。Unknown／repeated／wrong-target／
wrong-thread releaseは他HDCとallocatorを変更せずFALSEでLastErrorを保存する。

Nodeは18,608 supported calls、18,663 writebacks／37,577 dirty pages／2,946,766 changed bytes、17,295 host
deltas、generation 36,528、semantic SHA-256
`f7594f50a471b084263cdd5156fe7754b27c204fc9ac6d6b4258c199679ff712`、terminal memory SHA-256
`9a0fd6e08b3ac1e3c2a8cdc8343b32528ca672c01d341aa2a0df96be5020b632`を通した。Node reportは57,126
bytes／SHA-256 `ae916c7d12a432eb9291afa079c9bfff4d32f2643664833b5b41a10fa2fbe86d`である。Chrome Runtime
Workerもsame ordered semantics／terminal memoryを再現し、19,363 slices、19,361 writebacks／39,195 pages、
17,295 mapping deltas／18,810 dirty pages、generation 27→37,228、55,637 Wasm crossings、58,055 success
transitions、clean cancel／watchdogを通した。Browser reportは43,932 bytes／SHA-256
`7fa24d25ae9ebacaf30fc595516dc8f8b036781be0db4c6011dd562eb0b5a800`である。

次stopはinstruction 11,957,745、`GDI32.dll!EnumFontFamiliesExA` endpoint ID 253／address `0x7F000FC0`／IAT
`0x100D1080`／return `0x1002C16A`である。Argumentsはlogical HDC `0x47010003`、LOGFONTA
`0x006FFD00`、callback `0x1002D2E0`、LPARAM `0x100DCE7C`、flags 0で、faultは
`gdi32-api-contract／endpoint-contract-missing`である。Font catalog、callback delivery、metrics、text rasterization、
screen drawing、host font／HDC／DOM topologyはこのcheckpointでclaimしない。

## 136. BGI CP932 font enumeration callback

Callback-aware `screen-dc` profileを独立に2回取得した。Primary raw traceは98,030 bytes／95 records／SHA-256
`eb09cfc2a18967f74e6054beb57b92688e08c2d01556841cea94fa59e30d3027`、confirmationは97,732 bytes／95
records／SHA-256 `9a337ac82e09ec7c6c6c9a3e9ac9def7042e7af03be6e05c286ab81b9f0fbb33`で、diagnostics／hook
errorsは0である。Profileは`x86-bgi-screen-dc-font-enumeration-callback-v2`、oracleは
`x86-screen-dc-and-font-enumeration-v2`／`x86-cp932-ms-gothic-callback-enumeration-v1`である。Raw traceは
`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

Primary callはcharset 128、Windows-932 face bytes
`82 6c 82 72 20 83 53 83 56 83 62 83 4e`、flags 0を渡す。Callback RVA `0x0002D2E0`は一度だけ呼ばれ、
LPARAM RVA `0x000DCE7C`をそのまま受け、0を返す。Outer resultも0、LastErrorは0のままで、その後callerは結果を
BOOL TRUEへ正規化する。Callbackは`ENUMLOGFONTEXA + 0x1C`のfaceとLPARAMをconsumerへ渡し、stdcall
`ret 0x10`で戻る。

Stable record semanticsはheight 32、width 16、weight 400、charset 128、out／clip precision 3／2、quality 1、
pitch-family 49、face／full name `ＭＳ ゴシック`、style `標準`、script `日本語`、font type 4である。
`NEWTEXTMETRICEXA`はheight／ascent／descent 32／28／4、average／maximum width 16／63、weight 400、
aspect 96x96、first／last／default／break 0／255／165／2、pitch-family 54、charset 128、flags 2,359,360、
size-em／cell-height／average-width 256／256／128を返す。24-byte font signatureは
`ff 02 00 e0 fb fd c7 6a 12 00 00 08 00 00 00 00 9f 00 02 40 00 00 d7 df`である。

Controlled oracleはcallback 0のstop caseとcallback 1のcontinue caseを各一度実行し、outer result 0／1、callback
data round-trip、inputと前後guard不変、全success LastError sentinel保存を確認した。Fixed ANSI fieldのNUL以降は
Windowsが初期化せずraw bytesがcall間で異なる一方、NULまでのface／full name／style／scriptと全metric fieldsは
一致する。Vineはこのsemantic部分だけをimportし、canonical recordのtailをzero-fillする。

Nodeは18,610 supported calls、18,666 writebacks／37,580 dirty pages／2,946,837 changed bytes、17,296 host
deltas、generation 36,533、semantic SHA-256
`0ca0ec8d32618282a2c7113b21974a76d89c8a964c9176f7764456ba7c6e35e6`、terminal memory SHA-256
`aa3b09f8c749eb54213ce2a7be2545f3479700cd1ffebe9e9f3d9ca253ee1a6e`を通した。Node reportは64,609
bytes／SHA-256 `aa92175a31125d34770434b01c654a662ef40166bdd533b9d39d41fea4d48228`である。Chrome Runtime
Workerもsame ordered semantics／terminal memoryを再現し、19,366 slices、19,364 writebacks／39,198 pages、
17,296 mapping deltas／18,812 dirty pages、generation 27→37,233、55,644 Wasm crossings、58,063 success
transitions、clean cancel／watchdogを通した。Browser reportは44,083 bytes／SHA-256
`dc14a7759c0d805b57a45a1eacdff217776c7157974d145155818d28cdb09bde`である。

Enumerationの後に`ReleaseDC(NULL, 0x47010003)`が1を返してlive HDCを0にし、次stopはinstruction
11,958,023、`GDI32.dll!CreateFontA` endpoint ID 256／address `0x7F000FF0`／IAT `0x100D108C`／return
`0x1002C838`である。Captured argument prefixはheight 64、width 32、escapement／orientation 0／0、weight 100、
italic／underline 0／0で、faultは`gdi32-api-contract／endpoint-contract-missing`である。Font object identity、
selection、host／browser font resolution、text metrics consumer、glyph rasterizationはこのcheckpointでclaimしない。

## 137. BGI CP932 logical font object creation

`font-object` profileを独立に2回取得した。Primary raw traceは479,331 bytes／118 records／SHA-256
`e247e276261c0ed0fd03874c45e1b7f51fa51a02163b427b58c7e6b5b7b0fcd8`、confirmationは479,085 bytes／
117 records／SHA-256 `eade672e0bb09f6b14952059cbc092280ef760cc7138aa845dd2f1c3527f6642`で、diagnostics／
hook errorsは0である。Main profileは`x86-bgi-gdi-font-object-lifecycle-v1`、controlled oracleは
`x86-create-font-a-object-lifecycle-v1`である。Raw traceは`.work/`に保持しGitへ追加しない。Source executableは
1,193,472 bytes／SHA-256 `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

RVA `0x0002C838`へ戻る`CreateFontA`はheight 64、width 32、escapement／orientation 0、weight 100、
italic／underline／strikeout 0、charset 128、output／clip precision 4／0、quality 2、pitch-family 0、Windows-932
face `ＭＳ ゴシック`を渡す。Resultはnon-NULL、LastErrorは0で、callerはhandleを保存してnonzeroを検査した後、
`CreateCompatibleDC(NULL)`へ進む。Native numeric HFONTとpointerはartifact identityでありVineへimportしない。

Primary lifecycleは`CreateFontA`、`CreateCompatibleDC`、`CreateDIBSection`、`GetObjectA`、二回の`SelectObject`、
二回の`GetOutlineTextMetricsA`、`TextOutA`、二回のrestore `SelectObject`、二回の`DeleteObject`、`DeleteDC`の
14 callである。DIBは128 x -96、8 bpp `BI_RGB`、stride 128、12,288 pixel bytes、256-entry reverse-grayscale
paletteを持つ。`TextOutA(0, 0, "a", 1)`前後のpixel FNV-1aは`0x52965DC5`から`0xE6FC8461`へ変わり、0／255だけの
476 bytes、bounding box `(3,20)-(28,54)`が変化した。CallerはDIB bytesを走査するため、このraster結果は単なる
host artifactではなく後続guest-visible semanticsである。ただし本checkpointはrasterを実装しない。

Controlled oracleは`GetObjectA(font, 60)`が60を返し、要求したLOGFONTAをexactにround-tripして前後guardを
保存することを確認した。Font select／restoreはobject identityを保持し、`DeleteObject`はfirst callでTRUE、repeatで
FALSEとなる。Deleted objectへの`GetObjectA`は0でoutputを保存し、全success／failure pathがLastError sentinelを
保存する。Vineはこのうちcreationだけを実装し、`RuntimeGdi`がdistinct monotonic ID／handleとexact LOGFONTAを
owner-local plan／commitで所有する。Successはguest write 0、LastError preserve、browser font resource 0である。

Nodeは18,612 supported calls、18,668 writebacks／37,583 dirty pages／2,947,946 changed bytes、17,297 host
deltas、generation 36,536、semantic SHA-256
`1cba61d15517881f4eda4fbf65fb8a7544cddb24d3c7e2f9bfab18e3d319a753`、terminal memory SHA-256
`c85b014991b18f2e7a6e277d223c9fe49370f23186ddc13b9af2cc2eb5f4796d`を通した。Node reportは66,512
bytes／SHA-256 `3b77b539c3639fa7d160d7ffc1f7e9be80d30c5a8c089b94a99d0937a2bba2ea`である。Chrome Runtime
Workerもsame ordered semantics／terminal memoryを再現し、19,368 slices、19,366 writebacks／39,201 pages、
17,297 mapping deltas／18,813 dirty pages、generation 27→37,236、55,649 Wasm crossings、58,069 success
transitions、clean cancel／watchdogを通した。Browser reportは44,116 bytes／SHA-256
`46572eb6b101c70908079364ac98fd93741288fabe68633aacea76af9d64363c`である。

次stopはinstruction 11,960,140、`GDI32.dll!CreateCompatibleDC` endpoint ID 252／address `0x7F000FB0`／IAT
`0x100D107C`／return `0x1002C8AD`の`endpoint-contract-missing`で、argumentはNULL HDCである。Compatible
memory DC、DIB、selection／introspection／deletion、outline metrics、text output、glyph rasterization、host／browser
font resolutionはこのcheckpointで実装済みとしない。

## 138. BGI NULL-source compatible memory DC lifecycle

Compatible-DC oracleを含むfont-object profileを独立に2回取得した。Primary raw traceは489,171 bytes／132 records／
SHA-256 `4e7015df75b77ad133c8eb28437bfe26e45f5171e692aa913148cf3d74da1e72`、confirmationは489,137 bytes／
132 records／SHA-256 `48ac1ae635eeb927b0da807bafa565c96d6905ada1dbb50dac53b282ca276008`で、diagnostics／hook
errorsは0である。Main profileは`x86-bgi-gdi-font-object-lifecycle-v2`、outer controlled oracleは
`x86-create-font-a-object-lifecycle-v2`、compatible-DC oracleは`x86-create-compatible-dc-null-lifecycle-v1`である。
Raw traceは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

BGIのRVA `0x0002C8AD`へ戻る`CreateCompatibleDC(NULL)`はnon-NULL HDCを返しLastError 0を保存する。Callerは
resultをobject fieldへ格納し、testを挟まず同じHDCを`CreateDIBSection`へ渡す。Main ordered pipelineは
`CreateFontA`、`CreateCompatibleDC`、`CreateDIBSection`、`GetObjectA`、二回の`SelectObject`、二回の
`GetOutlineTextMetricsA`、`TextOutA`、二回のrestore `SelectObject`、二回の`DeleteObject`、`DeleteDC`で両run一致した。

Controlled oracleはNULL sourceから三つのdistinct non-NULL HDCを作り、全てのobject typeが10であることを確認した。
First／second DCのinitial fontは同じidentityでtype 6、initial bitmapも同じidentityでtype 7である。
`ReleaseDC(NULL, firstMemoryDc)`は0、LastError 1425で、その後もfirst HDCのtypeは10のままである。`DeleteDC`は
first／repeat／independent secondで1／0／1となり、first delete後のtypeは0である。三createと三deleteの全callは
success／failureとも各sentinel LastErrorを保存し、hook result／LastErrorとSystemFunction結果が一致した。

別variantでは`DeleteObject(thirdMemoryDc)`が1を返してtype 0へretireした。このcallはprimary pipelineで未到達であり、
Vineの`DeleteObject` contractへ一般化しない。Vineは到達済みの`CreateCompatibleDC(NULL)`とlifecycle上必要な
`DeleteDC`だけを実装する。`RuntimeGdi`はdistinct same-thread HDC、process-shared `SYSTEM_FONT` index 13、1 x 1
monochrome `DEFAULT_BITMAP` index 21、selected object IDを一つのowner-local plan／commitで所有する。`ReleaseDC`、
non-NULL source、native HDC、pixel storage、browser resourceはこのcontractに含めない。

Nodeは18,613 supported calls、18,669 writebacks／37,584 dirty pages／2,947,967 changed bytes、17,297 host
deltas、generation 36,537、semantic SHA-256
`7a5429d465517e76d66ba8982448f640b48e654d711ba9d1e35975a7cc78fec6`、terminal memory SHA-256
`2bcba5822cd7c9ec36e4a37a70865bc18411eeae5d3ce88e9fa2f38ce2f79e76`を通した。Node reportは68,391
bytes／SHA-256 `2746dc4d80b50518b93769de1a1762e5d220b814b40fd8b9d5b19172cb3a8c09`である。Sequence 18,613は
HDC `0x47010004`／DC ID 4、selected font／bitmap ID 3／4、stock handles `0x47020003`／`0x47020004`、
live DC／object 1／4、guest write 0、LastError 0を返した。

Chrome Runtime Worker protocol v8もsame ordered semantics／terminal memoryを再現し、19,369 slices、19,367
writebacks／39,202 pages、17,297 mapping deltas／18,813 dirty pages、generation 27→37,237、55,651 Wasm
crossings、58,072 success transitions、clean cancel／watchdogを通した。Chromeは150.0.7871.130、browser reportは
44,081 bytes／SHA-256 `6dbc90925d425a1574c68292db0a6e9cf23c1a477a75c45aa51a4e06409e3578`である。

次stopはinstruction 11,960,150、`GDI32.dll!CreateDIBSection` endpoint ID 257／address `0x7F001000`／IAT
`0x100D1090`／return `0x1002C8C5`の`endpoint-contract-missing`である。Argumentsはmemory HDC
`0x47010004`、`BITMAPINFO` `0x2013A250`、usage 0、bits output `0x006FFEF4`、section 0、offset 0である。
DIB identity／bytes、palette、guest output publication、object selection／introspection／deletion、metrics、text output、
glyph rasterizationは次checkpointで扱う。

## 139. BGI 8-bpp DIB section and object lifecycle

DIB lifecycle oracleを含むfont-object profileを独立に2回取得した。Primary raw traceは624,813 bytes／155 records／
SHA-256 `7088a2950ad7337d2e08e33134ec3aebda929912e5517a6956cd00ab24709abc`、confirmationは624,940 bytes／
156 records／SHA-256 `e8ff31fe0e6fdfb6c2115e41ccd0462cac149d6f01c44750b8aff3677787d7a3`で、diagnostics／hook
errorsは0である。Main profileは`x86-bgi-gdi-font-object-lifecycle-v3`、outer oracleは
`x86-create-font-a-object-lifecycle-v3`、DIB oracleは`x86-create-dib-section-8bpp-lifecycle-v1`である。Raw traceは
`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

BGIのRVA `0x0002C8C5`へ戻る`CreateDIBSection`は40-byte `BITMAPINFOHEADER`、width 128、height -96、planes 1、
8 bpp、`BI_RGB`、size-image／resolution 0、colors-used 256、important 0、usage `DIB_RGB_COLORS`、NULL section、
offset 0を渡す。Paletteは256-entry reverse-grayscale RGBQUAD、strideは128、pixel backingは12,288 bytesで初期値は
全zero、FNV-1aは`0x52965DC5`である。Resultとbits outputはいずれもnon-NULLでLastError 0を保存する。

直後の`GetObjectA(bitmap, 24)`は24を返し、`BITMAP` fieldsはtype 0、width 128、height 96、width-bytes 128、
planes 1、bits-pixel 8で、bits fieldはcreate outputと同じidentityを持つ。前後guardは不変である。続く二つの
`SelectObject`はDIBとcreated fontをmemory DCへ選択し、primary pipelineはtext処理後にdefault bitmap／fontを
同じidentityでrestoreしてから二つの`DeleteObject`を成功させる。Malformed zero headerはNULLを返してbits outputを
NULLへし、deleted objectの`GetObjectA`は0かつoutput preserve、全success／failureはLastError sentinelを保存した。

Native Windowsではselected DIBへの`DeleteObject`が1を返し、lifetimeをselection解除までdeferする。一方、primary
pipelineはdelete前にrestoreするため、Vineはより明示的なlifetime invariantを採用し、selected object deleteを
mutation-freeに拒否する。Native oracleではdeleted HDCを渡すDIB createも成功したがprimary未到達であり、Vineはlive
same-thread memory HDCを要求する。これら二variantを一般互換性としてclaimしない。Native numeric HGDIOBJ、HDC、bits
pointerはartifact identityである。

Vineはsequence 18,614でDIB handle `0x47020005`／object ID 5とbits `0x44001000`を返した。Descriptorは
`[128,-96,1,8,0,128,12288]`、allocation ID 59、memory owner 29、region kind `gdi-resource`、lifetime
`object-explicit-release`である。Creation deltaはgeneration 36,537→36,540、mapping operation 2、stack output pageと
三pixel pageの4 dirty pages、control 200 bytes、payload 16,384 bytes／SHA-256
`fcd36d346f9edd7098ec07a2d73a83319e75c08e9ac1c9ded494bf2470dcabe2`である。これはHDC／policy、GDI identity、
address allocation、memory owner／mapping／zero bytes、guest outputを全てpreflightしてからpublishした結果である。

Sequence 18,616の`GetObjectA`は24-byte hex
`000000008000000060000000800000000100080000100044`を書き、sequence 18,617／18,618の`SelectObject`はDIB／font
handleに対してdefault bitmap／font handleを返し、selection generationを1／2へ進めた。Fixture lifecycleはrestore後の
deleteでGDI object、memory region／owner、address allocationを一緒にretireし、selected delete、stale object、malformed／
unreached descriptor、stale HDCをmutation前に拒否する。

Nodeは18,618 supported calls、18,674 writebacks／37,589 dirty pages／2,948,065 changed bytes、17,300 host
deltas、40 callbacks、generation 36,547、semantic SHA-256
`0ec3628c95c953e17c795d0dfa27cfca876aef037c882e442402cd9bd96c410f`、terminal memory SHA-256
`b90c095742be75c1d751865978199540e5f2bfbe4ff25844a03d4fa5f4fb5d50`を通した。Node reportは205,533
bytes／SHA-256 `5e925915502cc3f7086697234facf77ffd0ec6246fe8f7f792359324dee5fc44`である。

Chrome Runtime Worker protocol v9もsame ordered semantics／terminal memoryを再現し、19,374 slices、19,372
writebacks／39,207 pages、17,300 mapping deltas／18,819 dirty pages、generation 27→37,247、55,664 Wasm
crossings、58,087 success transitions、clean cancel／watchdogを通した。Chromeは150.0.7871.130、browser reportは
44,128 bytes／SHA-256 `3245d4809b8099d5233926869cf597e13a50c891a8ad6499184084518abdf902`である。

次stopはinstruction 11,960,243、`GDI32.dll!GetOutlineTextMetricsA` endpoint ID 259／address `0x7F001020`／IAT
`0x100D1098`／return `0x1002C51F`の`endpoint-contract-missing`である。Argumentsはmemory HDC
`0x47010004`、size 0、output NULLである。Outline metric size query、nonzero output、text rasterization、host／browser
font resolutionはこのcheckpointでclaimしない。

## 140. BGI selected-font outline metrics output

Outline-metrics oracleを含むfont-object profileを独立に2回取得した。Primary raw traceは673,869 bytes／180
records／SHA-256 `0b46e3c68be56b8948bd9a748350f85d68ddbf8751e3362d146686a3639eac6e`、confirmationは
674,077 bytes／180 records／SHA-256 `e7f57e51fab0399c2a53197ab30a187451eeee28388217e5b6a507df473fe002`で、
diagnostics／hook errorsは0である。Main profileは`x86-bgi-gdi-font-object-lifecycle-v4`、outer oracleは
`x86-create-font-a-object-lifecycle-v4`、focused oracleは`x86-get-outline-text-metrics-a-cp932-v1`である。Raw
traceは`.work/`に保持しGitへ追加しない。Source executableは1,193,472 bytes／SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のままである。

BGIはselected 64 x 32、weight 100、charset 128の`ＭＳ ゴシック`に対して、RVA `0x0002C51F`でsize 0／output
NULLを渡して269を得た後、RVA `0x0002C539`で269-byte bufferを渡して269を得る。Second outputのfixed
212-byte prefixはsize／text height／ascent／descent 269／64／55／9、family／face／style／full-name relative
offsetは212／226／240／245である。FamilyとfaceはWindows-932 `ＭＳ ゴシック`、styleは`標準`、full nameは
`Microsoft:ＭＳ ゴシック`で、callerはfamily pointerを解決してrequested faceと比較する。

Controlled boundary callsは`cbData` 1／2／3で`0x122`のlow 1／2／3 bytesを書き、requested sizeを返す。
4..212では`otmSize`をrequested sizeにしてそのbytesを返す。213..268ではfixed 212 bytesだけを書き、0／
LastError 1003を返す。269以上では269 bytesだけを書いて269を返し、size query、partial success、full／oversized
successはLastError sentinelを保存する。Deleted HDCは0／LastError 6で、全buffer guardは不変である。Two runの
raw outputはnative padding offsets 57..59／71だけが異なるため、この4 bytesをzeroにしたcanonical outputの
SHA-256 `d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d`をsemantic policy valueとした。

Vineは`RuntimeGdi`がlive same-thread memory HDCとselected nonshared FontAを検証し、versioned GDI32 policyが
269-byte CP932 recordを構築する。Host／browser font measurement、native HDC、native paddingはimportしない。
Sequence 18,619はsize queryで269／guest write 0、sequence 18,621はaddress `0x2013A250`へ269 bytesを書いて269を
返し、HDC `0x47010004`、font `0x47020002`、selection generation 2を維持した。

Nodeは18,622 supported calls、18,678 writebacks／37,597 dirty pages／2,948,267 changed bytes、17,303 host
deltas、generation 36,554、semantic SHA-256
`cd1c460bbb9cf5f147f03632bf437f4135ade530629692caaf78fe374313347a`、terminal memory SHA-256
`539b45ea0e9361c435c20ee8c84633a553d9fb475cf97e061b7e0dad640647d6`を通した。Node reportは215,886
bytes／SHA-256 `85ff27e79157531267abbe8b25cf578072688cae25378e4a2edc5e819539223e`である。

Chrome Runtime Worker protocol v10もsame ordered semantics／terminal memoryを再現し、19,378 slices、19,376
writebacks／39,215 pages、17,303 mapping deltas／18,822 dirty pages、generation 27→37,254、55,675 Wasm
crossings、58,099 success transitions、clean cancel／watchdogを通した。Chromeは150.0.7871.130、browser reportは
44,078 bytes／SHA-256 `c6c725be6be03b7bdbc019612d0c914004b9353efdd7a8c1fbd692f13d6b76df`である。

次stopはinstruction 11,961,619、`GDI32.dll!TextOutA` endpoint ID 248／address `0x7F000F70`／IAT
`0x100D106C`／return `0x1002CAB1`の`endpoint-contract-missing`である。Argumentsはmemory HDC
`0x47010004`、x／y 0／0、Windows-932 text pointer `0x006FFD6C`、count 1で、byteはASCII `a`である。Text
output、DIB pixel mutation、glyph rasterization／substitution、host／browser font measurementはこのcheckpointでclaimしない。
