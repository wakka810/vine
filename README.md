# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-first-route-ending`: A complete route reaches an ending with readable rendering, responsive choices, sustained audio, save/load resume, and no unsupported stop |
| Last completed boundary | `primary-save-slot-restart-load`: Release Chrome advanced the post-Q.Load path through action 620, completed a new Q.Save at action 621 and its confirmation at action 622 with a bounded 480 MiB guest set, then a fresh process hydrated the exact 94,564-byte save and resumed visible presentation through action 60; no unsupported endpoint, semantic fault, WebGL error, pending host work, or source mutation was observed |
| Current forward stop | `post-action-622-qload-visible-progression-action-60` / `none-within-bounded-forward-run`: The action-622 save run completed all requested input and committed slot 102 at generation 1 with SHA-256 bd6187200187529a0a1b43bc23486a68e37c48b8394b013ec50384bea315857e; the restarted release run hydrated the same 94,564 bytes, produced 153 Presents, completed pointer action 60 on a non-black final frame, and stopped intentionally with a null endpoint, no semantic fault, and no pending host work |
| Next development goal | Continue from the validated action-622 Q.Save/Q.Load checkpoint beyond action 60 to the next route choice, first ending, or next real unsupported stop |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution, checked GuestMemory synchronization, process/thread scheduling, callbacks, and generic forward capture sustain the action-622 save boundary; save compression completes with a bounded 480 MiB guest set inside the 512 MiB execution-mirror cap |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, primary down/up dispatch, and native double-click generation are active; Q.Save completed at actions 621-622, and the restarted Q.Load run completed visible post-load input through action 60 |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the first-choice no-Enter and Enter A/B runs end on distinct deterministic frames with WebGL error zero |
| `source-storage` | `partial` | Immutable source reads and a title-local SQLite overlay remain separated; the 94,564-byte action-622 Q.Save is hash-preserved in .work slot 102 and hydrates unchanged after restart, while the executable and source assets remain unchanged |
| `audio` | `partial` | Reached DirectSound streaming buffers support concurrent non-overlapping partial and wrapping Lock/Unlock, captured notification waits, cursor/status/control, and Play/Stop through generation-safe Chrome Web Audio transport; multiple non-zero looping and one-shot playbacks span the representative flow |

### Important Non-Claims

- Only the first default route-choice activation is accepted; alternate options and later choices are not yet accepted
- The action-622 checkpoint is .work-only and covers one 94,564-byte Q.Save with SHA-256 bd6187200187529a0a1b43bc23486a68e37c48b8394b013ec50384bea315857e restored through slot 102; visible post-Q.Load input is accepted only through action 60
- The measured route live set is bounded to 480 MiB; later-route peak memory and long-run reclamation are not yet accepted
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
