# BGI post-FVF `SetStreamSource` profile

## Outcome

Primary BGIの4回目のsceneでFVF設定に続いて到達した、
`IDirect3DDevice9::SetStreamSource` slot 100を完了した。Sequence 282,864、occurrence 4は
stream 0へvertex buffer `0x01050004`、offset 0、stride 28を設定し、`D3D_OK`、zero guest
writes、reference count 5を返す。Device-owned stream sourceは既にlogical vertex buffer 3／
offset 0／stride 28なので同値repeatとなり、logical device generationは71のままである。
Active scene sequence 4、stage-0 texture 8、typed FVF `xyz-rhw-diffuse-texture1`、back-buffer
content generation 10、Clear sequence 4、presented frame sequence 3も保持する。

Call trapは392,354,072 instructions、return address `0x1005B3EC`、stack pointer
`0x006FB860`である。Replayは9 instructions後の392,354,081で、次のowned endpoint
`DrawPrimitive(D3DPT_TRIANGLESTRIP,0,2)` slot 81、return address `0x1005B402`、stack
pointer `0x006FB864`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice.stream_sources.stream_zero`が唯一のstream-binding authorityである。COM
gatewayはlive device／vertex-buffer identityとsame-device ownershipを解決し、graphics ownerがstream、
offset、stride、両resource generationをmutation-free planで検証した後だけtargeted commitする。Bindingは
non-owningでAddRefせず、同値設定はsuccessだがgenerationを進めない。Unsupported variantは
`D3DERR_NOTAVAILABLE`でmutation前に失敗し、NULL unbindもこの到達policyへ昇格しない。TypeScriptは
ordered completionとRust snapshotを検証するだけで、Windows deviceやstream stateの第二authorityを
持たない。

このcheckpointは到達したstream 0／same-device vertex buffer／offset 0／stride 28の同値設定だけをclaimする。
他stream、offset、stride、NULL unbind、後続draw mutation、scene end／present、可読title sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,864 supported calls、392,354,081 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`ef4d1c9b29d88f7d127800267441db75b590a6b4e691ac58135fac72d62aa5c0`、terminal memory digest
`c2b2df9f5af263a2445e683b9d991703da29581486dc17d40fffacdc3bf7cf8b`を再現する。
Chrome Worker protocol v115／BGI report schema 81は同じordered call、non-owning stream binding、device
state、next endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。
Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-set-stream-source-post-fvf
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-set-stream-source-post-fvf --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
