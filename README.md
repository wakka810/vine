# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-representative-flow`: A bounded real scenario covers launch, readable rendering, multiple inputs, audio, a scene transition, persistent state, restart, and restore without an unsupported stop |
| Last completed boundary | `primary-visible-action-response-or-readable-frame`: Chrome rendered the complete title including Config and Exit, consumed Start through the authoritative pointer path, advanced into a readable Japanese intro-poem frame, consumed and presented multiple subsequent scenario clicks, and played non-zero primary PCM while source files remained unchanged |
| Current forward stop | `readable-intro-frame-after-scenario-pointer-campaign` / `none-within-bounded-forward-run`: The release Chrome run reached 95 Presents, completed Start plus four post-title pointer actions and consumed the next release, retained audible streaming and atomic writable state, and stopped only at the former 14.4-billion-instruction development watchdog with no unsupported API, semantic fault, pending callback, or pending host request |
| Next development goal | Use the optimized JIT-enabled mirror path and higher wall-authoritative instruction budget to advance through the intro poem into the first dialogue or background-scene transition, implementing any reached semantic gap before considering a commit-bound cold-prefix resume cache |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution, checked GuestMemory synchronization, process/thread scheduling, callbacks, and generic forward capture are active; mirror host-delta preflight and guest reads use bounded indexes without weakening atomic validation |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, and primary down/up dispatch are active; Start and multiple centered post-title clicks are guest-consumed and visibly presented |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the complete title artwork and a readable Japanese intro-poem frame render with owner-validated texture updates |
| `source-storage` | `partial` | Immutable source reads and a title-local forward.sqlite overlay are separated; BGI.gdb commit generation and content survive browser/local-service restart and are hash-checked before Rust hydration |
| `audio` | `partial` | Reached DirectSound streaming buffers, partial and wrapping Lock/Unlock, captured notification waits, status/cursor control, and Play/Stop publish non-zero primary PCM through generation-safe Chrome Web Audio transport |

### Important Non-Claims

- The intro poem is readable and multiple post-title clicks are consumed, but the first dialogue/background scene transition and a representative scenario flow are not yet accepted
- The persistence milestone covers the reached title-local BGI.gdb configuration overlay; an in-story save-slot and Load-menu roundtrip are not yet claimed
- Primary audio progresses and content updates are observed, but the reported two-to-three-repeat cadence, format breadth, suspend/resume behavior, and long-run streaming stability are not yet accepted
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
