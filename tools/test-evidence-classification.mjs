import assert from "node:assert/strict";

import {
  assertRequiredPartialOrder,
  classifyBenchmarkDecision,
  EVIDENCE_CLASSIFICATION,
  EvidenceOrderingError,
  observeArtifactIdentity,
} from "./evidence-classification.mjs";

assert.deepEqual(EVIDENCE_CLASSIFICATION, {
  guestVisibleSemanticValue: "exact-or-allowed-set",
  requiredOrdering: "partial-order-or-exact-sequence",
  opaqueIdentity: "equality-stability-lifetime",
  performance: "repeated-distribution-threshold",
  browserNativeNondeterministicValue: "range-shape-presence",
  reportHash: "artifact-identity-only",
});

const artifactBytes = Buffer.from("historical report\n");
const exactArtifact = observeArtifactIdentity(artifactBytes, {
  bytes: 18,
  sha256: "251233969aaf3848b03a5ddd23bb64dbc0cf90360f6e2e5832a36855ad358ee6",
});
assert.equal(exactArtifact.matches, true);
assert.equal(exactArtifact.semanticGate, false);
const changedArtifact = observeArtifactIdentity(Buffer.from("new report representation\n"), {
  bytes: exactArtifact.expected.bytes,
  sha256: exactArtifact.expected.sha256,
});
assert.equal(changedArtifact.matches, false, "artifact mismatch became a semantic failure");

const requiredSchedulerOrder = [
  ["request:1", "completion:1"],
  ["request:2", "completion:2"],
  ["completion:1", "stop"],
  ["completion:2", "stop"],
];
for (const allowed of [
  ["request:1", "request:2", "completion:1", "completion:2", "stop"],
  ["request:2", "completion:2", "request:1", "completion:1", "stop"],
]) {
  assert.equal(
    assertRequiredPartialOrder(allowed, requiredSchedulerOrder).gate,
    "partial-order",
  );
}
assert.throws(
  () =>
    assertRequiredPartialOrder(
      ["completion:1", "request:1", "request:2", "completion:2", "stop"],
      requiredSchedulerOrder,
    ),
  EvidenceOrderingError,
);

const passingDecision = classifyBenchmarkDecision([
  { name: "latency-p95", passed: true },
  { name: "memory-slope", passed: true },
]);
assert.deepEqual(passingDecision, {
  classification: "repeated-distribution-threshold",
  status: "passed",
  reason: "all-2-versioned-thresholds-passed",
  evaluationCount: 2,
  failureIds: [],
});
assert.deepEqual(
  classifyBenchmarkDecision([
    { name: "latency-p95", passed: false },
    { name: "memory-slope", passed: true },
  ]),
  {
    classification: "repeated-distribution-threshold",
    status: "failed",
    reason: "1-of-2-versioned-thresholds-failed",
    evaluationCount: 2,
    failureIds: ["latency-p95"],
  },
);

console.log("evidence classification contract passed");
