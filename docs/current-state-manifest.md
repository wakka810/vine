# Current state manifest

<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [`current-checkpoint.json`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
<!-- END CURRENT CHECKPOINT REFERENCE -->

## 目的

Vineの機械可読な現在状態は`docs/current-checkpoint.json`だけで更新する。ルートREADMEの
Current Status、supported API contract一覧、canonical再現commandはmanifestから生成し、個別の
evidence文書へcurrent値を手作業で転載しない。

過去のreport hashやnative observationはartifact identityとしてmanifest内に保持できる。ただし、
full JSON byte identityはsemantic gateではない。semantic testとstructured stopが回帰判定の正本である。
具体的な分類とordering／performance contractは
[evidence-classification.md](evidence-classification.md)を正とする。

## Schema version 2

| Group | Authority |
|---|---|
| `historicalBaseline` | frozen migration baselineと対応するcheckpoint manifest |
| `memoryAuthorityEvidence` | Rust byte authority、binary transport、transaction evidence |
| `browserCheckpoint` | controlled Browser Worker state。BGI workloadの状態を別フィールドで明示する |
| `engineDecisionEvidence` | v86 adapterを継続する判断と、その測定artifact |
| `activeRuntimeCheckpoint` | current runtime profile、capability一覧、generic `latestEvidence` |
| `nextStructuredStop` | `api-endpoint`またはendpoint-null `execution-fault`のdiscriminated union、instruction、reason |

root直下へ旧schemaの`checkpoint`、`runtimeKernelCheckpoint`、`nextCheckpoint`を再追加しない。
historical、active、browserを同じ意味のcurrent checkpointとして扱わない。

## 更新手順

1. `docs/current-checkpoint.json`の該当groupだけを更新する。
2. `npm run docs:generate`でREADMEのmarker区間を再生成する。
3. `npm run check:docs`でschema、cross-field count、path、READMEの一致を検査する。
4. manifestが生成した`canonicalReproductionCommand`を実行し、semantic outcomeとnext stopを確認する。

現行のcanonical commandはmanifestから生成したルートREADMEだけに表示する。そのcommandはNode
semantic oracleを再生成した後、同じprofileをChrome Runtime Workerで実行して比較する。
`check:runtime:bgi`は引数なしでmanifestのactive profile／policy／latest report directoryを選ぶ。
Historical reproductionは同じcommandの`--checkpoint <id>`または`--profile <capability>`を使い、
API固有のnpm aliasは置かない。新しいcapability checkpointでは`latestEvidence`、profile、policy、
API一覧、next stopをmanifest内で更新し、runnerやBrowser oracle pathへcurrent endpoint／fault値を
複製しない。Execution fault branchはEIP、previous EIP、fault address、error codeを保持する。

## Browser non-claim

`browserCheckpoint.status`は`browserCheckpoint.workload`にだけ適用する。
controlled public fixtureの`passed`をBGI Browser executionの成功として扱わない。現在の
`browserCheckpoint.bgiSession = semantic-equivalence-passed`は、private source fingerprint、ordered
completion、最終memory digest、structured stop、cancel／watchdog teardownをChromeで確認した結果である。
`browserCheckpoint.bgiBenchmark`は別のversioned 3分 workload gateのoutcomeである。
`threshold-passed`はsession／slice分布、main／Worker memory slope、linear memory、Wasm crossingの
全threshold合格、`threshold-blocked`はfailure IDを伴う未完了checkpointを表す。Versioned BGI
workloadはtimer clamp非依存のWorker yieldを使う。どちらのstatusも
full play、title output、long-idle steady memoryを含めない。

`browserCheckpoint.executionEngineProfile`は共通source revision／CPU-context ABIとhost別Wasm
artifact allowlistの正本を指す。build manifest、browser report、private evidenceはallowlist全体を
実行identityにせず、実際に一致した単一artifactのbytes／SHA-256を共通identityと組み合わせて記録する。

## 文書分類と検証

- ルートREADMEのgenerated blockだけがmanifestの変動値を展開する。
- 現行の基準文書は同一のcurrent-checkpoint reference blockを一つだけ持ち、変動値を本文へ複製しない。
- `bgi-*`、`v86-*`、`w2-*`、`w3-*`とWindows native evidenceはcapture時点で凍結する。現行文書に残す
  checkpoint chronologyは`FROZEN HISTORICAL EVIDENCE` markerで囲う。

`tools/current-state-documents.mjs`はreference blockの欠落・改変、現行値tuple、generation、report
identity、next stopの手書き複製、frozen markerの不整合を検出する。Negative contract testsを含めて
`npm run check:docs`から常時実行する。
