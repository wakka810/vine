# BGI post-raster-status `Present` profile

## Outcome

Primary BGIの4回目のsceneで、post-EndScene raster sampleに続く
`IDirect3DDevice9::Present(NULL,NULL,NULL,NULL)` slot 17を完了した。Sequence
282,868、occurrence 4、trap instruction 392,354,183でdevice `0x01010004`は
`D3D_OK`とreference count 5を保持する。Rust device generationは74へ進み、back-buffer
content generation 11をpresented frame sequence 4としてpublishした後、discard modeのempty
back buffer generation 12へ進む。Frameはclear sequence 4、scene／submission sequence 4、112-byte
vertex snapshot、texture 8 content generation 4、そのRust GuestMemory pixel source 8,388,608 bytesを
保持する。

Replayは7 instructions後の392,354,190で、次のowned endpoint
`WINMM.dll!timeGetTime`、endpoint `0x7F000070`、IAT `0x100D14E4`、return address
`0x10091EBF`、stack pointer `0x006FB8A0`を記録する。

## Authority and failure ordering

Rust `RuntimeGraphics`だけがlogical device、closed back buffer、draw submission、frame sequence、
content generation、discard transitionを所有する。Present gatewayはlive COM relation、inactive scene、
NULL source／destination／override／dirty-region arguments、frame capacity、全generationをmutation-free
planで検証し、presented-frame publishとempty-back-buffer transitionを一つのtargeted commitとして行う。
Invalid argument、active scene、stale identity、capacity／generation exhaustionはframe、back buffer、COM
refcount、guest memoryを部分更新しない。

Workerはguest resume前にRust snapshotを検証し、既存の`browser-resource:1` WebGL2 device relationを
non-owningに再利用する。8,388,608 texture bytesと112 vertex bytesを同期materializeし、frame 4の
publication後にbrowser bindingを公開する。Browser resource tableはWebGL objectとcanvasだけを別namespaceで
所有し、Windows device／COM／frame stateの第二authorityを持たない。Capacity preflightとbinding publishの
orderingをreceiptに含め、WebGL error 0とresume前renderを検証する。

Center pixel receiptは`[0,0,0,255]`である。Browser publicationが成立したことはclaimするが、可読title
scene、visible input response、resize、partial／redirected Present、context／device loss recovery、後続frameの
一般化はclaimしない。

## Deterministic acceptance

Canonical Node reportは282,868 supported calls、392,354,190 instructions、100,556 clock samples、
40 TIMERPROC callbacks、4 Present、9,002 timer-pacing completions、semantic digest
`6ea6d734b05727424bd80ce6e7b0604a3a6ede3908552b72f9e1088c291e2afc`、terminal memory digest
`3ac81c564469a3818fd7e5966703e6d31c75969d8217553ea57979f832d2794c`を再現する。
Chrome Worker protocol v119／BGI report schema 85は同じlogical call／frame／terminal memoryに加え、
resource reuse、upload byte counts、black center pixel、resume前publicationを検証し、cancel／watchdog cleanup後に
pending host call／callbackを残さない。Raw generationとwriteback batchはadapter固有であり、ordered semanticsと
terminal memoryをcross-adapter equalityとする。Artifact identityの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
npm run check:runtime:bgi -- --profile d3d9-present-post-raster-status
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile d3d9-present-post-raster-status --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
