import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import { mkdir, copyFile, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "browser", "runtime-worker");
const benchmarkSource = path.join(root, "browser", "v86-benchmark");
const storageSource = path.join(root, "browser", "storage");
const output = path.join(root, ".work", "browser-runtime");
const executionEngineProfilePath = path.join(
  root,
  "fixtures",
  "execution-engines",
  "v86-browser-v1.json",
);
const executionEngineProfile = JSON.parse(await readFile(executionEngineProfilePath, "utf8"));
const assets = {
  runtimeCore: path.join(
    root,
    "target",
    "wasm32-unknown-unknown",
    "release",
    "vine_runtime_wasm.wasm",
  ),
  v86Adapter: path.join(
    root,
    ".work",
    "v86-adapter-source",
    "build",
    "v86-fallback.wasm",
  ),
  fixture: path.join(root, ".work", "toolchain-spike", "browser-vertical.exe"),
  sourceFixture: path.join(root, ".work", "toolchain-spike", "source-read.exe"),
  sourcePolicy: path.join(
    root,
    "fixtures",
    "runtime-policies",
    "kernel32-source-file-read-v1.json",
  ),
};

for (const [name, file] of Object.entries(assets)) {
  if (!fs.existsSync(file)) {
    throw new Error(`${name} is missing at ${path.relative(root, file)}`);
  }
}

const adapterBytes = await readFile(assets.v86Adapter);
const adapterArtifact = {
  bytes: adapterBytes.byteLength,
  sha256: crypto.createHash("sha256").update(adapterBytes).digest("hex"),
};
const adapterReport = JSON.parse(
  await readFile(path.join(root, ".work", "v86-adapter-spike", "report.json"), "utf8"),
);
assert.equal(executionEngineProfile.schema, "vine.execution-engine-profile");
assert.equal(executionEngineProfile.schemaVersion, 1);
assert.equal(executionEngineProfile.engine, "v86");
assert.equal(adapterReport.sourceRevision, executionEngineProfile.sourceRevision);
assert.equal(
  adapterReport.results.cpuState.version,
  executionEngineProfile.cpuContextAbi.version,
);
assert.equal(adapterReport.results.cpuState.size, executionEngineProfile.cpuContextAbi.bytes);
assert(Array.isArray(executionEngineProfile.artifacts));
const matchedAdapterArtifact = executionEngineProfile.artifacts.find(
  ({ bytes, sha256 }) => bytes === adapterArtifact.bytes && sha256 === adapterArtifact.sha256,
);
assert(
  matchedAdapterArtifact !== undefined,
  "execution engine artifact does not match its pinned profile",
);
const executionEngineIdentity = {
  schema: executionEngineProfile.schema,
  schemaVersion: executionEngineProfile.schemaVersion,
  engine: executionEngineProfile.engine,
  sourceRevision: executionEngineProfile.sourceRevision,
  cpuContextAbi: structuredClone(executionEngineProfile.cpuContextAbi),
  artifact: adapterArtifact,
};

await mkdir(output, { recursive: true });
await build({
  entryPoints: {
    main: path.join(source, "main.ts"),
    "source-check": path.join(source, "source-check.ts"),
    "bgi-check": path.join(source, "bgi-check.ts"),
    "bgi-benchmark": path.join(source, "bgi-benchmark.ts"),
    "runtime-worker": path.join(source, "runtime-worker.ts"),
    "browser-storage-worker": path.join(storageSource, "storage-worker.ts"),
    "benchmark-main": path.join(benchmarkSource, "benchmark-main.ts"),
    "benchmark-worker": path.join(benchmarkSource, "benchmark-worker.ts"),
  },
  bundle: true,
  entryNames: "[name]",
  format: "esm",
  logLevel: "info",
  outdir: output,
  platform: "browser",
  sourcemap: false,
  target: ["chrome120"],
});

await Promise.all([
  copyFile(path.join(source, "index.html"), path.join(output, "index.html")),
  copyFile(path.join(source, "source-check.html"), path.join(output, "source-check.html")),
  copyFile(path.join(source, "bgi-check.html"), path.join(output, "bgi-check.html")),
  copyFile(path.join(source, "bgi-benchmark.html"), path.join(output, "bgi-benchmark.html")),
  copyFile(path.join(source, "styles.css"), path.join(output, "styles.css")),
  copyFile(path.join(benchmarkSource, "index.html"), path.join(output, "benchmark.html")),
  copyFile(path.join(benchmarkSource, "benchmark.css"), path.join(output, "benchmark.css")),
  copyFile(
    path.join(benchmarkSource, "audio-probe-worklet.js"),
    path.join(output, "audio-probe-worklet.js"),
  ),
  copyFile(
    path.join(root, "fixtures", "benchmarks", "v86-browser-v1.json"),
    path.join(output, "v86-browser-v1.json"),
  ),
  copyFile(
    path.join(root, "docs", "current-checkpoint.json"),
    path.join(output, "current-checkpoint.json"),
  ),
  copyFile(executionEngineProfilePath, path.join(output, "execution-engine-profile.json")),
  copyFile(assets.runtimeCore, path.join(output, "runtime-core.wasm")),
  copyFile(assets.v86Adapter, path.join(output, "v86-adapter.wasm")),
  copyFile(assets.fixture, path.join(output, "browser-vertical.exe")),
  copyFile(assets.sourceFixture, path.join(output, "source-read.exe")),
  copyFile(
    assets.sourcePolicy,
    path.join(output, "kernel32-source-file-read-v1.json"),
  ),
  copyFile(
    path.join(root, "node_modules", "@sqlite.org", "sqlite-wasm", "dist", "sqlite3.wasm"),
    path.join(output, "sqlite3.wasm"),
  ),
]);

const manifest = {
  schema: "vine.browser-runtime-build",
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  executionEngine: {
    profile: path.relative(root, executionEngineProfilePath).replaceAll("\\", "/"),
    ...executionEngineIdentity,
  },
  assets: Object.fromEntries(
    await Promise.all(
      [
        "index.html",
        "source-check.html",
        "styles.css",
        "main.js",
        "source-check.js",
        "bgi-check.html",
        "bgi-check.js",
        "bgi-benchmark.html",
        "bgi-benchmark.js",
        "runtime-worker.js",
        "browser-storage-worker.js",
        "sqlite3.wasm",
        "benchmark.html",
        "benchmark.css",
        "benchmark-main.js",
        "benchmark-worker.js",
        "audio-probe-worklet.js",
        "v86-browser-v1.json",
        "current-checkpoint.json",
        "execution-engine-profile.json",
        "runtime-core.wasm",
        "v86-adapter.wasm",
        "browser-vertical.exe",
        "source-read.exe",
        "kernel32-source-file-read-v1.json",
      ].map(async (name) => {
        const bytes = await readFile(path.join(output, name));
        return [
          name,
          {
            bytes: bytes.byteLength,
            sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
          },
        ];
      }),
    ),
  ),
};
await writeFile(path.join(output, "build-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
  `browser runtime built: ${path.relative(root, output)}, ` +
    `${manifest.assets["runtime-worker.js"].bytes.toLocaleString()}-byte Worker bundle`,
);
