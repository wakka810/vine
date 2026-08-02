# BGI post-`TestCooperativeLevel` timer-arm `WaitForSingleObjectEx` profile

## Outcome

Primary BGIのpost-`TestCooperativeLevel` timer armに続く
`KERNEL32.dll!WaitForSingleObjectEx(0x57010001, 10, TRUE)`を、既存のRust
waitable-timer wait transactionで完了した。Callはsequence 282,872、occurrence
9,920、trap instruction 392,354,375である。Returnは`WAIT_OBJECT_0`、LastErrorは
0、return addressは`0x10050615`、stack pointerは
`0x006FF8E0 -> 0x006FF8F0`、guest writeは0 bytesである。

Replayはさらに48 instructions進み、instruction 392,354,423で次のowned endpoint
`USER32.dll!PeekMessageA(0x006FF8E8, NULL, 0, 0, PM_REMOVE)`を記録する。
EndpointはID 164／`0x7F000A30`、IATは`0x100D1368`、return addressは
`0x1009378B`、stack pointerは`0x006FF8C8`である。このcheckpointはfinite
alertable waitの完了までをclaimし、次のmessage retrievalは後続structured stopへ残す。

## Authority and failure ordering

`RuntimeProcessState`のlogical clockはdeadline `2,007,735,000`へ進み、generation
16,309となる。`RuntimeTimers`はtimer ID 1／arm generation 68のexact expirationを
記録し、`RuntimeObjectTable`はobject ID 20をmanual-reset `signaled`として保持する。
`RuntimeWaits`はregistration 9,003をretireし、次IDを9,004に進める。Logical handle
`0x57010001`とopen handle count 1は変わらない。

既存の`kernel32.wait-for-single-object-ex.waitable-timer.v1`は、thread、scheduler
lane、host request、registration、typed object、timer、clock、arm generationを
preflightし、1 msのpacing completion後に全identityを再検証してtargeted commitする。
Stale／duplicate completion、wrong ack、rearm、close、wrong object、clock／arm mismatch、
capacity failureはclock、timer、object、registration、scheduler、allocatorを部分更新しない。

Nodeはrequest 9,158をdeterministic ackするだけでtimer resourceを作らない。Chrome
Workerは同じ`runtime.timer-delay.v1` requestに対するcancellable logical-deadline taskだけを
所有する。TypeScriptはWindows timer、wait registration、logical clock、signal state、wait
result、thread resumeを所有せず、host／browser elapsed timeをRust authorityへimportしない。

## Evidence and acceptance

Native semanticsとcaller consumptionは既存の
[finite alertable waitable-timer oracle](bgi-wait-for-single-object-ex.md)を再利用する。
この到達は同じhandle／timeout／alertable variantなので、新しいnative traceは追加しない。

Canonical Node replayは282,872 supported calls、392,354,423 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Present、9,003 pacing completionsを
完了する。Ordered semantic digestは
`d720e2d2a9068b6c3041d427cc1a6a1524f20db89e6689fe3681fdafb88e7082`、terminal
memory digestは
`fc1902ed4967385d6bf77b55a94ad34a0910f45b8d7361b5e027b7f08fb44e89`である。
2,631,858-byte Node reportのSHA-256は
`68e50641e8a1159a91bd68c167d64a0fb5742e1607769ff765303b96a8ad4e42`である。

Chrome Worker protocol v123／BGI report schema 89は同じcall、expiration、signaled
object、registration retirement、ordered semantics、terminal memory、次のPeekMessageA
frameを一致させる。Private reportは2,623,604 bytes、SHA-256
`390a0b9b0ee4bff6e6cf748c463383f186d92199927636cd8331450ded85dcea`で、
9,003 pacing tasksを完了し、cancel／watchdog cleanup後のpending host call／callback／
timer taskは0である。Raw generation、writeback、artifact identityの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test api_dispatcher wait_for_single_object_ex --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
node tools/test-runtime-worker-timer-pacing.mjs
npm run check:runtime:bgi -- --profile kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- `PeekMessageA`のmessage retrieval、queue mutation、MSG writeはこのcheckpointに含めない。
- APC delivery、nonalertable／infinite／larger timeout、他object kind、multiple wait、
  cross-thread waiter、general timer behaviorは追加していない。
- Browser taskの実時間はguest clockやwait resultを選ばず、Windows timer identityでもない。
- この到達はfirst readable title、general frame cadence、long-run、save／load、full-playを
  証明しない。
