# BGI post-callback `timeGetTime` profile

## Outcome

Primary BGIのpost-`TestCooperativeLevel` timer cycleで、TIMERPROC frame 75、outer
`DispatchMessageA`、その直後のempty `PeekMessageA(PM_REMOVE)`が完了した後の
`WINMM.dll!timeGetTime()`を完了した。Callはsequence 282,878／occurrence 480／
instruction 392,355,124で、resultは3,800,773、LastErrorは0、guest writeは0である。

このreadは`scheduled-shared-logical-deadline-v1`のprocess clockを進めない。Timer clockは
2,007,735,000 x 100 ns／generation 16,309、scheduled cursorはretired 392,355,124／
pending 1,124 instructionsであり、同じcycleの`GetTickCount`と同じDWORD tickを返す。
Callback frameとouter dispatchはいずれもretire済みで、completion時のcallback depthは0である。

その後のguest codeを296 instructions進め、instruction 392,355,420の
`USER32.dll!GetWindowRect(0x48010001, 0x006FF894)`へ到達する。EndpointはID 228／
`0x7F000E30`、IAT `0x100D1468`、return `0x10088558`である。

## Authority and failure ordering

`RuntimeProcessState`がlogical timer clockとretired-instruction scheduleのauthorityである。
既存の`winmm.time-get-time.v1` handlerはdispatch前にmonotonic cursor、clock profile、DWORD
projectionを検証し、進行済みclockをread-onlyでsampleする。API readはclock、timer、scheduler、
callback、LastError、guest memory、allocator、generationを変更しない。Regressing cursor、invalid
profile、range failureはsample publication前に拒否され、部分更新を残さない。

TypeScript WorkerはRustがserializeしたcall、clock projection、callback completion、next endpointを
照合するだけである。Host／browser clockをsampleせず、Windows clock、thread、callback stateの
第二の正本を持たない。

## Evidence and acceptance

既存のcontrolled x86 oracleとpost-Present continuationがzero-argument stdcall、DWORD result、
LastError preservation、shared `GetTickCount` relation、consumer branchを既に固定しているため、新しい
native traceは追加していない。今回のsequence、instruction、guest addressはhandler selectorではなく
primary artifact identityである。

Canonical Node replayは282,878 completed calls、392,355,420 instructions、100,559 clock
samples、41 completed TIMERPROC callbacks、9,003 pacing completionsを記録する。Ordered semantic
digestは`e94e4c5abed4721d66b08acadf6b3e343fe95aef1cdc7ce115a69c1694d316c8`、
terminal memory digestは
`c1398f49cce0bfad683906c42e7fa05da421dcf43197f79610183091227934d5`である。
2,637,926-byte reportのSHA-256は
`b78563d485a21616d427ceaf8ad7313af68e783e644befff656859c3f8dab4b4`である。

Chrome Worker protocol v130／BGI report schema 95は同じclock sample、zero write、callback／
outer-dispatch completion、terminal memory、`GetWindowRect` stopを照合する。Private Chrome reportは
2,622,959 bytes、SHA-256
`03dc714290aa506f75ae010e6aa3983a3f278ba9fd345530fe94dfe9da666058`である。
Artifactのactive identityと次stopは[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --lib runtime::process::tests --locked
npm run check:runtime:bgi -- --profile winmm-time-get-time-post-test-cooperative-level-timer-callback
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile winmm-time-get-time-post-test-cooperative-level-timer-callback --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`GetWindowRect` completion、後続loop、timer replacement／`KillTimer`は含めない。
- Host elapsed time、background-tab catch-up、browser timer precision、audio clock同期を証明しない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
