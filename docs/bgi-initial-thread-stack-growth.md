# BGI initial-thread stack growth checkpoint

## Status

- Status: implemented and semantically equivalent in Node and Chrome
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-initial-thread-stack-growth`
- Generic dispatcher: 44 API contracts／268 completed calls
- Compatibility policy: `kernel32-initialize-critical-section-v1`
- Next structured stop: unsupported `MultiByteToWideChar(CP_ACP, 0, ...)` variant

This checkpoint connects the PE32 initial-stack reserve／commit layout to Rust
`GuestMemory`, registers the stack in the process-wide address allocator, and
grows its movable guard through the existing mapping-delta protocol. It does
not add another memory authority in Node or TypeScript.

## Reached fault and PE layout

The preceding checkpoint stopped at instruction `22,754`, EIP `0x100A47A9`,
with fault address `0x006FE000` and PFEC `4`. The fixed BGI PE header declares:

| Field | Value |
|---|---:|
| `SizeOfStackReserve` | 1,048,576 bytes |
| `SizeOfStackCommit` | 4,096 bytes |
| controlled stack top | `0x00700000` |

The production initial layout is therefore:

```text
reserved range:          [0x00600000, 0x00700000)
initial committed range: [0x006FF000, 0x00700000)
initial movable guard:    [0x006FE000, 0x006FF000)
initial ESP:               0x006FFFFC
```

The one-page controlled fixture remains `[0x006FF000, 0x00700000)` and fully
committed. Only the production BGI path consumes the PE reserve／commit values.

## Rust authority and atomic growth

`RuntimeState::register_initial_thread_stack` creates process allocation ID
`37` with purpose `thread-stack`, classification `stack`, thread lifetime,
explicit release policy, 1 MiB total range, and one guard-page requirement.
The record and the thread-owned stack state both refer to guest-memory owner
`4`; no fixed address is copied into a separate allocator implementation.

`RuntimeState::grow_initial_thread_stack` accepts only a non-present fault
whose containing page is the current guard. Validation covers runtime
lifecycle, allocation identity, Rust memory-region identity, reserve bounds,
and fully acknowledged memory before mutation. A successful operation commits
exactly the guard page as zeroed RW/NX storage, moves the guard down one page,
and emits one ordinary `MappingOperation::Commit` plus one dirty page.

The first fault happens exactly at the page base. Continued execution proves
that this cannot be the general classifier: the second fault is `0x006FDD94`,
inside the next guard page. The implementation therefore classifies the page
containing the fault, while still rejecting a fault in any unrelated page.

The public `runtime_initial_thread_stack` contract verifies the 1 MiB／4 KiB
layout, allocation record, zeroed RW delta, guard movement, non-present PFEC,
nonaligned in-page fault, and mutation-free rejection for unrelated,
protection, duplicate, and exhausted faults.

## Node result

`npm run check:runtime:bgi` applies four growth deltas before reaching the next
unsupported endpoint:

| Instruction | Fault / PFEC | Committed page | Next guard | Node generation |
|---:|---|---|---|---|
| 22,754 | `0x006FE000` / `4` | `0x006FE000` | `0x006FD000` | `365 -> 366` |
| 22,782 | `0x006FDD94` / `6` | `0x006FD000` | `0x006FC000` | `367 -> 368` |
| 22,812 | `0x006FC000` / `4` | `0x006FC000` | `0x006FB000` | `371 -> 372` |
| 22,817 | `0x006FB000` / `4` | `0x006FB000` | `0x006FA000` | `372 -> 373` |

Each zero page has payload SHA-256
`ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7`.
The second fault demonstrates a write access; the other observed adapter PFECs
are non-present reads. Both access classes use the same guard-page rule.

After growth, call 268 is the second `GetModuleFileNameA(NULL, 0x006FE0A4,
780)` occurrence at instruction `22,789`. It writes the existing virtual path
`C:\Vine\BGI.exe` and returns `15`. The Node session records 273 writeback
batches, 412 dirty pages, 5,929 changed bytes, 72 host deltas, and final
generation `374`. Final memory SHA-256 is
`ca1d082833b4349640451886843761aafecc0a5e63e5771694e09e5c1906c0aa`.
The 1,179,850-byte report SHA-256 is
`f8573f9bdd83a00d264fc870cf47ab4e1f63ff9332e4db1f938699245e1e3340`.

## Chrome equivalence and workload gate

Chrome `150.0.7871.115` reproduces all four fault identities, committed pages,
guard movements, the 268 ordered calls, instruction `22,843`, and the final
memory digest. It uses 2,939 CPU slices, 2,666 budget yields, 2,258 writeback
batches／2,934 pages, 72 mapping deltas, and 2,606 Rust/Wasm crossings. Its
generation ends at `2,010`; Node ends at `374` because their dirty-page
reconciliation boundaries differ. Every individual growth is nevertheless a
one-generation commit in both paths.

Success, cancellation, and watchdog cleanup leave zero pending host calls and
callbacks. The 265,458-byte Chrome report SHA-256 is
`7ae7759f0f24ed440c0f80457820feead35263c6067a99faf4ddc18b4daf6093`.

The separate 180-second workload gate completes 719 fresh sessions with zero
failure and passes all 17 versioned thresholds. Session P95 is `293.185 ms`,
slice P99 is `0.730 ms`, heartbeat P99 is `6.070 ms`, and the maximum heartbeat
gap is `17.905 ms`. The 1,224,346-byte report SHA-256 is
`98494a5c9986e03e70448325ab2f761e66a242a08b4d2a81dceba315f2e14489`.

## Next structured stop

```text
completed calls:       268
instruction:           22843
endpoint:              kernel32.dll!MultiByteToWideChar
endpoint address:      0x7F000810
return address:        0x1002BB13
arguments:             0, 0, 0x006FE0A4, -1, 0x006FD150, 780
fault class:           unsupported
fault stage:           win32-code-page
fault code:            multi-byte-to-wide-char-variant-unsupported
```

The existing policy enables explicit code page `932` with flags `1`. This call
uses `CP_ACP (0)` and flags `0`; it is the next trace-driven capability item.

## Non-claims

- stack overflow and guard-page exception delivery are not implemented;
- additional guest threads, their stacks, TEBs, or CPU switching are not added;
- the address of a fault inside a guard page is not required to equal its base;
- no general SEH, rendering, input, audio, VFS, COM, or full-play completion is
  claimed.
