# BGI post-Present `timeGetTime` profile

## Outcome

Primary BGIの4回目のdiscard Presentに続く`WINMM.dll!timeGetTime()`を、既存の
`winmm.time-get-time.v1` semantic handlerで完了した。Callはsequence 282,869、
occurrence 479、trap instruction 392,354,190で、argumentとguest writeはなく、
LastError 0を保持して`3,800,773`を返す。直前のsequence 282,868／instruction
392,354,183のPresentはframe 4をpublish済みであり、Chromeも同じ
`browser-resource:1`をguest resume前にmaterializeしている。

Replayはさらに133 instructions進み、instruction 392,354,323で次のowned endpoint
`IDirect3DDevice9::TestCooperativeLevel` slot 3、endpoint `0x01011060`、this pointer
`0x01010004`、return address `0x1005ADCD`、stack pointer `0x006FF8F4`を記録する。
このcheckpointはtime leafの完了までをclaimし、後続COM callの完了は次の
structured stopへ残す。

## Authority and failure ordering

Sampleは`runtime/process.rs::RuntimeProcessState`が所有するprocess clockから取得し、
Kernel32 `GetTickCount`と同じprojectionを使う。Scheduled profileではAPI read自体は
clockを進めないため、returned／next tickはいずれも`3,800,773`、incrementは0である。
同時点のtimer clockは`2,007,730,000` 100 ns units／generation 16,308、scheduled
execution clockはretired instructions 392,354,190、pending 190、2,000,000 instructions/s
である。Timer-resolution table、host uptime、browser clock、WebGL frame clockはこの値へ
importしない。

Handlerはzero-argument stdcall、preserved LastError、zero guest writeの既存contractを
そのまま再利用する。Clock sampleのvalidation後もruntime、timer、graphics、COM、guest
memoryを変更しない。TypeScriptはRust completionと前段のbrowser presentation receiptを
検証するだけで、Windows clockまたはframe stateの第二authorityを持たない。

## Deterministic acceptance

Canonical Node replayは282,869 supported calls、392,354,323 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Presentを完了する。Ordered semantic
digestは`5f224f94086095c739923b53509fe4e33c65fd11d63ec7c352044c42f7705dc8`、
terminal memory digestは
`cdbfdd820c96f520438bfd9eac89c39da2d7662d41cff9c4656765dc4fbec2b8`である。
Chrome Worker protocol v120／BGI report schema 86は同じcall、clock projection、
TestCooperativeLevel境界、ordered semantics、terminal memoryを一致させ、前段Presentの
resource reuse／black center pixel receiptとcancel／watchdog cleanupも保持する。Raw memory
generation、writeback batch、artifact hashの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile winmm-time-get-time-post-present
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile winmm-time-get-time-post-present --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- この一読はhost／browserのmillisecond精度、frame cadence、audio clock、background-tab
  catch-upをclaimしない。
- `timeBeginPeriod`はsample値、scheduler、browser timer、Web Audioを変更しない。
- 後続`TestCooperativeLevel`、device loss／recovery、resize、general subsequent frames、
  readable title scene、visible input responseはこのleafのacceptanceに含めない。
