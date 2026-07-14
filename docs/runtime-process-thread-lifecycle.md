# Runtime process and thread lifecycle foundation

## Purpose

review Step 9のthird foundationとして、single controlled process／initial threadの
implicit bootstrap／terminationをexplicit Rust state machineへ移した。loader bootstrap、
guest execution、host cancellation、clean process exit、waitable object signalが別々の
booleanやscheduler stateからprocess/thread authorityを推測しないためのcontractである。

このcheckpointはguest threadを作らず、Windows `ExitThread`／`ExitProcess` APIを追加せず、
DLL detach／TLS callbackを実行しない。future lifecycle workを挿入できるrequest／completion
boundaryまでを固定する。

## State model

Process lifecycle:

```text
bootstrapping -> running -> exit-requested(code, initiating-thread) -> exited(code)
```

Initial thread lifecycle:

```text
initialized -> running -> exit-requested(code) -> exited(code)
```

Scheduler laneはbootstrap中の`initialized`、exit request中の`terminating(code)`を追加し、
existing `runnable`／`waiting-host`／`waiting-object`／`in-callback`／`terminated`と区別する。
process／thread waitable objectsはbootstrapからexit request完了まで`nonsignaled`である。

`RuntimeState::new`はfully validatedだがguest未実行のbootstrap stateを返す。loaderはこの間に
module／endpoint metadataを登録できる。`start_initial_thread`だけがmatching process、thread、
scheduler statesを同時にrunningへ進める。double startとstart前のexit completionはstateを
変えずstructured conflictになる。

Thread lifecycle records now live in private
`runtime/thread.rs::RuntimeThreads`. Start、exit request、exit completion、direct cancellationは
mutation-free thread plansを作る。Aggregate coordinatorはprocess lifecycle、scheduler lane、
object wait state、message cleanup、host-request cancellation、wait registrationをpreflightしてからthread planを
commitし、thread collection／recordを直接mutateしない。

Scheduler lane／event records、pending host-request table、request ID allocator now live in private
`runtime/scheduler.rs::RuntimeScheduler`; wait registration／object FIFO live in private
`runtime/waits.rs::RuntimeWaits`. Host start／completion plans lane and identity together before an
infallible targeted commit. Lifecycle request builds a thread-scoped host cancellation plan before
thread、object、message、wait mutation. The aggregate no longer mutates scheduler、wait、or host-request
collections directly. Root `registries.hostRequests`／`nextHostRequestId` JSON remains a borrowed
serialization view and is not a second authority.

## Process-owned current directory

The process aggregate now owns `RuntimeProcessDirectories` in
`runtime/process.rs`. Its mutable collection is private; other subsystems see
only the normalized `RuntimeCurrentDirectoryA` value and monotonic generation.
Bootstrap explicitly supplies a Windows-932 absolute DOS path, and the process
image path derives the registered image-directory identity independently of the
host installation path.

`SetCurrentDirectoryA` mutation follows the same lifecycle coordination rule as
other cross-subsystem operations: the process subsystem produces a plan, the
aggregate preflights a running process, and the subsystem commits only if its
previous identity and generation still match. Null／empty／unterminated input,
unsupported relative or other absolute paths, lifecycle conflict, and stale plan
all stop before mutation. The runtime copies path bytes and never retains the
guest pointer. TypeScript receives directory metadata for equivalence reporting
only and does not own or update guest-visible current-directory state.

This bounded process-image-directory subset is not a VFS. General `.`／`..`,
drive-relative resolution, directory existence, file identity, and mount/backend
effects remain deferred until reached.

## Exit request and completion

Thread/process exitは二段階である。

1. Requestはlive scheduler laneを検査し、schedulerがplanしたそのthreadのpending host request IDsとcallback frames、
   completed scheduler eventsをretireする。threadは`exit-requested`、schedulerは`terminating`へ
   進むが、waitable objectsはまだsignalしない。
2. Completionはretained workが0であることを再検査し、planned wait-object recordsをcommitする。
   thread lifecycle／schedulerをterminalへ進め、process exitではprocess lifecycleも最後に
   `exited`へ進める。

`request_current_process_exit`／`complete_current_process_exit`の間は、将来のdependency-reverse
DLL detachやTLS cleanupを挿入するboundaryである。`terminate_current_process`と
`cancel_current_thread`は同じpreflight／commitを一回のhost operationとして完了する。

All fallible lifecycle and wait-record checks finish before mutation. Commit targets only the
current thread record, scheduler lane, thread-owned pending work, and at most two cloned waitable
object records. It does not clone `RuntimeState`, module graph, heap/module registries, or external
`GuestMemory`.

Thread-only host cancellation preserves the process `running` state and leaves the process object
nonsignaled. This retains the existing controlled cancellation contract; it is not a claim that
Windows last-thread process termination semantics are implemented.

## Runtime integration

All four `RuntimeState` builders now start the initial thread explicitly.

- browser runtime core starts immediately after controlled state construction
- BGI runner registers runtime module／dynamic endpoint metadata during bootstrap, then starts
- API dispatcher and object tests start through their controlled helper
- lifecycle test intentionally observes the pre-start state first

Worker cancel and clean entry-sentinel exit continue to call `terminate_current_process`, but now
receive thread cleanup and thread/process signal ordering from the common lifecycle path.

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --lib runtime::thread::tests --locked
cargo test -p vine-core --lib runtime::scheduler::tests --locked
cargo test -p vine-core --lib runtime::waits::tests --locked
cargo test -p vine-core --test runtime_lifecycle --locked
```

The test fixes:

- bootstrap `process=bootstrapping / thread=initialized / scheduler=initialized`
- explicit start to three running states and mutation-free double-start rejection
- one callback plus one pending host request retired by process exit request
- unknown host completion preserves the complete aggregate before lifecycle cancellation
- request state with both waitable objects still nonsignaled
- mutation-free duplicate request／completion rejection
- completion to thread/scheduler/process terminal state and exact exit code `7`
- thread-only request／completion with thread signal and process nonsignal
- current-directory bootstrap validation and lifecycle-preflighted plan commit

Current Node／Chrome semantic result、artifact identity、benchmark、next structured stopは
[current checkpoint manifest](current-checkpoint.json)だけを正とし、このfoundation文書へ
変動値を複製しない。

## Non-claims and next dependency

This checkpoint does not implement guest-created threads, per-thread CPU context、second-thread
stack／TEB allocation,
thread handle creation APIs, current-thread switching, natural last-thread process exit, process
termination across multiple lanes, suspend／resume, priorities, TLS／FLS cleanup callbacks, DLL
thread/process detach, waiter registration, timeout, wait-any／wait-all, APC, or message wake. It is
partial evidence for `P1-SCH-01`／`P3-WIN-02`, not their completion.

The controlled multi-thread registry dependency is now implemented in
[runtime-thread-registry.md](runtime-thread-registry.md): one additional logical thread preserves
per-thread state and typed object lifetime, and all lanes terminate deterministically. The controlled
wait registration／wake dependency is implemented in [runtime-wait-queue.md](runtime-wait-queue.md).
Per-thread message ownership and empty-queue wake are implemented in
[runtime-message-queue.md](runtime-message-queue.md). Controlled private-heap backing and the
required exit-request cleanup boundary are implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). Capability expansion is paused while the remaining
heap／module boundaries are extracted.
