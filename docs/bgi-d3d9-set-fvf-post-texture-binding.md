# BGI post-texture-binding `SetFVF` profile

## Outcome

Primary BGIの4回目のsceneでstage-0 texture bindingに続いて到達した、
`IDirect3DDevice9::SetFVF` slot 89を完了した。Sequence 282,863、occurrence 4は
FVF `0x144`をtyped `xyz-rhw-diffuse-texture1`へ解釈し、`D3D_OK`、zero guest
writes、reference count 5を返す。Device-owned FVFは既に同じtyped valueなので同値repeatとなり、
logical device generationは71のままである。Active scene sequence 4、stage-0 texture 8、
stream-0 vertex buffer 3／offset 0／stride 28、back-buffer content generation 10、Clear
sequence 4、presented frame sequence 3も保持する。

Call trapは392,354,055 instructions、return address `0x1005B291`、stack pointer
`0x006FB86C`である。Replayは17 instructions後の392,354,072で、次のowned endpoint
`SetStreamSource(0,0x01050004,0,28)` slot 100、return address `0x1005B3EC`、stack
pointer `0x006FB860`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice`のtyped flexible-vertex-format fieldが唯一のFVF authorityである。COM
gatewayはlive device identityを解決し、graphics ownerがraw value、policy admission、generation、
scene lifecycleをmutation-free planで検証した後だけtargeted commitする。`0x144`はversioned
policyでtyped valueへ変換し、raw DWORDを別authorityとして保持しない。同値設定はsuccessだが
generationを進めず、unsupported valueは`D3DERR_NOTAVAILABLE`でmutation前に失敗する。TypeScriptは
ordered completionとRust snapshotを検証するだけで、Windows deviceやFVF stateの第二authorityを
持たない。

このcheckpointは到達した`0x144`の同値設定だけをclaimする。他FVF、vertex declaration、後続
stream-source mutation、scene draw／end／present、可読title sceneはclaimしない。

## Deterministic acceptance

Canonical Node reportは282,863 supported calls、392,354,072 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`20fc814e8eb2ea9f4e6f7edae295e3f9c92d65c49486fe4e1834b459d7f6bdd5`、terminal memory digest
`50a490bd1d6f0db5bd299c9bc3448644232e528102bc359a94fa7f691f77cab5`を再現する。
Chrome Worker protocol v114／BGI report schema 80は同じordered call、typed FVF、device state、
next endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。
Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-set-fvf-post-texture-binding
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-set-fvf-post-texture-binding --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

Continuation: [post-FVF `SetStreamSource`](bgi-d3d9-set-stream-source-post-fvf.md).
