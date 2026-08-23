# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-first-route-ending`: A complete route reaches an ending with readable rendering, responsive choices, sustained audio, save/load resume, and no unsupported stop |
| Last completed boundary | `primary-post-choice-qsave-restart-qload`: Release Chrome created an 86,341-byte title-local Q.Save in slot 102, restarted with that exact hash hydrated, and consumed the authoritative Q.Load press/release plus the following pointer press/release; no unsupported endpoint, semantic fault, WebGL error, or source mutation was observed |
| Current forward stop | `post-qload-input-consumed-awaiting-presentation` / `none-within-bounded-forward-run`: The restarted run hydrated slot 102 at generation 1 with SHA-256 b28d66fe43e8920fb56a3907498c6125a5495267bf7d8fc9af88aa97450c8a70 and consumed Q.Load plus the next pointer input, then produced no further Present before the 500-second watchdog; the only stop was forward-run-budget-exhausted with a null endpoint and no semantic fault |
| Next development goal | Resolve the post-Q.Load no-Present wait, then continue from the hydrated checkpoint to the next route choice, first ending, or next real unsupported stop |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution, checked GuestMemory synchronization, process/thread scheduling, callbacks, and generic forward capture sustain the first-choice boundary; the measured two-buffer route live set remains bounded to 448 MiB within a 512 MiB execution-mirror cap |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, primary down/up dispatch, and native double-click generation are active; Q.Save and Q.Load controls consume authoritative pointer messages, and the restarted run also consumed the first post-load pointer input |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the first-choice no-Enter and Enter A/B runs end on distinct deterministic frames with WebGL error zero |
| `source-storage` | `partial` | Immutable source reads and a title-local SQLite overlay remain separated; an 86,341-byte Q.Save is hash-preserved in .work slot 102 and hydrates unchanged after restart, while the executable and source assets remain unchanged |
| `audio` | `partial` | Reached DirectSound streaming buffers support concurrent non-overlapping partial and wrapping Lock/Unlock, captured notification waits, cursor/status/control, and Play/Stop through generation-safe Chrome Web Audio transport; multiple non-zero looping and one-shot playbacks span the representative flow |

### Important Non-Claims

- Only the first default route-choice activation is accepted; alternate options and later choices are not yet accepted
- The post-choice checkpoint is .work-only and covers one 86,341-byte Q.Save with SHA-256 b28d66fe43e8920fb56a3907498c6125a5495267bf7d8fc9af88aa97450c8a70 restored through slot 102; a post-Q.Load Present is not yet accepted
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
