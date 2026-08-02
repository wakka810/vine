# BGI post-timer-wait `TranslateMessage` profile

## Outcome

Primary BGIのfinite alertable timer waitとdue `WM_TIMER` retrieval直後に到達する
`USER32.dll!TranslateMessage(0x006FF8E8)`を、既存のRust-owned read-only
translation contractで完了した。Callはsequence 282,874、occurrence 43、trap
instruction 392,354,433である。ReturnはFALSE、LastErrorは0、return addressは
`0x100937A2`、stack pointerは`0x006FF8D4 -> 0x006FF8DC`で、guest writeはない。

Inputは直前の`PeekMessageA(PM_REMOVE)`がguest address `0x006FF8E8`へpublishした
28-byte `MSG32`である。HWND `0x48010001`、message `WM_TIMER (0x0113)`、wParam
1、lParam／TIMERPROC `0x10099600`、time 3,800,773、point `(960, 540)`をexactに
読み、message content SHA-256
`e6521e2aa3109cd80f56da82d2662d3e526b5a40981927410a2d9061f5ad6135`を固定する。

Replayはさらに4 instructions進み、instruction 392,354,437で次のowned endpoint
`USER32.dll!DispatchMessageA(0x006FF8E8)`を記録する。EndpointはID 182／
`0x7F000B50`、IATは`0x100D13B0`、return addressは`0x100937AC`、stack
pointerは`0x006FF8D4`である。このcheckpointはtranslation validationまでをclaimし、
dispatchとTIMERPROC callbackは後続structured stopへ残す。

## Authority and failure ordering

`RuntimeMessages`、`RuntimeTimers`、`RuntimeWindows`、`GuestMemory`がmessage bytes、
current-thread queue、live HWND、timer registrationの唯一のauthorityである。Translationは
immutable guest bytesを読み、window ID 1／timer ID 1／owner TID 8192／registration
generation 1、delivery count 41、callback `0x10099600`とのrelationをvalidationする。
直前のtimer snapshotはscheduled deadline `1,915,270,000`、delivered time
`2,007,735,000`、next deadline `2,007,935,000`を保持する。

成功はvalidation-onlyであり、queue depth 0、timer deadline／delivery、callback depth 0、
guest bytes、LastError、allocator generationを変更しない。Bad pointer、malformed message、
stale window／timer relation、wrong ownerではmutation前にfaultを返す。TypeScriptはRustが
publishしたimmutable completionを比較するだけで、Windows message copy、queue、timer、
HWND relation、guest pointer、browser timerを所有しない。

## Evidence and acceptance

Caller consumptionは既存のnative caller captureで、`PeekMessageA`のTRUEを確認し、
`WM_QUIT`以外を同じ`MSG32` pointerで`TranslateMessage`、続いて`DispatchMessageA`へ渡す。
Reached `WM_TIMER`／TIMERPROC variantは既存のtimer translation oracleと同じなので新しい
native traceは追加せず、runtime sequence、occurrence、logical HWND／clockをsemantic
handlerのselectorにしない。

Canonical Node replayは282,874 supported calls、392,354,437 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Present、9,003 pacing completionsを
完了する。Ordered semantic digestは
`b13bfc18d983ebe62496144c5a49c95b1a3aeb153cc26b650e531003b352da1e`、terminal
memory digestは
`a85467a00711ff144b0a14740e571d50d00406ad8fafd49cafefdded17fd369f`である。
2,635,242-byte Node reportのSHA-256は
`31aa9a3f2de97bf0154507b96fc7530cedec82f2fcc93c68585e774d1d0c969a`である。

Chrome Worker protocol v125／BGI report schema 91は同じcall、exact `MSG32` digest、
timer snapshot、zero mutation、ordered semantics、terminal memory、次の
`DispatchMessageA` frameを一致させる。Private reportは2,623,115 bytes、SHA-256
`174254d7ff2ad014b6ba3dbefdb64d8211f17577b2ee446764e62fa7ca783427`で、
cancel／watchdog cleanup後のpending host call／callback／timer taskは0である。Raw
generation、writeback、artifact identityの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher pointer_button_ingress_commits_input_queue_and_window_proc_dispatch_atomically --locked
cargo test -p vine-core --test runtime_message_queue --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile user32-translate-message-post-test-cooperative-level-timer-peek
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-translate-message-post-test-cooperative-level-timer-peek --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`DispatchMessageA`、TIMERPROC callback、callback returnはこのcheckpointに含めない。
- Keyboard messageから`WM_CHAR`等を生成する一般的なtranslationを実装していない。
- Filtered／cross-thread retrieval、`GetMessageA`、`WM_QUIT`、timer replacement／
  `KillTimer`へsupport bandを広げていない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
