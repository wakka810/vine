# Runtime message queue foundation

## Purpose

review Step 9のsixth foundationとして、controlled thread registryへRust-owned per-thread FIFOを
追加した。別registered threadからordered messageをenqueueし、ownerがdequeueするまでのidentity、
empty-queue blocking、first-message wake、thread/process exit cleanupをauthoritative `RuntimeState`へ
置く。

このcheckpointはUser32 `PostMessage`／`PostThreadMessage`／`PeekMessage`／`GetMessage`／
`DispatchMessage`を追加しない。HWND、message、WPARAM、LPARAMはopaque 32-bit metadataであり、
window ownership、filter、paint generation、WM_QUIT semanticsを検証しない。

## Message identity and ownership

`RuntimeThreadMessage`はprocess-global monotonic IDと次を持つ。

- source／target `RuntimeThreadId`
- opaque `hwnd`
- `message`, `w_param`, `l_param`

Queueは`RuntimeThreadState`が所有するbounded `VecDeque`で、controlled capacityは4,096 messages／
threadである。registration時はemptyで、thread switch後もtarget owner recordに残る。IDsはoverflow
前に停止し、host pointerやJavaScript objectをguest metadataへ入れない。

## Enqueue and wake

`post_thread_message`はrunning processのrunnable／in-callback sourceと、registered running targetを
preflightする。Targetがrunnable、host/object wait、callback中ならFIFOへappendするだけで、その
scheduler reasonを上書きしない。

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
cargo test -p vine-core --test runtime_message_queue --locked
```

Production regression on the same implementation:

- Chrome Worker: 46 instructions、cancel／watchdog／clean exit、exit code `0`
- BGI: 17 calls／238 instructions; report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark regression: `go`, threshold failures `0`
- Rust runtime core Wasm: 769,885 bytes／SHA-256
  `db29e4846e40dc840bcf79ad70e613e2c32d251fcb05aeece1bf94ba09299db3`

The clean commit `a0d99e5` report remains the engine decision authority. Worker report identity is
not embedded in `current-checkpoint.json` because its build manifest hashes that file.

## Non-claims and next dependency

This checkpoint does not implement window／queue creation rules, HWND validation, message filters,
Peek versus Get, WM_QUIT special return, generated paint, input, timer messages, `SendMessage`
reentrancy, dispatch callback, cross-process post, broadcast, quota parity, guest API marshalling,
or CPU context switching. Existing W2 message／paint flow remains a separate fixed JavaScript harness.
This is partial evidence for `P1-SCH-01`／`REQ-UI-02`, not their completion.

The next Step 9 dependency is controlled private-heap backing: connect existing typed logical heap
metadata to bounded Rust guest-memory allocations with allocation identity, alignment, free／reuse,
and process-exit teardown before implementing broad HeapAlloc semantics.
