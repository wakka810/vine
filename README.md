# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-first-route-ending`: A complete route reaches an ending with readable rendering, responsive choices, sustained audio, save/load resume, and no unsupported stop |
| Last completed boundary | `primary-first-route-choice`: Release Chrome restored an 80,589-byte title-local Q.Save into the first route-choice input wait, consumed exactly one authoritative Enter down/up activation, and produced a deterministic visible response; the matched no-Enter and Enter A/B runs ended on distinct frame hashes with no unsupported endpoint, pending work, WebGL error, or source mutation |
| Current forward stop | `post-first-route-choice-visible-response` / `none-within-bounded-forward-run`: A .work-only choice snapshot reduced the replay from 567 seconds to 188 seconds; matched 45-second A/B runs reached frame SHA-256 14485586aa9fd6a4cf0f16cd6e5f8bddcb403d7cdbb2a0c6803460560e2e1575 without Enter and 6940bfdd33cfdad1821c100c188cd8b9fdbd64f4744c8bde97b642bd0ab65437 after one Enter activation, then stopped only at the bounded cancellation with no unsupported endpoint, pending callback, pending host request, WebGL error, or source mutation |
| Next development goal | Capture a post-choice .work-only Q.Save, resume the selected branch from that bounded snapshot, and continue to the next route choice, first ending, or next real unsupported stop |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution, checked GuestMemory synchronization, process/thread scheduling, callbacks, and generic forward capture sustain the first-choice boundary; the measured two-buffer route live set remains bounded to 448 MiB within a 512 MiB execution-mirror cap |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, primary down/up dispatch, and native double-click generation are active; title and Load controls consume authoritative pointer messages, while one authoritative Enter down/up produces the accepted first-choice visible response |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the first-choice no-Enter and Enter A/B runs end on distinct deterministic frames with WebGL error zero |
| `source-storage` | `partial` | Immutable source reads and a title-local SQLite overlay remain separated; an 80,589-byte Q.Save is hash-preserved into .work slot 004 and restores the first-choice wait, while the executable and source assets remain unchanged |
| `audio` | `partial` | Reached DirectSound streaming buffers support concurrent non-overlapping partial and wrapping Lock/Unlock, captured notification waits, cursor/status/control, and Play/Stop through generation-safe Chrome Web Audio transport; multiple non-zero looping and one-shot playbacks span the representative flow |

### Important Non-Claims

- Only the first default route-choice activation is accepted; alternate options and later choices are not yet accepted
- The current route snapshot is .work-only and covers one 80,589-byte Q.Save with SHA-256 cea28d54adc3c9f56a0cbc7bfe9e4bd39ee1a3a5f6b90c1394a179acf63a6223 restored through slot 004
- The measured route live set is bounded to 448 MiB; later-route peak memory and long-run reclamation are not yet accepted
- Primary audio spans the representative flow, but format breadth, suspend/resume behavior, and long-run streaming stability are not yet accepted
- No complete route, ending, alternate route, or long-run acceptance is claimed
- Historical per-API capability profiles are frozen and are not the active development queue

### Commands

```sh
npm run dev:target
npm run check:fast
npm run check:milestone
```
<!-- END GENERATED CURRENT STATUS -->

## Development

Run the private title forward in the release Chrome path:

```sh
npm run dev:target
```

The command writes a bounded diagnostic under `.work/target-forward/` and stops at the next unsupported behavior, fault, or visible milestone. Implement reached leaves in the owning Rust subsystem, run focused tests, and continue. Historical capability profiles and full Node／Chrome evidence are not part of this default loop.

Use `npm run check:fast` for normal development. Run `npm run check:milestone` once after a readable frame、visible input response、primary audio、save／restart／load、representative flow、or another authority-changing milestone.

## Architecture

Rust owns guest-visible Windows state, including memory、process／thread、handles、COM、filesystem、graphics、audio、input、and LastError. TypeScript owns browser resources and adapters only. Source executables、DLLs、and assets are immutable; save and configuration state live in a separate writable overlay.

See the [documentation index](docs/README.md), [requirements](docs/requirements.md), [architecture](docs/target-architecture.md), and [roadmap](docs/roadmap.md).
