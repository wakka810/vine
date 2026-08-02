# BGI post-`timeGetTime` `TestCooperativeLevel` profile

## Outcome

Primary BGIの4回目のPresent後の`WINMM.dll!timeGetTime`に続く
`IDirect3DDevice9::TestCooperativeLevel()` slot 3を、既存のRust D3D9 semantic
handlerで完了した。Callはsequence 282,870、occurrence 70、trap instruction
392,354,323、this pointer `0x01010004`であり、`D3D_OK`を返す。Return addressは
`0x1005ADCD`、stack pointerは`0x006FF8F4 -> 0x006FF8FC`、device COM refcountは
5のまま、guest writeは0 bytesである。

Replayはさらに46 instructions進み、instruction 392,354,369で次のowned endpoint
`KERNEL32.dll!SetWaitableTimer`、endpoint `0x7F000750`、IAT `0x100D1280`、
return address `0x10050607`、stack pointer `0x006FF8D4`を記録する。Reached argumentsは
timer handle `0x57010001`、due-time pointer `0x006FF8F0`、period／completion routine／
context／resumeがすべて0である。このcheckpointはcooperative-level queryの完了までを
claimし、timer armは次のstructured stopへ残す。

## Authority and failure ordering

QueryのauthorityはRust `RuntimeGraphics`が所有するlive logical device associationである。
Reached deviceはlogical device ID 3、COM object ID `21`、device generation 74で、
compatibility policy `d3d9-primary-virtual-adapter-v33`の
`primary-live-operational-device-query-v1`を使う。Handlerはassociationとlifetimeを検証して
`D3D_OK`を返すだけで、graphics state、COM refcount、guest memory、LastErrorを変更しない。

Unknown／retired／wrong-interface relationとdisabled policyはmutation前に拒否される。
TypeScriptはRust completion、前段frame-4 presentation receipt、次endpointを検証するだけで、
Windows device stateまたはCOM lifetimeの第二authorityを持たない。Browser context lossを
guest device-loss stateとしてimportしない。

## Deterministic acceptance

Canonical Node replayは282,870 supported calls、392,354,369 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Presentを完了する。Ordered semantic
digestは`2f4a7a85018f0270db87a750ff2a84adede1ffa3fb55cde9868469aef1d8e8fe`、
terminal memory digestは
`dfb7685d08834dc611a85984c679c0f3a8ea64e32a9601e6c6dd4ec2ed8aade7`である。
Query自体はzero-writeだが、次のtimer callへ進む46 guest instructionsがdue-time stack bytesを
更新するため、terminal memory digestは直前checkpointと異なる。

Chrome Worker protocol v121／BGI report schema 87は同じcall、device identity、refcount、
zero-mutation contract、SetWaitableTimer境界、ordered semantics、terminal memoryを一致させる。
前段Presentの`browser-resource:1`／black center pixel receiptとcancel／watchdog cleanupも保持する。
Raw memory generation、writeback batch、artifact hashの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-test-cooperative-level-post-time-get-time
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-test-cooperative-level-post-time-get-time --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- `D3DERR_DEVICELOST`、`D3DERR_DEVICENOTRESET`、`Reset`、resize、asynchronous
  browser context loss／recoveryはこのprofileで実装またはacceptしていない。
- Black center pixelはpublication receiptであり、readable title sceneまたはvisible input
  responseを証明しない。
- 後続`SetWaitableTimer`、wait cycle、general subsequent frames、long-run stabilityは次の
  checkpoint以降へ残す。
