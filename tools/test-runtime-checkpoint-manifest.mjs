import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  callsThroughCheckpoint,
  distinctDispatchesThroughCheckpoint,
  loadRuntimeCheckpointManifest,
  selectRuntimeCheckpoint,
  validateRuntimeCheckpointManifest,
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
assert(finalCheckpoint.evidence.reportBytes > 0);
assert.match(finalCheckpoint.evidence.reportSha256, /^[0-9a-f]{64}$/u);
assert.equal(manifest.evidenceClassification.reportHash, "artifact-identity-only");
assert.equal(
  manifest.evidenceClassification.historicalCheckpointOrdering,
  "exact-sequence",
);
const alternateArtifactIdentity = structuredClone(manifest);
alternateArtifactIdentity.checkpoints.at(-1).evidence.reportBytes += 1;
alternateArtifactIdentity.checkpoints.at(-1).evidence.reportSha256 = "0".repeat(64);
assert.doesNotThrow(() => validateRuntimeCheckpointManifest(alternateArtifactIdentity));
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
