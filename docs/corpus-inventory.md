# ローカル互換性 corpus 台帳

## 1. 文書情報

- ステータス: In discovery
- 基準日: 2026-07-10
- 開発形態: 1 人、完全個人用
- corpus root: `V:\softs`
- 主目標: `V:\softs\サクラノ詩\BGI.exe`
- 調査方法: `llvm-readobj` と hash による read-only static inventory、Frida 17.15.4 による focused native trace

この文書は、実装順と互換性 gap を決めるための T3 corpus 台帳である。ゲーム本体、実行ファイル、同梱 DLL、asset、既存の解析生成物は repository に格納しない。静的 import は実行時に通る API を保証せず、`LoadLibrary`／`GetProcAddress`、COM vtable、条件分岐後の call は dynamic trace で補う。

## 2. Scope decision

- Active completion target は、サクラノ詩の `boot -> title -> all gameplay -> save/load -> ending` を Chrome で不自由なく通せる full-play runtime とする。短い scenario は段階的な compatibility checkpoint として使う。
- Multi-title compatibility、第三者向け配布、beta release は active completion target に含めない。
- D3D9 と DirectInput 8 はサクラノ詩の静的 dependency なので active scope に入れる。ただし一般的な D3D9 compatibility ではなく、観測した method／format／state の bounded subset とする。
- PE32+、x86_64、D3D11、DXGI、XAudio2、Unity は active scope に入れない。
- Nursery Rhyme は同じ BGI／Ethornell 系統の DirectDraw 比較対象として扱う。DirectDraw 自体はサクラノ詩 completion の必須 gate にしない。

## 3. Executable inventory

全 7 entry executable が PE32、Intel i386、Windows GUI application であることを確認した。

| Title | Entry executable | Size | SHA-256 |
|---|---|---:|---|
| Nursery Rhyme | `NurseryRhyme.exe` | 548,864 | `3d6e80862c58cfa84d1843e0910d8b06383a9f690cd95c0bd3fde94a28feddd0` |
| School Days HQ | `SCHOOLDAYS HQ.exe` | 663,040 | `bbcbb8ffb1c55b0491d5671a762ed78f6045054833e8920f582bafb4c372db20` |
| Shiny Days | `SHINYDAYS.exe` | 667,648 | `c1fca1a5f1a1d53d56bde70560991364ab754365cb28ba800330f54c44267956` |
| PURIPA | `PURIPA.EXE` | 548,864 | `ca6c37d47925104920c64ce76d056dfba2984307cfceaff0c74fa0e98b876045` |
| しゅがてん！ | `syugaten.exe` | 4,559,872 | `0c761a76c67f46ceffedaa283374463a05d1f6f6cd057c8e8a30955f83985099` |
| ふぃぎゅ@メイト | `figuat.exe` | 753,664 | `9c68f1591f4cf4cfc8c2e157cea530dadbfa54d9c4c24db2c9efdb5c485fee5e` |
| サクラノ詩 | `BGI.exe` | 1,193,472 | `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1` |

Hash は今回観測した build の identity であり、別版を同一 profile とみなす根拠にはしない。

## 4. Static dependency summary

| Title | Graphics／input | Audio／media | その他の重要な静的 dependency | Corpus role |
|---|---|---|---|---|
| サクラノ詩 | D3D9、DirectInput 8、GDI、GDI+、User32 | DirectSound ordinal import、WinMM／MCI | COM、Shell、Common Dialog、registry、WinInet | Primary T3 target |
| Nursery Rhyme | DirectDraw、GDI、User32 | DirectSound ordinal import、WinMM／MCI | COM、Shell、Common Dialog、registry | Same-family DirectDraw comparison |
| PURIPA | DirectDraw、GDI、User32、IMM32 | DirectSound ordinal import、WinMM timer | COM、registry | Secondary DirectDraw comparison |
| School Days HQ | D3D9、GDI、User32 | DirectSound、WMVCore | custom DLL 2 本、DSETUP、MSVC 14／Universal CRT | Deferred D3D9／video comparison |
| Shiny Days | D3D9、GDI、User32 | DirectSound、WMVCore | custom DLL 2 本、DSETUP、MSVC 9 | Deferred D3D9／video comparison |
| しゅがてん！ | GDI、User32、IMM32。binary string に `Direct3DCreate9` | WinMM。binary string に `DirectSoundCreate` | Shell、Common Dialog、SHLWAPI、DbgHelp | Dynamic-resolution comparison |
| ふぃぎゅ@メイト | GDI、User32 | WinMM／MCI | Winsock 1.1、Shell、COM | Deferred GDI／MCI comparison |

Static import に現れない dependency があるため、この表だけで support task を閉じない。特にしゅがてん！の Direct3D／DirectSound と、各 title の COM method call は dynamic inventory が必要である。

## 5. Primary target facts

今回観測したサクラノ詩 `BGI.exe` の PE／dependency facts は次のとおり。

- PE32／i386、GUI subsystem 5.1、image base `0x00400000`
- 5 section、entry RVA `0x000A383E`、image size 1,556,480 bytes
- base relocation あり、`DYNAMIC_BASE`、`NX_COMPAT`
- static TLS、delay import、CLR header はなし
- binary string 上の runtime 表記は `Ethornell - BURIKO General Interpreter ( Version : 1.614.1 - Compatibility : 1.72 )`
- `Direct3DCreate9` を静的 import する。以後の D3D9 呼び出しは COM vtable 経由なので、必要 method は import table から判定できない
- `DirectInput8Create` を静的 import する
- DirectSound は ordinal で 1 symbol を importし、Windows startup traceでは `DirectSoundCreate8` に到達した。利用interfaceとmethodは未確定
- GDI は DIB、font、glyph、palette、text、clip、`StretchDIBits` を含む
- GDI+ は bitmap load／save、DIB conversion、LockBits／UnlockBits を含む 16 symbol を import する
- WinMM は timer、`PlaySoundA`、`mciSendCommandA` を含む
- Kernel32 は thread、wait、TLS、file mapping、file I/O、process creation、virtual memory を含む 146 symbol を import する
- title root 直下には `BGI.gdb`、`BGI.hvl` と 48 個の `.arc` がある。個別 asset decoder は作らず、guest file I/O と engine code に処理させる

60秒 startup traceでは main window、`Direct3DCreate9`、`DirectInput8Create`、`DirectSoundCreate8`、`GdiplusStartup`、title-local archive loadに到達した。`d3dx9_43.dll` と `D3DXCreateEffect` の動的解決も観測した。WinMM moduleはloadされたが、hook対象entryは未到達だった。詳細とattribution上の制約は [windows-native-evidence.md](windows-native-evidence.md) を参照する。

`WININET.dll` も静的 importされ、process module setにも現れたが、現traceはcallerを記録しないためBGIからのruntime callとは未確定である。静的に存在する、またはprocess内でloadされただけのAPIをcompletionの必須実装へ昇格させず、caller付きruntime call evidenceで判断する。

## 6. Corpus priority

| Priority | Target | Purpose | Gate |
|---|---|---|---|
| 1 | Synthetic T0／T1／T2 | CPU、PE、ABI、Win32、graphics、audio を title から分離して検証 | 各 phase acceptance |
| 2 | サクラノ詩 | Active personal completion target | `AC-P7-DP` |
| 3 | Nursery Rhyme | 同系統 runtime の DirectDraw path と共通 Win32 behavior を比較 | 非 blocking checkpoint |
| 4 | PURIPA | 別 title の DirectDraw／DirectSound behavior を比較 | Deferred |
| 5 | 残り 4 title | D3D9、video、dynamic resolution、GDI／MCI の将来 gap を観測 | Deferred |

## 7. 次に取得する evidence

1. manual marker／screenshot付きでサクラノ詩のvisual title checkpointを固定する。
2. caller module＋RVAとcall ID付きtraceでBGI direct callとOS／driver内部callを分ける。
3. D3D9 の interface version、CreateDevice parameters、呼び出す vtable method、format、render state、vertex path、Reset／device-lost pathを記録する。
4. DirectInput 8 の device type、data format、cooperative level、Acquire／GetDeviceState／GetDeviceData の実利用を記録する。
5. DirectSound、WinMM／MCI、GDI+、COM class の primary scenario method到達性を記録する。
6. executed instruction／CPU featureとthread／wait patternをPhase 1 workloadへ落とす。
7. file access／share／disposition／last errorとregistry accessをsource volume／save overlayの設計入力へ落とす。

既存の local trace／decompiler output は調査材料として利用できるが、Vine の acceptance evidence にする場合は取得条件、対象 hash、開始／終了 checkpoint、trace schema を固定して再取得する。
