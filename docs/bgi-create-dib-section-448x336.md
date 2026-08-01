# BGI CreateDIBSection 448 x 336 profile

## Outcome

Primary BGIの224 x 112 logical font作成後に到達する448 x -336、8-bpp
`CreateDIBSection` requestを、GDI32 compatibility policy version 8の二件目の
exact DIB profileとして実装した。Successはlogical bitmap object ID 7／handle
`0x47020007`とbits `0x44801000`を返す。Descriptor／object lifetimeは
`RuntimeGdi`、150,528 logical pixel bytesは`GuestMemory`、151,552-byte
page-rounded mappingは`ProcessAddressSpaceAllocator`が所有し、host HBITMAPや
browser bitmap resourceは作成しない。

同じrunで24-byte `GetObjectA`とbitmap／fontの`SelectObject`も完了し、次の
`GDI32.dll!GetOutlineTextMetricsA` variantでstructured stopする。Presentは2回の
ままなので、このDIB requestもthird Present停滞の近因ではない。

## Bounded contract and failure ordering

Profile `primary-448x-336-8bpp-reverse-grayscale-v1`は40-byte header、width 448、
height -336、planes 1、8 bpp、`BI_RGB`、256 used colors、exact reverse-grayscale
RGBQUAD palette、`DIB_RGB_COLORS`、NULL section、offset zeroだけを受理する。
Stride 448、logical bytes 150,528、mapped bytes 151,552はchecked derived valuesで
ある。HDC relation、descriptor、object identity、address allocation、memory owner／
mapping、zero backing、bits-output pointerをすべてpreflightしてからtargeted commit
する。448 x -335、palette mismatch、disabled policy、capacity、mapping、stale planは
GDI collection、allocator、guest bytes、LastErrorを部分更新しない。

既存128 x -96 profileと`TextOutA` contractはprofile IDで明示的に選択される。
この追加は一般DIB format、outline metrics、glyph rasterization、font substitution、
screen drawingの一般化ではない。

## Native and static oracle

Ignored Frida 17.11.0 script `.work/create-dib-section-variant-native-oracle.py`は
11,014 bytes、SHA-256
`3d4e66b6f10bb91e7c367c5c7a20e05d2dfc4cf05ae4d93d1fe2ef9bda0c0350`である。
二回のreportは44,250／44,251 bytes、SHA-256
`651c13f6bc155873697ff7d97fba4a3a2bf429403630489d16be3ae4ee8e9163`／
`6e4080b0b5dec0824841bb2b323ee80e2ee184b8c16f241f4e4217e28c0383fd`で、
source executable hashは両runで不変だった。両runはexact header／palette、initial
zero、24-byte `BITMAP`、二回のselection、LastError 0を一致して観測した。

`V:\softs\サクラノ詩\decompiled\BGI.exe.c`の`sub_42C5F0`は
`CreateFontA -> CreateCompatibleDC -> CreateDIBSection -> GetObjectA ->
SelectObject(bitmap) -> SelectObject(font) -> GetOutlineTextMetricsA`のconsumer orderを
示す。RuntimeはRVA、native handle、mapped addressをpolicy分岐へ使わない。

## Deterministic acceptance

Canonical Node reportは261,035 supported calls、342,875,244 instructions、semantic
digest `c10dd21fad419df594de40931412f9b823b7fbb59e5e44939c447d0d9db046c2`、
terminal memory digest
`a38022e80408a90e87d79e5c9ebec54389eb4c8331bd511889a04315266f4e00`を再現する。
Chrome Worker protocol v106／report schema 72は同じordered semantics、次endpoint、
terminal memoryを再現し、cancel／watchdog後にpending host call／callbackを残さない。
Artifact hash等の現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --lib gdi32::tests --locked
cargo test -p vine-core --test gdi32_dispatcher --locked
npm run check:runtime:bgi -- --profile gdi-create-dib-section-448x336
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile gdi-create-dib-section-448x336 --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
```
