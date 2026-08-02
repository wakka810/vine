# BGI post-sampler `SetTextureStageState` profile

## Outcome

Primary BGIの4回目のsceneでsampler pairと三件のbounded render-state rejectionに続いて到達した、
`IDirect3DDevice9::SetTextureStageState` slot 67の六件を完了した。Sequence 282,856..282,861、
occurrence 19..24はstage 0へstate 1–6、value `4,2,0,2,2,0`を順に設定し、全て
`D3D_OK`、zero guest writes、reference count 5を返す。Color operationは`modulate`、
color argumentsは`texture`／`diffuse`、alpha operationは`select-argument-one`、alpha
argumentsは`texture`／`diffuse`である。

六設定はいずれも既存値のrepeatなので、logical device generationは71のままである。Active scene
sequence 4、sampler-0 MIN／MAG linear、back-buffer content generation 10、Clear sequence 4、
presented frame sequence 3も保持する。Replayは392,354,048 instructionsで次のowned endpoint
`SetTexture(0,0x01030004)` slot 65、return address `0x1005B27C`、stack pointer
`0x006FB868`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice`のsparse typed stage-0 mapが唯一のtexture-stage-state authorityである。COM
gatewayはlive device identityとreference countを解決し、graphics ownerがstage／state／value、
generation、scene lifecycleをmutation-free planで検証した後だけtargeted commitする。同値設定は
successだがgenerationを進めない。TypeScriptはordered completionとRust snapshotを検証するだけで、
Windows device、COM、texture-stage stateの第二authorityを持たない。

このcheckpointは到達したstage 0の六設定だけをclaimする。他stage／state／value、native driverの
追加permissiveness、後続texture binding、scene draw／end／present、可読title sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,861 supported calls、392,354,048 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`c7b07ed41957516d5bb95814070ba858eee5a95e069a8bda53665867222cbb12`、terminal memory digest
`48b7fe34c489b126994490d0819a7a02bb3bfb82da13bcdd8d3d1b0af9697a4c`を再現する。
Chrome Worker protocol v112／BGI report schema 78は同じordered calls、device state、next endpoint、
terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。Artifact hash等の
現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-set-texture-stage-state-post-sampler-state
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-set-texture-stage-state-post-sampler-state --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

Continuation: [post-texture-stage texture-binding profile](bgi-d3d9-set-texture-post-texture-stage-state.md).
