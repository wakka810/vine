# BGI post-callback pointer-action continuation batch

## Outcome

Primary BGIのthird `WM_LBUTTONDOWN` flowを、message retrievalからWndProcと外側
`DispatchMessageA`の完全なunwindまで一つのvertical sliceとして進めた。前checkpointの
six-completion prefixに続き、sequence 283,428の
`SetFocus(0x48010001)`は既にfocusedなsame-current-thread windowを検証して同じHWNDを返す。
Sequence 283,429の`timeGetTime`はshared logical process clockから`3,827,181`を返し、
sequence 283,430の
`DefWindowProcA(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, 0x01680280)`はLRESULT 0で完了する。

WndProc frame 76はinstruction 445,135,361に0を返し、pending sequence 283,424の外側
`DispatchMessageA`も同じresultでretireする。Queue depthとactive callback depthは0のまま、
九guest instructions後のinstruction 445,135,370で次のunfiltered
`PeekMessageA(PM_REMOVE)`へ到達する。

## Authority and failure ordering

Logical window、focus relation、message queue、WndProc frame、outer dispatch lifetimeは
`RuntimeUser32`、process clockはRust runtime process state、guest bytesは`GuestMemory`が所有する。
WorkerはRust completionのimmutable projectionとframe identityを比較するだけで、TypeScript側に
Windows focus、queue、callback、clock stateを作らない。

`SetFocus`はlive／created／same-owner／active／focused／foreground relationをmutation前に検証し、
reached same-window caseではstate、LastError、guest bytesを変更しない。`timeGetTime`は既存の
scheduled shared-clock transactionだけを進める。`DefWindowProcA`はpointer payloadとlogical cursor／
window relationをpreflightしてzero-result／zero-mutationで完了する。Batch completionは三件のaction
completion、Rust-issued frame 76のreturn、outer dispatch retirement、terminal queue／callback depth、
memory digestが全て一致した場合だけpublishする。

## Evidence and acceptance

既存のcontrolled same-window `SetFocus` oracle、shared-clock `timeGetTime` contract、client
`WM_LBUTTONDOWN`の`DefWindowProcA` oracleでsemanticsとcaller consumerは固定済みなので、新しい
native traceは追加していない。Primary sequence、occurrence、frame ID、instruction count、guest addressは
artifact identityであり、semantic handlerのselectorではない。

Canonical Node replayは283,430 completed calls、445,135,370 instructions、100,567 clock
samples、41 completed TIMERPROC callbacks、9,036 pacing completionsを記録する。Ordered
semantic digestは
`0520fc051a9c6427305fc5044f731490b281756b1afe6925f4e49fa208d8f15f`、terminal memory
digestは
`f82b5e3e63f9d4676b97f130abc8da6fa375f3e5787357d56e25c83b49556b5b`である。

Chrome Worker protocol v134／BGI report schema 99は、同じnine-completion batch、action call
order、frame 76のreturn、outer dispatch result、queue／callback depth 0、next `PeekMessageA`、
semantic／memory digestを照合し、cancel／watchdog cleanupも維持する。Artifact identityと次stopは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher --locked
node tools/test-runtime-worker-evidence-level.mjs
npm run check:runtime:bgi -- --profile primary-main-loop-post-callback-pointer-action-continuation-batch
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile primary-main-loop-post-callback-pointer-action-continuation-batch --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Next `PeekMessageA`のcompletionと、その後のmain-loop iterationは含めない。
- Same-window `SetFocus`はfocus transition message、cross-thread focus、DOM focusを証明しない。
- Reached client `WM_LBUTTONDOWN` default processingはcapture、move／wheel、double-click、
  nonclient／right-button variantを証明しない。
- Clickのvisible gameplay response、readable title、audio、save／load、representative flow、
  long-run、full-playを証明しない。
