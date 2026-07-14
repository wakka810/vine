# Runtime mapped-module and PE32 export foundation

## Purpose

review Step 9のeighth foundationとして、existing process-owned `RuntimeModuleGraph`を
Rust-owned mapped guest-module registryへ接続した。Immutable PE32 source／load plan、
materialized `GuestMemory` owner／range、logical module handle、export identityを一つの
authoritative recordへ固定し、name／ordinal／forwarder lookupをguest addressへ解決する。

このcheckpointはguest IATをbindせず、dependency edgeをimportから生成せず、`DllMain`を
呼ばない。Mappingとexport identityだけをcallback lifecycleより先に成立させる。

Architecture refactor checkpoint S6ではmapped-image registryをgraph、import binding、runtime
export tableとともにprivate `runtime/modules.rs::RuntimeModules`へ移した。`RuntimeState`はimmutable
inspection／resolution methodだけを公開し、registry collectionを直接所有またはmutateしない。

Architecture refactor checkpoint S7ではDLL lifecycle operationもsame module ownerへ移し、active
callback中はgraphだけでなくimage registrationとimport-binding planningもsubsystem内で拒否する。
Lifecycle planが参照したmodule／image／binding identityはcallback completionまで変化しない。

## Bounded PE32 exports

`Pe32LoadPlan::exports`はexport directoryがある場合だけ次を保持する。

- module name、directory RVA／size、version／timestamp
- ordinal base、EAT／name pointer／ordinal table RVAとdeclared counts
- nonzero EAT slotごとのordinal、zero以上のnames、direct RVAまたはforwarder target
- forwarder raw identityとparsed module＋name／decimal ordinal lookup

Version 1 limitsは65,536 EAT slots、65,536 names、1,024-byte name、1,024-byte
forwarderである。Table size／RVA、file backing、name ordinal index、duplicate name、zero
named slot、direct target mapping、forwarder terminator／syntaxをchecked処理する。Zero EAT
slotはordinal gapとして保持し、symbol recordを作らない。Export name lookupはPE contractに
従いcase-sensitive、module lookupはexisting graph contractと同じASCII case-insensitive
canonical basenameである。

Export-free EXE／BGIのplanは`exports` fieldをserializeせず、new parser limitsもfrozen
version 1 reportへ追加しない。これにより既存BGI report／reflection digestを変更しない。

## Mapped image identity

Private `RuntimeModules` image registryは最大4,096 recordsをmodule ID、canonical name、logical
handleでindexする。Registrationは全checkをregistry mutationより先に実行する。

1. graph nodeは`registered`で、main EXE／guest DLL classとPE image kindが一致する。
2. runtime system moduleはguest PE ownerを持たない。
3. loader source、materialization source／layout、preferred／load base、relocation delta、entry
   point、image sizeが一致する。
4. `GuestMemory` ownerはImage kind、source SHA-256、exact one Image region、exact committed
   page setを持ち、sealedである。
5. export directory module nameはgraph canonical identityと一致する。
6. module ID／name／handle、guest rangeはexisting mapped recordと重複しない。
7. every direct export resolves inside its materialized image range.

PE `AddressOfEntryPoint == 0`はimage baseではなく`None`として記録する。これはentrypointの
ないDLLを後続lifecycleが誤ってcallbackしないためのcontractである。Registered ownerはsealed
なので、registryが保持したguest bytesをhostまたはguest writeで後から変更できない。

## Export resolution

Lookupはcanonical module＋`ExportLookup::Name`または`Ordinal`から始まり、direct targetを
`load_base + RVA`へchecked変換する。Forwarder moduleにextensionがなければ`.dll`を付け、
nameまたは`#ordinal`で次のmapped imageをlookupする。Resultはrequested identity、defining
module／ordinal／names、guest address、ordered forwarder hopsを保持する。

Public name lookupはprintable ASCII 1,024 bytes、ordinal lookupは16-bit rangeへ制限する。
Forwarderは32 hopsまで許可する。33件目、same module＋symbol pairのcycle、unknown target
module、missing export directory／symbolはstructured errorで停止する。Lookupはread-onlyで、
failure前後のregistryはequalである。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --test runtime_module_images --locked
```

SHA-256固定の2,048-byte `vine_fixture.dll`
`a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878`はordinal
`1`／`fixture_add`／RVA `0x1000`とordinal `2`／`fixture_call_callback`／RVA
`0x1010`を持つ。Test内だけで作る`bridge.dll`はname forwarder
`vine_fixture.fixture_add`とordinal forwarder`vine_fixture.#2`を持つ。

Controlled resolution:

| Request | Defining address | Forwarder hops |
|---|---:|---:|
| `vine_fixture.dll!fixture_add` | `0x10001000` | 0 |
| `vine_fixture.dll!#2` | `0x10001010` | 0 |
| `bridge.dll!fixture_add` | `0x10001000` | 1 name hop |
| `bridge.dll!#2` | `0x10001010` | 1 ordinal hop |

The same contract fixes export count／syntax bounds, unsealed owner retry, duplicate identity,
overlapping range, unknown target, and two-module forwarder cycle. Registration／resolution failure
preserves the complete image registry. Five valid images are registered; the overlapping sixth is
rejected.

S7 regression on the same behavior:

- Node BGI oracle: 284 calls／25,256 instructions, next `GetCurrentThread`; semantic report
  SHA-256 `0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`
- Chrome Worker: Node-equivalent memory／next stop, cancel／watchdog clean; browser report
  SHA-256 `42c572627aeb22e04ab4d06749ac5c520e6aa526c1f1a6376b0960b09038fb1e`
- 180-second BGI workload: 624 sessions, failure `0`, all 17 thresholds passed; report
  SHA-256 `c10dc7cfc610ca17abbc69559e2869d47ddc5351d6508b5ead9c06b26c4bade7`
- Rust runtime core Wasm: 2,085,717 bytes／SHA-256
  `9c3d9f02eae37441d3705e4b1ceb9ec37b6f9cc4622f0a962d0b63374ad28d8f`

The clean commit `a0d99e5` report remains the engine decision authority; current benchmark values
are regression evidence only.

## Non-claims and next dependency

This checkpoint does not implement module search／source import policy, guest import binding,
runtime-system export unification, graph dependency discovery from imports, delay loading,
`LoadLibrary`／`FreeLibrary`, TLS callbacks, `DllMain`, callback failure unwind, unload eligibility,
module memory／object retirement, or BGI continuation. It is partial evidence for `REQ-PE-05`／
`P2-PE-05`, not their completion.

The controlled guest import／runtime export catalog binding and dependency-ordered process callback
lifecycle are implemented in [runtime-dll-lifecycle.md](runtime-dll-lifecycle.md). The graph, image,
import binding, runtime export, and DLL lifecycle operation state now have one private module owner.
The next responsibility checkpoint audits remaining process/kernel aggregate state without adding
loader capability.
