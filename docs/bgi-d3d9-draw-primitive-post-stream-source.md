# BGI post-stream-source `DrawPrimitive` profile

## Outcome

Primary BGIの4回目のsceneでstream-source設定に続いて到達した、
`IDirect3DDevice9::DrawPrimitive` slot 81を完了した。Sequence 282,865、occurrence 4は
`D3DPT_TRIANGLESTRIP`、start vertex 0、primitive count 2を指定し、`D3D_OK`、zero guest
writes、reference count 5を返す。Rust-owned graphics authorityはactive scene sequence 4、typed
FVF `xyz-rhw-diffuse-texture1`、stream-zero vertex buffer 3／offset 0／stride 28、stage-0
texture 8のcontent generation 4を検証し、112-byte vertex snapshotをowning immutable
submission sequence 4としてback bufferへcommitする。Logical device generationは72、back-buffer
content generationは11へ進む。

Call trapは392,354,081 instructions、return address `0x1005B402`、stack pointer
`0x006FB864`である。Replayは8 instructions後の392,354,089で、次のowned endpoint
`IDirect3DDevice9::EndScene` slot 42、return address `0x1005B43B`、stack pointer
`0x006FB870`を記録する。

## Authority and failure ordering

`RuntimeGraphicsDevice.back_buffer.draw_submissions`が唯一のlogical draw authorityである。COM
gatewayはlive deviceとactive sceneを解決し、graphics ownerがfixed-function state、resource identity／
generation、queue limit 4,096、累積vertex-byte limit 1 MiBをmutation-free planとpreflightで固定する。
Aggregateはchecked rangeをauthoritative `GuestMemory`から同期readし、graphics ownerがvertex bytes、
render／sampler／texture-stage state、texture descriptor／contentをimmutable snapshotへprepareした後だけ
targeted commitする。Source resource retirement後もsubmissionは成立し、full-target Clearだけが同じchecked
commitでpending submissionsを破棄する。失敗時はguest memory、queue、binding、COM lifetime、generationを
部分更新しない。TypeScriptはordered completionとRust snapshotを検証するだけで、Windows draw stateの
第二authorityを持たない。

このcheckpointは到達したtriangle-strip quadだけをclaimする。Other primitive topology／range／layout、
scene外やrequired state不足、後続EndScene mutation、renderer materialization、visible pixel、可読title sceneは
claimしない。

## Deterministic acceptance

Canonical Node reportは282,865 supported calls、392,354,089 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`c06dd5c964e6b893ae0769763a30d3afcddaf3b196b680f63cd1806bc9301a1b`、terminal memory digest
`9b70297684d908d75432ac97d062d8a1f8f7ab595bd563e4915d48d7f9e60abe`を再現する。Vertex snapshot
SHA-256は`405bea74541d2aece445d0dac693da55fb404e777c91ff81d87d4c6272581362`である。
Chrome Worker protocol v116／BGI report schema 82は同じordered call、immutable submission、device state、
next endpoint、terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。
Artifact hash等の現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-draw-primitive-post-stream-source
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-draw-primitive-post-stream-source --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
