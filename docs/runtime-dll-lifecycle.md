# Runtime guest imports and DLL process lifecycle

## Purpose

review Step 9のninth checkpointとして、process-owned module graph、mapped image registry、
existing runtime export catalog、strict callback schedulerを一つのcontrolled loader lifecycleへ
接続した。Guest EXE／DLLのIATとstatic dependency edgeを同じplanからatomic commitし、全module
ready後にdependency-first `DLL_PROCESS_ATTACH`、attach failure unwind／retry、strict reverse
`DLL_PROCESS_DETACH`をRust `RuntimeState` authorityで進める。

このcheckpointはprocess load／termination時のstatic graphだけを扱う。`LoadLibrary`／
`FreeLibrary`、TLS callback、thread attach／detach、module memory／object retirement、broad system
DLL catalog、BGI continuationは含めない。

Architecture refactor checkpoint S6ではgraph、mapped images、committed import bindings、runtime
export table、process module recordsをprivate `runtime/modules.rs` ownerへ移した。Import bindingは
live graphをmutateせずcomplete graph candidateをplanし、object／memory preflight成功後にだけmodule
metadataをcommitする。DLL lifecycle phaseとin-flight callback operationは一時的に`RuntimeState`へ
残す。次checkpointはloader capabilityを増やさず、このoperation stateをsame subsystemへ移す。

## Controlled fixture and native oracle

`fixtures/pe32/dll-lifecycle/`はLLVM 22だけで再現生成するfixed graphである。

```text
dll-lifecycle.exe
  -> lifecycle_engine.dll!engine_verify_attached
  -> kernel32.dll!GetCurrentProcessId

lifecycle_engine.dll
  -> lifecycle_codec.dll!codec_get_state
  -> lifecycle_codec.dll!codec_note_engine_detached
```

| Image | Base | Entry RVA | Bytes | SHA-256 |
|---|---:|---:|---:|---|
| `dll-lifecycle.exe` | `0x32000000` | `0x1000` | 2,560 | `df9553bc5c10dae44d0b6f25f0a96c2a6fcda903895b7ffecebade641bff980a` |
| `lifecycle_engine.dll` | `0x31000000` | `0x1020` | 2,560 | `7154f24bec58d4e080de8bb98b512f53b425a54dcde273679c193cb1daaaf90b` |
| `lifecycle_codec.dll` | `0x30000000` | `0x1030` | 2,560 | `6e872f31dddb4932f06e0908c0cbe536a552dca24f338d174fa7e538a21c3d55` |

Native Windows execution self-verifies codec-before-engine attach and engine-before-codec detach;
exit code is `0`. `GetCurrentProcessId` keeps the same graph tied to a real system import while the
fixture remains freestanding from the CRT.

## Import binding contract

`ExecutionSession::bind_guest_module_imports` accepts the immutable loader/materialization identity
and ordered `RuntimeExportCatalog`. Planning performs no writes and checks:

- requester graph class、unsealed exact image owner／source／range／pages
- original IAT value and duplicate IAT address
- requested graph module identity
- mapped name／ordinal export and every guest forwarder hop
- runtime-system symbol membership in the ordered catalog
- candidate dependency graph, including cycle and ready-state rejection
- memory generation, fixed thunk arena capacity, owner／region／page quota

Mapped guest exports bind directly to guest addresses. Runtime-system imports use one process-owned
catalog table in `[0x7F000000, 0x7F010000)`, with 16-byte stable slots. The controlled catalog has
ID `1`／`0x7F000000` for `kernel32.dll!GetCurrentProcessId`; its one RX page has SHA-256
`4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059`.

The memory transaction writes and verifies every IAT, installs／seals the runtime table when needed,
and seals the requester image. Only after those operations succeed does private `RuntimeModules`
commit the candidate graph, export-table metadata, and binding record. A missing codec image leaves graph、IAT、
memory generation、binding registry unchanged, so the same materialization can retry.

Controlled direct targets are:

| Import | IAT target |
|---|---:|
| `codec_get_state` | `0x30001000` |
| `codec_note_engine_detached` | `0x30001010` |
| `engine_verify_attached` | `0x31001000` |
| `GetCurrentProcessId` | `0x7F000000` |

The sealed full reflection is 15 pages at generation `12` and is acknowledged before normal guest
reads.

## Callback lifecycle contract

Graph attach order is `kernel32 -> codec -> engine -> main`; runtime-system and main modules do not
receive `DllMain`, so callback order is `codec -> engine`. Each request owns a strict callback frame
and arguments `[module base, reason, 1]`, where nonzero reserved identifies static process load／
termination.

The controlled first engine attach returns `FALSE`. The coordinator does not commit graph attach
state, calls codec process detach, reports engine as the failing module and codec as unwound, returns
to `idle`, and permits retry. The successful retry commits all graph states together. Process detach
then calls `engine -> codec` before committing graph order `main -> engine -> codec -> kernel32`.
Frame IDs are monotonic `1..7`; wrong-frame completion and graph mutation during a callback are
structured conflicts without popping the live frame.

## Contract and regression

Representative contract:

```powershell
npm run check:toolchain
cargo test -p vine-core --test runtime_dll_lifecycle --locked
```

S6 regression preserves the Node BGI oracle at 284 calls／25,256 instructions and next
`GetCurrentThread`, with semantic report SHA-256
`0d3fcc6c6497c3b8ef005b88962f8448c7b321f0b77c3dffcae57b107648d164`.
Chrome `150.0.7871.115` reproduces the same memory and stop, with clean cancel／watchdog and browser
report SHA-256 `04af5d7305ebd17e1a1fc1415cebfc4dbc05743e50ed941500264aacb3b773c4`.
The 180-second workload completes 630 sessions with zero failures and all 17 thresholds passed;
report SHA-256 is `435eaf35b2fb2979c9bafe87ff2294cb585b10675ba063803b5d4d3a244fc234`.
Runtime core Wasm is 2,070,941 bytes／SHA-256
`7c94ef0ab815edef68ce51d612a6595274e9505ff2787ac7b3101e9e2ce69d4e`.

## Non-claims and next dependency

This checkpoint is partial evidence for `REQ-PE-05`／`P2-PE-05`, not their completion. It does not
execute these fixture callbacks on v86, write an x86 stack frame, dispatch the controlled runtime
thunk, implement arbitrary nested loader callbacks, or unload a module independently. It also does
not add a BGI-specific flag／request／receipt.

The generic BGI process path and Browser Worker equivalence are now established and capability
expansion remains paused at `GetCurrentThread`. The next responsibility checkpoint moves DLL
lifecycle phase, attach／detach operation, and callback progress into the private module subsystem
while preserving this controlled fixture and unwind／retry behavior.
