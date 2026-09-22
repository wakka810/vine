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
| Next development goal | Resume manual Chrome play feedback, remaining representative flows and long-run acceptance when requested; the optimization pass and final Start-transition follow-up are closed at the user's request, with transition latency unresolved |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
| `execution` | `partial` | PE32 loading, JIT-enabled x86 execution and the performance island share one Rust-owned GuestMemory arena with checked mapping and write receipts; process/thread scheduling, callbacks and generic forward capture remain active, while normal/debug v86 retain the separate copying mirror and differential oracle |
| `window-input` | `partial` | Logical window, message queue, timers, focus/cursor state, keyboard scan, primary down/up dispatch, and native double-click generation are active; the validated save/load continuation accepted input through the first route ending and exact title return |
| `graphics` | `partial` | GDI/GDI+ reached paths and bounded D3D9 fixed-function scenes publish through WebGL2; the ending run produced 135 Presents and returned to the exact title color and dark-region signature with WebGL error zero |
| `source-storage` | `partial` | Immutable source reads and a title-local SQLite overlay remain separated; the latest 95,058-byte Q.Save stayed hash-identical through the ending run with the lease inactive afterward, while the executable and source assets remained unchanged |
| `audio` | `partial` | Reached DirectSound streaming buffers support concurrent non-overlapping partial and wrapping Lock/Unlock, captured notification waits, cursor/status/control, and Play/Stop through generation-safe Chrome Web Audio transport; five non-zero playbacks occurred in the ending run and sustained audio spans the accepted route |

### Important Non-Claims

- The optimization pass is closed at the user's request, not because sustained 60 FPS-class responsiveness has been achieved. CPU-heavy transitions, long slices and input-to-semantic-completion latency remain limitations; further optimization is not an active queue
- The final Start-transition follow-up retained v160 unchanged: two fresh Chrome controls measured 5.18/5.24s from title Start ingress to the first non-black sampled texture upload after blackout, with 1.36-1.45s and 2.00-2.10s early draw gaps and a later 5.36-5.43s gap. Sparse texture sampling is a rendering marker, not full scene-ready or story-progress acceptance. Input reached the guest promptly; the early 2s gap was dominated by execution-island/JIT and x87 work. A later 5.36s gap included about 3.17s in executeBudget and 1.94s idle. Individual measured source fetches were tens of milliseconds, not a single multi-second fetch
- Three bounded Start-transition candidates were rejected: delivering already-expired logical timers through MessageChannel removed clamped zero-delay waits but increased guest busy-loop work and worsened the first populated-upload marker to 6.65s; inlining binary53 alone measured 5.39s, and also inlining its Add/Sub/Mul callers measured 5.60s. The unchanged controls achieved 588-602M CPU-time guest instructions/sec versus 544M/592M/577M for those candidates; workload-dependent wall throughput and draw timing did not establish a speedup. Candidate maximum slices were 89/88/108ms versus 92-96ms in the controls. All candidate code and artifact pins were removed. Normal/debug artifacts, timing semantics and the shipped performance engine remain unchanged; no additional performance improvement is claimed
- The final restored-engine manual check initially observed trusted mouse Start and Enter input, 2,506 draw notifications, nonzero audio, one underrun and next-draw latencies of about 14/14/1,902ms. The session then remained open across an approximately nine-hour interruption; on return, drawing had stopped and repeated audio underruns were present, and cancellation did not produce a terminal report. Its cause is undetermined and this is not successful long-run or graceful-stop acceptance. The test tab and server were closed; the accepted profile and copied save remained unchanged and the profile lease was inactive
- The final performance island reuses the checked SIMD-window compiler for bounded scalar DWORD windows with one unchanged base and one resident ordinary-RAM page. Mapping, privilege, write protection, dirty tracking, aliasing and code invalidation retain their guards and original-JIT fallback. Normal/debug CPU artifacts, instruction budgets, adaptive slice policy and JIT hotness thresholds are unchanged
- In the final same-session 20-input Chrome comparison, CPU-only throughput was 376.9M versus 364.7M guest instructions/sec (about 3.4% higher), but wall time was 142.24s versus 141.87s. Maximum slices were 87.39ms versus 82.73ms; Worker/API yields were 23,615/159 versus 21,620/158. Tail draw p50/p95 were 496/1,108ms versus 500/962ms, and audio underruns were zero versus one. An earlier integrated run measured 381.1M versus an adjacent 384.7M control, so the gain is variable. Timer-driven work differs; none of these numbers establishes an overall FPS or transition speedup
- Unchanged LastError updates now avoid cloning the selected thread's TLS/FLS and callback state; changed values retain the original validated commit. Sampled API completion time was about 0.93-0.95s versus 2.15s in an earlier control, not a matched whole-game speedup. The no-reallocation regression fails with the old setter; thread isolation and API completion regressions pass. Scalar-window differential checks cover privilege, page crossings, missing/read-only/remapped pages, aliases, dirty rearm, code invalidation, flags and exact budget boundaries; exact fault-retirement counts remain unclaimed
- The final 171s trusted browser-input check produced 2,433 draw notifications, nonzero audio, one underrun, zero pending host calls/callbacks and complete disconnection/closure of all six audio nodes. Left/right clicks reached the next draw in 9-12ms; Escape and a later Enter took 3,314ms and 229ms. An earlier Enter on a static screen overlapped the following click and cannot establish key latency. Maximum slice was 74.7ms; browser rAF p50/p95 were 16.66/16.88ms, not guest FPS. Next-draw timing is not semantic-completion latency. Source/accepted-save hashes and inactive leases were verified
- Rust GuestMemory remains authoritative; the performance island shares its checked frames without CPU/guest page copies, while normal/debug retain the copying mirror and differential oracle. Existing scoped JIT invalidation, relaxed-tail JIT retention, counted loops, checked SIMD windows, direct x87 fast paths and native page materialization remain in place. Source files are immutable and writable state is isolated in title-local profiles
- Only the first default route ending and title return are accepted. Alternate choices/routes, all-scenario completion, new save/load acceptance and long-run stability remain unclaimed. The accepted .work-only checkpoint retains one 95,058-byte Q.Save with SHA-256 0cd7c853e9c4afd032e9f1235c2a9bb065d6b5a15cb35fc57c87921a991306ec; final copied-save runs left it unchanged and completed with inactive leases and no pending host work
- The measured route live set is bounded to 480 MiB; later-route peak memory and long-run reclamation remain unaccepted. Primary audio spans the representative flow, but format breadth, suspend/resume behavior and long-run streaming stability remain unaccepted
- Historical per-API capability profiles and past optimization experiments are frozen evidence, not the active development queue

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
