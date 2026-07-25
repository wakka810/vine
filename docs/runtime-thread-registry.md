# Runtime thread registry foundation

## Purpose

reviewのruntime responsibility splitとして、single controlled processのthread registryを
`runtime/thread.rs`のprivate `RuntimeThreads`へ移した。Initial threadとは別のlogical threadを登録する
既存contractに加え、reached guest `CreateThread`／`ResumeThread`がthread identity、typed object／handle、
stack／TEB／FS bootstrap、LastError／TLS／FLS／callback state、logical affinity、COM apartment、scheduler
lane、logical priority、exitを同じRust authorityへ接続する。

Rustはguest-visible bootstrap metadataとlifetimeを所有し、Runtime WorkerはTIDでkeyedしたopaque execution-engine
CPU contextだけを保持する。Worker側にWin32 thread record、handle、suspend count、TEB lifetimeの第二の正本は
作らない。Controlled pre-registered threadとAPI-created threadは同じscheduler／exit boundaryを使う。

## Subsystem boundary

`RuntimeThreads`だけが`BTreeMap<RuntimeThreadId, RuntimeThreadState>`、process-wide TLS index set、
FLS slot map、TLS／FLS／callback-frame ID allocatorを所有する。Collectionは
`runtime/thread.rs`の外へ公開せず、aggregateはimmutable state viewと
`RuntimeThreadMutationPlan`だけを受け取る。Registration、initial start、LastError、TLS／FLS、
callback push／pop、stack registration／growth、logical affinity、exit request／completionはすべてprevious／next
recordを持つmutation-free planを返す。Logical priorityも同じowner-local plan／commitを使い、scheduler、
object、message、wait、GuestMemoryのpreflightが
成功した後にcommitする。

Aggregateはthread mapへ`get_mut`／`insert`せず、thread recordを直接変更しない。Scheduler lane／
host-request table／request ID、wait registration、message queueはそれぞれのownerに残し、thread subsystemはそれらの
mapを参照またはmutateしない。Serialized `RuntimeState.threads` shapeはtransparent newtypeで維持する。
Existing `process.allocatedTlsIndices`／`process.flsSlots`とroot `nextTlsIndex`／`nextFlsIndex`／
`nextCallbackFrameId`はborrowed compatibility viewであり、aggregate-owned stateではない。
Manifest boundary version `vine.runtime-thread-registry.v7` includes optional serialized
`affinityMask`、`priority`、`comApartment`、`environment`、`guestBootstrap` fields and the owner-local
`nextThreadId` allocator. Unset state is omitted, so historical reports retain their previous shape;
active records materialize each field only after its owner commit. There is no persistent snapshot
migration or duplicate compatibility field.

## Registration and selection

`register_thread(thread_id, initial_last_error)`はrunning processだけで成功する。nonzeroかつ
process内でuniqueなTIDを要求し、次を一回のcommitで作る。

- `RuntimeThreadState { running, LastError, empty TLS/FLS values, empty callback stack }`
- `SchedulerThreadState::Runnable`
- `RuntimeObjectKey::Thread`とnonsignaled wait state
- generation付きinitial thread handle

Object identityとhandle allocationはthread／scheduler mapのmutation前に完了する。Duplicate TID、
zero TID、object／scheduler registry mismatchはstateを変えずstructured errorになる。controlled
second thread `4097`のhandleは`0x54010002`である。

`select_thread`はregistered `running` threadとlive scheduler laneを両方検査してからsingle
current-thread selectorを変更する。runnableだけでなくwaiting-host／waiting-object／in-callback
laneを選択でき、host completionやnested callback stateをそのthreadへ帰属できる。initialized、
exit-requested、exited、terminating、terminated laneは選択できない。

SelectionはCPU context switchではない。現在のthread-local Rust APIsがどのregistry recordを
参照するかを選ぶだけである。

## Guest creation and CPU bootstrap

Reached `CreateThread(NULL, 0, start, parameter, flags, output)`はdefault 1 MiB reserve／4 KiB commitを
versioned policyから選び、flags `0`または`CREATE_SUSPENDED`だけを受理する。Kernelは次を全てmutation前に
preflightし、owner-local planをtargeted commitする。

- monotonic TID、thread record、typed waitable thread object／handle
- dedicated stack arenaのreserve、top commit、growth guard、stdcall entry frame
- dedicated environment arenaのone-page TEBとprocess-owned PEB relation
- TEBのexception list、stack base／limit、self、PID／TID、PEB fields
- scheduler laneのrunnableまたはpositive suspended count
- optional thread-ID outputを含むGuestMemory transactionとbinary mapping delta

Entry frameはreturn sentinelとparameterを持ち、bootstrap snapshotはentry point、initial ESP、FS base、
return sentinelを固定する。Address／memory owner、object／handle slot、TID allocator、thread／scheduler mapの
どれかが不足またはstaleならobservable stateを一切更新しない。Successful creationだけがoptional outputを
writeし、LastErrorを保存する。

`ResumeThread`はtyped live thread handleを再検証し、positive suspended countをdecrementしてprevious countを
返す。1から0へのtransitionだけがlaneをrunnableにする。Synchronous API completionではcallerを即時preemptせず、
primary callerが`ResumeThread`直後に同じhandleで`SetThreadPriority`へ進むorderingを保つ。別のrunnable laneが
存在する間だけcompleted guest API boundaryを数え、64 boundariesでruntime TID順のround-robin selectionを
plan／commitする。競合laneがなければcounterは0へ戻すため、adapterの起動履歴やhost slice数は後のswitch時点へ
影響しない。Callback frameの実行中はcounterを進めず、callbackを別threadへinterleaveしない。Selection planは
mutation-freeで、current thread／counter／runnable setがstaleならcommit前に拒否する。

このbounded cooperative policyはpriority-weighted、instruction／wall-clock preemption、CPU-bound threadのfairnessを
claimしない。Browserのperiodic slice yieldもguest thread selectionではない。

Dynamic threadがentry return sentinelへ到達すると、Rustはexit／signalをcommitした後、そのthreadのstack／TEB
mappingとaddress allocationをreleaseする。Workerはreturned mapping deltaをmirrorへ適用してCPU contextをretireする。
Handle closeはobject identityのlast referenceをretireし、released stack／TEB addressは後続creationで再利用できる。

Reached `GetCurrentThread` uses this same selector. The aggregate verifies the
selected `RuntimeThreadState` and its `RuntimeObjectKey::Thread`, then returns
Win32 pseudo value `0xFFFFFFFE` without opening a new typed handle. Selecting the
controlled second thread changes the resolved `RuntimeObjectId` while preserving
the pseudo value and handle-table cardinality. The contract is documented in
[bgi-get-current-thread.md](bgi-get-current-thread.md).

## Thread-local ownership

LastError、logical TLS／FLS value、callback frame stack、initial-thread stack metadata、
optional logical affinity mask、optional COM apartment model／successful initialization countは
TIDごとに分離する。Scheduler stateとpending host requestもTIDへ帰属し、private
`RuntimeScheduler`が同じplan／commit境界で所有する。
Controlled contractsは同じTLS indexへinitial thread `0x1111`、second
thread `0x2222`を保存し、同じprocess-owned FLS indexへ別pointerを保存する。LastError
`11`／`33`と別callback frameもswitch後に保持する。

TLS index allocationはindex setとcounter、FLS allocationはslot map／counterとselected-thread storage initialization、
callback entry／replacementはframe stackとframe ID counterをそれぞれsame `RuntimeThreads` commitで進める。
Collision、limit、ID overflow、scheduler preflight failureはcollection／allocatorを不変にする。

Process-owned clock、startup info、module graph、heapはprocess-wideである。Dynamic endpointとencoded pointer
identityはprivate `RuntimeRegistries`、TLS／FLS slot identityとcallback-frame IDはprivate `RuntimeThreads`が
所有する。Initial-thread reserve／commit／guard
metadataはthread recordへ接続する。CPU register／FPU stateのbinary layoutはexecution-engine adapterに閉じ、
RustのWindows stateへ複製しない。

An unset affinity mask resolves to the compatibility policy's virtual process
mask. `plan_affinity_mask` validates a running thread and a nonzero requested
subset, returns the previous effective mask, and prepares only that thread's
next record. The affinity-specific commit rejects stale／duplicate plans before
delegating to the ordinary targeted thread-record replacement. Selecting
another logical thread exposes its independent effective mask; no native handle,
host CPU mask, or browser scheduler state enters the subsystem.

An unset priority resolves to Win32 `THREAD_PRIORITY_NORMAL` (`0`).
`plan_priority` validates a running logical thread and returns the previous
effective value with a mutation-free replacement for that record only.
`commit_priority` compares the complete preflight record and rejects stale or
duplicate use before targeted commit. The semantic enum covers the Win32
priority values implemented by the handler, while the active compatibility
policy admits only the reached `THREAD_PRIORITY_ABOVE_NORMAL` (`1`) variant.
Priority is logical state in this checkpoint: it does not change cooperative
lane selection and imports neither host scheduler class nor browser task
priority.

An unset COM apartment is uninitialized. `plan_com_initialization` creates
count 1 for the requested model, checked-increments a same-model count, or
returns changed-mode without a next record. `plan_com_uninitialization`
decrements only a successful count and removes the apartment at zero. Both
commits compare the complete preflight thread record and reject stale or
duplicate use; overflow, missing apartment, and non-running lifecycle failures
do not mutate the registry. Exit completion clears the apartment on that thread
only. `CoInitialize(NULL)` selects STA through this boundary; COM object
identity and reference count remain in `RuntimeCom`. The reached contract is
documented in [bgi-co-initialize.md](bgi-co-initialize.md).

## Exit and deterministic process termination

Existing two-stage thread exitはselected threadを対象にし、second threadだけをexit code `23`で
signalしてもprocessとinitial threadをrunning／nonsignaledに保つ。exited threadは再選択できないが、
typed handleからterminal wait stateを取得できる。

Process exitはregistered thread mapのTID昇順で全live laneをpreflightする。各laneのpending host
request ID list、callback frame、scheduler eventをmutation-free planで数えてretireし、thread wait-object updateを同じ順で
planした後、process objectを最後にplanする。controlled orderは次である。

```text
thread 4097 -> thread 8192 -> process 4096
```

`RuntimeProcessCancellation.thread_cancellations`がこのdeterministic orderを保持する。controlled
terminationはhost request `2`、callback frame `2`、scheduler event `0`をretireし、両threadと
processをexit code `7`でsignalする。別threadが既にexit-requestedならprocess-wide requestを
mutation前に拒否する。

Process terminationは`RuntimeState`全体をcloneせず、各threadのnext record、最大3 wait-object
records、ordered cancellation receiptだけをplanする。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --lib runtime::thread::tests --locked
cargo test -p vine-core --test runtime_thread_registry --locked
cargo test -p vine-core --test runtime_lifecycle --locked
cargo test -p vine-core --test runtime_initial_thread_stack --locked
cargo test -p vine-core --test runtime_guest_thread --locked
cargo test -p vine-core --test kernel32_thread_dispatcher --locked
```

Subsystem unit tests fix mutation-free planning、per-thread value／affinity／COM-apartment isolation、callback／exit ordering、
TLS／FLS／callback allocator co-commit、collision／limit／overflow rollback、duplicate／zero identity rejection.
Scheduler tests add thread-scoped host start／completion／cancellation plans. Aggregate tests fix
object／scheduler／message／wait atomicity、initial-stack GuestMemory coordination and the existing process／root
serialization compatibility view. Current Node／Chrome result、runtime artifact、benchmark、
next structured stopはこのhistorical foundation文書へ複製せず
[current checkpoint manifest](current-checkpoint.json)だけを正とする。

## Non-claims and next dependency

This checkpoint does not implement priority-weighted scheduling, natural last-thread process exit,
simultaneous multi-core guest execution, TLS／FLS cleanup callbacks, DLL thread attach／detach,
waiter registration, timeout, wait-any／wait-all, APC, message wake, host affinity control,
arbitrary thread-handle affinity, host topology discovery, guest `CoInitializeEx`／`CoUninitialize`,
cross-thread COM marshaling, or apartment message pumping. It is partial evidence for
`P1-SCH-01`／`P3-WIN-02`, not their completion.

The controlled wait-queue dependency is implemented in
[runtime-wait-queue.md](runtime-wait-queue.md): one selected thread waits on the other thread object
and resumes deterministically after signal. The controlled message queue is implemented in
[runtime-message-queue.md](runtime-message-queue.md), and private-heap backing is implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). Scheduler／wait ownership is now private as documented in
[runtime-wait-queue.md](runtime-wait-queue.md). Runtime responsibility checkpoint S13 completes the
remaining process／kernel ownership audit. API-created CPU bootstrap now uses the same Worker context table
as the controlled second-thread fixture. `GetCurrentThread` consumes only the logical selector; it does
not claim CPU switching or host-thread identity. The reached `SetThreadAffinityMask` adds only a
policy-bounded logical mask to that selected record. Reached `SetThreadPriority` likewise adds only a
policy-bounded logical priority to the resolved typed thread record; it does not affect runnable-lane
selection or host/browser scheduling. The current next capability is recorded only in the
current checkpoint manifest.

The x86 `GetSystemInfo` contract reuses that policy-owned process mask for
`dwActiveProcessorMask` and derives `dwNumberOfProcessors` by popcount. It does
not add topology state to `RuntimeThreads`, query host CPUs, or duplicate the
mask in the system-information policy.
