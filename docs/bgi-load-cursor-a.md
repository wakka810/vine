# BGI `LoadCursorA` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-load-cursor-a`
- Previous capability checkpoint: `bgi-load-icon-a`
- First unsupported capability: `user32-register-class-ex-a`

This checkpoint implements the reached `LoadCursorA(NULL, IDC_ARROW)` leaf.
Rust owns the shared logical cursor identity and process lifetime. TypeScript
receives immutable cursor metadata for semantic equivalence and does not own a
Win32 HCURSOR or browser cursor resource.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/load-cursor-a-v1.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event load-cursor-a-complete `
  --observe load-cursor-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 333,790 | 806 | 367 | `252195f84e3d3d7e24d8f34b5531d248e738a7600ca9c2b1241663d31fb21382` |
| Confirmation | 333,385 | 805 | 366 | `30d6ec4272b6700a7d481276205ce25cbce7527e5ba22ff5820024a7c1cc844c` |

Both captures have balanced call IDs and zero hook, vtable, stack, agent, and
cleanup diagnostics. The source executable remained 1,193,472 bytes with
SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.

The guarded x86 oracle calls `LoadCursorA(NULL, IDC_ARROW)` twice. Both calls
succeed, preserve the seeded LastError, and return the same HCURSOR. Cursor
system metrics are 32×32. Missing predefined ID 65,535 returns NULL and sets
LastError 1,814 (`ERROR_RESOURCE_NAME_NOT_FOUND`). Hooked and direct system
results agree. Native HCURSOR values are opaque artifact identity and are not
imported into Vine.

BGI calls `LoadCursorA` four times. Return RVAs `0x00067D09`, `0x00067D8C`,
and `0x00067FD2` request `NULL, IDC_ARROW` and share the same native system
cursor identity. RVA `0x00067FE1` requests module resource 106 and succeeds on
Windows. The first arrow result becomes `WNDCLASSEXA.hCursor` immediately
before `RegisterClassExA`. Vine reaches only that first system-arrow call in
the current session, so the module resource and later occurrences remain
observed requirements rather than implemented behavior.

## Policy and ownership

User32 policy version 4 enables exactly one cursor variant:
`LoadCursorA(NULL, IDC_ARROW)`. Success preserves the selected thread's
LastError and performs no guest write. ID 65,535 is the bounded missing-name
negative oracle and returns NULL／1,814. Other predefined cursors, a non-NULL
module instance, string names, and a disabled cursor policy stop before
mutation with explicit unsupported faults.

Private `runtime/user32.rs::RuntimeUser32` owns the monotonic cursor ID,
logical handle namespace `vine.logical-cursor.v1`, source index, handle index,
and generation. The first cursor is ID 1／HCURSOR `0x43010001`; the process
limit is 256 shared cursor identities. Load is a mutation-free plan followed
by stale-checked preflight and targeted commit. Repeating the same system
source reuses identity without consuming an allocator or changing guest
memory. Cursor and icon plans share the User32 generation, so a cross-resource
commit cannot accept a stale plan.

Process teardown preflights icon and cursor counts and retires both private
identity sets in one User32 lifecycle commit. The logical HCURSOR is not a
Win32 object-table handle, COM pointer, native HCURSOR, DOM cursor value, or
browser resource ID. A future browser association must remain explicit and
non-owning.

Representative tests:

```powershell
cargo test -p vine-core --lib user32:: --locked
cargo test -p vine-core --test user32_dispatcher --locked
cargo check -p vine-pe-probe -p vine-runtime-wasm --locked
npm run check:js
npm run typecheck:runtime:browser-worker
npm run check:windows-trace
```

The negative contracts cover missing ID 65,535, unsupported predefined／module／
string variants, disabled policy, identity reuse, stale cross-resource plans,
capacity／allocator checks, and process teardown without memory, index, or
allocator drift.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-load-cursor-a
npm run check:runtime:bgi-browser
```

Node completes 1,340 calls across 61 contracts. `LoadCursorA` is sequence
1,340 at instruction 49,949, returns `0x43010001`, preserves LastError 0, and
emits the immutable system-arrow snapshot with no guest write. Node records
1,346 writeback batches, 2,543 dirty pages, 9,208 changed bytes, 85 host
deltas, final generation 1,460, and memory SHA-256
`8c580b84b365739f602e1e2c2307b87ecbd605d8bf28a52590e844f203af728f`.
The 5,471,491-byte report has SHA-256
`8511c805a287a324ea34b6709af3618d66d3f086c5f55ed9192213483b55f581`.

Chrome Runtime Worker reproduces every ordered completion, cursor metadata,
memory digest, and next stop with 6,850 slices, 5,504 yields, 4,977 writeback
batches／6,727 pages, 85 mapping deltas, final generation 3,613, and 6,411
Wasm crossings. Cancel and watchdog paths finish with no pending request or
callback. The 808,002-byte browser report has SHA-256
`18aa9a064327def1945685039d16941d487a2473cef680f0bda26ed77ccf9105`.
Runtime-core Wasm is 2,461,281 bytes with SHA-256
`1731911cc13c5b2749fbdc4c71af27a6e23e4fb449a69b5a84b2775d6b0149c7`;
the Worker bundle is 121,889 bytes with SHA-256
`3ee6730692f1369d08c6af72f0878e977c7dd6c850dabe09f73525acec57af9b`.

Because this checkpoint changes User32 process lifetime and the Worker
completion payload, the three-minute workload gate was rerun. It completed
376 sessions with zero failure and passed all 17 versioned thresholds.
Session p95 was 491.795 ms, slice p99 was 2.290 ms, heartbeat p99 was
6.020 ms, and the maximum heartbeat gap was 20.630 ms. The 1,072,091-byte
benchmark report has SHA-256
`4795b50b1561aac01c5fe08fb0442e6c46521f5d9576c8fd2b1090c7a73a3c4e`.

## Non-claims

- Module cursor resource 106, later BGI cursor loads, other predefined
  cursors, string names, cursor destruction, and active-cursor selection are
  not implemented by this checkpoint.
- The logical HCURSOR has no shape bitmap, hotspot, native handle, CSS cursor,
  DOM event behavior, or browser renderer resource.
- Class registration, logical window creation, message routing, and rendering
  are unchanged and remain outside this semantic leaf.

## Next structured stop

`RegisterClassExA` is a subsystem gateway because it introduces persistent
class identity, atom lifetime, callback ownership, module association, and
cross-field validation rather than an isolated return value.

```text
USER32.dll!RegisterClassExA(0x006FFB08)
endpoint ID:       191
endpoint address:  0x7F000BE0
IAT address:       0x100D13D4
return address:    0x10067D28
instruction count: 49,957
fault:             endpoint-contract-missing
```
