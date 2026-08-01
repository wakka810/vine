# Runtime wait queue foundation

## Purpose

reviewのruntime responsibility splitとして、controlled two-thread registryのscheduler lane／event
queueをprivate `runtime/scheduler.rs::RuntimeScheduler`へ、wait registration／object FIFOをprivate
`runtime/waits.rs::RuntimeWaits`へ移した。Selected threadがnonsignaled objectへwaitを登録し、別threadの
object signalでdeterministically runnableへ戻る既存contractとserialized scheduler shapeを維持する。

Foundation checkpoint自体はguest wait APIを追加しなかった。その後のbounded
`WaitForSingleObjectEx` checkpointはwaitable timerだけを対象に、0..10 msのfinite timeout、
alertable flag、logical deadline、versioned browser pacing requestをこのboundaryへ追加した。
General wait-any／wait-all、APC、cross-thread timer wakeはまだ実装しない。

## Subsystem boundary

`RuntimeScheduler`だけがcurrent-thread selector、lane map、event FIFO、pending host-request table／ID allocatorを所有する。`RuntimeWaits`だけが
registration table、object-indexed FIFO、monotonic registration ID allocatorを所有し、`serde(flatten)` adapterで既存の
`scheduler.{currentThreadId,threads,waitRegistrations,objectWaiters,events}` shapeを維持する。Collectionは
どちらもmodule外へ公開せず、inspectionはimmutable getterだけを使う。Root
`nextWaitRegistrationId`はborrowed compatibility viewであり、aggregate-owned counterではない。

Lane registration／selection／transitionはprevious／nextを持つmutation-free scheduler plan、wait
registrationはID reservation、table、object indexを同時に扱うwait planを返す。Host start／completionはschedulerが
request identityとlane transitionを一つのplanにし、pending requestをremoveする前にcompletionを
preflightする。Aggregate coordinatorはthread、object、message、wait、host cancellation planを検査してから
各planをcommitし、scheduler／wait／host-request collectionへ`insert`／`get_mut`／`remove`しない。

## Registration model

`wait_current_thread_on(handle)`はrunning processのrunnable current threadだけを受け付ける。
handleをtyped object tableで検証し、process／thread objectのwait stateを読み、次のどちらかを返す。

- `already-signaled`: queueやschedulerを変更せずexact terminal wait stateを返す
- `pending`: `RuntimeWaits`がmonotonic registration IDをplanし、thread ID、stable object ID、observed handleを保存してlaneを
  `WaitingObject { registration_id, object_id }`へ移す

One threadは同時にone waitだけを持つ。registration table、object-indexed FIFO、next IDは同じ
infallible commitで更新する。nonwaitable heap／module、non-runnable lane、duplicate registration、
ID／index mismatch、ID overflowはallocator／collection mutation前にstructured errorとなる。

Controlled registrationは次である。

```text
waiter thread:       8192
signaler thread:     4097
waited handle:       0x54010002
wait registration:  1
waited object:       second-thread object
```

## Bounded waitable-timer completion

Reached `WaitForSingleObjectEx(handle, 10, TRUE)`は、`RuntimeProcessState`内のshared logical
100 ns clock、`RuntimeTimers`のexact arm generation、`RuntimeObjectTable`のwait state、`RuntimeWaits`のregistration、
`RuntimeScheduler`のlane transitionを一つのbounded transactionとして扱う。Registrationは従来の
thread／object／handle identityにoptional timeout deadlineとalertable flagを加える。

Already-signaled manual-reset timerとzero-time waitはhost requestを作らず、logical stateだけから
`WAIT_OBJECT_0`または`WAIT_TIMEOUT`を返す。Nonsignaled timerでpositive delayが必要な場合は、
Rustがregistration ID、request ID、timer ID、target logical time、clock generation、arm generation、
terminal outcomeを一つのplanへ固定し、laneを`WaitingObject`へ移して
`runtime.timer-delay.v1` requestをpublishする。52-byte payloadはversion 1とfull-width U64 identityを持つ。

Browser completionのackはelapsed timeやWindows resultを運ばない。Kernelはrequest、thread、wait、
object、timer、clock、armのidentityを全件preflightし、logical clock advance、exact arm expiration、
manual-reset signal、registration retirement、scheduler owner-lane resumeを一つのtargeted commitで行う。
Timer deadlineが先なら`WAIT_OBJECT_0`、timeoutが先ならarmとobject stateを保持して`WAIT_TIMEOUT`を返す。
Wrong ack、stale／duplicate completion、rearm、close、thread cancellationはpartial mutationを残さない。

Workerはpositive delayにだけbrowser timer resourceを作る。Page lifecycle generationを検証し、
freeze時にremaining delayを保持、resume時に再armし、同じtaskでのcatch-upを一件に制限する。
Cancellationはresourceをretireし、late callbackはstaleとして数える。Node runnerはsleepせず同じpayloadへ
deterministic ackを返すため、guest-visible logical resultはbrowser wall timeから独立する。

このserviceに限り、Workerはdelay完了まで単一emulated CPUのguest実行を止め、Rust completionが指定した
owner laneを一度だけ直接復帰させる。Wall timingで別laneのinstruction orderが変わることを防ぐための
bounded policyであり、他のhost requestは従来どおり別guest laneを実行できる。Existing cross-thread waiterが
ある場合はinterleaveせずmutation前にstructured conflictとする。詳細は
[bgi-wait-for-single-object-ex.md](bgi-wait-for-single-object-ex.md)を正とする。

## Signal and wake

Wait state updateはobject recordsをcloneしたplan上で完成させ、`RuntimeWaits`がobject FIFO順のwake
candidateを作る。Aggregateは各candidateをrunning threadとexact `WaitingObject` laneへ
reverse-validationする。Process exitでwake対象から除外するthreadもvalidationを省略しない。全preflight
後だけ次をcommitする。

1. thread objectを`signaled(exit=23)`へ更新する
2. registrationとobject FIFO entryをretireする
3. waiter laneを`Runnable`へ戻す
4. `RuntimeSchedulerEvent::WaitSatisfied`を1件enqueueする

Controlled signal sourceはsecond threadのtwo-stage exit completionである。exit request中はthread
objectがnonsignaledのためwaiterはsleepを維持し、completion後だけwakeする。eventをconsumeした
initial threadが同じsignaled handleを再waitすると`already-signaled`になり、新しいregistration／
eventを作らない。

## Cancellation and process exit

Thread exit requestはそのthread自身のpending wait registrationをhost request／callback／scheduler
eventと同じcancellation planへ入れる。process-wide terminationは全threadのwaitを先にcancelし、
terminal object signalによって終了対象laneを誤ってwakeしない。

Controlled process exitはinitial-thread registration `1`をcancelし、aggregate
`wait_registrations_cancelled = 1`を返す。registration table、object FIFO、scheduler eventはterminal
stateでemptyである。Whole `RuntimeState` cloneは行わない。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --lib runtime::scheduler::tests --locked
cargo test -p vine-core --lib runtime::waits::tests --locked
cargo test -p vine-core --test runtime_wait_queue --locked
```

Subsystem tests fix mutation-free lane／selection／registration plans、event／wait FIFO、allocator overflow、
serialized shape、duplicate rejection and excluded-wake validation. Integration tests fix
object／thread／scheduler atomicity、root allocator compatibility view、wake ordering and process-exit
cancellation. Current Node／Chrome semantic result、artifact identity、benchmark、
next structured stopは[current checkpoint manifest](current-checkpoint.json)だけを正とする。

## Non-claims and next dependency

This checkpoint did not initially implement event／mutex／semaphore／timer objects. The reached
`CreateMutexA` checkpoint now adds named mutex object identity and immediate unowned／recursive
acquisition through this one-object boundary, but not cross-thread waiter registration. Reached
waitable-timer creation, arm, logical expiration, bounded finite timeout, manual-reset signal,
alertable-flag acceptance, versioned browser pacing, cancellation, stale completion, and page
freeze／resume policy are now implemented. Event／semaphore, mutex abandonment, waiter priority,
multiple waiters, wait-any／wait-all, APC delivery, message wake, `INFINITE`, general wait fairness, and
general guest wait API coverage remain unimplemented.
It is partial evidence for `P1-SCH-01`／`REQ-API-03`, not their completion.

The controlled per-thread message queue dependency is now implemented in
[runtime-message-queue.md](runtime-message-queue.md): ordered enqueue／owner dequeue and empty-queue
wake share the scheduler metadata. Controlled private-heap backing is implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). Named mutex ownership／recursion／handle lifetime
is documented in [runtime-synchronization.md](runtime-synchronization.md) and
[bgi-create-mutex-a.md](bgi-create-mutex-a.md). Runtime responsibility checkpoint S13 completes the
remaining aggregate ownership audit; capability expansion resumes from the saved `GetCurrentThread`
stop after its evidence gate.
