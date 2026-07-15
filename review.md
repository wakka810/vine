# Vine Architecture Review

## 役割

本書は、現在残る構造課題だけを記録する。進捗値、API実装順、historical evidenceは
`docs/current-checkpoint.json`と各正本文書を参照する。

## 1. Address allocation identityをmappingまで貫通させる

`ProcessAddressSpaceAllocator`は存在するが、既存control arenaとprocess environmentの
mapping commitはallocation ID駆動になっていない。

完了条件:

- import thunk、callback return、dynamic export、process environmentのmapping planが
  allocation recordを受け取る。
- reserve／commit／protect／decommit／releaseがallocation IDへ帰属する。
- owner、range、lifetime不一致をmutation前に拒否する代表testがある。

## 2. Current stateを一つのmanifestへ集約する

call数、next stop、generation、report identityの手書き複製が残り、現行文書が既に陳腐化している。

完了条件:

- 変動するcurrent値は`docs/current-checkpoint.json`だけで更新する。
- READMEと現行文書のstatusは生成blockまたはmanifest参照にする。
- historical文書を明示的に固定し、`check:docs`がcurrent値の重複とstale記述を検出する。

## 3. COM ABI／IUnknown foundationを作る

Native traceでD3D9 COM到達済みだが、共通ABIとidentity／lifetime基盤がない。Graphics backendや
device実装には広げず、共通foundationだけを作る。

完了条件:

- x86 COM stdcall vtable、HRESULT、stack cleanupをsynthetic fixtureで固定する。
- `QueryInterface`／`AddRef`／`Release`とlogical object／interface identityをRust authorityへ置く。
- TypeScript browser resourceをlogical COM identityから分離する。
- 未対応呼び出しがinterface、slot、callsite付きで構造化停止する。
