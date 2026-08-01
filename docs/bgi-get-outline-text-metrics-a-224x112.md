# BGI GetOutlineTextMetricsA 224 x 112 profile

## Outcome

Primary BGIの448 x 336 DIB作成とbitmap／font selection後に到達する、224 x 112
CP932 `ＭＳ ゴシック` selected-fontの`GetOutlineTextMetricsA`二呼出しを
GDI32 compatibility policy version 9の二件目のexact outline-metrics profileとして
実装した。NULL outputのsize queryと269-byte full outputはいずれも269を返し、
LastError 0を保存する。Metricsはversioned Rust policy、selected object relationは
`RuntimeGdi`、output bytesは`GuestMemory`がそれぞれ一意に所有し、host／browser
font measurementは使わない。

同じNode／Chrome replayは三回目のPresentまで進み、instruction 375,285,853の
`USER32.dll!GetCursorInfo` endpoint 168でstructured stopする。したがってこの
outline-metrics variantは、それ以前のthird-Present停滞の近因ではない。ただし
三回目のPresentが可読なtitle sceneであることはまだclaimしない。

## Bounded contract and failure ordering

Profile `primary-cp932-ms-gothic-224x112-outline-metrics-v1`は、creation profile
`primary-cp932-ms-gothic-224x112-thin-v1`から作られたlive nonshared FontAが、
same-thread memory DCに選択されている場合だけ解決される。Required sizeは269、
fixed recordは212 bytes、family／face／style／full-name offsetsは
212／226／240／245である。Full outputのcanonical SHA-256は
`61c4984d45246eb696425105af0908561546b01acf02a4242fe2c9f6cf884adc`である。

DispatcherはHDC、selected font identity、creation-to-metrics profile relation、
requested range、writable outputをすべてpreflightしてからguest bytesだけをcommitする。
NULL size queryはwriteを行わない。Unsupported font variant、stale／wrong-thread HDC、
stock font、malformed output、disabled policyはGDI collection、allocator、guest bytesを
部分更新しない。既存64 x 32 profileは独立したcatalog entryであり、callsite、RVA、
native handle、mapped addressをprofile選択に使わない。

この追加は224 x 112 fontのglyph raster、一般font substitution、任意のoutline metric、
host FontFace、browser bitmap resource、可読画面を一般化しない。

## Native and static oracle

Ignored Frida 17.15.4 script `.work/get-outline-text-metrics-224x112-native-oracle.py`
は8,990 bytes、SHA-256
`f2eefe715e9a47720e9113ecff30bae321cc31f1feeb3244fe2568bc0e105249`である。
二回のreportは13,088／13,080 bytes、SHA-256
`6fd2c5abe5bd98cbee5ee8e12c5d2438c50ed1924b0db609a37e04ed2c5efed3`／
`fd96fb2eb678e592abd2b33c26202eaf1b162f70281ba6d8363b4eae70978ae5`
で、source executable hashは不変だった。両runはsize／full callのreturn 269、
LastError 0、caller return RVA `0x0002C51F`／`0x0002C539`、raw output SHA-256
`62452e3bb6959c14d2ad749d3c58d34afd056fe32d0d678958fc513675b74907`
を一致して観測した。

Native run由来のpadding offsets 57／58／59／71はsemantic valueではないためzeroへ
canonicalizeする。Static decompileのconsumer orderは`CreateFontA ->
CreateCompatibleDC -> CreateDIBSection -> GetObjectA -> SelectObject(bitmap) ->
SelectObject(font) -> GetOutlineTextMetricsA(size) -> GetOutlineTextMetricsA(full)`である。

## Deterministic acceptance

Canonical Node reportは282,388 supported calls、375,285,853 instructions、100,550
clock samples、40 TIMERPROC callbacks、3 Present、semantic digest
`9a25b07001da053004f9007b1f1ed546c686a6a2b71e3ad1a0f1f3586067561e`、
terminal memory digest
`cb8497abf612962fe1d7f6cef19d62e17d786daa3018a666d2f88096b5f10312`
を再現する。Metrics callsはsequence 261,036／261,038、instruction
342,875,244／342,875,297、HDC `0x4701000b`、font `0x47020006`、bitmap
`0x47020007`、selection generation 2、output `0x20ffd9f8`である。

Chrome Worker protocol v107／report schema 73は同じordered semantics、269-byte
output、third Present、next endpoint、terminal memoryを再現し、cancel／watchdog後に
pending host call／callbackを残さない。Artifact hash等の現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --lib gdi32::tests --locked
cargo test -p vine-core --test gdi32_dispatcher --locked
npm run check:runtime:bgi -- --profile gdi-outline-text-metrics-224x112
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile gdi-outline-text-metrics-224x112 --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
```
