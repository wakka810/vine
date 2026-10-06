# Vine

Vine is a Chrome-hosted PE32/i386 Windows userspace runtime for unmodified VN/ADV and light-game executables, DLLs, and assets. It is under active development and does **not** yet provide general game compatibility.

## Run

```sh
npm install
npm run build:runtime:browser-worker
npm run serve:runtime:browser-worker
```

Open the served root page in Chrome. Choose a game directory, browse within it, select an `.exe`, then launch it. Directory browsing and executable reads use the selected directory handle without writing to source files. The executable is checked by the Rust PE32 loader before launch.

**Current status:** Seventeen small PE32 fixtures execute through Rust/Wasm and v86 in Chrome. They cover thread return/exit, guarded stack growth, finite timer waits and Sleep, callbacks, COM lifetimes, message boxes, concurrent large heap allocations, immutable source reads, keyboard polling, and logical cursor state, and guest DirectShow filters receiving decoded MPEG-1 samples and EOF. Bounded D3D9 `Present` now displays color clears and fixed-function textured quads through WebGL2; Chrome tests check actual canvas pixels, texture orientation, malformed-frame rejection, and context loss. The connected texture path supports rectangle fills and bounded BGRX guest-memory snapshots captured at DrawPrimitive; pixel bytes cross Wasm as a binary buffer, with no guest pointers in the browser adapter. This is fixture rendering, not proof of a readable or playable game.

Source files remain read-only. Guest `CreateDirectoryA`, `CreateFileA`, `WriteFile`, `ReadFile`, `SetFilePointer`, `FlushFileBuffers`, `CloseHandle`, file attributes, and deletion now use a sandboxed game-local OPFS overlay for the bounded reached variants. A PE32 fixture writes a nonempty save, closes it, starts a fresh Chrome worker/runtime, and reads the exact bytes back; failed OPFS writes preserve the bytes, cursor, and output in focused checks. **This is a fixture test, not a completed save/load or playable-title claim.** Unsupported APIs and variants stop explicitly. Acyclic static source DLL imports now map and attach before the EXE starts; Chrome fixtures call transitive DLL exports and reuse an attached DLL through `LoadLibraryA`. Unmaterialized dynamic DLL loading, circular imports, full-title rendering/audio/save behavior, and broader texture-lock/update variants remain unfinished.

## Development

```sh
npm run check:fast
npm run check:rust
npm run check:runtime:browser-worker:prepared
```

The last command needs the prepared toolchain, v86 adapter, and media decoder (`npm run check:toolchain`, `npm run prepare:engine`, and `npm run prepare:media` if absent). The Chrome check runs the generic directory/executable picker and the PE32 smoke fixtures, including WebGL2 pixels and guest OPFS save/close/fresh-worker/read. `npm run dev:target` serves the same generic product entry point. Node is build/test tooling, not a production runtime or a Windows semantic oracle.

One Rust `Runtime` owns `GuestMemory` and guest-visible Windows state; one pinned [Vine-v86 fork](https://github.com/wakka810/vine-v86) executes it. TypeScript owns browser resources and adapters. Source files are immutable inputs. Guest writable files live in the game-local OPFS tree; future reached registry/configuration state belongs there too, not in the source directory. Historical checkpoint documents and reports are not the development queue. See [Architecture](ARCHITECTURE.md), [Development](DEVELOPMENT.md), and [Status](STATUS.md) for the current boundaries, commands, and unfinished work.
