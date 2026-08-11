import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  README_STATUS_END,
  README_STATUS_START,
  loadCurrentCheckpoint,
  renderReadmeStatus,
  validateCurrentCheckpoint,
} from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadCurrentCheckpoint(path.join(root, "docs", "current-checkpoint.json"));
assert.deepEqual(validateCurrentCheckpoint(manifest), []);
assert.equal(manifest.schemaVersion, 5);
assert.equal(manifest.development.mode, "forward");
assert(manifest.development.maximumRunSeconds <= 600);
assert(manifest.development.semanticTailEntries >= 256);
assert(manifest.development.semanticTailEntries <= 512);

const rendered = renderReadmeStatus(manifest);
assert(rendered.startsWith(README_STATUS_START));
assert(rendered.endsWith(README_STATUS_END));
assert(rendered.includes(manifest.activeMilestone.id));
assert(rendered.includes(manifest.currentForwardStop.endpoint));
assert(rendered.includes(manifest.commands.forward));
for (const subsystem of manifest.supportedSubsystems) {
  assert(rendered.includes(subsystem.id));
  assert(rendered.includes(subsystem.summary));
}

const invalid = structuredClone(manifest);
invalid.development.maximumRunSeconds = 601;
assert(validateCurrentCheckpoint(invalid).some((error) => error.includes("maximumRunSeconds")));

console.log("compact current state and generated README contract passed");
