# BGI GetCursorInfo profile

## Outcome

Primary BGIが三回目のPresent後に二度呼ぶ`GetCursorInfo`を、既存のUser32／input
authorityから組み立てるread-only projectionとして実装した。成功時のx86
`CURSORINFO`は20 bytesで、`cbSize = 0`、`flags = CURSOR_SHOWING`、既にload済みの
system-arrow HCURSOR `0x43010001`、Rust-owned cursor point `(960, 540)`を返す。
LastErrorは保存し、logical cursor identity、cursor position、guest bytesのどれも
API自身は変更しない。

このhistorical Node／Chrome replayは二呼出しを完了し、instruction 392,353,932の
`IDirect3DDevice9::UpdateTexture` slot 31で、当時のpolicyがまだ受理しないpartial
dirty-region variantとしてstructured stopした。そのvariantは続く
[partial dirty-region UpdateTexture profile](bgi-d3d9-update-texture-partial-dirty-region.md)
で完了済みである。`GetCursorInfo`はそれ以前の進行停止の近因ではない。可読title
sceneやvisible input responseはこのcheckpointではclaimしない。

## Bounded contract and failure ordering

Implementation `user32.get-cursor-info.system-arrow.v1`は、User32 policyが許可する
`IDC_ARROW` sourceを`RuntimeUser32`のsource indexで解決し、`RuntimeInput`のconfigured
cursor pointを読む。Logical HCURSORとPOINTをTypeScript、DOM、native hostへ複製せず、
WorkerはRust completion evidenceだけを観測する。

Dispatcherはnon-NULL pointer、readable `cbSize`、exact size 20、live system-arrow
identity、configured cursor point、LastError accessを検証してから20-byte output rangeを
一括commitする。NULLはFALSE／`ERROR_NOACCESS`、size 0／19／21は
FALSE／`ERROR_INVALID_PARAMETER`でoutputを保存する。Unreadable／partially writable range、
missing cursor、unconfigured point、disabled cursor policyはguest bytes、cursor collection、
input generationを部分更新しない。Callsite、RVA、native HCURSOR、native cursor pointは
handlerの分岐条件にしない。

このprofileは他のpredefined／module cursor、active cursor selection、`SetCursor`、
cursor suppression、animation、CSS cursor、browser pointer position、multi-monitor
coordinate conversionを一般化しない。

## Native and static oracle

Ignored Frida 17.15.4 script `.work/get-cursor-info-native-oracle.py`は7,245 bytes、
SHA-256
`a24c6ed9057e86c9ffad018100517fbd9187f58dc80a1504770d7c3c513a3355`である。
二回のreportは7,933／7,935 bytes、SHA-256
`000ec313e08b21236c9db1c76f7f36a6883efe7a011bb0d9fc0c57f497b8f12a`／
`c491f80227ba72a6c1c2004dcca40ed5acb17217289895f2017434a0f6ee6cdf`
で、source executable hashは不変だった。

両runはcaller return RVA `0x00088C98`、input `cbSize = 20`、TRUE、output先頭の
`cbSize = 0`、`CURSOR_SHOWING`、success LastError preserveを一致して観測した。
Controlled oracleはNULLをFALSE／998、size 0／19／21をFALSE／87かつoutput unchangedと
した。Native HCURSORとdesktop cursor pointはopaque／host-dependent artifact identityなので
Vineへimportしない。Static consumerはreturn後に`flags & 2`だけを検査し、suppressed bitが
clearなら後続処理へ進む。

## Deterministic acceptance

Canonical Node reportは282,848 supported calls、392,353,932 instructions、100,556
clock samples、40 TIMERPROC callbacks、3 Present、semantic digest
`c261b54c013bca8637802c8851a232580c875fba70b477db3244df79a3c57967`、
historical terminal memory digest
`8af674327bd71aa3281a541d83d886a82653f9559b1f8c146dde8b7fd138d4a6`
を再現する。GetCursorInfo callsはsequence 282,389／282,396、instruction
375,285,853／375,288,207、output addresses `0x006ff530`／`0x006ff550`で、
logical output SHA-256は
`83409a8eb120ec1c8528928c2ad5ec6935dc2a2dbca6bfbc4be4efe11b761780`
である。

Historical Chrome Worker protocol v108／BGI report schema 74は同じordered semantics、二つの
CURSORINFO projection、next COM fault、terminal memoryを再現し、cancel／watchdog後に
pending host call／callbackを残さない。Artifact hash等の現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher get_cursor_info --locked
npm run check:runtime:bgi -- --profile user32-get-cursor-info
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-get-cursor-info --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```
