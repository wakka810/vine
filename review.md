# Review

1. **Address allocation identity** — 完了（`0e5b176`）
   mappingのowner／range／lifetimeをallocation IDで一貫管理する。

2. **Current-state manifest** — 完了（`0860bd0`）
   current値を`docs/current-checkpoint.json`へ集約し、重複・陳腐化をCIで検出する。

3. **COM ABI／IUnknown foundation** — 完了
   x86 COM ABIとRust-owned identity／refcountを実装し、browser resourceから分離する。
