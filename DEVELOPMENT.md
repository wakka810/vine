# Development

## Local loop

1. Check `git status --short --branch` and preserve unrelated changes and immutable game files.
2. `npm run dev:target` builds and serves the **generic** Chrome directory picker, executable browser, and runtime. Select a source directory and PE32 executable; it does not auto-run a title-specific checkpoint.
3. On the first unsupported call, fault, or visible milestone, inspect how the guest uses the return value, LastError/HRESULT, outputs, identity, and lifetime. Read the subsystem implementation, production callers, Wasm/browser consumers, and relevant tests **before** editing.
4. Implement the smallest reusable Windows behavior in its Rust owner and rerun. Do not add title hashes, EIP/occurrence branches, fake successes, or a new policy/schema for one reached variant.
5. Use local focused tests for tricky behavior and trust boundaries. Cross-owner synchronous mutations preflight all fallible work before updating any state; keep pending identity checks when control leaves Rust for Chrome, CPU execution, or a guest callback.

## Checks

- `npm run check:fast`: Rust core unit tests, formatting, TypeScript, JS syntax, and small browser-adapter tests.
- `npm run check:rust`: workspace tests and strict Clippy.
- `npm run check:toolchain` and `npm run prepare:engine`: prepare the fixture toolchain and build the pinned Vine-v86 fork in `.work/vine-v86`. Local changes in that CPU checkout must be preserved before preparing another revision.
- `npm run prepare:media`: build the pinned MPEG-1/MP2 Wasm decoder from verified source archives with Emscripten 4.0.23. Generated codec files and source archives stay in `.work/libav-mpeg-build`; the browser build copies the codec and its license into `libav-media/`.
- `npm run check:media`: test the prepared decoder with synthetic MPEG-1 ES/PS and MP2 samples, including pixels, PCM, timestamps, duration, EOF and failed bounded reads.
- `npm run check:runtime:browser-worker:prepared`: release Wasm/browser build plus Chrome PE32 fixture smoke checks using prepared dependencies. `npm run check:runtime:browser-worker` prepares them itself.
- `npm run check:milestone`: broader Rust and Chrome integration at a meaningful user-visible or authority milestone.
- `git diff --check`: catch whitespace errors. Check the actual diff, untracked paths, and source-file immutability before any allowed commit.

Fixture pixels, actual Web Audio PCM/Stop, audio notification waits, browser OPFS primitives, and PE32 save/close/fresh-worker/load show their *own* behavior; none proves full-title play, in-story save/load, or long-run stability. Private title scenarios belong outside this repository. Keep raw traces, generated reports, save data, selected source files, and build output under `.work/` or outside Git. Never modify a user's EXE/DLL/assets to advance a check.
