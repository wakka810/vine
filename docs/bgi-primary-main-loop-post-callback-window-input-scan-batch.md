# BGI post-callback window/input scan batch

## Outcome

Primary BGIのpost-callback main loopを、`GetCursorPos`一件ではなく次の入力判定境界まで
一つのvertical sliceとして進めた。Node／Chromeはsequence 282,880の
`GetCursorPos(0x006FF88C)`でRust-owned cursor `(960,540)`を一回の8-byte writeとして
publishし、その直後にnormalized primary-button down sequence 3をRust input ingressへ
commitする。

Guestは続いて`GetSystemMetrics(7,4,8)`を6組、合計18回実行する。結果は各組
`(3,23,3)`で、occurrence 450から467まで既存のbounded display policyと一致する。
最後にsequence 283,412／instruction 445,134,233の
`IsIconic(0x48010001)`が`FALSE`を返し、9 instructions後の
`WINMM.dll!timeGetTime`でcheckpointを閉じる。

投入したactionはclient point `(640,360)`、screen point `(960,540)`、
`WM_LBUTTONDOWN`、`wParam=1`、`lParam=0x01680280`へ正規化される。生成されたmessage
ID 44はcurrent-thread queueに残っており、このcheckpointは入力の消費をまだ主張しない。

## Authority and failure ordering

Cursorとkey/button stateは`runtime/input.rs::RuntimeInput`、logical windowは
`runtime/user32.rs::RuntimeUser32`、messageはRust-owned current-thread queue、output bytesは
`GuestMemory`が所有する。Workerは第三のpointer eventを`GetCursorPos` completion後にだけ
commitし、receiptのwindow/thread identity、keyboard generation、cursor generationを照合する。
DOM pointer state、browser window state、TypeScript message queueを第二の正本にしない。

各semantic callは既存ownerのvalidation／preflight／targeted commitを再利用する。
`GetCursorPos`のoutput spanが不正なら入力を投入せず、input commitが失敗すればmessage、
keyboard generation、cursor、guest memoryを部分更新しない。Metric queryと`IsIconic`は
read-onlyで、batch completionは18 metric calls、最後の`IsIconic`、一件のaction receipt、
pending message identityが全て揃った時だけpublishする。

## Evidence and acceptance

既存のnative `GetCursorPos`、`GetSystemMetrics`、`IsIconic` evidenceで意味契約とcaller
consumerは固定済みなので、新しいnative traceは追加していない。Primary sequence、address、
occurrenceはartifact identityであり、handler selectorではない。

Canonical Node replayは283,412 completed calls、445,134,242 instructions、100,565 clock
samples、41 completed TIMERPROC callbacks、9,035 pacing completionsを記録する。Ordered
semantic digestは
`81bbe99a26372d63efc1ad4318bebcd4387b9461d7e666de3e1975ea6a8a16a2`、terminal memory
digestは
`1354319d15490d41a85e6468c58ebc39f39e9ac0f55a720dd3aab71a7448e6df`である。

Chrome Worker protocol v132／BGI report schema 97は、同じ20-completion batch、第三のpointer
receipt、message ID 44、terminal endpoint、semantic／memory digestを照合し、cancel／watchdog
cleanupも維持する。Artifact identityと次stopは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher --locked
node tools/test-runtime-worker-evidence-level.mjs
npm run check:runtime:bgi -- --profile user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Pending `WM_LBUTTONDOWN` ID 44の`PeekMessageA`／`TranslateMessage`／`DispatchMessageA`消費は含めない。
- Visible gameplay response、pointer capture／move／wheel、focus transition、DirectInput breadthを証明しない。
- 18 metric callsは到達した`7,4,8` variantだけで、任意display topology、DPI、multi-monitorを一般化しない。
- Readable title、audio、save／load、representative flow、long-run、full-playを証明しない。
