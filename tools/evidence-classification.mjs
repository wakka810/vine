import crypto from "node:crypto";

const SHA256 = /^[0-9a-f]{64}$/u;

export const EVIDENCE_CLASSIFICATION = Object.freeze({
  guestVisibleSemanticValue: "exact-or-allowed-set",
  requiredOrdering: "partial-order-or-exact-sequence",
  opaqueIdentity: "equality-stability-lifetime",
  performance: "repeated-distribution-threshold",
  browserNativeNondeterministicValue: "range-shape-presence",
  reportHash: "artifact-identity-only",
});

function invariant(condition, message) {
  if (!condition) {
    throw new Error(`evidence classification: ${message}`);
  }
}

function validateArtifactIdentity(identity, label) {
  invariant(identity !== null && typeof identity === "object", `${label} must be an object`);
  invariant(
    Number.isSafeInteger(identity.bytes) && identity.bytes > 0,
    `${label}.bytes must be a positive integer`,
  );
  invariant(SHA256.test(identity.sha256), `${label}.sha256 must be lowercase SHA-256`);
}

export function observeArtifactIdentity(bytes, expected) {
  validateArtifactIdentity(expected, "expected artifact identity");
  const owned = Buffer.from(bytes);
  const observed = {
    bytes: owned.byteLength,
    sha256: crypto.createHash("sha256").update(owned).digest("hex"),
  };
  return {
    classification: EVIDENCE_CLASSIFICATION.reportHash,
    semanticGate: false,
    matches: observed.bytes === expected.bytes && observed.sha256 === expected.sha256,
    expected: structuredClone(expected),
    observed,
  };
}

export class EvidenceOrderingError extends Error {
  constructor(message) {
    super(message);
    this.name = "EvidenceOrderingError";
    this.code = "required-order-violated";
  }
}

export function assertRequiredPartialOrder(events, requiredRelations) {
  invariant(Array.isArray(events), "events must be an array");
  invariant(Array.isArray(requiredRelations), "required relations must be an array");
  const positions = new Map();
  for (const [index, event] of events.entries()) {
    invariant(typeof event === "string" && event.length > 0, `events[${index}] is invalid`);
    if (positions.has(event)) {
      throw new EvidenceOrderingError(`event ${event} is duplicated`);
    }
    positions.set(event, index);
  }
  for (const [index, relation] of requiredRelations.entries()) {
    invariant(
      Array.isArray(relation) && relation.length === 2,
      `requiredRelations[${index}] must contain before and after event IDs`,
    );
    const [before, after] = relation;
    invariant(typeof before === "string" && typeof after === "string", "event IDs must be strings");
    invariant(positions.has(before), `required event ${before} is missing`);
    invariant(positions.has(after), `required event ${after} is missing`);
    if (positions.get(before) >= positions.get(after)) {
      throw new EvidenceOrderingError(`required order ${before} -> ${after} was violated`);
    }
  }
  return {
    classification: EVIDENCE_CLASSIFICATION.requiredOrdering,
    gate: "partial-order",
    eventCount: events.length,
    relationCount: requiredRelations.length,
  };
}

export function classifyBenchmarkDecision(thresholdEvaluations) {
  invariant(
    Array.isArray(thresholdEvaluations) && thresholdEvaluations.length > 0,
    "benchmark threshold evaluations must be non-empty",
  );
  const names = new Set();
  const failureIds = [];
  for (const [index, evaluation] of thresholdEvaluations.entries()) {
    invariant(
      evaluation !== null && typeof evaluation === "object",
      `thresholdEvaluations[${index}] must be an object`,
    );
    invariant(
      typeof evaluation.name === "string" && evaluation.name.length > 0,
      `thresholdEvaluations[${index}].name is invalid`,
    );
    invariant(!names.has(evaluation.name), `threshold ${evaluation.name} is duplicated`);
    names.add(evaluation.name);
    invariant(
      typeof evaluation.passed === "boolean",
      `thresholdEvaluations[${index}].passed must be boolean`,
    );
    if (!evaluation.passed) {
      failureIds.push(evaluation.name);
    }
  }
  const evaluationCount = thresholdEvaluations.length;
  return {
    classification: EVIDENCE_CLASSIFICATION.performance,
    status: failureIds.length === 0 ? "passed" : "failed",
    reason:
      failureIds.length === 0
        ? `all-${evaluationCount}-versioned-thresholds-passed`
        : `${failureIds.length}-of-${evaluationCount}-versioned-thresholds-failed`,
    evaluationCount,
    failureIds,
  };
}
