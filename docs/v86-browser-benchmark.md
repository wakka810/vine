# v86 browser benchmark and engine gate

## Decision

2026-07-13のreference Windows desktop／stable Chrome測定は`go`である。v86を
交換可能なbaseline interpreterとして継続し、Step 9のloader／scheduler／object
modelへ進む。no-go triggerがないため、このcheckpointでは代替engineを測定せず、
独自interpreterまたは広いengine比較を開始しない。

この判断はv86を最終production engineとして固定するものではない。adopted ISA／ABI
suite、general process／thread／wait、title browser workload、long-play audio／graphicsは
別gateとして残る。

## Reproduction

Configurationは
[`fixtures/benchmarks/v86-browser-v1.json`](../fixtures/benchmarks/v86-browser-v1.json)
のschema version 1を正とする。

```powershell
npm run check:runtime:v86-benchmark
```

adapterとfixtureを準備済みの場合:

```powershell
npm run check:runtime:v86-benchmark:prepared
```

runnerはclean temporary Chrome profile、loopback server、headless stable Chromeを使う。
foreground suite後もRust runtime core＋v86 mirrorのproduction sessionをWorker内に保持し、
その状態でsteady memoryを測る。別tabをactivateしてoriginal pageが`hidden`であることを
確認してから、同じWorkerで3秒のbackground throughputを測る。

## Authoritative environment

| Field | Value |
|---|---|
| Source commit | `b1a393cd1e0b1be79774f64fd591f7eb20488e03` |
| Dirty | `true` |
| Host | Windows `10.0.26200` x64 |
| CPU | AMD Ryzen 7 8840U with Radeon 780M Graphics |
| Logical CPUs | 16 |
| Host memory | 16,398,209,024 bytes |
| Chrome | `150.0.7871.115` |
| Node.js | `v24.18.0` |
| Measured report | 19,866 bytes |
| Report SHA-256 | `22631423fad2d91cf9c6fe250e7d4312c518547130bbc9b5f14e09c4e31814f9` |

Tracked raw binaries are not added. The local report is
`.work/v86-browser-benchmark/report.json` and the screenshot is adjacent to it.
The source is dirty because this is the pre-commit checkpoint measurement. The
report hash identifies that capture; `go` comes from the versioned thresholds,
and whole-report identity is not a production ABI.

## Artifact identity

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| v86 adapter Wasm | 1,882,836 | `f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26` |
| Rust runtime core Wasm | 1,519,513 | `479faa4d393c9fd21aa68fec8ab063e378ac6bb43907e7c54753404c07de830b` |
| `browser-vertical.exe` | 2,560 | `24c98ec43bc76dcd0c1ddbbbf68c2ea1d86207bc2ebea2aea536a4757bf4189f` |
| benchmark Worker bundle | 62,001 | `41a393557afb01f193da95710544b241de782a5e9e41df426feaf9f8b9e0ae30` |
| benchmark main bundle | 13,440 | `24e960025815ef3743da045ae5f8ef8a60481ccbd37ff1fbff1d6eeae94791b9` |
| version 1 config | 1,123 | `c4d0c3f8927943658b063fe0ba0fc4c998fdd2f09f8a3fc1c9425e4b0c942322` |

## Gate result

全thresholdはauthoritative runより前のclean benchmark commitに固定した。

| Metric | Go threshold | Result |
|---|---:|---:|
| Integer instructions/s | >= 1,000,000 | 138,619,074 |
| x87 representative instructions/s | >= 250,000 | 52,245,387 |
| SSE2 representative instructions/s | >= 500,000 | 81,434,038 |
| Adapter-only API traps/s | >= 2,000 | 309,119 |
| Production API roundtrips/s | >= 1,000 | 11,875 |
| Callback roundtrip | <= 1,000 us | 8.350 us |
| Context save＋restore | <= 100 us | 1.159 us |
| 1 ms slice relative throughput | >= 0.50 | 1.001 |
| 5 ms slice relative throughput | >= 0.75 | 1.011 |
| 10 ms slice relative throughput | >= 0.85 | 1.003 |
| 16 ms slice relative throughput | >= 0.90 | 1.000 |
| 256-page reflection | >= 100 MiB/s | 1,087.0 MiB/s |
| 128-page mirror writeback | >= 100 MiB/s | 885.0 MiB/s |
| Production one-page writeback | >= 10 MiB/s | 71.023 MiB/s |
| Code／TLB invalidation | <= 50 us | 0.074 us |
| Worker cold start p95 | <= 1,000 ms | 36.760 ms |
| Active steady memory | <= 268,435,456 bytes | 14,773,637 bytes |
| 30-minute heap slope | <= 1,048,576 bytes/min | 136.214 bytes/min |
| Hidden-tab throughput ratio | >= 0.50 | 0.924 |
| AudioWorklet delivery gap | <= 100 ms | 51.985 ms |
| Production Wasm exports/API roundtrip | <= 20 | 20 |

Production API roundtripは101 fresh sessionsでfirst owned thunkまでCPU実行し、stackの
automatic writebackをRustへreconcileした後、Rust generic dispatch、scheduler host
request、`completeHost(42)`、v86 CPU state resumeだけをtimed regionにする。結果は
median 65 us、p95 110 usである。20 crossingsはRuntimeBridgeの2 operationsに含まれる
14 exportsとadapter state resumeの6 exportsである。adapter-only trapは別に14
crossings／3.235 usを測る。

Active steady memoryは`measureUserAgentSpecificMemory()`で測定し、pre-suite 537,939
bytesから14,773,637 bytesとなった。これはunreferenced Wasm instanceを回収したidle
page値ではなく、production Rust＋v86 sessionを保持した値である。

AudioWorkletは678 delivery messages、48 kHz、frame discontinuity 0だった。cold startは
7 samples、median 34.060 msである。30-minute slopeは同じruntime core／adapter artifact
を使ったStep 7の11,326-session、failure 0、report SHA-256
`13a7b579bb13841aa073de4dcf5ed7db0743485810e12b3159272205fbb43bb3`
を再利用し、benchmark時に30分soakを重複実行しない。

## Private title checkpoint

private BGI fileが存在するreference hostでは、generic seventeenth-call runnerも同時に
実行した。238 instructions／17 dispatched callsは698.059 ms、report SHA-256は
`ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`、
reflection SHA-256は
`0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
で既存semantic evidenceを維持した。reportにはactual artifact identityとcapture-time
baselineのmatch observationを分けて保存し、whole-report mismatchをsemantic failureにしない。
private fileがないCIでは理由付きskipとし、go
thresholdの入力には使わない。

## CI policy

GitHub Actionsはpublic fixture E2E後にprepared benchmarkを実行する。`go`だけがexit 0
であり、`conditional-go`と`no-go`はいずれもregression failureである。report、PNG、
toolchain report、Worker semantic report、browser build manifestは`if: always()`
artifactとして保存する。reference Windows値は
decision evidence、Linux stable Chrome値はportable floor regressionとして扱い、両者の
raw throughput equalityは要求しない。

## Non-claims

- synthetic integer／x87／SSE2 loopはadopted ISA correctness suiteではない。
- 238-instruction BGI checkpointはtitle workload throughputまたはfull-play evidenceではない。
- current stable Chromeの一つのdesktopだけをreferenceとし、Firefox／mobile Tierを決めない。
- AudioWorklet delivery message gapはproduction mixer、long-play drift、underrunを代用しない。
- current synthetic threshold passだけではsnapshot、general heap／stack、process／thread／wait、
  DLL lifecycle、graphics／audio backend、BGI browser executionを成立扱いにしない。BGI executionと
  workload performanceは後続の独立gateで判定する。
