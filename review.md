# Vine Architecture Review

## 役割

本書は、現在残る構造課題だけを記録する。進捗値、API実装順、historical evidenceは
`docs/current-checkpoint.json`と各正本文書を参照する。

## 1. Current stateを一つのmanifestへ集約する

call数、next stop、generation、report identityの手書き複製が残り、現行文書が既に陳腐化している。

完了条件:

- 変動するcurrent値は`docs/current-checkpoint.json`だけで更新する。
- READMEと現行文書のstatusは生成blockまたはmanifest参照にする。
- historical文書を明示的に固定し、`check:docs`がcurrent値の重複とstale記述を検出する。

## 2. COM ABI／IUnknown foundationを作る

Native traceでD3D9 COM到達済みだが、共通ABIとidentity／lifetime基盤がない。Graphics backendや
device実装には広げず、共通foundationだけを作る。

完了条件:

- x86 COM stdcall vtable、HRESULT、stack cleanupをsynthetic fixtureで固定する。
- `QueryInterface`／`AddRef`／`Release`とlogical object／interface identityをRust authorityへ置く。
- TypeScript browser resourceをlogical COM identityから分離する。
- 未対応呼び出しがinterface、slot、callsite付きで構造化停止する。
