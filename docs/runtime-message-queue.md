# Runtime message queue foundation

## Purpose

Controlled thread registryへRust-owned per-thread FIFOを置き、別registered threadからordered
messageをenqueueし、ownerがdequeueするまでのidentity、empty-queue blocking、first-message wake、
thread/process exit cleanupをauthoritative Rust runtime kernelへ置く。Mutable queue authorityそのものは
private `RuntimeMessages`に限定する。Reached `PeekMessageA` sliceはcurrent threadのunfiltered queue headを
nonblockingにinspectし、optional removeとx86 `MSG` publicationを一つのtransactionとして扱う。Reached
window-timer profileでは、queueがemptyかつlogical deadlineがdueの場合だけ`RuntimeTimers`のdelivery planを
同じtransactionへ参加させ、low-priority `WM_TIMER`をlazy materializeする。

このcheckpointはgeneral User32 `PostMessage`／`PostThreadMessage`／`GetMessage`を追加しない。Reached
`DispatchMessageA`は登録済みwindow timerの`TIMERPROC`だけを対象とする。Window filter、cross-thread
retrieval、paint generation、WM_QUIT special returnは未実装である。

## Message identity and ownership

`RuntimeThreadMessage`はprocess-global monotonic IDと次を持つ。

- source／target `RuntimeThreadId`
- opaque `hwnd`
- `message`, `w_param`, `l_param`
- post時のlogical tick `time`
- post時のlogical screen cursor `point_x`, `point_y`

Queueとprocess-global ID allocatorは`runtime/messages.rs`のprivate `RuntimeMessages`が所有する。
mutable `BTreeMap<RuntimeThreadId, VecDeque<_>>`は`RuntimeKernel`、thread subsystem、schedulerへ公開しない。
read-only inspectionは`queued_thread_messages`が返すcopy snapshotを使う。controlled capacityは4,096
messages／threadであり、registration時はemptyである。IDsはoverflow前に停止し、host pointerや
JavaScript objectをguest metadataへ入れない。

## Mutation boundary

`RuntimeMessages`はqueue／allocatorだけを検査して`RuntimeMessagePostPlan`、receive plan、peek plan、
clear planを返す。plan作成はstateを変更しない。aggregate rootの`RuntimeKernel`（互換名`RuntimeState`）は
process／source thread／target thread／scheduler laneをpreflightし、message planをcommitした後、必要な場合だけ`MessageReady`と
`Runnable`を反映する。message subsystemはscheduler mapを直接mutateせず、schedulerはqueueを直接
mutateしない。

Scheduler lane／event state is now owned by private
`runtime/scheduler.rs::RuntimeScheduler`. Message wake and empty receive preflight a scheduler lane plan
before queue mutation, then commit lane／event through subsystem methods; the aggregate no longer mutates
the scheduler map or event FIFO directly.

thread registrationはmessage queueの重複をobjects／thread／scheduler commit前にpreflightする。
thread exitはclear planのmessage countをcancellation receiptへ入れ、同じcoordinator commitでqueue、
host request、callback、wait、event、lifecycleをretireする。

## Enqueue and wake

`post_thread_message`はrunning processのrunnable／in-callback sourceと、registered running targetを
preflightする。capacity、ID overflow、waiting/queue consistencyのfailureはqueue、ID allocator、thread、
scheduler eventを一切変更しない。Targetがrunnable、host/object wait、callback中ならFIFOへappend
するだけで、そのscheduler reasonを上書きしない。

Targetが`WaitingMessage`ならqueueがemptyであることを再検査し、first messageをappendしたsame
commitでlaneを`Runnable`へ戻し、`RuntimeSchedulerEvent::MessageReady`を1件enqueueする。後続
messageはownerが既にrunnableなので追加wake eventを作らない。

Controlled first wake:

```text
producer:       thread 4097
owner:          thread 8192
message IDs:    1, 2
first wake:     message 1, queue depth 1
second wake:    none
```

## Dequeue and blocking

`receive_current_thread_message`はrunning／runnable ownerだけを受け付ける。nonempty queueではfrontを
一件popしてrunnableを維持する。empty queueでは`Blocked`を返し、laneを`WaitingMessage`へ移す。

Controlled ownerはIDs `1`、`2`をFIFOでconsumeし、再びempty receiveでblockする。producerがID `3`
をpostするとsecond exact wake eventが生じ、ownerはID `3`をconsumeする。blocked ownerによるduplicate
receiveとexited targetへのpostはfull stateを変えずstructured errorとなる。

## Nonblocking PeekMessageA transaction

Reached policyは`hWnd = 0`、minimum／maximum message `0`、flag `PM_NOREMOVE (0)`または
`PM_REMOVE (1)`だけを受け付ける。QueueがemptyならFALSEを返し、28-byte outputとLastErrorを変更しない。
Messageがある場合はqueue headから次のx86 `MSG`を組み立てる。

```text
HWND, message, WPARAM, LPARAM, time, POINT.x, POINT.y
4     4        4       4      4     4        4       bytes
```

`RuntimeKernel`はmutation-free queue planを取得し、messageがある場合は完全な28-byte GuestMemory writeを
transactionへpreflightする。次に同じmessage IDがqueue headであることを再検証し、remove/no-removeを
targeted commitしてからpreflight済みmemory transactionをcommitする。したがってbad pointer、unsupported
filter／cross-thread variant、stale planはqueue、allocator、guest bytes、LastErrorを部分更新しない。
Empty resultではguest write transaction自体を作らない。

## Lazy window-timer materialization and dispatch

`RuntimeTimers`はwindow timer recordにarmed time、next deadline、delivery count、last deliveryを保持する。
Current-thread unfiltered `PeekMessageA`で通常queueがemptyの場合だけ、selected owner thread、live HWND、timer
ID、registration generation、logical clockを検査し、due timerから一件の`WM_TIMER`をmaterializeする。同じ
timerが複数period遅れていても一件だけ配送し、次deadlineはcurrent logical timeからeffective interval後へ
rearmする。Browser task timer、`performance.now()`、Worker-side deadline tableは使わない。

`PM_NOREMOVE`はmaterialized messageをqueue headに残し、`PM_REMOVE`はdelivery plan、queue append／pop、完全な
28-byte `MSG32` writeを全てpreflightしてからtargeted commitする。Bad output pointer、stale timer／queue plan、
wrong owner、capacity failureではdeadline、delivery count、queue、guest bytesのどれも変えない。通常messageが
既にある場合はtimerより先にFIFO headを返すため、window timerはlow-priorityである。

Reached `TranslateMessage`はpublished `MSG32`を読み、live timer registrationとのHWND／`WM_TIMER`／ID／
TIMERPROC relationを検証してFALSEを返す。Reached `DispatchMessageA`は同じrelationとcallback preflightを行い、
selected threadのstdcall frameへ`(hwnd, WM_TIMER, timerId, time)`を積む。Guest callback resultはWin32
`TIMERPROC` contractどおりouter resultへ伝播させず、callback完了後の`DispatchMessageA`は0を返す。Callback
frame、timer、queue、window、guest memoryのauthorityはRustにあり、Workerはimmutable snapshotだけを記録する。

## Lifecycle cleanup

Thread exit requestはowned queued messagesをcallback／host request／wait registration／scheduler eventと
同じcancellation planでretireする。Process termination receiptはTID昇順のper-thread
`messages_discarded`とaggregate countを保持する。

Controlled process exitはinitial-thread queueの2 messagesをdiscardし、aggregate
`messages_discarded = 2`、producer `0`／owner `2`を返す。terminal thread queuesとscheduler eventsは
emptyである。Post operationはqueue全体や`RuntimeState`をcloneせず、one FIFO appendだけをcommitする。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core runtime::messages::tests --locked
cargo test -p vine-core --test runtime_message_queue --locked
```

Subsystem tests fix non-mutating plan、FIFO identity、receive／peek commit、no-remove、remove、empty、due／
not-due timer selection、coalesced rearm、missing-owner rejection。Integration tests fix first-message wake、no
duplicate wake、exit cleanup、capacity rejection、failure-atomic PeekMessageA publication and validated
TIMERPROC dispatch across message／timer／memory／thread／scheduler state。Current production regressionは
[`current-checkpoint.json`](current-checkpoint.json)のcanonical commandを使う。

## Non-claims and next dependency

This checkpoint does not implement window／queue creation rules, filtered or cross-thread Peek, GetMessage,
WM_QUIT special return, generated paint, input-message production, general `DispatchMessage`, cross-process
post, broadcast, or quota parity. Window-timer delivery is limited to the reached live-HWND／ID 1／20 ms／
TIMERPROC profile. Sixteen ordered coalesced deliveries under one registration generation are fixed, but
replacement race, `KillTimer`, NULL-HWND allocated IDs, combined action-input response, and background
freeze／resume remain unproven. Existing W2 message／paint flow remains a separate fixed JavaScript harness.
This is partial evidence for `P1-SCH-01`／`REQ-UI-02`, not their completion.

The controlled private-heap dependency is implemented in
[runtime-heap-backing.md](runtime-heap-backing.md): typed logical heaps own bounded Rust guest-memory
allocations, deterministic free／reuse, and exit-request teardown. The active profile and next structured
stop remain authoritative only in [`current-checkpoint.json`](current-checkpoint.json).
