# LLVM toolchain spike

## 1. 文書情報

- ステータス: Verified baseline。production decision は Open
- 基準日: 2026-07-10
- 対応 task: `P0-ARCH-01`、`P0-COR-03`、`P0-TOOL-01`
- canonical command: `npm run check:toolchain`

## 2. 目的

production workspace を決める前に、現在の LLVM toolchain だけで次を再現できるか確認する。

1. source から PE32／i386 EXE と DLL を生成する。
2. EXE から DLL の named import と guest callback を表現する。
3. freestanding C を Wasm32 module にして host JavaScript から実行する。
4. PE header、import、export と生成 artifact hash を機械検査する。

この spike は LLVM/C を production core に決定する ADR ではない。x86 engine 候補を同じ browser／Wasm 条件で比較するための最小 baseline である。

## 3. Current environment

| Component | Observed |
|---|---|
| Node.js | 24.18.0 |
| npm | 11.16.0 |
| LLVM／Clang | 22.1.8、x86／wasm32 target あり |
| LLD | 22.1.8、`lld-link`／`wasm-ld` あり |
| PE import library | `llvm-dlltool` 22.1.8 suite、SHA-256 `80934e8f208a0cc2a87a6057f871d0f492461952b8672464749a6c3dff34109c` |
| PE inspection | `llvm-readobj` 22.1.8 |
| Rust | 1.97.0、workspace-local、`wasm32-unknown-unknown` target あり |
| Emscripten | 未導入 |

version は環境観測値であり、採用 version の決定ではない。toolchain pin と導入方法は ADR-001 で確定する。

## 4. Fixture

`fixtures/pe32/abi-smoke/` は次の制御経路を表す。

```text
abi-smoke.exe
  -> import fixture_add from vine_fixture.dll
  -> import fixture_call_callback from vine_fixture.dll
  -> DLL calls function pointer back into EXE
  -> entry returns pass/fail
```

fixture は CRT と Windows SDK header を使わず、Clang と LLD だけで生成する。Windows では生成直後の EXE も実行し、DLL import、cdecl、DLL から EXE への callback が entry code `0` まで通ることを native oracle とする。

`fixtures/pe32/window-pixel/` はtracked DEFから`llvm-dlltool -m i386 -k`でsystem import libraryを生成し、次のnative sequenceを自己検証する。Windows SDK header／libraryは使わない。

```text
CreateWindowExA -> queued GetMessageA / DispatchMessageA
  -> __stdcall WndProc
  -> WM_CREATE -> WM_PAINT -> WM_DESTROY
  -> BeginPaint -> SetPixel(red) -> EndPaint
  -> DestroyWindow -> PostQuitMessage -> WM_QUIT -> entry returns 0
```

`fixtures/pe32/high-memory/`はimage base `0xF1000000`、LAA、importなしのPE32 fixtureである。`.text` RX、`.rdata`／`.rprobe`／`.nxprobe` R、`.data` RWを分離し、high entry、read-only write、unmapped read／write、cross-page write、non-executable fetchのprobeをexportする。Windows native oracleではなく、guest memory候補を同じfixed imageで比較するinputである。

`fixtures/pe32/bootstrap-fs/`はimportなしのPE32 fixtureであり、`FS:[0x18]`のTEB self pointerと`FS:[0x30]`のPEB pointerを読み、TEB内の同じ2 fieldと一致することを自己検証する。Windowsではnative exit code `0`を要求し、source-v86ではTEB未実装時のexact first stopを別contractで固定する。

`fixtures/pe32/bootstrap-params/`もimportなしのPE32 fixtureである。native i386の`PEB+0x10` process-parameters pointerから`+0x40` command-line `UNICODE_STRING`とbuffer `+0x04`を読み、32 KiB以内で`bootstrap-params.exe`をbounded matchする。Windowsではnative exit code `0`を要求し、source-v86ではzero guest pointerのexplicit result `2`を別contractで固定する。

## 5. Generated evidence

`npm run check:toolchain` は `.work/toolchain-spike/` を作り直し、次を検証する。

- Wasm export `vine_add(19, 23)` が `42` を返す
- EXE が `COFF-i386`／`IMAGE_FILE_MACHINE_I386` である
- EXE が `vine_fixture.dll` の 2 symbol を named import する
- DLL が同じ 2 symbol を export する
- Windows では `abi-smoke.exe` を直接実行し、exit code が `0` である
- window fixtureのPE32形式、3 DLL／13 symbol、関連message順、native exit code `0`を検証する
- high-memory fixtureのPE32／LAA、image base、section flags、named probe exportを検証し、nativeでは起動しない
- bootstrap FS／process-parameters fixturesがPE32／i386、import-free、required exportsを持ち、Windowsでexit code `0`になることを検証する
- native processを10秒で打ち切り、paintに必要な通常のwindow表示を抑止しない
- Wasm、EXE、DLL、import library、window／high-memory／bootstrap fixtures の size と SHA-256 を `report.json` に記録する

`.work/` は生成物なので Git 管理しない。

2026-07-10／11 の実行結果:

| Artifact | Size | SHA-256 |
|---|---:|---|
| `wasm-smoke.wasm` | 399 bytes | `d8e2fee03819c64707910bd2fa529ab9aa8319b6742a5556c00293f91b9feaf8` |
| `abi-smoke.exe` | 2,560 bytes | `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| `vine_fixture.dll` | 2,048 bytes | `a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878` |
| `vine_fixture.lib` | 1,708 bytes | `ebb025e3a0c82f6726331aa8799b7a5fdafd4b2dfa3b3dcdce19d0630075c031` |
| `window-pixel.exe` | 4,096 bytes | `ad8f4a0bddd759ac52b18119e60838c3ad13d29994449f38b0393fdedde454e9` |
| `high-memory.exe` | 3,072 bytes | `084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7` |
| `bootstrap-fs.exe` | 2,560 bytes | `5712377ff8a8bd2be292f8678902d19d3b4b2f1c1919922cce9d373f0696637c` |
| `bootstrap-params.exe` | 2,560 bytes | `50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40` |

旧文書のWasm値は319 bytes／`6e5218...`だったが、そのraw artifactは保持されておらず差の原因を再検証できない。現在のWindows canonical commandを複数回実行した結果は399 bytes／`d8e2fe...`で安定しているため、本表で旧観測を置き換える。source変化やcompiler regressionが確認できたという意味にはしない。

## 6. Interpretation

今回の実行により、次を Phase 0 の事実として扱える。

- T0／T1 PE32 fixture とSDK非依存のi386 system import libraryをsource／DEFから再現生成できる
- Windows native で EXE -> DLL -> EXE callback の cdecl oracle が exit code `0` になる
- Windows native でstdcall WndProc、queued message dispatch、paint、pixel、destroy／quit oracleがexit code `0`になる
- Windows native でimport-free bootstrap fixtureのTEB self／PEB pointer oracleがexit code `0`になる
- Windows native でseparate import-free fixtureのprocess-parameters／bounded command-line oracleがexit code `0`になる
- C／C++ x86 engine 候補を freestanding Wasm へ移すための最低限の compiler／linker path がある
- PE fixture build は production browser frontend の framework 選定と分離できる

x86 engine の比較と v86 adapter smoke は [x86-engine-scorecard.md](x86-engine-scorecard.md) に進んだ。production memory／bootstrap、BGI full trap binding／owner seal、389-page dynamic-endpoint mode、first seventeen Kernel32 returns、automatic binary writeback、controlled Chrome Runtime Worker vertical slice、version 1 browser engine gate、typed object／handle、process-owned module graph foundationsまで進み、v86継続`go`となった。BGI continuationは凍結し、次はprocess／thread lifecycle foundationである。詳細は[runtime-module-graph.md](runtime-module-graph.md)、[runtime-object-model.md](runtime-object-model.md)、[v86-browser-benchmark.md](v86-browser-benchmark.md)、[browser-runtime-worker.md](browser-runtime-worker.md)を正とする。
