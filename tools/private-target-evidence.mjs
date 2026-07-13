import crypto from "node:crypto";

export const PRIVATE_TARGET_EVIDENCE_SCHEMA = "vine.private-target-evidence";
export const PRIVATE_TARGET_EVIDENCE_VERSION = 4;

const GIT_OBJECT_ID = /^[0-9a-f]{40}$/u;
const SHA256 = /^[0-9a-f]{64}$/u;

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireRecord(value, label, errors) {
  if (!isRecord(value)) {
    errors.push(`${label} must be an object`);
    return {};
  }
  return value;
}

function requireString(value, label, errors) {
  if (typeof value !== "string" || value.length === 0) {
    errors.push(`${label} must be a non-empty string`);
  }
}

function requireInteger(value, label, errors, minimum = 0) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    errors.push(`${label} must be an integer >= ${minimum}`);
  }
}

function requireBoolean(value, label, errors) {
  if (typeof value !== "boolean") {
    errors.push(`${label} must be a boolean`);
  }
}

function requireSha256(value, label, errors) {
  if (typeof value !== "string" || !SHA256.test(value)) {
    errors.push(`${label} must be a lowercase SHA-256 digest`);
  }
}

function requireGitObjectId(value, label, errors) {
  if (typeof value !== "string" || !GIT_OBJECT_ID.test(value)) {
    errors.push(`${label} must be a lowercase Git object ID`);
  }
}

function requireStringArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return [];
  }
  if (new Set(value).size !== value.length) {
    errors.push(`${label} must not contain duplicates`);
  }
  return value;
}

function checkEqual(actual, expected, label, errors) {
  if (actual !== expected) {
    errors.push(`${label} must equal ${JSON.stringify(expected)}`);
  }
}

function validateExecutionStateMachine(value, label, errors) {
  const stateMachine = requireRecord(value, label, errors);
  for (const scenario of ["success", "cancellation", "watchdog"]) {
    const outcome = requireRecord(
      stateMachine[scenario],
      `${label}.${scenario}`,
      errors,
    );
    checkEqual(
      outcome.finalState,
      "stopped",
      `${label}.${scenario}.finalState`,
      errors,
    );
    requireInteger(
      outcome.transitionCount,
      `${label}.${scenario}.transitionCount`,
      errors,
      1,
    );
    checkEqual(
      outcome.lastTransitionTo,
      "stopped",
      `${label}.${scenario}.lastTransitionTo`,
      errors,
    );
    checkEqual(
      outcome.pendingHostRequestRecords,
      0,
      `${label}.${scenario}.pendingHostRequestRecords`,
      errors,
    );
    if (scenario !== "watchdog") {
      checkEqual(
        outcome.watchdogContext,
        null,
        `${label}.${scenario}.watchdogContext`,
        errors,
      );
      continue;
    }
    const context = requireRecord(
      outcome.watchdogContext,
      `${label}.watchdog.watchdogContext`,
      errors,
    );
    requireInteger(
      context.threadId,
      `${label}.watchdog.watchdogContext.threadId`,
      errors,
      1,
    );
    if (
      context.requestId !== null &&
      (typeof context.requestId !== "string" || !/^(0|[1-9][0-9]*)$/u.test(context.requestId))
    ) {
      errors.push(`${label}.watchdog.watchdogContext.requestId must be null or a decimal ID`);
    }
    const transition = requireRecord(
      context.lastTransition,
      `${label}.watchdog.watchdogContext.lastTransition`,
      errors,
    );
    for (const field of ["from", "to", "cause"]) {
      requireString(
        transition[field],
        `${label}.watchdog.watchdogContext.lastTransition.${field}`,
        errors,
      );
    }
  }
}

function digestPayload(evidence) {
  const payload = structuredClone(evidence);
  delete payload.payloadSha256;
  return crypto
    .createHash("sha256")
    .update(`${JSON.stringify(payload, null, 2)}\n`)
    .digest("hex");
}

export function finalizePrivateTargetEvidence(payload) {
  const evidence = structuredClone(payload);
  evidence.payloadSha256 = digestPayload(evidence);
  return evidence;
}

export function validatePrivateTargetEvidence(evidence, checkpoint) {
  const errors = [];
  const root = requireRecord(evidence, "evidence", errors);
  checkEqual(root.schema, PRIVATE_TARGET_EVIDENCE_SCHEMA, "evidence.schema", errors);
  checkEqual(
    root.schemaVersion,
    PRIVATE_TARGET_EVIDENCE_VERSION,
    "evidence.schemaVersion",
    errors,
  );
  checkEqual(root.status, "passed", "evidence.status", errors);
  requireString(root.recordedAt, "evidence.recordedAt", errors);
  if (typeof root.recordedAt === "string" && !Number.isFinite(Date.parse(root.recordedAt))) {
    errors.push("evidence.recordedAt must be an ISO timestamp");
  }

  const repository = requireRecord(root.repository, "evidence.repository", errors);
  requireGitObjectId(
    repository.implementationCommit,
    "evidence.repository.implementationCommit",
    errors,
  );
  requireGitObjectId(
    repository.implementationTree,
    "evidence.repository.implementationTree",
    errors,
  );
  requireGitObjectId(repository.headCommit, "evidence.repository.headCommit", errors);
  checkEqual(
    repository.headCommit,
    repository.implementationCommit,
    "evidence.repository.headCommit",
    errors,
  );
  requireBoolean(repository.dirty, "evidence.repository.dirty", errors);
  const dirtyPaths = requireStringArray(
    repository.dirtyPaths,
    "evidence.repository.dirtyPaths",
    errors,
  );
  const allowedDirtyPaths = requireStringArray(
    repository.allowedDirtyPaths,
    "evidence.repository.allowedDirtyPaths",
    errors,
  );
  if (allowedDirtyPaths.some((entry) => !dirtyPaths.includes(entry))) {
    errors.push("evidence.repository.allowedDirtyPaths must be a subset of dirtyPaths");
  }
  checkEqual(
    repository.dirty,
    dirtyPaths.length > 0,
    "evidence.repository.dirty",
    errors,
  );
  checkEqual(
    repository.executionInputsClean,
    true,
    "evidence.repository.executionInputsClean",
    errors,
  );
  const disallowedDirtyPaths = requireStringArray(
    repository.disallowedDirtyPaths,
    "evidence.repository.disallowedDirtyPaths",
    errors,
  );
  if (disallowedDirtyPaths.length !== 0) {
    errors.push("evidence.repository.disallowedDirtyPaths must be empty");
  }

  const command = requireRecord(root.command, "evidence.command", errors);
  checkEqual(
    command.canonical,
    "npm run check:private-target",
    "evidence.command.canonical",
    errors,
  );
  requireString(command.invocation, "evidence.command.invocation", errors);
  checkEqual(
    command.semanticGate,
    "required",
    "evidence.command.semanticGate",
    errors,
  );
  checkEqual(
    command.performanceGate,
    "separate-observation",
    "evidence.command.performanceGate",
    errors,
  );

  const source = requireRecord(root.source, "evidence.source", errors);
  requireString(source.fileName, "evidence.source.fileName", errors);
  requireInteger(source.bytes, "evidence.source.bytes", errors, 1);
  requireSha256(source.sha256, "evidence.source.sha256", errors);

  const expectedBrowser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  checkEqual(source.bytes, expectedBrowser.sourceBytes, "evidence.source.bytes", errors);
  checkEqual(source.sha256, expectedBrowser.sourceSha256, "evidence.source.sha256", errors);

  const profile = requireRecord(root.profile, "evidence.profile", errors);
  checkEqual(
    profile.currentCheckpointSchema,
    checkpoint.schema,
    "evidence.profile.currentCheckpointSchema",
    errors,
  );
  checkEqual(
    profile.currentCheckpointSchemaVersion,
    checkpoint.schemaVersion,
    "evidence.profile.currentCheckpointSchemaVersion",
    errors,
  );
  checkEqual(
    profile.runtimeCheckpoint,
    checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
    "evidence.profile.runtimeCheckpoint",
    errors,
  );
  checkEqual(
    profile.runtimeProfile,
    checkpoint.activeRuntimeCheckpoint.profile,
    "evidence.profile.runtimeProfile",
    errors,
  );
  const policy = checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
  checkEqual(profile.policySchema, policy.schema, "evidence.profile.policySchema", errors);
  checkEqual(
    profile.policySchemaVersion,
    policy.schemaVersion,
    "evidence.profile.policySchemaVersion",
    errors,
  );
  checkEqual(profile.policyId, policy.id, "evidence.profile.policyId", errors);
  checkEqual(profile.policySource, policy.source, "evidence.profile.policySource", errors);
  requireString(profile.nodeReportSchema, "evidence.profile.nodeReportSchema", errors);
  requireInteger(
    profile.nodeReportSchemaVersion,
    "evidence.profile.nodeReportSchemaVersion",
    errors,
    1,
  );
  checkEqual(
    profile.browserRuntimeProtocol,
    checkpoint.browserCheckpoint.browserRuntimeProtocol,
    "evidence.profile.browserRuntimeProtocol",
    errors,
  );
  checkEqual(
    profile.executionEngineProfile,
    checkpoint.browserCheckpoint.executionEngineProfile,
    "evidence.profile.executionEngineProfile",
    errors,
  );

  const environment = requireRecord(root.environment, "evidence.environment", errors);
  for (const field of ["platform", "osRelease", "architecture", "node", "rustc", "cargo", "chrome"] ) {
    requireString(environment[field], `evidence.environment.${field}`, errors);
  }
  const llvm = requireRecord(environment.llvm, "evidence.environment.llvm", errors);
  for (const field of ["clang", "wasmClang", "lldLink", "llvmReadobj"]) {
    requireString(llvm[field], `evidence.environment.llvm.${field}`, errors);
  }
  const executionEngine = requireRecord(
    environment.executionEngine,
    "evidence.environment.executionEngine",
    errors,
  );
  requireString(
    executionEngine.engine,
    "evidence.environment.executionEngine.engine",
    errors,
  );
  requireString(
    executionEngine.sourceRevision,
    "evidence.environment.executionEngine.sourceRevision",
    errors,
  );
  const cpuContextAbi = requireRecord(
    executionEngine.cpuContextAbi,
    "evidence.environment.executionEngine.cpuContextAbi",
    errors,
  );
  requireInteger(
    cpuContextAbi.version,
    "evidence.environment.executionEngine.cpuContextAbi.version",
    errors,
    1,
  );
  requireInteger(
    cpuContextAbi.bytes,
    "evidence.environment.executionEngine.cpuContextAbi.bytes",
    errors,
    1,
  );
  const adapterArtifact = requireRecord(
    executionEngine.adapterArtifact,
    "evidence.environment.executionEngine.adapterArtifact",
    errors,
  );
  requireInteger(
    adapterArtifact.bytes,
    "evidence.environment.executionEngine.adapterArtifact.bytes",
    errors,
    1,
  );
  requireSha256(
    adapterArtifact.sha256,
    "evidence.environment.executionEngine.adapterArtifact.sha256",
    errors,
  );

  const semantic = requireRecord(root.semantic, "evidence.semantic", errors);
  checkEqual(semantic.outcome, "passed", "evidence.semantic.outcome", errors);
  const node = requireRecord(semantic.node, "evidence.semantic.node", errors);
  const browser = requireRecord(semantic.browser, "evidence.semantic.browser", errors);
  checkEqual(node.outcome, "structured-stop", "evidence.semantic.node.outcome", errors);
  checkEqual(
    browser.outcome,
    "structured-stop",
    "evidence.semantic.browser.outcome",
    errors,
  );
  for (const [label, result] of [
    ["node", node],
    ["browser", browser],
  ]) {
    checkEqual(
      result.supportedCalls,
      checkpoint.activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount,
      `evidence.semantic.${label}.supportedCalls`,
      errors,
    );
    checkEqual(
      result.instructions,
      checkpoint.nextStructuredStop.instructionCount,
      `evidence.semantic.${label}.instructions`,
      errors,
    );
    checkEqual(
      result.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
      `evidence.semantic.${label}.memoryContentSha256`,
      errors,
    );
  }
  checkEqual(
    node.implementedApiContracts,
    checkpoint.activeRuntimeCheckpoint.genericDispatcherApiCount,
    "evidence.semantic.node.implementedApiContracts",
    errors,
  );
  checkEqual(
    node.finalGeneration,
    expectedBrowser.nodeFinalGeneration,
    "evidence.semantic.node.finalGeneration",
    errors,
  );
  checkEqual(
    browser.finalGeneration,
    expectedBrowser.finalGeneration,
    "evidence.semantic.browser.finalGeneration",
    errors,
  );
  validateExecutionStateMachine(
    browser.executionStateMachine,
    "evidence.semantic.browser.executionStateMachine",
    errors,
  );
  const browserEngine = requireRecord(
    browser.executionEngine,
    "evidence.semantic.browser.executionEngine",
    errors,
  );
  checkEqual(
    browserEngine.schema,
    "vine.execution-engine-profile",
    "evidence.semantic.browser.executionEngine.schema",
    errors,
  );
  checkEqual(
    browserEngine.schemaVersion,
    1,
    "evidence.semantic.browser.executionEngine.schemaVersion",
    errors,
  );
  checkEqual(
    browserEngine.engine,
    executionEngine.engine,
    "evidence.semantic.browser.executionEngine.engine",
    errors,
  );
  checkEqual(
    browserEngine.sourceRevision,
    executionEngine.sourceRevision,
    "evidence.semantic.browser.executionEngine.sourceRevision",
    errors,
  );
  checkEqual(
    browserEngine.cpuContextAbi?.version,
    cpuContextAbi.version,
    "evidence.semantic.browser.executionEngine.cpuContextAbi.version",
    errors,
  );
  checkEqual(
    browserEngine.cpuContextAbi?.bytes,
    cpuContextAbi.bytes,
    "evidence.semantic.browser.executionEngine.cpuContextAbi.bytes",
    errors,
  );
  checkEqual(
    browserEngine.artifact?.bytes,
    adapterArtifact.bytes,
    "evidence.semantic.browser.executionEngine.artifact.bytes",
    errors,
  );
  checkEqual(
    browserEngine.artifact?.sha256,
    adapterArtifact.sha256,
    "evidence.semantic.browser.executionEngine.artifact.sha256",
    errors,
  );
  checkEqual(
    browser.executionEngineLinearMemoryBytes,
    expectedBrowser.executionEngineLinearMemoryBytes,
    "evidence.semantic.browser.executionEngineLinearMemoryBytes",
    errors,
  );
  checkEqual(
    semantic.memoryEquivalent,
    true,
    "evidence.semantic.memoryEquivalent",
    errors,
  );

  const next = requireRecord(
    semantic.nextStructuredStop,
    "evidence.semantic.nextStructuredStop",
    errors,
  );
  for (const field of [
    "api",
    "capabilityCluster",
    "endpointKind",
    "endpointId",
    "endpointAddress",
    "iatAddress",
    "returnAddress",
    "reason",
  ]) {
    checkEqual(
      next[field],
      checkpoint.nextStructuredStop[field],
      `evidence.semantic.nextStructuredStop.${field}`,
      errors,
    );
  }

  const cleanup = requireRecord(semantic.cleanup, "evidence.semantic.cleanup", errors);
  for (const field of [
    "pendingHostCalls",
    "pendingCallbacks",
    "cancelPendingHostCalls",
    "cancelPendingCallbacks",
    "watchdogPendingHostCalls",
    "watchdogPendingCallbacks",
  ]) {
    checkEqual(cleanup[field], 0, `evidence.semantic.cleanup.${field}`, errors);
  }
  checkEqual(
    cleanup.cancelInstructions,
    expectedBrowser.cancelInstructions,
    "evidence.semantic.cleanup.cancelInstructions",
    errors,
  );
  checkEqual(
    cleanup.watchdogInstructions,
    expectedBrowser.watchdogInstructions,
    "evidence.semantic.cleanup.watchdogInstructions",
    errors,
  );

  const artifacts = requireRecord(root.artifacts, "evidence.artifacts", errors);
  for (const name of [
    "nodeReport",
    "browserReport",
    "toolchainReport",
    "v86AdapterReport",
    "browserBuildManifest",
    "runtimeCoreWasm",
    "v86AdapterWasm",
    "cargoLock",
    "npmLock",
  ]) {
    const artifact = requireRecord(artifacts[name], `evidence.artifacts.${name}`, errors);
    requireString(artifact.path, `evidence.artifacts.${name}.path`, errors);
    requireInteger(artifact.bytes, `evidence.artifacts.${name}.bytes`, errors, 1);
    requireSha256(artifact.sha256, `evidence.artifacts.${name}.sha256`, errors);
  }
  checkEqual(
    artifacts.runtimeCoreWasm.bytes,
    checkpoint.browserCheckpoint.browserRuntimeCoreWasm.bytes,
    "evidence.artifacts.runtimeCoreWasm.bytes",
    errors,
  );
  checkEqual(
    artifacts.runtimeCoreWasm.sha256,
    checkpoint.browserCheckpoint.browserRuntimeCoreWasm.sha256,
    "evidence.artifacts.runtimeCoreWasm.sha256",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.bytes,
    adapterArtifact.bytes,
    "evidence.artifacts.v86AdapterWasm.bytes",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.sha256,
    adapterArtifact.sha256,
    "evidence.artifacts.v86AdapterWasm.sha256",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.sha256,
    checkpoint.browserCheckpoint.bgiBenchmarkEvidence.v86AdapterWasmSha256,
    "evidence.artifacts.v86AdapterWasm checkpoint digest",
    errors,
  );

  const performance = requireRecord(
    root.performanceObservation,
    "evidence.performanceObservation",
    errors,
  );
  checkEqual(
    performance.classification,
    "observation-not-semantic-gate",
    "evidence.performanceObservation.classification",
    errors,
  );
  checkEqual(
    performance.status,
    checkpoint.browserCheckpoint.bgiBenchmark,
    "evidence.performanceObservation.status",
    errors,
  );
  checkEqual(
    performance.configuration,
    checkpoint.browserCheckpoint.bgiBenchmarkEvidence.configuration,
    "evidence.performanceObservation.configuration",
    errors,
  );
  const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
  checkEqual(
    performance.benchmarkClassification,
    benchmark.performanceClassification,
    "evidence.performanceObservation.benchmarkClassification",
    errors,
  );
  checkEqual(
    performance.decisionStatus,
    benchmark.decisionStatus,
    "evidence.performanceObservation.decisionStatus",
    errors,
  );
  checkEqual(
    performance.decisionReason,
    benchmark.decisionReason,
    "evidence.performanceObservation.decisionReason",
    errors,
  );
  checkEqual(
    performance.thresholdEvaluationCount,
    benchmark.thresholdEvaluationCount,
    "evidence.performanceObservation.thresholdEvaluationCount",
    errors,
  );
  requireInteger(
    performance.durationMilliseconds,
    "evidence.performanceObservation.durationMilliseconds",
    errors,
    1,
  );
  requireInteger(
    performance.completedSessions,
    "evidence.performanceObservation.completedSessions",
    errors,
    1,
  );
  requireInteger(
    performance.failedSessions,
    "evidence.performanceObservation.failedSessions",
    errors,
  );
  for (const field of [
    "durationMilliseconds",
    "completedSessions",
    "failedSessions",
    "reportSha256",
  ]) {
    checkEqual(
      performance[field],
      benchmark[field],
      `evidence.performanceObservation.${field}`,
      errors,
    );
  }
  const thresholdFailureIds = requireStringArray(
    performance.thresholdFailureIds,
    "evidence.performanceObservation.thresholdFailureIds",
    errors,
  );
  checkEqual(
    thresholdFailureIds.length,
    benchmark.thresholdFailures,
    "evidence.performanceObservation.thresholdFailureIds.length",
    errors,
  );
  checkEqual(
    JSON.stringify(thresholdFailureIds),
    JSON.stringify(benchmark.thresholdFailureIds),
    "evidence.performanceObservation.thresholdFailureIds",
    errors,
  );
  requireSha256(
    performance.reportSha256,
    "evidence.performanceObservation.reportSha256",
    errors,
  );

  requireSha256(root.payloadSha256, "evidence.payloadSha256", errors);
  if (typeof root.payloadSha256 === "string" && SHA256.test(root.payloadSha256)) {
    checkEqual(
      root.payloadSha256,
      digestPayload(root),
      "evidence.payloadSha256",
      errors,
    );
  }
  return errors;
}

export function assertValidPrivateTargetEvidence(evidence, checkpoint) {
  const errors = validatePrivateTargetEvidence(evidence, checkpoint);
  if (errors.length > 0) {
    throw new Error(`private target evidence is invalid:\n- ${errors.join("\n- ")}`);
  }
  return evidence;
}
