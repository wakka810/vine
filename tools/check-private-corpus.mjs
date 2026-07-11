import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultBgi = process.platform === "win32" ? "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe" : null;
const bgi = process.env.VINE_BGI_EXE ?? defaultBgi;
if (!bgi || !fs.existsSync(bgi)) {
  console.log(
    "check:private-corpus skipped: BGI.exe is unavailable; set VINE_BGI_EXE to the private corpus path",
  );
  process.exit(0);
}

const wasm = path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm");
if (!fs.existsSync(wasm)) {
  throw new Error(
    "private corpus is available but the source adapter is missing; run npm run check:adapter first",
  );
}
for (const modeArguments of [
  [],
  ["--dispatch-system-time"],
  ["--dispatch-process-id"],
  ["--dispatch-thread-id"],
  ["--dispatch-tick-count"],
  ["--dispatch-performance-counter"],
  ["--dispatch-startup-info"],
]) {
  const result = spawnSync(
    process.execPath,
    [
      path.join(root, "tools", "run-runtime-v86-bgi-first-trap.mjs"),
      ...modeArguments,
      wasm,
      bgi,
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
