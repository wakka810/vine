# BGI post-TranslateMessage timer `DispatchMessageA` profile

## Outcome

Primary BGIのpost-timer-wait `TranslateMessage`直後に到達する
`USER32.dll!DispatchMessageA(0x006FF8E8)`を、既存のRust-owned timer dispatch
contractでTIMERPROC callback進入まで完了した。Callはsequence 282,875、occurrence 43、
trap instruction 392,354,437であり、outer `DispatchMessageA`はcallback完了待ちのまま
pendingである。

InputはHWND `0x48010001`、`WM_TIMER (0x0113)`、wParam 1、TIMERPROC
`0x10099600`、time 3,800,773、point `(960, 540)`を持つ28-byte `MSG32`である。
Content SHA-256は
`e6521e2aa3109cd80f56da82d2662d3e526b5a40981927410a2d9061f5ad6135`で、
直前の`TranslateMessage`から不変である。

Runtimeはframe ID 75をcurrent thread 8192へ一件だけenterする。Targetは
`0x10099600`、20-byte stdcall frameのstack／allocation startは`0x006FF8C0`、
return sentinelは`0x7F010000`、argumentsは
`(0x48010001, 0x0113, 1, 3,800,773)`である。Callback result policyは
`ignore-and-return-zero`だが、このcheckpointではcallback returnもouter return 0も
まだ実行しない。

Callback内を21 instructions進め、instruction 392,354,458でnested
`KERNEL32.dll!GetTickCount()`へ到達する。EndpointはID 13／`0x7F0000C0`、IATは
`0x100D10DC`、return addressは`0x10099537`、stack pointerは`0x006FF890`である。

## Authority and failure ordering

`RuntimeMessages`、`RuntimeTimers`、`RuntimeWindows`、`RuntimeThreads`、
`GuestMemory`がMSG、live HWND／timer relation、callback identity／stackのauthorityである。
Dispatch planはpublished `WM_TIMER`、window ID 1、timer ID 1、owner TID 8192、
registration generation 1、delivery count 41、callback addressをmutation前に検証する。
Timer snapshotはscheduled deadline `1,915,270,000`、delivered time
`2,007,735,000`、next deadline `2,007,935,000`を保持する。

Semantic dispatchが`GuestCallback` planを返す時点ではruntime、28-byte MSG、memory
generation、LastError、空queueを変更しない。Callback beginだけがframe capacityとstack
rangeをpreflightして20 bytesをatomicに書き、frame identityをpublishする。Bad pointer、
malformed MSG、stale／wrong-owner window timer、callback frame capacity不足ではpartial frame、
allocator advance、queue／timer mutationを残さない。

TypeScriptはRust callback identityをprotocolで検証し、browser callback object、Windows
message copy、queue、timer、HWND relationの第二の正本を作らない。Browser renderer、pacing
task、callback transportはnon-owning adapter resourceのままである。

## Evidence and acceptance

Caller consumptionとTIMERPROC `(HWND, WM_TIMER, ID, time)`、callback resultを無視して
outer return 0とするbehaviorは既存native／controlled fixture oracleを再利用する。今回の
primary evidenceは、同じ一般handlerへexact MSGとlive timer relationが入り、callback
frame 75を一度だけpublishしたことを固定する。Sequence、occurrence、EIP、hashはhandlerの
selectorにしない。

Canonical Node replayは282,874 completed supported callsと一件のpending dispatch entry、
392,354,458 instructions、100,557 clock samples、40 completed TIMERPROC callbacks、4
Present、9,003 pacing completionsを記録する。Ordered semantic digestは
`b13bfc18d983ebe62496144c5a49c95b1a3aeb153cc26b650e531003b352da1e`、terminal
memory digestは
`6a0bd75c14fb1f79e3b6b4428ae6cfa2d9d804bff08222f68f34c0d0e60a6b76`である。
2,635,704-byte Node reportのSHA-256は
`8831bc999c7d03e668ec3467a471610572603d8d26d26349756b1ca8b4ac6ae8`である。

Chrome Worker protocol v126／BGI report schema 92は同じpending dispatch、frame identity、
MSG／timer snapshot、nested endpoint、ordered completed-call semantics、terminal memoryを
照合する。Private reportとbrowser artifactのidentityは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。Cancel／watchdog
scenarioはpending host request、browser timer task、callback transportを残さず停止する。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher due_window_timer_materializes_once_and_dispatches_timerproc_on_the_owner_thread --locked
cargo test -p vine-core --test runtime_message_queue --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile user32-dispatch-message-a-post-test-cooperative-level-timer-translate
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-dispatch-message-a-post-test-cooperative-level-timer-translate --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Nested `GetTickCount` completion、TIMERPROC return、outer `DispatchMessageA` returnは含めない。
- Callbackのguest result、return unwind、constant outer resultは既存contractで実装済みだが、
  このprimary stopでは完了evidenceをclaimしない。
- Non-timer window procedure、cross-thread dispatch、malformed／unpublished MSG、一般的な
  keyboard translationへsupport bandを広げていない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
