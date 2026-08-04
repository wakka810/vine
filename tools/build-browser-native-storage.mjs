import crypto from "node:crypto";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "browser", "storage");
const output = path.join(root, ".work", "browser-native-storage");

await mkdir(output, { recursive: true });
await build({
  entryPoints: {
    "storage-check": path.join(source, "check-main.ts"),
    "browser-storage-worker": path.join(source, "storage-worker.ts"),
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
  copyFile(path.join(source, "check.html"), path.join(output, "storage-check.html")),
  copyFile(
    path.join(root, "node_modules", "@sqlite.org", "sqlite-wasm", "dist", "sqlite3.wasm"),
    path.join(output, "sqlite3.wasm"),
  ),
]);

const assetNames = [
  "storage-check.html",
  "storage-check.js",
  "browser-storage-worker.js",
  "sqlite3.wasm",
];
const manifest = {
  schema: "vine.browser-native-storage-build",
  schemaVersion: 1,
  assets: Object.fromEntries(
    await Promise.all(
      assetNames.map(async (name) => {
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
await writeFile(
  path.join(output, "build-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
console.log(
  `browser-native storage built: ${path.relative(root, output)}, ` +
    `${manifest.assets["browser-storage-worker.js"].bytes.toLocaleString()}-byte Worker bundle`,
);
