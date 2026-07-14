import fs from "node:fs";

export const README_STATUS_START = "<!-- BEGIN GENERATED CURRENT STATUS -->";
export const README_STATUS_END = "<!-- END GENERATED CURRENT STATUS -->";

const SHA256 = /^[0-9a-f]{64}$/;
const HEX32 = /^0x[0-9a-f]{1,8}$/;

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

function requireFiniteNumber(value, label, errors) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    errors.push(`${label} must be a finite number`);
  }
}

function requireSha256(value, label, errors) {
  if (typeof value !== "string" || !SHA256.test(value)) {
    errors.push(`${label} must be a lowercase SHA-256 digest`);
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

function requireOrderedStringArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return [];
  }
  return value;
}

function validateExecutionStateMachine(value, label, errors) {
  const stateMachine = requireRecord(value, label, errors);
  for (const scenario of ["success", "cancellation", "watchdog"]) {
    const outcome = requireRecord(
      stateMachine[scenario],
      `${label}.${scenario}`,
      errors,
    );
    if (outcome.finalState !== "stopped") {
      errors.push(`${label}.${scenario}.finalState must be stopped`);
    }
    requireInteger(
      outcome.transitionCount,
      `${label}.${scenario}.transitionCount`,
      errors,
      1,
    );
    if (outcome.lastTransitionTo !== "stopped") {
      errors.push(`${label}.${scenario}.lastTransitionTo must be stopped`);
    }
    requireInteger(
      outcome.pendingHostRequestRecords,
      `${label}.${scenario}.pendingHostRequestRecords`,
      errors,
    );
    if (outcome.pendingHostRequestRecords !== 0) {
      errors.push(`${label}.${scenario} must not retain host request records`);
    }
    if (scenario !== "watchdog") {
      if (outcome.watchdogContext !== null) {
        errors.push(`${label}.${scenario}.watchdogContext must be null`);
      }
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

export function validateCurrentCheckpoint(manifest) {
  const errors = [];
  const root = requireRecord(manifest, "manifest", errors);
  const expectedRootKeys = [
    "schema",
    "schemaVersion",
    "historicalBaseline",
    "memoryAuthorityEvidence",
    "browserCheckpoint",
    "engineDecisionEvidence",
    "activeRuntimeCheckpoint",
    "nextStructuredStop",
  ];
  const rootKeys = Object.keys(root);
  if (
    rootKeys.length !== expectedRootKeys.length ||
    expectedRootKeys.some((key) => !rootKeys.includes(key))
  ) {
    errors.push(
      `manifest root keys must be exactly ${expectedRootKeys.join(", ")}`,
    );
  }
  if (root.schema !== "vine.current-checkpoint") {
    errors.push("manifest.schema must be vine.current-checkpoint");
  }
  if (root.schemaVersion !== 2) {
    errors.push("manifest.schemaVersion must be 2");
  }

  const historical = requireRecord(
    root.historicalBaseline,
    "historicalBaseline",
    errors,
  );
  requireString(historical.checkpoint, "historicalBaseline.checkpoint", errors);
  requireInteger(
    historical.supportedCallCount,
    "historicalBaseline.supportedCallCount",
    errors,
  );
  requireInteger(
    historical.entryInstructionCount,
    "historicalBaseline.entryInstructionCount",
    errors,
  );
  requireString(
    historical.checkpointManifest,
    "historicalBaseline.checkpointManifest",
    errors,
  );

  requireRecord(root.memoryAuthorityEvidence, "memoryAuthorityEvidence", errors);

  const browser = requireRecord(root.browserCheckpoint, "browserCheckpoint", errors);
  if (browser.status !== "passed") {
    errors.push("browserCheckpoint.status must be passed");
  }
  if (browser.bgiSession !== "semantic-equivalence-passed") {
    errors.push("browserCheckpoint.bgiSession must identify passed semantic equivalence");
  }
  if (!new Set(["threshold-passed", "threshold-blocked"]).has(browser.bgiBenchmark)) {
    errors.push("browserCheckpoint.bgiBenchmark must identify a threshold gate outcome");
  }
  if (browser.browserRuntimeProtocol !== "vine.browser-runtime-worker.v4") {
    errors.push("browserCheckpoint.browserRuntimeProtocol must identify protocol v4");
  }
  if (
    browser.executionEngineProfile !==
    "fixtures/execution-engines/v86-browser-v1.json"
  ) {
    errors.push("browserCheckpoint.executionEngineProfile must identify the pinned adapter");
  }
  requireString(browser.workload, "browserCheckpoint.workload", errors);
  const browserEvidence = requireRecord(
    browser.browserRuntimeEvidence,
    "browserCheckpoint.browserRuntimeEvidence",
    errors,
  );
  requireInteger(
    browserEvidence.guestInstructions,
    "browserCheckpoint.browserRuntimeEvidence.guestInstructions",
    errors,
  );
  if (browserEvidence.rustSchedulerTransitions !== browserEvidence.hostRequests) {
    errors.push("browser scheduler transitions must authorize every completed host request");
  }
  requireInteger(
    browserEvidence.executionEngineLinearMemoryBytes,
    "browserCheckpoint.browserRuntimeEvidence.executionEngineLinearMemoryBytes",
    errors,
    1,
  );
  validateExecutionStateMachine(
    browserEvidence.executionStateMachine,
    "browserCheckpoint.browserRuntimeEvidence.executionStateMachine",
    errors,
  );
  const mappingOperations = requireRecord(
    browserEvidence.mappingOperations,
    "browserCheckpoint.browserRuntimeEvidence.mappingOperations",
    errors,
  );
  const mappingVariants = ["reserve", "commit", "protect", "decommit", "release"];
  for (const variant of mappingVariants) {
    requireInteger(
      mappingOperations[variant],
      `browserCheckpoint.browserRuntimeEvidence.mappingOperations.${variant}`,
      errors,
    );
  }
  const mappingCacheEffects = requireRecord(
    browserEvidence.mappingCacheEffects,
    "browserCheckpoint.browserRuntimeEvidence.mappingCacheEffects",
    errors,
  );
  if (
    mappingVariants.some((variant) => mappingOperations[variant] < 1) ||
    browserEvidence.mappingDeltas < 1 ||
    browserEvidence.mappingDirtyPages < 1 ||
    browserEvidence.mappingAtomicRejections < 1 ||
    browserEvidence.mappingStaleAccessStops < 2 ||
    browserEvidence.mirrorRebuilds < 1 ||
    mappingCacheEffects.tlbFlushes < 1 ||
    mappingCacheEffects.instructionFetchInvalidations < 1 ||
    mappingCacheEffects.generatedCodeInvalidations < 1
  ) {
    errors.push("browser mapping-operation evidence is incomplete");
  }
  const bgiBrowserEvidence = requireRecord(
    browser.bgiRuntimeEvidence,
    "browserCheckpoint.bgiRuntimeEvidence",
    errors,
  );
  for (const field of [
    "guestInstructions",
    "cpuSlices",
    "budgetYields",
    "supportedCalls",
    "writebackBatches",
    "writebackPages",
    "hostDeltaBatches",
    "initialGeneration",
    "finalGeneration",
    "nodeFinalGeneration",
    "wasmBridgeCalls",
    "executionEngineLinearMemoryBytes",
    "heartbeatSamples",
    "cancelInstructions",
    "watchdogInstructions",
    "pendingHostCallsAfterCleanup",
    "pendingCallbacksAfterCleanup",
    "nodeOracleReportBytes",
    "browserReportBytes",
  ]) {
    requireInteger(
      bgiBrowserEvidence[field],
      `browserCheckpoint.bgiRuntimeEvidence.${field}`,
      errors,
    );
  }
  const bgiMappingOperations = requireRecord(
    bgiBrowserEvidence.mappingOperations,
    "browserCheckpoint.bgiRuntimeEvidence.mappingOperations",
    errors,
  );
  for (const variant of mappingVariants) {
    requireInteger(
      bgiMappingOperations[variant],
      `browserCheckpoint.bgiRuntimeEvidence.mappingOperations.${variant}`,
      errors,
    );
  }
  validateExecutionStateMachine(
    bgiBrowserEvidence.executionStateMachine,
    "browserCheckpoint.bgiRuntimeEvidence.executionStateMachine",
    errors,
  );
  for (const field of [
    "sourceSha256",
    "memoryContentSha256",
    "nodeOracleReportSha256",
    "browserReportSha256",
  ]) {
    requireSha256(
      bgiBrowserEvidence[field],
      `browserCheckpoint.bgiRuntimeEvidence.${field}`,
      errors,
    );
  }
  if (
    bgiBrowserEvidence.nodeRole !== "semantic-oracle-only" ||
    bgiBrowserEvidence.runtimeStateAuthority !== "rust-wasm-in-runtime-worker" ||
    bgiMappingOperations.reserve !== 1 ||
    bgiMappingOperations.commit < 1 ||
    bgiBrowserEvidence.pendingHostCallsAfterCleanup !== 0 ||
    bgiBrowserEvidence.pendingCallbacksAfterCleanup !== 0 ||
    bgiBrowserEvidence.processLifecycleAfterCleanup !== "exited" ||
    bgiBrowserEvidence.dllLifecycleAfterCleanup !== "process-detached"
  ) {
    errors.push("browser BGI authority or teardown evidence is invalid");
  }
  const bgiBenchmarkEvidence = requireRecord(
    browser.bgiBenchmarkEvidence,
    "browserCheckpoint.bgiBenchmarkEvidence",
    errors,
  );
  requireString(
    bgiBenchmarkEvidence.chromeVersion,
    "browserCheckpoint.bgiBenchmarkEvidence.chromeVersion",
    errors,
  );
  for (const field of [
    "performanceClassification",
    "decisionStatus",
    "decisionReason",
  ]) {
    requireString(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  const benchmarkFailureIds = requireStringArray(
    bgiBenchmarkEvidence.thresholdFailureIds,
    "browserCheckpoint.bgiBenchmarkEvidence.thresholdFailureIds",
    errors,
  );
  for (const field of [
    "schemaVersion",
    "durationMilliseconds",
    "completedSessions",
    "failedSessions",
    "mainHeapSamples",
    "workerHeapSamples",
    "mainHeapMaximumBytes",
    "workerHeapMaximumBytes",
    "workerHeapCapacityMaximumBytes",
    "workerBackingStorageMaximumBytes",
    "wasmBridgeCallsPerSession",
    "runtimeCoreLinearMemoryBytes",
    "executionEngineLinearMemoryBytes",
    "thresholdFailures",
    "thresholdEvaluationCount",
    "runtimeWorkerBytes",
    "benchmarkMainBytes",
    "v86AdapterWasmBytes",
    "reportBytes",
  ]) {
    requireInteger(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  for (const field of [
    "sessionP95Milliseconds",
    "sliceP99Milliseconds",
    "heartbeatP99Milliseconds",
    "maximumHeartbeatGapMilliseconds",
    "mainHeapSlopeBytesPerMinute",
    "workerHeapSlopeBytesPerMinute",
    "workerBackingStorageSlopeBytesPerMinute",
  ]) {
    requireFiniteNumber(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  for (const field of [
    "configurationSha256",
    "runtimeCoreWasmSha256",
    "runtimeWorkerSha256",
    "benchmarkMainSha256",
    "v86AdapterWasmSha256",
    "reportSha256",
  ]) {
    requireSha256(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  if (
    bgiBenchmarkEvidence.schema !== "vine.browser-bgi-benchmark-report" ||
    bgiBenchmarkEvidence.schemaVersion !== 2 ||
    bgiBenchmarkEvidence.configuration !== "fixtures/benchmarks/bgi-browser-v1.json" ||
    bgiBenchmarkEvidence.performanceClassification !== "repeated-distribution-threshold" ||
    bgiBenchmarkEvidence.decisionStatus !==
      (benchmarkFailureIds.length === 0 ? "passed" : "failed") ||
    bgiBenchmarkEvidence.decisionReason !==
      (benchmarkFailureIds.length === 0
        ? `all-${bgiBenchmarkEvidence.thresholdEvaluationCount}-versioned-thresholds-passed`
        : `${benchmarkFailureIds.length}-of-${bgiBenchmarkEvidence.thresholdEvaluationCount}-versioned-thresholds-failed`) ||
    bgiBenchmarkEvidence.thresholdEvaluationCount < 1 ||
    bgiBenchmarkEvidence.completedSessions < 1 ||
    bgiBenchmarkEvidence.failedSessions !== 0 ||
    bgiBenchmarkEvidence.mainHeapSamples < 2 ||
    bgiBenchmarkEvidence.workerHeapSamples < 2 ||
    bgiBenchmarkEvidence.thresholdFailures !== benchmarkFailureIds.length ||
    (browser.bgiBenchmark === "threshold-passed" && benchmarkFailureIds.length !== 0) ||
    (browser.bgiBenchmark === "threshold-blocked" && benchmarkFailureIds.length === 0) ||
    bgiBenchmarkEvidence.wasmBridgeCallsPerSession !== bgiBrowserEvidence.wasmBridgeCalls ||
    bgiBenchmarkEvidence.executionEngineLinearMemoryBytes !==
      bgiBrowserEvidence.executionEngineLinearMemoryBytes ||
    bgiBenchmarkEvidence.runtimeCoreWasmSha256 !== browser.browserRuntimeCoreWasm?.sha256
  ) {
    errors.push("browser BGI benchmark evidence is invalid or inconsistent");
  }

  const engine = requireRecord(
    root.engineDecisionEvidence,
    "engineDecisionEvidence",
    errors,
  );
  const engineGate = requireRecord(
    engine.v86EngineGate,
    "engineDecisionEvidence.v86EngineGate",
    errors,
  );
  if (!new Set(["go", "no-go"]).has(engineGate.decision)) {
    errors.push("engineDecisionEvidence.v86EngineGate.decision is invalid");
  }
  requireInteger(
    engineGate.thresholdFailures,
    "engineDecisionEvidence.v86EngineGate.thresholdFailures",
    errors,
  );
  if (
    engineGate.performanceClassification !== "repeated-distribution-threshold" ||
    engineGate.reportHashClassification !== "artifact-identity-only" ||
    engineGate.decisionReason !==
      (engineGate.thresholdFailures === 0
        ? "all-versioned-thresholds-passed"
        : `${engineGate.thresholdFailures}-versioned-thresholds-failed`) ||
    (engineGate.decision === "go" && engineGate.thresholdFailures !== 0) ||
    (engineGate.decision === "no-go" && engineGate.thresholdFailures === 0)
  ) {
    errors.push("engineDecisionEvidence.v86EngineGate classification is invalid");
  }
  requireSha256(
    engineGate.reportSha256,
    "engineDecisionEvidence.v86EngineGate.reportSha256",
    errors,
  );

  const active = requireRecord(
    root.activeRuntimeCheckpoint,
    "activeRuntimeCheckpoint",
    errors,
  );
  requireString(
    active.runtimeKernelCheckpoint,
    "activeRuntimeCheckpoint.runtimeKernelCheckpoint",
    errors,
  );
  requireString(active.profile, "activeRuntimeCheckpoint.profile", errors);
  if (active.status !== "structured-stop") {
    errors.push("activeRuntimeCheckpoint.status must be structured-stop");
  }
  if (active.runtimeStateAuthority !== "rust") {
    errors.push("activeRuntimeCheckpoint.runtimeStateAuthority must be rust");
  }
  const APIs = requireStringArray(
    active.genericDispatcherApis,
    "activeRuntimeCheckpoint.genericDispatcherApis",
    errors,
  );
  requireInteger(
    active.genericDispatcherApiCount,
    "activeRuntimeCheckpoint.genericDispatcherApiCount",
    errors,
  );
  if (active.genericDispatcherApiCount !== APIs.length) {
    errors.push(
      "activeRuntimeCheckpoint.genericDispatcherApiCount must match genericDispatcherApis",
    );
  }
  requireInteger(
    active.genericDispatcherCallOccurrenceCount,
    "activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount",
    errors,
  );
  if (active.genericDispatcherCallOccurrenceCount < APIs.length) {
    errors.push(
      "activeRuntimeCheckpoint call occurrences must cover every API contract",
    );
  }
  if (active.allSupportedCallsUseGenericDispatcher !== true) {
    errors.push(
      "activeRuntimeCheckpoint.allSupportedCallsUseGenericDispatcher must be true",
    );
  }
  const objects = requireRecord(
    active.runtimeObjectModel,
    "activeRuntimeCheckpoint.runtimeObjectModel",
    errors,
  );
  if (
    objects.schema !== "vine.runtime-object-table.v2" ||
    objects.stateOwner !== "runtime/objects.rs::RuntimeObjectTable" ||
    objects.mutableCollectionVisibility !== "private" ||
    objects.readInspection !== "immutable-borrow" ||
    objects.crossSubsystemMutation !== "plan-preflight-commit" ||
    objects.subsystemTests < 3 ||
    objects.crossSubsystemAtomicTests < 3 ||
    objects.failedRegistrationPreservesAllocator !== true ||
    objects.aggregateDirectCollectionMutation !== false ||
    objects.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime object subsystem boundary is invalid");
  }
  const threads = requireRecord(
    active.runtimeThreadRegistry,
    "activeRuntimeCheckpoint.runtimeThreadRegistry",
    errors,
  );
  if (
    threads.schema !== "vine.runtime-thread-registry.v2" ||
    threads.stateOwner !== "runtime/thread.rs::RuntimeThreads" ||
    threads.mutableCollectionVisibility !== "private" ||
    threads.serializedRuntimeField !== "threads" ||
    threads.crossSubsystemMutation !== "thread-plan-aggregate-preflight-commit" ||
    threads.subsystemTests < 3 ||
    threads.crossSubsystemAtomicTests < 3 ||
    threads.directRecordMutationFromAggregate !== false ||
    threads.stackMetadataPlanCommit !== true ||
    !Array.isArray(threads.perThreadState) ||
    threads.perThreadState.join(",") !==
      "last-error,tls-values,fls-values,callback-frames,stack-metadata" ||
    threads.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime thread subsystem boundary is invalid");
  }
  const scheduler = requireRecord(
    active.runtimeScheduler,
    "activeRuntimeCheckpoint.runtimeScheduler",
    errors,
  );
  if (
    scheduler.schema !== "vine.runtime-scheduler.v1" ||
    scheduler.stateOwner !== "runtime/scheduler.rs::RuntimeScheduler" ||
    scheduler.mutableCollectionVisibility !== "private" ||
    scheduler.serializedRuntimeField !== "scheduler" ||
    !Array.isArray(scheduler.serializedFields) ||
    scheduler.serializedFields.join(",") !==
      "currentThreadId,threads,waitRegistrations,objectWaiters,events" ||
    scheduler.waitSubsystemAdapter !== "serde-flatten-private-runtime-waits" ||
    scheduler.crossSubsystemMutation !==
      "scheduler-plan-aggregate-preflight-commit" ||
    scheduler.subsystemTests < 4 ||
    scheduler.crossSubsystemAtomicTests < 4 ||
    scheduler.directCollectionMutationFromAggregate !== false ||
    scheduler.hostCompletionPreflightBeforeRemoval !== true ||
    scheduler.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime scheduler subsystem boundary is invalid");
  }
  const waits = requireRecord(
    active.runtimeWaitQueue,
    "activeRuntimeCheckpoint.runtimeWaitQueue",
    errors,
  );
  if (
    waits.schema !== "vine.runtime-wait-queue.v2" ||
    waits.stateOwner !== "runtime/waits.rs::RuntimeWaits" ||
    waits.mutableCollectionVisibility !== "private" ||
    waits.readInspection !== "immutable-borrow" ||
    !Array.isArray(waits.serializedSchedulerFields) ||
    waits.serializedSchedulerFields.join(",") !==
      "waitRegistrations,objectWaiters" ||
    waits.crossSubsystemMutation !==
      "wait-plan-aggregate-validation-scheduler-plan-commit" ||
    waits.subsystemTests < 4 ||
    waits.crossSubsystemAtomicTests < 3 ||
    waits.directCollectionMutationFromAggregate !== false ||
    waits.excludedWakeCandidatesValidated !== true ||
    waits.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime wait subsystem boundary is invalid");
  }
  const messages = requireRecord(
    active.runtimeMessageQueue,
    "activeRuntimeCheckpoint.runtimeMessageQueue",
    errors,
  );
  if (
    messages.schema !== "vine.runtime-message-queue.v2" ||
    messages.mutableCollectionVisibility !== "private" ||
    messages.crossSubsystemMutation !== "plan-preflight-commit"
  ) {
    errors.push("active runtime message subsystem boundary is invalid");
  }
  const synchronization = requireRecord(
    active.runtimeSynchronization,
    "activeRuntimeCheckpoint.runtimeSynchronization",
    errors,
  );
  if (
    synchronization.schema !== "vine.runtime-synchronization.v2" ||
    synchronization.stateOwner !== "runtime/synchronization.rs::RuntimeSynchronization" ||
    synchronization.mutableCollectionVisibility !== "private" ||
    !Array.isArray(synchronization.serializedProcessFields) ||
    synchronization.serializedProcessFields.join(",") !== "criticalSections,mutexes" ||
    synchronization.crossSubsystemMutation !==
      "lifecycle-preflight-then-subsystem-plan-commit" ||
    synchronization.subsystemTests < 2 ||
    synchronization.crossSubsystemAtomicTests < 2 ||
    synchronization.criticalSectionLimit !== 4096 ||
    synchronization.mutexLimit !== 4096 ||
    synchronization.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime synchronization subsystem boundary is invalid");
  }
  const addressSpace = requireRecord(
    active.processAddressSpaceAllocator,
    "activeRuntimeCheckpoint.processAddressSpaceAllocator",
    errors,
  );
  if (
    addressSpace.schema !== "vine.process-address-space-allocator.v1" ||
    addressSpace.authority !== "rust-runtime-kernel" ||
    addressSpace.totalAllocationCount !==
      addressSpace.rootAllocationCount + addressSpace.heapSlotAllocationCount ||
    addressSpace.heapBackingUsesAllocationId !== true
  ) {
    errors.push("active process address-space allocator boundary is invalid");
  }
  const kernel32Policy = requireRecord(
    active.kernel32CompatibilityPolicy,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy",
    errors,
  );
  requireString(
    kernel32Policy.source,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source",
    errors,
  );
  if (
    kernel32Policy.schema !== "vine.kernel32-compatibility-policy" ||
    kernel32Policy.schemaVersion !== 1 ||
    kernel32Policy.handlerDispatch !== "single-semantic-implementation-id-with-policy" ||
    kernel32Policy.evidenceSelectsImplementationBranch !== false ||
    kernel32Policy.semanticTestsRequireEvidenceProfile !== false ||
    kernel32Policy.activeAnsiCodePage !== 932 ||
    kernel32Policy.multiByteAllowActiveCodePageSelector !== true ||
    !Array.isArray(kernel32Policy.multiByteAllowedFlags) ||
    kernel32Policy.multiByteAllowedFlags.join(",") !== "0,1" ||
    kernel32Policy.setCurrentDirectoryProcessImageDirectoryOnly !== true ||
    kernel32Policy.setCurrentDirectoryMaximumPathBytes !== 32768 ||
    kernel32Policy.createMutexAllowNamed !== true ||
    kernel32Policy.createMutexAllowUnnamed !== false ||
    kernel32Policy.createMutexMaximumNameBytes !== 32768 ||
    kernel32Policy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active Kernel32 compatibility policy boundary is invalid");
  }
  const latestEvidence = requireRecord(
    active.latestEvidence,
    "activeRuntimeCheckpoint.latestEvidence",
    errors,
  );
  if (latestEvidence.implementedApiContractCount !== APIs.length) {
    errors.push("latest active evidence API count does not match the active summary");
  }
  if (
    latestEvidence.genericSupportedCallCount !==
    active.genericDispatcherCallOccurrenceCount
  ) {
    errors.push("latest active evidence call count does not match the active summary");
  }
  requireInteger(
    latestEvidence.finalMemoryGeneration,
    "activeRuntimeCheckpoint.latestEvidence.finalMemoryGeneration",
    errors,
  );
  requireSha256(
    latestEvidence.reportSha256,
    "activeRuntimeCheckpoint.latestEvidence.reportSha256",
    errors,
  );
  requireString(
    active.canonicalReproductionCommand,
    "activeRuntimeCheckpoint.canonicalReproductionCommand",
    errors,
  );
  if (
    active.canonicalReproductionCommand !== "npm run check:runtime:bgi-browser" ||
    bgiBrowserEvidence.supportedCalls !== active.genericDispatcherCallOccurrenceCount ||
    bgiBrowserEvidence.guestInstructions !== root.nextStructuredStop?.instructionCount
  ) {
    errors.push("browser BGI evidence must match the active runtime checkpoint");
  }

  const next = requireRecord(root.nextStructuredStop, "nextStructuredStop", errors);
  requireString(next.nextCheckpoint, "nextStructuredStop.nextCheckpoint", errors);
  requireInteger(next.instructionCount, "nextStructuredStop.instructionCount", errors);
  requireString(next.capabilityCluster, "nextStructuredStop.capabilityCluster", errors);
  requireString(next.reason, "nextStructuredStop.reason", errors);
  if (next.kind === "api-endpoint") {
    requireString(next.endpointKind, "nextStructuredStop.endpointKind", errors);
    requireInteger(next.endpointId, "nextStructuredStop.endpointId", errors);
    requireString(next.api, "nextStructuredStop.api", errors);
    requireOrderedStringArray(next.arguments, "nextStructuredStop.arguments", errors);
    if (typeof next.endpointAddress !== "string" || !HEX32.test(next.endpointAddress)) {
      errors.push("nextStructuredStop.endpointAddress must be a 32-bit hex string");
    }
    if (typeof next.iatAddress !== "string" || !HEX32.test(next.iatAddress)) {
      errors.push("nextStructuredStop.iatAddress must be a 32-bit hex string");
    }
    if (typeof next.returnAddress !== "string" || !HEX32.test(next.returnAddress)) {
      errors.push("nextStructuredStop.returnAddress must be a 32-bit hex string");
    }
  } else if (next.kind === "execution-fault") {
    if (next.endpoint !== null) {
      errors.push("nextStructuredStop.endpoint must be null for an execution fault");
    }
    const fault = requireRecord(next.fault, "nextStructuredStop.fault", errors);
    for (const field of ["class", "stage", "code"]) {
      requireString(fault[field], `nextStructuredStop.fault.${field}`, errors);
    }
    for (const field of [
      "instructionPointer",
      "previousInstructionPointer",
      "faultAddress",
    ]) {
      if (typeof fault[field] !== "string" || !HEX32.test(fault[field])) {
        errors.push(`nextStructuredStop.fault.${field} must be a 32-bit hex string`);
      }
    }
    requireInteger(
      fault.pageFaultErrorCode,
      "nextStructuredStop.fault.pageFaultErrorCode",
      errors,
    );
    if (
      fault.class !== "fault" ||
      fault.stage !== "guest-execution" ||
      fault.code !== next.reason
    ) {
      errors.push("nextStructuredStop execution fault identity is inconsistent");
    }
  } else {
    errors.push("nextStructuredStop.kind must be api-endpoint or execution-fault");
  }

  if (
    bgiBrowserEvidence.nextCapabilityCluster !== next.capabilityCluster ||
    bgiBrowserEvidence.nextFaultCode !== next.reason
  ) {
    errors.push("browser BGI structured-stop summary must match nextStructuredStop");
  }
  if (next.kind === "execution-fault") {
    const expectedFault = next.fault ?? {};
    if (
      bgiBrowserEvidence.nextEndpoint !== null ||
      bgiBrowserEvidence.nextInstructionPointer !== expectedFault.instructionPointer ||
      bgiBrowserEvidence.nextPreviousInstructionPointer !==
        expectedFault.previousInstructionPointer ||
      bgiBrowserEvidence.nextFaultAddress !== expectedFault.faultAddress ||
      bgiBrowserEvidence.nextPageFaultErrorCode !== expectedFault.pageFaultErrorCode
    ) {
      errors.push("browser BGI execution-fault summary must match nextStructuredStop");
    }
  }

  return errors;
}

export function loadCurrentCheckpoint(file) {
  const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  const errors = validateCurrentCheckpoint(manifest);
  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }
  return manifest;
}

export function renderReadmeStatus(manifest) {
  const historical = manifest.historicalBaseline;
  const active = manifest.activeRuntimeCheckpoint;
  const latest = active.latestEvidence;
  const browser = manifest.browserCheckpoint;
  const engine = manifest.engineDecisionEvidence.v86EngineGate;
  const next = manifest.nextStructuredStop;
  const APIs = active.genericDispatcherApis.map((api) => `- \`${api}\``).join("\n");
  const nextSummary =
    next.kind === "execution-fault"
      ? `\`${next.capabilityCluster}\` at instruction ${next.instructionCount}: ` +
        `\`${next.reason}\` at \`${next.fault.faultAddress}\``
      : `\`${next.api}(${next.arguments.join(", ")})\` at instruction ` +
        `${next.instructionCount}: \`${next.reason}\``;

  return `${README_STATUS_START}
## Current Status

This section is generated from [\`docs/current-checkpoint.json\`](docs/current-checkpoint.json).
Edit the manifest and run \`npm run docs:generate\`; do not edit this block by hand.

| Area | Current state |
|---|---|
| Historical baseline | \`${historical.checkpoint}\`: ${historical.supportedCallCount} calls, ${historical.entryInstructionCount} instructions |
| Active runtime | \`${active.runtimeKernelCheckpoint}\`: ${active.genericDispatcherCallOccurrenceCount} calls across ${active.genericDispatcherApiCount} API contracts, memory generation ${latest.finalMemoryGeneration} |
| Browser | \`${browser.workload}\` ${browser.status}; BGI session \`${browser.bgiSession}\`; benchmark \`${browser.bgiBenchmark}\` (${browser.bgiBenchmarkEvidence.completedSessions} sessions) |
| Engine evidence | v86 \`${engine.decision}\` from \`${engine.configuration}\` |
| Next structured stop | ${nextSummary} |

The active semantic report is ${latest.reportBytes.toLocaleString("en-US")} bytes with SHA-256
\`${latest.reportSha256}\`. The hash identifies the artifact; semantic checks are the gate.

### Supported API Contracts

${APIs}

### Reproduce Current Checkpoint

\`\`\`sh
${active.canonicalReproductionCommand}
\`\`\`
${README_STATUS_END}`;
}

export function replaceReadmeStatus(readme, generated) {
  const start = readme.indexOf(README_STATUS_START);
  const end = readme.indexOf(README_STATUS_END);
  if (start < 0 || end < 0 || end < start) {
    throw new Error("README generated status markers are missing or out of order");
  }
  if (readme.indexOf(README_STATUS_START, start + 1) >= 0) {
    throw new Error("README contains duplicate generated status start markers");
  }
  if (readme.indexOf(README_STATUS_END, end + 1) >= 0) {
    throw new Error("README contains duplicate generated status end markers");
  }
  return `${readme.slice(0, start)}${generated}${readme.slice(
    end + README_STATUS_END.length,
  )}`;
}
