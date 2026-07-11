# PE32 loader ingress

## 1. 文書情報

- ステータス: Verified checkpoint
- 取得日: 2026-07-11
- checkpoint: `W7` production-oriented PE32 load-plan ingress
- schema: `vine.pe32-loader-ingress` version `1`
- primary fingerprint: `f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`

このcheckpointは、immutableなPE32 sourceから、header／section、data directory、import、base relocationを所有するchecked load planをRustで作り、実BGI.exeを最初のprocess bootstrap blockerまで進める。guest memoryへのmaterialize、IAT binding、PEB／TEB、initial stack、CPU実行は行わない。静的planの成功をtitle起動またはproduction loader完成とは扱わない。

## 2. Implementation boundary

root Cargo workspaceはRust `1.97.0`、edition 2024、resolver 3を固定する。`vine-core`がimmutable module snapshotとPE32 load planを所有し、`vine-pe-probe`はbounded file read、expected SHA-256、deterministic JSON outputだけを提供する。parser dependencyは`goblin 0.10.7`の`std`／`pe32`／`endian_fd` featureだけを有効にし、PE64／ELF／Mach-O parserをproduction dependency graphへ入れない。

`ModuleSnapshot`はfileを一度だけowned bytesへ取り込み、import時のSHA-256を保持する。load plan作成時に同じowned bytesを再hashする。path、file handle、external backing providerはplanへ保持せず、section copy／zero-fill／relocation／IAT操作はsource bytesを変更しない。

probeの`--output`はcreate-newだけを許可し、既存reportまたは誤指定したsource fileを上書きしない。再取得時は新しい`.work/` pathを指定する。

PE headerとsection tableはstrict structured parserで読み、loader側で次を追加検査する。

- PE32／i386、executable image、32-bit flag、224-byte optional header
- file／section alignment、64 KiB image base、image／header／stack／heap range
- section table、raw range、mapped range、RVA順、raw／virtual overlap
- entry pointがfile-backed executable sectionにあること
- 16 data directoryのaddress kind、mapped／file-backed range
- bounded ASCII import module／symbol、descriptor／thunk terminator、IAT span
- `ABSOLUTE`と`HIGHLOW`だけのbounded base relocation block／target
- PE32+、CLR、TLS bootstrap、delay import、未知relocation typeのstructured rejection

Valid low-alignment images with `SectionAlignment < 0x1000` and matching
`FileAlignment` are not malformed, but the 4 KiB `GuestMemory` materializer
cannot represent their sub-page layout yet. Planning rejects them early with
`image-layout / low-section-alignment-materialization-unimplemented` and
includes both alignment values and the 4 KiB backing limitation. Mismatched,
zero, non-power-of-two, or `SectionAlignment < FileAlignment` cases remain
structured invalid input. The synthetic contract covers `0x200/0x200`,
`0x400/0x400`, normal `0x200/0x1000`, and malformed variants.

IATはloader phaseで書いた後にsectionのfinal protectionを適用する。BGIとcontrolled fixtureはいずれもIATをfinal read-only `.rdata`に置くため、final sectionがwritableであることは要求しない。planはoriginal IAT valueと各slot RVAを保持するが、runtime thunk addressを成功した値として書かない。

### 2.1 Resource limits

| Limit | Value |
|---|---:|
| Source file | 256 MiB |
| Mapped image | 512 MiB |
| Sections | 96 |
| Import modules | 512 |
| Imports | 65,536 |
| Import string | 1,024 bytes |
| Relocation blocks | 65,536 |
| Relocation words | 1,000,000 |

これらはこのcheckpointのfail-closed上限であり、browser memory budgetまたは最終compatibility policyの決定ではない。

## 3. Representative fixture contract

`npm run check:toolchain`で再現生成する`abi-smoke.exe`を唯一のrepresentative automated contractに使う。

| Field | Result |
|---|---|
| Source | 2,560 bytes、SHA-256 `60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7` |
| Image | PE32／i386、base `0x00400000`、entry RVA `0x1000` |
| Sections | 4 |
| Imports | 1 module、2 named symbols |
| Relocations | 1 block、6 words、1 `ABSOLUTE`、5 `HIGHLOW` |
| Stop | `process-bootstrap`／`peb-teb-process-parameters-unimplemented` |

contractはsource fingerprint、image、section数、`vine_fixture.dll!fixture_add`／`fixture_call_callback`、relocation count、structured stop、同じreport objectのdeterministic JSONを検査する。W1のCPU／callback contractを重複実行せず、今回のparse／plan境界だけを代表する。

## 4. Primary BGI result

再現command:

```powershell
npm run check:toolchain
npm run check:runtime:pe
npm run probe:runtime:pe -- `
  --expect-sha256 f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1 `
  --output .work\pe-loader\bgi.json `
  "V:\softs\サクラノ詩\BGI.exe"
```

| Field | Result |
|---|---|
| Source | 1,193,472 bytes、expected SHA-256 matched before both runs |
| Image | PE32／i386 GUI、base `0x00400000`、entry RVA `0x000A383E`、image `0x17C000` |
| Sections | 5。`.text` RX、`.rdata` R、`.data` RW、`.rsrc` R、`.reloc` R／discardable |
| Directories | export、import、resource、base relocation、IAT |
| Imports | 14 modules、330 symbols、329 named、1 ordinal |
| Ordinal | `DSOUND.dll` ordinal `11`、IAT RVA `0x000D1058` |
| Relocations | 237 blocks、18,308 words、109 `ABSOLUTE`、18,199 `HIGHLOW` |
| Features absent | exception、certificate、debug、architecture、global pointer、TLS、load config、bound import、delay import、CLR |
| Stop | `process-bootstrap`／`peb-teb-process-parameters-unimplemented` |
| Local report | 1,976,261 bytes、SHA-256 `692852dc3bdbb13e39202138e725a8c6a58a20ea1d3d9e5c20c5e21b33f658be` |

同じcommandを別outputへ2回実行し、report bytes／SHA-256が一致した。`llvm-readobj --coff-imports`の独立countも14 modules／330 symbolsで一致した。実行後のBGI.exeも元のsize／SHA-256を保持した。full report、game bytes、absolute source pathは`.work/`からGitへ追加しない。

planは5 sectionのcopy／alignment padding／zero-fill、全IAT slot、全`HIGHLOW` targetをowned metadataとして保持する。最初のdescriptorは`d3d9.dll!Direct3DCreate9`であるが、static import orderをruntime実装順にしない。native traceで到達済みのfactory／APIを引き続き優先する。

## 5. Structured stop and non-claims

valid planの次に返すstopは次である。

```json
{
  "stage": "process-bootstrap",
  "code": "peb-teb-process-parameters-unimplemented"
}
```

これは「all imports supported」または「image mapped」を意味しない。stop messageはguest memory materialization、import binding、PEB／TEB／process parameters、initial stack、entry executionをすべてnot attemptedとして記録する。silent API stub、title hash／EIPによるskip、source patchはない。

このcheckpointは`REQ-IMP-07`、`REQ-PE-01`／`REQ-PE-02`／`REQ-PE-03`／`REQ-PE-06`、`P2-PE-01`／`P2-PE-02`／`P2-PE-03`／`P2-PE-04`／`P2-PE-10`、`P2-DIAG-01`、`P2-COR-01`へのpartial evidenceである。`cargo-fuzz` targetはarbitrary bytesを`ModuleSnapshot`とplannerへ通すが、長時間campaign／curated malformed corpusは未完了である。DLL graph／export resolution／forwarder／resource parser／TLS／delay import／module lifecycle／API binding／bootstrap／executionも未完了のため、`AC-P2-01`、`AC-P2-05`、`AC-P2-06`、`AC-P2-09`、`AC-P2-10`、`AC-P2-11`を含むacceptance criterionは一つも完了しない。

このowned planをmemory ownerへ接続するcheckpoint、controlled entry／bootstrap reflection、BGI full trap binding、large-image transfer／first runtime trapは[v86-bgi-first-trap.md](v86-bgi-first-trap.md)まで進んだ。このload-plan report自体のstopとnon-claimsは変更しない。
