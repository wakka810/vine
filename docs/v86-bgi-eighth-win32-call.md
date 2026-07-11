# BGI eighth supported Win32 call return

## 1. Status

- Status: Verified primary-title API-return checkpoint
- Date: 2026-07-12
- Source: `V:\softs\サクラノ詩\BGI.exe`
- Source size／SHA-256: 1,193,472 bytes／`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
- Report schema: `vine.v86-bgi-eighth-win32-call` version `1`
- Report size／SHA-256: 49,699 bytes／`f8c8ca1e678bb1e118248a6626825ddf585dc240aa59e1abc428429cbab3a1ad`

This checkpoint continues [v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md). It declares exact thunk ID `102`, validates the three-argument stdcall frame and indirect IAT call in Rust, creates one bounded logical private-heap record, returns its typed handle, and stops at the next exact owned thunk. It does not dispatch a heap allocation.

## 2. Native observation

The focused Windows trace was captured with:

```powershell
py tools/windows/trace.py `
  --exe "V:\softs\サクラノ詩\BGI.exe" `
  --output ".work\windows-traces\bgi-heap-lifecycle-20260712.jsonl" `
  --seconds 3 `
  --kill-after `
  --observe heap-lifecycle
```

The uncommitted raw trace is 6,870,828 bytes with SHA-256 `9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682`. The target call returned to `BGI.exe + 0xA8886`:

| Field | Native observation |
|---|---:|
| `flOptions` | `0` |
| `dwInitialSize` | `0x1000` |
| `dwMaximumSize` | `0` |
| Native result | `0x04A40000` |
| LastError before／after | `0`／`0` |

The result is a host-process／ASLR-specific address and is not imported into Vine. The first BGI-owned consumer is `HeapAlloc` at caller RVA `0xAC3CC` with `[createdHeap, HEAP_ZERO_MEMORY, 0x214]`; the next allocation uses size `0x800`. This consumer evidence bounds the next heap checkpoint but does not make allocation part of this one.

## 3. Typed logical heap state

Rust creates exactly one record owned by `checkpoint-process-private-heaps`:

| Field | Controlled value |
|---|---:|
| Variant | `heapcreate-growable-initial-4096` |
| Handle namespace | `vine.logical-private-heap.v1` |
| Type tag／generation／slot | `0x68`／`1`／`1` |
| Encoded handle | `0x68010001` |
| Lifecycle | `active` |
| Creation flags／initial／maximum | `0`／`0x1000`／`0` |
| Growable | `true` |
| Live allocations | `0` |
| Backing | `deferred-until-first-reached-heap-operation` |

The high-byte type tag, generation, and slot make accidental use of the native address observable and leave room for stale-handle checks. The exact encoding is a checkpoint version-1 choice, not a final cross-subsystem object-manager ABI. No guest range is reserved or committed yet because `HeapCreate` exposes only the opaque handle before the first reached allocation.

## 4. Checked three-argument stdcall

This mode adds `HeapCreate` as the ninth declared import endpoint, leaving 321 identity-preserving unsupported traps. Reflected bytes and generation remain unchanged. The initial offer is 4,904,338 bytes.

| Field | Value |
|---|---:|
| Thunk／IAT | `0x7F000650`／`0x100D1240` |
| Indirect call／return | `0x100A8880`／`0x100A8886` |
| ESP before return | `0x006FFF6C` |
| Frame | `[0x100A8886,0,0x1000,0]` |
| stdcall argument bytes | `12` |
| ESP after callee return | `0x006FFF7C` |
| EAX before／after | `0x00000110`／`0x68010001` |

The call must be `FF 15 <0x100D1240>`, and that IAT slot must still contain owned thunk `0x7F000650`. Rust derives the result from the typed heap record; the runner does not supply or reuse the native pointer.

## 5. Generation-preserving reconciliation

The runner supplies the current generation `27` stack page at `0x006FF000`, owner `4`. Execution since the heap-policy return changed 8 bytes; the page SHA-256 is `48f30a675492096d7110c249c8f8bc35da1b58ab3db07df9553e168e56e05d93`.

Rust reconciles the page and validates the exact frame and IAT provenance transactionally. Heap creation writes no guest bytes at this boundary, so generation and acknowledged generation remain `27`, dirty pages remain zero, and no cache effect is requested. The request is 8,691 bytes and receipt is 1,233 bytes, excluding newlines.

## 6. Next runtime stop

After the stdcall return, BGI executes 14 more instructions in 14 execute stops with 13 budget stops. At cumulative instruction count `122`, it reaches:

| Field | Observed |
|---|---|
| Thunk | ID `72`, `KERNEL32.dll!GetModuleHandleW` |
| Disposition／address | `unsupported-trap`／`0x7F000470` |
| IAT | `0x100D11C8` |
| ESP／return | `0x006FFF70`／`0x100A5811` |
| Argument | `0x100D24D0` -> UTF-16 `KERNEL32.DLL` |
| Raw stack words | `[0x100A5811,0x100D24D0,0,0x100A375B,0xE93F6D87,0,0,0]` |

The immutable source fingerprint, reflection SHA-256 `f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0`, memory generation, and all earlier report hashes remain unchanged.

## 7. Reproduction

```powershell
npm run prepare:engine
npm run check:runtime:bgi-eighth-win32-call
```

Two consecutive runs produced the same 49,699-byte report and SHA-256. `npm run check:private-corpus` runs the first-trap contract and all eight cumulative call-return contracts. Raw traces and generated reports remain under `.work/` and are not committed.

## 8. Non-claims and next checkpoint

This is one exact heap creation variant, one logical handle, and one metadata-only live heap record. It is not a general heap allocator, process default heap, guest virtual-memory reservation policy, allocation／free／reallocation implementation, failure path, synchronization model, corruption detector, or general LastError model.

The next checkpoint should measure and return exact thunk ID `72` for `GetModuleHandleW(L"KERNEL32.DLL")`, resolve it to a typed logical loaded-module identity, and stop at the next exact owned thunk. It must not return a host `HMODULE` or claim general loader／DLL lifecycle support.
