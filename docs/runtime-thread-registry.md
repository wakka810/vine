# Runtime thread registry foundation

## Purpose

review Step 9のfourth foundationとして、single controlled processの`RuntimeState`へ
initial threadとは別のlogical threadを登録するversion 1 registry contractを追加した。
thread identity、typed object／handle、LastError／TLS／FLS／callback state、scheduler lane、exitを
同じRust authorityへ置き、wait queueとper-thread CPU bootstrapの前提を作る。

このcheckpointはguest `CreateThread`を追加せず、CPU context、stack、TEB／FS、start routine、
parameter、suspend countを作らない。controlled testとfuture schedulerがlogical laneを明示的に
登録／選択するmetadata boundaryである。

## Registration and selection

`register_thread(thread_id, initial_last_error)`はrunning processだけで成功する。nonzeroかつ
process内でuniqueなTIDを要求し、次を一回のcommitで作る。

- `RuntimeThreadState { running, LastError, empty TLS/FLS values, empty callback stack }`
- `SchedulerThreadState::Runnable`
- `RuntimeObjectKey::Thread`とnonsignaled wait state
- generation付きinitial thread handle

object identityとhandle allocationはthread／scheduler mapのmutation前に完了する。duplicate TID、
zero TID、object／scheduler registry mismatchはstateを変えずstructured errorになる。controlled
second thread `4097`のhandleは`0x54010002`である。

`select_thread`はregistered `running` threadとlive scheduler laneを両方検査してからsingle
current-thread selectorを変更する。runnableだけでなくwaiting-host／waiting-object／in-callback
laneを選択でき、host completionやnested callback stateをそのthreadへ帰属できる。initialized、
exit-requested、exited、terminating、terminated laneは選択できない。

SelectionはCPU context switchではない。現在のthread-local Rust APIsがどのregistry recordを
参照するかを選ぶだけである。

## Thread-local ownership

LastError、logical TLS／FLS value、callback frame stack、scheduler state、pending host requestは
TIDごとに分離する。controlled contractsは同じTLS indexへinitial thread `0x1111`、second
thread `0x2222`を保存し、同じprocess-owned FLS indexへ別pointerを保存する。LastError
`11`／`33`と別callback frameもswitch後に保持する。

Process-owned clock、startup info、module graph、heap、TLS index allocator、FLS slot／callback、
dynamic endpoint、encoded pointer registryは共有のままである。CPU register／FPU state、stack、
TEBはまだthread recordへ追加しない。

## Exit and deterministic process termination

Existing two-stage thread exitはselected threadを対象にし、second threadだけをexit code `23`で
signalしてもprocessとinitial threadをrunning／nonsignaledに保つ。exited threadは再選択できないが、
typed handleからterminal wait stateを取得できる。

Process exitはregistered thread mapのTID昇順で全live laneをpreflightする。各laneのpending host
request、callback frame、scheduler eventを数えてretireし、thread wait-object updateを同じ順で
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
cargo test -p vine-core --test runtime_thread_registry --locked
```

Production regression on the same implementation:

- Chrome Worker: 46 instructions、cancel／watchdog／clean exit、exit code `0`
- BGI: 17 calls／238 instructions; report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark regression: `go`, threshold failures `0`
- Rust runtime core Wasm: 732,779 bytes／SHA-256
  `dcbade608dc66c75c3179d4d7b67d7c0ddc32891c6769b1fdc38e20dd70d4ab8`

The clean commit `a0d99e5` report remains the engine decision authority. Worker report identity is
not embedded in `current-checkpoint.json` because its build manifest hashes that file.

## Non-claims and next dependency

This checkpoint does not implement guest thread creation APIs, per-thread CPU／x87／XMM context,
stack／guard／TEB allocation, start routine execution, suspend／resume, priority, natural last-thread
process exit, concurrent guest execution, TLS／FLS cleanup callbacks, DLL thread attach／detach,
waiter registration, timeout, wait-any／wait-all, APC, or message wake. It is partial evidence for
`P1-SCH-01`／`P3-WIN-02`, not their completion.

The controlled wait-queue dependency is now implemented in
[runtime-wait-queue.md](runtime-wait-queue.md): one selected thread waits on the other thread object
and resumes deterministically after signal. The controlled message queue is implemented in
[runtime-message-queue.md](runtime-message-queue.md), and private-heap backing is implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). The next dependency is DLL lifecycle.
Per-thread CPU bootstrap remains separate before guest-created threads can execute.
