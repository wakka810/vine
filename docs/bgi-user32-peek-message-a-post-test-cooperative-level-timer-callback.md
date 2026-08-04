# BGI post-callback empty `PeekMessageA` profile

## Outcome

Primary BGIのpost-`TestCooperativeLevel` timer cycleで、TIMERPROC frame 75とouter
`DispatchMessageA`が完了した直後の
`USER32.dll!PeekMessageA(0x006FF8E8, NULL, 0, 0, PM_REMOVE)`を完了した。
Callはsequence 282,877／occurrence 111／instruction 392,355,090で、current thread
8192のqueue depthはbefore／afterとも0、resultはFALSE、LastErrorは0、guest writeは0である。

このreadは直前の`WM_TIMER MSG32`を変更しない。Call boundaryでの28-byte content
SHA-256は`e6521e2aa3109cd80f56da82d2662d3e526b5a40981927410a2d9061f5ad6135`で、
callback frame 75とpending outer dispatchはいずれも既にretireされ、callback depthは0である。
その後のguest codeを34 instructions進め、instruction 392,355,124の
`WINMM.dll!timeGetTime`へ到達する。Terminal memory digestがcall boundaryから変わるのは、
guest自身がMSGのtime／point fieldsを書き換えるためであり、`PeekMessageA`のwriteではない。

## Authority and failure ordering

`RuntimeMessages`がcurrent-thread queue、`RuntimeThreads`がselected threadとcallback
lifecycle、`GuestMemory`がMSG bytesのauthorityである。既存のunfiltered current-thread
retrieval planはthread、filter、remove flag、output range、queue headをmutation前に検証する。
空queueではFALSEを返すだけで、output、queue、timer、callback、LastError、allocator、memory
generationを変更しない。Bad pointer、unsupported filter、wrong thread、stale planも同じownerを
部分更新しない。

TypeScript Workerは、Rustがserializeしたcompletion、queue depth、callback depth、call-time
message digest、next endpointを照合する。Browser message、DOM event、timer taskからWindows queue
stateを合成せず、guest pointerやcallback frameの第二の正本を持たない。

## Evidence and acceptance

Canonical Node replayは282,877 completed calls、392,355,124 instructions、100,558 clock
samples、41 completed TIMERPROC callbacks、9,003 pacing completionsを記録する。Ordered semantic
digestは`3eaa8bd819d4734c03e42670a942d8862148da54eb6a54d45f661a4f5ac18cf9`、
terminal memory digestは
`a49abb18e8952699d36c2c1c1a0fe2b7e2d1d73ae4cb963a20895caa9686c664`である。
2,636,161-byte reportのSHA-256は
`dd2ee95852aade62733001ac04a7a42f809ce2a4fa1599ad95813218ca9d82a7`である。

Chrome Worker protocol v129／BGI report schema 94は同じcall、empty queue、FALSE、zero
write、callback／outer-dispatch completion、call-time MSG preservation、terminal memory、next
endpointを照合する。Private Chrome reportは2,622,273 bytes、SHA-256
`62bdc32f34caaa341b4b6a3d0b39b11a2528a4ebb5e89463228c7245652177b4`である。
Artifactのactive identityと次stopは[current-checkpoint.json](current-checkpoint.json)だけを
正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test runtime_message_queue --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile user32-peek-message-a-post-test-cooperative-level-timer-callback
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile user32-peek-message-a-post-test-cooperative-level-timer-callback --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- 次の`timeGetTime` completion、後続loop、timer replacement／`KillTimer`は含めない。
- Filtered／cross-thread retrieval、host／DOM message import、一般的なmessage pumpへsupport bandを広げていない。
- この到達はreadable title、visible input response、long-run、save／load、full-playを証明しない。
