import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  callsThroughCheckpoint,
  distinctDispatchesThroughCheckpoint,
  loadRuntimeCheckpointManifest,
  selectRuntimeCheckpoint,
} from "./runtime-checkpoint-manifest.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadRuntimeCheckpointManifest(
  path.join(root, "fixtures", "checkpoints", "bgi-startup-win32.json"),
);
const finalCheckpoint = selectRuntimeCheckpoint(manifest, "bgi-seventeenth-win32-call");

assert.equal(manifest.checkpoints.length, 18);
assert.equal(finalCheckpoint.supportedCallCount, 17);
assert.equal(callsThroughCheckpoint(manifest, finalCheckpoint).length, 17);
assert.equal(distinctDispatchesThroughCheckpoint(manifest, finalCheckpoint).length, 13);
assert.equal(finalCheckpoint.evidence.instructionCount, 238);
assert.equal(finalCheckpoint.evidence.reportBytes, 140_660);
assert.equal(
  finalCheckpoint.evidence.reportSha256,
  "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
);
assert.deepEqual(finalCheckpoint.nextStop, {
  module: "KERNEL32.dll",
  symbol: "EncodePointer",
  occurrence: 3,
  arguments: ["0x7f020000"],
});

console.log(
  `runtime checkpoint manifest passed: ${finalCheckpoint.supportedCallCount} calls, ` +
    `${distinctDispatchesThroughCheckpoint(manifest, finalCheckpoint).length} APIs`,
);
