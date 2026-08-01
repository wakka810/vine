# BGI partial dirty-region UpdateTexture profile

## Outcome

Primary BGIが三回目のPresentと二度の`GetCursorInfo`後に実行する
`IDirect3DDevice9::UpdateTexture` slot 31を、既存のRust-owned texture byte／content
authorityへ追加したexact profileとして完了した。Sequence 282,849、instruction
392,353,932のcallはdevice `0x01010004`、SYSTEMMEM source `0x01020004`、static
DEFAULT destination `0x01030004`を渡し、dirty rectangle
`[583,530,1260,582]`を`D3D_OK`でconsumeする。Guest outputとLastErrorを変更しない。

同じNode／Chrome replayはcallから6 instructions後、instruction 392,353,938の
`IDirect3DDevice9::BeginScene` slot 41をnext owned endpointとして記録して
capability checkpointを完了する。したがってこのpartial transferはそれ以前の進行停止の
近因ではない。新しいsceneの完了、可読title scene、一般的なpartial upload、renderer
更新はこのcheckpointではclaimしない。

## Bounded contract and failure ordering

Compatibility policy `d3d9-primary-virtual-adapter-v33`は、従来のexact rectangle
`[0,0,1280,720]`に加え、今回到達した`[583,530,1260,582]`だけを
`primary-recreated-system-memory-to-static-default-texture-update-v3`へ許可する。
Callsite、EIP、occurrence、report hashをsemantic handlerの分岐条件にしない。

`RuntimeGraphics`はsource／destination identity、same-device relation、SYSTEMMEM-to-static
DEFAULT role、descriptor compatibility、source dirty state、destination lineage、全generation
capacityをmutation前に検証する。Destinationがsourceの直前content generationを表す場合だけ
full authoritative source backingを新しいpixel sourceとしてpublishし、destination contentと
source dirty-region consumptionを一回のtwo-texture targeted commitで確定する。今回の完了後は
両textureのcontent generationが4、source dirty stateがclean、destination update sequenceが4に
なる。GuestMemoryのallocation 77／owner 50／range `0x44001000 + 0x00800000`がbyte authorityで、
TypeScriptはWindows texture stateやdirty trackerを複製しない。

Wrong role／owner／shape、stale identity／generation、clean source、lineage mismatch、許可されて
いないrectangle、capacity exhaustionはCOM identity、texture content、dirty region、allocator、
GuestMemory、generationを部分更新せず失敗する。Policy v32は新しいrectangleを拒否し、同じ
pre-stateにpolicy v33を適用した場合だけ成功することをmodel／dispatcher integration testで固定する。
Arbitrary partial history、dynamic destination、partial byte copy、browser-side texture ownershipは
profile外である。

## Deterministic acceptance

Canonical Node reportは282,849 supported calls、392,353,938 instructions、100,556 clock
samples、40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`75b983dd6bff893961769349c74bff97ebaf7b4c33a1eab229b215fa98cd58b6`、
terminal memory digest
`568061b7a8f6722b0556c6f9add62e5f9b35e62591b1769c16a441250331329e`
を再現する。Source texture 7／COM object 22はgeneration 54、destination texture 8／COM
object 24はgeneration 48で、callはzero guest writes、reference count 5、active callback depth
0を記録する。

Chrome Worker protocol v109／BGI report schema 75は同じordered semantics、texture state、
completion、next BeginScene endpoint、terminal memoryを再現し、cancel／watchdog後にpending
host call／callbackを残さない。Artifact hash等の現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test d3d9_dispatcher --locked
npm run check:runtime:bgi -- --profile d3d9-update-texture-partial-dirty-region
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-update-texture-partial-dirty-region --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
