# Vine

Vine is a browser-hosted Windows userspace compatibility runtime for unmodified PE32 VN／ADV executables and their original DLLs and assets. The active target is the fingerprinted サクラノ詩 `BGI.exe` running in Chrome through readable graphics, input, audio, save／load, full scenarios, and long-run play.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json). Edit the compact state and run `npm run docs:generate`.

| Area | Current state |
|---|---|
| Active target | Sakura no Uta `BGI.exe` (PE32/i386) in Chrome |
| Active milestone | `primary-full-play-long-run`: All scenarios complete through endings with readable rendering, responsive choices, sustained audio, save/load resume, and measured long-run stability without unsupported stops |
| Last completed boundary | `primary-first-route-ending`: Release Chrome resumed the validated save and returned to the exact title target after 174.85 seconds with 135 Presents, 10 completed pointer actions, and 5 audio playbacks; no unsupported endpoint, WebGL error, pending host work, lease leak, or source mutation was observed, and the persisted 95,058-byte save remained hash-identical |
| Current forward stop | `primary-first-route-ending-title-return` / `none`: The release forward run returned to the exact title target and stopped with outcome checkpoint-complete after 135 Presents; the final frame independently matched the bounded title color and dark-region probes, while source immutability, zero pending host work, an inactive profile lease, and unchanged save bytes were preserved |
| Next development goal | Build on the verified bounded counted-loop x86 compiler and remove remaining multi-second input/transition latency in actual Chrome play while retaining v86 fallback, then resume additional routes and long-run acceptance |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution and the performance island share one Rust-owned GuestMemory arena with checked mapping and write receipts; process/thread scheduling, callbacks and generic forward capture remain active, while normal/debug v86 retain the separate copying mirror and differential oracle |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, primary down/up dispatch, and native double-click generation are active; the validated save/load continuation accepted input through the first route ending and exact title return |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the ending run produced 135 Presents and returned to the exact title color and dark-region signature with WebGL error zero |
| `source-storage` | `partial` | Immutable source reads and a title-local SQLite overlay remain separated; the latest 95,058-byte Q.Save stayed hash-identical through the ending run with the lease inactive afterward, while the executable and source assets remained unchanged |
| `audio` | `partial` | Reached DirectSound streaming buffers support concurrent non-overlapping partial and wrapping Lock/Unlock, captured notification waits, cursor/status/control, and Play/Stop through generation-safe Chrome Web Audio transport; five non-zero playbacks occurred in the ending run and sustained audio spans the accepted route |

### Important Non-Claims

- Only the first default route ending is accepted; alternate choices, alternate routes, all-scenario completion, and long-run acceptance are not yet claimed
- The accepted checkpoint is .work-only and retains one 95,058-byte Q.Save with SHA-256 0cd7c853e9c4afd032e9f1235c2a9bb065d6b5a15cb35fc57c87921a991306ec; the ending run hydrated it unchanged and did not overwrite it
- The measured route live set is bounded to 480 MiB; later-route peak memory and long-run reclamation are not yet accepted
- Primary audio spans the representative flow, but format breadth, suspend/resume behavior, and long-run streaming stability are not yet accepted
- Performance mode retains JIT across data mapping changes, selectively rearms dirty-page TLB entries, and delivers independent live host completions without historical replay serialization (two measured transitions improved from 4.42s/3.33s to 1.60s/0.54s). Fused x87 memory operations, exact binary32 widening, guarded native 53-bit arithmetic and batched memory-journal allocation further reduced a CPU-heavy transition from 3.76-4.11s to 2.70-2.83s in Chrome (two runs per variant). Startup through 12 inputs improved from 52.17-53.30s to 50.10-50.26s at 279-287M guest instructions/sec, with zero observed audio underruns but 83ms maximum slices. Independent normal-engine differential checks cover rounding, precision, special values and exponent boundaries; memory rollback retains original bytes and ordering. The normal/debug v86 artifact is unchanged. Whole-run Presents/sec is not steady-play FPS, and 60 FPS-class response remains unaccepted
- The direct-memory performance island removes CPU/GuestMemory page copies while preserving Rust frame ownership, zero-initialization, checked protection and transactional rollback. Chrome 152.0.7977.83 startup/12-input runs used 539MB of Wasm memory versus 660MB with the previous MMX island; measured TS writeback synchronization fell from 1.11-1.18s to about 64ms, but wall time remained 50.4-51.7s and maximum slices reached 70-116ms across startup/load checks. Normal/debug keep their unchanged artifact and diagnostic heap fills. These results establish memory/copy reduction, not 60 FPS or a broad speedup
- The direct-memory isolated load check hydrated the accepted 95,058-byte save, consumed 20 pointer actions and preserved its hash, source files and inactive lease with no pending work, but its black final frame and sustained audio underruns do not establish playable load continuation. A subsequent cold-start manual Chrome check correctly activated play, accepted trusted mouse and Enter input, continued rendering and produced non-zero measured WebAudio output. Lightweight responses were 11-20ms in one run, but another run measured 68ms/303ms mouse/Enter response and later streaming starvation; sustained responsiveness is not accepted. A separate automated-frame-probe fix avoids retaining a black transition sample forever; manual performance mode omits that probe
- The v47 Chrome run fixes an audio lifetime race at its shared owners: DirectSound Stop persists a consumable Rust Event signal, WaitForSingleObject supports zero/infinite Thread waits, and the browser bridge acknowledges committed Rust wakeups. The old audio threads now exit before handle/buffer reuse. A matched 30-second profiled playback window supplied 33.3s of audio with zero underruns, versus 11 updates and 126 underruns in the preceding MMX-only run. A separate 162s trusted mouse/Enter manual run continued drawing and multiple nonzero WebAudio spans with zero underruns, but next-canvas-frame delays of 63ms, 2.07s and 4.52s still reject sustained responsiveness. Generic MMX arithmetic lowering reduced a 40.9M-instruction synthetic median from 288ms to 251ms; whole-game speedup is not established (87.49s profiled run, 251M instructions/sec, maximum slice 144ms). Normal/debug CPU artifacts remain unchanged; Windows thread/event fixes are shared
- The bounded counted-loop compiler reuses v86 decoding, cache invalidation and precise fallback but lowers proven integer/MMX loops without per-instruction memory-state round trips. A 65,536-pixel kernel in ordinary RAM improved from 2.68-2.72ms to 0.18-0.20ms with matching output and instruction count. The earlier test overlapping VGA-reserved memory is not a RAM benchmark. Profiled Chrome playback tail windows improved median draw intervals from 59-69ms to 33-44ms across candidates; these timer-driven windows are not identical frame workloads. The shipped counted-loop candidate's 147s trusted mouse/Escape/Enter run produced 2,454 Presents and nonzero audio with no unsupported endpoint or pending host calls. Next changed-frame delays were 23-39ms for two left clicks but 2.3-3.9s for right-click/keyboard, with a 77ms maximum slice. One profiled integer-extension run observed one audio underrun; sustained responsiveness and 60 FPS remain unaccepted. Normal/debug artifact hashes are unchanged
- Direct mapping reuse removes redundant protection-only page recreation; profiled tail mapping cost fell from about 1.10s to 8ms. Address-ordered Rust free-frame reuse reduces fragmented render copies: two Chrome tail windows spent 39-61ms in readGuestBytes versus 769-790ms with stack-order reuse. Overall tail draw medians overlap (29-40ms versus 37-40ms); a broad FPS gain is not established. Both ordered-frame runs observed zero audio underruns. A 180s trusted IAB run produced 2,293 Presents, nonzero audio and zero pre-stop underruns without unsupported or pending host work, but right-click/Escape/Enter next-frame waits remained 1.56s/2.07s/4.59s and maximum slice 88ms. Post-cancel audio nodes kept running; shared renderer session cleanup now disconnects nodes and rejects pending old-session plays. Sustained 60 FPS-class response, matched transition speedup and new save/load acceptance remain unclaimed
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
