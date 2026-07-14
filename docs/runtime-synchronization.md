# Runtime synchronization subsystem

## Status

`crates/vine-core/src/runtime/synchronization.rs`の`RuntimeSynchronization`を、process-owned
critical-section collectionのRust state ownerとする。これは`review.md` 3.2に従う責務分割であり、
Win32 capability、supported variant、guest-visible resultは追加しない。

## Ownership boundary

`RuntimeSynchronization`だけがprivate `critical_sections` mapを保持する。公開
`RuntimeProcessState`はこのcollectionをmutable fieldとして公開しない。既存report consumerとの
互換性のため、serialized process shapeでは引き続き`criticalSections` fieldへflattenする。このJSON
shapeはcurrent diagnostic representationであり、persistent snapshot ABIではない。

`RuntimeState` aggregate rootに残す責務は次だけである。

- process lifecycleが`running`であることを確認する
- selected thread lifecycleが`running`であることを確認する
- scheduler laneが`runnable`または`in-callback`であることを確認する
- current thread IDをsubsystem planへ渡す
- 全preflight成功後にsubsystem commitを呼ぶ

address、duplicate、capacity、owner、recursion、contentionのvalidationとstate transitionは
`RuntimeSynchronization::plan_*`がowned stateのcloneだけへ適用する。planはcollectionを変更せず、
`commit`だけが1 recordを置換する。したがってlifecycle、Win32 spin-count、subsystem transitionの
いずれかが失敗してもaggregate stateは変化しない。

## Preserved behavior

- maximum live critical sections: `4096`
- storage identity: nonzero 4-byte-aligned guest address
- initialize: unowned、recursion `0`
- enter: current thread ownership、checked recursive increment
- leave: current ownerだけがdecrementし、zeroでownerをclear
- contention: `critical-section-contended`をmutation前に返す

Blocking contention、delete、wait integration、arbitrary native raw stateは追加していない。

## Regression

```powershell
cargo test -p vine-core runtime::synchronization::tests --locked
cargo test -p vine-core --test runtime_synchronization --locked
cargo test -p vine-core --test api_dispatcher critical_section_contract --locked
```

Subsystem unit testsはplanのmutation-free性、recursive transition、rejected transitionのstate不変を
固定する。Integration testはaggregate lifecycle rejection、invalid spin-count、second-thread contention、
serialized field compatibilityを検査する。Existing dispatcher testはGuestMemory bytes、LastError、ABI、
recursive owner、contentionのend-to-end behaviorを保持する。

Current private regressionはNode／Chromeで268 calls、instruction `22,843`、final memory digest、
four initial-stack growths、next `MultiByteToWideChar` stopを変更前と同一に保つ。新しいRust/Wasm
artifactを使う180秒gateは745 sessions／failure 0で17 thresholdをすべて通した。
