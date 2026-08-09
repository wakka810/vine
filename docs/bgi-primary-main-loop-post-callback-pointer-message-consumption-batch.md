# BGI post-callback pointer-message consumption batch

## Outcome

Primary BGIのpost-callback main loopを、pending `WM_LBUTTONDOWN`の消費から
WndProc内の最初のaction API直前まで一つのvertical sliceとして進めた。
Node／ChromeはRust-owned current-thread queueのmessage ID 44を
sequence 283,422の`PeekMessageA(PM_REMOVE)`で取得し、28-byte `MSG32`のpublishと
queue removalを同じowner transactionで完了する。

続くsequence 283,423の`TranslateMessage`はpointer messageをread-only no-opとして
検証し、sequence 283,424の`DispatchMessageA`はWndProc `0x10092550`へframe 76を
publishする。Callback内では既存handlerにより`EnterCriticalSection(0x1014C8A0)`、
`HeapAlloc(0x68010001, 0, 16)`、`LeaveCriticalSection(0x1014C8A0)`が順に完了する。
Heap allocation resultは`0x3E429780`で、最後のcompletionはsequence 283,427／
instruction 445,135,265である。六命令後の
`USER32.dll!SetFocus(0x48010001)`でcheckpointを閉じる。

## Authority and failure ordering

Message identity、queue、logical window、callback frameは`RuntimeUser32`、critical-section
stateはruntime synchronization owner、heap allocationとaddress identityはruntime heap、
published bytesは`GuestMemory`が所有する。WorkerはRustが返すimmutable completionを
比較するだけで、TypeScript側にWindows queue、callback、handle、heap stateを作らない。

`PeekMessageA`はoutput span、thread、filter、queue headをmutation前に検証し、MSG32 writeと
removalをtargeted commitする。`DispatchMessageA`はmessage/window/thread relationとcallback
capacityをpreflightしてからframeをpublishする。Nested callsは各ownerの既存validationと
failure atomicityを再利用し、batch completionはmessage removal、callback entry、三件の
nested completion、terminal memory digestが全て揃った場合だけpublishする。

## Evidence and acceptance

既存のnative `PeekMessageA`、pointer `TranslateMessage`／`DispatchMessageA`、critical-section、
heap evidenceでsemanticsとcaller consumerは固定済みなので、新しいnative traceは追加して
いない。Primary sequence、occurrence、frame ID、addressはartifact identityであり、semantic
handlerのselectorではない。

Canonical Node replayは283,426 completed calls、445,135,271 instructions、100,566 clock
samples、41 completed TIMERPROC callbacks、9,036 pacing completionsを記録する。Ordered
semantic digestは
`90bff2119a16d7aca682bde9f03f8f111cac56f8436a2b8886e85f71f4003af7`、terminal memory
digestは
`d22b59031f485dbc11347930f72893696ed32844c07df27f0919c0b6e6656f42`である。

Chrome Worker protocol v133／BGI report schema 98は、同じsix-completion batch、queue
depth `1 -> 0`、frame 76、nested call order、active callback depth 1、next `SetFocus`、
semantic／memory digestを照合し、cancel／watchdog cleanupも維持する。Artifact identityと
次stopは[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test user32_dispatcher --locked
cargo test -p vine-core --test runtime_heap_backing --locked
node tools/test-runtime-worker-evidence-level.mjs
npm run check:runtime:bgi -- --profile primary-main-loop-post-callback-pointer-message-consumption-batch
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile primary-main-loop-post-callback-pointer-message-consumption-batch --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Reached `SetFocus`のcompletion、WndProc／outer `DispatchMessageA` returnは含めない。
- Clickのvisible gameplay response、pointer capture／move／wheel、general focus transitionを証明しない。
- Nested heap allocation一件はgeneral heap workload、resource pressure、long-run stabilityを証明しない。
- Readable title、audio、save／load、representative flow、long-run、full-playを証明しない。
