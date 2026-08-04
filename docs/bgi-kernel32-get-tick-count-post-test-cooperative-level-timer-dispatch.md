# BGI timer-callback `GetTickCount` and dispatch completion profile

## Outcome

Primary BGIのpost-`TestCooperativeLevel` timer cycleで、sequence 282,875の
`DispatchMessageA`がenterしたTIMERPROC frame 75から、nested
`KERNEL32.dll!GetTickCount`、callback return、outer dispatch completionまでを完了した。
`GetTickCount`はsequence 282,876／occurrence 100,079／instruction 392,354,458で
3,800,773を返す。Callback depthはこのread時点で1である。

Callbackはinstruction 392,355,081で64を返す。Timer dispatch contractはこのresultを
無視し、pending sequence 282,875をouter result 0で完了する。Instruction 392,355,090で
callback depth 0のまま次の`USER32.dll!PeekMessageA`へ到達する。直前にpublishされた
28-byte `WM_TIMER MSG32`は不変である。

## Authority and failure ordering

`RuntimeProcessState`のscheduled process clockとretired-instruction cursorがtick projectionの
authorityである。Readはtimer time `2,007,735,000` 100 ns、generation 16,309、retired
instructions 392,354,458からDWORD tickをprojectionするだけで、clock、guest memory、
LastErrorを変更しない。Host uptime、browser clock、Worker schedulingは入力にしない。

`RuntimeThreads`がframe ID、depth、return sentinel、pending outer callを所有し、
`RuntimeMessages`／`RuntimeTimers`／`GuestMemory`がimmutable MSG、live timer relation、guest
bytesを所有する。Callback returnはcurrent thread、top frame、sequence、sentinelを検証して
からframeをretireし、outer completionをcommitする。Stale／wrong-thread／wrong-frame returnは
callback stack、pending call、message、timer、clockを部分更新しない。TypeScriptはRustが
serializeしたcompletionを照合するだけで、Windows callbackやclock stateを所有しない。

## Evidence and acceptance

Canonical Node replayは282,876 completed calls、100,558 clock samples、41 completed
TIMERPROC callbacks、9,003 pacing completionsを記録する。Ordered semantic digestは
`4759db3b97a4e71467ae9ccd93330b6a45befb528339ca419bcbddd962efdcaa`、terminal memory
digestは`bd50f8282664659b53c4ab720e735885696265ac05a4bf7235f0bef82518c58c`である。
2,637,877-byte reportのSHA-256は
`69d2790e54993a16523fa9056e523b679329943f7856de8d9312e9423b843f99`である。

Chrome Worker protocol v128／BGI report schema 93は同じcall、frame return、ignored result、
outer result、callback depth、MSG preservation、next endpointを照合する。Private Chrome reportは
2,624,704 bytes、SHA-256
`3ef01445a4743fc8b6f72a2dac9aa8f3877201f4a52463d3d9dd7b3cbde21a28`である。Artifactの
active identityと次stopは[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test runtime_message_queue --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`PeekMessageA` completion、後続loop、timer replacement／`KillTimer`は含めない。
- Non-timer WndProc、cross-thread dispatch、malformed callback returnへsupport bandを広げていない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
