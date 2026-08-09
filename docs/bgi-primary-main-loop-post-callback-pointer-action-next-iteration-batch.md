# BGI post-callback pointer-action next-iteration batch

## Outcome

前checkpointのpointer-action completionから、次のmain-loop input scanまでを一つの
vertical sliceとして進めた。Sequence 283,431のunfiltered
`PeekMessageA(PM_REMOVE)`から57,602件の既存contractを継続し、sequence 341,032／
instruction 3,114,597,133の`GetAsyncKeyState(VK_LBUTTON)`まで完了する。Reached queryは
held bitとpressed-since bitを含む`0xFFFF8001`を返す。そのcompletion後にnormalized
primary-button release event 4をRust input ownerへcommitし、`WM_LBUTTONUP` message 117を
logical time 5,163,564でcurrent-thread queueへ追加する。88 guest instructions後の
instruction 3,114,597,221で、同じscanの次の
`GetAsyncKeyState(VK_RBUTTON)`へ到達する。

この長いcontinuationで初到達した`KERNEL32!DeleteCriticalSection`三件も同じcheckpointで
閉じた。Sequence 302,673、310,290、328,190はそれぞれguest address
`0x3E4298B0`、`0x3D3843E8`、`0x3D384510`のunowned recordをretireし、各24 bytesをzero化する。
APIはVOID／stdcall、EAX／LastError preserveである。

Replayは従来の16,384-page execution-mirror ceilingを超え、release時点で21,399 physical
pagesへ到達した。BGI profile ceilingを24,576 pages／96 MiB guest bytesへ上げ、page tables、
directory、zero page、VGA holeを含む最大poolを104,996,864 bytesに固定した。Retired
instruction countはsigned i32境界も越えるため、Nodeはadapterのraw i32／u32 bitsを
unsigned化し、u32 wrap epochを加えたsafe integerとして保持する。Workerは各sliceの
unsigned modular deltaを既存のmonotonic report counterへ加算する。

## Authority and failure ordering

Critical-section identity、collection、generation、retirementは
`RuntimeSynchronization`が所有する。Delete planはrecordのbefore-imageとsubsystem
generationを保持し、unowned state、guest bytes、lifecycleをmutationなしで検証する。
Dispatcherは24-byte guest write全体をpreflightしてからowner retirementをcommitし、最後に
zero bytesをpublishする。Read-only span、owned delete、duplicate plan、delete／recreate ABAは
record、generation、guest bytesを変更せずtyped faultになる。

Mirror capacityはRust prepared metadataがauthorityであり、browser hard ceilingは128 MiBの
ままである。Instruction counterのraw Wasm export、physical page number、adapter generationは
execution identityに留め、guest clockやWindows stateへ昇格させない。Input release、logical
window、keyboard state、message queue、guest bytesもRust ownerだけが更新し、Workerは
versioned completion projectionとbrowser resourceだけを扱う。

## Evidence and acceptance

Controlled x86 oracle `x86-uncontended-recursive-v1`はinitialize、recursive enter／leave、
final leave、deleteを一続きで観測する。DeleteはLastError sentinel 92を保持し、unowned
24-byte structureを全zeroへ変える。Primary replayの三件は同じsemantic contractを異なる
guest identityで消費する。Primary sequence、instruction、address、report hashはartifact
identityであり、handler selectorではない。

Canonical Node replayは341,032 completed calls、3,114,597,221 instructions、101,366 clock
samples、113 completed TIMERPROC callbacks、12,214 pacing completions、32 presentsを記録する。
Ordered semantic digestは
`20e034774f95ec69008f55477c8d6b1a2b7d9e3add2d694ef62d0d8b25cd1575`、terminal memory
digestは
`5840ee438cbbdaae914c5a7169f0dc8aac5169c70388234778cd96079e8d0b11`である。

Chrome Worker protocol v135／BGI report schema 100は、同じ57,603-completion checkpoint、
三件のcritical-section retirement、pressed-since consumption、release receipt、queued
`WM_LBUTTONUP`、next `VK_RBUTTON` query、semantic／memory digestを一致させる。Chromeは
1,632,970 execution-state transitionsと191,120 mapping deltasを処理し、cancel／watchdog
cleanupも維持する。Exact artifact identityと次stopは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

Representative checks:

```powershell
cargo test -p vine-core --test api_dispatcher --locked
cargo test -p vine-core --test runtime_synchronization --locked
node tools/test-bgi-checkpoint-command.mjs
npm run check:runtime:bgi -- --profile primary-main-loop-post-callback-pointer-action-next-iteration-batch
node tools/run-browser-runtime-check.mjs --bgi --bgi-profile primary-main-loop-post-callback-pointer-action-next-iteration-batch --bgi-policy fixtures/runtime-policies/kernel32-delete-file-a-v1.json
npm run check:runtime:browser-worker
```

## Non-claims

- Next `GetAsyncKeyState(VK_RBUTTON)`のcompletionと残りのinput scanは含めない。
- Queued `WM_LBUTTONUP`のretrieval、dispatch、visible gameplay responseは証明しない。
- Critical-section contention、owned delete、try-enter、spin-count mutation、native layoutの
  一般化は証明しない。
- 96 MiB ceilingはこのreplayの成立性を固定する値であり、full-play peak memoryや
  memory-pressure acceptanceを証明しない。
- Readable title、audio、save／load、representative flow、long-run、full-playは証明しない。
