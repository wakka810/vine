# BGI CreateFontA 224 x 112 variant

## Outcome

Primary BGIのheap-growth後に到達する
`GDI32.dll!CreateFontA(224,112,0,0,100,0,0,0,128,4,0,2,0,"ＭＳ ゴシック")`
を、GDI32 compatibility policy version 7の二件目のexact creation profileとして
実装した。成功は`RuntimeGdi`が所有するdistinct nonshared logical font object ID 6／
handle `0x47020006`を返し、LastErrorを保持し、guest bytesを書かない。Host font、native
HDC、browser `FontFace`／Canvas text stateは作成しない。

実行はその後も進み、次のowned endpointである`GDI32.dll!CreateDIBSection` endpoint 257
でstructured stopする。Presentは2回のままであるため、このlogical font creationもthird
Present停滞の近因ではない。

## Authority and bounded policy

`Gdi32CompatibilityPolicy`が二件のexact `LOGFONTA` profileを一つのcatalogとして所有し、
`RuntimeGdi`がobject collection、monotonic ID／handle allocator、lifetimeを所有する。
Handlerは14-word x86 stdcall frameと最大32-byteのNUL終端Windows-932 faceを検証し、
matching profileとowner capacityをpreflightしてから既存GDI plan／commit boundaryで一件だけ
publishする。Unsupported field、NULL／unterminated／cross-page face、disabled policy、capacity、
stale／duplicate planはidentity、allocator、LastError、guest memoryを変更しない。

既存64 x 32 profileのoutline metricsと`TextOutA` raster contractはprofile IDで明示的に
選択される。224 x 112 profileの追加はmetric、glyph、DIB、browser rasterizationの一般化ではない。

## Native and static oracle

Ignored focused Frida script `.work/create-font-variant-native-oracle.py`は5,850 bytes、
SHA-256 `cf6085dd5e4bc5d439b2f658a134f91145fae3187db16dfc752423af31f9e9b3`、
reportは3,192 bytes、SHA-256
`ca8e7d11f1de4d6e40bc952a441d5ba558117562ebd31f0cfab542b88bcd1680`である。
Frida 17.15.4はcaller return RVA `0x0002C838`でexact arguments、non-NULL HFONT、
entry／exit LastError 0を観測した。Source BGI.exeは1,193,472 bytes、SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`のまま不変である。

`V:\softs\サクラノ詩\decompiled\BGI.exe.c`の`sub_42C5F0`は、返されたhandleを
object+152へ保存し、nonzero branch後に`CreateCompatibleDC(0)`と
`CreateDIBSection`を含むfont／DIB pipelineへ進むconsumerを示す。Runtime handlerはこの
RVAやtitle identityを分岐条件に使わない。

## Deterministic evidence

Node report `vine.v86-bgi-create-font-a-224x112-primary` v1は261,030 supported calls、
342,875,151 instructions、semantic digest
`8058bf7a6ebd37f36c7f486316a8b22faf44368dbc313938dfe4d0eb076c1c13`、
100,534 shared-clock samples、39 TIMERPROC callbacks、7,721 timer-pacing completions、
139 source reads、2 Presentを記録する。Terminal memory digestは
`ae64fe329dde1e281c28e72fd56000e3c64b7e85f6e83119f830600f21840039`である。

Chrome Worker protocol v105／browser report schema 71は同じcall count、semantic digest、
instruction count、logical font completion、next endpoint、terminal memoryを再現し、cancel／
watchdog後にpending host call／callbackを残さない。Adapter generationとwriteback countは
transport evidenceでありguest semanticsではない。

## Acceptance and non-claims

Representative checks:

```powershell
cargo test -p vine-core --lib gdi32::tests --locked
cargo test -p vine-core --test gdi32_dispatcher --locked
npm run check:runtime:bgi -- --profile gdi-create-font-a-224x112
npm run build:runtime:browser-worker:prepared
node tools/run-browser-runtime-check.mjs --bgi
```

このcheckpointはother face／charset／style／size、font substitution、outline metrics、glyph
raster、browser font availability、new DIB variant、third Present、readable title sceneをclaimしない。
