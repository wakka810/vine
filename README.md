# Vine

Browser-native Windows userspace compatibility runtime for Windows VN/ADV
executables. The active target is full play of `サクラノ詩/BGI.exe` on
PE32/i386 in Chrome.

<!-- BEGIN GENERATED CURRENT STATUS -->
## Current Status

This section is generated from [`docs/current-checkpoint.json`](docs/current-checkpoint.json).
Edit the manifest and run `npm run docs:generate`; do not edit this block by hand.

| Area | Current state |
|---|---|
| Historical baseline | `bgi-seventeenth-win32-call`: 17 calls, 238 instructions |
| Active scenario | `first-present-readable-screen` |
| Runtime diagnostics | `gdi32-create-dib-section-448x336-post-create-font-a-224x112`: 261,035 calls across 136 endpoint contracts / 134 APIs, memory generation 296,729 |
| Runtime architecture | `S13`: `runtime/kernel.rs::RuntimeKernel`; `RuntimeState` remains a compatibility alias |
| Browser | `private-bgi-generic-session` passed; BGI session `semantic-equivalence-passed`; benchmark `threshold-passed` (17 sessions) |
| Shared clock | `scheduled-shared-logical-deadline-v1`: cumulative retired guest instructions at 2,000,000/s with 1 ms resolution; API reads do not advance it; Node primary recorded 100,534 samples / 39 TIMERPROC callbacks; Chrome `vine.browser-runtime-worker.v106` boundary passed |
| Engine evidence | v86 `go` from `fixtures/benchmarks/v86-browser-v1.json` |
| Next structured stop | `gdi32-create-dib-section-448x336-post-create-font-a-224x112` completed 1 recurring waits by instruction 342875151; verification stopped at `GDI32.dll!GetOutlineTextMetricsA` on instruction 342875244 (`gdi32-create-dib-section-448x336-post-create-font-a-224x112-checkpoint-complete`) |

The active semantic report is 2,450,520 bytes with SHA-256
`d177b3e803bc9dbb4e7d1e99e6d6b582fa3529b71fd1bb91d1914dca41402c2d`. The hash identifies the artifact; semantic checks are the gate.

### Scenario Milestones

| ID | Milestone | Status | Evidence boundary |
|---|---|---|---|
| `boot-and-process-initialization` | Boot and process initialization | `in-progress` | Primary executable reaches the current structured stop after creating and resuming one guest thread; title entry and child workload completion are not complete |
| `logical-window-and-message-flow` | Logical window and message flow | `in-progress` | Primary logical window creation, activation, paint, owner-thread timers, and the normalized primary-button flow pass. One Rust-owned scheduled clock advances from cumulative retired guest instructions across all API/COM boundaries; Node and private Chrome agree on 261,035 semantic calls, 100,534 samples, 39 TIMERPROC callbacks, and the deterministic pointer down/up receipts. The reached SetWindowTextA synchronously commits サクラノ詩, Runtime VFS search enumerates all 45 data0????.arc entries, both missing-file DeleteFileA calls complete, the third ordinary private-heap segment succeeds, and the reached CreateFontA(224,112,...) plus exact 448x336 DIB pipeline complete before the next GetOutlineTextMetricsA variant. Replacement, KillTimer, focus-transition messages, cross-thread focus, capture transitions, move/wheel, filtered and cross-thread retrieval, and general message-flow breadth remain partial. |
| `immutable-source-asset-access` | Immutable source asset access | `in-progress` | A 48-object immutable source snapshot covers the three boot archives and all 45 reached data0????.arc archives without copying the approximately 5.9 GiB corpus into bootstrap JSON. Primary system.arc completes bounded member reads and the archive search matches the native lexical file/size sequence while BGI.gdb stays writable profile state; archive content breadth beyond reached bounded ranges is not claimed |
| `first-present-readable-screen` | First present and readable screen | `in-progress` | Controlled Canvas pixel, public Chrome Worker, and the private Node-equivalent scheduled-clock replay pass; the primary D3D9 logical device has completed two bounded scene/draw/present cycles and synchronous Worker WebGL2 materialization. Under the deterministic scheduled clock, 100,534 samples, 39 TIMERPROC callbacks, deterministic pointer down/up consumption, SetWindowTextA／WM_SETTEXT, the complete 45-entry archive search, both missing-file DeleteFileA calls, the third ordinary private-heap segment, the reached CreateFontA(224,112,...) logical object, exact 448x336 DIB creation, GetObjectA, and bitmap／font selection all finish, but Present remains at two before the next GetOutlineTextMetricsA font variant. Clock freeze, missing timer delivery, browser wall-clock pacing, input scheduling, the title API, archive enumeration, missing-file deletion, the old two-segment heap capacity, logical font creation, and DIB creation are therefore not the proximate cause of the current presentation stall; readable title-scene content, visible input response, resize, device loss/recovery, general stack overflow/SEH, and long-run stability remain unproven. |
| `user-input-consumption` | User input consumption | `in-progress` | The verified primary-button down/up path commits normalized input through Rust-owned cursor, window, message queue, wait, guest-memory, and callback-frame authorities, consumes VK_LBUTTON, and unwinds the registered WndProc at callback depth zero. Node and private Chrome commit down after scheduled main-timer cycle 253 and up after GetAsyncKeyState sequence 222,096 with identical receipts, then continue through 39 TIMERPROC callbacks, SetWindowTextA／WM_SETTEXT, the complete archive search, both missing-file DeleteFileA calls, the third ordinary private-heap segment, the reached CreateFontA(224,112,...) logical object, exact 448x336 DIB creation, GetObjectA, and bitmap／font selection without producing a third Present. Visible gameplay response, focus-transition messages, pointer capture/move/wheel, DirectInput keyboard/mouse devices, and general input remain unproven. |
| `audible-output` | Audible output | `fixture-only` | Controlled 880 Hz browser pulse passes; primary DirectSound8 reaches an 88,200-byte zero-filled secondary sample backing through Lock/Unlock and accepts looping Play, while browser audio transport, audible primary output, and WinMM audio are not reached |
| `save-browser-restart-load` | Save, browser restart, and load | `not-reached` | Rust-owned writable profile and SQLite generation/idempotency/rollback foundation pass; controlled guest browser commit, restart, and load are not demonstrated |
| `representative-flow` | Representative scenario flow | `not-reached` | No primary representative flow is claimed |
| `full-play-long-run` | Full play and long run | `not-reached` | No route, ending, or long-run primary acceptance is claimed |

### Supported API Contracts

- `KERNEL32.dll!GetSystemTimeAsFileTime`
- `KERNEL32.dll!GetCurrentProcessId`
- `KERNEL32.dll!GetCurrentThreadId`
- `KERNEL32.dll!GetTickCount`
- `KERNEL32.dll!QueryPerformanceCounter`
- `KERNEL32.dll!QueryPerformanceFrequency`
- `KERNEL32.dll!GetStartupInfoW`
- `KERNEL32.dll!HeapSetInformation`
- `KERNEL32.dll!HeapCreate`
- `KERNEL32.dll!GetModuleHandleW`
- `KERNEL32.dll!GetProcAddress`
- `KERNEL32.dll!TlsAlloc`
- `KERNEL32.dll!TlsSetValue`
- `KERNEL32.dll!EncodePointer`
- `KERNEL32.dll!DecodePointer`
- `KERNEL32.dll!InitializeCriticalSectionAndSpinCount`
- `KERNEL32.dll!EnterCriticalSection`
- `KERNEL32.dll!LeaveCriticalSection`
- `KERNEL32.dll!FlsAlloc`
- `KERNEL32.dll!FlsSetValue`
- `KERNEL32.dll!HeapAlloc`
- `KERNEL32.dll!HeapFree`
- `KERNEL32.dll!InterlockedIncrement`
- `KERNEL32.dll!GetStdHandle`
- `KERNEL32.dll!GetFileType`
- `KERNEL32.dll!SetHandleCount`
- `KERNEL32.dll!GetCommandLineA`
- `KERNEL32.dll!GetEnvironmentStringsW`
- `KERNEL32.dll!FreeEnvironmentStringsW`
- `KERNEL32.dll!WideCharToMultiByte`
- `KERNEL32.dll!GetLastError`
- `KERNEL32.dll!TlsGetValue`
- `KERNEL32.dll!FlsGetValue`
- `KERNEL32.dll!SetLastError`
- `KERNEL32.dll!GetACP`
- `KERNEL32.dll!GetCPInfo`
- `KERNEL32.dll!MultiByteToWideChar`
- `KERNEL32.dll!GetStringTypeW`
- `KERNEL32.dll!LCMapStringW`
- `KERNEL32.dll!InterlockedDecrement`
- `KERNEL32.dll!GetModuleFileNameA`
- `KERNEL32.dll!IsProcessorFeaturePresent`
- `KERNEL32.dll!SetUnhandledExceptionFilter`
- `KERNEL32.dll!HeapSize`
- `KERNEL32.dll!InitializeCriticalSection`
- `KERNEL32.dll!SetCurrentDirectoryA`
- `KERNEL32.dll!CreateMutexA`
- `KERNEL32.dll!GetCurrentThread`
- `KERNEL32.dll!SetThreadAffinityMask`
- `KERNEL32.dll!GetSystemInfo`
- `KERNEL32.dll!VirtualAlloc`
- `USER32.dll!EnumDisplayMonitors`
- `OLE32.dll!CoInitialize`
- `KERNEL32.dll!GetSystemDefaultLangID`
- `KERNEL32.dll!GetDriveTypeA`
- `USER32.dll!SwapMouseButton`
- `WINMM.dll!timeGetDevCaps`
- `WINMM.dll!timeBeginPeriod`
- `WINMM.dll!timeEndPeriod`
- `KERNEL32.dll!GetVersionExA`
- `KERNEL32.dll!CreateWaitableTimerA`
- `USER32.dll!LoadIconA`
- `USER32.dll!LoadCursorA`
- `USER32.dll!RegisterClassExA`
- `USER32.dll!GetSystemMetrics`
- `USER32.dll!CreateWindowExA`
- `USER32.dll!DefWindowProcA`
- `USER32.dll!GetSystemMenu`
- `USER32.dll!EnableMenuItem`
- `KERNEL32.dll!CreateFileA`
- `KERNEL32.dll!GetFileSize`
- `KERNEL32.dll!ReadFile`
- `KERNEL32.dll!SetFilePointer`
- `KERNEL32.dll!CloseHandle`
- `USER32.dll!LoadImageA`
- `USER32.dll!SendMessageA`
- `USER32.dll!SetWindowPos`
- `d3d9.dll!Direct3DCreate9`
- `KERNEL32.dll!LoadLibraryA`
- `USER32.dll!GetClientRect`
- `USER32.dll!GetWindowRect`
- `USER32.dll!GetCursorPos`
- `USER32.dll!GetDC`
- `GDI32.dll!GetStockObject`
- `GDI32.dll!EnumFontFamiliesExA`
- `GDI32.dll!CreateFontA`
- `GDI32.dll!CreateCompatibleDC`
- `GDI32.dll!CreateDIBSection`
- `GDI32.dll!GetObjectA`
- `GDI32.dll!GetOutlineTextMetricsA`
- `GDI32.dll!TextOutA`
- `gdiplus.dll!GdiplusStartup`
- `gdiplus.dll!GdiplusShutdown`
- `GDI32.dll!SelectObject`
- `GDI32.dll!DeleteObject`
- `GDI32.dll!DeleteDC`
- `USER32.dll!FillRect`
- `USER32.dll!ReleaseDC`
- `USER32.dll!SetWindowLongA`
- `USER32.dll!MonitorFromWindow`
- `USER32.dll!GetMonitorInfoA`
- `WINMM.dll!timeGetTime`
- `USER32.dll!GetForegroundWindow`
- `USER32.dll!GetAsyncKeyState`
- `DINPUT8.dll!DirectInput8Create`
- `DSOUND.dll!#11`
- `USER32.dll!SetTimer`
- `KERNEL32.dll!CreateThread`
- `KERNEL32.dll!ResumeThread`
- `KERNEL32.dll!SetThreadPriority`
- `IMM32.dll!ImmGetDefaultIMEWnd`
- `IMM32.dll!ImmAssociateContext`
- `KERNEL32.dll!SetWaitableTimer`
- `KERNEL32.dll!WaitForSingleObjectEx`
- `KERNEL32.dll!Sleep`
- `KERNEL32.dll!GlobalMemoryStatus`
- `KERNEL32.dll!GetFileAttributesA`
- `KERNEL32.dll!CreateDirectoryA`
- `KERNEL32.dll!FindFirstFileA`
- `KERNEL32.dll!FindNextFileA`
- `KERNEL32.dll!FindClose`
- `KERNEL32.dll!DeleteFileA`
- `SHELL32.dll!DragAcceptFiles`
- `USER32.dll!ShowWindow`
- `USER32.dll!UpdateWindow`
- `USER32.dll!BeginPaint`
- `USER32.dll!EndPaint`
- `USER32.dll!SetRect`
- `USER32.dll!PeekMessageA`
- `USER32.dll!IsIconic`
- `USER32.dll!TranslateMessage`
- `USER32.dll!DispatchMessageA`
- `USER32.dll!SetFocus`
- `USER32.dll!SetWindowTextA`

### Reproduce Current Checkpoint

```sh
npm run check:runtime:bgi
```
<!-- END GENERATED CURRENT STATUS -->

## Requirements

- Node.js 22 or newer
- Rust 1.97.0; the root toolchain file pins the production workspace
- LLVM/Clang with `wasm32` and `i686-pc-windows-msvc` targets
- `lld-link`, `wasm-ld`, and `llvm-readobj`
- MSYS2 `make` and `sh` on `PATH` for `npm run prepare:engine`
- npm dependencies from the committed lockfile

## Checks

```sh
npm ci
npm run check
```

`check:fast` validates docs and the runtime checkpoint manifest, rebuilds public
fixtures, runs Rust format/tests/clippy with warnings denied, checks JavaScript
syntax, runs the small v86 smoke,
and checks the Windows trace summarizer. `check:fixture-e2e` prepares the pinned
source adapter and runs the public entry, ABI callback/return, stack, TEB/PEB,
process-parameters, MemoryDelta, automatic binary CPU writeback, window, and
browser-presentation paths, including the guest CPU Runtime Worker vertical
slice.
`check:adapter` holds the broader adapter checks. `check:private-target` is the
required local commit-bound BGI gate; it fails when the source is unavailable
and writes a small evidence manifest only after Node／Chrome semantic agreement.
`check:private-corpus` retains the historical prefix checks and treats absence
as an explicit skip. GitHub Actions runs the two public check groups on Linux.
Linux bootstrap checks consume fixture-hash-bound recorded Windows oracle
evidence and label that provenance; they do not claim to execute PE32 natively.
Generated reports are written under `.work/` and are not tracked by Git.

`npm run prepare:engine` reproduces the source adapter and its single CPU state
round-trip contract check when the pinned v86 source build is needed.
`npm run check:runtime:entry` then verifies the production-memory reflection
and controlled entry-to-thunk boundary. `npm run check:runtime:cdecl` verifies
the fixed cdecl return and callback-service ingress without invoking the
callback. `npm run check:runtime:callback` verifies the separate Rust-owned
return sentinel, checked frame, guest callback, and exact result `43` stop.
`npm run check:runtime:outer-return` retires that frame, returns through the
outer cdecl thunk, and verifies the following result write, entry result, stack
balance, and structured unmapped-return fault.
`npm run check:runtime:entry-return` replaces that harness return value with a
second production-owned control sentinel and verifies exact successful entry
completion without accepting unrelated faults.
`npm run check:runtime:stack-bootstrap` reflects a Rust-owned initialized stack
page and verifies the same completion with no adapter-local stack allocation.
`npm run check:runtime:bootstrap-fs-first-stop` reruns the native FS oracle and
verifies the exact zero-FS-base source-v86 page fault before TEB／PEB setup.
`npm run check:runtime:bootstrap-fs` adds Rust-owned TEB／PEB pages, restores FS
base through the CPU state ABI, and verifies exact entry completion.
`npm run check:runtime:bootstrap-params-first-stop` compares the native
command-line oracle with the explicit missing-pointer guest result.
`npm run check:runtime:bootstrap-params` installs the measured Rust-owned
parameters and UTF-16 bytes and verifies exact guest completion.
`npm run check:runtime:memory-delta` applies one dirty data page to live v86
mapping and verifies the updated eight-byte value from guest code.
`npm run check:runtime:memory-writeback` runs two bounded guest slices, scans
v86 PTE Dirty bits, and atomically returns stack／heap／surface／RWX pages to the
Rust byte authority through binary control and payload buffers.
`npm run check:runtime:browser-worker` builds the zero-import Rust/Wasm core and
TypeScript application, then runs the controlled guest CPU, async completion,
nested callback, pixel／audio／storage, cancellation, and watchdog paths in a
clean Chrome profile. `npm run check:runtime:browser-worker:stress` adds the
30-minute repeated lifecycle measurement.
`npm run check:runtime:bgi` selects the current manifest profile. Historical
frozen checkpoints use `--checkpoint <id>`; historical runtime capabilities use
`--profile <capability>` (or `--profile baseline`). These are one generic runner
contract rather than permanent npm aliases. Detailed evidence remains in the
linked checkpoint documents.

See [docs/README.md](docs/README.md) for the current requirements,
architecture, corpus inventory, acceptance criteria, and roadmap.
Agents must read [AGENTS.md](AGENTS.md) before continuing development.
