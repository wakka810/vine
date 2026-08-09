# BGI post-`timeGetTime` `GetWindowRect` profile

## Outcome

Primary BGIのpost-`TestCooperativeLevel` timer cycleで、TIMERPROC frame 75、outer
`DispatchMessageA`、empty `PeekMessageA(PM_REMOVE)`、post-callback `timeGetTime`が完了した後の
`USER32.dll!GetWindowRect(0x48010001, 0x006FF894)`を完了した。Callはsequence
282,879／occurrence 145／instruction 392,355,420で、`TRUE`、LastError 0を返す。

Rust-owned logical windowのouter rectangleは`[317,154,1603,903]`である。Handlerは16-byte
output spanをpreflightした後、`3D 01 00 00 9A 00 00 00 43 06 00 00 87 03 00 00`を一回だけ
guest memoryへpublishする。Window state、allocator、callback、LastErrorは変更しない。

その後のguest codeを4 instructions進め、instruction 392,355,424の
`USER32.dll!GetCursorPos(0x006FF88C)`へ到達する。EndpointはID 190／`0x7F000BD0`、
IAT `0x100D13D0`、return `0x10088563`である。

## Authority and failure ordering

`runtime/user32.rs::RuntimeUser32`がlogical HWND、window lifetime、作成時outer geometryの
authorityであり、`GuestMemory`がoutput bytesのauthorityである。既存の
`user32.get-window-rect.v1` handlerはlive handle、policy variant、完全な16-byte writable spanを
mutation前に検証し、一回のtargeted writeだけをcommitする。Invalid／stale HWNDは1400、invalid
outputは998で失敗し、output sentinel、window state、identity allocator、generationを部分更新しない。

TypeScript WorkerはRustがserializeしたcall、RECT bytes、write relation、次endpointを照合するだけで
ある。DOM window bounds、Chrome viewport、host HWNDを取り込まず、Windows window／RECT stateの
第二の正本を持たない。

## Evidence and acceptance

既存のprimary／confirmation native captureとcontrolled invalid-HWND caseが、caller RVA
`0x00088558`、`TRUE`／LastError preservation、outer rectangle、failure時output preservation、直後の
`GetClientRect`／coordinate comparisonを既に固定しているため、新しいnative traceは追加していない。
今回のsequence、instruction、guest addressはhandler selectorではなくprimary artifact identityである。

Canonical Node replayは282,879 completed calls、392,355,424 instructions、100,559 clock
samples、41 completed TIMERPROC callbacks、9,003 pacing completionsを記録する。Ordered semantic
digestは`c0d54e73c82a938aaf0694ab57377dcac8d6b7729b53ef273cf1988d8202c504`、
terminal memory digestは
`76ec157e2098a4401055790c710388759c00566570c382573123f15989ea4341`である。
2,642,326-byte reportのSHA-256は
`0a26e7854653c5159a142032d969dc348b9ccaf47d7db9b91a8c9d64b4178bd3`である。

Chrome Worker protocol v131／BGI report schema 96は同じBOOL、LastError、RECT bytes、authority、
failure atomicity、callback completion、terminal memory、`GetCursorPos` stopを照合する。Private
Chrome reportは2,625,725 bytes、SHA-256
`b9bd0efa3bde8a4b66929d8b4279b5fe22d0ae5bb5a5a08ef9c7b6636b9cd55e`である。
Artifactのactive identityと次stopは[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher --locked
npm run check:runtime:bgi -- --profile user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`GetCursorPos` completion、後続`GetClientRect` comparison、loop継続は含めない。
- Browser／host window geometry、multi-window placement、resize、DPI／multi-monitor追従を証明しない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
