# BGI `GetSystemMetrics` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-get-system-metrics`
- Previous capability checkpoint: `bgi-register-class-ex-a`
- First unsupported capability: `kernel32-private-heap-size-67108864`

This semantic leaf exposes only the five system metrics consumed while BGI
derives its initial window geometry. Screen dimensions come from the existing
Rust display authority; bounded non-client dimensions come from versioned
User32 compatibility policy. No native, DOM, or browser-window metric becomes
guest-visible state.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/get-system-metrics-v1-20260717-a.jsonl `
  --seconds 45 --kill-after `
  --stop-on-event get-system-metrics-complete `
  --observe get-system-metrics
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary | 55,901 | 99 | 22 | `368d62d858ca8b26102b330d805ca1b7671deda64f7d9d6159ef08985f717d4f` |
| Confirmation | 55,839 | 99 | 22 | `32884be3566ecb1b15d25fc91b42a44eab223b1cd932a3ff3396fd1882036f08` |

Both captures have balanced call IDs and zero hook, vtable, call-stack,
agent-message, or cleanup diagnostics. The immutable executable remained
1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`.
Raw traces and summaries remain under ignored `.work/` paths.

The guarded x86 oracle calls each reached index with a distinct LastError
sentinel. Hooked and direct `SystemFunction` results agree, and every successful
call preserves its sentinel:

| Index | Win32 metric | Native value |
|---:|---|---:|
| 7 | `SM_CXFIXEDFRAME` | 3 |
| 8 | `SM_CYFIXEDFRAME` | 3 |
| 4 | `SM_CYCAPTION` | 23 |
| 0 | `SM_CXSCREEN` | 1,920 |
| 1 | `SM_CYSCREEN` | 1,080 |

BGI performs those five calls in exactly that order and returns to RVAs
`0x00067DDB`, `0x00067DE6`, `0x00067DEE`, `0x00067E26`, and
`0x00067E2E`. All five reached calls observe LastError zero. The immediate
consumer doubles the fixed-frame width to 6, doubles the fixed-frame height
and adds the caption to obtain 29, then consumes the screen width／height ratio
while selecting the initial window geometry. It does not treat the returned
integers as native handles or persist host display identity.

## Policy, ABI, and authority

User32 policy version 6 selects profile
`initial-window-metrics-1920x1080-v1`. It owns caption height 23 and fixed-frame
width／height 3. It does not own screen geometry: indices 0 and 1 are derived
from the primary monitor rectangle held by
`runtime/displays.rs::RuntimeDisplays`. The configured display profile ID,
primary flag, and rectangle must still match the selected User32 display
profile before any value is returned.

`user32.get-system-metrics.v1` is one-argument x86 stdcall with a 32-bit result,
callee cleanup, LastError preserve, no callback, no guest write, and no runtime
mutation. The reached values are nonnegative and therefore use the existing
U32 register representation without claiming support for signed virtual-screen
coordinates.

Only indices 0, 1, 4, 7, and 8 are enabled. A disabled metrics policy returns
`get-system-metrics-policy-disabled`; an unlisted index returns
`get-system-metrics-index-unsupported`; missing or mismatched Rust display
authority is a runtime defect. Every such path stops before memory, LastError,
display state, or allocator mutation. The semantic handler contains no BGI
RVA, call occurrence, executable hash, native screen query, or browser branch.

Representative tests:

```powershell
cargo test -p vine-core --lib user32:: --locked
cargo test -p vine-core --test user32_dispatcher --locked
cargo check -p vine-pe-probe -p vine-runtime-wasm --locked
npm run check:windows-trace
```

The dispatcher fixture fixes ABI cleanup, all five results, LastError
preservation, zero writes, unchanged memory generation, policy-disabled and
unlisted-index faults, missing authority, and mismatched authority.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-get-system-metrics
npm run check:runtime:bgi-browser
```

Node completes 1,349 calls across 63 contracts. The metric calls are sequences
1,345 through 1,349 at instructions 49,990, 49,995, 50,000, 50,020, and 50,024.
They return `3, 3, 23, 1920, 1080`, preserve LastError zero, write no guest
bytes, and advance no memory generation themselves.

Execution reaches the next structured stop at instruction 50,105. Node records
1,355 writeback batches, 2,555 dirty pages, 9,442 changed bytes, 85 host deltas,
final generation 1,469, and memory SHA-256
`79534ea1107d5cb023fa0d2a0d23d766195067f6634301dfc5d14d782f4405f9`.
The 5,508,907-byte report has SHA-256
`512f69d2349d77e4266fabc9124d1c486621f06fa2801bbd556414a8fa957d8a`.

Chrome Runtime Worker reproduces the same ordered calls, results, memory digest,
and stop with 6,872 slices, 5,517 yields, 4,998 writeback batches／6,751 pages,
85 mapping deltas, final generation 3,634, and 6,441 Wasm crossings. Cancel and
watchdog cleanup leave no pending host call or callback. The 815,811-byte
browser report has SHA-256
`329b35a45adb2b458cbb19406225a0482155057b756aaf3408b6d10304147ce7`.
Runtime-core Wasm is 2,522,438 bytes with SHA-256
`8892012b40e1fab19b0c56e423c18624f620781fc9020519232c3a095003b4b2`.

The three-minute performance gate was not rerun. This read-only semantic leaf
changes no Worker transport, scheduler, memory topology, graphics, audio, or
lifecycle boundary. The manifest retains the explicitly labeled previous
`user32-register-class-ex-a` observation and does not claim it measured this
Wasm artifact.

## Non-claims

- Unlisted system-metric indices, multi-monitor virtual-screen coordinates,
  DPI-aware metrics, theme-dependent values, and per-monitor metrics are not
  implemented.
- The 1,920×1,080 geometry is a controlled logical display profile, not host
  screen detection or a promise that every title profile uses that size.
- This checkpoint does not create a logical window, DOM canvas, renderer,
  message route, input path, or browser display association.
- Later icon-size metric calls are not enabled ahead of the intervening file
  and window subsystem gateways.

## Next structured stop

```text
KERNEL32.dll!HeapAlloc(0x68010001, 0, 0x04000000)
endpoint ID:       75
endpoint address:  0x7F0004A0
IAT address:       0x100D11D4
return address:    0x100A4A33
instruction count: 50,105
fault:             heap-alloc-variant-unsupported
cluster:           kernel32-private-heap-allocation
```

The requested 64 MiB exceeds the active 16 MiB logical heap range. Before
expanding policy or backing memory, the next checkpoint must establish native
allocation lifetime and consumer behavior, quantify Rust／Wasm／v86 memory
cost, and preserve allocation failure atomicity and browser responsiveness.
