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

## Exit request and completion

Thread/process exitは二段階である。

1. Requestはlive scheduler laneを検査し、そのthreadのpending host requests、callback frames、
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
cargo test -p vine-core --test runtime_lifecycle --locked
```

The test fixes:

- bootstrap `process=bootstrapping / thread=initialized / scheduler=initialized`
- explicit start to three running states and mutation-free double-start rejection
- one callback plus one pending host request retired by process exit request
- request state with both waitable objects still nonsignaled
- mutation-free duplicate request／completion rejection
- completion to thread/scheduler/process terminal state and exact exit code `7`
- thread-only request／completion with thread signal and process nonsignal

Production regression on the same implementation:

- Chrome Worker: 46 instructions, cancel／watchdog／clean exit unchanged, exit code `0`
- BGI: 17 calls／238 instructions; report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark regression: `go`, threshold failures `0`
- Rust runtime core Wasm: 710,027 bytes／SHA-256
  `a641572313cf256ea28ff5c2b53b7b4a04bc6a7f9eab294780a59eed5bb4c38f`

The clean commit `a0d99e5` report remains the engine decision authority. Worker report identity is
not embedded in `current-checkpoint.json` because it contains a build manifest that hashes the
checkpoint file itself.

## Non-claims and next dependency

This checkpoint does not implement additional guest threads, CPU context／stack／TEB allocation,
thread handle creation APIs, current-thread switching, natural last-thread process exit, process
termination across multiple lanes, suspend／resume, priorities, TLS／FLS cleanup callbacks, DLL
thread/process detach, waiter registration, timeout, wait-any／wait-all, APC, or message wake. It is
partial evidence for `P1-SCH-01`／`P3-WIN-02`, not their completion.

The controlled multi-thread registry dependency is now implemented in
[runtime-thread-registry.md](runtime-thread-registry.md): one additional logical thread preserves
per-thread state and typed object lifetime, and all lanes terminate deterministically. The controlled
wait registration／wake dependency is implemented in [runtime-wait-queue.md](runtime-wait-queue.md).
Per-thread message ownership and empty-queue wake are implemented in
[runtime-message-queue.md](runtime-message-queue.md). The next dependency is private-heap backing.
