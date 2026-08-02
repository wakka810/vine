# BGI post-DrawPrimitive `EndScene` profile

## Outcome

Primary BGIの4回目のsceneでimmutable draw submissionに続いて到達した、
`IDirect3DDevice9::EndScene` slot 42を完了した。Sequence 282,866、occurrence 4は
device `0x01010004`に対して`D3D_OK`、zero guest writes、reference count 5を返す。
Rust-owned graphics authorityはactive scene sequence 4をclearし、logical device generationを
72から73へ進める一方、back-buffer content generation 11、Clear sequence 4、immutable draw
submission／scene sequence 4、texture 8のcontent generation 4、presented frame sequence 3を保持する。

Call trapは392,354,089 instructions、return address `0x1005B43B`、stack pointer
`0x006FB870`である。Replayは57 instructions後の392,354,146で、次のowned endpoint
`IDirect3DDevice9::GetRasterStatus` slot 19、endpoint `0x01011160`、return address
`0x1005AEDD`、stack pointer `0x006FB878`、output pointer `0x006FB890`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice.active_scene`が唯一のlogical scene-lifecycle authorityである。COM gatewayは
live device identityとactive sceneを解決し、graphics ownerがdevice／scene／owner generationを
mutation-free planへ固定する。全generationとlifecycleをpreflightした後だけtargeted commitし、
`active_scene`だけをclearしてdevice／graphics generationを進める。Back-buffer draw／clear content、
resource binding、COM lifetime、guest memoryは変更しない。Outside／repeated EndScene、unknown identity、
stale／duplicate planはmutation前に失敗し、scene state、allocator、queue、generationを部分更新しない。
TypeScriptはordered completionとRust snapshotを検証するだけで、Windows scene stateの第二authorityを
持たない。

このcheckpointは到達したactive-to-inactive transitionだけをclaimする。Other scene lifecycle variant、
後続GetRasterStatusのoutput semantics、Present、renderer translation／materialization、visible pixel、可読title
sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,866 supported calls、392,354,146 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`b7b087f10c4b670bbbcdeabf9db36d5723d965a344d0f0bc0d78505495d93953`、terminal memory digest
`876429740fedd52bf01c21cebd1cc1e912182c4df317d2109fcff631ed49b664`を再現する。
Chrome Worker protocol v117／BGI report schema 83は同じordered call、closed scene、preserved submission、
device state、next endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを
残さない。Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-end-scene-post-draw-primitive
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-end-scene-post-draw-primitive --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

Continuation: [post-EndScene GetRasterStatus profile](bgi-d3d9-get-raster-status-post-end-scene.md).
