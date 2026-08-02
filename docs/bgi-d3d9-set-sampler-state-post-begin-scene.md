# BGI post-`BeginScene` `SetSamplerState` profile

## Outcome

Primary BGIの4回目のsceneで到達した`IDirect3DDevice9::SetSamplerState` slot 69を、
既存のRust-owned typed sampler-state contractで完了した。Sequence 282,851／282,852は
sampler 0へstate 6 (`D3DSAMP_MINFILTER`)／state 5 (`D3DSAMP_MAGFILTER`) とvalue 2
(`D3DTEXF_LINEAR`)を設定し、いずれも`D3D_OK`、zero guest writes、reference count 5を返す。
既に同値のlinear stateであるためlogical device generationは71のままで、active scene sequence 4、
back-buffer content generation 10、Clear sequence 4、presented frame sequence 3も保持する。

続くsequence 282,853..282,855の`SetRenderState` slot 57はstate／value
`171/1`、`20/6`、`19/5`を`D3DERR_NOTAVAILABLE` (`0x8876086A`)で拒否する。
三件ともgraphics snapshotとguest writesをpublishせず、sampler、render state、device generation、
COM lifetimeを変更しない。Replayは392,353,994 instructionsで次のowned endpoint
`SetTextureStageState(0,1,4)` slot 67、return address `0x1005B1F5`、stack pointer
`0x006FB864`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice`のsparse typed sampler mapが唯一のsampler-state authorityである。COM gatewayは
live device identityとreference countを解決し、graphics ownerがsampler／state／value、generation、
lifecycleをmutation-free planで検証した後だけtargeted commitする。同値設定はsuccessだがgenerationを
進めない。未profileのrender stateはmutation前にtyped HRESULTへ変換し、TypeScriptはWindows device、
COM、sampler／render stateの第二authorityを持たない。

このcheckpointは到達したMIN／MAG linear pairと三件のbounded rejectionだけをclaimする。
一般sampler／render-state permissiveness、後続texture-stage mutation、scene draw／end／present、
可読title sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,855 supported calls、392,353,994 instructions、100,556 clock
samples、40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`1e4e938bf53b87deee21506c39e3d510ad53971a71a23f5133c327ba6e0e4a3d`、terminal memory
digest `f6b4db633a16a90781587cf86ae16d4ad72cc6718230ca5f0c2a9076381261af`
を再現する。Chrome Worker protocol v111／BGI report schema 77は同じordered calls、rejections、
device state、next endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを
残さない。Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-set-sampler-state-post-begin-scene
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-set-sampler-state-post-begin-scene --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
