import {createHash} from "node:crypto";
import {spawn} from "node:child_process";
import {copyFile, mkdir, readFile, stat, writeFile} from "node:fs/promises";
import path from "node:path";
import {fileURLToPath} from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const build = path.join(root, ".work", "libav-mpeg-build");
const sdk = path.join(root, ".work", "emsdk-media");
const packageCache = path.join(root, ".work", "libav-media-source");
const archive = path.join(root, ".work", "libav.js-6.10.9.tgz");
const integrity = "sha512-g1v+2ZWShgAwvJY8t3Q1c7n1XV+7jq/uDdj9Ak+twsgBdSdEGbXhKTHkBBuBW7SbmTH7TpoBaVleRpP+YiCh/Q==";
const sdkCommit = "c0bb220cb6e6f4e0fabb6f6db9efd53390ef5e56";
const version = "6.10.9.0";
const fragments = ["avformat", "avcodec", "avfilter", "swscale", "swresample",
  "demuxer-mpegps", "demuxer-mpegvideo", "parser-mpegvideo", "decoder-mpeg1video",
  "parser-mpegaudio", "decoder-mp2", "filter-abuffer", "filter-abuffersink",
  "filter-aformat", "filter-aresample"];
const runtimeFiles = [`libav-${version}-vine-mpeg.mjs`,
  `libav-${version}-vine-mpeg.wasm.mjs`, `libav-${version}-vine-mpeg.wasm.wasm`];

async function run(command, args, cwd = root) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {cwd, stdio: "inherit", windowsHide: true});
    child.once("error", reject);
    child.once("exit", code => code === 0 ? resolve() : reject(new Error(`${command} exited with ${code}`)));
  });
}

async function exists(file) {
  try { await readFile(file); return true; }
  catch (error) { if (error.code === "ENOENT") return false; throw error; }
}

function sourceIntegrity(bytes) {
  return `sha512-${createHash("sha512").update(bytes).digest("base64")}`;
}

await mkdir(path.join(root, ".work"), {recursive: true});
let source = await readFile(archive).catch(error => {
  if (error.code === "ENOENT") return null;
  throw error;
});
if (source === null) {
  const response = await fetch("https://registry.npmjs.org/libav.js/-/libav.js-6.10.9.tgz");
  if (!response.ok) throw new Error(`libav.js source fetch failed with ${response.status}`);
  if (Number(response.headers.get("content-length")) > 96 * 1024 * 1024) throw new Error("libav.js source package exceeds its bound");
  source = new Uint8Array(await response.arrayBuffer());
  if (source.byteLength > 96 * 1024 * 1024 || sourceIntegrity(source) !== integrity) {
    throw new Error("libav.js source package integrity disagrees with the pinned release");
  }
  await writeFile(archive, source);
}
if (sourceIntegrity(source) !== integrity) throw new Error("Cached libav.js source failed its pinned integrity check");

await mkdir(packageCache, {recursive: true});
await run("tar", ["-xzf", archive, "-C", packageCache,
  "package/sources/libav.js.tar.xz", "package/sources/ffmpeg-9.0.tar.xz",
  "package/sources/emfiberthreads-1.3.tar.gz"]);
const sources = path.join(packageCache, "package", "sources");
await mkdir(path.join(build, "sources"), {recursive: true});
for (const file of ["libav.js.tar.xz", "ffmpeg-9.0.tar.xz", "emfiberthreads-1.3.tar.gz"]) {
  await copyFile(path.join(sources, file), path.join(build, "sources", file));
}

let cachedConfig = null;
try { cachedConfig = JSON.parse(await readFile(path.join(build, "configs", "configs", "vine-mpeg", "config.json"), "utf8")); }
catch (error) { if (error.code !== "ENOENT") throw error; }
if (JSON.stringify(cachedConfig) === JSON.stringify(fragments) &&
    await exists(path.join(build, "LICENSE.md")) &&
    (await Promise.all(runtimeFiles.map(async file => {
      try { return (await stat(path.join(build, "dist", file))).size > 0; }
      catch (error) { if (error.code === "ENOENT") return false; throw error; }
    }))).every(Boolean)) {
  console.log("Pinned MPEG-1/MP2 decoder is prepared");
  process.exit(0);
}

await mkdir(build, {recursive: true});
if (!await exists(path.join(build, "Makefile"))) {
  await run("tar", ["-xJf", path.join(sources, "libav.js.tar.xz"), "-C", build]);
}
await mkdir(path.join(build, "build"), {recursive: true});
for (const file of ["ffmpeg-9.0.tar.xz", "emfiberthreads-1.3.tar.gz"]) {
  await copyFile(path.join(sources, file), path.join(build, "build", file));
}
if (!await exists(path.join(sdk, "emsdk.py"))) {
  await run("git", ["clone", "--depth", "1", "--branch", "4.0.23",
    "https://github.com/emscripten-core/emsdk.git", sdk]);
}
const actualCommit = (await readFile(path.join(sdk, ".git", "HEAD"), "utf8")).trim();
if (actualCommit !== sdkCommit) throw new Error("Media compiler SDK is not the pinned 4.0.23 checkout");
const python = process.platform === "win32" ? "python" : "python3";
await run(python, [path.join(sdk, "emsdk.py"), "install", "4.0.23"], sdk);
await run(python, [path.join(sdk, "emsdk.py"), "activate", "4.0.23"], sdk);
await run("node", ["mkconfig.js", "vine-mpeg", JSON.stringify(fragments)], path.join(build, "configs"));
const npmArgs = ["install", "--ignore-scripts", "--no-audit", "--no-fund"];
if (process.platform === "win32") {
  // A .cmd wrapper cannot be spawned directly without a command interpreter.
  const npmCli = path.join(path.dirname(process.execPath), "node_modules", "npm", "bin", "npm-cli.js");
  await run(process.execPath, [npmCli, ...npmArgs], build);
} else await run("npm", npmArgs, build);

const script = path.join(build, "build-media.sh");
if (process.platform === "win32") {
  const unix = value => value.replaceAll("\\", "/").replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);
  const hostTools = path.join(build, "host-tools");
  await mkdir(hostTools, {recursive: true});
  for (const tool of ["emcc", "em++", "emar", "emranlib", "emconfigure", "emmake"]) {
    await writeFile(path.join(hostTools, tool), `#!/usr/bin/env bash\nexec "${unix(path.join(sdk, "python", "3.13.3_64bit", "python.exe"))}" "${unix(path.join(sdk, "upstream", "emscripten", `${tool}.py`))}" "$@"\n`);
  }
  const gitTools = unix(path.join(process.env.ProgramFiles ?? "C:\\Program Files", "Git", "usr", "bin"));
  await writeFile(script, `#!/usr/bin/env bash
set -e
export EM_CONFIG="${sdk.replaceAll("\\", "/")}/.emscripten"
export EMSDK="${sdk.replaceAll("\\", "/") }"
export PATH="${unix(hostTools)}:/usr/bin:${gitTools}:$PATH"
chmod +x host-tools/*
make -j4 "$@"
`);
} else {
  await writeFile(script, `#!/usr/bin/env bash\nset -e\nsource "${sdk}/emsdk_env.sh"\nmake -j4 "$@"\n`);
}
await run("bash", [script, ...runtimeFiles.filter(file => !file.endsWith(".wasm")).map(file => `dist/${file}`)], build);
for (const file of runtimeFiles) {
  const bytes = await readFile(path.join(build, "dist", file));
  if (bytes.byteLength === 0) throw new Error(`Media decoder build produced an empty ${file}`);
  console.log(`${file}: ${bytes.byteLength} bytes, SHA256 ${createHash("sha256").update(bytes).digest("hex")}`);
}
