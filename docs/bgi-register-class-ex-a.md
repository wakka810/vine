# BGI `RegisterClassExA` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-register-class-ex-a`
- Previous capability checkpoint: `bgi-load-cursor-a`
- First unsupported capability: `user32-get-system-metrics`

This subsystem gateway registers the two reached process-local window classes.
Rust owns class identity, case-folded module/name lookup, logical ATOM allocation,
WndProc metadata, resource references, and process teardown. TypeScript receives
an immutable completion snapshot for semantic equivalence and does not own a
Win32 class, ATOM, callback, HICON, or HCURSOR.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/register-class-ex-a-v1.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event register-class-ex-a-complete `
  --observe register-class-ex-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 326,543 | 804 | 366 | `003cfc9d9c9de92f9b0dad3421393d757fc7641a5c2661b27898c28122df7a97` |
| Confirmation | 327,416 | 805 | 366 | `0cd19660c72f5ec35a83ed25f59486f814ca908783a412c5d13af14e8951345b` |

Both captures have balanced call IDs and zero hook, vtable, stack, agent, and
cleanup diagnostics. The immutable executable remained 1,193,472 bytes with
SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.
Raw traces and summaries remain under ignored `.work/` paths.

The guarded x86 oracle establishes these guest semantics:

- a valid 48-byte `WNDCLASSEXA` succeeds and preserves seeded LastError;
- the same module/name is rejected with ATOM zero and
  `ERROR_CLASS_ALREADY_EXISTS` (1,410);
- `cbSize=47` is rejected with ATOM zero and `ERROR_INVALID_PARAMETER` (87);
- correcting that invalid structure to 48 bytes succeeds, proving the failure
  did not reserve class identity;
- successful registrations receive distinct opaque native ATOMs;
- `GetClassInfoExA` resolves the class by both name and ATOM;
- `UnregisterClassA` succeeds and a later query is rejected; and
- every 48-byte input remains unchanged.

BGI registers two classes consecutively through the same contract:

| Field | Main | Child |
|---|---:|---:|
| Return RVA | `0x00067D28` | `0x00067DAB` |
| `cbSize` | 48 | 48 |
| style | `0x00000028` | `0x00003020` |
| WndProc RVA | `0x00092550` | `0x0002E6E0` |
| class name | `BGI - Main window` | `BGI - Child window` |

Both use the main module HINSTANCE and the same previously loaded icon and
system-arrow cursor. Class/window extra bytes are zero; brush, menu name, small
icon are NULL. Both calls succeed with LastError zero and preserve all input
bytes. Native ATOM, HICON, and HCURSOR values are opaque artifact identities and
are not imported into Vine.

## Policy, ABI, and ownership

User32 policy version 5 enables a required 48-byte input structure, ASCII names
NUL-terminated within 64 bytes, styles `0x28` and `0x3020`, zero extra bytes,
NULL optional fields, a mapped local module, a same-module executable WndProc,
and live logical icon/cursor references. Integer/global/non-ASCII class names,
brush/menu/small-icon variants, and other styles stop before mutation.

`RegisterClassExA` is one-argument x86 stdcall, returns a U16 value zero-extended
into EAX, performs no guest write, and preserves LastError on success. Bad
`cbSize` returns zero/87; a duplicate module/name returns zero/1,410. Pointer
access is checked before field interpretation. All module, execute, resource,
policy, capacity, and identity validation completes before the state commit.

Private `runtime/user32.rs::RuntimeUser32` owns:

- monotonic class IDs and logical ATOMs beginning at `0xC000` in namespace
  `vine.logical-window-class-atom.v1`;
- a class collection plus module/name and ATOM indexes;
- ASCII case-folded `(module ID, class name)` identity;
- copied module hash/base, WndProc RVA/address/stdcall descriptor, styles, names,
  and non-owning logical icon/cursor identity pairs; and
- generation-checked registration and process teardown plans.

Registration planning is mutation-free. Stale, duplicate, invalid-reference,
invalid-metadata, capacity, and ATOM-exhaustion paths do not advance allocators
or modify indexes. Teardown retires classes before their referenced icon and
cursor collections in one User32 lifecycle commit. Guest pointers are not kept
in the authoritative record.

Representative tests:

```powershell
cargo test -p vine-core --lib user32:: --locked
cargo test -p vine-core --test user32_dispatcher --locked
cargo check -p vine-pe-probe -p vine-runtime-wasm --locked
npm run typecheck:runtime:browser-worker
npm run check:windows-trace
```

Dispatcher negatives cover bad structure size/pointer, wrong module,
non-executable callback, unknown icon/cursor, unsupported fields/style/name,
disabled policy, duplicate name, success LastError preservation, input-memory
preservation, first-ATOM reuse after failure, and process teardown.

## Node, Chrome, and benchmark evidence

```powershell
npm run check:runtime:bgi -- --profile user32-register-class-ex-a
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 1,344 calls across 62 contracts. Main and child registrations are
sequences 1,341 and 1,344 at instructions 49,957 and 49,984. They return logical
ATOMs `0xC000` and `0xC001`, preserve LastError zero, make no guest writes, and
carry immutable class snapshots. The intervening second `LoadIconA` and
`LoadCursorA` calls reuse logical identities 1 without allocating new resources.
Node records 1,350 writeback batches, 2,547 dirty pages, 9,236 changed bytes, 85
host deltas, final generation 1,464, and memory SHA-256
`1aa4139c9a3a70a3f2090a6381212846a3171f40cfb93954867a2dc7efe6011c`.
The 5,489,595-byte report has SHA-256
`58fd6b9636e595a942ed1e3e3e0b02cf6ad5a033942f25eb97d5ddf970a58e47`.

Chrome Runtime Worker reproduces every ordered completion, class snapshot,
memory digest, and next stop with 6,855 slices, 5,505 yields, 4,982 writeback
batches/6,732 pages, 85 mapping deltas, final generation 3,618, and 6,420 Wasm
crossings. Cancel and watchdog paths finish with no pending request or callback.
The 813,260-byte browser report has SHA-256
`20cb8d042aa22114ceaf7fb78bf48ff1fec87e81020168d4b4944278d0e25b72`.
Runtime-core Wasm is 2,516,544 bytes with SHA-256
`56cbc4b04d570c623548795b7f9a9cddbac7f7eb198f24f7f85bb3b89745f292`;
the Worker bundle is 122,207 bytes with SHA-256
`49eb66e1c8ed2e6aa6dfa335deba66385b8d774898a60f4082ea3b091fdafe36`.

The three-minute gate completed 363 sessions with zero failures and passed all
17 unchanged thresholds. Session p95 was 539.585 ms, slice p99 was 1.770 ms,
heartbeat p99 was 6.030 ms, and the maximum heartbeat gap was 18.810 ms. Main
and Worker heap slopes were 344,309.74 and 641,072.77 bytes/minute. The
1,067,375-byte benchmark report has SHA-256
`04a470934c28677e899841004317fe6c637bc80abfaa3a5826ee2dddb05f13c1`.

## Non-claims

- `GetClassInfoExA`, `UnregisterClassA`, global classes, integer names,
  non-ASCII names, class brushes/menus/small icons, and general style variants
  are not implemented guest APIs.
- Logical ATOM values intentionally do not match native ATOM values.
- A stored WndProc descriptor is not yet window creation, callback dispatch,
  message routing, rendering, focus, input, or browser DOM state.
- Class references do not own or extend icon/cursor lifetime independently of
  the process-wide User32 teardown.

## Next structured stop

```text
USER32.dll!GetSystemMetrics(7)
endpoint ID:       203
endpoint address:  0x7F000CA0
IAT address:       0x100D1404
return address:    0x10067DDB
instruction count: 49,990
fault:             endpoint-contract-missing
```
