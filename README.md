# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-representative-flow`: A bounded real scenario covers launch, readable rendering, multiple inputs, audio, a scene transition, persistent state, restart, and restore without an unsupported stop |
| Last completed boundary | `primary-save-browser-restart-load`: The primary title atomically committed its BGI.gdb configuration overlay from generation 1 to 2; a separate Chrome and local-service run loaded the same 123657 bytes and SHA-256 into the authoritative Rust runtime while source files remained unchanged |
| Current forward stop | `title-action-consumed-without-scene-transition` / `primary-title-start-action`: The generic forward runner rendered the complete title artwork, waited for the Start region to stabilize, delivered its pointer move and primary down/up through WndProc and GetAsyncKeyState, held the title press across seven Present calls, completed ten Enter activations, played non-zero title audio, and reached no unsupported endpoint; the first scene transition was not observed before the bounded run ended |
| Next development goal | Add a commit- and engine-bound development resume cache at the stable title boundary, then use fast iterations to classify the remaining title hit-test or action-state requirement and enter the first scenario |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, x86 execution mirror, checked GuestMemory synchronization, process/thread scheduling, callbacks, and generic forward capture from the first API without historical exact-checkpoint gating |
| `window-input` | `gateway` | Logical window, message queue, timers, focus/cursor state, keyboard scan, and Start-coordinate primary down/up dispatch are active; the remaining gateway is the title engine's action consumption and scene transition |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the complete title artwork renders and owner-validated bounded UpdateTexture dirty regions are confirmed past their reached call |
| `source-storage` | `partial` | Immutable source reads and a title-local forward.sqlite overlay are separated; BGI.gdb commit generation and content survive browser/local-service restart and are hash-checked before Rust hydration |
| `audio` | `partial` | Reached DirectSound streaming buffers, partial and wrapping Lock/Unlock, notifications, status/cursor control, Play/Stop with generation-safe browser transport, and non-zero primary PCM through Chrome Web Audio |

### Important Non-Claims

- The Start-coordinate down is guest-observed and the matching up is committed to Rust input authority, but its WndProc consumption, a first-scene transition, and representative flow are not yet accepted
- The persistence milestone covers the reached title-local BGI.gdb configuration overlay; an in-story save-slot and Load-menu roundtrip are not yet claimed
- Primary audio format breadth, suspend/resume behavior, and long-run streaming stability are not yet accepted
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
