# Runtime wait queue foundation

## Purpose

review Step 9のfifth foundationとして、controlled two-thread registryとtyped waitable objectを
Rust-owned wait queueへ接続した。selected threadがnonsignaled objectへwaitを登録し、別threadの
object signalでdeterministically runnableへ戻るまでを、object lifetimeとscheduler stateから
推測しないexplicit contractにする。

このcheckpointはguest `WaitForSingleObject`／`WaitForMultipleObjects`を追加せず、timeout、
wait-any／wait-all、alertable wait、APC、CPU context switchを実装しない。logical scheduler laneと
object signalを接続するmetadata boundaryである。

## Registration model

`wait_current_thread_on(handle)`はrunning processのrunnable current threadだけを受け付ける。
handleをtyped object tableで検証し、process／thread objectのwait stateを読み、次のどちらかを返す。

- `already-signaled`: queueやschedulerを変更せずexact terminal wait stateを返す
- `pending`: monotonic registration ID、thread ID、stable object ID、observed handleを保存し、laneを
  `WaitingObject { registration_id, object_id }`へ移す

One threadは同時にone waitだけを持つ。registration tableとobject-indexed FIFOは同じcommitで
更新する。nonwaitable heap／module、non-runnable lane、duplicate registration、ID／index mismatchは
mutation前にstructured errorとなる。

Controlled registrationは次である。

```text
waiter thread:       8192
signaler thread:     4097
waited handle:       0x54010002
wait registration:  1
waited object:       second-thread object
```

## Signal and wake

Wait state updateはobject recordsをcloneしたplan上で完成させ、そのsame planからobject FIFO順の
`RuntimeWaitCompletion`を作る。各registrationはrunning threadとexact `WaitingObject` laneへ
reverse-validationされる。全preflight後だけ次をcommitする。

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
cargo test -p vine-core --test runtime_wait_queue --locked
```

Production regression on the same implementation:

- Chrome Worker: 46 instructions、cancel／watchdog／clean exit、exit code `0`
- BGI: 17 calls／238 instructions; report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark regression: `go`, threshold failures `0`
- Rust runtime core Wasm: 765,531 bytes／SHA-256
  `3b92ff588f944db29e59cd46ae9a873bbb1d52a1433958499c6eaddd641d59aa`

The clean commit `a0d99e5` report remains the engine decision authority. Worker report identity is
not embedded in `current-checkpoint.json` because its build manifest hashes that file.

## Non-claims and next dependency

This checkpoint did not initially implement event／mutex／semaphore／timer objects. The reached
`CreateMutexA` checkpoint now adds named mutex object identity and immediate unowned／recursive
acquisition through this one-object boundary, but not cross-thread waiter registration. Event／
semaphore／timer, manual／auto reset, mutex abandonment, waiter priority, multiple waiters contract evidence,
timeout clock, wait-any／wait-all, alertable wait, APC, message wake, guest API dispatch, and CPU context
switching remain unimplemented.
It is partial evidence for `P1-SCH-01`／`REQ-API-03`, not their completion.

The controlled per-thread message queue dependency is now implemented in
[runtime-message-queue.md](runtime-message-queue.md): ordered enqueue／owner dequeue and empty-queue
wake share the scheduler metadata. Controlled private-heap backing is implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). Named mutex ownership／recursion／handle lifetime
is documented in [runtime-synchronization.md](runtime-synchronization.md) and
[bgi-create-mutex-a.md](bgi-create-mutex-a.md).
