import { copyFile, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "browser", "runtime-worker");
const output = path.join(root, ".work", "browser-runtime");
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await build({
  entryPoints: {
    play: path.join(source, "play.ts"),
    "inspect-worker": path.join(source, "inspect-worker.ts"),
    "game-worker": path.join(source, "game-worker.ts"),
    "writable-tree": path.join(source, "writable-tree.ts"),
    "gdi-text-raster": path.join(source, "gdi-text-raster.ts"),
    "game-surface": path.join(source, "game-surface.ts"),
  },
  bundle: true,
  entryNames: "[name]",
  format: "esm",
  platform: "browser",
  target: ["chrome120"],
  outdir: output,
});
const fixtures = ["browser-vertical.exe", "bootstrap-fs.exe", "fls-get-value.exe",
  "fls-exit-thread.exe", "enum-display-monitors.exe", "d3d9-display.exe", "pointer-input.exe", "source-read.exe",
  "keyboard-input.exe", "sleep-delay.exe", "cursor-state.exe", "timer-wait.exe", "timer-threads.exe",
  "dll-lifecycle.exe", "message-box.exe", "heap-growth.exe", "directshow.exe"];
await Promise.all([
  copyFile(path.join(source, "pcm-worklet.js"), path.join(output, "pcm-worklet.js")),
  copyFile(path.join(source, "index.html"), path.join(output, "index.html")),
  copyFile(path.join(root, "target/wasm32-unknown-unknown/release/vine_runtime_wasm.wasm"),
    path.join(output, "runtime-core.wasm")),
  copyFile(path.join(root, ".work/vine-v86/build/v86-fallback.wasm"),
    path.join(output, "v86-adapter.wasm")),
  copyFile(path.join(root, "fixtures/storage/source-read-v1.dat"),
    path.join(output, "source-read-v1.dat")),
  mkdir(path.join(output, "libav-media"), { recursive: true }).then(async () => {
    const media = path.join(root, ".work", "libav-mpeg-build");
    await Promise.all([
      ...["libav-6.10.9.0-vine-mpeg.mjs", "libav-6.10.9.0-vine-mpeg.wasm.mjs",
        "libav-6.10.9.0-vine-mpeg.wasm.wasm"].map(name =>
        copyFile(path.join(media, "dist", name), path.join(output, "libav-media", name))),
      copyFile(path.join(media, "LICENSE.md"), path.join(output, "libav-media", "LICENSE.md")),
    ]);
  }),
  ...[...fixtures, "vine_browser_fixture.dll", "lifecycle_engine.dll", "lifecycle_codec.dll"]
    .map((name) =>
    copyFile(path.join(root, ".work/toolchain-spike", name), path.join(output, name))),
]);
console.log(`Chrome product built: ${path.relative(root, output)} (${fixtures.length} PE32 smoke fixtures)`);
