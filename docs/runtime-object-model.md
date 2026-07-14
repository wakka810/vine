# Runtime object and handle foundation

## Purpose

review Step 9の最初のdependencyとして、single controlled process／initial threadの
`RuntimeState`へversion 1 typed object／handle tableを導入した。module graph、wait
queue、message、heap backing、DLL lifecycleが別々のlogical integerを作らず、同じ
identity、lookup、close、stale detection、waitable lifetimeを使うためのfoundationである。

このcheckpointはWin32 `CloseHandle`、`OpenProcess`、`OpenThread`、wait APIを追加しない。
guest-visible API semanticsへ接続する前のRust-owned runtime contractである。

## Handle encoding

`RuntimeHandle`は次の32-bit layoutを使う。

```text
31             24 23             16 15                         0
+----------------+-----------------+-----------------------------+
| object type: 8 | generation: 8   | slot: 16                    |
+----------------+-----------------+-----------------------------+
```

generationとslotはzeroを禁止する。close後はslot generationを進め、old valueのlookup／
double closeを`runtime-handle-stale`で拒否する。generation `255`をcloseしたslotはwrap
させずretireし、old handleが再びvalidになることを防ぐ。

| Kind | Type tag | Namespace | Initial／retained value |
|---|---:|---|---:|
| Process | `0x50` | `vine.runtime-process-object.v1` | `0x50010001` |
| Thread | `0x54` | `vine.runtime-thread-object.v1` | `0x54010001` |
| Mutex | `0x58` | `vine.runtime-mutex-object.v1` | first reached `0x58010001` |
| Module | `0x4D` | `vine.logical-loaded-module.v1` | existing `0x4D010001` |
| Private heap | `0x68` | `vine.logical-private-heap.v1` | existing `0x68010001` |

type tagsとnumeric valuesはVine runtime ABI version 1であり、native Windows handle値との
一致を主張しない。

## Object and handle separation

`RuntimeObjectTable`はstable `RuntimeObjectId`／`RuntimeObjectKey`のobject recordと、
generation付きhandle entryを分離する。一つのprocess／thread objectは複数handleを
持てる。handle closeはentryだけをretireし、object record、wait state、scheduler／
process payloadを破棄しない。全handle close後に同じobjectを再openしてもtermination
signalを保持する。

Process／thread handleはgeneric close policy、mutex closeはsynchronization-coordinated object
retirement、module／heap handleはsubsystem-owned policyである。generic closeをmodule／heapへ適用すると`runtime-handle-close-policy`で
停止する。将来の`FreeLibrary`／`HeapDestroy`がobject retirementとpayload teardownを
所有する。

Lookupは順にhandle type、nonzero fields、slot existence、generation、live entry、object
recordを検査する。type mismatch、never-allocated、stale／closed、internal dangling stateを
別codeにし、native pointerや別kindへaliasしない。

## Waitable lifetime

Process／thread objectは`nonsignaled`でbootstrapする。Mutex objectはprivate synchronization
recordのowner／recursionからacquisition stateを得る。schedulerのrunnable／callback／
host-wait stateとは別に、object tableがwaitable lifetimeを所有する。

- `cancel_current_thread(exit)`はthread objectを`signaled(exit)`へ遷移する。
- `terminate_current_process(exit)`はinitial threadとprocess objectを同じexit codeでsignalする。
- open済みの全handleと、termination後にopenしたhandleが同じsignal stateを観測する。
- mutex waitはunowned／same-threadならacquireし、other-thread ownershipはstructured unsupportedで止める。
- module／heapはwaitableでなく、queryを`runtime-object-not-waitable`で拒否する。

Process／thread waiter registrationはseparate wait-queue foundationへ接続済みである。Mutexの
cross-thread registration、wait-any／wait-all、wake ordering、timeout、alertable waitは未実装である。

## Existing state integration

`RuntimeState::new`はprocess／initial-thread objectとfirst handleを必ず作る。existing
`RuntimeProcessState.heaps`／`modules`はpayload registryとして残すが、create／load時に
同じexact handleをobject tableへatomic registrationし、lookup前にkind／generation／
livenessを検証する。これにより既存BGIのheap／KERNEL32 handlesを変えずにcommon
lifetime boundaryへ入れた。

Rust/Wasm browser sessionのcancel／clean exitは`terminate_current_process`を使う。
external report schemaとpending work countは変えず、内部process／thread objectをsignalする。

Reached `CreateMutexA` extensionはsame named objectへ複数handleを発行し、one closeではobjectを
保持、last closeでobject record／name／synchronization payloadをtransactionally retireする。
Process teardownも全mutex handleとpayloadを同時にretireする。Object tableはnameやownerを持たず、
private `RuntimeSynchronization`のmapを直接mutateしない。詳細は
[bgi-create-mutex-a.md](bgi-create-mutex-a.md)と
[runtime-synchronization.md](runtime-synchronization.md)を正とする。

## Contract test and regression

Representative test:

```powershell
cargo test -p vine-core --test runtime_objects --locked
```

The test fixes:

- initial process／thread handles `0x50010001`／`0x54010001`
- second thread handle `0x54010002`
- close＋slot reuse `0x54020001`
- later all-handle close＋reopen `0x54030001`
- stale／double-close and type-mismatch errors
- subsystem-owned heap／module policy and existing exact values
- thread-only cancellation signal and process-wide termination signal
- signal preservation after all thread handles close

Production regression on the same implementation:

- Chrome Worker vertical slice: 46 instructions, clean success／cancel／watchdog
- BGI: 17 calls／238 instructions, report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark: `go`, threshold failures `0`
- current Rust runtime core Wasm: 687,523 bytes, SHA-256
  `ad1941f95e96577f2e84462d534b95ca9e09f39d1e39e9f4f922ba0fac00b633`

## Non-claims and next dependency

This checkpoint does not implement access masks, duplicate-handle inheritance, pseudo handles,
cross-process handle tables, general object destruction/refcount, thread creation, mutex blocking,
wait-any／wait-all, or guest wait／close API coverage. Later checkpoints have added one-object wait
registration, message queues, heap backing, exports／forwarders, DLL callbacks, and named-mutex
lifetime over this foundation. It remains partial evidence
for `REQ-API-02`／`REQ-API-03` and `P3-WIN-01`, not their completion.

The process-owned module graph and controlled process／initial-thread lifecycle dependencies are now
implemented in [runtime-module-graph.md](runtime-module-graph.md) and
[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md). The additional controlled
thread registry is implemented in [runtime-thread-registry.md](runtime-thread-registry.md), and the
controlled wait-queue dependency is implemented in [runtime-wait-queue.md](runtime-wait-queue.md).
Per-thread message ownership is implemented in [runtime-message-queue.md](runtime-message-queue.md).
Subsystem-owned heap retirement and guest-memory backing are implemented in
[runtime-heap-backing.md](runtime-heap-backing.md). The next dependency is controlled DLL lifecycle.
