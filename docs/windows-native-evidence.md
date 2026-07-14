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
