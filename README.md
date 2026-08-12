# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-save-browser-restart-load`: The primary title commits save or configuration state to the writable overlay and restores it after browser and local service restart without changing source files |
| Last completed boundary | `primary-visible-action-response-or-readable-frame`: The primary title rendered a readable Start/Load menu, accepted a real pointer press through the guest input path, and visibly highlighted Start while primary Web Audio continued |
| Current forward stop | `semantic-leaf-implemented-pending-forward-confirmation` / `IDirect3DDevice9::UpdateTexture`: The last bounded report reached a valid (232,556)-(346,619) partial dirty region at 9,790,835,734 instructions; the v33 policy now accepts owner-validated bounded regions, but two post-fix cold runs reached the host time bound before producing a later endpoint |
| Next development goal | Add a commit-scoped .work forward resume cache for the repeatedly measured long cold prefix, confirm the bounded UpdateTexture path, then activate Start and follow the first scenario persistence behavior into save/restart/load |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, x86 execution mirror, checked GuestMemory synchronization, process/thread scheduling, callbacks, and reached Win32/COM dispatch |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, pointer down/up dispatch, and a guest-observed primary pointer press that visibly highlights Start |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes published through WebGL2; readable menu rendering and owner-validated bounded UpdateTexture dirty regions are supported |
| `source-storage` | `partial` | Immutable source reads and a separate writable overlay are active; one directory create and commit are reached, but their save/configuration role and restart/load are not yet accepted |
| `audio` | `partial` | Reached DirectSound streaming buffers, partial and wrapping Lock/Unlock, notifications, status/cursor control, Play/Stop with generation-safe browser transport, and non-zero primary PCM through Chrome Web Audio |

### Important Non-Claims

- Start visibly responds to pointer input, but activating it and advancing a representative scenario are not yet accepted
- The reached bounded UpdateTexture region is implemented, but a post-fix private run has not yet produced the following endpoint because the cold prefix exceeded the host time bound
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
