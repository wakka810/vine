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

fixture は CRT と Windows SDK header を使わず、Clang と LLD だけで生成する。現在の spike は PE structure の生成／検査までを扱い、Windows oracle result は Windows 環境へ移行後に同じ source／hashへ追加する。

## 5. Generated evidence

`npm run check:toolchain` は `.work/toolchain-spike/` を作り直し、次を検証する。

- Wasm export `vine_add(19, 23)` が `42` を返す
- EXE が `COFF-i386`／`IMAGE_FILE_MACHINE_I386` である
- EXE が `vine_fixture.dll` の 2 symbol を named import する
- DLL が同じ 2 symbol を export する
- Wasm、EXE、DLL、import library の size と SHA-256 を `report.json` に記録する

`.work/` は生成物なので Git 管理しない。

2026-07-10 の実行結果:

| Artifact | Size | SHA-256 |
|---|---:|---|
| `wasm-smoke.wasm` | 319 bytes | `6e52188e2ed1567154ab5a6f909c2c69d1e5818fde968c4570ed7a97ba758b28` |
| `abi-smoke.exe` | 2,560 bytes | `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| `vine_fixture.dll` | 2,048 bytes | `a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878` |
| `vine_fixture.lib` | 1,708 bytes | `ebb025e3a0c82f6726331aa8799b7a5fdafd4b2dfa3b3dcdce19d0630075c031` |

## 6. Interpretation

今回の実行により、次を Phase 0 の事実として扱える。

- T0／T1 PE32 fixture を source から再現生成できる
- C／C++ x86 engine 候補を freestanding Wasm へ移すための最低限の compiler／linker path がある
- PE fixture build は production browser frontend の framework 選定と分離できる

x86 engine の比較と v86 adapter smoke は [x86-engine-scorecard.md](x86-engine-scorecard.md) に進んだ。PC boot を迂回した integer／x87／SSE 実行に加え、Rust 1.97.0 から source を buildし、release fallback の exact instruction budget、CPL3 structured fault stop、version 1 CPU state round trip まで成立した。次は Windows native trace を取得し、その evidence に沿って memory ownership、guest callback、browser Worker を検証して ADR-001／002／003 を決める。
