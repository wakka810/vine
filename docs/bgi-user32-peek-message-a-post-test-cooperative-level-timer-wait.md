# BGI post-`TestCooperativeLevel` timer-wait `PeekMessageA` profile

## Outcome

Primary BGIのfinite alertable timer wait直後に到達する
`USER32.dll!PeekMessageA(0x006FF8E8, NULL, 0, 0, PM_REMOVE)`を、既存の
Rust current-thread queue／window-timer transactionで完了した。Callはsequence
282,873、occurrence 110、trap instruction 392,354,423である。ReturnはTRUE、
LastErrorは0、return addressは`0x1009378B`、stack pointerは
`0x006FF8C8 -> 0x006FF8E0`である。

Ordinary queueが空でlogical timer ID 1がdueだったため、一件のcoalesced
`WM_TIMER`をlazy materializeし、`PM_REMOVE`と28-byte `MSG32` writeをjoint
commitした。Published valueはHWND `0x48010001`、message `0x0113`、wParam 1、
lParam／TIMERPROC `0x10099600`、time `3,800,773`、point `(960, 540)`である。

Replayはさらに10 instructions進み、instruction 392,354,433で次のowned endpoint
`USER32.dll!TranslateMessage(0x006FF8E8)`を記録する。EndpointはID 162／
`0x7F000A10`、IATは`0x100D1360`、return addressは`0x100937A2`、stack
pointerは`0x006FF8D4`である。このcheckpointはmessage retrievalまでをclaimし、
translationは後続structured stopへ残す。

## Authority and failure ordering

`RuntimeTimers`はwindow ID 1／timer ID 1／owner TID 8192／registration
generation 1を保持する。Delivery count 41はscheduled deadline
`1,915,270,000`をlogical time `2,007,735,000`で一度だけ配送し、missed periodsを
coalesceして次deadlineを`2,007,935,000`へrearmする。Queue depthは通常queue 0から
lazy publication／removeを経て0へ戻る。

`RuntimeKernel`はdue-timer delivery、queue append／peek、full 28-byte guest-memory
writeをmutation-freeにplanし、owner thread、live window、registration generation、
clock、queue head、memory accessを全てpreflightする。その後にtimer、queue、guest
bytesだけをtargeted commitする。Bad pointer、stale timer／queue plan、wrong owner、
capacity failureではdeadline、delivery count、queue、guest bytes、allocator、LastErrorを
部分更新しない。

TypeScriptはimmutable completion snapshotを検査するだけで、Windows message、queue
head、timer identity、deadline、timestamp、HWND relation、guest pointerの第二の正本を
持たない。このdeliveryのためのbrowser timerやhost message importも作らない。

## Evidence and acceptance

Native caller consumptionとcurrent-thread unfiltered `PM_REMOVE` behaviorは既存の
[first `PeekMessageA` oracle](windows-native-evidence.md#178-bgi-first-peekmessagea-and-current-thread-queue-contract)
およびwindow-timer caller flowを再利用する。同じretrieval／output contractなので新しい
native traceは追加せず、logical HWND、clock、sequence、occurrenceはdispatch selectorにしない。

Canonical Node replayは282,873 supported calls、392,354,433 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Present、9,003 pacing completionsを
完了する。Ordered semantic digestは
`92e24cd4b779a733e416e122ac6a1948db5ab2bf9744751eeaeb0dc535ee0d49`、terminal
memory digestは
`7232fd6aa6418165a888720edf97c9f60cf0991ec6fffa704a0b5c808b1fa66e`である。
2,634,877-byte Node reportのSHA-256は
`1402984c13f342a85af5da276bec332e759fb6b74afb5d32d0e5f847364ca2fa`である。

Chrome Worker protocol v124／BGI report schema 90は同じcall、exact `MSG32`、delivery
count、deadline transition、queue depths、ordered semantics、terminal memory、次の
`TranslateMessage` frameを一致させる。Private reportは2,624,681 bytes、SHA-256
`b3ae03ca00f9ddd01c2b1110bec02db0e38123059a72b866e8c04004d522453a`で、
cancel／watchdog cleanup後のpending host call／callback／timer taskは0である。Raw
generation、writeback、artifact identityの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test api_dispatcher peek_message_a --locked
cargo test -p vine-core --test runtime_message_queue --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile user32-peek-message-a-post-test-cooperative-level-timer-wait
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-peek-message-a-post-test-cooperative-level-timer-wait --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`TranslateMessage`、`DispatchMessageA`、TIMERPROC callbackはこのcheckpointに含めない。
- Filtered／cross-thread retrieval、`WM_QUIT`、`GetMessageA`、一般的なmessage priorityや
  timer replacement／`KillTimer`へsupport bandを広げていない。
- Multiple overdue periodsを一件へcoalesceするreached policy以外のcadenceをclaimしない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
