import { copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "browser", "runtime-worker");
const target = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target");
const output = path.join(root, ".work", "browser-runtime");
const localModes = await readFile(path.join(root, ".work/v86-execution-modes.json"), "utf8")
  .then(JSON.parse).catch(error => { if (error.code === "ENOENT") return {}; throw error; });
const cpuSource = path.resolve(root, process.env.VINE_V86_SOURCE ?? ".work/vine-v86");
const fastSource = path.resolve(root, process.env.VINE_V86_FAST_SOURCE ?? localModes.fastSource ?? cpuSource);
const strictWasm = path.resolve(root, process.env.VINE_V86_STRICT_WASM ?? localModes.strictWasm ??
  path.join(cpuSource, "build/v86-fallback.wasm"));
const supportsFast = (await readFile(path.join(fastSource, "src/rust/cpu/vine.rs"), "utf8"))
  .includes("fn set_common_memory(");
if (process.env.VINE_V86_FAST_SOURCE && !supportsFast) throw new Error("Selected Fast CPU does not support the common Runtime arena");
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
  copyFile(path.join(target, "wasm32-unknown-unknown/release/vine_runtime_wasm.wasm"),
    path.join(output, "runtime-core.wasm")),
  copyFile(path.join(target, "wasm32-unknown-unknown/release/vine_recompiler.wasm"),
    path.join(output, "vine_recompiler.wasm")),
  copyFile(path.join(root, "crates/vine-x87/vendor/COPYING.txt"),
    path.join(output, "vine-x87-LICENSE.txt")),
  copyFile(strictWasm,
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
if (supportsFast) {
  const result = spawnSync(process.execPath, [path.join(root, "tools/build-shared-runtime.mjs"),
    path.join(output, "runtime-cpu.wasm")], { cwd: root,
    env: { ...process.env, VINE_V86_FAST_SOURCE: fastSource }, stdio: "inherit", windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Fast Runtime build failed: ${result.status}`);
} else {
  const html = await readFile(path.join(output, "index.html"), "utf8");
  await writeFile(path.join(output, "index.html"), html.replace(/<option value="fast">[^<]*<\/option>/u, ""));
}
const artifacts = ["runtime-core.wasm", "v86-adapter.wasm", ...(supportsFast ? ["runtime-cpu.wasm"] : [])];
await writeFile(path.join(output, "execution-modes.json"), JSON.stringify({ default: supportsFast ? "fast" : "strict", fast: supportsFast,
  artifacts: Object.fromEntries(await Promise.all(artifacts.map(async name => [name,
    createHash("sha256").update(await readFile(path.join(output, name))).digest("hex")]))) }, null, 2) + "\n");
console.log(`Chrome product built: ${path.relative(root, output)} (${fixtures.length} PE32 smoke fixtures)`);
