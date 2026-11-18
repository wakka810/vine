# Status

Vine is a work-in-progress Chrome-hosted PE32/i386 Windows userspace runtime.
General game compatibility, full-title stability and sustained 60 FPS are unqualified.

## Product path

- Select a read-only source directory, browse it, select a PE32 executable and launch. Executables, DLLs and assets remain unchanged. Rust owns Windows state and guest memory; Chrome supplies rendering, audio, input and durable game-local OPFS storage.
- Fast is the default. It links the separate Vine-v86 fork and Rust Runtime into one Wasm module with one address-stable guest arena. CPU stores update Rust-owned bytes directly, and ordinary synchronous Win32 calls resume inside Rust. Mapping, callbacks, scheduling and browser/OPFS work retain checked handoffs.
- Strict retains the separate CPU/Runtime execution path. Execution architecture and x87 numerical mode are independent settings; a frozen comparison CPU can be selected for local measurements. Both execution paths use one CPU Worker and the Rust guest-thread scheduler.
- The CPU retains exact public instruction budgets, tail-call chaining with GPR passing, bounded short-call/return specialization, same-page RAM-check sharing, direct MMX SIMD and binary64 x87 value runs. The strict x87 path remains available. Approximation does not relax guest memory permissions, ownership or fault handling.
- Reached Windows behavior includes PE relocation, static and dynamic DLL loading, API/COM callbacks, threads/events/timers, input, GDI/GDI+ and Direct3D 8/9 presentation, DirectSound/WinMM audio, bounded MPEG decoding and durable save/restart/load fixtures. Unsupported reached behavior stops explicitly.

The CPU revision is pinned in `tools/prepare-v86-adapter.mjs`; a normal checkout
builds both execution paths without private artifacts. See [DEVELOPMENT.md](DEVELOPMENT.md)
for build commands and [ARCHITECTURE.md](ARCHITECTURE.md) for ownership boundaries.

## Verification and limits

Rust workspace tests and Clippy, CPU budget/fault/SMC/growth/x87/MMX regressions,
shared-memory authority checks and Chrome product suites have passed. Chrome
fixtures exercise graphics, actual PCM, input, callbacks, thread/timer behavior,
source immutability and OPFS save/close/fresh-runtime/load in Fast and Strict.

Fixed captured VM, pixel and audio work improved during CPU development, but
those comparisons exclude parts of the full Runtime/browser path. Actual-title
observations include unattended startup to title screens, opening scenes,
text progression and a sampled save/restart/load; they do not qualify full-title
play, save compatibility, animation FPS or long-run stability.
Generated guest code remains a major measured cost. Private comparisons, source
snapshots and raw traces are retained outside Git.

Fast currently accepts guest addresses below 2 GiB and reserves a little over
2 GiB of virtual Wasm space while retaining a physical-page quota. Dynamic DLL
loading supports selected-directory images whose guest dependencies are already
loaded, with stable import endpoints and process-attach callbacks. Dynamic releases
retain shared references, call process detach at the final reference, and remove
the image and its TLS storage. PE TLS templates without callbacks initialize
separate storage for existing and new guest threads.
Recursive dependency discovery, nested loads, cascading unload, nonempty TLS callbacks,
cyclic imports, general IME/text entry and broad Windows compatibility remain incomplete.

## Preserved independent backend

`crates/vine-recompiler` and `crates/vine-x87` retain the experimental independent
PE32 AOT/JIT backend, its Runtime/browser integration and semantic tests. It is
an explicit alternative to the product v86 path. Known image roots can be
prepared before execution; unknown or invalidated targets compile on demand.
There is no automatic cross-backend fallback. See [INDEPENDENT_BACKEND.md](INDEPENDENT_BACKEND.md).

Its Chrome semantic and Runtime fixture checks pass, including bootstrap and
OPFS behavior. Practical title throughput, approximate audio quality and full-title
stability remain unqualified. Historical measurements and archived prototype
snapshots stay outside the working source tree.

Out of scope: x64/PE32+, Win64, D3D11/DXGI, XAudio2, networking, multi-process
frameworks, generic desktop emulation and a game library.
