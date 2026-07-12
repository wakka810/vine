import fs from "node:fs";
import path from "node:path";

const SHA256 = /^[0-9a-f]{64}$/u;
const CHECKPOINT_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/u;

function invariant(condition, message) {
  if (!condition) {
    throw new Error(`runtime checkpoint manifest: ${message}`);
  }
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function dispatchKey(dispatch) {
  return `${dispatch.module.toLowerCase()}!${dispatch.symbol}`;
}

function validateDispatch(dispatch, label) {
  invariant(isRecord(dispatch), `${label} must be an object`);
  invariant(
    typeof dispatch.module === "string" && dispatch.module.length > 0,
    `${label}.module must be a non-empty string`,
  );
  invariant(
    typeof dispatch.symbol === "string" && dispatch.symbol.length > 0,
    `${label}.symbol must be a non-empty string`,
  );
  invariant(
    Number.isSafeInteger(dispatch.occurrence) && dispatch.occurrence > 0,
    `${label}.occurrence must be a positive integer`,
  );
}

function validateEvidence(evidence, checkpoint) {
  const label = `${checkpoint.id}.evidence`;
  invariant(isRecord(evidence), `${label} must be an object`);
  for (const field of [
    "instructionCount",
    "offerLineBytes",
    "reflectionReceiptLineBytes",
    "reportBytes",
  ]) {
    invariant(
      Number.isSafeInteger(evidence[field]) && evidence[field] > 0,
      `${label}.${field} must be a positive integer`,
    );
  }
  invariant(
    evidence.reportSchema === `vine.v86-${checkpoint.id}`,
    `${label}.reportSchema must match the checkpoint ID`,
  );
  invariant(SHA256.test(evidence.reflectionSha256), `${label}.reflectionSha256 is invalid`);
  invariant(SHA256.test(evidence.reportSha256), `${label}.reportSha256 is invalid`);
}

export function validateRuntimeCheckpointManifest(manifest) {
  invariant(isRecord(manifest), "root must be an object");
  invariant(
    manifest.schema === "vine.runtime-checkpoint-manifest" && manifest.schemaVersion === 1,
    "schema must be vine.runtime-checkpoint-manifest version 1",
  );
  invariant(typeof manifest.id === "string" && CHECKPOINT_ID.test(manifest.id), "id is invalid");
  invariant(isRecord(manifest.source), "source must be an object");
  invariant(
    Number.isSafeInteger(manifest.source.bytes) && manifest.source.bytes > 0,
    "source.bytes must be a positive integer",
  );
  invariant(SHA256.test(manifest.source.sha256), "source.sha256 is invalid");
  invariant(isRecord(manifest.adapter), "adapter must be an object");
  invariant(
    Number.isSafeInteger(manifest.adapter.bytes) && manifest.adapter.bytes > 0,
    "adapter.bytes must be a positive integer",
  );
  invariant(SHA256.test(manifest.adapter.sha256), "adapter.sha256 is invalid");
  invariant(Array.isArray(manifest.checkpoints), "checkpoints must be an array");
  invariant(manifest.checkpoints.length > 1, "checkpoints must include a trap and at least one call");

  const ids = new Set();
  const occurrences = new Map();
  for (const [index, checkpoint] of manifest.checkpoints.entries()) {
    const label = `checkpoints[${index}]`;
    invariant(isRecord(checkpoint), `${label} must be an object`);
    invariant(
      typeof checkpoint.id === "string" && CHECKPOINT_ID.test(checkpoint.id),
      `${label}.id is invalid`,
    );
    invariant(!ids.has(checkpoint.id), `${label}.id is duplicated`);
    ids.add(checkpoint.id);
    invariant(
      checkpoint.supportedCallCount === index,
      `${checkpoint.id}.supportedCallCount must equal its sequence index`,
    );
    invariant(
      typeof checkpoint.artifactDirectory === "string" &&
        CHECKPOINT_ID.test(checkpoint.artifactDirectory),
      `${checkpoint.id}.artifactDirectory is invalid`,
    );
    invariant(
      checkpoint.extends === (index === 0 ? null : manifest.checkpoints[index - 1].id),
      `${checkpoint.id}.extends must name the preceding checkpoint`,
    );

    if (index === 0) {
      invariant(checkpoint.dispatch === null, `${checkpoint.id}.dispatch must be null`);
      invariant(checkpoint.expected === null, `${checkpoint.id}.expected must be null`);
    } else {
      validateDispatch(checkpoint.dispatch, `${checkpoint.id}.dispatch`);
      const key = dispatchKey(checkpoint.dispatch);
      const occurrence = (occurrences.get(key) ?? 0) + 1;
      invariant(
        checkpoint.dispatch.occurrence === occurrence,
        `${checkpoint.id}.dispatch.occurrence is not sequential`,
      );
      occurrences.set(key, occurrence);
      invariant(isRecord(checkpoint.expected), `${checkpoint.id}.expected must be an object`);
      invariant(
        Array.isArray(checkpoint.expected.arguments),
        `${checkpoint.id}.expected.arguments must be an array`,
      );
      invariant(
        isRecord(checkpoint.expected.result),
        `${checkpoint.id}.expected.result must be an object`,
      );
      invariant(
        Array.isArray(checkpoint.expected.guestWrites),
        `${checkpoint.id}.expected.guestWrites must be an array`,
      );
    }

    validateDispatch(checkpoint.nextStop, `${checkpoint.id}.nextStop`);
    invariant(
      Array.isArray(checkpoint.nextStop.arguments),
      `${checkpoint.id}.nextStop.arguments must be an array`,
    );
    const nextKey = dispatchKey(checkpoint.nextStop);
    invariant(
      checkpoint.nextStop.occurrence === (occurrences.get(nextKey) ?? 0) + 1,
      `${checkpoint.id}.nextStop.occurrence does not follow dispatched calls`,
    );
    validateEvidence(checkpoint.evidence, checkpoint);
  }

  return manifest;
}

export function loadRuntimeCheckpointManifest(file) {
  const resolved = path.resolve(file);
  const manifest = JSON.parse(fs.readFileSync(resolved, "utf8"));
  validateRuntimeCheckpointManifest(manifest);
  return manifest;
}

export function selectRuntimeCheckpoint(manifest, checkpointId) {
  const checkpoint = manifest.checkpoints.find(({ id }) => id === checkpointId);
  invariant(checkpoint !== undefined, `unknown checkpoint ${checkpointId}`);
  return checkpoint;
}

export function callsThroughCheckpoint(manifest, checkpoint) {
  return manifest.checkpoints.slice(1, checkpoint.supportedCallCount + 1);
}

export function distinctDispatchesThroughCheckpoint(manifest, checkpoint) {
  const dispatches = new Map();
  for (const { dispatch } of callsThroughCheckpoint(manifest, checkpoint)) {
    dispatches.set(dispatchKey(dispatch), dispatch);
  }
  return [...dispatches.values()];
}
