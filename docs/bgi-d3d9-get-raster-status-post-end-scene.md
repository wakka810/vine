# BGI post-EndScene `GetRasterStatus` profile

## Outcome

Primary BGIの4回目のscene close直後に到達した
`IDirect3DDevice9::GetRasterStatus(0, 0x006FB890)` slot 19を完了した。
Sequence 282,867、occurrence 9,080、trap instruction 392,354,146で、device
`0x01010004`は`D3D_OK`、reference count 5を保持し、8-byte
`D3DRASTER_STATUS`を`0000000037040000`として一回だけ書く。これは
`InVBlank=FALSE`、`ScanLine=1079`であり、versioned 1920x1080 virtual displayの
last-visible-line compatibility sampleである。

Replayは37 instructions後の392,354,183で、次のowned endpoint
`IDirect3DDevice9::Present(NULL,NULL,NULL,NULL)` slot 17、endpoint
`0x01011140`、return address `0x1005B7B1`、stack pointer `0x006FB890`を記録する。

## Authority and failure ordering

`d3d9-primary-virtual-adapter-v33`と
`primary-stable-last-visible-scanline-v2`がsample semanticsを所有し、Rustの
D3D9 compatibility policyだけが値を構成する。COM gatewayはlive device relation、swap-chain
index 0、display profile、8-byte guest output spanをmutation前に検証する。全spanをpreflightした後に
一つのatomic guest writeをcommitし、LastError、graphics state、device／COM generation、reference
countを変更しない。NULL／unwritable output、unsupported swap-chain、stale／retired identity、display
authority不整合はguest bytesやruntime stateを部分更新せず失敗する。

Host raster counter、browser scanout、renderer stateはimportしない。TypeScriptはRust completionと
guest writeをordered evidenceとして検証するだけで、Windows raster-status stateの第二authorityを
持たない。このcheckpointは後続Presentを実行せず、renderer materialization、visible pixel、可読title
sceneをclaimしない。

## Deterministic acceptance

Canonical Node reportは282,867 supported calls、392,354,183 instructions、100,556 clock samples、
40 TIMERPROC callbacks、3 Present、9,002 timer-pacing completions、semantic digest
`548a7a119d932b44b360988cdecf53367e292766d0136ebb1b174568808b14d0`、terminal memory digest
`3e2ad4caff37cab8e510e3bdeb5bca8a7376236fe3820e50d64411f14bd296cf`を再現する。
Chrome Worker protocol v118／BGI report schema 84は同じcall、atomic output、unchanged
graphics／COM state、次のPresent endpoint、terminal memoryを再現し、cancel／watchdog cleanup後に
pending host call／callbackを残さない。NodeのCOM occurrence 9,080とWorker sessionのcumulative
completion 9,076は別counterであり、各namespace内のexact valueとして検証する。Artifact identityの
現在値は[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-get-raster-status-post-end-scene
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-get-raster-status-post-end-scene --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

Continuation: [post-raster-status Present profile](bgi-d3d9-present-post-raster-status.md).
