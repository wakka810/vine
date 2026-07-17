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

function requireIntegerArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => !Number.isSafeInteger(item))) {
    errors.push(`${label} must be an array of integers`);
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

  const memoryAuthority = requireRecord(
    root.memoryAuthorityEvidence,
    "memoryAuthorityEvidence",
    errors,
  );
  const mirrorCapacity = requireRecord(
    memoryAuthority.executionMirrorCapacity,
    "memoryAuthorityEvidence.executionMirrorCapacity",
    errors,
  );
  for (const field of [
    "maximumPhysicalPages",
    "maximumGuestBytes",
    "initialPhysicalPoolBytes",
    "maximumPhysicalPoolBytes",
    "maximumPageTablePages",
    "excludedPhysicalRangeBytes",
    "contractExpansionPages",
    "contractExpandedPoolBytes",
  ]) {
    requireInteger(
      mirrorCapacity[field],
      `memoryAuthorityEvidence.executionMirrorCapacity.${field}`,
      errors,
      1,
    );
  }
  if (
    mirrorCapacity.schema !== "vine.v86-physical-pool.v1" ||
    mirrorCapacity.authority !== "rust-prepared-metadata" ||
    mirrorCapacity.maximumPhysicalPages * 4096 !== mirrorCapacity.maximumGuestBytes ||
    mirrorCapacity.initialPhysicalPoolBytes >= mirrorCapacity.contractExpandedPoolBytes ||
    mirrorCapacity.contractExpandedPoolBytes > mirrorCapacity.maximumPhysicalPoolBytes ||
    mirrorCapacity.maximumPageTablePages !== 1024 ||
    mirrorCapacity.growthPolicy !==
      "geometric-adapter-reinstantiation-before-mutation" ||
    mirrorCapacity.releasePolicy !==
      "zero-free-list-reuse-and-empty-page-table-reclamation" ||
    mirrorCapacity.exhaustionCode !== "v86-physical-pool-exhausted" ||
    mirrorCapacity.exhaustionPolicy !== "typed-preflight-before-mutation"
  ) {
    errors.push("execution mirror capacity contract is inconsistent");
  }

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
  if (browser.browserRuntimeProtocol !== "vine.browser-runtime-worker.v5") {
    errors.push("browserCheckpoint.browserRuntimeProtocol must identify protocol v5");
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
  if (
    JSON.stringify(browserEvidence.registeredThreadIds) !== JSON.stringify([8192, 8193]) ||
    JSON.stringify(browserEvidence.executedThreadIds) !== JSON.stringify([8192, 8193]) ||
    browserEvidence.contextSwitches < 2 ||
    !Array.isArray(browserEvidence.hostWaitExecutions) ||
    browserEvidence.hostWaitExecutions.length !== 1 ||
    browserEvidence.hostWaitExecutions[0]?.waitingThreadId !== 8192 ||
    browserEvidence.hostWaitExecutions[0]?.executedThreadId !== 8193 ||
    !Array.isArray(browserEvidence.threadExits) ||
    browserEvidence.threadExits.length !== 1 ||
    browserEvidence.threadExits[0]?.threadId !== 8193 ||
    browserEvidence.threadExits[0]?.exitCode !== 7 ||
    browserEvidence.threadExits[0]?.markerValue !== 0x5448_5244
  ) {
    errors.push("browser multi-thread execution evidence is incomplete");
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
    "largeHeapPageMaterializations",
    "largeHeapAllocationBytes",
    "largeHeapAllocationId",
    "largeHeapAddressAllocationId",
    "largeHeapMemoryOwner",
    "largeHeapFirstFaultInstructionCount",
    "largeHeapFirstPageFaultErrorCode",
    "largeHeapFirstCommittedPageBytes",
    "largeHeapNodeMaterializationFromGeneration",
    "largeHeapNodeMaterializationGeneration",
    "largeHeapBrowserMaterializationFromGeneration",
    "largeHeapBrowserMaterializationGeneration",
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
    "largeHeapMaterializationPayloadSha256",
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
    bgiMappingOperations.reserve !== 2 ||
    bgiMappingOperations.commit < 1 ||
    bgiBrowserEvidence.pendingHostCallsAfterCleanup !== 0 ||
    bgiBrowserEvidence.pendingCallbacksAfterCleanup !== 0 ||
    bgiBrowserEvidence.processLifecycleAfterCleanup !== "exited" ||
    bgiBrowserEvidence.dllLifecycleAfterCleanup !== "process-detached" ||
    bgiBrowserEvidence.largeHeapPageMaterializations !== 1 ||
    bgiBrowserEvidence.largeHeapAllocationAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapAllocationBytes !== 64 * 1024 * 1024 ||
    bgiBrowserEvidence.largeHeapAllocationId !== 12 ||
    bgiBrowserEvidence.largeHeapAddressAllocationId !== 37 ||
    bgiBrowserEvidence.largeHeapMemoryOwner !== 11 ||
    bgiBrowserEvidence.largeHeapFirstFaultInstructionCount !== 50_123 ||
    bgiBrowserEvidence.largeHeapFirstFaultAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapFirstPageFaultErrorCode !== 6 ||
    bgiBrowserEvidence.largeHeapFirstCommittedPageBytes !== 4_096 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationFromGeneration !== 1_470 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationGeneration !== 1_472 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationFromGeneration !== 3_635 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationGeneration !== 3_637
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
    "runtimeProfile",
    "observationStatus",
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
    bgiBenchmarkEvidence.runtimeProfile !== "kernel32-large-heap-allocation" ||
    bgiBenchmarkEvidence.observationStatus !==
      "current-large-heap-lazy-materialization-boundary-observation" ||
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
    bgiBenchmarkEvidence.executionEngineLinearMemoryBytes !==
      bgiBrowserEvidence.executionEngineLinearMemoryBytes ||
    bgiBenchmarkEvidence.wasmBridgeCallsPerSession !== bgiBrowserEvidence.wasmBridgeCalls ||
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
  const kernelBoundary = requireRecord(
    active.runtimeKernelBoundary,
    "activeRuntimeCheckpoint.runtimeKernelBoundary",
    errors,
  );
  const kernelRoles = requireStringArray(
    kernelBoundary.aggregateRole,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.aggregateRole",
    errors,
  );
  const kernelFields = requireStringArray(
    kernelBoundary.directFields,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.directFields",
    errors,
  );
  const processIndependentOwners = requireStringArray(
    kernelBoundary.processIndependentSubsystemOwners,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.processIndependentSubsystemOwners",
    errors,
  );
  const representativeSubsystemTests = requireStringArray(
    kernelBoundary.representativeSubsystemTests,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.representativeSubsystemTests",
    errors,
  );
  const representativeAtomicTests = requireStringArray(
    kernelBoundary.representativeCrossSubsystemAtomicTests,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests",
    errors,
  );
  if (
    kernelBoundary.schema !== "vine.runtime-kernel-boundary.v1" ||
    kernelBoundary.checkpoint !== "S13" ||
    kernelBoundary.publicFacade !== "runtime.rs" ||
    kernelBoundary.aggregateType !== "runtime/kernel.rs::RuntimeKernel" ||
    kernelBoundary.compatibilityAlias !== "RuntimeState" ||
    kernelBoundary.compatibilityAliasKind !== "type-alias" ||
    kernelRoles.join(",") !==
      "private-subsystem-ownership,cross-subsystem-transaction-coordination,compatibility-serialization" ||
    kernelFields.join(",") !==
      "process-state,process-address-space-allocator,threads,scheduler-and-waits,messages,displays,input,user32,timers,objects,com,heaps,module-records,synchronization,modules,registries" ||
    kernelBoundary.directMutableCollectionFields !== 0 ||
    kernelBoundary.directScalarAllocators !== 0 ||
    kernelBoundary.directSubsystemCollectionMutation !== false ||
    kernelBoundary.subsystemMutableCollectionVisibility !== "private" ||
    kernelBoundary.processStateOwner !==
      "runtime/process.rs::RuntimeProcessState" ||
    kernelBoundary.processMutableCollectionVisibility !== "private" ||
    processIndependentOwners.join(",") !==
      "displays,input,user32,timers,heaps,module-records,synchronization,com" ||
    kernelBoundary.processSerializationView !==
      "runtime/kernel.rs::RuntimeProcessView" ||
    kernelBoundary.processSerializationViewAccess !== "immutable-borrow" ||
    kernelBoundary.processLifecyclePlansRejectStaleCommit !== true ||
    kernelBoundary.closeHandleObjectPlan !==
      "runtime/objects.rs::RuntimeCloseHandleObjectMutationPlan" ||
    kernelBoundary.closeHandleObjectPlanVisibility !== "opaque-to-kernel" ||
    kernelBoundary.payloadObjectSubsystemRootCloneOrReplacementFromKernel !== false ||
    representativeSubsystemTests.join(",") !==
      "runtime::displays::tests::configuration_is_single_owner_and_rejects_invalid_geometry_without_mutation,runtime::input::tests::configuration_and_swaps_are_planned_and_generation_tracks_only_changes,runtime::user32::tests::shared_icon_plans_reuse_identity_and_teardown_atomically,runtime::user32::tests::shared_system_cursor_identity_and_cross_resource_plans_are_atomic,runtime::user32::tests::window_class_registration_is_module_scoped_case_folded_and_failure_atomic,runtime::user32::tests::invalid_window_class_metadata_and_atom_exhaustion_do_not_consume_identity,runtime::timers::tests::begin_end_pairing_counts_duplicates_and_rejects_stale_plans,runtime::timers::tests::capacity_overflow_and_teardown_are_failure_atomic,runtime::timers::tests::waitable_timer_creation_retirement_and_capacity_are_planned,runtime::heaps::tests::stale_or_duplicate_plans_preserve_all_heap_collections,runtime::modules::tests::dll_lifecycle_plan_commits_private_pending_callback_state,runtime::thread::tests::callback_and_exit_plans_commit_in_explicit_order,runtime::waits::tests::wake_plans_preserve_fifo_and_remove_only_during_commit,runtime::com::tests::stale_or_duplicate_plans_preserve_com_identity_and_refcounts" ||
    representativeAtomicTests.join(",") !==
      "runtime_heap_backing::private_heap_backing_is_atomic_reusable_and_process_owned,runtime_large_heap_materialization::large_heap_reserves_lazily_materializes_pages_and_reuses_its_permanent_slot,runtime_dll_lifecycle::guest_import_graph_drives_atomic_dll_process_lifecycle,runtime_lifecycle::process_and_thread_lifecycle_separates_exit_request_from_waitable_completion,runtime_wait_queue::wait_queue_wakes_on_thread_signal_and_cancels_on_process_exit,com_iunknown_foundation::x86_iunknown_abi_owns_identity_refcounts_and_structured_stops,user32_dispatcher::callback_preparation_failures_preserve_memory_and_frame_allocator,user32_dispatcher::swap_mouse_button_policy_and_authority_failures_are_atomic,user32_dispatcher::load_icon_a_negative_paths_are_failure_atomic_and_do_not_consume_identity,user32_dispatcher::load_cursor_a_negative_variants_are_failure_atomic,user32_dispatcher::register_class_ex_a_negative_paths_are_failure_atomic_and_do_not_consume_atom,user32_dispatcher::get_system_metrics_rejects_unreached_or_inconsistent_profiles_without_mutation,winmm_dispatcher::time_period_policy_and_lifecycle_failures_are_atomic,runtime_waitable_timer::waitable_timer_identity_wait_state_close_and_process_teardown_are_atomic" ||
    kernelBoundary.capabilityExpansionPausedAt !==
      root.nextStructuredStop?.nextCheckpoint
  ) {
    errors.push("active runtime kernel responsibility boundary is invalid");
  }
  const objects = requireRecord(
    active.runtimeObjectModel,
    "activeRuntimeCheckpoint.runtimeObjectModel",
    errors,
  );
  const objectKinds = requireStringArray(
    objects.objectKinds,
    "activeRuntimeCheckpoint.runtimeObjectModel.objectKinds",
    errors,
  );
  const waitableKinds = requireStringArray(
    objects.waitableKinds,
    "activeRuntimeCheckpoint.runtimeObjectModel.waitableKinds",
    errors,
  );
  if (
    objects.schema !== "vine.runtime-object-table.v2" ||
    objects.stateOwner !== "runtime/objects.rs::RuntimeObjectTable" ||
    objects.mutableCollectionVisibility !== "private" ||
    objects.readInspection !== "immutable-borrow" ||
    objects.crossSubsystemMutation !== "plan-preflight-commit" ||
    objects.subsystemTests < 4 ||
    objects.crossSubsystemAtomicTests < 4 ||
    objects.failedRegistrationPreservesAllocator !== true ||
    objects.aggregateDirectCollectionMutation !== false ||
    objects.wholeRuntimeStateClone !== false ||
    objectKinds.join(",") !== "process,thread,mutex,waitable-timer,private-heap,module" ||
    waitableKinds.join(",") !== "process,thread,mutex,waitable-timer"
  ) {
    errors.push("active runtime object subsystem boundary is invalid");
  }
  const comFoundation = requireRecord(
    active.runtimeComFoundation,
    "activeRuntimeCheckpoint.runtimeComFoundation",
    errors,
  );
  const comMethods = requireStringArray(
    comFoundation.implementedMethods,
    "activeRuntimeCheckpoint.runtimeComFoundation.implementedMethods",
    errors,
  );
  const comReturnKinds = requireStringArray(
    comFoundation.returnKinds,
    "activeRuntimeCheckpoint.runtimeComFoundation.returnKinds",
    errors,
  );
  const comFaultIdentityFields = requireStringArray(
    comFoundation.faultIdentityFields,
    "activeRuntimeCheckpoint.runtimeComFoundation.faultIdentityFields",
    errors,
  );
  if (
    comFoundation.schema !== "vine.runtime-com-foundation.v2" ||
    comFoundation.status !== "implemented-and-verified" ||
    comFoundation.stateOwner !== "runtime/com.rs::RuntimeCom" ||
    comFoundation.aggregateCoordinator !== "runtime/kernel.rs::RuntimeKernel" ||
    comFoundation.mutableCollectionVisibility !== "private" ||
    comFoundation.readInspection !== "immutable-borrow" ||
    comFoundation.logicalIdentityAuthority !== "rust" ||
    comFoundation.logicalIdentitySerialization !== "decimal-string" ||
    comFoundation.sharedReferenceCountAuthority !== "rust" ||
    comFoundation.apartmentStateOwner !== "runtime/thread.rs::RuntimeThreads" ||
    comFoundation.apartmentStateScope !==
      "per-thread-model-and-successful-initialization-count" ||
    comFoundation.apartmentMutation !==
      "plan-stale-check-targeted-thread-commit" ||
    comFoundation.apartmentThreadExitCleanup !== true ||
    comFoundation.canonicalIUnknownIid !==
      "00000000-0000-0000-c000-000000000046" ||
    comFoundation.abi !== "x86-com-stdcall" ||
    comMethods.join(",") !== "QueryInterface,AddRef,Release" ||
    comReturnKinds.join(",") !== "hresult,u32" ||
    comFoundation.guestLayout !== "two-page-readonly-data-rx-endpoints" ||
    comFoundation.endpointInstruction !== "ud2" ||
    comFoundation.addressAllocationPurpose !== "com-endpoint" ||
    comFoundation.addressAllocationIdentity !== true ||
    comFoundation.queryInterfaceOutputAndRefcountAtomic !== true ||
    comFoundation.completionPreflightBeforeMutation !== true ||
    comFoundation.finalReleaseRetiresGuestMappingAndAllocation !== true ||
    comFoundation.lastErrorPolicy !== "preserve" ||
    comFoundation.unsupportedMethodPolicy !== "structured-stop-before-mutation" ||
    comFaultIdentityFields.join(",") !== "interface,vtable-slot,method,call-site" ||
    comFoundation.browserResourceStateOwner !==
      "browser/runtime-worker/browser-resource-table.ts" ||
    comFoundation.browserIdentityNamespace !==
      "browser-resource:<positive-safe-integer>" ||
    comFoundation.browserAssociationOwnsComReference !== false ||
    comFoundation.subsystemTests < 1 ||
    comFoundation.fixtureTests < 4 ||
    comFoundation.atomicFailureTests < 3 ||
    comFoundation.wholeRuntimeStateClone !== false ||
    comFoundation.concreteGraphicsOrAudioInterfaces !== false ||
    comFoundation.coInitializeApi !== "null-reserved-single-threaded-v1" ||
    comFoundation.coInitializeExApiImplemented !== false ||
    comFoundation.coUninitializeApiImplemented !== false ||
    comFoundation.classFactoryOrAutomationImplemented !== false
  ) {
    errors.push("active runtime COM foundation boundary is invalid");
  }
  const registries = requireRecord(
    active.runtimeRegistries,
    "activeRuntimeCheckpoint.runtimeRegistries",
    errors,
  );
  if (
    registries.schema !== "vine.runtime-registries.v1" ||
    registries.stateOwner !== "runtime/registries.rs::RuntimeRegistries" ||
    registries.mutableCollectionVisibility !== "private" ||
    registries.readInspection !== "immutable-borrow" ||
    !Array.isArray(registries.serializedRegistryFields) ||
    registries.serializedRegistryFields.join(",") !==
      "dynamicEndpoints,encodedPointers" ||
    !Array.isArray(registries.serializedRuntimeCompatibilityFields) ||
    registries.serializedRuntimeCompatibilityFields.join(",") !==
      "encodedPointerNamespace,encodedPointerTokenPrefix,encodedPointerSources,nextEncodedPointerId" ||
    registries.hostRequestCompatibilityField !== "registries.hostRequests" ||
    registries.hostRequestAuthority !== "runtime/scheduler.rs::RuntimeScheduler" ||
    !Array.isArray(registries.ownedCollections) ||
    registries.ownedCollections.join(",") !==
      "dynamic-endpoint-table,encoded-pointer-token-table,encoded-pointer-source-index,encoded-pointer-id-allocator,encoded-pointer-policy" ||
    registries.dynamicEndpointTransition !==
      "registry-plan-aggregate-address-preflight-infallible-commit" ||
    registries.encodedPointerTransition !==
      "registry-plan-infallible-targeted-commit" ||
    registries.subsystemTests < 5 ||
    registries.crossSubsystemAtomicTests < 3 ||
    registries.directCollectionMutationFromAggregate !== false ||
    registries.encodedPointerIndexAtomic !== true ||
    registries.duplicateEndpointMutationFree !== true ||
    registries.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime registry subsystem boundary is invalid");
  }
  const modules = requireRecord(
    active.runtimeModules,
    "activeRuntimeCheckpoint.runtimeModules",
    errors,
  );
  if (
    modules.schema !== "vine.runtime-modules.v2" ||
    modules.stateOwner !== "runtime/modules.rs::RuntimeModules" ||
    modules.processRecordOwner !== "runtime/modules.rs::RuntimeModuleRecords" ||
    modules.dllLifecycleOperationOwner !==
      "runtime/modules.rs::RuntimeDllLifecycle" ||
    modules.mutableCollectionVisibility !== "private" ||
    modules.readInspection !== "immutable-borrow" ||
    modules.serializedRuntimeField !== "moduleGraph" ||
    modules.serializedProcessField !== "modules" ||
    modules.serializationAdapter !== "serde-flatten-private-runtime-modules" ||
    modules.lifecycleSerialized !== false ||
    !Array.isArray(modules.ownedCollections) ||
    modules.ownedCollections.join(",") !==
      "process-module-records,module-graph,module-images,module-import-bindings,runtime-system-exports,dll-lifecycle-operation-state" ||
    modules.crossSubsystemMutation !==
      "module-lifecycle-plan-object-memory-callback-preflight-targeted-commit" ||
    modules.lifecycleTransition !==
      "module-plan-thread-scheduler-preflight-infallible-commit" ||
    modules.subsystemTests < 6 ||
    modules.crossSubsystemAtomicTests < 4 ||
    modules.registrationRollbackMutation !== false ||
    modules.callbackReplacementAtomic !== true ||
    modules.activeLifecycleBlocksModuleMutation !== true ||
    modules.directCollectionMutationFromAggregate !== false ||
    modules.directLifecycleMutationFromAggregate !== false ||
    modules.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime module subsystem boundary is invalid");
  }
  const threads = requireRecord(
    active.runtimeThreadRegistry,
    "activeRuntimeCheckpoint.runtimeThreadRegistry",
    errors,
  );
  if (
    threads.schema !== "vine.runtime-thread-registry.v5" ||
    threads.stateOwner !== "runtime/thread.rs::RuntimeThreads" ||
    threads.mutableCollectionVisibility !== "private" ||
    threads.serializedRuntimeField !== "threads" ||
    !Array.isArray(threads.serializedProcessCompatibilityFields) ||
    threads.serializedProcessCompatibilityFields.join(",") !==
      "allocatedTlsIndices,flsSlots" ||
    !Array.isArray(threads.serializedRuntimeCompatibilityFields) ||
    threads.serializedRuntimeCompatibilityFields.join(",") !==
      "nextTlsIndex,nextFlsIndex,nextCallbackFrameId" ||
    threads.serializationAdapter !== "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(threads.processWideState) ||
    threads.processWideState.join(",") !==
      "tls-index-registry,fls-slot-registry,tls-index-allocator,fls-index-allocator,callback-frame-id-allocator" ||
    threads.crossSubsystemMutation !==
      "thread-resource-plan-aggregate-lifecycle-scheduler-preflight-infallible-commit" ||
    threads.subsystemTests < 8 ||
    threads.crossSubsystemAtomicTests < 4 ||
    threads.directRecordMutationFromAggregate !== false ||
    threads.directResourceMutationFromAggregate !== false ||
    threads.resourceAllocatorFailureMutationFree !== true ||
    threads.callbackAllocatorAtomic !== true ||
    threads.stackMetadataPlanCommit !== true ||
    threads.comApartmentPlanCommit !== true ||
    threads.comApartmentOverflowMutationFree !== true ||
    threads.comApartmentThreadExitCleanup !== true ||
    !Array.isArray(threads.perThreadState) ||
    threads.perThreadState.join(",") !==
      "last-error,tls-values,fls-values,callback-frames,stack-metadata,logical-affinity-mask,com-apartment" ||
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
    scheduler.schema !== "vine.runtime-scheduler.v2" ||
    scheduler.stateOwner !== "runtime/scheduler.rs::RuntimeScheduler" ||
    scheduler.mutableCollectionVisibility !== "private" ||
    scheduler.serializedRuntimeField !== "scheduler" ||
    !Array.isArray(scheduler.serializedFields) ||
    scheduler.serializedFields.join(",") !==
      "currentThreadId,threads,waitRegistrations,objectWaiters,events" ||
    !Array.isArray(scheduler.hostRequestSerializedRuntimeFields) ||
    scheduler.hostRequestSerializedRuntimeFields.join(",") !==
      "registries.hostRequests,nextHostRequestId" ||
    scheduler.hostRequestSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(scheduler.ownedCollections) ||
    scheduler.ownedCollections.join(",") !==
      "current-thread-selector,thread-lanes,event-fifo,host-request-table,host-request-id-allocator" ||
    scheduler.waitSubsystemAdapter !== "serde-flatten-private-runtime-waits" ||
    scheduler.crossSubsystemMutation !==
      "scheduler-host-plan-aggregate-lifecycle-preflight-targeted-commit" ||
    scheduler.hostRequestTransition !== "scheduler-plan-infallible-targeted-commit" ||
    scheduler.subsystemTests < 6 ||
    scheduler.crossSubsystemAtomicTests < 5 ||
    scheduler.directCollectionMutationFromAggregate !== false ||
    scheduler.directHostRequestMutationFromAggregate !== false ||
    scheduler.hostRequestCancellationPlan !== true ||
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
    waits.schema !== "vine.runtime-wait-queue.v3" ||
    waits.stateOwner !== "runtime/waits.rs::RuntimeWaits" ||
    waits.mutableCollectionVisibility !== "private" ||
    waits.readInspection !== "immutable-borrow" ||
    !Array.isArray(waits.serializedSchedulerFields) ||
    waits.serializedSchedulerFields.join(",") !==
      "waitRegistrations,objectWaiters" ||
    waits.allocatorSerializedRuntimeField !== "nextWaitRegistrationId" ||
    waits.allocatorSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(waits.ownedCollections) ||
    waits.ownedCollections.join(",") !==
      "registration-table,object-fifo,registration-id-allocator" ||
    waits.crossSubsystemMutation !==
      "wait-allocation-plan-aggregate-validation-scheduler-plan-infallible-commit" ||
    waits.subsystemTests < 5 ||
    waits.crossSubsystemAtomicTests < 4 ||
    waits.directCollectionMutationFromAggregate !== false ||
    waits.directAllocatorMutationFromAggregate !== false ||
    waits.allocatorOverflowMutationFree !== true ||
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
  const heaps = requireRecord(
    active.runtimeHeapBacking,
    "activeRuntimeCheckpoint.runtimeHeapBacking",
    errors,
  );
  if (
    heaps.schema !== "vine.runtime-private-heap-backing.v5" ||
    heaps.stateOwner !== "runtime/heaps.rs::RuntimeHeaps" ||
    heaps.mutableCollectionVisibility !== "private" ||
    heaps.readInspection !== "immutable-borrow" ||
    heaps.serializedProcessField !== "heaps" ||
    heaps.serializationAdapter !== "serde-flatten-private-runtime-heaps" ||
    !Array.isArray(heaps.serializedRuntimeCompatibilityFields) ||
    heaps.serializedRuntimeCompatibilityFields.join(",") !==
      "nextPrivateHeapSlot" ||
    heaps.allocatorSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(heaps.ownedState) ||
    heaps.ownedState.join(",") !==
      "logical-heap-map,guest-memory-backing-map,private-heap-slot-allocator,per-heap-allocation-id-allocators,process-large-allocation-backing,large-allocation-materialized-page-index" ||
    heaps.crossSubsystemMutation !==
      "heap-slot-plan-object-memory-preflight-targeted-commit" ||
    heaps.subsystemTests < 7 ||
    heaps.crossSubsystemAtomicTests < 3 ||
    heaps.logicalAndBackingCollectionsCoOwned !== true ||
    heaps.directCollectionMutationFromAggregate !== false ||
    heaps.directAllocatorMutationFromAggregate !== false ||
    heaps.automaticAllocatorCommitAtomic !== true ||
    heaps.explicitCreationPreservesAllocator !== true ||
    heaps.allocatorExhaustionMutationFree !== true ||
    heaps.failedCreationPreservesSlotAllocator !== true ||
    heaps.runtimeStateDirectScalarAllocators !== 0 ||
    heaps.wholeRuntimeStateClone !== false ||
    heaps.largeSlotBase !== "0x40000000" ||
    heaps.largeSlotEndExclusive !== "0x44000000" ||
    heaps.largeSlotBytes !== 64 * 1024 * 1024 ||
    heaps.largeSlotAddressAllocationPurpose !== "private-heap-large-slot" ||
    heaps.largeSlotAddressAllocationLifetime !== "process-permanent" ||
    heaps.largeSlotConcurrentLiveAllocations !== 1 ||
    heaps.largeAllocationSizePolicy !== "exact-67108864-only" ||
    heaps.largeAllocationMaterializationStateOwner !==
      "runtime/heaps.rs::RuntimeHeaps" ||
    heaps.largeAllocationMemoryAuthority !== "rust-guest-memory" ||
    heaps.largeAllocationFailuresMutationFree !== true ||
    heaps.guestHeapAllocOrdinaryMaximumBytes !== 16 * 1024 * 1024 ||
    heaps.guestHeapAllocLargeExactBytes?.join(",") !== `${64 * 1024 * 1024}` ||
    heaps.guestHeapAllocIntermediateLargeSizes !== false
  ) {
    errors.push("active runtime heap subsystem boundary is invalid");
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
      addressSpace.rootAllocationCount +
        addressSpace.heapSlotAllocationCount +
        addressSpace.largeHeapSlotAllocationCount ||
    addressSpace.heapBackingUsesAllocationId !== true ||
    addressSpace.largeHeapBackingUsesAllocationId !== true ||
    addressSpace.largeHeapAddressAllocationPermanent !== true ||
    addressSpace.comBackingUsesAllocationId !== true ||
    addressSpace.comFinalReleaseRetiresAllocation !== true ||
    addressSpace.controlMappingCommitUsesAllocationId !== true ||
    addressSpace.mappingDeltaBinaryVersion !== 2 ||
    addressSpace.mappingDeltaAllocationIdWidthBits !== 64 ||
    addressSpace.browserMirrorValidatesAllocationId !== true ||
    addressSpace.processEnvironmentReservationCommit !==
      "memory-runtime-atomic-plan"
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
  const heapAllocator = requireRecord(
    kernel32Policy.heapAllocator,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.heapAllocator",
    errors,
  );
  const systemInfo = requireRecord(
    kernel32Policy.systemInfo,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.systemInfo",
    errors,
  );
  const osVersionInfoA = requireRecord(
    kernel32Policy.osVersionInfoA,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.osVersionInfoA",
    errors,
  );
  if (
    kernel32Policy.schema !== "vine.kernel32-compatibility-policy" ||
    kernel32Policy.schemaVersion !== 2 ||
    kernel32Policy.id !== "kernel32-large-heap-allocation-v1" ||
    kernel32Policy.source !==
      "fixtures/runtime-policies/kernel32-large-heap-allocation-v1.json" ||
    kernel32Policy.handlerDispatch !== "single-semantic-implementation-id-with-policy" ||
    kernel32Policy.evidenceSelectsImplementationBranch !== false ||
    kernel32Policy.semanticTestsRequireEvidenceProfile !== false ||
    kernel32Policy.activeAnsiCodePage !== 932 ||
    kernel32Policy.multiByteAllowActiveCodePageSelector !== true ||
    !Array.isArray(kernel32Policy.multiByteAllowedFlags) ||
    kernel32Policy.multiByteAllowedFlags.join(",") !== "0,1" ||
    kernel32Policy.setCurrentDirectoryProcessImageDirectoryOnly !== true ||
    kernel32Policy.setCurrentDirectoryMaximumPathBytes !== 32768 ||
    kernel32Policy.getDriveTypeProcessImageDriveOnly !== true ||
    kernel32Policy.getDriveTypeMaximumPathBytes !== 32768 ||
    kernel32Policy.getDriveTypeAuthority !==
      "runtime/process.rs::RuntimeProcessDirectories" ||
    kernel32Policy.hostDriveTopologyImported !== false ||
    kernel32Policy.browserDriveTopologyImported !== false ||
    kernel32Policy.createMutexAllowNamed !== true ||
    kernel32Policy.createMutexAllowUnnamed !== false ||
    kernel32Policy.createMutexMaximumNameBytes !== 32768 ||
    kernel32Policy.createWaitableTimerAllowManualReset !== true ||
    kernel32Policy.createWaitableTimerAllowAutoReset !== false ||
    kernel32Policy.createWaitableTimerAllowUnnamed !== true ||
    kernel32Policy.threadAffinityProcessMask !== 1 ||
    systemInfo.processorArchitecture !== 0 ||
    systemInfo.pageSize !== 4096 ||
    systemInfo.minimumApplicationAddress !== 0x00010000 ||
    systemInfo.maximumApplicationAddress !== 0x7ffeffff ||
    systemInfo.processorType !== 586 ||
    systemInfo.allocationGranularity !== 65536 ||
    systemInfo.processorLevel !== 6 ||
    systemInfo.processorRevision !== 0 ||
    kernel32Policy.systemInfoActiveProcessorMaskSource !==
      "thread-affinity-process-mask" ||
    kernel32Policy.systemInfoProcessorCountSource !==
      "active-processor-mask-popcount" ||
    kernel32Policy.systemDefaultLangId !== 0x0411 ||
    kernel32Policy.systemDefaultLangIdSource !== "explicit-policy-field" ||
    kernel32Policy.systemDefaultLangIdDerivedFromAllowedLocales !== false ||
    kernel32Policy.hostLocaleImported !== false ||
    osVersionInfoA.majorVersion !== 10 ||
    osVersionInfoA.minorVersion !== 0 ||
    osVersionInfoA.buildNumber !== 19045 ||
    osVersionInfoA.platformId !== 2 ||
    osVersionInfoA.servicePack !== "" ||
    osVersionInfoA.servicePackMajor !== 0 ||
    osVersionInfoA.servicePackMinor !== 0 ||
    osVersionInfoA.suiteMask !== 0x0100 ||
    osVersionInfoA.productType !== 1 ||
    kernel32Policy.osVersionInfoAAuthority !==
      "Kernel32CompatibilityPolicy.os_version_info_a" ||
    kernel32Policy.osVersionInfoASupportedDeclaredSizes?.join(",") !== "148,156" ||
    kernel32Policy.osVersionInfoAHostVersionImported !== false ||
    kernel32Policy.osVersionInfoABrowserVersionImported !== false ||
    kernel32Policy.tickCountIncrementPerRead !== 1 ||
    !Array.isArray(heapAllocator.allowedFlags) ||
    heapAllocator.allowedFlags.join(",") !== "0,8" ||
    heapAllocator.minimumAllocationBytes !== 1 ||
    heapAllocator.maximumAllocationBytes !== 16 * 1024 * 1024 ||
    !Array.isArray(heapAllocator.largeAllocationBytes) ||
    heapAllocator.largeAllocationBytes.join(",") !== `${64 * 1024 * 1024}` ||
    heapAllocator.sizePolicy !==
      "ordinary-inclusive-nonzero-range-plus-explicit-large-variants" ||
    kernel32Policy.historicalExactHeapPolicySchemaVersion !== 1 ||
    kernel32Policy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active Kernel32 compatibility policy boundary is invalid");
  }
  const user32Policy = requireRecord(
    active.user32CompatibilityPolicy,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy",
    errors,
  );
  const primaryMonitorRect = requireRecord(
    user32Policy.primaryMonitorRect,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.primaryMonitorRect",
    errors,
  );
  const enumerationVariants = requireStringArray(
    user32Policy.enumerationVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.enumerationVariants",
    errors,
  );
  const swapMouseButtonVariants = requireStringArray(
    user32Policy.swapMouseButtonVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.swapMouseButtonVariants",
    errors,
  );
  const loadIconVariants = requireStringArray(
    user32Policy.loadIconVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadIconVariants",
    errors,
  );
  const loadCursorVariants = requireStringArray(
    user32Policy.loadCursorVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadCursorVariants",
    errors,
  );
  const registerClassExAVariants = requireStringArray(
    user32Policy.registerClassExAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.registerClassExAVariants",
    errors,
  );
  const registerClassExAAllowedStyles = requireIntegerArray(
    user32Policy.registerClassExAAllowedStyles,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.registerClassExAAllowedStyles",
    errors,
  );
  const getSystemMetricsIndices = requireIntegerArray(
    user32Policy.getSystemMetricsIndices,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.getSystemMetricsIndices",
    errors,
  );
  if (
    user32Policy.schema !== "vine.user32-compatibility-policy" ||
    user32Policy.schemaVersion !== 6 ||
    user32Policy.id !==
      "user32-single-monitor-normal-mouse-icon-cursor-class-metrics-v6" ||
    user32Policy.source !==
      "crates/vine-core/src/user32.rs::User32CompatibilityPolicy::controlled_v6" ||
    user32Policy.handlerDispatch !== "single-semantic-implementation-id-with-policy" ||
    user32Policy.logicalMonitorAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    user32Policy.displayProfileId !== "single-monitor-1920x1080-v1" ||
    user32Policy.monitorHandleNamespace !== "vine.logical-monitor.v1" ||
    user32Policy.primaryMonitorHandle !== "0x4f010001" ||
    primaryMonitorRect.left !== 0 ||
    primaryMonitorRect.top !== 0 ||
    primaryMonitorRect.right !== 1920 ||
    primaryMonitorRect.bottom !== 1080 ||
    enumerationVariants.join(",") !== "null-hdc-null-clip" ||
    user32Policy.logicalInputAuthority !== "runtime/input.rs::RuntimeInput" ||
    user32Policy.inputProfileId !== "normal-mouse-buttons-v1" ||
    user32Policy.initialMouseButtonsSwapped !== false ||
    swapMouseButtonVariants.join(",") !== "zero-normalize,nonzero-swap" ||
    user32Policy.swapMouseButtonReturn !== "previous-mapping-state" ||
    user32Policy.swapMouseButtonMutation !== "owner-plan-stale-check-targeted-commit" ||
    user32Policy.idempotentSwapPreservesGeneration !== true ||
    user32Policy.callbackCompletion !== "zero-false-nonzero-true" ||
    user32Policy.lastErrorPolicy !== "preserve" ||
    user32Policy.configuration !== "configure-once-plan-commit" ||
    user32Policy.hostDisplayTopologyImported !== false ||
    user32Policy.browserDisplayTopologyImported !== false ||
    user32Policy.hostMouseButtonMappingImported !== false ||
    user32Policy.browserMouseButtonMappingImported !== false ||
    user32Policy.logicalIconAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.loadIconProfileId !== "module-shared-large-icon-32x32-v1" ||
    user32Policy.loadIconTargetWidth !== 32 ||
    user32Policy.loadIconTargetHeight !== 32 ||
    user32Policy.loadIconMaximumGroupEntries !== 64 ||
    loadIconVariants.join(",") !== "mapped-module-integer-resource" ||
    user32Policy.loadIconLastErrorPolicy !==
      "handler-success-zero-specific-failure-code" ||
    user32Policy.iconHandleNamespace !== "vine.logical-icon.v1" ||
    user32Policy.firstIconHandle !== "0x49010001" ||
    user32Policy.sharedIconMutation !== "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedResourceSharesIdentity !== true ||
    user32Policy.processExitRetiresIcons !== true ||
    user32Policy.hostIconImported !== false ||
    user32Policy.browserIconImported !== false ||
    user32Policy.systemIconImplemented !== false ||
    user32Policy.stringResourceNameImplemented !== false ||
    user32Policy.pngIconImplemented !== false ||
    user32Policy.logicalCursorAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.loadCursorProfileId !== "system-arrow-shared-cursor-v1" ||
    user32Policy.loadCursorArrowResourceId !== 32_512 ||
    loadCursorVariants.join(",") !== "null-instance-idc-arrow" ||
    user32Policy.loadCursorSuccessLastErrorPolicy !== "preserve" ||
    user32Policy.loadCursorMissingResourceId !== 65_535 ||
    user32Policy.loadCursorMissingResourceLastError !== 1_814 ||
    user32Policy.cursorHandleNamespace !== "vine.logical-cursor.v1" ||
    user32Policy.firstCursorHandle !== "0x43010001" ||
    user32Policy.sharedCursorMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedSystemCursorSharesIdentity !== true ||
    user32Policy.processExitRetiresCursors !== true ||
    user32Policy.hostCursorImported !== false ||
    user32Policy.browserCursorImported !== false ||
    user32Policy.moduleCursorImplemented !== false ||
    user32Policy.otherPredefinedCursorImplemented !== false ||
    user32Policy.stringCursorNameImplemented !== false ||
    user32Policy.logicalWindowClassAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.registerClassExAProfileId !== "local-ascii-module-class-v1" ||
    user32Policy.registerClassExAStructureBytes !== 48 ||
    user32Policy.registerClassExAMaximumNameBytes !== 64 ||
    registerClassExAAllowedStyles.join(",") !== "40,12320" ||
    registerClassExAVariants.join(",") !==
      "mapped-local-module-ascii-main-and-child" ||
    user32Policy.windowClassAtomNamespace !==
      "vine.logical-window-class-atom.v1" ||
    user32Policy.firstWindowClassAtom !== "0xc000" ||
    user32Policy.windowClassMutation !== "owner-plan-stale-check-targeted-commit" ||
    user32Policy.windowClassNameIndex !== "ascii-case-folded-module-and-name" ||
    user32Policy.windowClassAtomIndex !== "logical-atom-to-class-identity" ||
    user32Policy.windowClassReferences !==
      "copy-owned-non-owning-logical-icon-and-cursor-identities" ||
    user32Policy.windowProcedureContract !==
      "same-module-executable-x86-stdcall-four-arguments" ||
    user32Policy.registerClassExASuccessLastErrorPolicy !== "preserve" ||
    user32Policy.registerClassExADuplicateLastError !== 1_410 ||
    user32Policy.registerClassExAInvalidSizeLastError !== 87 ||
    user32Policy.processExitRetiresWindowClasses !== true ||
    user32Policy.hostWindowClassImported !== false ||
    user32Policy.browserWindowClassImported !== false ||
    user32Policy.globalWindowClassImplemented !== false ||
    user32Policy.integerWindowClassNameImplemented !== false ||
    user32Policy.nonAsciiWindowClassNameImplemented !== false ||
    user32Policy.classQueryOrUnregisterApiImplemented !== false ||
    user32Policy.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-v1" ||
    getSystemMetricsIndices.join(",") !== "0,1,4,7,8" ||
    user32Policy.getSystemMetricsCaptionHeight !== 23 ||
    user32Policy.getSystemMetricsFixedFrameWidth !== 3 ||
    user32Policy.getSystemMetricsFixedFrameHeight !== 3 ||
    user32Policy.getSystemMetricsScreenDimensionsSource !==
      "runtime-displays-primary-monitor-rect" ||
    user32Policy.getSystemMetricsLastErrorPolicy !== "preserve" ||
    user32Policy.getSystemMetricsGuestWrites !== 0 ||
    user32Policy.hostSystemMetricsImported !== false ||
    user32Policy.browserSystemMetricsImported !== false ||
    user32Policy.unreachedSystemMetricIndicesImplemented !== false ||
    user32Policy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active User32 compatibility policy boundary is invalid");
  }
  const winmmPolicy = requireRecord(
    active.winmmCompatibilityPolicy,
    "activeRuntimeCheckpoint.winmmCompatibilityPolicy",
    errors,
  );
  if (
    winmmPolicy.schema !== "vine.winmm-compatibility-policy" ||
    winmmPolicy.schemaVersion !== 1 ||
    winmmPolicy.id !== "winmm-timecaps-1-1000000-period-1-v1" ||
    winmmPolicy.source !==
      "crates/vine-core/src/winmm.rs::WinmmCompatibilityPolicy::controlled_v1" ||
    winmmPolicy.handlerDispatch !== "single-semantic-implementation-id-with-policy" ||
    winmmPolicy.timeCapsProfileId !== "timecaps-1-1000000-v1" ||
    winmmPolicy.periodMinimumMilliseconds !== 1 ||
    winmmPolicy.periodMaximumMilliseconds !== 1_000_000 ||
    winmmPolicy.x86TimeCapsBytes !== 8 ||
    winmmPolicy.shortSizeResult !== 97 ||
    winmmPolicy.shortSizeMutation !== "none" ||
    winmmPolicy.acceptedSizeRange !== "eight-or-larger" ||
    winmmPolicy.successfulWriteBytes !== 8 ||
    winmmPolicy.timerResolutionProfileId !== "timer-resolution-period-1-v1" ||
    !Array.isArray(winmmPolicy.allowedPeriodsMilliseconds) ||
    winmmPolicy.allowedPeriodsMilliseconds.join(",") !== "1" ||
    winmmPolicy.timerResolutionStateOwner !== "runtime/timers.rs::RuntimeTimers" ||
    winmmPolicy.timerResolutionMutation !== "owner-plan-stale-check-targeted-commit" ||
    winmmPolicy.duplicateBeginRequiresMatchingEnds !== true ||
    winmmPolicy.unmatchedEndResult !== 97 ||
    winmmPolicy.processExitRetiresRequests !== true ||
    winmmPolicy.lastErrorPolicy !== "preserve" ||
    winmmPolicy.hostTimerCapabilitiesImported !== false ||
    winmmPolicy.browserTimerCapabilitiesImported !== false ||
    winmmPolicy.hostTimerResolutionImported !== false ||
    winmmPolicy.browserTimerResolutionImported !== false ||
    winmmPolicy.clockAdvancedByQuery !== false ||
    winmmPolicy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active WinMM compatibility policy boundary is invalid");
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
  if (
    latestEvidence.getDriveTypeAImplemented !== true ||
    latestEvidence.getDriveTypeACallCount !== 26 ||
    latestEvidence.getDriveTypeAStateOwner !==
      "runtime/process.rs::RuntimeProcessDirectories" ||
    latestEvidence.getDriveTypeAHostDriveTopologyImported !== false ||
    latestEvidence.getDriveTypeABrowserDriveTopologyImported !== false ||
    latestEvidence.vfsBackendImplemented !== false
  ) {
    errors.push("latest drive-type authority evidence is invalid");
  }
  if (
    latestEvidence.swapMouseButtonImplemented !== true ||
    latestEvidence.swapMouseButtonCallCount !== 2 ||
    latestEvidence.swapMouseButtonStateOwner !== "runtime/input.rs::RuntimeInput" ||
    latestEvidence.swapMouseButtonFinalSwapped !== false ||
    latestEvidence.swapMouseButtonFinalGeneration !== 0 ||
    latestEvidence.swapMouseButtonHostStateImported !== false ||
    latestEvidence.swapMouseButtonBrowserStateImported !== false
  ) {
    errors.push("latest mouse-button mapping authority evidence is invalid");
  }
  for (const field of [
    "nativeTimeGetDevCapsPrimaryTraceSha256",
    "nativeTimeGetDevCapsConfirmationTraceSha256",
    "nativeTimePeriodPrimaryTraceSha256",
    "nativeTimePeriodConfirmationTraceSha256",
    "nativeGetVersionExAPrimaryTraceSha256",
    "nativeGetVersionExAConfirmationTraceSha256",
    "nativeCreateWaitableTimerAPrimaryTraceSha256",
    "nativeCreateWaitableTimerAConfirmationTraceSha256",
    "nativeLoadIconAPrimaryTraceSha256",
    "nativeLoadIconAConfirmationTraceSha256",
    "nativeLoadCursorAPrimaryTraceSha256",
    "nativeLoadCursorAConfirmationTraceSha256",
    "nativeRegisterClassExAPrimaryTraceSha256",
    "nativeRegisterClassExAConfirmationTraceSha256",
    "nativeGetSystemMetricsPrimaryTraceSha256",
    "nativeGetSystemMetricsConfirmationTraceSha256",
    "nativeLargeHeapPrimaryTraceSha256",
    "nativeLargeHeapConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeLargeHeapPrimaryTraceBytes !== 419_374 ||
    latestEvidence.nativeLargeHeapPrimaryTraceLines !== 204 ||
    latestEvidence.nativeLargeHeapPrimaryBalancedApiPairs !== 75 ||
    latestEvidence.nativeLargeHeapConfirmationTraceBytes !== 419_375 ||
    latestEvidence.nativeLargeHeapConfirmationTraceLines !== 204 ||
    latestEvidence.nativeLargeHeapConfirmationBalancedApiPairs !== 75 ||
    latestEvidence.nativeLargeHeapTraceDiagnostics !== 0 ||
    latestEvidence.nativeLargeHeapProfile !==
      "main-module-large-heap-allocation-lifetime-v1" ||
    latestEvidence.nativeLargeHeapFlags !== 0 ||
    latestEvidence.nativeLargeHeapRequestedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.nativeLargeHeapRequestedPages !== 16_384 ||
    latestEvidence.nativeLargeHeapAccessedPages !== 30 ||
    latestEvidence.nativeLargeHeapUntouchedPages !== 16_354 ||
    latestEvidence.nativeLargeHeapAccessedPageRun?.join(",") !== "0,29" ||
    latestEvidence.nativeLargeHeapAccessedExtentBytes !== 122_880 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.read !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.write !== 30 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.execute !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.other !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessRva !== "0x00061aef" ||
    latestEvidence.nativeLargeHeapLastNewPageAccessRva !== "0x000a3f37" ||
    latestEvidence.nativeLargeHeapLastNewPageOffset !== 122_848 ||
    latestEvidence.nativeLargeHeapFreeResult !== 1 ||
    latestEvidence.nativeLargeHeapLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLargeHeapPointerClassification !==
      "opaque-native-artifact-identity" ||
    latestEvidence.nativeLargeHeapExecutableHashUnchanged !== true
  ) {
    errors.push("latest native large-heap lifetime evidence is invalid");
  }
  if (
    latestEvidence.largeHeapAllocImplemented !== true ||
    latestEvidence.largeHeapAllocSequence !== 1_350 ||
    latestEvidence.largeHeapAllocOccurrence !== 12 ||
    latestEvidence.largeHeapAllocInstructionCount !== 50_105 ||
    latestEvidence.largeHeapAllocArguments?.join(",") !==
      "0x68010001,0x00000000,0x04000000" ||
    latestEvidence.largeHeapAllocReturnAddress !== "0x100a4a33" ||
    latestEvidence.largeHeapAllocResult !== "0x40000000" ||
    latestEvidence.largeHeapAllocLastError !== 0 ||
    latestEvidence.largeHeapAllocGuestWrites !== 0 ||
    latestEvidence.largeHeapAllocImplementationId !== "kernel32.heap-alloc.v2" ||
    latestEvidence.largeHeapAllocAllocationId !== 12 ||
    latestEvidence.largeHeapAllocRequestedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.largeHeapAllocReservedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.largeHeapAllocInitialization !== "deterministic-uninitialized" ||
    latestEvidence.largeHeapAllocLiveAllocations !== 10 ||
    latestEvidence.largeHeapReserveFromGeneration !== 1_469 ||
    latestEvidence.largeHeapReserveGeneration !== 1_470 ||
    latestEvidence.largeHeapReserveMappingOperations !== 1 ||
    latestEvidence.largeHeapReserveDirtyPages !== 0 ||
    latestEvidence.largeHeapReservePhysicalPageDelta !== 0 ||
    latestEvidence.largeHeapMaterializationInstructionCount !== 50_123 ||
    latestEvidence.largeHeapMaterializationFaultAddress !== "0x40000000" ||
    latestEvidence.largeHeapMaterializationPageFaultErrorCode !== 6 ||
    latestEvidence.largeHeapMaterializationMemoryOwner !== 11 ||
    latestEvidence.largeHeapMaterializationAddressAllocationId !== 37 ||
    latestEvidence.largeHeapMaterializationCommittedPage !== "0x40000000" ||
    latestEvidence.largeHeapMaterializationCommittedBytes !== 4_096 ||
    latestEvidence.largeHeapMaterializationFromGeneration !== 1_470 ||
    latestEvidence.largeHeapMaterializationGeneration !== 1_472 ||
    latestEvidence.largeHeapMaterializationPayloadSha256 !==
      "f600eca824e84a43f0691b267bd620e462c50da165c5b80e17aecb7a924f1fa8" ||
    latestEvidence.largeHeapMaterializationFillByte !== 0xa5 ||
    latestEvidence.largeHeapMaterializedPages !== 1 ||
    latestEvidence.largeHeapReservedPagesBefore !== 4_784 ||
    latestEvidence.largeHeapReservedPagesAfter !== 21_168 ||
    latestEvidence.largeHeapPhysicalPagesBefore !== 395 ||
    latestEvidence.largeHeapPhysicalPagesAfter !== 396 ||
    latestEvidence.largeHeapSecondLiveRejectedBeforeMutation !== true ||
    latestEvidence.largeHeapDuplicateStaleOutsideProtectionQuotaRejectedBeforeMutation !== true ||
    latestEvidence.largeHeapAddressRecordPermanentAcrossFree !== true
  ) {
    errors.push("latest large-heap allocation and materialization evidence is invalid");
  }
  if (
    latestEvidence.nativeTimeGetDevCapsTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimeGetDevCapsOracleLastErrorsPreserved !== true ||
    latestEvidence.nativeTimeGetDevCapsOracleWritesBounded !== true ||
    latestEvidence.nativeTimePeriodTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimePeriodOracleProfile !==
      "x86-time-period-range-and-pairing-v1" ||
    latestEvidence.nativeTimePeriodOracleCallCount !== 17 ||
    latestEvidence.nativeTimePeriodOracleAllPreserveLastError !== true ||
    latestEvidence.nativeTimePeriodUnmatchedEndResult !== 97 ||
    latestEvidence.nativeTimePeriodZeroBeginResult !== 97 ||
    latestEvidence.nativeTimePeriodDuplicatePairingExact !== true ||
    latestEvidence.nativeTimeBeginPeriodReached !== true ||
    latestEvidence.timeGetDevCapsImplemented !== true ||
    latestEvidence.timeGetDevCapsSequence !== 1334 ||
    latestEvidence.timeGetDevCapsInstructionCount !== 49731 ||
    latestEvidence.timeGetDevCapsResult !== 0 ||
    latestEvidence.timeGetDevCapsLastError !== 0 ||
    latestEvidence.timeGetDevCapsGuestWriteBytes !== 8 ||
    latestEvidence.timeGetDevCapsOutputHex !== "0100000040420f00" ||
    latestEvidence.timeGetDevCapsPeriodMinimumMilliseconds !== 1 ||
    latestEvidence.timeGetDevCapsPeriodMaximumMilliseconds !== 1_000_000 ||
    latestEvidence.timeGetDevCapsPolicyOwner !==
      "WinmmCompatibilityPolicy.time_get_dev_caps" ||
    latestEvidence.timeGetDevCapsHostCapabilitiesImported !== false ||
    latestEvidence.timeGetDevCapsBrowserCapabilitiesImported !== false ||
    latestEvidence.timeGetDevCapsAdvancesClock !== false ||
    latestEvidence.timeBeginPeriodImplemented !== true ||
    latestEvidence.timeBeginPeriodSequence !== 1335 ||
    latestEvidence.timeBeginPeriodInstructionCount !== 49737 ||
    latestEvidence.timeBeginPeriodArgument !== 1 ||
    latestEvidence.timeBeginPeriodResult !== 0 ||
    latestEvidence.timeBeginPeriodLastError !== 0 ||
    latestEvidence.timeBeginPeriodGuestWrites !== 0 ||
    latestEvidence.timeBeginPeriodStateOwner !== "runtime/timers.rs::RuntimeTimers" ||
    latestEvidence.timeBeginPeriodRequestCount !== 1 ||
    latestEvidence.timeBeginPeriodTotalRequestCount !== 1 ||
    latestEvidence.timeBeginPeriodEffectivePeriodMilliseconds !== 1 ||
    latestEvidence.timeBeginPeriodTimerGeneration !== 1 ||
    latestEvidence.timeBeginPeriodHostTimerResolutionImported !== false ||
    latestEvidence.timeBeginPeriodBrowserTimerResolutionImported !== false ||
    latestEvidence.timeBeginPeriodAdvancesClock !== false ||
    latestEvidence.timeEndPeriodImplemented !== true ||
    latestEvidence.timeEndPeriodReached !== false ||
    latestEvidence.timerResolutionProcessExitTeardown !== true
  ) {
    errors.push("latest WinMM timer-resolution evidence is invalid");
  }
  if (
    latestEvidence.nativeGetVersionExATraceDiagnostics !== 0 ||
    latestEvidence.nativeGetVersionExAOracleProfile !==
      "x86-get-version-ex-a-size-and-output-v1" ||
    latestEvidence.nativeGetVersionExABaseStructureBytes !== 148 ||
    latestEvidence.nativeGetVersionExAExtendedStructureBytes !== 156 ||
    latestEvidence.nativeGetVersionExAInvalidSizeResult !== 0 ||
    latestEvidence.nativeGetVersionExAInvalidSizeLastError !== 122 ||
    latestEvidence.nativeGetVersionExAInvalidSizeMutation !== false ||
    latestEvidence.nativeGetVersionExASuccessPreservesLastError !== true ||
    latestEvidence.nativeGetVersionExABgiDeclaredSize !== 148 ||
    latestEvidence.nativeGetVersionExABgiResult !== 1 ||
    latestEvidence.nativeGetVersionExABgiConsumer !==
      "compare-platform-id-with-ver-platform-win32-nt" ||
    latestEvidence.getVersionExAImplemented !== true ||
    latestEvidence.getVersionExASequence !== 1337 ||
    latestEvidence.getVersionExAInstructionCount !== 49890 ||
    latestEvidence.getVersionExAResult !== 1 ||
    latestEvidence.getVersionExALastError !== 0 ||
    latestEvidence.getVersionExAGuestWriteBytes !== 148 ||
    latestEvidence.getVersionExAPlatformId !== 2 ||
    latestEvidence.getVersionExAHostVersionImported !== false ||
    latestEvidence.getVersionExABrowserVersionImported !== false
  ) {
    errors.push("latest ANSI OS-version evidence is invalid");
  }
  if (
    latestEvidence.schema !== "vine.v86-bgi-large-heap-allocation.v1" ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryTraceBytes !== 256_897 ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryTraceLines !== 779 ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryBalancedApiPairs !== 354 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationTraceBytes !== 255_564 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationTraceLines !== 777 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationBalancedApiPairs !== 353 ||
    latestEvidence.nativeCreateWaitableTimerATraceDiagnostics !== 0 ||
    latestEvidence.nativeCreateWaitableTimerAOracleProfile !==
      "x86-create-waitable-timer-initial-state-v1" ||
    latestEvidence.nativeCreateWaitableTimerAOracleManualAndAutoCreated !== true ||
    latestEvidence.nativeCreateWaitableTimerAOracleSuccessLastError !== 0 ||
    latestEvidence.nativeCreateWaitableTimerAOracleInitialWaitResult !== 258 ||
    latestEvidence.nativeCreateWaitableTimerAOracleAllInitiallyNonsignaled !== true ||
    latestEvidence.nativeCreateWaitableTimerAOracleAllClosed !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiCallerReturnRva !== "0x0005053b" ||
    latestEvidence.nativeCreateWaitableTimerABgiSecurityAttributes !== "0x00000000" ||
    latestEvidence.nativeCreateWaitableTimerABgiManualReset !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiNameAddress !== "0x00000000" ||
    latestEvidence.nativeCreateWaitableTimerABgiResultNonNull !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiLastError !== 0 ||
    latestEvidence.nativeCreateWaitableTimerABgiConsumer !==
      "store-eax-directly-at-object-offset-4" ||
    latestEvidence.createWaitableTimerAImplemented !== true ||
    latestEvidence.createWaitableTimerASequence !== 1338 ||
    latestEvidence.createWaitableTimerAInstructionCount !== 49_897 ||
    latestEvidence.createWaitableTimerAArguments?.join(",") !==
      "0x00000000,0x00000001,0x00000000" ||
    latestEvidence.createWaitableTimerAReturnAddress !== "0x1005053b" ||
    latestEvidence.createWaitableTimerAResult !== "0x57010001" ||
    latestEvidence.createWaitableTimerALastError !== 0 ||
    latestEvidence.createWaitableTimerAGuestWrites !== 0 ||
    latestEvidence.createWaitableTimerAStateOwner !==
      "runtime/timers.rs::RuntimeTimers" ||
    latestEvidence.createWaitableTimerATimerId !== 1 ||
    latestEvidence.createWaitableTimerAManualReset !== true ||
    latestEvidence.createWaitableTimerAObjectTableId !== 20 ||
    latestEvidence.createWaitableTimerAObjectNamespace !==
      "vine.runtime-waitable-timer-object.v1" ||
    latestEvidence.createWaitableTimerAInitialWaitState !== "nonsignaled" ||
    latestEvidence.createWaitableTimerAOpenHandles !== 1 ||
    latestEvidence.createWaitableTimerALastCloseRetiresPayloadAndObject !== true ||
    latestEvidence.createWaitableTimerAProcessExitRetiresPayloadAndObject !== true ||
    latestEvidence.createWaitableTimerAHostTimerImported !== false ||
    latestEvidence.createWaitableTimerABrowserTimerImported !== false
  ) {
    errors.push("latest waitable-timer identity and lifetime evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadIconAPrimaryTraceBytes !== 285_147 ||
    latestEvidence.nativeLoadIconAPrimaryTraceLines !== 779 ||
    latestEvidence.nativeLoadIconAPrimaryBalancedApiPairs !== 354 ||
    latestEvidence.nativeLoadIconAConfirmationTraceBytes !== 2_257_599 ||
    latestEvidence.nativeLoadIconAConfirmationTraceLines !== 6_952 ||
    latestEvidence.nativeLoadIconAConfirmationBalancedApiPairs !== 3_438 ||
    latestEvidence.nativeLoadIconATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadIconAOracleProfile !==
      "x86-module-group-icon-shared-identity-v1" ||
    latestEvidence.nativeLoadIconAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadIconAOracleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadIconAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadIconAOracleMissingResourceLastError !== 1_813 ||
    latestEvidence.nativeLoadIconABgiProfile !== "x86-bgi-load-icon-a-v1" ||
    latestEvidence.nativeLoadIconABgiCallCount !== 2 ||
    latestEvidence.nativeLoadIconABgiCallerReturnRvas?.join(",") !==
      "0x00067cf3,0x00067d7c" ||
    latestEvidence.nativeLoadIconABgiResourceId !== 101 ||
    latestEvidence.nativeLoadIconABgiResultsNonNull !== true ||
    latestEvidence.nativeLoadIconABgiIdentityShared !== true ||
    latestEvidence.nativeLoadIconABgiLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLoadIconABgiConsumer !==
      "wndclassexa-hicon-with-null-hiconsm" ||
    latestEvidence.nativeLoadIconASelectedWidth !== 32 ||
    latestEvidence.nativeLoadIconASelectedHeight !== 32 ||
    latestEvidence.nativeLoadIconASelectedPlanes !== 1 ||
    latestEvidence.nativeLoadIconASelectedBitsPerPixel !== 32 ||
    latestEvidence.nativeLoadIconALargeSystemMetrics?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadIconASmallSystemMetrics?.join(",") !== "16,16" ||
    latestEvidence.nativeLoadIconAGroupResourceId !== 101 ||
    latestEvidence.nativeLoadIconAGroupLanguageId !== 1_041 ||
    latestEvidence.nativeLoadIconAGroupDataBytes !== 62 ||
    latestEvidence.nativeLoadIconASelectedIconResourceId !== 3 ||
    latestEvidence.nativeLoadIconASelectedIconDataBytes !== 4_264 ||
    latestEvidence.loadIconAImplemented !== true ||
    latestEvidence.loadIconASequence !== 1_339 ||
    latestEvidence.loadIconAInstructionCount !== 49_942 ||
    latestEvidence.loadIconAArguments?.join(",") !== "0x10000000,0x00000065" ||
    latestEvidence.loadIconAReturnAddress !== "0x10067cf3" ||
    latestEvidence.loadIconAResult !== "0x49010001" ||
    latestEvidence.loadIconALastError !== 0 ||
    latestEvidence.loadIconAGuestWrites !== 0 ||
    latestEvidence.loadIconAStateOwner !== "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadIconAIconId !== 1 ||
    latestEvidence.loadIconAHandleNamespace !== "vine.logical-icon.v1" ||
    latestEvidence.loadIconAShared !== true ||
    latestEvidence.loadIconAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadIconAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadIconAGroupResourceId !== 101 ||
    latestEvidence.loadIconALanguageId !== 1_041 ||
    latestEvidence.loadIconAIconResourceId !== 3 ||
    latestEvidence.loadIconAGroupDataRva !== "0x0016ea08" ||
    latestEvidence.loadIconAGroupDataBytes !== 62 ||
    latestEvidence.loadIconAIconDataRva !== "0x0016bf5c" ||
    latestEvidence.loadIconAIconDataBytes !== 4_264 ||
    latestEvidence.loadIconAWidth !== 32 ||
    latestEvidence.loadIconAHeight !== 32 ||
    latestEvidence.loadIconAPlanes !== 1 ||
    latestEvidence.loadIconABitsPerPixel !== 32 ||
    latestEvidence.loadIconARepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadIconAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadIconAHostIconImported !== false ||
    latestEvidence.loadIconABrowserIconImported !== false
  ) {
    errors.push("latest module-icon identity and resource evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadCursorAPrimaryTraceBytes !== 333_790 ||
    latestEvidence.nativeLoadCursorAPrimaryTraceLines !== 806 ||
    latestEvidence.nativeLoadCursorAPrimaryBalancedApiPairs !== 367 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceBytes !== 333_385 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceLines !== 805 ||
    latestEvidence.nativeLoadCursorAConfirmationBalancedApiPairs !== 366 ||
    latestEvidence.nativeLoadCursorATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadCursorAOracleProfile !==
      "x86-system-arrow-cursor-shared-identity-v1" ||
    latestEvidence.nativeLoadCursorAOracleArrowResourceId !== 32_512 ||
    latestEvidence.nativeLoadCursorAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadCursorAOracleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadCursorAOracleSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceLastError !== 1_814 ||
    latestEvidence.nativeLoadCursorASystemMetrics?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadCursorABgiProfile !== "x86-bgi-load-cursor-a-v1" ||
    latestEvidence.nativeLoadCursorABgiCallCount !== 4 ||
    latestEvidence.nativeLoadCursorABgiSystemArrowCallCount !== 3 ||
    latestEvidence.nativeLoadCursorABgiCallerReturnRvas?.join(",") !==
      "0x00067d09,0x00067d8c,0x00067fd2,0x00067fe1" ||
    latestEvidence.nativeLoadCursorABgiSystemArrowResultsShared !== true ||
    latestEvidence.nativeLoadCursorABgiLastErrors?.join(",") !== "0,0,0,0" ||
    latestEvidence.nativeLoadCursorABgiFirstConsumer !== "wndclassexa-hcursor" ||
    latestEvidence.nativeLoadCursorABgiModuleResourceId !== 106 ||
    latestEvidence.nativeLoadCursorABgiModuleResultNonNull !== true ||
    latestEvidence.loadCursorAImplemented !== true ||
    latestEvidence.loadCursorASequence !== 1_340 ||
    latestEvidence.loadCursorAInstructionCount !== 49_949 ||
    latestEvidence.loadCursorAArguments?.join(",") !==
      "0x00000000,0x00007f00" ||
    latestEvidence.loadCursorAReturnAddress !== "0x10067d09" ||
    latestEvidence.loadCursorAResult !== "0x43010001" ||
    latestEvidence.loadCursorALastError !== 0 ||
    latestEvidence.loadCursorASuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.loadCursorAGuestWrites !== 0 ||
    latestEvidence.loadCursorAStateOwner !== "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadCursorACursorId !== 1 ||
    latestEvidence.loadCursorAHandleNamespace !== "vine.logical-cursor.v1" ||
    latestEvidence.loadCursorAShared !== true ||
    latestEvidence.loadCursorASourceKind !== "system-predefined" ||
    latestEvidence.loadCursorAResourceId !== 32_512 ||
    latestEvidence.loadCursorACursorKind !== "arrow" ||
    latestEvidence.loadCursorARepeatedSystemResourceSharesIdentity !== true ||
    latestEvidence.loadCursorAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadCursorAHostCursorImported !== false ||
    latestEvidence.loadCursorABrowserCursorImported !== false
  ) {
    errors.push("latest system-cursor identity and lifetime evidence is invalid");
  }
  if (
    latestEvidence.nativeRegisterClassExAPrimaryTraceBytes !== 326_543 ||
    latestEvidence.nativeRegisterClassExAPrimaryTraceLines !== 804 ||
    latestEvidence.nativeRegisterClassExAPrimaryBalancedApiPairs !== 366 ||
    latestEvidence.nativeRegisterClassExAConfirmationTraceBytes !== 327_416 ||
    latestEvidence.nativeRegisterClassExAConfirmationTraceLines !== 805 ||
    latestEvidence.nativeRegisterClassExAConfirmationBalancedApiPairs !== 366 ||
    latestEvidence.nativeRegisterClassExATraceDiagnostics !== 0 ||
    latestEvidence.nativeRegisterClassExAOracleProfile !==
      "x86-register-class-ex-a-identity-lifetime-v1" ||
    latestEvidence.nativeRegisterClassExAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeRegisterClassExAOracleValidAtomsDistinct !== true ||
    latestEvidence.nativeRegisterClassExAOracleDuplicateRejected !== true ||
    latestEvidence.nativeRegisterClassExAOracleDuplicateLastError !== 1_410 ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeRejected !== true ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeLastError !== 87 ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeDidNotReserveIdentity !== true ||
    latestEvidence.nativeRegisterClassExAOracleSuccessLastErrorsPreserved !== true ||
    latestEvidence.nativeRegisterClassExAOracleInputsPreserved !== true ||
    latestEvidence.nativeRegisterClassExAOracleNameAndAtomQueriesSucceeded !== true ||
    latestEvidence.nativeRegisterClassExAOracleCleanupSucceeded !== true ||
    latestEvidence.nativeRegisterClassExAOracleQueryAfterCleanupRejected !== true ||
    latestEvidence.nativeRegisterClassExABgiProfile !==
      "x86-bgi-register-class-ex-a-v1" ||
    latestEvidence.nativeRegisterClassExABgiCallCount !== 2 ||
    latestEvidence.nativeRegisterClassExABgiCallerReturnRvas?.join(",") !==
      "0x00067d28,0x00067dab" ||
    latestEvidence.nativeRegisterClassExABgiStructureBytes?.join(",") !== "48,48" ||
    latestEvidence.nativeRegisterClassExABgiStyles?.join(",") !== "40,12320" ||
    latestEvidence.nativeRegisterClassExABgiWindowProcedureRvas?.join(",") !==
      "0x00092550,0x0002e6e0" ||
    latestEvidence.nativeRegisterClassExABgiClassNames?.join(",") !==
      "BGI - Main window,BGI - Child window" ||
    latestEvidence.nativeRegisterClassExABgiSameMainModule !== true ||
    latestEvidence.nativeRegisterClassExABgiSharedIconAndCursorInputs !== true ||
    latestEvidence.nativeRegisterClassExABgiOptionalFieldsNull !== true ||
    latestEvidence.nativeRegisterClassExABgiResultsNonzero !== true ||
    latestEvidence.nativeRegisterClassExABgiLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeRegisterClassExABgiInputsPreserved !== true
  ) {
    errors.push("latest native window-class registration evidence is invalid");
  }
  if (
    latestEvidence.registerClassExAImplemented !== true ||
    latestEvidence.registerClassExACallCount !== 2 ||
    latestEvidence.registerClassExASequences?.join(",") !== "1341,1344" ||
    latestEvidence.registerClassExAInstructionCounts?.join(",") !== "49957,49984" ||
    latestEvidence.registerClassExAArguments?.join(",") !==
      "0x006ffb08,0x006ffb08" ||
    latestEvidence.registerClassExAReturnAddresses?.join(",") !==
      "0x10067d28,0x10067dab" ||
    latestEvidence.registerClassExAResults?.join(",") !==
      "0x0000c000,0x0000c001" ||
    latestEvidence.registerClassExALastErrors?.join(",") !== "0,0" ||
    latestEvidence.registerClassExASuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.registerClassExAGuestWrites !== 0 ||
    latestEvidence.registerClassExAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.registerClassExAClassIds?.join(",") !== "1,2" ||
    latestEvidence.registerClassExAAtomNamespace !==
      "vine.logical-window-class-atom.v1" ||
    latestEvidence.registerClassExAAtoms?.join(",") !== "49152,49153" ||
    latestEvidence.registerClassExALocal !== true ||
    latestEvidence.registerClassExAModuleId !== 15 ||
    latestEvidence.registerClassExAModuleLoadBase !== "0x10000000" ||
    latestEvidence.registerClassExAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.registerClassExAStyles?.join(",") !== "40,12320" ||
    latestEvidence.registerClassExAWindowProcedureAddresses?.join(",") !==
      "0x10092550,0x1002e6e0" ||
    latestEvidence.registerClassExAWindowProcedureRvas?.join(",") !==
      "0x00092550,0x0002e6e0" ||
    latestEvidence.registerClassExAWindowProcedureCallingConvention !== "stdcall" ||
    latestEvidence.registerClassExAWindowProcedureArgumentBytes !== 16 ||
    latestEvidence.registerClassExAClassExtraBytes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAWindowExtraBytes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAIconId !== 1 ||
    latestEvidence.registerClassExAIconHandle !== "0x49010001" ||
    latestEvidence.registerClassExACursorId !== 1 ||
    latestEvidence.registerClassExACursorHandle !== "0x43010001" ||
    latestEvidence.registerClassExABackgroundBrushes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAClassNames?.join(",") !==
      "BGI - Main window,BGI - Child window" ||
    latestEvidence.registerClassExAClassNameEncoding !== "windows-932-v1" ||
    latestEvidence.registerClassExAMenuNamesNull !== true ||
    latestEvidence.registerClassExASmallIconsNull !== true ||
    latestEvidence.registerClassExASecondIconAndCursorReuseIdentity !== true ||
    latestEvidence.registerClassExADuplicateSameModuleNameLastError !== 1_410 ||
    latestEvidence.registerClassExAInvalidSizeLastError !== 87 ||
    latestEvidence.registerClassExAProcessExitRetiresIdentity !== true ||
    latestEvidence.registerClassExAHostClassImported !== false ||
    latestEvidence.registerClassExABrowserClassImported !== false
  ) {
    errors.push("latest logical window-class identity and lifetime evidence is invalid");
  }
  if (
    latestEvidence.nativeGetSystemMetricsPrimaryTraceBytes !== 55_901 ||
    latestEvidence.nativeGetSystemMetricsPrimaryTraceLines !== 99 ||
    latestEvidence.nativeGetSystemMetricsPrimaryBalancedApiPairs !== 22 ||
    latestEvidence.nativeGetSystemMetricsConfirmationTraceBytes !== 55_839 ||
    latestEvidence.nativeGetSystemMetricsConfirmationTraceLines !== 99 ||
    latestEvidence.nativeGetSystemMetricsConfirmationBalancedApiPairs !== 22 ||
    latestEvidence.nativeGetSystemMetricsTraceDiagnostics !== 0 ||
    latestEvidence.nativeGetSystemMetricsOracleProfile !==
      "x86-system-metrics-last-error-v1" ||
    latestEvidence.nativeGetSystemMetricsOracleIndices?.join(",") !== "7,8,4,0,1" ||
    latestEvidence.nativeGetSystemMetricsOracleValues?.join(",") !==
      "3,3,23,1920,1080" ||
    latestEvidence.nativeGetSystemMetricsOracleAllResultsMatchHooks !== true ||
    latestEvidence.nativeGetSystemMetricsOracleSuccessLastErrorsPreserved !== true ||
    latestEvidence.nativeGetSystemMetricsBgiProfile !==
      "x86-bgi-initial-window-metrics-v1" ||
    latestEvidence.nativeGetSystemMetricsBgiCallCount !== 5 ||
    latestEvidence.nativeGetSystemMetricsBgiIndices?.join(",") !== "7,8,4,0,1" ||
    latestEvidence.nativeGetSystemMetricsBgiValues?.join(",") !==
      "3,3,23,1920,1080" ||
    latestEvidence.nativeGetSystemMetricsBgiCallerReturnRvas?.join(",") !==
      "0x00067ddb,0x00067de6,0x00067dee,0x00067e26,0x00067e2e" ||
    latestEvidence.nativeGetSystemMetricsBgiLastErrors?.join(",") !== "0,0,0,0,0" ||
    latestEvidence.nativeGetSystemMetricsBgiFrameWidth !== 6 ||
    latestEvidence.nativeGetSystemMetricsBgiFrameHeight !== 29 ||
    latestEvidence.getSystemMetricsImplemented !== true ||
    latestEvidence.getSystemMetricsCallCount !== 5 ||
    latestEvidence.getSystemMetricsSequences?.join(",") !==
      "1345,1346,1347,1348,1349" ||
    latestEvidence.getSystemMetricsInstructionCounts?.join(",") !==
      "49990,49995,50000,50020,50024" ||
    latestEvidence.getSystemMetricsArguments?.join(",") !== "7,8,4,0,1" ||
    latestEvidence.getSystemMetricsReturnAddresses?.join(",") !==
      "0x10067ddb,0x10067de6,0x10067dee,0x10067e26,0x10067e2e" ||
    latestEvidence.getSystemMetricsResults?.join(",") !== "3,3,23,1920,1080" ||
    latestEvidence.getSystemMetricsLastErrors?.join(",") !== "0,0,0,0,0" ||
    latestEvidence.getSystemMetricsSuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.getSystemMetricsGuestWrites !== 0 ||
    latestEvidence.getSystemMetricsDisplayStateOwner !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.getSystemMetricsNonclientPolicyOwner !==
      "User32CompatibilityPolicy.get_system_metrics" ||
    latestEvidence.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-v1" ||
    latestEvidence.getSystemMetricsSupportedIndices?.join(",") !== "0,1,4,7,8" ||
    latestEvidence.getSystemMetricsHostValuesImported !== false ||
    latestEvidence.getSystemMetricsBrowserValuesImported !== false
  ) {
    errors.push("latest controlled system-metrics evidence is invalid");
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
    bgiBrowserEvidence.guestInstructions !== root.nextStructuredStop?.instructionCount ||
    bgiBrowserEvidence.nodeFinalGeneration !== latestEvidence.finalMemoryGeneration ||
    bgiBrowserEvidence.memoryContentSha256 !== latestEvidence.memoryContentSha256 ||
    bgiBrowserEvidence.nodeOracleReportBytes !== latestEvidence.reportBytes ||
    bgiBrowserEvidence.nodeOracleReportSha256 !== latestEvidence.reportSha256 ||
    bgiBrowserEvidence.nextCapabilityCluster !== root.nextStructuredStop?.capabilityCluster ||
    bgiBrowserEvidence.nextEndpoint?.toLowerCase() !==
      root.nextStructuredStop?.api?.toLowerCase() ||
    bgiBrowserEvidence.nextFaultCode !== root.nextStructuredStop?.reason ||
    bgiBrowserEvidence.nextEndpointAddress !== root.nextStructuredStop?.endpointAddress ||
    bgiBrowserEvidence.nextArguments?.join(",") !==
      root.nextStructuredStop?.arguments?.join(",")
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
  const kernelBoundary = active.runtimeKernelBoundary;
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
| Runtime architecture | \`${kernelBoundary.checkpoint}\`: \`${kernelBoundary.aggregateType}\`; \`${kernelBoundary.compatibilityAlias}\` remains a compatibility alias |
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
