# Runtime synchronization subsystem

## Status

`crates/vine-core/src/runtime/synchronization.rs`の`RuntimeSynchronization`を、process-owned
critical-section／mutex collectionのRust state ownerとする。最初の分割checkpointでは
critical-section behaviorを移動するだけだった。Reached `CreateMutexA` checkpointは同じprivate
boundaryへnamed mutex identity、owner、recursion、lifetimeを追加し、generic object／handle tableとの
cross-subsystem transactionを固定した。

## Ownership boundary

`RuntimeSynchronization`だけがprivate `critical_sections`／`mutexes` mapとnamed-mutex indexを保持する。公開
`RuntimeProcessState`はこのcollectionをmutable fieldとして公開しない。既存report consumerとの
互換性のため、serialized process shapeでは`criticalSections`／`mutexes` fieldへflattenする。このJSON
shapeはcurrent diagnostic representationであり、persistent snapshot ABIではない。

`RuntimeKernel` aggregate root（既存caller向けaliasは`RuntimeState`）に残す責務は次だけである。

- process lifecycleが`running`であることを確認する
- selected thread lifecycleが`running`であることを確認する
- scheduler laneが`runnable`または`in-callback`であることを確認する
- current thread IDをsubsystem planへ渡す
- 全preflight成功後にsubsystem commitを呼ぶ

address、duplicate、capacity、owner、recursion、contentionのvalidationとstate transitionは
`RuntimeSynchronization::plan_*`がowned stateのcloneだけへ適用する。planはcollectionを変更せず、
`commit`だけが1 recordを置換する。したがってlifecycle、Win32 spin-count、subsystem transitionの
いずれかが失敗してもaggregate stateは変化しない。

Mutex create／open／last-closeはobject tableも変更する。Object subsystemはprevious／next stateを閉じ込めた
opaque `RuntimeMutexObjectMutationPlan`を返し、stale commitをmutation前に拒否する。Aggregate coordinatorは
object／synchronization collectionを参照・clone・置換せず、両方のplanとcapacity／lifecycle preflightが
成功した後だけ各ownerのcommitをsame operationで呼ぶ。Synchronization subsystemはhandle slotやobject
refcountを直接mutateせず、object tableはname、owner、recursionを保持しない。

## Preserved behavior

- maximum live critical sections: `4096`
- storage identity: nonzero 4-byte-aligned guest address
- initialize: unowned、recursion `0`
- enter: current thread ownership、checked recursive increment
- leave: current ownerだけがdecrementし、zeroでownerをclear
- contention: `critical-section-contended`をmutation前に返す
- maximum live mutexes: `4096`
- named identity: owned ANSI bytes plus Windows-932 UTF-16 key
- create: new typed object／type-tag `0x58` handle; optional current-thread initial ownership
- existing name: same objectへdistinct handle、initial-owner requestは無視
- wait: unowned acquire／same-thread recursive acquire; cross-thread contentionはmutation-free stop
- release: current ownerだけがchecked decrementし、zeroでownerをclear
- close／teardown: remaining handleはobjectを保持し、last close／process exitでrecordとnameをretire

Blocking contention、abandonment、guest wait／release／close contract、critical-section delete、
arbitrary native raw stateは追加していない。

## Regression

```powershell
cargo test -p vine-core runtime::synchronization::tests --locked
cargo test -p vine-core --test runtime_synchronization --locked
cargo test -p vine-core --test runtime_mutex --locked
cargo test -p vine-core --test api_dispatcher critical_section_contract --locked
cargo test -p vine-core --test api_dispatcher create_mutex_a --locked
```

Subsystem unit testsはplanのmutation-free性、recursive transition、rejected transitionのstate不変を
固定する。Integration testsはaggregate lifecycle rejection、invalid spin-count、second-thread contention、
named identity copy、distinct-handle shared object、recursive mutex wait／release、last-close recreation、
process teardown、serialized field compatibilityを検査する。Dispatcher testsはGuestMemory bytes、
LastError、ABI、policy rejection、critical-section and mutex ownership behaviorを保持する。

Primary targetはbounded startupの後半で4,096件のlive critical sectionを保持し、次の
`InitializeCriticalSection`を`critical-section-limit-exceeded`としてmutation前に停止できる。この観測は
limit enforcementとfailure atomicityのevidenceであり、4,096がprimary scenarioに十分なcapacityであることを
意味しない。Active occurrence、instruction、artifact identity、次stopは
[current-checkpoint.json](current-checkpoint.json)だけを正とし、capacity変更は独立checkpointで検証する。
