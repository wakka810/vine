# BGI post-partial `BeginScene` profile

## Outcome

Primary BGIがexact partial dirty-region `UpdateTexture`の直後に実行する4回目の
`IDirect3DDevice9::BeginScene` slot 41を、既存のRust-owned device scene contractで
完了した。Sequence 282,850、trap instruction 392,353,938のcallはdevice
`0x01010004`を渡し、`D3D_OK`を返す。Guest memory、LastError、COM reference countを
変更せず、logical graphics device 3／COM object 21をinactiveからactive scene sequence
4へ進め、device generationを71にする。

Replayは14 instructions後の392,353,952で、次のowned endpoint
`IDirect3DDevice9::SetSamplerState` slot 69を記録する。到達引数はsampler 0、state 6
(`D3DSAMP_MINFILTER`)、value 2 (`D3DTEXF_LINEAR`)、return address `0x1005B18A`、
stack pointer `0x006FB864`である。このcheckpointはscene beginの完了までをclaimし、
後続sampler mutation、scene draw／end／present、可読title sceneはclaimしない。

## Authority and failure ordering

`RuntimeGraphicsDevice`がactive-scene identity、scene sequence、back-buffer state、device
generationの唯一のauthorityである。COM gatewayはlive device identityとreference countを
解決し、graphics ownerがinactive state、generation capacity、scene transitionを
mutation-free planとして検証した後、一回のtargeted commitでscene sequence 4をpublishする。
TypeScriptはWindows device、COM、scene stateの第二authorityを持たず、このleafでbrowser
renderer resourceも作成しない。

Pre-stateは三回目のPresentが保持するpresented frame sequence 3、back-buffer content
generation 10、full-target Clear sequence 4、直前のpartial `UpdateTexture` completion
sequence 282,849で固定する。Stale／wrong-owner device、既にactiveなscene、capacity
exhaustionはscene、back buffer、COM lifetime、GuestMemory、generationを部分更新せず失敗する。
Callsite、EIP、occurrence、report hashはsemantic handlerの分岐条件にしない。

## Deterministic acceptance

Canonical Node reportは282,850 supported calls、392,353,952 instructions、100,556 clock
samples、40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`698f2ffd737999918c14b33fa1d4699c71523ac1b93acc854d103211926e535a`、terminal memory
digest `ec00efea1a7f56c0ef8af45da4c614673c6374265c0cd9ee4b79b884626a14db`
を再現する。直前のpartial `UpdateTexture`は両texture content generation 4とclean source
dirty stateを保持し、`BeginScene`はzero guest writes、reference count 5、callback depth 0で
完了する。

Chrome Worker protocol v110／BGI report schema 76は同じordered semantics、device state、
completion、次のslot 69 endpoint、terminal memoryを再現し、cancel／watchdog後にpending
host call／callbackを残さない。Artifact hash等の現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-begin-scene-post-partial-dirty-region
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-begin-scene-post-partial-dirty-region --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

Continuation: [post-`BeginScene` sampler-state profile](bgi-d3d9-set-sampler-state-post-begin-scene.md).
