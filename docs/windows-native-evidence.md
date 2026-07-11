# Windows native evidence

## 1. 文書情報

- ステータス: W0／trace v2 attribution／selected IDirect3D9 root／W1／W2／W3／W7 PE32 load／memory／entry return／stack／bootstrap／explicit import traps／BGI firstからseventh Win32 callはVerified。visual title／device-level method coverageはIn discovery
- 取得日: 2026-07-10／2026-07-11／2026-07-12
- runtime checkpoint更新日: 2026-07-12
- 完了 checkpoint: `W0`、`trace v2 attribution ingress`、`selected IDirect3D9 root capture`、`W1`、`W2 runtime flow`、`W3 browser first-pixel boundary`、`W7 PE32 load／memory／entry return ingress`、`production stack bootstrap ingress`、`bootstrap FS first-stop ingress`、`bootstrap FS ingress`、`process-parameters first stop`、`process-parameters ingress`、`explicit unsupported trap binding`、`BGI first runtime trap`、`BGI first Win32 call return`、`BGI second Win32 call return`、`BGI third Win32 call return`、`BGI fourth Win32 call return`、`BGI fifth Win32 call return`、`BGI sixth Win32 call return`、`BGI seventh Win32 call return`、`P0-COR-04` の最初のnative sample
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

`npm run check:runtime:bgi-first-trap`はexisting reflection schemaをopt-in上限400 pagesへ拡張し、dedicated runnerで8 MiB protocol-line上限を適用する。BGI image 380 pages、import arena 2、control 1、stack 1、TEB／PEB 2、process parameters／command line 2の合計388 pagesをgeneration `24`でreflectした。4,904,602-byte offerのcontent SHA-256は`f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`である。

2 MiB adapter poolではphysical VGA MMIO hole `[0xA0000,0xC0000)`を除外した。directory 1、page tables 4、reflected pages 388の393 allocationsとPDE indices `[1,64,508,511]`を使い、MMIO callbackから値を返さずに配置した。actual TLB／fetch／generated-code invalidation後だけgeneration `24`をacknowledgeした。

entry `0x100A383E`はinstruction budget `1`で各logical execute boundaryを検査し、19 execute stop／18 budget stop／19命令でexact owned thunk ID `104`／`KERNEL32.dll!GetSystemTimeAsFileTime`／`0x7F000670`へ到達した。IATは`0x100D1248`、ESPは`0x006FFFD0`、returnは`0x100A88D2`、raw stack wordsは`[0x100A88D2,0x006FFFEC,0,0,0,0,0,0]`である。API dispatch／write／returnは行わない。9,592-byte reportのSHA-256は`4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840`で2回一致し、source hashは前後不変だった。詳細は[v86-bgi-first-trap.md](v86-bgi-first-trap.md)を正とする。

## 27. BGI first supported Win32 call return

`npm run check:runtime:bgi-first-win32-call`はID `104`を2番目のdeclared endpointとし、`GetSystemTimeAsFileTime`のcall site `0x100A88CC`、IAT `0x100D1248`、one-argument stdcall frame `[0x100A88D2,0x006FFFEC]`をchecked処理する。host clock owner `checkpoint-host-clock`はUTC `2026-07-11T00:00:00Z`をWindows ticks `134282016000000000`／bytes `[00,40,67,37,C8,10,DD,01]`として供給する。

outputがv86-mutated stack page上にあるため、current page `0x006FF000`／owner `4`をgeneration `24`と照合してRustへreconcileした。initial pageとの差は11 bytes、page SHA-256は`46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785`である。続く8-byte writeはgeneration `25`、dirty page 1／4 KiB、mapping operations 0、cache effects 0、delta SHA-256 `dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840`となった。full dirty pageがcurrent v86 page＋exact outputだけであることを検査し、live mapping適用／Rust acknowledgement後はdirty pages 0である。

stdcall returnはESPを`0x006FFFD0`から`0x006FFFD8`へ進め、VOID APIのcheckpoint policyとしてEAXを保持した。4命令後、entryから累計23命令でexact thunk ID `103`／`KERNEL32.dll!GetCurrentProcessId`／`0x7F000660`へ到達した。IATは`0x100D1244`、ESPは`0x006FFFD4`、returnは`0x100A88DE`である。14,540-byte version 1 reportのSHA-256は`7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16`、source hashは前後不変だった。これはone fixed clock sample／page reconciliation／API returnであり、general clock、dirty tracking、process identity、ABI gatewayを実装しない。詳細は[v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md)を正とする。

## 28. BGI second supported Win32 call return

`npm run check:runtime:bgi-second-win32-call`はID `103`を3番目のdeclared endpointとし、`GetCurrentProcessId`のcall site `0x100A88D8`、IAT `0x100D1244`、zero-argument stdcall frame `[0x100A88DE]`をadapterとRustの両方で検査する。process identity owner `checkpoint-process-identity`はhost PIDに依存しないcontrolled PID `4096`を供給する。

guest call frameの4 changed bytesを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`0x006FFFEC`から`4096`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

3命令後、entryから累計26命令でexact thunk ID `149`／`KERNEL32.dll!GetCurrentThreadId`／`0x7F000940`へ到達した。IATは`0x100D12FC`、ESPは`0x006FFFD4`、returnは`0x100A88E6`である。18,472-byte reportのSHA-256は`bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67`で、source hashは不変だった。これはone controlled PID／page reconciliation／API returnであり、general process／handle／thread modelを実装しない。詳細は[v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md)を正とする。

## 29. BGI third supported Win32 call return

`npm run check:runtime:bgi-third-win32-call`はID `149`を4番目のdeclared endpointとし、`GetCurrentThreadId`のcall site `0x100A88E0`、IAT `0x100D12FC`、zero-argument stdcall frame `[0x100A88E6]`をadapterとRustの両方で検査する。thread identity owner `checkpoint-thread-identity`はhost TIDに依存しないcontrolled TID `8192`を供給する。

guest call frameの1 changed byteを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`4096`から`8192`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

3命令後、entryから累計29命令でexact thunk ID `13`／`KERNEL32.dll!GetTickCount`／`0x7F0000C0`へ到達した。IATは`0x100D10DC`、ESPは`0x006FFFD4`、returnは`0x100A88EE`である。22,383-byte reportのSHA-256は`d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa`で、source hashは不変だった。これはone controlled TID／page reconciliation／API returnであり、general process／thread／TEB client-ID／scheduler modelを実装しない。詳細は[v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md)を正とする。

## 30. BGI fourth supported Win32 call return

`npm run check:runtime:bgi-fourth-win32-call`はID `13`を5番目のdeclared endpointとし、`GetTickCount`のcall site `0x100A88E8`、IAT `0x100D10DC`、zero-argument stdcall frame `[0x100A88EE]`をadapterとRustの両方で検査する。monotonic clock owner `checkpoint-monotonic-clock`はhost uptimeに依存しないsample ID `uptime-ms-3600000`／tick count `3,600,000`を既存fixed wall-clock sampleとpairする。

guest call frameの1 changed byteを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。page SHA-256は`f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b`である。host memory writeはないためgeneration／acknowledged generationは`25`、dirty pagesは0のままで、deltaは発行しない。EAXを`8192`から`3,600,000`へ置き換え、ESPを`0x006FFFD4`から`0x006FFFD8`へ進めてreturnした。

5命令後、entryから累計34命令でexact thunk ID `157`／`KERNEL32.dll!QueryPerformanceCounter`／`0x7F0009C0`へ到達した。IATは`0x100D131C`、ESPは`0x006FFFD0`、returnは`0x100A88FA`、output pointerは`0x006FFFE4`である。26,558-byte reportのSHA-256は`79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111`で、source hashは不変だった。これはone controlled tick-count sample／page reconciliation／API returnであり、general clock／timer／scheduler／snapshot modelを実装しない。詳細は[v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md)を正とする。

## 31. BGI fifth supported Win32 call return

`npm run check:runtime:bgi-fifth-win32-call`はID `157`を6番目のdeclared endpointとし、`QueryPerformanceCounter`のcall site `0x100A88F4`、IAT `0x100D131C`、frame `[0x100A88FA,0x006FFFE4]`をadapterとRustの両方で検査する。同じmonotonic ownerのsample ID `qpc-ticks-12345678901`は64-bit counter `12,345,678,901`をtick sampleとpairするが、frequency／originを確定しない。

guest call frameを含むstack page `0x006FF000`／owner `4`をgeneration `25`へreconcileした。5 changed bytes、page SHA-256 `a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc`を保持し、zeroだったoutputへbytes `[35,1C,DC,DF,02,00,00,00]`を書いた。generation `26` deltaはdirty page 1／4 KiB、mapping／cache effects 0、SHA-256 `e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951`である。ack後にEAX `1`、ESP `0x006FFFD8`でreturnした。

44命令後、entryから累計78命令でexact thunk ID `83`／`KERNEL32.dll!GetStartupInfoW`／`0x7F000520`へ到達した。IATは`0x100D11F4`、ESPは`0x006FFF78`、returnは`0x100A36E7`、output pointerは`0x006FFF90`である。31,427-byte reportのSHA-256は`eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1`で、source hashは不変だった。これはone controlled counter／page reconciliation／delta／API returnであり、general clock、frequency、timer、scheduler、snapshotを実装しない。詳細は[v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md)を正とする。

## 32. BGI sixth supported Win32 call return

opt-in `--observe startup-info` traceを2回取得し、target callがBGI caller RVA `0xA36E7`へreturnすること、x86 output sizeが68 bytesであること、未初期化inputが全書込されることを確認した。native outputは`cb=68`、empty reserved／desktop strings、absolute image-path title、flags `0x100`、3 nonzero inherited standard handlesを持つ。pointer／handle valuesはlaunchごとに変わるため、runtimeのcontrolled stateへコピーしない。

`npm run check:runtime:bgi-sixth-win32-call`はID `83`を7番目のdeclared endpointとし、call／return `0x100A36E1`／`0x100A36E7`、IAT `0x100D11F4`、frame `[0x100A36E7,0x006FFF90]`をadapterとRustで検査する。owner `checkpoint-process-startup`のprofile `startupinfow-v68-zero-optional-fields`は`cb=68`だけを定義し、残る64 bytesをzeroにする。generation `26` stack pageとの差は31 bytes、page SHA-256は`75e2211e3e9a79f92313319d9897bb1e868d74bb15f0dc513cc9cb393c9bb843`である。

68-byte writeはgeneration `27`、dirty 1 page／4 KiB、mapping／cache effects 0、delta SHA-256 `a2c82c133e41916b0b9af10314376ece9a281a8503db4b811031e5ce52919de8`となった。ack後はVOID policyでEAX `0x006FFF90`を保持し、ESP `0x006FFF80`へreturnする。9命令後、entryから累計87命令でexact ID `84`／`KERNEL32.dll!HeapSetInformation`／`0x7F000530`へ到達した。frameは`[return,NULL,1,NULL,0]`、39,031-byte reportのSHA-256は`208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c`で、source hashと先行report hashesは不変だった。これはone controlled startup-info profileであり、general process creation、desktop、title path、console／standard handlesを実装しない。詳細は[v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md)を正とする。

## 33. BGI seventh supported Win32 call return

opt-in `--observe heap-set-information` traceを2回取得し、target callがBGI caller RVA `0xA36FC`へreturnすること、argumentsが`[NULL,1,NULL,0]`、BOOL resultが`1`であることを確認した。LastErrorはenter／leaveとも`87`で、成功時に変更されないstale thread stateだった。

`npm run check:runtime:bgi-seventh-win32-call`はID `84`を8番目のdeclared endpointとし、call／return `0x100A36F6`／`0x100A36FC`、IAT `0x100D11F8`、frame `[return,NULL,1,NULL,0]`をadapterとRustで検査する。owner `checkpoint-process-heap-policy`のpolicy `heap-enable-termination-on-corruption`だけを受け付け、generation `27` stack pageとの差12 bytes／SHA-256 `d121a898c549de8cf2eea5d76912375a993cac9a7dfb41d101ef2eb2b68fa3da`をreconcileする。guest writeはなくgeneration `27`／dirty 0を保持し、LastErrorを変更せずBOOL `1`、ESP `0x006FFF80`でreturnする。

21命令後、entryから累計108命令でexact ID `102`／`KERNEL32.dll!HeapCreate`／`0x7F000650`へ到達した。frameは`[return,0,0x1000,0]`、43,980-byte reportのSHA-256は`09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f`で、source hashと先行report hashesは不変だった。これはone controlled heap-policy selectionであり、general heap allocator、corruption termination、LastError modelを実装しない。詳細は[v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md)を正とする。
