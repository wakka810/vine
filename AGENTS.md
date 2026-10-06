# Vine agent guide

Vine is a Chrome-hosted PE32/i386 Windows userspace runtime for **unmodified** executables, DLLs, and assets. The product path is directory selection → browse inside that directory → select a PE32 executable → launch. There is no title detector, BGI/Sakura runtime identity, checkpoint queue, or per-title policy. See [ARCHITECTURE.md](ARCHITECTURE.md), [DEVELOPMENT.md](DEVELOPMENT.md), and [STATUS.md](STATUS.md). Git history archives the old evidence and design documents.

## Priorities

1. The user's current request.
2. Immutable source, durable saves, and trust-boundary/authority invariants.
3. User-visible launch, rendering, input, audio, save/restart/load, and representative flow.
4. Correctness of reached behavior and focused regression.
5. Small documentation of current facts, not checkpoint or report growth.

Do not measure progress by API counts, call counts, instruction counts, profile counts, or report sizes. Unsupported reached behavior is an implementation task, not by itself a blocker. Do not silently succeed on unsupported input, malformed buffers, or resource exhaustion.

## Architecture and scope

Rust owns guest bytes/mappings/protection, PE/module loading, Windows ABI and LastError, process/thread/scheduler, objects/handles, COM identity/refcounts, file/path/cursor, message/graphics/audio/input state. TypeScript owns Chrome resources only: UI/Canvas/WebGL2, Web Audio, source-directory capability/permissions, OPFS bytes, and lifecycle. It must not own a second copy of Win32 handle, pointer, COM, thread, file cursor, or message state. `GuestMemory` is the authority; v86 paging is an execution mirror. Verify guest address ranges, mapping/owner/lifetime, checked CPU writeback, and browser↔Wasm buffers before mutation.

The aggregate is **one Rust `Runtime` that owns `GuestMemory`**. API/COM/callback/deferred entry points use that owned memory; `ExecutionSession` has been deleted. The CPU is **one pinned separate Vine-v86 fork**. CPU/JIT changes and CPU benchmarks belong there; Vine keeps the execution/memory adapter and generic integration checks.

Fixed Windows environment: NT 10.0/build 19045-ish, Japanese/ACP 932, PE32/i386, reached Win32/COM/GDI/GDI+/D3D9/DirectInput/DirectSound/WinMM. Chrome, WebGL2, and Web Audio only. x64/PE32+, Win64, D3D11/DXGI, XAudio2, Unity, networking, multi-process framework, generic desktop UI, and a game library are deferred. Do not add APIs from import lists alone.

Source `FileSystemDirectoryHandle` is read-only. Source files, including executable and DLL, must never be modified. Rust owns Windows-932 path/sandbox semantics; browser source reads are bounded and verified. Guest writable paths live in game-local direct OPFS, not SQLite, profiles, or allowlists. Do not report a guest open or write successful before its necessary durable browser completion. A failed write must not change authoritative bytes, cursor, output, or handle publication.

## Working loop

1. Check `git status --short --branch`, HEAD, and unrelated dirty/untracked files; preserve them. Do not stage or commit without permission.
2. Start from `npm run dev:target` for the generic release Chrome path, or a smaller real fixture if reproducing a subsystem fault. Capture the first unsupported/fault/milestone; inspect how the caller uses result, LastError/HRESULT, outputs, identity, lifetime, and callbacks.
3. Before editing a subsystem, read its implementation, production callers, Wasm/browser consumers, and relevant tests. Implement the smallest reusable Windows behavior, rerun, and continue past a single leaf when the next call belongs to the same subsystem.
4. For ordinary synchronous work, validate all fallible conditions before mutation; do not build plan/preflight/stale-generation frameworks solely to prove adjacent Rust calls are atomic. Retain pending state and live identity checks for actual browser/OPFS, CPU, callback, and deferred handoffs. Cross-owner operations must not partially publish on failure.
5. Use focused tests for semantics and safety; `npm run check:fast` normally, `npm run check:rust` for workspace/Clippy, and `npm run check:runtime:browser-worker:prepared` for prepared Chrome PE32 integration. `npm run check:milestone` is for meaningful user-visible/authority milestones. `git diff --check` and review the final diff/status.

Keep reports, generated artifacts, raw traces, saves, private executables/DLLs/assets, secrets, and `.work/` out of Git. No destructive Git/history rewrite/force push or edits to unrelated user files. Full-title play/save/long-run scenarios and title-specific evidence live outside Vine; distinguish Chrome fixtures and OPFS primitives from actual full-title evidence. Keep README/STATUS factual. Docs/tools/tests preserve semantics and trust boundaries, not serialization shapes, policy fixtures, native-oracle scaffolding, or old checkpoint expectations.
