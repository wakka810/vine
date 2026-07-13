import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const args = process.argv.slice(2);
let profile = manifest.activeRuntimeCheckpoint.profile;
for (let index = 0; index < args.length; index += 1) {
  if (args[index] !== "--profile" || index + 1 >= args.length) {
    console.error(`unsupported argument: ${args[index]}`);
    process.exit(2);
  }
  profile = args[index + 1];
  index += 1;
}
if (profile !== manifest.activeRuntimeCheckpoint.profile) {
  console.error(
    `profile ${profile} is not the active runtime profile ${manifest.activeRuntimeCheckpoint.profile}`,
  );
  process.exit(2);
}

const result = spawnSync(
  process.execPath,
  [
    path.join(root, "tools", "run-runtime-v86-bgi-first-trap.mjs"),
    "--checkpoint",
    manifest.historicalBaseline.checkpoint,
    "--generic-runtime-reentry",
    "--runtime-capability",
    profile,
    "--runtime-policy",
    path.join(
      root,
      manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
    ),
  ],
  { cwd: root, stdio: "inherit" },
);
if (result.error) {
  throw result.error;
}
process.exit(result.status ?? 1);
