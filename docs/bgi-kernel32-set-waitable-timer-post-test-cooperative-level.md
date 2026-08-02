# BGI post-`TestCooperativeLevel` `SetWaitableTimer` profile

## Outcome

Primary BGIのpost-`timeGetTime` `TestCooperativeLevel`に続く
`KERNEL32.dll!SetWaitableTimer`を、既存のRust waitable-timer transactionで完了した。
Callはsequence 282,871、occurrence 9,921、trap instruction 392,354,369で、
`SetWaitableTimer(0x57010001, 0x006FF8F0, 0, NULL, NULL, FALSE)`を実行する。
Guestの`LARGE_INTEGER`はrelative due time -5,000（100 ns単位）である。ReturnはTRUE、
LastErrorは0、return addressは`0x10050607`、stack pointerは
`0x006FF8D4 -> 0x006FF8F0`、guest writeは0 bytesである。

Replayはさらに6 instructions進み、instruction 392,354,375で次のowned endpoint
`WaitForSingleObjectEx(0x57010001, 10, TRUE)`を記録する。EndpointはID 122／
`0x7F000790`、IATは`0x100D1290`、return addressは`0x10050615`、stack pointerは
`0x006FF8E0`である。このcheckpointはarmの完了までをclaimし、expirationとwait resultは
次のstructured stopへ残す。

## Authority and failure ordering

`RuntimeTimers`はtimer ID 1、manual-reset mode、arm payload、deadline、generation 68を所有する。
`RuntimeObjectTable`はlogical handle `0x57010001`、object ID 20、namespace
`vine.runtime-waitable-timer-object.v1`、open handle count 1、`nonsignaled` wait stateを
別authorityとして所有する。Shared process clockのarm時刻`2,007,730,000`からdue magnitude
5,000を加え、deadline `2,007,735,000`をpublishする。

既存の`kernel32.set-waitable-timer.relative-one-shot.v1`はguest due-time read、policy、
timer identity／generation、typed-object relation／lifetime、両ownerのcapacityをpreflightし、
timer arm planとobject wait-state planをtargeted commitする。Unknown／stale／wrong-kind handle、
unsupported period／completion routine／resume、unreadable due time、stale／duplicate plan、
preflight failureは両owner、allocator、guest memoryを変更しない。Aggregate cloneによるrollbackは
使わない。

TypeScriptはimmutable completionを検証するだけで、Windows timer state、logical deadline、
wait stateを複製せず、このarmのためのbrowser timerも作らない。Browser pacingは後続waitの
adapter boundaryであり、このarm transactionのauthorityではない。

## Deterministic acceptance

Canonical Node replayは282,871 supported calls、392,354,375 instructions、100,557
scheduled-clock samples、40 TIMERPROC callbacks、4 Presentを完了する。Ordered semantic digestは
`640bc23ea65411d491b62e372e78562e5f2646fff44a4adf5738626c39677c80`、terminal memory digestは
`f476539a157bd61c93ce31b1569baa78e0e3e50342d618a5256859f45927f174`である。
2,629,834-byte Node reportのSHA-256は
`fd53c2894ea5ee952cf46714ff13882a6ae2f84c51529d93563c7d7a46dbdef4`である。

Chrome Worker protocol v122／BGI report schema 88は同じcall、timer／object snapshot、
ordered semantics、terminal memory、次のwait frameを一致させる。Private reportは2,623,041 bytes、
SHA-256 `e65a8503632a931ff7d538d57ecd95fae444a63a1750fc997537fe2ba627dcb6`で、
cancel／watchdog cleanup後のpending host call／callback／timer pacing requestは0である。
Raw generation、writeback、artifact identityの現在値は
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test api_dispatcher set_waitable_timer --locked
cargo test -p vine-core --test runtime_waitable_timer --locked
npm run check:runtime:bgi -- --profile kernel32-set-waitable-timer-post-test-cooperative-level
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile kernel32-set-waitable-timer-post-test-cooperative-level --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Arm transaction自体はtimerをexpire／signalせず、wait result、APC delivery、thread resumeを
  選ばない。
- Nonzero period、completion routine、resume、absolute／zero／policy外due timeは未対応である。
- 後続finite alertable wait、general timer variants、host wall-clock scheduling、long-run behaviorは
  このcheckpointのacceptanceへ含めない。
- Logical timer handle／object IDはVine identityであり、native handleやbrowser timer IDではない。
