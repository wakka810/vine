# Vine Architecture Review

1. **Address allocation identity**（完了: `0e5b176`）
   mappingをallocation IDへ帰属させ、owner／range／lifetime不一致をmutation前に拒否する。

2. **Current-state manifest**（完了）
   変動値を`docs/current-checkpoint.json`へ集約し、文書の重複・陳腐化を検出する。

3. **COM ABI／IUnknown foundation**
   x86 COM ABI、Rust-owned identity／refcount、`QueryInterface`／`AddRef`／`Release`、構造化停止を実装する。
