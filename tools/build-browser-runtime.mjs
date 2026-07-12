import crypto from "node:crypto";
import fs from "node:fs";
import { mkdir, copyFile, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "browser", "runtime-worker");
const output = path.join(root, ".work", "browser-runtime");
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
};

for (const [name, file] of Object.entries(assets)) {
  if (!fs.existsSync(file)) {
    throw new Error(`${name} is missing at ${path.relative(root, file)}`);
  }
}

await mkdir(output, { recursive: true });
await build({
  entryPoints: {
    main: path.join(source, "main.ts"),
    "runtime-worker": path.join(source, "runtime-worker.ts"),
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
  copyFile(path.join(source, "styles.css"), path.join(output, "styles.css")),
  copyFile(assets.runtimeCore, path.join(output, "runtime-core.wasm")),
  copyFile(assets.v86Adapter, path.join(output, "v86-adapter.wasm")),
  copyFile(assets.fixture, path.join(output, "browser-vertical.exe")),
]);

const manifest = {
  schema: "vine.browser-runtime-build",
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  assets: Object.fromEntries(
    await Promise.all(
      [
        "index.html",
        "styles.css",
        "main.js",
        "runtime-worker.js",
        "runtime-core.wasm",
        "v86-adapter.wasm",
        "browser-vertical.exe",
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
