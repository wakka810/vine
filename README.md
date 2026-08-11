# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-visible-action-response-or-readable-frame`: A repeatable visible response to real pointer/keyboard input or a readable non-black primary frame |
| Last completed boundary | `primary-audible-output`: The primary title created and filled a DirectSound streaming buffer, armed position notifications, entered looping playback, and delivered non-zero three-second PCM to a running Chrome Web Audio context |
| Current forward stop | `development-boundary` / `forward-run-budget-exhausted`: After primary audio started, the scheduler passed the former PeekMessageA thread-state fault and continued through the title loop until the bounded forward watchdog; no feasibility blocker was reached |
| Next development goal | Continue the post-audio title loop toward a visible input response or readable frame, implementing the next reached semantic family or smallest gateway slice without reviving per-API checkpoints |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, x86 execution mirror, checked GuestMemory synchronization, process/thread scheduling, callbacks, and reached Win32/COM dispatch |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, pointer down/up dispatch |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes published through WebGL2; latest verified center pixel remained black |
| `source-storage` | `partial` | Immutable source snapshot reads and a separate writable profile foundation; primary restart/load is not reached |
| `audio` | `partial` | Reached DirectSound streaming buffers, partial and wrapping Lock/Unlock, position notifications, status/cursor control, looping Play, and non-zero primary PCM through Chrome Web Audio |

### Important Non-Claims

- No readable primary title frame or repeatable visible input response is claimed yet
- Primary audio format breadth, suspend/resume behavior, and long-run streaming stability are not yet accepted
- No primary save/browser-restart/load flow is claimed
- No representative route, ending, or long-run acceptance is claimed
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
