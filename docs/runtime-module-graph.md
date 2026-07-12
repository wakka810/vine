# Runtime module graph foundation

## Purpose

review Step 9のsecond foundationとして、single controlled processの`RuntimeState`へ
version 1 `RuntimeModuleGraph`を導入した。loader、static import、dynamic load、DLL
lifecycleが別々のmodule identity、reference count、load orderを持たず、process-owned
graphを共有するためのmetadata contractである。

このcheckpointはguest DLLをmapせず、export／forwarderを解決せず、`DllMain`を呼ばない。
ordered state transitionまでをRust authorityへ置き、image／callback lifecycleは後続へ残す。

## Canonical identity and classes

Graph nodeはmonotonic `RuntimeModuleId`、existing typed module handle、requested name、
canonical name、class、discovery sequenceを持つ。requested nameはsource lookupのprovenance、
canonical nameはprocess内のidentityであり、次を満たす。

- 260 bytes以下のASCII basename
- ASCII case-insensitiveで、storage／lookup時はlowercase
- control、NUL、`/`、`\`、`:`を含まない
- main executableは`.exe`、guest／runtime moduleは`.dll`
- canonical nameとlogical handleはprocess内でunique

| Class | Initial reference owner | Role |
|---|---|---|
| `main-executable` | `process-root` | process entry image。1 processに1件 |
| `guest-dynamic-library` | none | static importまたはdynamic loadがlifetimeを所有 |
| `runtime-system-module` | `runtime-service` | Vineが提供するlogical system DLL |

Native HMODULEやhost pointerをidentityとして使わない。existing version 1 module handle
`type 0x4D / generation 1 / slot`を`RuntimeObjectTable`へ同時登録する。

## Dependencies and references

Dependency edge `requester -> dependency`は両方向metadataを保持し、dependency nodeへ
`static-import { requester }` referenceを1件加える。self edge、duplicate edge、cycle、
ready後のedge追加をmutation前に拒否する。edge insertionはcandidate nodeをclone上で
完成させてから両nodeをcommitするため、失敗時に片側edgeやreferenceを残さない。

Reference ownerは次の4種類である。

- process root
- runtime service
- static import requester
- dynamic load thread ID

Dynamic referenceはownerごとのcountを持ち、zeroでowner recordを除去する。unknown ownerの
releaseはunderflowさせず`module-reference-unknown`で停止する。version 1 limitはgraph
4,096 modules、1 moduleあたり4,096 distinct reference ownersである。

## Load states and order

State machineは次の一方向transitionだけを持つ。

```text
registered -> ready -> process-attached(sequence) -> process-detached(sequence)
```

Attachはmain executableの存在、全nodeの`ready`、全nodeのnonzero referenceをpreflight
する。その後、dependency-first topological orderを作る。同時にreadyなnodeはcanonical
name、discovery sequence、module IDでstableに並べる。Detachはactual attach sequenceの
strict reverseである。全transitionはcloned node tableへ適用し、成功後に一括commitする。

Controlled graphは次のorderを固定する。

```text
attach: codec.dll -> kernel32.dll -> engine.dll -> game.exe
detach: game.exe -> engine.dll -> kernel32.dll -> codec.dll
```

`process-attached`／`process-detached`はordered lifecycle metadataであり、guest callbackを
実行済みという意味ではない。

## RuntimeState integration

`RuntimeState::load_module`はruntime system／guest DLL classを選び、
`register_main_module`はmain classを明示する。graph registration後にtyped object handle
registrationが失敗した場合はunlinked graph node、canonical index、main identityを
rollbackし、latest module ID／discovery countersもretry前の値へ戻す。`module_by_name`は
parallel registry scanではなくgraph canonical indexをauthorityとしてhandleへ解決する。

`RuntimeState`はhandle-based dependency、current-thread dynamic reference、ready、attach、
detach methodsを公開する。これらはobject tableでlive typed module handleを検証してから
graph IDへ変換する。

## Contract and regression

Representative contracts:

```powershell
cargo test -p vine-core --test module_graph --locked
cargo test -p vine-core --test runtime_objects --locked
```

The graph contract fixes canonical lookup／class validation, unique main identity, mirrored edges,
cycle rejection without mutation, static／dynamic reference ownership, failed attach without state
change, deterministic attach／detach, and registration rollback. The `RuntimeState` contract fixes
typed-handle integration and the controlled `kernel32.dll -> controlled.exe` lifecycle order.

Production regression on the same implementation:

- Chrome Worker: 46 instructions, semantic output／cancel／watchdog unchanged, exit code `0`
- BGI: 17 calls／238 instructions; report SHA-256
  `ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4`
- BGI reflection SHA-256
  `0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a`
- version 1 browser benchmark regression: `go`, threshold failures `0`
- current Rust runtime core Wasm: 690,816 bytes／SHA-256
  `5bfa7b7141a3f0a30acd1c58e6a28afe2c5fc1e8f8b5e5859c97e3068a11cac0`

The dirty-checkpoint benchmark is regression evidence only. The clean commit `a0d99e5` report in
[v86-browser-benchmark.md](v86-browser-benchmark.md) remains the engine decision authority.
The final artifact-only run passed with `go`／zero failures. Two immediately preceding local runs
were `conditional-go` only from slice-throughput scheduling outliers (all 5／10／16 ms ratios near
`0.64`, then only 16 ms at `0.827`); CPU／API／memory／background／audio thresholds still passed.
These transient dirty-run failures do not replace the clean decision evidence.
Worker report identity is not embedded in `current-checkpoint.json`: the report contains the build
manifest, whose assets contain that checkpoint file, so pinning it there would create a hash cycle.

## Non-claims and next dependency

This checkpoint does not implement module search policy, immutable DLL source registration, guest
DLL mapping, relocations across modules, exports by name／ordinal, forwarders, delay loading,
`LoadLibrary`／`FreeLibrary`, TLS callbacks, guest `DllMain`, unload eligibility, stale endpoint
invalidation, or module object retirement. It is partial evidence for `REQ-PE-05`／`P2-PE-05`, not
their completion.

The controlled process／initial-thread lifecycle dependency is now implemented in
[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md), and the additional
controlled thread registry is implemented in [runtime-thread-registry.md](runtime-thread-registry.md).
The wait queue is implemented in [runtime-wait-queue.md](runtime-wait-queue.md). The next dependency
is a per-thread message queue; DLL thread callbacks remain later work.
