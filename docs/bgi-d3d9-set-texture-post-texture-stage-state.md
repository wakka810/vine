# BGI post-texture-stage `SetTexture` profile

## Outcome

Primary BGIの4回目のsceneでtexture-stage state六件に続いて到達した、
`IDirect3DDevice9::SetTexture` slot 65を完了した。Sequence 282,862、occurrence 4は
stage 0へCOM pointer `0x01030004`のstatic DEFAULT textureをbindingし、`D3D_OK`、
zero guest writes、reference count 5を返す。Device-owned bindingは既にlogical texture 8を
指しているため同値repeatとなり、logical device generationは71のままである。Active scene
sequence 4、back-buffer content generation 10、Clear sequence 4、presented frame sequence 3、
typed FVF `xyz-rhw-diffuse-texture1`、stream-0 vertex buffer 3／offset 0／stride 28も保持する。

Replayは392,354,055 instructionsで次のowned endpoint `SetFVF(0x144)` slot 89、return
address `0x1005B291`、stack pointer `0x006FB86C`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice`のstage-to-texture relationが唯一のtexture-binding authorityである。COM
gatewayはlive device／texture identityとsame-device ownershipを解決し、graphics ownerがstage、
texture ID、generation、scene lifecycleをmutation-free planで検証した後だけtargeted commitする。
Bindingはnon-owningでAddRefを保持せず、texture final retirement時は同じretirement transactionが
残存bindingをclearする。同値bindingはsuccessだがgenerationを進めない。TypeScriptはordered
completionとRust snapshotを検証するだけで、Windows device、COM、texture bindingの第二authorityを
持たない。

このcheckpointは到達したstage-0 static DEFAULT textureの同値bindingだけをclaimする。他stage、
別texture／device relation、NULL unbindの実到達、後続FVF mutation、scene draw／end／present、
可読title sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,862 supported calls、392,354,055 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`88cb3f30ab4ccdec929519e4fd389124d71a53fb19e5142553b3a8e47a74ad79`、terminal memory digest
`0fdb9cf3234a7280e607a8100e98516144ea00e727ccf20f9be73eedf7d05c8d`を再現する。
Chrome Worker protocol v113／BGI report schema 79は同じordered call、binding、device state、next
endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。
Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-set-texture-post-texture-stage-state
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-set-texture-post-texture-stage-state --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
