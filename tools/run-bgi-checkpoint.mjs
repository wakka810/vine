import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { planBgiCheckpointCommand } from "./bgi-checkpoint-command.mjs";
import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runtimeReplayHeapLimitMiB = 8 * 1024;
const manifest = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);

let plan;
try {
  plan = planBgiCheckpointCommand(process.argv.slice(2), manifest, root);
} catch (error) {
  console.error(error.message);
  process.exit(2);
}
const result = spawnSync(
  process.execPath,
  [
    `--max-old-space-size=${runtimeReplayHeapLimitMiB}`,
    path.join(root, "tools", "run-runtime-v86-bgi-first-trap.mjs"),
    ...plan.runnerArguments,
  ],
  { cwd: root, stdio: "inherit" },
);
if (result.error) {
  throw result.error;
}
process.exit(result.status ?? 1);
