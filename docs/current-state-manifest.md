# Current state manifest

## 目的

Vineの機械可読な現在状態は`docs/current-checkpoint.json`だけで更新する。ルートREADMEの
Current Status、supported API contract一覧、canonical再現commandはmanifestから生成し、個別の
evidence文書へcurrent値を手作業で転載しない。

過去のreport hashやnative observationはartifact identityとしてmanifest内に保持できる。ただし、
full JSON byte identityはsemantic gateではない。semantic testとstructured stopが回帰判定の正本である。

## Schema version 2

| Group | Authority |
|---|---|
| `historicalBaseline` | frozen migration baselineと対応するcheckpoint manifest |
| `memoryAuthorityEvidence` | Rust byte authority、binary transport、transaction evidence |
| `browserCheckpoint` | controlled Browser Worker state。BGI workloadの状態を別フィールドで明示する |
| `engineDecisionEvidence` | v86 adapterを継続する判断と、その測定artifact |
| `activeRuntimeCheckpoint` | current runtime profile、capability一覧、latest semantic evidence |
| `nextStructuredStop` | 次のunsupported API、argument、instruction、reason |

root直下へ旧schemaの`checkpoint`、`runtimeKernelCheckpoint`、`nextCheckpoint`を再追加しない。
historical、active、browserを同じ意味のcurrent checkpointとして扱わない。

## 更新手順

1. `docs/current-checkpoint.json`の該当groupだけを更新する。
2. `npm run docs:generate`でREADMEのmarker区間を再生成する。
3. `npm run check:docs`でschema、cross-field count、path、READMEの一致を検査する。
4. manifestが生成した`canonicalReproductionCommand`を実行し、semantic outcomeとnext stopを確認する。

現在のgeneric BGI commandは`check:runtime:bgi`一つであり、`--profile`はmanifestのactive profileを
選ぶ。API固有のscript群はhistorical compatibility entry pointとして残るが、current stateの
再現commandには使用しない。

## Browser non-claim

`browserCheckpoint.status`は`browserCheckpoint.workload`にだけ適用する。
controlled public fixtureの`passed`をBGI Browser executionの成功として扱わない。
`browserCheckpoint.bgiSession`が明示的に更新されるまで、BGI Browser checkpointは未開始である。
