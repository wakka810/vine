# Runtime module graph foundation

## Purpose

review Step 9のsecond foundationとして、single controlled processの`RuntimeState`へ
version 1 `RuntimeModuleGraph`を導入した。loader、static import、dynamic load、DLL
lifecycleが別々のmodule identity、reference count、load orderを持たず、process-owned
graphを共有するためのmetadata contractである。

このcheckpointはguest DLLをmapせず、export／forwarderを解決せず、`DllMain`を呼ばない。
ordered state transitionまでをRust authorityへ置き、image／callback lifecycleは後続へ残す。

Architecture refactor checkpoint S6では、process module recordsをprivate
`runtime/modules.rs::RuntimeModuleRecords`へ、graph、mapped-image registry、import binding、
runtime export tableをprivate `runtime/modules.rs::RuntimeModules`へ移した。`RuntimeState`は
aggregate coordinatorとして残るが、これらのcollectionを直接mutateしない。既存serialized
`process.modules`とroot `moduleGraph` shapeは`serde(flatten)`で維持する。

Architecture refactor checkpoint S7ではDLL lifecycle phase、attach／unwind／detach operation、
remaining／completed callback order、pending frame identity、attach failureをprivate
`runtime/modules.rs::RuntimeDllLifecycle`へ移した。Graph mutation guardもsame ownerが検査し、
aggregateはlifecycle stateを直接read／writeしない。

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
`register_main_module`はmain classを明示する。`RuntimeModules`はprivate process recordsを検証し、
complete next graphをmutationなしでplanする。Aggregateはその後typed object handleを登録し、
records／graphをinfallible commitする。Object-table failureは両module collectionとgraph allocation
counterを不変に保ち、graph rollback mutationを必要としない。`module_by_name`はprocess recordsを
parallel scanせず、graph canonical indexをauthorityとしてhandleへ解決する。

`RuntimeState`はhandle-based dependency、current-thread dynamic reference、ready、attach、
detach methodsを公開する。これらはobject tableでlive typed module handleを検証してから
graph IDへ変換する。

## Contract and regression

Representative contracts:

```powershell
cargo test -p vine-core --lib runtime::modules::tests --locked
cargo test -p vine-core --test module_graph --locked
cargo test -p vine-core --test runtime_objects --locked
```

The subsystem contract fixes mutation-free registration planning, stable `process.modules`／
`moduleGraph` serialization, duplicate rejection, and graph operations resolved only through owned
records. The graph contract fixes canonical lookup／class validation, unique main identity, mirrored
edges, cycle rejection without mutation, static／dynamic reference ownership, failed attach without
state change, and deterministic attach／detach. The aggregate contract fixes typed-handle integration,
object-registration failure with unchanged records and graph, and the controlled
`kernel32.dll -> controlled.exe` lifecycle order.

S7 regression on the same behavior:

- Node BGI oracle: 284 calls／25,256 instructions, next `GetCurrentThread`; semantic report
  SHA-256 `0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`
- Chrome Worker: Node-equivalent memory／next stop, cancel／watchdog clean; browser report
  SHA-256 `854725373a91fb8ca30577755c42c2351208d7bb7299cbef0b73c4e9ab1d3019`
- 180-second BGI workload: 620 sessions, failure `0`, all 17 thresholds passed; report
  SHA-256 `86028b5ea79ecd6640cc4ec7ab55f0d89e44d8590bf32034031b32af5b3acbf2`
- Rust runtime core Wasm: 2,090,657 bytes／SHA-256
  `c7b888d70a6c29ca0ce42f725dec573b6013cd40f4039345c948ec2d7ec1c3a5`

The dirty-checkpoint benchmark is regression evidence only. The clean commit `a0d99e5` report in
[v86-browser-benchmark.md](v86-browser-benchmark.md) remains the engine decision authority.
The final artifact-only run passed with `go`／zero failures. Two immediately preceding local runs
were `conditional-go` only from slice-throughput scheduling outliers (all 5／10／16 ms ratios near
`0.64`, then only 16 ms at `0.827`); CPU／API／memory／background／audio thresholds still passed.
These transient dirty-run failures do not replace the clean decision evidence.
Worker report identity is not embedded in `current-checkpoint.json`: the report contains the build
manifest, whose assets contain that checkpoint file, so pinning it there would create a hash cycle.

## Non-claims and next dependency

This checkpoint does not implement module search policy, guest import binding, relocations across
modules, delay loading, `LoadLibrary`／`FreeLibrary`, TLS callbacks, guest `DllMain`, unload
eligibility, stale endpoint invalidation, or module object retirement. It is partial evidence for
`REQ-PE-05`／`P2-PE-05`, not their completion. Immutable mapped image identity and bounded
name／ordinal／forwarder resolution are now implemented in
[runtime-module-images.md](runtime-module-images.md).

The controlled process／initial-thread lifecycle dependency is now implemented in
[runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md), and the additional
controlled thread registry is implemented in [runtime-thread-registry.md](runtime-thread-registry.md).
The wait queue is implemented in [runtime-wait-queue.md](runtime-wait-queue.md), the message queue is
implemented in [runtime-message-queue.md](runtime-message-queue.md), and private-heap backing is
implemented in [runtime-heap-backing.md](runtime-heap-backing.md). Guest import binding plus
dependency-ordered `DllMain` process attach／detach callbacks are implemented in
[runtime-dll-lifecycle.md](runtime-dll-lifecycle.md). The graph／image／import／export collections and
DLL lifecycle operation now share the private module owner. Responsibility checkpoint S13 completes
the remaining process/kernel aggregate audit; capability expansion resumes from the saved
`GetCurrentThread` stop after its evidence gate.
