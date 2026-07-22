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

function requireOrderedIntegerArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => !Number.isSafeInteger(item))) {
    errors.push(`${label} must be an array of integers`);
    return [];
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
    "scenarioMilestones",
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
  if (root.schemaVersion !== 3) {
    errors.push("manifest.schemaVersion must be 3");
  }

  const scenarioMilestones = requireRecord(
    root.scenarioMilestones,
    "scenarioMilestones",
    errors,
  );
  if (scenarioMilestones.schema !== "vine.scenario-milestones") {
    errors.push("scenarioMilestones.schema must be vine.scenario-milestones");
  }
  if (scenarioMilestones.schemaVersion !== 1) {
    errors.push("scenarioMilestones.schemaVersion must be 1");
  }
  requireString(
    scenarioMilestones.activeMilestone,
    "scenarioMilestones.activeMilestone",
    errors,
  );
  const expectedMilestoneIds = [
    "boot-and-process-initialization",
    "logical-window-and-message-flow",
    "immutable-source-asset-access",
    "first-present-readable-screen",
    "user-input-consumption",
    "audible-output",
    "save-browser-restart-load",
    "representative-flow",
    "full-play-long-run",
  ];
  const milestones = Array.isArray(scenarioMilestones.milestones)
    ? scenarioMilestones.milestones
    : [];
  if (!Array.isArray(scenarioMilestones.milestones)) {
    errors.push("scenarioMilestones.milestones must be an array");
  }
  const milestoneIds = [];
  for (let index = 0; index < milestones.length; index += 1) {
    const milestone = requireRecord(
      milestones[index],
      `scenarioMilestones.milestones[${index}]`,
      errors,
    );
    requireString(milestone.id, `scenarioMilestones.milestones[${index}].id`, errors);
    requireString(milestone.label, `scenarioMilestones.milestones[${index}].label`, errors);
    requireString(
      milestone.evidenceBoundary,
      `scenarioMilestones.milestones[${index}].evidenceBoundary`,
      errors,
    );
    if (!["in-progress", "bounded-foundation", "fixture-only", "not-reached"].includes(milestone.status)) {
      errors.push(`scenarioMilestones.milestones[${index}].status is invalid`);
    }
    milestoneIds.push(milestone.id);
  }
  if (milestoneIds.join(",") !== expectedMilestoneIds.join(",")) {
    errors.push("scenarioMilestones must contain the canonical scenario order");
  }
  const activeScenario = milestones.find(
    ({ id }) => id === scenarioMilestones.activeMilestone,
  );
  if (activeScenario?.status !== "in-progress") {
    errors.push("scenarioMilestones.activeMilestone must identify an in-progress milestone");
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
  if (browser.browserRuntimeProtocol !== "vine.browser-runtime-worker.v7") {
    errors.push("browserCheckpoint.browserRuntimeProtocol must identify protocol v7");
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
  const browserRuntimeCoreWasm = requireRecord(
    browser.browserRuntimeCoreWasm,
    "browserCheckpoint.browserRuntimeCoreWasm",
    errors,
  );
  requireInteger(
    browserRuntimeCoreWasm.bytes,
    "browserCheckpoint.browserRuntimeCoreWasm.bytes",
    errors,
    1,
  );
  requireSha256(
    browserRuntimeCoreWasm.sha256,
    "browserCheckpoint.browserRuntimeCoreWasm.sha256",
    errors,
  );
  if (browserRuntimeCoreWasm.imports !== 0) {
    errors.push("browser runtime core Wasm must remain import-free");
  }
  const browserRuntimeFixture = requireRecord(
    browser.browserRuntimeFixture,
    "browserCheckpoint.browserRuntimeFixture",
    errors,
  );
  requireSha256(
    browserRuntimeFixture.sha256,
    "browserCheckpoint.browserRuntimeFixture.sha256",
    errors,
  );
  if (
    browserRuntimeFixture.bytes !== 3_584 ||
    browserRuntimeFixture.sha256 !==
      "6f3f5d91ee8fe586b70da650c204cc07b1bf2fdbf210b3fa04bc723d09942b56" ||
    browserRuntimeFixture.nativeExitCode !== 0
  ) {
    errors.push("browser runtime fixture identity is invalid");
  }
  if (
    browserEvidence.guestInstructions !== 1_966 ||
    browserEvidence.cpuSlices !== 255 ||
    browserEvidence.budgetYields !== 233 ||
    browserEvidence.hostRequests !== 2 ||
    browserEvidence.rustSchedulerTransitions !== 2 ||
    browserEvidence.callbackEntries !== 2 ||
    browserEvidence.maximumCallbackDepth !== 2 ||
    browserEvidence.writebackBatches !== 63 ||
    browserEvidence.writebackPages !== 65 ||
    browserEvidence.initialGeneration !== 15 ||
    browserEvidence.finalGeneration !== 123 ||
    browserEvidence.runtimeCoreLinearMemoryBytes !== 1_638_400 ||
    browserEvidence.executionEngineLinearMemoryBytes !== 9_961_472 ||
    browserEvidence.executionStateMachine?.success?.transitionCount !== 769 ||
    browserEvidence.executionStateMachine?.cancellation?.transitionCount !== 12 ||
    browserEvidence.executionStateMachine?.watchdog?.transitionCount !== 12 ||
    browserEvidence.exitCode !== 0 ||
    browserEvidence.cancelInstructions !== 13 ||
    browserEvidence.cancelPendingHostRequests !== 1 ||
    browserEvidence.cancelPendingHostCallsAfterCleanup !== 0 ||
    browserEvidence.cancelPendingCallbacksAfterCleanup !== 0 ||
    browserEvidence.watchdogInstructions !== 13 ||
    browserEvidence.watchdogPendingHostCallsAfterCleanup !== 0 ||
    browserEvidence.watchdogPendingCallbacksAfterCleanup !== 0
  ) {
    errors.push("controlled browser runtime evidence is invalid");
  }
  const sourceFixture = requireRecord(
    browser.browserSourceReadFixture,
    "browserCheckpoint.browserSourceReadFixture",
    errors,
  );
  const sourceEvidence = requireRecord(
    browser.browserSourceReadEvidence,
    "browserCheckpoint.browserSourceReadEvidence",
    errors,
  );
  for (const [field, value] of [
    ["fixture sha256", sourceFixture.sha256],
    ["source object sha256", sourceFixture.sourceObjectSha256],
    ["report sha256", sourceEvidence.reportSha256],
  ]) {
    requireSha256(value, `browser source ${field}`, errors);
  }
  if (
    sourceFixture.bytes !== 2_560 ||
    sourceFixture.sha256 !==
      "292d298cb77a8223cb0f6fb66333457b3fab9b8e4f0c1643cf01936b769a1d63" ||
    sourceFixture.nativeExitCode !== 0 ||
    sourceFixture.sourceObjectBytes !== 94 ||
    sourceFixture.sourceObjectSha256 !==
      "95c642cb0120d8973d8dc66972cd09c1191f31dc85f30af4f75a80fb5836ed2c" ||
    sourceEvidence.guestInstructions !== 217 ||
    sourceEvidence.cpuSlices !== 30 ||
    sourceEvidence.budgetYields !== 23 ||
    sourceEvidence.hostRequests !== 2 ||
    sourceEvidence.rustSchedulerTransitions !== 2 ||
    sourceEvidence.sourceReadCompletions !== 2 ||
    sourceEvidence.sourceReadOffsets?.join(",") !== "0,50" ||
    sourceEvidence.sourceReadRequestedBytes?.join(",") !== "8,11" ||
    sourceEvidence.sourceReadTransferredBytes?.join(",") !== "8,11" ||
    sourceEvidence.sourceReadPayloadSha256?.join(",") !==
      "202034b246570abc8e0e549637dbeb3ce7990de1fedeef6bcbf795934a3bb2da,71c02fe83d1ee0855a1875c47a53372209e4892b3b8bd7bcdc5d1ea9610d0dc3" ||
    sourceEvidence.titleSnapshotObjectHttpIdentity !== true ||
    sourceEvidence.initialGeneration !== 12 ||
    sourceEvidence.finalGeneration !== 24 ||
    sourceEvidence.wasmBridgeCalls !== 26 ||
    sourceEvidence.runtimeCoreLinearMemoryBytes !== 1_572_864 ||
    sourceEvidence.executionEngineLinearMemoryBytes !== 9_764_864 ||
    sourceEvidence.successExitCode !== 0 ||
    sourceEvidence.successPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.successPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.cancelInstructions !== 32 ||
    sourceEvidence.cancelSourceReadCompletions !== 0 ||
    sourceEvidence.cancelPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.cancelPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.watchdogInstructions !== 32 ||
    sourceEvidence.watchdogRequestId !== "1" ||
    sourceEvidence.watchdogLastTransition !== "waiting-host" ||
    sourceEvidence.watchdogSourceReadCompletions !== 0 ||
    sourceEvidence.watchdogPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.watchdogPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.reportBytes !== 23_505 ||
    sourceEvidence.reportSha256 !==
      "a8a5aba21b10092fa616ac7c128c5985e464976200c1dc8d1bf4735aa6bda4ab"
  ) {
    errors.push("browser immutable source-read evidence is invalid");
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
    "mappingDeltas",
    "mappingDirtyPages",
    "mappingAtomicRejections",
    "mappingStaleAccessStops",
    "mirrorRebuilds",
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
    "sourceReadCompletions",
    "sourceReadPendingAfterCleanup",
    "callbackReplacements",
    "logicalWindowCompletions",
    "d3d9GetAdapterCountCompletions",
    "d3d9GetAdapterCountSequence",
    "d3d9GetAdapterCountResult",
    "d3d9GetAdapterCountGuestWriteBytes",
    "d3d9GetAdapterCountReferenceCount",
    "d3d9GetAdapterMonitorCompletions",
    "d3d9GetAdapterMonitorGuestWriteBytes",
    "d3d9GetAdapterMonitorReferenceCount",
    "d3d9GetAdapterDisplayModeCompletions",
    "d3d9GetAdapterDisplayModeSequence",
    "d3d9GetAdapterDisplayModeReferenceCount",
    "d3d9GetAdapterIdentifierCompletions",
    "d3d9GetAdapterIdentifierSequence",
    "d3d9GetAdapterIdentifierOutputBytes",
    "d3d9GetAdapterIdentifierReferenceCount",
    "d3d9GetDeviceCapsCompletions",
    "d3d9GetDeviceCapsSequence",
    "d3d9GetDeviceCapsOutputBytes",
    "d3d9GetDeviceCapsReferenceCount",
    "d3d9CheckDeviceTypeCompletions",
    "d3d9CheckDeviceTypeReferenceCount",
    "d3d9CheckDeviceTypeGuestWriteBytes",
    "d3d9CheckDeviceFormatCompletions",
    "d3d9CheckDeviceFormatSequence",
    "d3d9CheckDeviceFormatReferenceCount",
    "d3d9CheckDeviceFormatGuestWriteBytes",
    "d3d9CreateDeviceCompletions",
    "d3d9CreateDeviceSequence",
    "d3d9CreateDeviceLastError",
    "d3d9CreateDeviceGuestWriteBytes",
    "d3d9CreateDeviceObjectId",
    "d3d9CreateDeviceInterfaceId",
    "d3d9CreateDeviceRootReferenceCount",
    "d3d9CreateDeviceReferenceCount",
    "d3d9CreateDeviceLogicalGraphicsDeviceCount",
    "d3d9CreateTextureCompletions",
    "d3d9CreateTextureSequence",
    "d3d9CreateTextureLastError",
    "d3d9CreateTextureGuestWriteBytes",
    "d3d9CreateTextureObjectId",
    "d3d9CreateTextureInterfaceId",
    "d3d9CreateTextureDeviceReferenceCount",
    "d3d9CreateTextureReferenceCount",
    "d3d9CreateTextureLogicalGraphicsDeviceId",
    "d3d9CreateTextureLogicalGraphicsTextureId",
    "d3d9CreateTextureLogicalGraphicsDeviceCount",
    "d3d9CreateTextureLogicalGraphicsTextureCount",
    "d3d9GetSurfaceLevelCompletions",
    "d3d9GetSurfaceLevelSequence",
    "d3d9GetSurfaceLevelLastError",
    "d3d9GetSurfaceLevelGuestWriteBytes",
    "d3d9GetSurfaceLevelObjectId",
    "d3d9GetSurfaceLevelInterfaceId",
    "d3d9GetSurfaceLevelTextureReferenceCount",
    "d3d9GetSurfaceLevelReferenceCount",
    "d3d9GetSurfaceLevelLogicalGraphicsTextureId",
    "d3d9GetSurfaceLevelLogicalGraphicsSurfaceId",
    "d3d9GetSurfaceLevelLogicalGraphicsSurfaceCount",
    "d3d9SurfaceGetDcCompletions",
    "d3d9SurfaceGetDcSequence",
    "d3d9SurfaceGetDcLastError",
    "d3d9SurfaceGetDcGuestWriteBytes",
    "d3d9SurfaceGetDcDeviceContextId",
    "d3d9SurfaceGetDcLogicalGraphicsSurfaceId",
    "d3d9SurfaceGetDcSurfaceComObjectId",
    "d3d9SurfaceGetDcReferenceCount",
    "d3d9SurfaceGetDcLiveDeviceContexts",
    "d3d9SurfaceGetStockObjectSequence",
    "d3d9SurfaceFillRectSequence",
    "d3d9SurfaceFillRectLastError",
    "d3d9SurfaceFillRectGuestWriteBytes",
    "d3d9SurfaceFillRectLogicalGraphicsSurfaceId",
    "d3d9SurfaceFillRectColorRef",
    "d3d9SurfaceFillRectContentGeneration",
    "d3d9SurfaceFillRectSurfaceGeneration",
    "d3d9SurfaceReleaseDcCompletions",
    "d3d9SurfaceReleaseDcSequence",
    "d3d9SurfaceReleaseDcLastError",
    "d3d9SurfaceReleaseDcGuestWriteBytes",
    "d3d9SurfaceReleaseDcLiveDeviceContexts",
    "d3d9SurfaceReleaseSequence",
    "d3d9SurfaceReleaseResult",
    "d3d9SurfaceReleaseReferenceCount",
    "d3d9SurfaceReleaseLogicalGraphicsSurfaceCount",
    "d3d9SecondCreateTextureSequence",
    "d3d9SecondCreateTexturePool",
    "d3d9SecondCreateTextureLastError",
    "d3d9SecondCreateTextureGuestWriteBytes",
    "d3d9SecondCreateTextureDeviceReferenceCount",
    "d3d9SecondCreateTextureReferenceCount",
    "d3d9ThirdCreateTextureSequence",
    "d3d9ThirdCreateTexturePool",
    "d3d9ThirdCreateTextureUsage",
    "d3d9ThirdCreateTextureLastError",
    "d3d9ThirdCreateTextureGuestWriteBytes",
    "d3d9ThirdCreateTextureDeviceReferenceCount",
    "d3d9ThirdCreateTextureReferenceCount",
    "d3d9CreateVertexBufferCompletions",
    "d3d9CreateVertexBufferSequence",
    "d3d9CreateVertexBufferInstructionCount",
    "d3d9CreateVertexBufferLastError",
    "d3d9CreateVertexBufferGuestWriteBytes",
    "d3d9CreateVertexBufferDeviceReferenceCountAtCompletion",
    "d3d9CreateVertexBufferReferenceCount",
    "d3d9CreateVertexBufferLogicalGraphicsDeviceId",
    "d3d9CreateVertexBufferLogicalGraphicsVertexBufferId",
    "d3d9CreateVertexBufferLogicalGraphicsVertexBufferCount",
    "d3d9CreateVertexBufferBackingBytes",
    "d3d9VertexBufferLockCompletions",
    "d3d9VertexBufferLockSequence",
    "d3d9VertexBufferLockInstructionCount",
    "d3d9VertexBufferLockLastError",
    "d3d9VertexBufferLockGuestWriteBytes",
    "d3d9VertexBufferLockReferenceCount",
    "d3d9VertexBufferUnlockCompletions",
    "d3d9VertexBufferUnlockSequence",
    "d3d9VertexBufferUnlockInstructionCount",
    "d3d9VertexBufferUnlockLastError",
    "d3d9VertexBufferUnlockGuestWriteBytes",
    "d3d9VertexBufferUnlockReferenceCount",
    "d3d9VertexBufferUnlockContentGeneration",
    "d3d9SetRenderStateCompletions",
    "d3d9SetRenderStateDeviceReferenceCount",
    "d3d9SetRenderStateGenerationDelta",
    "loadLibraryACompletions",
    "loadLibraryASequence",
    "loadLibraryALastError",
    "loadLibraryAGuestWriteBytes",
    "loadLibraryAModuleId",
    "loadLibraryAAttachSequence",
    "loadLibraryAObjectId",
    "loadLibraryAThreadReferenceCount",
    "d3dxGetProcAddressCompletions",
    "d3dxGetProcAddressSequence",
    "d3dxGetProcAddressLastError",
    "getClientRectCompletions",
    "getClientRectSequence",
    "getClientRectLastError",
    "getClientRectGuestWriteBytes",
    "getDcCompletions",
    "getDcSequence",
    "getDcLastError",
    "getDcGuestWriteBytes",
    "getDcDeviceContextId",
    "getDcSurfaceId",
    "getDcLiveDeviceContexts",
    "getStockObjectCompletions",
    "getStockObjectSequence",
    "getStockObjectLastError",
    "getStockObjectGuestWriteBytes",
    "getStockObjectId",
    "getStockObjectColorRef",
    "getStockObjectLiveObjects",
    "fillRectCompletions",
    "fillRectSequence",
    "fillRectLastError",
    "fillRectGuestWriteBytes",
    "fillRectSurfaceId",
    "fillRectSurfaceWidth",
    "fillRectSurfaceHeight",
    "fillRectSurfaceGeneration",
    "fillRectColorRef",
    "releaseDcCompletions",
    "releaseDcSequence",
    "releaseDcLastError",
    "releaseDcGuestWriteBytes",
    "releaseDcLiveDeviceContexts",
    "releaseDcLiveSurfaces",
    "setWindowLongACompletions",
    "setWindowLongASequence",
    "setWindowLongALastError",
    "setWindowLongAGuestWriteBytes",
    "setWindowPosActivationCompletions",
    "setWindowPosActivationSequence",
    "setWindowPosActivationLastError",
    "setWindowPosActivationGuestWriteBytes",
    "timeGetTimeCompletions",
    "timeGetTimeIncrementPerRead",
    "successExecutionTransitionCount",
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
    "callSemanticsSha256",
    "largeHeapMaterializationPayloadSha256",
    "sourceReadPayloadSha256",
    "d3d9GetAdapterIdentifierOutputSha256",
    "d3d9GetDeviceCapsOutputSha256",
    "d3d9CreateVertexBufferInitialContentSha256",
    "d3d9VertexBufferUnlockContentSha256",
  ]) {
    requireSha256(
      bgiBrowserEvidence[field],
      `browserCheckpoint.bgiRuntimeEvidence.${field}`,
      errors,
    );
  }
  const bgiD3d9CheckDeviceTypeSequences = requireIntegerArray(
    bgiBrowserEvidence.d3d9CheckDeviceTypeSequences,
    "browserCheckpoint.bgiRuntimeEvidence.d3d9CheckDeviceTypeSequences",
    errors,
  );
  const bgiD3d9AdapterMonitorSequences = requireIntegerArray(
    bgiBrowserEvidence.d3d9GetAdapterMonitorSequences,
    "browserCheckpoint.bgiRuntimeEvidence.d3d9GetAdapterMonitorSequences",
    errors,
  );
  if (
    bgiBrowserEvidence.nodeRole !== "semantic-oracle-only" ||
    bgiBrowserEvidence.runtimeStateAuthority !== "rust-wasm-in-runtime-worker" ||
    bgiBrowserEvidence.evidenceLevel !== "summary" ||
    bgiBrowserEvidence.callSemanticsAlgorithm !== "sha256-canonical-json-v1" ||
    bgiBrowserEvidence.nodeOracleReportBytes > 2 * 1024 * 1024 ||
    bgiBrowserEvidence.browserReportBytes > 2 * 1024 * 1024 ||
    bgiMappingOperations.reserve !== 12 ||
    bgiMappingOperations.commit !== 58 ||
    bgiMappingOperations.protect !== 19 ||
    bgiMappingOperations.decommit !== 0 ||
    bgiMappingOperations.release !== 3 ||
    bgiBrowserEvidence.mappingDeltas !== 16_573 ||
    bgiBrowserEvidence.mappingDirtyPages !== 17_773 ||
    bgiBrowserEvidence.mappingAtomicRejections !== 0 ||
    bgiBrowserEvidence.mappingStaleAccessStops !== 0 ||
    bgiBrowserEvidence.mirrorRebuilds !== 0 ||
    bgiBrowserEvidence.successExecutionTransitionCount !== 87_260 ||
    JSON.stringify(bgiBrowserEvidence.registeredThreadIds) !== JSON.stringify([8_192]) ||
    JSON.stringify(bgiBrowserEvidence.executedThreadIds) !== JSON.stringify([8_192]) ||
    bgiBrowserEvidence.contextSwitches !== 0 ||
    bgiBrowserEvidence.pendingHostCallsAfterCleanup !== 0 ||
    bgiBrowserEvidence.pendingCallbacksAfterCleanup !== 0 ||
    bgiBrowserEvidence.processLifecycleAfterCleanup !== "exited" ||
    bgiBrowserEvidence.dllLifecycleAfterCleanup !== "process-detached" ||
    bgiBrowserEvidence.largeHeapPageMaterializations !== 1 ||
    bgiBrowserEvidence.largeHeapAllocationAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapAllocationBytes !== 64 * 1024 * 1024 ||
    bgiBrowserEvidence.largeHeapAllocationId !== 12 ||
    bgiBrowserEvidence.largeHeapAddressAllocationId !== 37 ||
    bgiBrowserEvidence.largeHeapMemoryOwner !== 12 ||
    bgiBrowserEvidence.largeHeapFirstFaultInstructionCount !== 50_123 ||
    bgiBrowserEvidence.largeHeapFirstFaultAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapFirstPageFaultErrorCode !== 6 ||
    bgiBrowserEvidence.largeHeapFirstCommittedPageBytes !== 4_096 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationFromGeneration !== 1_473 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationGeneration !== 1_475 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationFromGeneration !== 1_487 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationGeneration !== 1_489 ||
    bgiBrowserEvidence.sourceReadCompletions !== 2 ||
    bgiBrowserEvidence.sourceReadPendingAfterCleanup !== 0 ||
    bgiBrowserEvidence.callbackEntries !== 19 ||
    bgiBrowserEvidence.callbackReplacements !== 10 ||
    bgiBrowserEvidence.maximumCallbackDepth !== 3 ||
    bgiBrowserEvidence.logicalWindowCompletions !== 5 ||
    bgiBrowserEvidence.logicalWindowHandle !== "0x48010001" ||
    bgiBrowserEvidence.logicalWindowFinalLifecycle !== "created" ||
    bgiBrowserEvidence.logicalWindowRequestedStyle !== "0x80ca0000" ||
    bgiBrowserEvidence.logicalWindowEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.logicalWindowVisible !== true ||
    bgiBrowserEvidence.logicalWindowTopmost !== false ||
    bgiBrowserEvidence.browserOwnsLogicalWindowState !== false ||
    bgiBrowserEvidence.monitorFromWindowCompletions !== 2 ||
    bgiBrowserEvidence.monitorFromWindowSequence !== 17_842 ||
    bgiBrowserEvidence.monitorFromWindowArguments?.join(",") !==
      "0x48010001,0x00000001" ||
    bgiBrowserEvidence.monitorFromWindowResult !== "0x4f010001" ||
    bgiBrowserEvidence.monitorFromWindowLastError !== 0 ||
    bgiBrowserEvidence.monitorFromWindowGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.monitorFromWindowImplementationId !==
      "user32.monitor-from-window.v1" ||
    bgiBrowserEvidence.monitorFromWindowDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    bgiBrowserEvidence.monitorFromWindowMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    bgiBrowserEvidence.monitorFromWindowOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.monitorFromWindowStateAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    bgiBrowserEvidence.monitorFromWindowWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    bgiBrowserEvidence.monitorFromWindowInvalidWindowResult !== "0x00000000" ||
    bgiBrowserEvidence.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    bgiBrowserEvidence.monitorFromWindowUnsupportedFlagsFault !==
      "monitor-from-window-flags-unsupported" ||
    bgiBrowserEvidence.monitorFromWindowFailureAtomicity !== true ||
    bgiBrowserEvidence.monitorFromWindowHostDisplayTopologyImported !== false ||
    bgiBrowserEvidence.monitorFromWindowBrowserDisplayTopologyImported !== false ||
    bgiBrowserEvidence.d3d9GetAdapterCountCompletions !== 2 ||
    bgiBrowserEvidence.d3d9GetAdapterCountSequence !== 17_843 ||
    bgiBrowserEvidence.d3d9GetAdapterCountArguments?.join(",") !==
      "0x01000004" ||
    bgiBrowserEvidence.d3d9GetAdapterCountResult !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterCountGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9GetAdapterCountReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterCountObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterCountOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorCompletions !== 2 ||
    bgiD3d9AdapterMonitorSequences.join(",") !== "17844,17848" ||
    JSON.stringify(bgiBrowserEvidence.d3d9GetAdapterMonitorArguments) !==
      JSON.stringify([["0x01000004", "0x00000000"], ["0x01000004", "0x00000000"]]) ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorResults?.join(",") !==
      "0x4f010001,0x4f010001" ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorIdentityMatchesMonitorFromWindow !== true ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeCompletions !== 2 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeSequence !== 17_817 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeArguments?.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierSequence !== 17_818 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOutputBytes !== 1_100 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOutputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetDeviceCapsCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsSequence !== 17_819 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOutputBytes !== 304 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOutputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeCompletions !== 2 ||
    bgiD3d9CheckDeviceTypeSequences.join(",") !== "17820,17821" ||
    JSON.stringify(bgiBrowserEvidence.d3d9CheckDeviceTypeArguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000001"],
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000000"],
      ]) ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeResults?.join(",") !==
      "0x00000000,0x00000000" ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatSequence !== 17_822 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateDeviceCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceSequence !== 17_849 ||
    bgiBrowserEvidence.d3d9CreateDeviceArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    bgiBrowserEvidence.d3d9CreateDeviceResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateDeviceLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateDeviceGuestWriteBytes !== 16 ||
    bgiBrowserEvidence.d3d9CreateDevicePresentationPrefixHex !==
      "200300005802000016000000" ||
    bgiBrowserEvidence.d3d9CreateDevicePointer !== "0x01010004" ||
    bgiBrowserEvidence.d3d9CreateDeviceObjectId !== 3 ||
    bgiBrowserEvidence.d3d9CreateDeviceInterfaceId !== 6 ||
    bgiBrowserEvidence.d3d9CreateDeviceRootReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9CreateDeviceReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateDeviceLogicalGraphicsDeviceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateDeviceOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateTextureCompletions !== 3 ||
    bgiBrowserEvidence.d3d9CreateTextureSequence !== 17_850 ||
    bgiBrowserEvidence.d3d9CreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    bgiBrowserEvidence.d3d9CreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9CreateTexturePointer !== "0x01020004" ||
    bgiBrowserEvidence.d3d9CreateTextureObjectId !== 4 ||
    bgiBrowserEvidence.d3d9CreateTextureInterfaceId !== 8 ||
    bgiBrowserEvidence.d3d9CreateTextureDeviceReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9CreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsDeviceId !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsTextureId !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsDeviceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsTextureCount !== 3 ||
    bgiBrowserEvidence.d3d9CreateTextureBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateTextureOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelSequence !== 17_851 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelArguments?.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLastError !== 0 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelPointer !== "0x01030004" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelObjectId !== 5 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelInterfaceId !== 10 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelTextureReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsTextureId !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceGetDcCompletions !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSequence !== 17_852 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcArguments?.join(",") !==
      "0x01030004,0x006ffd3c" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcOutputAddress !== "0x006ffd3c" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcHandle !== "0x47010002" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcDeviceContextId !== 2 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcHandleNamespace !==
      "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcTargetKind !== "graphics-surface" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSurfaceComObjectId !== 5 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSurfaceGeometry?.join(",") !==
      "1024,1024" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcColorFormat !== "b8g8r8x8-unorm" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcObjectRetired !== false ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceGetStockObjectSequence !== 17_853 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectSequence !== 17_854 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectArguments?.join(",") !==
      "0x47010002,0x006ffd4c,0x47020001" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectResult !== "0x00000001" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectRect?.join(",") !== "0,0,800,600" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectColorRef !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectContentGeneration !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectSurfaceGeneration !== 4 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcCompletions !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcSequence !== 17_855 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcArguments?.join(",") !==
      "0x01030004,0x47010002" ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcLiveDeviceContexts !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseSequence !== 17_856 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseResult !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseReferenceCount !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseObjectRetired !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseLogicalGraphicsSurfaceCount !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SecondCreateTextureSequence !== 17_857 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000000,0x101517d0,0x00000000" ||
    bgiBrowserEvidence.d3d9SecondCreateTexturePool !== 0 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SecondCreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9SecondCreateTexturePointer !== "0x01030004" ||
    bgiBrowserEvidence.d3d9SecondCreateTextureDeviceReferenceCount !== 3 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureSequence !== 17_858 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000200,0x00000016,0x00000000,0x101517d4,0x00000000" ||
    bgiBrowserEvidence.d3d9ThirdCreateTexturePool !== 0 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureUsage !== 512 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9ThirdCreateTexturePointer !== "0x01040004" ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureDeviceReferenceCount !== 4 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateVertexBufferCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferSequence !== 17_859 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferInstructionCount !== 11_807_897 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferArguments?.join(",") !==
      "0x01010004,0x00000070,0x00000008,0x00000144,0x00000001,0x101517dc,0x00000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferPointer !== "0x01050004" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferDeviceReferenceCountAtCompletion !== 4 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsDeviceId !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsVertexBufferId !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsVertexBufferCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBackingAddress !== "0x44000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBackingBytes !== 112 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferInitialContentSha256 !==
      "b5fdab78d8947eacc864bfeecb4d2100780e5afe1cd8efafb124887913ac49fa" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateVertexBufferOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9VertexBufferLockCompletions !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferLockSequence !== 17_860 ||
    bgiBrowserEvidence.d3d9VertexBufferLockInstructionCount !== 11_808_429 ||
    bgiBrowserEvidence.d3d9VertexBufferLockArguments?.join(",") !==
      "0x01050004,0x00000000,0x00000000,0x006ffd54,0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockLastError !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferLockGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9VertexBufferLockDataPointer !== "0x44000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferLockOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockCompletions !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockSequence !== 17_861 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockInstructionCount !== 11_808_439 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockArguments?.join(",") !==
      "0x01050004" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockLastError !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockContentGeneration !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockContentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockBackingProtection !== "read-only" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SetRenderStateCompletions !== 4 ||
    bgiBrowserEvidence.d3d9SetRenderStateSequences?.join(",") !==
      "17862,17863,17864,17865" ||
    bgiBrowserEvidence.d3d9SetRenderStateInstructionCounts?.join(",") !==
      "11808458,11808466,11808474,11808482" ||
    JSON.stringify(bgiBrowserEvidence.d3d9SetRenderStateArguments) !==
      JSON.stringify([
        ["0x01010004", "0x00000016", "0x00000001"],
        ["0x01010004", "0x00000007", "0x00000000"],
        ["0x01010004", "0x00000089", "0x00000000"],
        ["0x01010004", "0x0000001b", "0x00000001"],
      ]) ||
    bgiBrowserEvidence.d3d9SetRenderStateResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    bgiBrowserEvidence.d3d9SetRenderStateReturnAddresses?.join(",") !==
      "0x1005ad13,0x1005ad26,0x1005ad3c,0x1005ad50" ||
    bgiBrowserEvidence.d3d9SetRenderStateGuestWriteBytes?.join(",") !==
      "0,0,0,0" ||
    bgiBrowserEvidence.d3d9SetRenderStateDeviceReferenceCount !== 5 ||
    bgiBrowserEvidence.d3d9SetRenderStateLogicalValues?.join(",") !==
      "cull-none,depth-test-disabled,lighting-disabled,alpha-blend-enabled" ||
    bgiBrowserEvidence.d3d9SetRenderStateGenerationDelta !== 4 ||
    bgiBrowserEvidence.d3d9SetRenderStateBrowserOwnsState !== false ||
    bgiBrowserEvidence.d3d9SetRenderStateBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9SetRenderStateOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.loadLibraryACompletions !== 1 ||
    bgiBrowserEvidence.loadLibraryASequence !== 17_823 ||
    bgiBrowserEvidence.loadLibraryAArguments?.join(",") !== "0x100dc9c8" ||
    bgiBrowserEvidence.loadLibraryAResult !== "0x4d010010" ||
    bgiBrowserEvidence.loadLibraryALastError !== 0 ||
    bgiBrowserEvidence.loadLibraryAGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.loadLibraryAModuleId !== 16 ||
    bgiBrowserEvidence.loadLibraryAModuleClass !== "runtime-system-module" ||
    bgiBrowserEvidence.loadLibraryAModuleState !== "process-attached" ||
    bgiBrowserEvidence.loadLibraryAAttachSequence !== 16 ||
    bgiBrowserEvidence.loadLibraryAObjectId !== 23 ||
    bgiBrowserEvidence.loadLibraryAThreadReferenceCount !== 1 ||
    bgiBrowserEvidence.loadLibraryAOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3dxGetProcAddressCompletions !== 1 ||
    bgiBrowserEvidence.d3dxGetProcAddressSequence !== 17_824 ||
    bgiBrowserEvidence.d3dxGetProcAddressArguments?.join(",") !==
      "0x4d010010,0x100dc9d8" ||
    bgiBrowserEvidence.d3dxGetProcAddressResult !== "0x7f020040" ||
    bgiBrowserEvidence.d3dxGetProcAddressLastError !== 0 ||
    bgiBrowserEvidence.d3dxGetProcAddressOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getClientRectCompletions !== 1 ||
    bgiBrowserEvidence.getClientRectSequence !== 17_825 ||
    bgiBrowserEvidence.getClientRectArguments?.join(",") !==
      "0x48010001,0x006ffdc4" ||
    bgiBrowserEvidence.getClientRectResult !== "0x00000001" ||
    bgiBrowserEvidence.getClientRectLastError !== 0 ||
    bgiBrowserEvidence.getClientRectGuestWriteBytes !== 16 ||
    bgiBrowserEvidence.getClientRectOutputHex !==
      "00000000000000002003000058020000" ||
    bgiBrowserEvidence.getClientRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getDcCompletions !== 1 ||
    bgiBrowserEvidence.getDcSequence !== 17_826 ||
    bgiBrowserEvidence.getDcArguments?.join(",") !== "0x48010001" ||
    bgiBrowserEvidence.getDcResult !== "0x47010001" ||
    bgiBrowserEvidence.getDcLastError !== 0 ||
    bgiBrowserEvidence.getDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.getDcDeviceContextId !== 1 ||
    bgiBrowserEvidence.getDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.getDcSurfaceId !== 1 ||
    bgiBrowserEvidence.getDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.getDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getStockObjectCompletions !== 2 ||
    bgiBrowserEvidence.getStockObjectSequence !== 17_827 ||
    bgiBrowserEvidence.getStockObjectArguments?.join(",") !== "0x00000004" ||
    bgiBrowserEvidence.getStockObjectResult !== "0x47020001" ||
    bgiBrowserEvidence.getStockObjectLastError !== 0 ||
    bgiBrowserEvidence.getStockObjectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.getStockObjectId !== 1 ||
    bgiBrowserEvidence.getStockObjectHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    bgiBrowserEvidence.getStockObjectColorRef !== 0 ||
    bgiBrowserEvidence.getStockObjectLiveObjects !== 1 ||
    bgiBrowserEvidence.getStockObjectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.fillRectCompletions !== 2 ||
    bgiBrowserEvidence.fillRectSequence !== 17_828 ||
    bgiBrowserEvidence.fillRectArguments?.join(",") !==
      "0x47010001,0x006ffdc4,0x47020001" ||
    bgiBrowserEvidence.fillRectResult !== "0x00000001" ||
    bgiBrowserEvidence.fillRectLastError !== 0 ||
    bgiBrowserEvidence.fillRectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.fillRectSurfaceId !== 1 ||
    bgiBrowserEvidence.fillRectSurfaceWidth !== 800 ||
    bgiBrowserEvidence.fillRectSurfaceHeight !== 600 ||
    bgiBrowserEvidence.fillRectSurfaceGeneration !== 1 ||
    bgiBrowserEvidence.fillRectFillRect?.join(",") !== "0,0,800,600" ||
    bgiBrowserEvidence.fillRectColorRef !== 0 ||
    bgiBrowserEvidence.fillRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.releaseDcCompletions !== 1 ||
    bgiBrowserEvidence.releaseDcSequence !== 17_829 ||
    bgiBrowserEvidence.releaseDcArguments?.join(",") !==
      "0x48010001,0x47010001" ||
    bgiBrowserEvidence.releaseDcResult !== "0x00000001" ||
    bgiBrowserEvidence.releaseDcLastError !== 0 ||
    bgiBrowserEvidence.releaseDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.releaseDcLiveDeviceContexts !== 0 ||
    bgiBrowserEvidence.releaseDcLiveSurfaces !== 1 ||
    bgiBrowserEvidence.releaseDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.setWindowLongACompletions !== 1 ||
    bgiBrowserEvidence.setWindowLongASequence !== 17_830 ||
    bgiBrowserEvidence.setWindowLongAArguments?.join(",") !==
      "0x48010001,0xfffffff0,0x90ca0000" ||
    bgiBrowserEvidence.setWindowLongAResult !== "0x84ca0000" ||
    bgiBrowserEvidence.setWindowLongALastError !== 0 ||
    bgiBrowserEvidence.setWindowLongAGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.setWindowLongAPreviousEffectiveStyle !==
      "0x84ca0000" ||
    bgiBrowserEvidence.setWindowLongAEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.setWindowLongAVisible !== true ||
    bgiBrowserEvidence.setWindowLongATopmost !== false ||
    bgiBrowserEvidence.setWindowLongAOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.setWindowPosActivationCompletions !== 1 ||
    bgiBrowserEvidence.setWindowPosActivationSequence !== 17_831 ||
    bgiBrowserEvidence.setWindowPosActivationArguments?.join(",") !==
      "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000326,0x00000275,0x0000010a" ||
    bgiBrowserEvidence.setWindowPosActivationResult !== "0x00000001" ||
    bgiBrowserEvidence.setWindowPosActivationLastError !== 0 ||
    bgiBrowserEvidence.setWindowPosActivationGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.setWindowPosActivationEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.setWindowPosActivationGeometry?.join(",") !==
      "15360,8640,806,629,15363,8666,800,600" ||
    bgiBrowserEvidence.setWindowPosActivationVisible !== true ||
    bgiBrowserEvidence.setWindowPosActivationTopmost !== false ||
    bgiBrowserEvidence.setWindowPosActivationMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    bgiBrowserEvidence.setWindowPosActivationMessageResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    bgiBrowserEvidence.setWindowPosActivationActiveWindow !== "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationFocusWindow !== "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationForegroundWindow !== "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.timeGetTimeCompletions !== 0 ||
    bgiBrowserEvidence.timeGetTimeReached !== false ||
    bgiBrowserEvidence.timeGetTimeImplementationId !==
      "winmm.time-get-time.v1" ||
    bgiBrowserEvidence.timeGetTimePolicyProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    bgiBrowserEvidence.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    bgiBrowserEvidence.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    bgiBrowserEvidence.timeGetTimeIncrementPerRead !== 1 ||
    bgiBrowserEvidence.timeGetTimeTimerResolutionAffectsClock !== false ||
    bgiBrowserEvidence.timeGetTimeHostClockImported !== false ||
    bgiBrowserEvidence.timeGetTimeBrowserClockImported !== false ||
    bgiBrowserEvidence.timeGetTimeNodeChromeReachabilityMatched !== true ||
    bgiBrowserEvidence.browserOwnsGdiState !== false ||
    bgiBrowserEvidence.runtimeGraphicsCreatedByWindowDcSlice !== false ||
    bgiBrowserEvidence.browserRendererResourceCreatedByWindowDcSlice !== false ||
    bgiBrowserEvidence.runtimeGraphicsSurfaceFilledBySurfaceDcSlice !== true ||
    bgiBrowserEvidence.browserRendererResourceCreatedBySurfaceDcSlice !== false ||
    bgiBrowserEvidence.graphicsSurfacePixelStorageCreatedBySurfaceDcSlice !== false
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
    "runtimeCoreWasmBytes",
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
    bgiBenchmarkEvidence.configuration !==
      "fixtures/benchmarks/bgi-browser-d3d9-initial-render-states-v1.json" ||
    bgiBenchmarkEvidence.runtimeProfile !== "d3d9-initial-render-states" ||
    bgiBenchmarkEvidence.observationStatus !==
      "current-d3d9-initial-render-states-observation" ||
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
    bgiBenchmarkEvidence.runtimeCoreWasmBytes !== browserRuntimeCoreWasm.bytes ||
    bgiBenchmarkEvidence.runtimeCoreWasmSha256 !== browserRuntimeCoreWasm.sha256 ||
    bgiBenchmarkEvidence.executionEngineLinearMemoryBytes !==
      bgiBrowserEvidence.executionEngineLinearMemoryBytes
  ) {
    errors.push("browser BGI benchmark evidence is invalid or inconsistent");
  }
  if (
    browser.bgiBenchmark === "threshold-blocked" ||
    browser.bgiBenchmarkBlocker !== undefined
  ) {
    const benchmarkBlocker = requireRecord(
      browser.bgiBenchmarkBlocker,
      "browserCheckpoint.bgiBenchmarkBlocker",
      errors,
    );
    for (const field of [
      "schema",
      "id",
      "kind",
      "reason",
      "status",
      "blockedWork",
      "exactReproduction",
      "requiredAuthority",
      "resumeCondition",
    ]) {
      requireString(
        benchmarkBlocker[field],
        `browserCheckpoint.bgiBenchmarkBlocker.${field}`,
        errors,
      );
    }
    requireInteger(
      benchmarkBlocker.schemaVersion,
      "browserCheckpoint.bgiBenchmarkBlocker.schemaVersion",
      errors,
    );
    const firstBenchmarkFailure = requireRecord(
      benchmarkBlocker.firstFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure",
      errors,
    );
    requireString(
      firstBenchmarkFailure.thresholdId,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.thresholdId",
      errors,
    );
    requireString(
      firstBenchmarkFailure.comparison,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.comparison",
      errors,
    );
    requireFiniteNumber(
      firstBenchmarkFailure.observed,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.observed",
      errors,
    );
    requireFiniteNumber(
      firstBenchmarkFailure.threshold,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.threshold",
      errors,
    );
    const confirmationBenchmarkFailure = requireRecord(
      benchmarkBlocker.confirmationFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure",
      errors,
    );
    requireString(
      confirmationBenchmarkFailure.thresholdId,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.thresholdId",
      errors,
    );
    requireString(
      confirmationBenchmarkFailure.comparison,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.comparison",
      errors,
    );
    requireFiniteNumber(
      confirmationBenchmarkFailure.observed,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.observed",
      errors,
    );
    requireFiniteNumber(
      confirmationBenchmarkFailure.threshold,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.threshold",
      errors,
    );
    const benchmarkBlockerEvidence = requireRecord(
      benchmarkBlocker.evidence,
      "browserCheckpoint.bgiBenchmarkBlocker.evidence",
      errors,
    );
    for (const field of [
      "firstBenchmarkReport",
      "confirmationBenchmarkReport",
      "nodeComparisonReport",
    ]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportBytes",
      "firstHeartbeatSamples",
      "firstHeartbeatSamplesOverThreshold",
      "firstHeartbeatSamplesOverThirtyMilliseconds",
      "confirmationBenchmarkReportBytes",
      "passingThresholds",
      "thresholdEvaluations",
      "confirmationCompletedSessions",
      "confirmationFailedSessions",
      "confirmationHeartbeatSamples",
      "confirmationHeartbeatSamplesOverThreshold",
      "confirmationHeartbeatSamplesOverThirtyMilliseconds",
      "nodeComparisonReportBytes",
      "logicalCpus",
    ]) {
      requireInteger(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstHeartbeatP99Milliseconds",
      "firstMaximumHeartbeatGapMilliseconds",
      "confirmationHeartbeatP99Milliseconds",
      "confirmationMaximumHeartbeatGapMilliseconds",
      "vertexBufferMedianMilliseconds",
      "renderStatesMedianMilliseconds",
      "nodeMedianIncreasePercent",
    ]) {
      requireFiniteNumber(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportSha256",
      "confirmationBenchmarkReportSha256",
      "nodeComparisonReportSha256",
    ]) {
      requireSha256(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    const hostQuiescenceObservation = requireRecord(
      benchmarkBlockerEvidence.hostQuiescenceObservation,
      "browserCheckpoint.bgiBenchmarkBlocker.evidence.hostQuiescenceObservation",
      errors,
    );
    for (const field of ["classification", "topProcess"]) {
      requireString(
        hostQuiescenceObservation[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.hostQuiescenceObservation.${field}`,
        errors,
      );
    }
    for (const field of [
      "durationMilliseconds",
      "samples",
      "samplesOver25Percent",
      "samplesOver50Percent",
    ]) {
      requireInteger(
        hostQuiescenceObservation[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.hostQuiescenceObservation.${field}`,
        errors,
      );
    }
    for (const field of [
      "meanCpuPercent",
      "p95CpuPercent",
      "maximumCpuPercent",
      "topProcessHostPercent",
    ]) {
      requireFiniteNumber(
        hostQuiescenceObservation[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.hostQuiescenceObservation.${field}`,
        errors,
      );
    }
    for (const field of ["host", "cpu", "chromeVersion", "nodeVersion"]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    const attemptedOptions = requireStringArray(
      benchmarkBlocker.attemptedOptions,
      "browserCheckpoint.bgiBenchmarkBlocker.attemptedOptions",
      errors,
    );
    const protectedInvariants = requireStringArray(
      benchmarkBlocker.protectedInvariants,
      "browserCheckpoint.bgiBenchmarkBlocker.protectedInvariants",
      errors,
    );
    if (
      browser.bgiBenchmark !== "threshold-blocked" ||
      benchmarkBlocker.schema !== "vine.structured-blocker" ||
      benchmarkBlocker.schemaVersion !== 2 ||
      benchmarkBlocker.id !==
        "d3d9-initial-render-states-heartbeat-threshold-v2" ||
      benchmarkBlocker.kind !== "feasibility-blocker" ||
      benchmarkBlocker.reason !==
        "browser-heartbeat-p99-repeats-above-versioned-threshold-on-quiescent-host-without-node-cost-regression" ||
      benchmarkBlocker.status !== "blocked" ||
      benchmarkBlocker.blockedWork !==
        "accept-d3d9-initial-render-states-and-advance-to-get-raster-status" ||
      benchmarkBlocker.exactReproduction !==
        "npm run check:runtime:bgi-browser-benchmark:prepared" ||
      firstBenchmarkFailure.thresholdId !== benchmarkFailureIds[0] ||
      firstBenchmarkFailure.observed !== 30.50500000268221 ||
      firstBenchmarkFailure.comparison !== "<=" ||
      firstBenchmarkFailure.threshold !== 25 ||
      confirmationBenchmarkFailure.thresholdId !== benchmarkFailureIds[0] ||
      confirmationBenchmarkFailure.observed !==
        bgiBenchmarkEvidence.heartbeatP99Milliseconds ||
      confirmationBenchmarkFailure.comparison !== "<=" ||
      confirmationBenchmarkFailure.threshold !== 25 ||
      benchmarkBlockerEvidence.firstBenchmarkReport !==
        ".work/browser-runtime/bgi-browser-benchmark-report.json" ||
      benchmarkBlockerEvidence.firstBenchmarkReportBytes !== 642_010 ||
      benchmarkBlockerEvidence.firstBenchmarkReportSha256 !==
        "68d66db730712da40f924defbd5669db55a97d40c3b095d00f1a27986ea8d4d9" ||
      benchmarkBlockerEvidence.firstHeartbeatSamples !== 24_487 ||
      benchmarkBlockerEvidence.firstHeartbeatSamplesOverThreshold !== 405 ||
      benchmarkBlockerEvidence.firstHeartbeatSamplesOverThirtyMilliseconds !== 260 ||
      benchmarkBlockerEvidence.firstHeartbeatP99Milliseconds !==
        firstBenchmarkFailure.observed ||
      benchmarkBlockerEvidence.firstMaximumHeartbeatGapMilliseconds !==
        43.96000000089407 ||
      benchmarkBlockerEvidence.confirmationBenchmarkReport !==
        ".work/browser-runtime/bgi-browser-benchmark-report-d3d9-initial-render-states-quiescent-r2.json" ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportBytes !==
        bgiBenchmarkEvidence.reportBytes ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportSha256 !==
        bgiBenchmarkEvidence.reportSha256 ||
      benchmarkBlockerEvidence.passingThresholds !==
        bgiBenchmarkEvidence.thresholdEvaluationCount - bgiBenchmarkEvidence.thresholdFailures ||
      benchmarkBlockerEvidence.thresholdEvaluations !==
        bgiBenchmarkEvidence.thresholdEvaluationCount ||
      benchmarkBlockerEvidence.confirmationCompletedSessions !==
        bgiBenchmarkEvidence.completedSessions ||
      benchmarkBlockerEvidence.confirmationFailedSessions !==
        bgiBenchmarkEvidence.failedSessions ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamples !== 23_815 ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamplesOverThreshold !== 601 ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamplesOverThirtyMilliseconds !== 411 ||
      benchmarkBlockerEvidence.confirmationHeartbeatP99Milliseconds !==
        bgiBenchmarkEvidence.heartbeatP99Milliseconds ||
      benchmarkBlockerEvidence.confirmationMaximumHeartbeatGapMilliseconds !==
        bgiBenchmarkEvidence.maximumHeartbeatGapMilliseconds ||
      hostQuiescenceObservation.classification !==
        "diagnostic-preflight-not-versioned-threshold" ||
      hostQuiescenceObservation.durationMilliseconds !== 18_499 ||
      hostQuiescenceObservation.samples !== 15 ||
      hostQuiescenceObservation.meanCpuPercent !== 12.267 ||
      hostQuiescenceObservation.p95CpuPercent !== 17 ||
      hostQuiescenceObservation.maximumCpuPercent !== 17 ||
      hostQuiescenceObservation.samplesOver25Percent !== 0 ||
      hostQuiescenceObservation.samplesOver50Percent !== 0 ||
      hostQuiescenceObservation.topProcess !== "codex" ||
      hostQuiescenceObservation.topProcessHostPercent !== 1.964 ||
      benchmarkBlockerEvidence.nodeComparisonReportBytes !== 1_108 ||
      benchmarkBlockerEvidence.nodeComparisonReportSha256 !==
        "7320265a1c826761d751c93e6f7fa30ad228444db4441348f2e3993e749b32fb" ||
      benchmarkBlockerEvidence.vertexBufferMedianMilliseconds !== 29_258.362198 ||
      benchmarkBlockerEvidence.renderStatesMedianMilliseconds !== 29_105.954998 ||
      benchmarkBlockerEvidence.nodeMedianIncreasePercent !== -0.5209013374317206 ||
      benchmarkBlockerEvidence.host !== "win32 10.0.26200 x64" ||
      benchmarkBlockerEvidence.cpu !==
        "AMD Ryzen 7 8840U w/ Radeon 780M Graphics" ||
      benchmarkBlockerEvidence.logicalCpus !== 16 ||
      benchmarkBlockerEvidence.chromeVersion !== bgiBenchmarkEvidence.chromeVersion ||
      benchmarkBlockerEvidence.nodeVersion !== "v24.18.0" ||
      attemptedOptions.length < 5 ||
      protectedInvariants.length < 3 ||
      benchmarkBlocker.requiredAuthority !==
        "profiled-browser-scheduling-remediation-or-versioned-performance-contract-decision" ||
      !benchmarkBlocker.resumeCondition.includes("unchanged 180-second gate") ||
      !benchmarkBlocker.resumeCondition.includes("GetRasterStatus")
    ) {
      errors.push("browser BGI structured performance blocker is invalid");
    }
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
  if (
    active.runtimeWindowActivationScope !==
    "same-owner-thread-single-window-same-geometry-active-focus-foreground-callback-lifecycle"
  ) {
    errors.push("activeRuntimeCheckpoint window activation scope is invalid");
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
      "process-state,process-address-space-allocator,threads,scheduler-and-waits,messages,displays,input,user32,gdi,graphics,timers,objects,com,heaps,files,module-records,synchronization,modules,registries" ||
    kernelBoundary.directMutableCollectionFields !== 0 ||
    kernelBoundary.directScalarAllocators !== 0 ||
    kernelBoundary.directSubsystemCollectionMutation !== false ||
    kernelBoundary.subsystemMutableCollectionVisibility !== "private" ||
    kernelBoundary.processStateOwner !==
      "runtime/process.rs::RuntimeProcessState" ||
    kernelBoundary.processMutableCollectionVisibility !== "private" ||
    processIndependentOwners.join(",") !==
      "displays,input,user32,gdi,graphics,timers,heaps,files,module-records,synchronization,com" ||
    kernelBoundary.processSerializationView !==
      "runtime/kernel.rs::RuntimeProcessView" ||
    kernelBoundary.processSerializationViewAccess !== "immutable-borrow" ||
    kernelBoundary.processLifecyclePlansRejectStaleCommit !== true ||
    kernelBoundary.closeHandleObjectPlan !==
      "runtime/objects.rs::RuntimeCloseHandleObjectMutationPlan" ||
    kernelBoundary.closeHandleObjectPlanVisibility !== "opaque-to-kernel" ||
    kernelBoundary.payloadObjectSubsystemRootCloneOrReplacementFromKernel !== false ||
    representativeSubsystemTests
      .filter(
        (test) =>
          !test.startsWith("runtime::gdi::") &&
          !test.startsWith("runtime::graphics::"),
      )
      .join(",") !==
      "runtime::displays::tests::configuration_is_single_owner_and_rejects_invalid_geometry_without_mutation,runtime::input::tests::configuration_and_swaps_are_planned_and_generation_tracks_only_changes,runtime::user32::tests::shared_icon_plans_reuse_identity_and_teardown_atomically,runtime::user32::tests::shared_cursor_identity_and_cross_resource_plans_are_atomic,runtime::user32::tests::shared_cursor_capacity_failure_preserves_identity_and_allocators,runtime::user32::tests::window_class_registration_is_module_scoped_case_folded_and_failure_atomic,runtime::user32::tests::invalid_window_class_metadata_and_atom_exhaustion_do_not_consume_identity,runtime::user32::tests::logical_window_creation_callbacks_are_ordered_and_abort_without_live_identity_leaks,runtime::user32::tests::small_icon_and_window_position_plans_are_atomic_and_reject_stale_commits,runtime::timers::tests::begin_end_pairing_counts_duplicates_and_rejects_stale_plans,runtime::timers::tests::capacity_overflow_and_teardown_are_failure_atomic,runtime::timers::tests::waitable_timer_creation_retirement_and_capacity_are_planned,runtime::heaps::tests::stale_or_duplicate_plans_preserve_all_heap_collections,runtime::files::tests::source_snapshot_paths_are_case_insensitive_and_mount_bounded,runtime::files::tests::read_plans_advance_cursor_only_after_matching_completion,runtime::modules::tests::dll_lifecycle_plan_commits_private_pending_callback_state,runtime::thread::tests::callback_and_exit_plans_commit_in_explicit_order,runtime::waits::tests::wake_plans_preserve_fifo_and_remove_only_during_commit,runtime::com::tests::stale_or_duplicate_plans_preserve_com_identity_and_refcounts" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::gdi::"))
      .join(",") !==
      "runtime::gdi::tests::device_context_acquisitions_are_distinct_and_release_is_relation_checked,runtime::gdi::tests::stock_brush_and_surface_fill_preserve_failure_atomicity,runtime::gdi::tests::device_context_serialization_preserves_window_shape_and_types_graphics_targets,runtime::gdi::tests::graphics_surface_dc_is_single_outstanding_and_guards_com_retirement,runtime::gdi::tests::graphics_surface_dc_capacity_failure_preserves_allocator_state" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::graphics::"))
      .join(",") !==
      "runtime::graphics::tests::creation_retirement_and_stale_plans_preserve_owner_invariants,runtime::graphics::tests::texture_creation_and_cascading_retirement_preserve_device_relation,runtime::graphics::tests::process_teardown_is_planned_and_rejects_stale_state,runtime::graphics::tests::device_capacity_failure_preserves_allocator_and_generation,runtime::graphics::tests::texture_capacity_failure_preserves_allocator_relations_and_generation,runtime::graphics::tests::surface_capacity_and_duplicate_level_failures_preserve_owner_state,runtime::graphics::tests::surface_fill_owns_content_history_and_rejects_stale_or_invalid_plans" ||
    representativeAtomicTests
      .filter(
        (test) =>
          test !==
            "user32_dispatcher::set_window_long_a_updates_effective_style_visibility_and_preserves_failures" &&
          test !==
            "user32_dispatcher::set_window_pos_same_geometry_activation_commits_ordered_focus_and_ime_callbacks" &&
          test !==
            "d3d9_dispatcher::create_device_rejects_invalid_or_unavailable_requests_without_partial_state" &&
          test !==
            "d3d9_dispatcher::d3d9_device_texture_and_surface_catalogs_fix_the_complete_x86_vtables" &&
          test !==
            "runtime::kernel::tests::d3d9_device_and_texture_commit_guest_com_and_graphics_state_as_one_lifetime",
      )
      .join(",") !==
      "runtime_heap_backing::private_heap_backing_is_atomic_reusable_and_process_owned,runtime_large_heap_materialization::large_heap_reserves_lazily_materializes_pages_and_reuses_its_permanent_slot,runtime_dll_lifecycle::guest_import_graph_drives_atomic_dll_process_lifecycle,runtime_system_module_load::dynamic_runtime_system_load_owns_identity_reference_and_attached_state,runtime_lifecycle::process_and_thread_lifecycle_separates_exit_request_from_waitable_completion,runtime_wait_queue::wait_queue_wakes_on_thread_signal_and_cancels_on_process_exit,com_iunknown_foundation::x86_iunknown_abi_owns_identity_refcounts_and_structured_stops,user32_dispatcher::callback_preparation_failures_preserve_memory_and_frame_allocator,user32_dispatcher::swap_mouse_button_policy_and_authority_failures_are_atomic,user32_dispatcher::load_icon_a_negative_paths_are_failure_atomic_and_do_not_consume_identity,user32_dispatcher::load_image_a_negative_variants_are_failure_atomic,user32_dispatcher::load_cursor_a_negative_variants_are_failure_atomic,user32_dispatcher::register_class_ex_a_negative_paths_are_failure_atomic_and_do_not_consume_atom,user32_dispatcher::get_system_metrics_rejects_unreached_or_inconsistent_profiles_without_mutation,user32_dispatcher::create_window_ex_a_preflight_and_callback_rejection_do_not_leave_live_windows,user32_dispatcher::set_window_pos_rejects_unreached_variants_and_invalid_state_atomically,user32_dispatcher::set_window_pos_callback_result_failures_do_not_publish_window_state,user32_dispatcher::invalid_menu_chain_rejects_unreached_variants_before_mutation,user32_dispatcher::window_dc_fill_slice_preserves_identity_lifetime_and_failure_atomicity,winmm_dispatcher::time_period_policy_and_lifecycle_failures_are_atomic,runtime_waitable_timer::waitable_timer_identity_wait_state_close_and_process_teardown_are_atomic,source_file_gateway::source_file_api_slice_opens_reads_seeks_and_closes_atomically,source_file_gateway::source_file_sharing_mount_and_pending_exit_paths_preserve_authority,d3d9_dispatcher::direct3d_create9_installs_bounded_root_identity_and_retires_atomically,d3d9_dispatcher::get_adapter_display_mode_reads_logical_geometry_and_writes_one_atomic_mode,d3d9_dispatcher::get_adapter_display_mode_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::get_adapter_identifier_writes_one_policy_owned_virtual_identity,d3d9_dispatcher::get_adapter_identifier_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::get_device_caps_writes_one_bounded_virtual_capability_profile,d3d9_dispatcher::get_device_caps_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::check_device_type_accepts_the_bounded_windowed_and_fullscreen_band,d3d9_dispatcher::check_device_type_failure_classes_preserve_memory_and_authority,d3d9_dispatcher::check_device_format_accepts_the_bounded_texture_usage_band,d3d9_dispatcher::check_device_format_failure_classes_preserve_memory_and_authority,d3d9_dispatcher::direct3d_create9_contract_and_resource_exhaustion_fail_before_publication" ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
          "user32_dispatcher::set_window_long_a_updates_effective_style_visibility_and_preserves_failures",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
          "user32_dispatcher::set_window_pos_same_geometry_activation_commits_ordered_focus_and_ime_callbacks",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
          "d3d9_dispatcher::create_device_rejects_invalid_or_unavailable_requests_without_partial_state",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
          "d3d9_dispatcher::d3d9_device_texture_and_surface_catalogs_fix_the_complete_x86_vtables",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
          "runtime::kernel::tests::d3d9_device_and_texture_commit_guest_com_and_graphics_state_as_one_lifetime",
    ).length !== 1 ||
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
    objectKinds.join(",") !== "process,thread,mutex,waitable-timer,file,private-heap,module" ||
    objects.sourceFileHandleNamespace !== "vine.runtime-source-file.v1" ||
    objects.sourceFileClosePlan !==
      "payload-last-handle-retirement-plan-preflight-commit" ||
    objects.sourceFilePendingReadCloseRejected !== true ||
    objects.sourceFileProcessExitCancelsBeforeRetirement !== true ||
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
  const concreteComInterfaces = requireStringArray(
    comFoundation.concreteInterfaces,
    "activeRuntimeCheckpoint.runtimeComFoundation.concreteInterfaces",
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
    comFoundation.endpointInstruction !== "ud2-vcom-interface-slot-marker" ||
    comFoundation.addressAllocationPurpose !== "com-endpoint" ||
    comFoundation.addressAllocationIdentity !== true ||
    comFoundation.queryInterfaceOutputAndRefcountAtomic !== true ||
    comFoundation.completionPreflightBeforeMutation !== true ||
    comFoundation.finalReleaseRetiresGuestMappingAndAllocation !== true ||
    comFoundation.childObjectRetainsParentInterfaceReference !== true ||
    comFoundation.childFinalReleaseCascadesParentRetirement !== true ||
    comFoundation.cascadeRetirementOrder !== "child-before-parent" ||
    comFoundation.crossSubsystemRetirementPreflight !==
      "com-address-space-guest-memory-runtime-graphics" ||
    comFoundation.lastErrorPolicy !== "preserve" ||
    comFoundation.unsupportedMethodPolicy !== "structured-stop-before-mutation" ||
    comFaultIdentityFields.join(",") !== "interface,vtable-slot,method,call-site" ||
    comFoundation.browserResourceStateOwner !==
      "browser/runtime-worker/browser-resource-table.ts" ||
    comFoundation.browserIdentityNamespace !==
      "browser-resource:<positive-safe-integer>" ||
    comFoundation.browserAssociationOwnsComReference !== false ||
    comFoundation.subsystemTests < 3 ||
    comFoundation.fixtureTests < 6 ||
    comFoundation.atomicFailureTests < 5 ||
    comFoundation.wholeRuntimeStateClone !== false ||
    comFoundation.concreteGraphicsOrAudioInterfaces !== true ||
    concreteComInterfaces.join(",") !==
      "IDirect3D9,IDirect3DDevice9,IDirect3DTexture9,IDirect3DSurface9" ||
    comFoundation.coInitializeApi !== "null-reserved-single-threaded-v1" ||
    comFoundation.coInitializeExApiImplemented !== false ||
    comFoundation.coUninitializeApiImplemented !== false ||
    comFoundation.classFactoryOrAutomationImplemented !== false
  ) {
    errors.push("active runtime COM foundation boundary is invalid");
  }
  const d3d9 = requireRecord(
    active.d3d9Frontend,
    "activeRuntimeCheckpoint.d3d9Frontend",
    errors,
  );
  const d3d9IUnknownMethods = requireStringArray(
    d3d9.iunknownMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.iunknownMethods",
    errors,
  );
  const d3d9ImplementedRootMethods = requireStringArray(
    d3d9.implementedRootMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.implementedRootMethods",
    errors,
  );
  const d3d9CompatibilityPolicy = requireRecord(
    d3d9.compatibilityPolicy,
    "activeRuntimeCheckpoint.d3d9Frontend.compatibilityPolicy",
    errors,
  );
  const d3d9AdapterCount = requireRecord(
    d3d9.getAdapterCount,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterCount",
    errors,
  );
  const d3d9NativeAdapterCount = requireRecord(
    d3d9.nativeAdapterCountEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterCountEvidence",
    errors,
  );
  const d3d9PrivateAdapterCount = requireRecord(
    d3d9.privateRuntimeAdapterCountEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterCountEvidence",
    errors,
  );
  const d3d9PrivateAdapterCountArguments = requireOrderedStringArray(
    d3d9PrivateAdapterCount.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterCountEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeAdapterCount.traceSha256, "nativeAdapterCountEvidence.traceSha256"],
    [d3d9NativeAdapterCount.summarySha256, "nativeAdapterCountEvidence.summarySha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9AdapterMonitor = requireRecord(
    d3d9.getAdapterMonitor,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterMonitor",
    errors,
  );
  const d3d9NativeAdapterMonitor = requireRecord(
    d3d9.nativeAdapterMonitorEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterMonitorEvidence",
    errors,
  );
  const d3d9NativeAdapterMonitorOracleAdapters = requireIntegerArray(
    d3d9NativeAdapterMonitor.oracleAdapters,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterMonitorEvidence.oracleAdapters",
    errors,
  );
  const d3d9PrivateAdapterMonitor = requireRecord(
    d3d9.privateRuntimeAdapterMonitorEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence",
    errors,
  );
  const d3d9PrivateAdapterMonitorNodeSequences = requireIntegerArray(
    d3d9PrivateAdapterMonitor.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateAdapterMonitorNodeInstructionCounts = requireIntegerArray(
    d3d9PrivateAdapterMonitor.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateAdapterMonitorChromeSequences = requireIntegerArray(
    d3d9PrivateAdapterMonitor.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.chromeSequences",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeAdapterMonitor.traceSha256, "nativeAdapterMonitorEvidence.traceSha256"],
    [d3d9NativeAdapterMonitor.summarySha256, "nativeAdapterMonitorEvidence.summarySha256"],
    [d3d9NativeAdapterMonitor.oracleSourceSha256, "nativeAdapterMonitorEvidence.oracleSourceSha256"],
    [d3d9NativeAdapterMonitor.oracleExecutableSha256, "nativeAdapterMonitorEvidence.oracleExecutableSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9DisplayMode = requireRecord(
    d3d9.getAdapterDisplayMode,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterDisplayMode",
    errors,
  );
  const d3d9NativeDisplayMode = requireRecord(
    d3d9.nativeDisplayModeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDisplayModeEvidence",
    errors,
  );
  const d3d9NativeDisplayModeCallerRvas = requireStringArray(
    d3d9NativeDisplayMode.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDisplayModeEvidence.callerRvas",
    errors,
  );
  const d3d9PrivateDisplayMode = requireRecord(
    d3d9.privateRuntimeDisplayModeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDisplayModeEvidence",
    errors,
  );
  const d3d9PrivateDisplayModeArguments = requireStringArray(
    d3d9PrivateDisplayMode.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDisplayModeEvidence.arguments",
    errors,
  );
  const d3d9AdapterIdentifier = requireRecord(
    d3d9.getAdapterIdentifier,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterIdentifier",
    errors,
  );
  const d3d9NativeAdapterIdentifier = requireRecord(
    d3d9.nativeAdapterIdentifierEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterIdentifierEvidence",
    errors,
  );
  const d3d9PrivateAdapterIdentifier = requireRecord(
    d3d9.privateRuntimeAdapterIdentifierEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence",
    errors,
  );
  const d3d9PrivateAdapterIdentifierArguments = requireOrderedStringArray(
    d3d9PrivateAdapterIdentifier.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence.arguments",
    errors,
  );
  for (const field of [
    "primaryTraceSha256",
    "confirmationTraceSha256",
    "inputSha256",
    "outputSha256",
  ]) {
    requireSha256(
      d3d9NativeAdapterIdentifier[field],
      `activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterIdentifierEvidence.${field}`,
      errors,
    );
  }
  requireSha256(
    d3d9PrivateAdapterIdentifier.outputSha256,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence.outputSha256",
    errors,
  );
  const d3d9DeviceCaps = requireRecord(
    d3d9.getDeviceCaps,
    "activeRuntimeCheckpoint.d3d9Frontend.getDeviceCaps",
    errors,
  );
  const d3d9NativeDeviceCaps = requireRecord(
    d3d9.nativeDeviceCapsEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedFields = requireStringArray(
    d3d9NativeDeviceCaps.consumedFields,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.consumedFields",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedFieldOffsets = requireIntegerArray(
    d3d9NativeDeviceCaps.consumedFieldOffsets,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.consumedFieldOffsets",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedValues = requireStringArray(
    d3d9NativeDeviceCaps.nativeConsumedValues,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.nativeConsumedValues",
    errors,
  );
  const d3d9PrivateDeviceCaps = requireRecord(
    d3d9.privateRuntimeDeviceCapsEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceCapsEvidence",
    errors,
  );
  const d3d9PrivateDeviceCapsArguments = requireOrderedStringArray(
    d3d9PrivateDeviceCaps.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceCapsEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeDeviceCaps.primaryTraceSha256, "nativeDeviceCapsEvidence.primaryTraceSha256"],
    [d3d9NativeDeviceCaps.confirmationTraceSha256, "nativeDeviceCapsEvidence.confirmationTraceSha256"],
    [d3d9NativeDeviceCaps.outputSha256, "nativeDeviceCapsEvidence.outputSha256"],
    [d3d9PrivateDeviceCaps.outputSha256, "privateRuntimeDeviceCapsEvidence.outputSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9CheckDeviceType = requireRecord(
    d3d9.checkDeviceType,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceType",
    errors,
  );
  const d3d9NativeDeviceType = requireRecord(
    d3d9.nativeDeviceTypeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence",
    errors,
  );
  const d3d9NativeDeviceTypeCallerRvas = requireStringArray(
    d3d9NativeDeviceType.directCallerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence.directCallerRvas",
    errors,
  );
  const d3d9NativeDeviceTypeWindowedValues = requireIntegerArray(
    d3d9NativeDeviceType.directWindowedValues,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence.directWindowedValues",
    errors,
  );
  const d3d9PrivateDeviceType = requireRecord(
    d3d9.privateRuntimeDeviceTypeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence",
    errors,
  );
  const d3d9PrivateDeviceTypeNodeSequences = requireIntegerArray(
    d3d9PrivateDeviceType.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateDeviceTypeInstructionCounts = requireIntegerArray(
    d3d9PrivateDeviceType.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateDeviceTypeChromeSequences = requireIntegerArray(
    d3d9PrivateDeviceType.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.chromeSequences",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeDeviceType.primaryTraceSha256, "nativeDeviceTypeEvidence.primaryTraceSha256"],
    [d3d9NativeDeviceType.confirmationTraceSha256, "nativeDeviceTypeEvidence.confirmationTraceSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9CheckDeviceFormat = requireRecord(
    d3d9.checkDeviceFormat,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat",
    errors,
  );
  const d3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    d3d9CheckDeviceFormat.supportedUsages,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat.supportedUsages",
    errors,
  );
  const d3d9CheckDeviceFormatSupportedUsageNames = requireStringArray(
    d3d9CheckDeviceFormat.supportedUsageNames,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat.supportedUsageNames",
    errors,
  );
  const d3d9NativeDeviceFormat = requireRecord(
    d3d9.nativeDeviceFormatEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceFormatEvidence",
    errors,
  );
  const d3d9NativeDeviceFormatSupportedUsages = requireIntegerArray(
    d3d9NativeDeviceFormat.supportedUsages,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceFormatEvidence.supportedUsages",
    errors,
  );
  const d3d9PrivateDeviceFormat = requireRecord(
    d3d9.privateRuntimeDeviceFormatEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceFormatEvidence",
    errors,
  );
  const d3d9PrivateDeviceFormatArguments = requireOrderedStringArray(
    d3d9PrivateDeviceFormat.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceFormatEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeDeviceFormat.primaryTraceSha256, "nativeDeviceFormatEvidence.primaryTraceSha256"],
    [d3d9NativeDeviceFormat.confirmationTraceSha256, "nativeDeviceFormatEvidence.confirmationTraceSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9CreateDevice = requireRecord(
    d3d9.createDevice,
    "activeRuntimeCheckpoint.d3d9Frontend.createDevice",
    errors,
  );
  const d3d9CreateDeviceBehaviorFlagNames = requireStringArray(
    d3d9CreateDevice.behaviorFlagNames,
    "activeRuntimeCheckpoint.d3d9Frontend.createDevice.behaviorFlagNames",
    errors,
  );
  const d3d9DeviceInterface = requireRecord(
    d3d9.deviceInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.deviceInterface",
    errors,
  );
  const d3d9DeviceImplementedMethods = requireStringArray(
    d3d9DeviceInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.deviceInterface.implementedMethods",
    errors,
  );
  const runtimeGraphics = requireRecord(
    d3d9.runtimeGraphics,
    "activeRuntimeCheckpoint.d3d9Frontend.runtimeGraphics",
    errors,
  );
  const d3d9NativeCreateDevice = requireRecord(
    d3d9.nativeCreateDeviceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateDeviceEvidence",
    errors,
  );
  const d3d9NativeCreateDeviceInput = requireOrderedIntegerArray(
    d3d9NativeCreateDevice.inputWidthHeightFormat,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateDeviceEvidence.inputWidthHeightFormat",
    errors,
  );
  const d3d9PrivateCreateDevice = requireRecord(
    d3d9.privateRuntimeCreateDeviceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence",
    errors,
  );
  const d3d9PrivateCreateDeviceArguments = requireOrderedStringArray(
    d3d9PrivateCreateDevice.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence.arguments",
    errors,
  );
  const d3d9PrivateLogicalPresentation = requireOrderedIntegerArray(
    d3d9PrivateCreateDevice.logicalPresentation,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence.logicalPresentation",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeCreateDevice.primaryTraceSha256, "nativeCreateDeviceEvidence.primaryTraceSha256"],
    [d3d9NativeCreateDevice.primarySummarySha256, "nativeCreateDeviceEvidence.primarySummarySha256"],
    [d3d9NativeCreateDevice.confirmationTraceSha256, "nativeCreateDeviceEvidence.confirmationTraceSha256"],
    [d3d9NativeCreateDevice.confirmationSummarySha256, "nativeCreateDeviceEvidence.confirmationSummarySha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9CreateTexture = requireRecord(
    d3d9.createTexture,
    "activeRuntimeCheckpoint.d3d9Frontend.createTexture",
    errors,
  );
  const d3d9CreateTextureVariants = Array.isArray(d3d9CreateTexture.variants)
    ? d3d9CreateTexture.variants.map((variant, index) =>
        requireRecord(
          variant,
          `activeRuntimeCheckpoint.d3d9Frontend.createTexture.variants[${index}]`,
          errors,
        ),
      )
    : (errors.push(
        "activeRuntimeCheckpoint.d3d9Frontend.createTexture.variants must be an array",
      ), []);
  const d3d9TextureInterface = requireRecord(
    d3d9.textureInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.textureInterface",
    errors,
  );
  const d3d9TextureImplementedMethods = requireStringArray(
    d3d9TextureInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.textureInterface.implementedMethods",
    errors,
  );
  const d3d9NativeCreateTexture = requireRecord(
    d3d9.nativeCreateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateTextureEvidence",
    errors,
  );
  const d3d9NativeCreateTextureArguments = requireOrderedIntegerArray(
    d3d9NativeCreateTexture.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateTextureEvidence.arguments",
    errors,
  );
  const d3d9NativeInitialTextures = requireRecord(
    d3d9.nativeInitialTextureSequenceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence",
    errors,
  );
  const d3d9NativeInitialTextureCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialTextures.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialTextureResults = requireOrderedStringArray(
    d3d9NativeInitialTextures.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.results",
    errors,
  );
  const d3d9NativeInitialTextureLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialTextures.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.lastErrors",
    errors,
  );
  const d3d9NativeInitialTextureOutputAddresses = requireOrderedStringArray(
    d3d9NativeInitialTextures.outputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.outputAddresses",
    errors,
  );
  const d3d9NativeInitialTextureNextArguments = requireOrderedIntegerArray(
    d3d9NativeInitialTextures.immediateNextArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.immediateNextArguments",
    errors,
  );
  const d3d9PrivateCreateTexture = requireRecord(
    d3d9.privateRuntimeCreateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence",
    errors,
  );
  const d3d9PrivateCreateTextureArguments = requireOrderedStringArray(
    d3d9PrivateCreateTexture.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence.arguments",
    errors,
  );
  const d3d9PrivateTextureDescriptor = requireOrderedIntegerArray(
    d3d9PrivateCreateTexture.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence.logicalDescriptor",
    errors,
  );
  const d3d9PrivateInitialTextures = requireRecord(
    d3d9.privateRuntimeInitialTextureSequenceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence",
    errors,
  );
  const d3d9PrivateInitialTextureNodeSequences = requireIntegerArray(
    d3d9PrivateInitialTextures.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialTextureInstructionCounts = requireIntegerArray(
    d3d9PrivateInitialTextures.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateInitialTextureChromeSequences = requireIntegerArray(
    d3d9PrivateInitialTextures.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateInitialTextureReturnAddresses = requireOrderedStringArray(
    d3d9PrivateInitialTextures.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateInitialTextureResults = requireOrderedStringArray(
    d3d9PrivateInitialTextures.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.results",
    errors,
  );
  const d3d9PrivateInitialTextureOutputAddresses = requireOrderedStringArray(
    d3d9PrivateInitialTextures.textureOutputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.textureOutputAddresses",
    errors,
  );
  const d3d9PrivateInitialTexturePointers = requireOrderedStringArray(
    d3d9PrivateInitialTextures.texturePointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.texturePointers",
    errors,
  );
  const d3d9PrivateInitialTextureGuestWrites = requireOrderedIntegerArray(
    d3d9PrivateInitialTextures.guestWriteBytes,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.guestWriteBytes",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeCreateTexture.primaryTraceSha256, "nativeCreateTextureEvidence.primaryTraceSha256"],
    [d3d9NativeCreateTexture.primarySummarySha256, "nativeCreateTextureEvidence.primarySummarySha256"],
    [d3d9NativeCreateTexture.confirmationTraceSha256, "nativeCreateTextureEvidence.confirmationTraceSha256"],
    [d3d9NativeCreateTexture.confirmationSummarySha256, "nativeCreateTextureEvidence.confirmationSummarySha256"],
    [d3d9NativeInitialTextures.primaryTraceSha256, "nativeInitialTextureSequenceEvidence.primaryTraceSha256"],
    [d3d9NativeInitialTextures.primarySummarySha256, "nativeInitialTextureSequenceEvidence.primarySummarySha256"],
    [d3d9NativeInitialTextures.confirmationTraceSha256, "nativeInitialTextureSequenceEvidence.confirmationTraceSha256"],
    [d3d9NativeInitialTextures.confirmationSummarySha256, "nativeInitialTextureSequenceEvidence.confirmationSummarySha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9CreateVertexBuffer = requireRecord(
    d3d9.createVertexBuffer,
    "activeRuntimeCheckpoint.d3d9Frontend.createVertexBuffer",
    errors,
  );
  const d3d9VertexBufferInterface = requireRecord(
    d3d9.vertexBufferInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferInterface",
    errors,
  );
  const d3d9VertexBufferImplementedMethods = requireStringArray(
    d3d9VertexBufferInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferInterface.implementedMethods",
    errors,
  );
  const d3d9VertexBufferLock = requireRecord(
    d3d9.vertexBufferLock,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferLock",
    errors,
  );
  const d3d9VertexBufferUnlock = requireRecord(
    d3d9.vertexBufferUnlock,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferUnlock",
    errors,
  );
  const d3d9NativeInitialVertexBuffer = requireRecord(
    d3d9.nativeInitialVertexBufferEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence",
    errors,
  );
  const d3d9NativeInitialVertexBufferCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialVertexBuffer.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialVertexBufferMethods = requireStringArray(
    d3d9NativeInitialVertexBuffer.methods,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.methods",
    errors,
  );
  const d3d9NativeInitialVertexBufferSlots = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.slots,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.slots",
    errors,
  );
  const d3d9NativeInitialVertexBufferCreateArguments = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.createArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.createArguments",
    errors,
  );
  const d3d9NativeInitialVertexBufferLockArguments = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.lockArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.lockArguments",
    errors,
  );
  const d3d9NativeInitialVertexBufferResults = requireOrderedStringArray(
    d3d9NativeInitialVertexBuffer.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.results",
    errors,
  );
  const d3d9NativeInitialVertexBufferLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.lastErrors",
    errors,
  );
  const d3d9NativeInitialVertexBufferNextArguments = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.immediateNextArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.immediateNextArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBuffer = requireRecord(
    d3d9.privateRuntimeInitialVertexBufferEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence",
    errors,
  );
  const d3d9PrivateInitialVertexBufferNodeSequences = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialVertexBufferInstructionCounts = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateInitialVertexBufferChromeSequences = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateInitialVertexBufferReturnAddresses = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateInitialVertexBufferCreateArguments = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.createArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.createArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBufferLockArguments = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.lockArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.lockArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBufferUnlockArguments = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.unlockArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.unlockArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBufferResults = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.results",
    errors,
  );
  const d3d9PrivateInitialVertexBufferDescriptor = requireOrderedIntegerArray(
    d3d9PrivateInitialVertexBuffer.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.logicalDescriptor",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeInitialVertexBuffer.primaryTraceSha256, "nativeInitialVertexBufferEvidence.primaryTraceSha256"],
    [d3d9NativeInitialVertexBuffer.primarySummarySha256, "nativeInitialVertexBufferEvidence.primarySummarySha256"],
    [d3d9NativeInitialVertexBuffer.confirmationTraceSha256, "nativeInitialVertexBufferEvidence.confirmationTraceSha256"],
    [d3d9NativeInitialVertexBuffer.confirmationSummarySha256, "nativeInitialVertexBufferEvidence.confirmationSummarySha256"],
    [d3d9NativeInitialVertexBuffer.initialContentSha256, "nativeInitialVertexBufferEvidence.initialContentSha256"],
    [d3d9NativeInitialVertexBuffer.writtenContentSha256, "nativeInitialVertexBufferEvidence.writtenContentSha256"],
    [d3d9PrivateInitialVertexBuffer.contentSha256, "privateRuntimeInitialVertexBufferEvidence.contentSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9SetRenderState = requireRecord(
    d3d9.setRenderState,
    "activeRuntimeCheckpoint.d3d9Frontend.setRenderState",
    errors,
  );
  const d3d9SetRenderStateSettings = Array.isArray(d3d9SetRenderState.orderedSettings)
    ? d3d9SetRenderState.orderedSettings
    : [];
  if (!Array.isArray(d3d9SetRenderState.orderedSettings)) {
    errors.push(
      "activeRuntimeCheckpoint.d3d9Frontend.setRenderState.orderedSettings must be an array",
    );
  }
  const d3d9NativeInitialRenderStates = requireRecord(
    d3d9.nativeInitialRenderStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence",
    errors,
  );
  const d3d9NativeInitialRenderStateCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialRenderStates.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialRenderStateStates = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.states,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.states",
    errors,
  );
  const d3d9NativeInitialRenderStateValues = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.values,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.values",
    errors,
  );
  const d3d9NativeInitialRenderStateResults = requireOrderedStringArray(
    d3d9NativeInitialRenderStates.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.results",
    errors,
  );
  const d3d9NativeInitialRenderStateLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.lastErrors",
    errors,
  );
  const d3d9PrivateInitialRenderStates = requireRecord(
    d3d9.privateRuntimeInitialRenderStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence",
    errors,
  );
  const d3d9PrivateInitialRenderStateNodeSequences = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialRenderStateInstructionCounts = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateInitialRenderStateChromeSequences = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateInitialRenderStateReturnAddresses = requireOrderedStringArray(
    d3d9PrivateInitialRenderStates.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateInitialRenderStateStates = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.states,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.states",
    errors,
  );
  const d3d9PrivateInitialRenderStateValues = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.values,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.values",
    errors,
  );
  const d3d9PrivateInitialRenderStateResults = requireOrderedStringArray(
    d3d9PrivateInitialRenderStates.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.results",
    errors,
  );
  const d3d9PrivateInitialRenderStateGuestWrites = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.guestWriteBytes,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.guestWriteBytes",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeInitialRenderStates.primaryTraceSha256, "nativeInitialRenderStateEvidence.primaryTraceSha256"],
    [d3d9NativeInitialRenderStates.primarySummarySha256, "nativeInitialRenderStateEvidence.primarySummarySha256"],
    [d3d9NativeInitialRenderStates.confirmationTraceSha256, "nativeInitialRenderStateEvidence.confirmationTraceSha256"],
    [d3d9NativeInitialRenderStates.confirmationSummarySha256, "nativeInitialRenderStateEvidence.confirmationSummarySha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9GetSurfaceLevel = requireRecord(
    d3d9.getSurfaceLevel,
    "activeRuntimeCheckpoint.d3d9Frontend.getSurfaceLevel",
    errors,
  );
  const d3d9SurfaceInterface = requireRecord(
    d3d9.surfaceInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceInterface",
    errors,
  );
  const d3d9SurfaceImplementedMethods = requireStringArray(
    d3d9SurfaceInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceInterface.implementedMethods",
    errors,
  );
  const d3d9NativeGetSurfaceLevel = requireRecord(
    d3d9.nativeGetSurfaceLevelEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeGetSurfaceLevelEvidence",
    errors,
  );
  const d3d9ControlledGetSurfaceLevel = requireRecord(
    d3d9.controlledGetSurfaceLevelOracle,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledGetSurfaceLevelOracle",
    errors,
  );
  const d3d9PrivateGetSurfaceLevel = requireRecord(
    d3d9.privateRuntimeGetSurfaceLevelEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence",
    errors,
  );
  const d3d9PrivateGetSurfaceLevelArguments = requireOrderedStringArray(
    d3d9PrivateGetSurfaceLevel.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence.arguments",
    errors,
  );
  const d3d9PrivateSurfaceDescriptor = requireOrderedIntegerArray(
    d3d9PrivateGetSurfaceLevel.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence.logicalDescriptor",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeGetSurfaceLevel.primaryTraceSha256, "nativeGetSurfaceLevelEvidence.primaryTraceSha256"],
    [d3d9NativeGetSurfaceLevel.primarySummarySha256, "nativeGetSurfaceLevelEvidence.primarySummarySha256"],
    [d3d9NativeGetSurfaceLevel.confirmationTraceSha256, "nativeGetSurfaceLevelEvidence.confirmationTraceSha256"],
    [d3d9NativeGetSurfaceLevel.confirmationSummarySha256, "nativeGetSurfaceLevelEvidence.confirmationSummarySha256"],
    [d3d9ControlledGetSurfaceLevel.sourceSha256, "controlledGetSurfaceLevelOracle.sourceSha256"],
    [d3d9ControlledGetSurfaceLevel.executableSha256, "controlledGetSurfaceLevelOracle.executableSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  const d3d9SurfaceDc = requireRecord(
    d3d9.surfaceDc,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceDc",
    errors,
  );
  const d3d9NativeSurfaceDc = requireRecord(
    d3d9.nativeSurfaceDcEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence",
    errors,
  );
  const d3d9ControlledSurfaceDc = requireRecord(
    d3d9.controlledSurfaceDcOracle,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledSurfaceDcOracle",
    errors,
  );
  const d3d9PrivateSurfaceDc = requireRecord(
    d3d9.privateRuntimeSurfaceDcEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence",
    errors,
  );
  const d3d9NativeSurfaceDcCallerRvas = requireOrderedStringArray(
    d3d9NativeSurfaceDc.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.callerRvas",
    errors,
  );
  const d3d9NativeSurfaceDcMethods = requireStringArray(
    d3d9NativeSurfaceDc.orderedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.orderedMethods",
    errors,
  );
  const d3d9NativeSurfaceDcRect = requireOrderedIntegerArray(
    d3d9NativeSurfaceDc.rect,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.rect",
    errors,
  );
  const d3d9NativeSurfaceDcLastErrors = requireOrderedIntegerArray(
    d3d9NativeSurfaceDc.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.lastErrors",
    errors,
  );
  const d3d9ControlledSurfaceDcReferenceCounts = requireOrderedIntegerArray(
    d3d9ControlledSurfaceDc.surfaceReferenceCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledSurfaceDcOracle.surfaceReferenceCounts",
    errors,
  );
  const d3d9PrivateSurfaceDcNodeSequences = requireIntegerArray(
    d3d9PrivateSurfaceDc.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateSurfaceDcInstructionCounts = requireIntegerArray(
    d3d9PrivateSurfaceDc.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateSurfaceDcChromeSequences = requireIntegerArray(
    d3d9PrivateSurfaceDc.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateSurfaceDcReturnAddresses = requireOrderedStringArray(
    d3d9PrivateSurfaceDc.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateSurfaceDcGeometry = requireOrderedIntegerArray(
    d3d9PrivateSurfaceDc.surfaceGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.surfaceGeometry",
    errors,
  );
  const d3d9PrivateSurfaceDcFillRect = requireOrderedIntegerArray(
    d3d9PrivateSurfaceDc.fillRect,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.fillRect",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeSurfaceDc.primaryTraceSha256, "nativeSurfaceDcEvidence.primaryTraceSha256"],
    [d3d9NativeSurfaceDc.primarySummarySha256, "nativeSurfaceDcEvidence.primarySummarySha256"],
    [d3d9NativeSurfaceDc.confirmationTraceSha256, "nativeSurfaceDcEvidence.confirmationTraceSha256"],
    [d3d9NativeSurfaceDc.confirmationSummarySha256, "nativeSurfaceDcEvidence.confirmationSummarySha256"],
    [d3d9ControlledSurfaceDc.sourceSha256, "controlledSurfaceDcOracle.sourceSha256"],
    [d3d9ControlledSurfaceDc.executableSha256, "controlledSurfaceDcOracle.executableSha256"],
    [d3d9ControlledSurfaceDc.nullOutputExecutableSha256, "controlledSurfaceDcOracle.nullOutputExecutableSha256"],
    [d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256, "controlledSurfaceDcOracle.readOnlyOutputExecutableSha256"],
  ]) {
    requireSha256(value, `activeRuntimeCheckpoint.d3d9Frontend.${label}`, errors);
  }
  if (
    d3d9.schema !== "vine.d3d9-frontend.v1" ||
    d3d9.status !== "initial-render-state-gateway-verified" ||
    d3d9.stateOwner !== "crates/vine-core/src/d3d9.rs" ||
    d3d9.comAuthority !== "runtime/com.rs::RuntimeCom" ||
    d3d9.sdkVersion !== 32 ||
    d3d9.factoryImplementationId !== "d3d9.direct3d-create9.v1" ||
    d3d9.rootClassIdentity !== "d3d9.idirect3d9-root.v1" ||
    d3d9.rootInterface !== "IDirect3D9" ||
    d3d9.rootInterfaceSlots !== 17 ||
    d3d9.rootInitialReferenceCount !== 1 ||
    d3d9.invalidSdkReturnsNullBeforeMutation !== true ||
    d3d9IUnknownMethods.join(",") !== "QueryInterface,AddRef,Release" ||
    d3d9ImplementedRootMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetAdapterCount,GetAdapterIdentifier,GetAdapterDisplayMode,GetDeviceCaps,GetAdapterMonitor,CheckDeviceType,CheckDeviceFormat,CreateDevice" ||
    d3d9.backingPages !== 2 ||
    d3d9.dataPageFinalProtection !== "read-only" ||
    d3d9.endpointPageFinalProtection !== "read-execute" ||
    d3d9.workerComResolutionAuthority !== "rust-live-endpoint-index" ||
    d3d9.browserOwnsComState !== false ||
    d3d9.versionSpecificFrontendBoundary !== true ||
    d3d9.frontendDispatchBoundary !==
      "version-neutral-com-gateway-to-versioned-d3d9-handler" ||
    d3d9CompatibilityPolicy.schema !== "vine.d3d9-compatibility-policy" ||
    d3d9CompatibilityPolicy.schemaVersion !== 14 ||
    d3d9CompatibilityPolicy.id !== "d3d9-primary-virtual-adapter-v14" ||
    d3d9CompatibilityPolicy.displayProfileId !==
      "single-monitor-1920x1080-v1" ||
    d3d9CompatibilityPolicy.surfaceDcProfileId !==
      "primary-surface-gdi-dc-v1" ||
    d3d9AdapterCount.slot !== 4 ||
    d3d9AdapterCount.explicitArgumentWords !== 0 ||
    d3d9AdapterCount.returnKind !== "u32" ||
    d3d9AdapterCount.profileId !== "single-logical-adapter-v1" ||
    d3d9AdapterCount.adapterCount !== 1 ||
    d3d9AdapterCount.countAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterCount.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterCount.displayProfileId !== "single-monitor-1920x1080-v1" ||
    d3d9AdapterCount.guestWriteBytes !== 0 ||
    d3d9AdapterCount.referenceCountMutation !== false ||
    d3d9AdapterCount.lastErrorPolicy !== "preserve" ||
    d3d9AdapterCount.hostGpuTopologyImported !== false ||
    d3d9AdapterCount.browserGpuTopologyImported !== false ||
    d3d9NativeAdapterCount.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterCount.traceBytes !== 2_267_972 ||
    d3d9NativeAdapterCount.traceLines !== 7_133 ||
    d3d9NativeAdapterCount.traceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    d3d9NativeAdapterCount.summaryBytes !== 58_021 ||
    d3d9NativeAdapterCount.summarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    d3d9NativeAdapterCount.diagnostics !== 0 ||
    d3d9NativeAdapterCount.directCallsPerTrace !== 6 ||
    d3d9NativeAdapterCount.callerRva !== "0x00059d36" ||
    d3d9NativeAdapterCount.result !== 1 ||
    d3d9NativeAdapterCount.lastError !== 0 ||
    d3d9NativeAdapterCount.consumer !==
      "store-count-in-edi-loop-adapter-indices-and-compare-monitor-identity" ||
    d3d9NativeAdapterCount.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterCount.nodeSequence !== 17_843 ||
    d3d9PrivateAdapterCount.nodeInstructionCount !== 11_807_197 ||
    d3d9PrivateAdapterCount.chromeSequence !== 17_843 ||
    d3d9PrivateAdapterCountArguments.join(",") !== "0x01000004" ||
    d3d9PrivateAdapterCount.returnAddress !== "0x10059d36" ||
    d3d9PrivateAdapterCount.result !== 1 ||
    d3d9PrivateAdapterCount.guestWriteBytes !== 0 ||
    d3d9PrivateAdapterCount.referenceCount !== 1 ||
    d3d9PrivateAdapterCount.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9AdapterMonitor.slot !== 15 ||
    d3d9AdapterMonitor.explicitArgumentWords !== 1 ||
    d3d9AdapterMonitor.returnKind !== "u32" ||
    d3d9AdapterMonitor.profileId !== "primary-adapter-monitor-v1" ||
    d3d9AdapterMonitor.adapter !== 0 ||
    d3d9AdapterMonitor.adapterMappingAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterMonitor.monitorIdentityAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterMonitor.displayProfileId !== "single-monitor-1920x1080-v1" ||
    d3d9AdapterMonitor.monitorHandleNamespace !== "vine.logical-monitor.v1" ||
    d3d9AdapterMonitor.primaryMonitorHandle !== "0x4f010001" ||
    d3d9AdapterMonitor.invalidAdapterResult !== "0x00000000" ||
    d3d9AdapterMonitor.guestWriteBytes !== 0 ||
    d3d9AdapterMonitor.referenceCountMutation !== false ||
    d3d9AdapterMonitor.lastErrorPolicy !== "preserve" ||
    d3d9AdapterMonitor.hostMonitorIdentityImported !== false ||
    d3d9AdapterMonitor.browserDisplayTopologyImported !== false ||
    d3d9NativeAdapterMonitor.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterMonitor.traceBytes !== 2_267_972 ||
    d3d9NativeAdapterMonitor.traceLines !== 7_133 ||
    d3d9NativeAdapterMonitor.traceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    d3d9NativeAdapterMonitor.summaryBytes !== 58_021 ||
    d3d9NativeAdapterMonitor.summarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    d3d9NativeAdapterMonitor.diagnostics !== 0 ||
    d3d9NativeAdapterMonitor.directCallsPerTrace !== 6 ||
    d3d9NativeAdapterMonitor.callerRva !== "0x00059d5e" ||
    d3d9NativeAdapterMonitor.adapter !== 0 ||
    d3d9NativeAdapterMonitor.resultOpaqueNonNull !== true ||
    d3d9NativeAdapterMonitor.lastError !== 0 ||
    d3d9NativeAdapterMonitor.oracleArchitecture !== "coff-i386" ||
    d3d9NativeAdapterMonitor.oracleSourceBytes !== 5_089 ||
    d3d9NativeAdapterMonitor.oracleSourceSha256 !==
      "337ddf83ec68ba80a25d70f1f9a9d0361a83833657ca7c852075043d16b57c67" ||
    d3d9NativeAdapterMonitor.oracleExecutableBytes !== 3_584 ||
    d3d9NativeAdapterMonitor.oracleExecutableSha256 !==
      "1d2681032fad258d9930c1a4a88ac6d4cf67d53b7d26ff447a17d5129bb436d5" ||
    d3d9NativeAdapterMonitor.oracleAdapterCount !== 1 ||
    d3d9NativeAdapterMonitorOracleAdapters.join(",") !== "0,1,4294967295" ||
    JSON.stringify(d3d9NativeAdapterMonitor.oracleResultsNonNull) !==
      JSON.stringify([true, false, false]) ||
    d3d9NativeAdapterMonitor.oraclePrimaryIdentityStable !== true ||
    d3d9NativeAdapterMonitor.oracleLastErrorSentinel !== 23_130 ||
    d3d9NativeAdapterMonitor.oracleLastErrorPreserved !== true ||
    d3d9NativeAdapterMonitor.consumer !==
      "compare-result-with-monitor-from-window-primary-identity" ||
    d3d9NativeAdapterMonitor.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterMonitorNodeSequences.join(",") !== "17844,17848" ||
    d3d9PrivateAdapterMonitorNodeInstructionCounts.join(",") !==
      "11807216,11807374" ||
    d3d9PrivateAdapterMonitorChromeSequences.join(",") !== "17844,17848" ||
    JSON.stringify(d3d9PrivateAdapterMonitor.arguments) !==
      JSON.stringify([["0x01000004", "0x00000000"], ["0x01000004", "0x00000000"]]) ||
    d3d9PrivateAdapterMonitor.returnAddresses?.join(",") !==
      "0x10059d5e,0x10059d5e" ||
    d3d9PrivateAdapterMonitor.results?.join(",") !==
      "0x4f010001,0x4f010001" ||
    d3d9PrivateAdapterMonitor.guestWriteBytes !== 0 ||
    d3d9PrivateAdapterMonitor.referenceCount !== 1 ||
    d3d9PrivateAdapterMonitor.matchesMonitorFromWindowIdentity !== true ||
    d3d9PrivateAdapterMonitor.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9DisplayMode.slot !== 8 ||
    d3d9DisplayMode.explicitArgumentWords !== 2 ||
    d3d9DisplayMode.returnKind !== "h-result" ||
    d3d9DisplayMode.adapter !== 0 ||
    d3d9DisplayMode.geometryAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9DisplayMode.width !== 1_920 ||
    d3d9DisplayMode.height !== 1_080 ||
    d3d9DisplayMode.refreshRateAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DisplayMode.refreshRate !== 60 ||
    d3d9DisplayMode.formatAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DisplayMode.format !== 22 ||
    d3d9DisplayMode.formatName !== "D3DFMT_X8R8G8B8" ||
    d3d9DisplayMode.outputStructure !== "D3DDISPLAYMODE" ||
    d3d9DisplayMode.outputBytes !== 16 ||
    d3d9DisplayMode.fullOutputOverwrite !== true ||
    d3d9DisplayMode.invalidAdapterOrNullOutputResult !== "0x8876086c" ||
    d3d9DisplayMode.invalidCallMutatesState !== false ||
    d3d9DisplayMode.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9DisplayMode.referenceCountMutation !== false ||
    d3d9DisplayMode.lastErrorPolicy !== "preserve" ||
    d3d9NativeDisplayMode.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDisplayMode.primaryTraceBytes !== 2_192_729 ||
    d3d9NativeDisplayMode.primaryTraceLines !== 6_886 ||
    d3d9NativeDisplayMode.primaryTraceSha256 !==
      "6b9bd17809ef235f1817fb08ccd4bfb92f27f3cd424bd0f15500470d73b05208" ||
    d3d9NativeDisplayMode.primaryBalancedApiPairs !== 3_405 ||
    d3d9NativeDisplayMode.confirmationTraceBytes !== 2_249_188 ||
    d3d9NativeDisplayMode.confirmationTraceLines !== 7_068 ||
    d3d9NativeDisplayMode.confirmationTraceSha256 !==
      "fac93ab62effd0228dc8c8a88b617f3aced5fcff9429c9e90c9347979c660f92" ||
    d3d9NativeDisplayMode.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDisplayMode.diagnostics !== 0 ||
    d3d9NativeDisplayMode.successfulCallsPerTrace !== 4 ||
    d3d9NativeDisplayModeCallerRvas.join(",") !==
      "0x0005b8d6,0x00059da5" ||
    d3d9NativeDisplayMode.callerCounts?.join(",") !== "1,3" ||
    d3d9NativeDisplayMode.firstCallInputZeroed !== true ||
    d3d9NativeDisplayMode.firstCallFullOverwrite !== true ||
    d3d9NativeDisplayMode.allCallsSameOutput !== true ||
    d3d9NativeDisplayMode.hresultSignConsumed !== true ||
    d3d9NativeDisplayMode.allOutputFieldsConsumed !== true ||
    d3d9NativeDisplayMode.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDisplayMode.nodeSequence !== 17_817 ||
    d3d9PrivateDisplayMode.nodeInstructionCount !== 11_806_455 ||
    d3d9PrivateDisplayMode.chromeSequence !== 17_817 ||
    d3d9PrivateDisplayModeArguments.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    d3d9PrivateDisplayMode.returnAddress !== "0x1005b8d6" ||
    d3d9PrivateDisplayMode.result !== "0x00000000" ||
    d3d9PrivateDisplayMode.outputHex !==
      "80070000380400003c00000016000000" ||
    d3d9PrivateDisplayMode.referenceCount !== 1 ||
    d3d9PrivateDisplayMode.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9AdapterIdentifier.slot !== 5 ||
    d3d9AdapterIdentifier.explicitArgumentWords !== 3 ||
    d3d9AdapterIdentifier.returnKind !== "h-result" ||
    d3d9AdapterIdentifier.adapter !== 0 ||
    d3d9AdapterIdentifier.flags !== 0 ||
    d3d9AdapterIdentifier.identityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterIdentifier.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterIdentifier.profileId !== "primary-virtual-adapter-identifier-v1" ||
    d3d9AdapterIdentifier.driver !== "vine-d3d9.dll" ||
    d3d9AdapterIdentifier.description !== "Vine Virtual Direct3D9 Adapter" ||
    d3d9AdapterIdentifier.deviceName !== "\\\\.\\DISPLAY1" ||
    d3d9AdapterIdentifier.driverVersion !== "0x0001000000000000" ||
    d3d9AdapterIdentifier.vendorId !== 0 ||
    d3d9AdapterIdentifier.deviceId !== 0 ||
    d3d9AdapterIdentifier.subSysId !== 0 ||
    d3d9AdapterIdentifier.revision !== 0 ||
    d3d9AdapterIdentifier.deviceIdentifier !==
      "56494e45-4433-4439-8000-000000000001" ||
    d3d9AdapterIdentifier.whqlLevel !== 0 ||
    d3d9AdapterIdentifier.outputStructure !== "D3DADAPTER_IDENTIFIER9" ||
    d3d9AdapterIdentifier.outputBytes !== 1_100 ||
    d3d9AdapterIdentifier.fullOutputOverwrite !== true ||
    d3d9AdapterIdentifier.invalidAdapterFlagsOrNullOutputResult !== "0x8876086c" ||
    d3d9AdapterIdentifier.whqlEnumerationPolicy !==
      "typed-unsupported-before-mutation" ||
    d3d9AdapterIdentifier.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9AdapterIdentifier.referenceCountMutation !== false ||
    d3d9AdapterIdentifier.hostGpuIdentityImported !== false ||
    d3d9AdapterIdentifier.lastErrorPolicy !== "preserve" ||
    d3d9NativeAdapterIdentifier.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterIdentifier.primaryTraceBytes !== 2_207_106 ||
    d3d9NativeAdapterIdentifier.primaryTraceLines !== 6_888 ||
    d3d9NativeAdapterIdentifier.primaryTraceSha256 !==
      "c635bde508a727e5c42dd0d17b75d3e82e30d62f77ad06bf497133cb4f8bd7d3" ||
    d3d9NativeAdapterIdentifier.primaryBalancedApiPairs !== 3_406 ||
    d3d9NativeAdapterIdentifier.confirmationTraceBytes !== 2_263_088 ||
    d3d9NativeAdapterIdentifier.confirmationTraceLines !== 7_068 ||
    d3d9NativeAdapterIdentifier.confirmationTraceSha256 !==
      "d13217c0bc87789d61da46af87339e63bb0a92ac11dd406698a6b75982574da5" ||
    d3d9NativeAdapterIdentifier.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeAdapterIdentifier.diagnostics !== 0 ||
    d3d9NativeAdapterIdentifier.successfulCallsPerTrace !== 1 ||
    d3d9NativeAdapterIdentifier.callerRva !== "0x0005b91c" ||
    d3d9NativeAdapterIdentifier.adapter !== 0 ||
    d3d9NativeAdapterIdentifier.flags !== 0 ||
    d3d9NativeAdapterIdentifier.result !== "0x00000000" ||
    d3d9NativeAdapterIdentifier.inputZeroed !== true ||
    d3d9NativeAdapterIdentifier.inputSha256 !==
      "a805fd6f7ad225e289321ba6e2b079e92c3accc1675a7ba6626101da38f727f2" ||
    d3d9NativeAdapterIdentifier.outputSha256 !==
      "773a5d417dceeab588a8724b951f9926543e39b42fad71b58d1ba6efb6a8497c" ||
    d3d9NativeAdapterIdentifier.changedBytes !== 169 ||
    d3d9NativeAdapterIdentifier.hresultSignConsumed !== true ||
    d3d9NativeAdapterIdentifier.outputFieldsConsumed !== false ||
    d3d9NativeAdapterIdentifier.hostGpuIdentityImported !== false ||
    d3d9NativeAdapterIdentifier.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterIdentifier.nodeSequence !== 17_818 ||
    d3d9PrivateAdapterIdentifier.nodeInstructionCount !== 11_806_466 ||
    d3d9PrivateAdapterIdentifier.chromeSequence !== 17_818 ||
    d3d9PrivateAdapterIdentifierArguments.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    d3d9PrivateAdapterIdentifier.returnAddress !== "0x1005b91c" ||
    d3d9PrivateAdapterIdentifier.result !== "0x00000000" ||
    d3d9PrivateAdapterIdentifier.outputAddress !== "0x10103410" ||
    d3d9PrivateAdapterIdentifier.outputBytes !== 1_100 ||
    d3d9PrivateAdapterIdentifier.outputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    d3d9PrivateAdapterIdentifier.referenceCount !== 1 ||
    d3d9PrivateAdapterIdentifier.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9DeviceCaps.slot !== 14 ||
    d3d9DeviceCaps.explicitArgumentWords !== 3 ||
    d3d9DeviceCaps.returnKind !== "h-result" ||
    d3d9DeviceCaps.adapter !== 0 ||
    d3d9DeviceCaps.deviceType !== 1 ||
    d3d9DeviceCaps.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9DeviceCaps.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DeviceCaps.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9DeviceCaps.profileId !==
      "primary-hal-readscanline-dynamic-textures-ps3-v1" ||
    d3d9DeviceCaps.caps !== "0x00020000" ||
    d3d9DeviceCaps.capsNames?.join(",") !== "D3DCAPS_READ_SCANLINE" ||
    d3d9DeviceCaps.caps2 !== "0x20000000" ||
    d3d9DeviceCaps.caps2Names?.join(",") !== "D3DCAPS2_DYNAMICTEXTURES" ||
    d3d9DeviceCaps.pixelShaderVersion !== "0xffff0300" ||
    d3d9DeviceCaps.numberOfAdaptersInGroup !== 1 ||
    d3d9DeviceCaps.otherFieldsZero !== true ||
    d3d9DeviceCaps.outputStructure !== "D3DCAPS9" ||
    d3d9DeviceCaps.outputBytes !== 304 ||
    d3d9DeviceCaps.fullOutputOverwrite !== true ||
    d3d9DeviceCaps.invalidAdapterDeviceTypeOrNullOutputResult !== "0x8876086c" ||
    d3d9DeviceCaps.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9DeviceCaps.referenceCountMutation !== false ||
    d3d9DeviceCaps.hostGpuCapsImported !== false ||
    d3d9DeviceCaps.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceCaps.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceCaps.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceCaps.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceCaps.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceCaps.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceCaps.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceCaps.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceCaps.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceCaps.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceCaps.diagnostics !== 0 ||
    d3d9NativeDeviceCaps.successfulCallsPerTrace !== 1 ||
    d3d9NativeDeviceCaps.callerRva !== "0x0005b949" ||
    d3d9NativeDeviceCaps.adapter !== 0 ||
    d3d9NativeDeviceCaps.deviceType !== 1 ||
    d3d9NativeDeviceCaps.result !== "0x00000000" ||
    d3d9NativeDeviceCaps.outputBytes !== 304 ||
    d3d9NativeDeviceCaps.outputSha256 !==
      "5c48557292c6b254d6e9eb9612e3b24aae72f183bcbbcaf803d7a5ac16b41803" ||
    d3d9NativeDeviceCaps.outputEqualsCallerStackPointerAfterReturn !== true ||
    d3d9NativeDeviceCaps.hresultSignConsumed !== true ||
    d3d9NativeDeviceCapsConsumedFields.join(",") !==
      "Caps,Caps2,PixelShaderVersion" ||
    d3d9NativeDeviceCapsConsumedFieldOffsets.join(",") !== "8,12,204" ||
    d3d9NativeDeviceCapsConsumedValues.join(",") !==
      "0x00020000,0xe0010000,0xffff0300" ||
    d3d9NativeDeviceCaps.consumerInstructionCount !== 256 ||
    d3d9NativeDeviceCaps.hostGpuCapsImported !== false ||
    d3d9NativeDeviceCaps.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceCaps.nodeSequence !== 17_819 ||
    d3d9PrivateDeviceCaps.nodeInstructionCount !== 11_806_478 ||
    d3d9PrivateDeviceCaps.chromeSequence !== 17_819 ||
    d3d9PrivateDeviceCapsArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    d3d9PrivateDeviceCaps.returnAddress !== "0x1005b949" ||
    d3d9PrivateDeviceCaps.result !== "0x00000000" ||
    d3d9PrivateDeviceCaps.outputAddress !== "0x006ffe18" ||
    d3d9PrivateDeviceCaps.outputBytes !== 304 ||
    d3d9PrivateDeviceCaps.outputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    d3d9PrivateDeviceCaps.referenceCount !== 1 ||
    d3d9PrivateDeviceCaps.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CheckDeviceType.slot !== 9 ||
    d3d9CheckDeviceType.explicitArgumentWords !== 5 ||
    d3d9CheckDeviceType.returnKind !== "h-result" ||
    d3d9CheckDeviceType.adapter !== 0 ||
    d3d9CheckDeviceType.deviceType !== 1 ||
    d3d9CheckDeviceType.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CheckDeviceType.adapterFormat !== 22 ||
    d3d9CheckDeviceType.backBufferFormat !== 22 ||
    d3d9CheckDeviceType.formatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceType.windowedSupported !== true ||
    d3d9CheckDeviceType.fullscreenSupported !== true ||
    d3d9CheckDeviceType.boolNormalization !== "zero-fullscreen-nonzero-windowed" ||
    d3d9CheckDeviceType.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9CheckDeviceType.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CheckDeviceType.profileId !==
      "primary-hal-x8r8g8b8-windowed-fullscreen-v1" ||
    d3d9CheckDeviceType.invalidAdapterOrDeviceTypeResult !== "0x8876086c" ||
    d3d9CheckDeviceType.unsupportedFormatResult !== "0x8876086a" ||
    d3d9CheckDeviceType.guestWriteBytes !== 0 ||
    d3d9CheckDeviceType.referenceCountMutation !== false ||
    d3d9CheckDeviceType.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceType.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceType.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceType.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceType.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceType.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceType.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceType.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceType.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceType.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceType.diagnostics !== 0 ||
    d3d9NativeDeviceType.successfulCallsPerTrace !== 5 ||
    d3d9NativeDeviceType.directCallsPerTrace !== 2 ||
    d3d9NativeDeviceType.internalCallsPerTrace !== 3 ||
    d3d9NativeDeviceTypeCallerRvas.join(",") !== "0x0005b9d6,0x0005ba1d" ||
    d3d9NativeDeviceTypeWindowedValues.join(",") !== "1,0" ||
    d3d9NativeDeviceType.adapter !== 0 ||
    d3d9NativeDeviceType.deviceType !== 1 ||
    d3d9NativeDeviceType.adapterFormat !== 22 ||
    d3d9NativeDeviceType.backBufferFormat !== 22 ||
    d3d9NativeDeviceType.result !== "0x00000000" ||
    d3d9NativeDeviceType.hresultSignConsumed !== true ||
    d3d9NativeDeviceType.consumerOracle !==
      "immutable-pe-disassembly-and-device-caps-consumer-snapshot" ||
    d3d9NativeDeviceType.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceTypeNodeSequences.join(",") !== "17820,17821" ||
    d3d9PrivateDeviceTypeInstructionCounts.join(",") !== "11806497,11806510" ||
    d3d9PrivateDeviceTypeChromeSequences.join(",") !== "17820,17821" ||
    JSON.stringify(d3d9PrivateDeviceType.arguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000001"],
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000000"],
      ]) ||
    d3d9PrivateDeviceType.returnAddresses?.join(",") !==
      "0x1005b9d6,0x1005ba1d" ||
    d3d9PrivateDeviceType.results?.join(",") !== "0x00000000,0x00000000" ||
    d3d9PrivateDeviceType.guestWriteBytes !== 0 ||
    d3d9PrivateDeviceType.referenceCount !== 1 ||
    d3d9PrivateDeviceType.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CheckDeviceFormat.slot !== 10 ||
    d3d9CheckDeviceFormat.explicitArgumentWords !== 6 ||
    d3d9CheckDeviceFormat.returnKind !== "h-result" ||
    d3d9CheckDeviceFormat.adapter !== 0 ||
    d3d9CheckDeviceFormat.deviceType !== 1 ||
    d3d9CheckDeviceFormat.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CheckDeviceFormat.adapterFormat !== 22 ||
    d3d9CheckDeviceFormat.adapterFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    d3d9CheckDeviceFormatSupportedUsageNames.join(",") !==
      "none,D3DUSAGE_DYNAMIC" ||
    d3d9CheckDeviceFormat.resourceType !== 3 ||
    d3d9CheckDeviceFormat.resourceTypeName !== "D3DRTYPE_TEXTURE" ||
    d3d9CheckDeviceFormat.checkFormat !== 22 ||
    d3d9CheckDeviceFormat.checkFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceFormat.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9CheckDeviceFormat.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CheckDeviceFormat.profileId !==
      "primary-hal-x8r8g8b8-texture-basic-dynamic-v1" ||
    d3d9CheckDeviceFormat.invalidAdapterOrDeviceTypeResult !== "0x8876086c" ||
    d3d9CheckDeviceFormat.unsupportedCombinationResult !== "0x8876086a" ||
    d3d9CheckDeviceFormat.guestWriteBytes !== 0 ||
    d3d9CheckDeviceFormat.referenceCountMutation !== false ||
    d3d9CheckDeviceFormat.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceFormat.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceFormat.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceFormat.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceFormat.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceFormat.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceFormat.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceFormat.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceFormat.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceFormat.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceFormat.diagnostics !== 0 ||
    d3d9NativeDeviceFormat.callsPerTrace !== 13 ||
    d3d9NativeDeviceFormat.directCallsPerTrace !== 1 ||
    d3d9NativeDeviceFormat.createDeviceParentedCallsPerTrace !== 3 ||
    d3d9NativeDeviceFormat.otherInternalCallsPerTrace !== 9 ||
    d3d9NativeDeviceFormat.directCallerRva !== "0x0005ba69" ||
    d3d9NativeDeviceFormat.adapter !== 0 ||
    d3d9NativeDeviceFormat.deviceType !== 1 ||
    d3d9NativeDeviceFormat.adapterFormat !== 22 ||
    d3d9NativeDeviceFormat.resourceType !== 3 ||
    d3d9NativeDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    d3d9NativeDeviceFormat.supportedCheckFormat !== 22 ||
    d3d9NativeDeviceFormat.basicUsageSuccessCallsPerTrace !== 6 ||
    d3d9NativeDeviceFormat.dynamicUsageSuccessCallsPerTrace !== 4 ||
    d3d9NativeDeviceFormat.unsupportedCheckFormat !== 118 ||
    d3d9NativeDeviceFormat.unsupportedCallsPerTrace !== 3 ||
    d3d9NativeDeviceFormat.unsupportedResult !== "0x8876086a" ||
    d3d9NativeDeviceFormat.directUsage !== 512 ||
    d3d9NativeDeviceFormat.directCheckFormat !== 22 ||
    d3d9NativeDeviceFormat.directResult !== "0x00000000" ||
    d3d9NativeDeviceFormat.hresultSignConsumed !== true ||
    d3d9NativeDeviceFormat.consumerOracle !== "immutable-pe-disassembly" ||
    d3d9NativeDeviceFormat.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceFormat.nodeSequence !== 17_822 ||
    d3d9PrivateDeviceFormat.nodeInstructionCount !== 11_806_524 ||
    d3d9PrivateDeviceFormat.chromeSequence !== 17_822 ||
    d3d9PrivateDeviceFormatArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    d3d9PrivateDeviceFormat.returnAddress !== "0x1005ba69" ||
    d3d9PrivateDeviceFormat.result !== "0x00000000" ||
    d3d9PrivateDeviceFormat.guestWriteBytes !== 0 ||
    d3d9PrivateDeviceFormat.referenceCount !== 1 ||
    d3d9PrivateDeviceFormat.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateDevice.slot !== 16 ||
    d3d9CreateDevice.explicitArgumentWords !== 6 ||
    d3d9CreateDevice.returnKind !== "h-result" ||
    d3d9CreateDevice.profileId !== "primary-windowed-hal-device-v1" ||
    d3d9CreateDevice.adapter !== 0 ||
    d3d9CreateDevice.deviceType !== 1 ||
    d3d9CreateDevice.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CreateDevice.behaviorFlags !== "0x00000044" ||
    d3d9CreateDeviceBehaviorFlagNames.join(",") !==
      "D3DCREATE_MULTITHREADED,D3DCREATE_HARDWARE_VERTEXPROCESSING" ||
    d3d9CreateDevice.focusWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    d3d9CreateDevice.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CreateDevice.displayProfileId !== "single-monitor-1920x1080-v1" ||
    d3d9CreateDevice.presentationStructure !== "D3DPRESENT_PARAMETERS" ||
    d3d9CreateDevice.presentationBytes !== 56 ||
    d3d9CreateDevice.requestedBackBufferWidth !== 0 ||
    d3d9CreateDevice.requestedBackBufferHeight !== 0 ||
    d3d9CreateDevice.requestedBackBufferFormat !== 0 ||
    d3d9CreateDevice.requestedBackBufferFormatName !== "D3DFMT_UNKNOWN" ||
    d3d9CreateDevice.resolvedGeometryAuthority !==
      "live-window-client-geometry" ||
    d3d9CreateDevice.resolvedBackBufferFormat !== 22 ||
    d3d9CreateDevice.resolvedBackBufferFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CreateDevice.backBufferCount !== 1 ||
    d3d9CreateDevice.multiSampleType !== 0 ||
    d3d9CreateDevice.multiSampleQuality !== 0 ||
    d3d9CreateDevice.swapEffect !== 3 ||
    d3d9CreateDevice.swapEffectName !== "D3DSWAPEFFECT_DISCARD" ||
    d3d9CreateDevice.deviceWindowMatchesFocusWindow !== true ||
    d3d9CreateDevice.windowed !== true ||
    d3d9CreateDevice.enableAutoDepthStencil !== false ||
    d3d9CreateDevice.autoDepthStencilFormat !== 80 ||
    d3d9CreateDevice.autoDepthStencilFormatName !== "D3DFMT_D16" ||
    d3d9CreateDevice.flags !== 0 ||
    d3d9CreateDevice.fullScreenRefreshRateInHz !== 0 ||
    d3d9CreateDevice.presentationInterval !== 0 ||
    d3d9CreateDevice.presentationPrefixWriteBytes !== 12 ||
    d3d9CreateDevice.deviceOutputWriteBytes !== 4 ||
    d3d9CreateDevice.guestWritesAndObjectPublicationAtomic !== true ||
    d3d9CreateDevice.invalidIdentityOrPointerResult !== "0x8876086c" ||
    d3d9CreateDevice.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateDevice.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateDevice.lastErrorPolicy !== "preserve" ||
    d3d9DeviceInterface.classIdentity !== "d3d9.idirect3ddevice9.v1" ||
    d3d9DeviceInterface.interface !== "IDirect3DDevice9" ||
    d3d9DeviceInterface.vtableSlots !== 119 ||
    d3d9DeviceImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,CreateTexture,CreateVertexBuffer,SetRenderState" ||
    d3d9DeviceInterface.initialReferenceCount !== 1 ||
    d3d9DeviceInterface.retainsRootInterfaceReference !== true ||
    d3d9DeviceInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9DeviceInterface.finalReleaseMayCascadeRootRetirement !== true ||
    runtimeGraphics.stateOwner !== "runtime/graphics.rs::RuntimeGraphics" ||
    runtimeGraphics.aggregateCoordinator !== "runtime/kernel.rs::RuntimeKernel" ||
    runtimeGraphics.deviceNamespace !== "vine.logical-graphics-device.v1" ||
    runtimeGraphics.deviceCapacity !== 4_096 ||
    runtimeGraphics.textureNamespace !== "vine.logical-graphics-texture.v1" ||
    runtimeGraphics.textureCapacity !== 4_096 ||
    runtimeGraphics.surfaceNamespace !== "vine.logical-graphics-surface.v1" ||
    runtimeGraphics.surfaceCapacity !== 4_096 ||
    runtimeGraphics.vertexBufferNamespace !==
      "vine.logical-graphics-vertex-buffer.v1" ||
    runtimeGraphics.vertexBufferCapacity !== 4_096 ||
    runtimeGraphics.frontend !== "d3d9" ||
    runtimeGraphics.windowRelation !==
      "non-owning-revalidated-live-window-id-and-owner-thread" ||
    runtimeGraphics.comRelation !== "non-owning-retired-with-com-object" ||
    runtimeGraphics.textureDeviceRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.surfaceTextureRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.vertexBufferDeviceRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.vertexBufferContentAuthority !== "rust-guest-memory-backing" ||
    runtimeGraphics.vertexBufferLockAuthority !==
      "runtime-graphics-single-active-lock" ||
    runtimeGraphics.deviceRenderStateAuthority !==
      "runtime-graphics-device-owned-typed-state" ||
    runtimeGraphics.deviceRenderStateShape !==
      "explicit-reached-values-without-inferred-windows-defaults" ||
    runtimeGraphics.deviceRenderStateMutation !==
      "owner-plan-preflight-stale-check-targeted-commit" ||
    runtimeGraphics.deviceRenderStateSameValuePolicy !==
      "success-without-generation-change" ||
    runtimeGraphics.surfaceFillCapacity !== 16_384 ||
    runtimeGraphics.surfaceContentAuthority !== "bounded-fill-command-history" ||
    runtimeGraphics.surfaceContentGeneration !== true ||
    runtimeGraphics.mutation !== "owner-plan-stale-check-targeted-commit" ||
    runtimeGraphics.processExitTeardown !== true ||
    runtimeGraphics.browserRendererNamespace !==
      "vine.browser-renderer-resource.v1" ||
    runtimeGraphics.browserRendererAssociationOwnership !== "non-owning" ||
    runtimeGraphics.browserOwnsLogicalGraphicsState !== false ||
    d3d9NativeCreateDevice.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeCreateDevice.primaryTraceBytes !== 2_333_677 ||
    d3d9NativeCreateDevice.primaryTraceLines !== 7_075 ||
    d3d9NativeCreateDevice.primaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    d3d9NativeCreateDevice.primaryBalancedApiPairs !== 3_498 ||
    d3d9NativeCreateDevice.primarySummaryBytes !== 275_843 ||
    d3d9NativeCreateDevice.primarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    d3d9NativeCreateDevice.confirmationTraceBytes !== 2_355_307 ||
    d3d9NativeCreateDevice.confirmationTraceLines !== 7_121 ||
    d3d9NativeCreateDevice.confirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    d3d9NativeCreateDevice.confirmationBalancedApiPairs !== 3_521 ||
    d3d9NativeCreateDevice.confirmationSummaryBytes !== 275_855 ||
    d3d9NativeCreateDevice.confirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    d3d9NativeCreateDevice.diagnostics !== 0 ||
    d3d9NativeCreateDevice.successfulCallsPerTrace !== 3 ||
    d3d9NativeCreateDevice.callerRva !== "0x0005aca5" ||
    d3d9NativeCreateDevice.adapter !== 0 ||
    d3d9NativeCreateDevice.deviceType !== 1 ||
    d3d9NativeCreateDevice.behaviorFlags !== "0x00000044" ||
    d3d9NativeCreateDeviceInput.join(",") !== "0,0,0" ||
    d3d9NativeCreateDevice.preservedPresentationFields?.join(",") !==
      "1,0,0,3,1,0,80,0,0,0" ||
    JSON.stringify(d3d9NativeCreateDevice.resolvedWidthHeightFormat) !==
      JSON.stringify([[800, 600, 22], [1280, 720, 22], [1280, 720, 22]]) ||
    d3d9NativeCreateDevice.result !== "0x00000000" ||
    d3d9NativeCreateDevice.lastError !== 0 ||
    d3d9NativeCreateDevice.deviceOutputNonNull !== true ||
    d3d9NativeCreateDevice.hresultSignConsumed !== true ||
    d3d9NativeCreateDevice.firstReachedDeviceMethod !== "CreateTexture" ||
    d3d9NativeCreateDevice.firstReachedDeviceSlot !== 23 ||
    d3d9NativeCreateDevice.nestedDeviceAddRefResult !== 2 ||
    d3d9NativeCreateDevice.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateCreateDevice.nodeSequence !== 17_849 ||
    d3d9PrivateCreateDevice.nodeInstructionCount !== 11_807_387 ||
    d3d9PrivateCreateDevice.chromeSequence !== 17_849 ||
    d3d9PrivateCreateDeviceArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    d3d9PrivateCreateDevice.returnAddress !== "0x1005aca5" ||
    d3d9PrivateCreateDevice.result !== "0x00000000" ||
    d3d9PrivateCreateDevice.presentationAddress !== "0x101038f0" ||
    d3d9PrivateCreateDevice.presentationPrefixHex !==
      "200300005802000016000000" ||
    d3d9PrivateCreateDevice.deviceOutputAddress !== "0x101517c4" ||
    d3d9PrivateCreateDevice.devicePointer !== "0x01010004" ||
    d3d9PrivateCreateDevice.guestWriteBytes !== 16 ||
    d3d9PrivateCreateDevice.deviceObjectId !== 3 ||
    d3d9PrivateCreateDevice.parentInterfaceId !== 4 ||
    d3d9PrivateCreateDevice.deviceInterfaceId !== 6 ||
    d3d9PrivateCreateDevice.deviceVtable !== "0x0101001c" ||
    d3d9PrivateCreateDevice.deviceVtableSlots !== 119 ||
    d3d9PrivateCreateDevice.rootReferenceCount !== 2 ||
    d3d9PrivateCreateDevice.deviceReferenceCount !== 1 ||
    d3d9PrivateCreateDevice.logicalGraphicsDeviceId !== 1 ||
    d3d9PrivateCreateDevice.logicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    d3d9PrivateLogicalPresentation.join(",") !== "800,600,22,1,1,3,1" ||
    d3d9PrivateCreateDevice.browserRendererCreated !== false ||
    d3d9PrivateCreateDevice.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateTexture.slot !== 23 ||
    d3d9CreateTexture.explicitArgumentWords !== 8 ||
    d3d9CreateTexture.returnKind !== "h-result" ||
    d3d9CreateTexture.profileId !==
      "primary-initial-x8r8g8b8-textures-v1" ||
    JSON.stringify(
      d3d9CreateTextureVariants.map((variant) => [
        variant.width,
        variant.height,
        variant.levels,
        variant.usage,
        variant.usageName,
        variant.format,
        variant.formatName,
        variant.pool,
        variant.poolName,
      ]),
    ) !==
      JSON.stringify([
        [1_024, 1_024, 1, 0, "none", 22, "D3DFMT_X8R8G8B8", 2, "D3DPOOL_SYSTEMMEM"],
        [1_024, 1_024, 1, 0, "none", 22, "D3DFMT_X8R8G8B8", 0, "D3DPOOL_DEFAULT"],
        [1_024, 1_024, 1, 512, "dynamic", 22, "D3DFMT_X8R8G8B8", 0, "D3DPOOL_DEFAULT"],
      ]) ||
    d3d9CreateTexture.sharedHandlePointer !== "NULL" ||
    d3d9CreateTexture.textureOutputWriteBytes !== 4 ||
    d3d9CreateTexture.guestComGraphicsPublicationAtomic !== true ||
    d3d9CreateTexture.invalidArgumentOrPointerResult !== "0x8876086c" ||
    d3d9CreateTexture.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateTexture.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateTexture.lastErrorPolicy !== "preserve" ||
    d3d9TextureInterface.classIdentity !== "d3d9.idirect3dtexture9.v1" ||
    d3d9TextureInterface.interface !== "IDirect3DTexture9" ||
    d3d9TextureInterface.vtableSlots !== 22 ||
    d3d9TextureImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetSurfaceLevel" ||
    d3d9TextureInterface.initialReferenceCount !== 1 ||
    d3d9TextureInterface.retainsDeviceInterfaceReference !== true ||
    d3d9TextureInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9TextureInterface.finalReleaseMayCascadeDeviceAndRootRetirement !== true ||
    d3d9NativeCreateTexture.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeCreateTexture.primaryTraceBytes !== 2_333_677 ||
    d3d9NativeCreateTexture.primaryTraceLines !== 7_075 ||
    d3d9NativeCreateTexture.primaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    d3d9NativeCreateTexture.primaryBalancedApiPairs !== 3_498 ||
    d3d9NativeCreateTexture.primarySummaryBytes !== 275_843 ||
    d3d9NativeCreateTexture.primarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    d3d9NativeCreateTexture.confirmationTraceBytes !== 2_355_307 ||
    d3d9NativeCreateTexture.confirmationTraceLines !== 7_121 ||
    d3d9NativeCreateTexture.confirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    d3d9NativeCreateTexture.confirmationBalancedApiPairs !== 3_521 ||
    d3d9NativeCreateTexture.confirmationSummaryBytes !== 275_855 ||
    d3d9NativeCreateTexture.confirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    d3d9NativeCreateTexture.diagnostics !== 0 ||
    d3d9NativeCreateTexture.successfulCallsPerTrace !== 1 ||
    d3d9NativeCreateTexture.callerRva !== "0x0005a715" ||
    d3d9NativeCreateTextureArguments.join(",") !== "1024,1024,1,0,22,2" ||
    d3d9NativeCreateTexture.sharedHandleNull !== true ||
    d3d9NativeCreateTexture.result !== "0x00000000" ||
    d3d9NativeCreateTexture.lastError !== 0 ||
    d3d9NativeCreateTexture.hresultSignConsumed !== true ||
    d3d9NativeCreateTexture.textureOutputSnapshotCaptured !== false ||
    d3d9NativeCreateTexture.nativeTexturePointerClaimed !== false ||
    d3d9NativeCreateTexture.immediateNextMethod !== "GetSurfaceLevel" ||
    d3d9NativeCreateTexture.immediateNextSlot !== 18 ||
    d3d9NativeCreateTexture.immediateNextLevel !== 0 ||
    d3d9NativeCreateTexture.sourceExecutableHashUnchanged !== true ||
    d3d9NativeInitialTextures.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialTextures.profileId !== "x86-bgi-initial-textures-v1" ||
    d3d9NativeInitialTextures.primaryTraceBytes !== 261_943 ||
    d3d9NativeInitialTextures.primaryTraceLines !== 505 ||
    d3d9NativeInitialTextures.primaryTraceSha256 !==
      "2fe16a38d7a8c8ee18b673d06f759ef3bfa3d412d3f688689692edb4d6d55ad4" ||
    d3d9NativeInitialTextures.primaryBalancedApiPairs !== 219 ||
    d3d9NativeInitialTextures.primarySummaryBytes !== 328_531 ||
    d3d9NativeInitialTextures.primarySummarySha256 !==
      "85db2bcea0fb895c1481a3af7330a3c79b430cbed997761cdb23d30e0fbf875d" ||
    d3d9NativeInitialTextures.confirmationTraceBytes !== 262_766 ||
    d3d9NativeInitialTextures.confirmationTraceLines !== 508 ||
    d3d9NativeInitialTextures.confirmationTraceSha256 !==
      "cdf646718a1a187e72129d9ae5562d44b4c5432c7c9c60ee1c217d30ae90179f" ||
    d3d9NativeInitialTextures.confirmationBalancedApiPairs !== 220 ||
    d3d9NativeInitialTextures.confirmationSummaryBytes !== 328_698 ||
    d3d9NativeInitialTextures.confirmationSummarySha256 !==
      "20eb05e775a84075aec1c48b2137c9d237f684e2e19d580b1aac5671a7039837" ||
    d3d9NativeInitialTextures.diagnostics !== 0 ||
    d3d9NativeInitialTextures.callsPerTrace !== 3 ||
    d3d9NativeInitialTextureCallerRvas.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    JSON.stringify(d3d9NativeInitialTextures.variants) !==
      JSON.stringify([[1_024, 1_024, 1, 0, 22, 2], [1_024, 1_024, 1, 0, 22, 0], [1_024, 1_024, 1, 512, 22, 0]]) ||
    d3d9NativeInitialTextures.sharedHandlesNull !== true ||
    d3d9NativeInitialTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialTextureLastErrors.join(",") !== "0,0,0" ||
    d3d9NativeInitialTextures.sameDevicePerTrace !== true ||
    d3d9NativeInitialTextureOutputAddresses.join(",") !==
      "0x00d117cc,0x00d117d0,0x00d117d4" ||
    d3d9NativeInitialTextures.returnedTexturesNonNullAndDistinctPerTrace !== true ||
    d3d9NativeInitialTextures.nativeTexturePointersAreOpaqueArtifactIdentity !== true ||
    d3d9NativeInitialTextures.immediateNextMethod !== "CreateVertexBuffer" ||
    d3d9NativeInitialTextures.immediateNextSlot !== 26 ||
    d3d9NativeInitialTextureNextArguments.join(",") !== "112,8,324,1" ||
    d3d9NativeInitialTextures.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateCreateTexture.nodeSequence !== 17_850 ||
    d3d9PrivateCreateTexture.nodeInstructionCount !== 11_807_772 ||
    d3d9PrivateCreateTexture.chromeSequence !== 17_850 ||
    d3d9PrivateCreateTextureArguments.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    d3d9PrivateCreateTexture.returnAddress !== "0x1005a715" ||
    d3d9PrivateCreateTexture.result !== "0x00000000" ||
    d3d9PrivateCreateTexture.textureOutputAddress !== "0x101517cc" ||
    d3d9PrivateCreateTexture.texturePointer !== "0x01020004" ||
    d3d9PrivateCreateTexture.guestWriteBytes !== 4 ||
    d3d9PrivateCreateTexture.textureObjectId !== 4 ||
    d3d9PrivateCreateTexture.parentInterfaceId !== 6 ||
    d3d9PrivateCreateTexture.textureInterfaceId !== 8 ||
    d3d9PrivateCreateTexture.textureVtable !== "0x0102001c" ||
    d3d9PrivateCreateTexture.textureVtableSlots !== 22 ||
    d3d9PrivateCreateTexture.deviceReferenceCount !== 2 ||
    d3d9PrivateCreateTexture.textureReferenceCount !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsDeviceId !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureId !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureNamespace !==
      "vine.logical-graphics-texture.v1" ||
    d3d9PrivateTextureDescriptor.join(",") !== "1024,1024,1,0,22,2" ||
    d3d9PrivateCreateTexture.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureCount !== 1 ||
    d3d9PrivateCreateTexture.browserRendererCreated !== false ||
    d3d9PrivateCreateTexture.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9PrivateInitialTextureNodeSequences.join(",") !== "17850,17857,17858" ||
    d3d9PrivateInitialTextureInstructionCounts.join(",") !==
      "11807772,11807840,11807856" ||
    d3d9PrivateInitialTextureChromeSequences.join(",") !== "17850,17857,17858" ||
    d3d9PrivateInitialTextureReturnAddresses.join(",") !==
      "0x1005a715,0x1005a7b7,0x1005a7dc" ||
    JSON.stringify(d3d9PrivateInitialTextures.arguments) !==
      JSON.stringify([
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000000", "0x00000016", "0x00000002", "0x101517cc", "0x00000000"],
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000000", "0x00000016", "0x00000000", "0x101517d0", "0x00000000"],
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000200", "0x00000016", "0x00000000", "0x101517d4", "0x00000000"],
      ]) ||
    d3d9PrivateInitialTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialTextureOutputAddresses.join(",") !==
      "0x101517cc,0x101517d0,0x101517d4" ||
    d3d9PrivateInitialTexturePointers.join(",") !==
      "0x01020004,0x01030004,0x01040004" ||
    d3d9PrivateInitialTextureGuestWrites.join(",") !== "4,4,4" ||
    JSON.stringify(d3d9PrivateInitialTextures.logicalDescriptors) !==
      JSON.stringify([[1_024, 1_024, 1, 0, 22, 2], [1_024, 1_024, 1, 0, 22, 0], [1_024, 1_024, 1, 512, 22, 0]]) ||
    d3d9PrivateInitialTextures.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateInitialTextures.logicalGraphicsTextureCount !== 3 ||
    d3d9PrivateInitialTextures.deviceReferenceCountAtStop !== 4 ||
    d3d9PrivateInitialTextures.browserRendererCreated !== false ||
    d3d9PrivateInitialTextures.pixelStorageCreated !== false ||
    d3d9PrivateInitialTextures.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateVertexBuffer.slot !== 26 ||
    d3d9CreateVertexBuffer.explicitArgumentWords !== 6 ||
    d3d9CreateVertexBuffer.returnKind !== "h-result" ||
    d3d9CreateVertexBuffer.profileId !==
      "primary-initial-xyzw-diffuse-tex1-quad-v1" ||
    d3d9CreateVertexBuffer.byteLength !== 112 ||
    d3d9CreateVertexBuffer.usage !== 8 ||
    d3d9CreateVertexBuffer.usageName !== "D3DUSAGE_WRITEONLY" ||
    d3d9CreateVertexBuffer.flexibleVertexFormat !== 324 ||
    d3d9CreateVertexBuffer.flexibleVertexFormatName !==
      "D3DFVF_XYZRHW|D3DFVF_DIFFUSE|D3DFVF_TEX1" ||
    d3d9CreateVertexBuffer.pool !== 1 ||
    d3d9CreateVertexBuffer.poolName !== "D3DPOOL_MANAGED" ||
    d3d9CreateVertexBuffer.sharedHandlePointer !== "NULL" ||
    d3d9CreateVertexBuffer.vertexBufferOutputWriteBytes !== 4 ||
    d3d9CreateVertexBuffer.initialContent !== "zeroed" ||
    d3d9CreateVertexBuffer.initialBackingProtection !== "read-only" ||
    d3d9CreateVertexBuffer.guestComAddressGraphicsMemoryPublicationAtomic !== true ||
    d3d9CreateVertexBuffer.invalidArgumentOrPointerResult !== "0x8876086c" ||
    d3d9CreateVertexBuffer.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateVertexBuffer.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateVertexBuffer.lastErrorPolicy !== "preserve" ||
    d3d9VertexBufferInterface.classIdentity !==
      "d3d9.idirect3dvertexbuffer9.v1" ||
    d3d9VertexBufferInterface.interface !== "IDirect3DVertexBuffer9" ||
    d3d9VertexBufferInterface.iid !==
      "b64bb1b5-fd70-4df6-bf91-19d0a12455e3" ||
    d3d9VertexBufferInterface.vtableSlots !== 14 ||
    d3d9VertexBufferImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,Lock,Unlock" ||
    d3d9VertexBufferInterface.initialReferenceCount !== 1 ||
    d3d9VertexBufferInterface.retainsDeviceInterfaceReference !== true ||
    d3d9VertexBufferInterface.finalReleaseRetiresComBackingGraphicsBackingAndAddressRecords !== true ||
    d3d9VertexBufferInterface.finalReleaseMayCascadeDeviceAndRootRetirement !== true ||
    d3d9VertexBufferLock.slot !== 11 ||
    d3d9VertexBufferLock.explicitArgumentWords !== 4 ||
    d3d9VertexBufferLock.returnKind !== "h-result" ||
    d3d9VertexBufferLock.supportedOffset !== 0 ||
    d3d9VertexBufferLock.supportedSize !== 0 ||
    d3d9VertexBufferLock.supportedFlags !== 0 ||
    d3d9VertexBufferLock.zeroSizeMeansRemainingBytes !== true ||
    d3d9VertexBufferLock.outputWriteBytes !== 4 ||
    d3d9VertexBufferLock.lockProtection !== "read-write" ||
    d3d9VertexBufferLock.duplicateOrUnsupportedLockResult !== "0x8876086c" ||
    d3d9VertexBufferLock.guestWriteAndProtectionTransitionAtomic !== true ||
    d3d9VertexBufferLock.lastErrorPolicy !== "preserve" ||
    d3d9VertexBufferUnlock.slot !== 12 ||
    d3d9VertexBufferUnlock.explicitArgumentWords !== 0 ||
    d3d9VertexBufferUnlock.returnKind !== "h-result" ||
    d3d9VertexBufferUnlock.unlockedProtection !== "read-only" ||
    d3d9VertexBufferUnlock.incrementsContentGeneration !== true ||
    d3d9VertexBufferUnlock.unlockWithoutLockResult !== "0x8876086c" ||
    d3d9VertexBufferUnlock.graphicsAndProtectionTransitionAtomic !== true ||
    d3d9VertexBufferUnlock.lastErrorPolicy !== "preserve" ||
    d3d9NativeInitialVertexBuffer.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialVertexBuffer.profileId !==
      "x86-bgi-initial-vertex-buffer-v1" ||
    d3d9NativeInitialVertexBuffer.primaryTraceBytes !== 286_932 ||
    d3d9NativeInitialVertexBuffer.primaryTraceLines !== 512 ||
    d3d9NativeInitialVertexBuffer.primaryTraceSha256 !==
      "0a1719739f86eaa42353e0c762e802119225e30975a318a29669e9aea2ea5c30" ||
    d3d9NativeInitialVertexBuffer.primaryBalancedApiPairs !== 222 ||
    d3d9NativeInitialVertexBuffer.primarySummaryBytes !== 282_623 ||
    d3d9NativeInitialVertexBuffer.primarySummarySha256 !==
      "68133eb819c3055d58580f2bc63dff5b56c116aff6d5ff9762c4428108e1c849" ||
    d3d9NativeInitialVertexBuffer.confirmationTraceBytes !== 285_889 ||
    d3d9NativeInitialVertexBuffer.confirmationTraceLines !== 509 ||
    d3d9NativeInitialVertexBuffer.confirmationTraceSha256 !==
      "e2f7c1cc1d9c073d9325bf05d7f46a44e80bbbce54b0f8ffe08684f6bf393ed8" ||
    d3d9NativeInitialVertexBuffer.confirmationBalancedApiPairs !== 220 ||
    d3d9NativeInitialVertexBuffer.confirmationSummaryBytes !== 282_874 ||
    d3d9NativeInitialVertexBuffer.confirmationSummarySha256 !==
      "7835288860d359c695d98bb7cbc19ecbc004f1d05ec6f54d384eaa7356ec3700" ||
    d3d9NativeInitialVertexBuffer.diagnostics !== 0 ||
    d3d9NativeInitialVertexBuffer.callsPerTrace !== 3 ||
    d3d9NativeInitialVertexBufferCallerRvas.join(",") !==
      "0x0005a890,0x0005aa96,0x0005aab2" ||
    d3d9NativeInitialVertexBufferMethods.join(",") !==
      "CreateVertexBuffer,Lock,Unlock" ||
    d3d9NativeInitialVertexBufferSlots.join(",") !== "26,11,12" ||
    d3d9NativeInitialVertexBufferCreateArguments.join(",") !==
      "112,8,324,1" ||
    d3d9NativeInitialVertexBuffer.createOutputInitiallyNull !== true ||
    d3d9NativeInitialVertexBuffer.createOutputNonNull !== true ||
    d3d9NativeInitialVertexBufferLockArguments.join(",") !== "0,0,0" ||
    d3d9NativeInitialVertexBuffer.lockDataPointerNonNull !== true ||
    d3d9NativeInitialVertexBuffer.initialContentBytes !== 112 ||
    d3d9NativeInitialVertexBuffer.initialContentSha256 !==
      "b5fdab78d8947eacc864bfeecb4d2100780e5afe1cd8efafb124887913ac49fa" ||
    d3d9NativeInitialVertexBuffer.writtenContentBytes !== 112 ||
    d3d9NativeInitialVertexBuffer.writtenContentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    d3d9NativeInitialVertexBufferResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialVertexBufferLastErrors.join(",") !== "0,0,0" ||
    d3d9NativeInitialVertexBuffer.sameDeviceAndVertexBufferPerTrace !== true ||
    d3d9NativeInitialVertexBuffer.nativePointersAreOpaqueArtifactIdentity !== true ||
    d3d9NativeInitialVertexBuffer.immediateNextMethod !== "SetRenderState" ||
    d3d9NativeInitialVertexBuffer.immediateNextSlot !== 57 ||
    d3d9NativeInitialVertexBufferNextArguments.join(",") !== "22,1" ||
    d3d9NativeInitialVertexBuffer.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateInitialVertexBufferNodeSequences.join(",") !==
      "17859,17860,17861" ||
    d3d9PrivateInitialVertexBufferInstructionCounts.join(",") !==
      "11807897,11808429,11808439" ||
    d3d9PrivateInitialVertexBufferChromeSequences.join(",") !==
      "17859,17860,17861" ||
    d3d9PrivateInitialVertexBufferReturnAddresses.join(",") !==
      "0x1005a890,0x1005aa96,0x1005aab2" ||
    d3d9PrivateInitialVertexBufferCreateArguments.join(",") !==
      "0x01010004,0x00000070,0x00000008,0x00000144,0x00000001,0x101517dc,0x00000000" ||
    d3d9PrivateInitialVertexBufferLockArguments.join(",") !==
      "0x01050004,0x00000000,0x00000000,0x006ffd54,0x00000000" ||
    d3d9PrivateInitialVertexBufferUnlockArguments.join(",") !== "0x01050004" ||
    d3d9PrivateInitialVertexBufferResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialVertexBuffer.vertexBufferOutputAddress !== "0x101517dc" ||
    d3d9PrivateInitialVertexBuffer.vertexBufferPointer !== "0x01050004" ||
    d3d9PrivateInitialVertexBuffer.lockOutputAddress !== "0x006ffd54" ||
    d3d9PrivateInitialVertexBuffer.backingAddress !== "0x44000000" ||
    d3d9PrivateInitialVertexBuffer.backingBytes !== 112 ||
    d3d9PrivateInitialVertexBufferDescriptor.join(",") !== "112,8,324,1" ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsTextureCount !== 3 ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsVertexBufferCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.deviceReferenceCountAtStop !== 5 ||
    d3d9PrivateInitialVertexBuffer.vertexBufferReferenceCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.contentGeneration !== 1 ||
    d3d9PrivateInitialVertexBuffer.contentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    d3d9PrivateInitialVertexBuffer.backingProtectionAtStop !== "read-only" ||
    d3d9PrivateInitialVertexBuffer.browserRendererCreated !== false ||
    d3d9PrivateInitialVertexBuffer.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9SetRenderState.slot !== 57 ||
    d3d9SetRenderState.explicitArgumentWords !== 2 ||
    d3d9SetRenderState.returnKind !== "h-result" ||
    d3d9SetRenderState.profileId !==
      "primary-initial-fixed-function-render-states-v1" ||
    JSON.stringify(d3d9SetRenderStateSettings) !==
      JSON.stringify([
        { state: 22, stateName: "D3DRS_CULLMODE", value: 1, valueName: "D3DCULL_NONE" },
        { state: 7, stateName: "D3DRS_ZENABLE", value: 0, valueName: "FALSE" },
        { state: 137, stateName: "D3DRS_LIGHTING", value: 0, valueName: "FALSE" },
        { state: 27, stateName: "D3DRS_ALPHABLENDENABLE", value: 1, valueName: "TRUE" },
      ]) ||
    d3d9SetRenderState.stateAuthority !==
      "runtime/graphics.rs::RuntimeGraphicsDevice" ||
    d3d9SetRenderState.mutation !==
      "owner-plan-preflight-stale-check-targeted-commit" ||
    d3d9SetRenderState.sameValueResult !== "0x00000000" ||
    d3d9SetRenderState.sameValueGenerationChange !== 0 ||
    d3d9SetRenderState.unsupportedVariantResult !== "0x8876086a" ||
    d3d9SetRenderState.invalidDeviceFaultsBeforeMutation !== true ||
    d3d9SetRenderState.lastErrorPolicy !== "preserve" ||
    d3d9NativeInitialRenderStates.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialRenderStates.profileId !==
      "x86-bgi-initial-render-states-v1" ||
    d3d9NativeInitialRenderStates.primaryTraceBytes !== 208_326 ||
    d3d9NativeInitialRenderStates.primaryTraceLines !== 511 ||
    d3d9NativeInitialRenderStates.primaryTraceSha256 !==
      "db86df8455db6ed3e2c36b13618730b7d40b6750012136308148a7fc302d86e9" ||
    d3d9NativeInitialRenderStates.primaryBalancedApiPairs !== 222 ||
    d3d9NativeInitialRenderStates.primarySummaryBytes !== 180_986 ||
    d3d9NativeInitialRenderStates.primarySummarySha256 !==
      "31202c168e9f22f0cec2570d41cdadd95ef9eab602e3bab3e1c8b409f2bafa4b" ||
    d3d9NativeInitialRenderStates.confirmationTraceBytes !== 208_999 ||
    d3d9NativeInitialRenderStates.confirmationTraceLines !== 512 ||
    d3d9NativeInitialRenderStates.confirmationTraceSha256 !==
      "66e7b34f2900bbf5470c6433bcd0d7ea0818e87396cc2108d5ee72a163b8a704" ||
    d3d9NativeInitialRenderStates.confirmationBalancedApiPairs !== 222 ||
    d3d9NativeInitialRenderStates.confirmationSummaryBytes !== 181_209 ||
    d3d9NativeInitialRenderStates.confirmationSummarySha256 !==
      "1d04f1ec63c1cf3769f97267bf06bc6133e21a08aedd9173fa6af5a59c934100" ||
    d3d9NativeInitialRenderStates.diagnostics !== 0 ||
    d3d9NativeInitialRenderStates.callsPerTrace !== 4 ||
    d3d9NativeInitialRenderStateCallerRvas.join(",") !==
      "0x0005ad13,0x0005ad26,0x0005ad3c,0x0005ad50" ||
    d3d9NativeInitialRenderStateStates.join(",") !== "22,7,137,27" ||
    d3d9NativeInitialRenderStateValues.join(",") !== "1,0,0,1" ||
    d3d9NativeInitialRenderStateResults.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialRenderStateLastErrors.join(",") !== "0,0,0,0" ||
    d3d9NativeInitialRenderStates.contiguousSameDeviceAndThreadPerTrace !== true ||
    d3d9NativeInitialRenderStates.nativeDeviceAndThreadValuesAreOpaqueArtifactIdentity !==
      true ||
    d3d9NativeInitialRenderStates.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateInitialRenderStateNodeSequences.join(",") !==
      "17862,17863,17864,17865" ||
    d3d9PrivateInitialRenderStateInstructionCounts.join(",") !==
      "11808458,11808466,11808474,11808482" ||
    d3d9PrivateInitialRenderStateChromeSequences.join(",") !==
      "17862,17863,17864,17865" ||
    d3d9PrivateInitialRenderStateReturnAddresses.join(",") !==
      "0x1005ad13,0x1005ad26,0x1005ad3c,0x1005ad50" ||
    d3d9PrivateInitialRenderStates.thisPointer !== "0x01010004" ||
    d3d9PrivateInitialRenderStateStates.join(",") !== "22,7,137,27" ||
    d3d9PrivateInitialRenderStateValues.join(",") !== "1,0,0,1" ||
    d3d9PrivateInitialRenderStateResults.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialRenderStateGuestWrites.join(",") !== "0,0,0,0" ||
    d3d9PrivateInitialRenderStates.deviceReferenceCountAtStop !== 5 ||
    d3d9PrivateInitialRenderStates.logicalCullMode !== "none" ||
    d3d9PrivateInitialRenderStates.logicalDepthTestEnabled !== false ||
    d3d9PrivateInitialRenderStates.logicalLightingEnabled !== false ||
    d3d9PrivateInitialRenderStates.logicalAlphaBlendEnabled !== true ||
    d3d9PrivateInitialRenderStates.renderStateGenerationDelta !== 4 ||
    d3d9PrivateInitialRenderStates.browserOwnsRenderState !== false ||
    d3d9PrivateInitialRenderStates.browserRendererCreated !== false ||
    d3d9PrivateInitialRenderStates.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9PrivateInitialRenderStates.nextMethod !== "GetRasterStatus" ||
    d3d9PrivateInitialRenderStates.nextSlot !== 19 ||
    d3d9GetSurfaceLevel.slot !== 18 ||
    d3d9GetSurfaceLevel.explicitArgumentWords !== 2 ||
    d3d9GetSurfaceLevel.returnKind !== "h-result" ||
    d3d9GetSurfaceLevel.profileId !== "primary-texture-level-zero-surface-v1" ||
    d3d9GetSurfaceLevel.level !== 0 ||
    d3d9GetSurfaceLevel.sourceTextureVariantIndex !== 0 ||
    d3d9GetSurfaceLevel.otherTextureVariantFault !==
      "get-surface-level-texture-variant-unsupported" ||
    d3d9GetSurfaceLevel.surfaceOutputWriteBytes !== 4 ||
    d3d9GetSurfaceLevel.repeatReturnsSameIdentityAndAddsReference !== true ||
    d3d9GetSurfaceLevel.guestComGraphicsPublicationAtomic !== true ||
    d3d9GetSurfaceLevel.invalidLevelOrNullOutputResult !== "0x8876086c" ||
    d3d9GetSurfaceLevel.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9GetSurfaceLevel.lastErrorPolicy !== "preserve" ||
    d3d9SurfaceInterface.classIdentity !== "d3d9.idirect3dsurface9.v1" ||
    d3d9SurfaceInterface.interface !== "IDirect3DSurface9" ||
    d3d9SurfaceInterface.vtableSlots !== 17 ||
    d3d9SurfaceImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetDC,ReleaseDC" ||
    d3d9SurfaceInterface.initialReferenceCount !== 1 ||
    d3d9SurfaceInterface.retainsTextureInterfaceReference !== true ||
    d3d9SurfaceInterface.liveDeviceContextBlocksRetirement !== true ||
    d3d9SurfaceInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9SurfaceInterface.finalReleaseMayCascadeTextureDeviceAndRootRetirement !== true ||
    d3d9NativeGetSurfaceLevel.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeGetSurfaceLevel.primaryTraceBytes !== 228_637 ||
    d3d9NativeGetSurfaceLevel.primaryTraceLines !== 514 ||
    d3d9NativeGetSurfaceLevel.primaryTraceSha256 !==
      "54aa56bade26d675f864162bb7a7a2f8b4fc30f8df8dbd0835681cc09f9eced2" ||
    d3d9NativeGetSurfaceLevel.primaryBalancedApiPairs !== 222 ||
    d3d9NativeGetSurfaceLevel.primarySummaryBytes !== 230_104 ||
    d3d9NativeGetSurfaceLevel.primarySummarySha256 !==
      "c68ef403d864c89ad466ed3bda3cef750065d7555b815fb23f3ad99a43697c8e" ||
    d3d9NativeGetSurfaceLevel.confirmationTraceBytes !== 229_203 ||
    d3d9NativeGetSurfaceLevel.confirmationTraceLines !== 514 ||
    d3d9NativeGetSurfaceLevel.confirmationTraceSha256 !==
      "e3f6d853293cfa8f395695c2de21cb4675cc71f27efa4b3715f2171575ad1422" ||
    d3d9NativeGetSurfaceLevel.confirmationBalancedApiPairs !== 222 ||
    d3d9NativeGetSurfaceLevel.confirmationSummaryBytes !== 230_131 ||
    d3d9NativeGetSurfaceLevel.confirmationSummarySha256 !==
      "1ed3b5f247a8b6dd82efd2cdaa7cb0393e68d7db9639d07eaa1afa2244b4a81f" ||
    d3d9NativeGetSurfaceLevel.diagnostics !== 0 ||
    d3d9NativeGetSurfaceLevel.successfulCallsPerTrace !== 1 ||
    d3d9NativeGetSurfaceLevel.callerRva !== "0x0005a731" ||
    d3d9NativeGetSurfaceLevel.level !== 0 ||
    d3d9NativeGetSurfaceLevel.result !== "0x00000000" ||
    d3d9NativeGetSurfaceLevel.lastError !== 0 ||
    d3d9NativeGetSurfaceLevel.surfaceOutputNonNull !== true ||
    d3d9NativeGetSurfaceLevel.hresultSignConsumed !== true ||
    d3d9NativeGetSurfaceLevel.immediateNextMethod !== "GetDC" ||
    d3d9NativeGetSurfaceLevel.immediateNextSlot !== 15 ||
    d3d9NativeGetSurfaceLevel.immediateNextCallerRva !== "0x0005a746" ||
    d3d9NativeGetSurfaceLevel.sourceExecutableHashUnchanged !== true ||
    d3d9ControlledGetSurfaceLevel.architecture !== "coff-i386" ||
    d3d9ControlledGetSurfaceLevel.sourceBytes !== 13_977 ||
    d3d9ControlledGetSurfaceLevel.sourceSha256 !==
      "1ec0c191bb9783abcbfae7b92ba6d7136018111c4b1da7c860d7dd343daca562" ||
    d3d9ControlledGetSurfaceLevel.executableBytes !== 12_288 ||
    d3d9ControlledGetSurfaceLevel.executableSha256 !==
      "7ca28333d06b7352b032b0af9ceb22a016e03d5830f17a74231569ef2b2fee9e" ||
    d3d9ControlledGetSurfaceLevel.repeatReturnsSameSurfacePointer !== true ||
    d3d9ControlledGetSurfaceLevel.repeatReturnsSameIunknownIdentity !== true ||
    d3d9ControlledGetSurfaceLevel.surfaceIunknownDistinctFromTextureIunknown !== true ||
    d3d9ControlledGetSurfaceLevel.getContainerReturnsOriginalTexture !== true ||
    d3d9ControlledGetSurfaceLevel.invalidLevelResult !== "0x8876086c" ||
    d3d9ControlledGetSurfaceLevel.invalidLevelZerosOutput !== true ||
    d3d9ControlledGetSurfaceLevel.nullOutputException !== "0xc0000005" ||
    d3d9ControlledGetSurfaceLevel.successPreservesLastError !== true ||
    d3d9PrivateGetSurfaceLevel.nodeSequence !== 17_851 ||
    d3d9PrivateGetSurfaceLevel.nodeInstructionCount !== 11_807_783 ||
    d3d9PrivateGetSurfaceLevel.chromeSequence !== 17_851 ||
    d3d9PrivateGetSurfaceLevelArguments.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    d3d9PrivateGetSurfaceLevel.returnAddress !== "0x1005a731" ||
    d3d9PrivateGetSurfaceLevel.result !== "0x00000000" ||
    d3d9PrivateGetSurfaceLevel.surfaceOutputAddress !== "0x006ffd38" ||
    d3d9PrivateGetSurfaceLevel.surfacePointer !== "0x01030004" ||
    d3d9PrivateGetSurfaceLevel.guestWriteBytes !== 4 ||
    d3d9PrivateGetSurfaceLevel.surfaceObjectId !== 5 ||
    d3d9PrivateGetSurfaceLevel.parentInterfaceId !== 8 ||
    d3d9PrivateGetSurfaceLevel.surfaceInterfaceId !== 10 ||
    d3d9PrivateGetSurfaceLevel.surfaceVtable !== "0x0103001c" ||
    d3d9PrivateGetSurfaceLevel.surfaceVtableSlots !== 17 ||
    d3d9PrivateGetSurfaceLevel.textureReferenceCount !== 2 ||
    d3d9PrivateGetSurfaceLevel.surfaceReferenceCount !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsTextureId !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceId !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceNamespace !==
      "vine.logical-graphics-surface.v1" ||
    d3d9PrivateSurfaceDescriptor.join(",") !== "0,1024,1024,22" ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceCount !== 1 ||
    d3d9PrivateGetSurfaceLevel.browserRendererCreated !== false ||
    d3d9PrivateGetSurfaceLevel.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9SurfaceDc.profileId !== "primary-surface-gdi-dc-v1" ||
    d3d9SurfaceDc.getDcSlot !== 15 ||
    d3d9SurfaceDc.getDcExplicitArgumentWords !== 1 ||
    d3d9SurfaceDc.releaseDcSlot !== 16 ||
    d3d9SurfaceDc.releaseDcExplicitArgumentWords !== 1 ||
    d3d9SurfaceDc.returnKind !== "h-result" ||
    d3d9SurfaceDc.outputWriteBytes !== 4 ||
    d3d9SurfaceDc.deviceContextAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    d3d9SurfaceDc.graphicsSurfaceAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    d3d9SurfaceDc.surfaceRelation !==
      "non-owning-revalidated-live-com-and-graphics-identity" ||
    d3d9SurfaceDc.singleOutstandingDeviceContextPerSurface !== true ||
    d3d9SurfaceDc.acquisitionAndGuestWriteAtomic !== true ||
    d3d9SurfaceDc.repeatGetDcResult !== "0x8876086c" ||
    d3d9SurfaceDc.wrongOrRepeatedReleaseDcResult !== "0x8876086c" ||
    d3d9SurfaceDc.liveDeviceContextBlocksSurfaceRetirement !== true ||
    d3d9SurfaceDc.fillRectWritesBoundedCommandHistory !== true ||
    d3d9SurfaceDc.lastErrorPolicy !== "preserve" ||
    d3d9SurfaceDc.pixelStorageCreated !== false ||
    d3d9SurfaceDc.browserRendererCreated !== false ||
    d3d9NativeSurfaceDc.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeSurfaceDc.profileId !== "x86-bgi-system-memory-surface-dc-fill-v1" ||
    d3d9NativeSurfaceDc.primaryTraceBytes !== 259_841 ||
    d3d9NativeSurfaceDc.primaryTraceLines !== 519 ||
    d3d9NativeSurfaceDc.primaryTraceSha256 !==
      "3d0010cace0f0e13e3f0c6918b57d3aaac46afb8b2fb399d16ec2197de41acef" ||
    d3d9NativeSurfaceDc.primaryBalancedApiPairs !== 225 ||
    d3d9NativeSurfaceDc.primarySummaryBytes !== 235_461 ||
    d3d9NativeSurfaceDc.primarySummarySha256 !==
      "0a08225c4a15cd82f15d5401a8247b2b301bad1e787551fcb1f34e34c26b4192" ||
    d3d9NativeSurfaceDc.confirmationTraceBytes !== 258_887 ||
    d3d9NativeSurfaceDc.confirmationTraceLines !== 518 ||
    d3d9NativeSurfaceDc.confirmationTraceSha256 !==
      "8e5109f42a539e04421bf2ac33fa85a98b27a084b52e2b2f2738f1861393c03c" ||
    d3d9NativeSurfaceDc.confirmationBalancedApiPairs !== 224 ||
    d3d9NativeSurfaceDc.confirmationSummaryBytes !== 235_576 ||
    d3d9NativeSurfaceDc.confirmationSummarySha256 !==
      "2b8e16261cdc17afc3b8779b94ecae3de9a4265277105aba0dad82277872010c" ||
    d3d9NativeSurfaceDc.diagnostics !== 0 ||
    d3d9NativeSurfaceDc.callsPerTrace !== 5 ||
    d3d9NativeSurfaceDcCallerRvas.join(",") !==
      "0x0005a746,0x0005a76c,0x0005a77d,0x0005a78e,0x0005a79a" ||
    d3d9NativeSurfaceDcMethods.join(",") !==
      "GetDC,GetStockObject,FillRect,ReleaseDC,Release" ||
    d3d9NativeSurfaceDc.getDcResult !== "0x00000000" ||
    d3d9NativeSurfaceDc.getDcOutputNonNull !== true ||
    d3d9NativeSurfaceDc.stockObjectIndex !== 4 ||
    d3d9NativeSurfaceDc.stockObjectResultNonNull !== true ||
    d3d9NativeSurfaceDcRect.join(",") !== "0,0,800,600" ||
    d3d9NativeSurfaceDc.fillRectResult !== 1 ||
    d3d9NativeSurfaceDc.releaseDcResult !== "0x00000000" ||
    d3d9NativeSurfaceDc.surfaceReleaseResult !== 1 ||
    d3d9NativeSurfaceDcLastErrors.join(",") !== "0,0,0,0,0" ||
    d3d9NativeSurfaceDc.sourceExecutableHashUnchanged !== true ||
    d3d9ControlledSurfaceDc.architecture !== "coff-i386" ||
    d3d9ControlledSurfaceDc.sourceBytes !== 16_434 ||
    d3d9ControlledSurfaceDc.sourceSha256 !==
      "33d79e003ff050f8e22787629b375d60c1650a331148d7e7ef40ed68654ad685" ||
    d3d9ControlledSurfaceDc.executableBytes !== 8_704 ||
    d3d9ControlledSurfaceDc.executableSha256 !==
      "056d6fd36e5cfc14840b63137219e8e3fa36bcfb396104d1d1cba6faf7e6bd00" ||
    d3d9ControlledSurfaceDc.nullOutputExecutableSha256 !==
      "f533018f6f7619ce6d67b27ebee5e0b2f9ca8401f7e4c5abf1a7585e70d68e6f" ||
    d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256 !==
      "ad7b34ea2ddd5206d050cd8550b0fb8074f9ceafacbc9819d330590c0759f6eb" ||
    d3d9ControlledSurfaceDc.firstGetDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.repeatGetDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.repeatGetDcPreservesOutput !== true ||
    d3d9ControlledSurfaceDc.reacquireGetDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDcReferenceCounts.join(",") !== "2,2,2" ||
    d3d9ControlledSurfaceDc.wrongReleaseDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.repeatedReleaseDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.correctReleaseDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.lockRectWhileDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.lockRectAfterReleaseResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.nullOutputException !== "0xc0000005" ||
    d3d9ControlledSurfaceDc.readOnlyOutputException !== "0xc0000005" ||
    d3d9ControlledSurfaceDc.successPreservesLastError !== true ||
    d3d9PrivateSurfaceDcNodeSequences.join(",") !==
      "17852,17853,17854,17855,17856" ||
    d3d9PrivateSurfaceDcInstructionCounts.join(",") !==
      "11807793,11807805,11807812,11807820,11807826" ||
    d3d9PrivateSurfaceDcChromeSequences.join(",") !==
      "17852,17853,17854,17855,17856" ||
    d3d9PrivateSurfaceDcReturnAddresses.join(",") !==
      "0x1005a746,0x1005a76c,0x1005a77d,0x1005a78e,0x1005a79a" ||
    d3d9PrivateSurfaceDc.surfacePointer !== "0x01030004" ||
    d3d9PrivateSurfaceDc.getDcOutputAddress !== "0x006ffd3c" ||
    d3d9PrivateSurfaceDc.deviceContextHandle !== "0x47010002" ||
    d3d9PrivateSurfaceDc.deviceContextId !== 2 ||
    d3d9PrivateSurfaceDc.deviceContextNamespace !== "vine.logical-gdi-dc.v1" ||
    d3d9PrivateSurfaceDc.deviceContextTargetKind !== "graphics-surface" ||
    d3d9PrivateSurfaceDc.logicalGraphicsSurfaceId !== 1 ||
    d3d9PrivateSurfaceDc.surfaceComObjectId !== 5 ||
    d3d9PrivateSurfaceDcGeometry.join(",") !== "1024,1024" ||
    d3d9PrivateSurfaceDc.surfaceColorFormat !== "b8g8r8x8-unorm" ||
    d3d9PrivateSurfaceDc.stockBrushHandle !== "0x47020001" ||
    d3d9PrivateSurfaceDcFillRect.join(",") !== "0,0,800,600" ||
    d3d9PrivateSurfaceDc.fillColorRef !== 0 ||
    d3d9PrivateSurfaceDc.surfaceContentGeneration !== 1 ||
    d3d9PrivateSurfaceDc.surfaceGeneration !== 4 ||
    d3d9PrivateSurfaceDc.surfaceReleaseReferenceCount !== 0 ||
    d3d9PrivateSurfaceDc.surfaceObjectRetired !== true ||
    d3d9PrivateSurfaceDc.logicalGraphicsSurfaceCountAfterRelease !== 0 ||
    d3d9PrivateSurfaceDc.secondCreateTextureSequence !== 17_857 ||
    d3d9PrivateSurfaceDc.secondCreateTextureInstructionCount !== 11_807_840 ||
    d3d9PrivateSurfaceDc.secondCreateTexturePool !== 0 ||
    d3d9PrivateSurfaceDc.secondCreateTextureResult !== "0x00000000" ||
    d3d9PrivateSurfaceDc.secondCreateTextureOutputAddress !== "0x101517d0" ||
    d3d9PrivateSurfaceDc.secondCreateTexturePointer !== "0x01030004" ||
    d3d9PrivateSurfaceDc.secondCreateTextureGuestWriteBytes !== 4 ||
    d3d9PrivateSurfaceDc.browserRendererCreated !== false ||
    d3d9PrivateSurfaceDc.pixelStorageCreated !== false ||
    d3d9PrivateSurfaceDc.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9.sharedGraphicsBoundary !==
      "RuntimeGraphics-owned-logical-device-texture-surface-vertex-buffer-and-render-state-at-D3D9-gateways" ||
    d3d9.runtimeGraphicsCreated !== true ||
    d3d9.runtimeTextureCreated !== true ||
    d3d9.runtimeSurfaceCreated !== true ||
    d3d9.runtimeVertexBufferCreated !== true ||
    d3d9.runtimeRenderStatesSet !== true ||
    d3d9.rendererResourceCreated !== false ||
    d3d9.d3d8Implemented !== false ||
    d3d9.d3d10Or11Implemented !== false ||
    d3d9.generalD3d9CompatibilityClaimed !== false
  ) {
    errors.push("active D3D9 frontend boundary is invalid");
  }
  const gdi = requireRecord(
    active.runtimeGdiFrontend,
    "activeRuntimeCheckpoint.runtimeGdiFrontend",
    errors,
  );
  const gdiStockObjectIndices = requireIntegerArray(
    gdi.implementedStockObjectIndices,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.implementedStockObjectIndices",
    errors,
  );
  const gdiSurfaceGeometry = requireIntegerArray(
    gdi.surfaceGeometry,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.surfaceGeometry",
    errors,
  );
  const gdiTeardownOrder = requireStringArray(
    gdi.processTeardownOrder,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.processTeardownOrder",
    errors,
  );
  const gdiImplementedApis = requireStringArray(
    gdi.implementedApis,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.implementedApis",
    errors,
  );
  if (
    gdi.schema !== "vine.runtime-gdi-frontend.v1" ||
    gdi.status !==
      "window-and-graphics-surface-dc-fill-slices-implemented-and-verified" ||
    gdi.stateOwner !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi.aggregateCoordinator !== "runtime/kernel.rs::RuntimeKernel" ||
    gdi.mutableCollectionVisibility !== "private" ||
    gdi.readInspection !== "immutable-borrow" ||
    gdi.windowAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    gdi.windowRelation !==
      "non-owning-revalidated-before-each-cross-subsystem-operation" ||
    gdi.graphicsAuthority !== "runtime/graphics.rs::RuntimeGraphics" ||
    gdi.graphicsSurfaceRelation !==
      "non-owning-revalidated-live-com-and-graphics-identity" ||
    gdi.deviceContextHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    gdi.deviceContextHandleBase !== "0x47010001" ||
    gdi.deviceContextAcquisitionIdentity !== "distinct-monotonic-per-getdc" ||
    gdi.deviceContextTargetVariants?.join(",") !== "window,graphics-surface" ||
    gdi.graphicsSurfaceOutstandingDeviceContexts !==
      "at-most-one-per-surface" ||
    gdi.graphicsSurfaceDeviceContextRetainsComReference !== false ||
    gdi.deviceContextSerializationCompatibility !==
      "window-shape-preserved-graphics-target-explicitly-typed" ||
    gdi.nativeNumericHandleReuseImported !== false ||
    gdi.gdiObjectHandleNamespace !== "vine.logical-gdi-object.v1" ||
    gdi.gdiObjectHandleBase !== "0x47020001" ||
    gdiStockObjectIndices.join(",") !== "4" ||
    gdi.stockObjectLifetime !== "process-shared-until-teardown" ||
    gdi.surfaceIdentity !== "one-logical-surface-per-window" ||
    gdiSurfaceGeometry.join(",") !== "800,600" ||
    gdi.surfaceFillHistoryBounded !== true ||
    gdi.surfaceGeneration !== 1 ||
    gdi.graphicsSurfaceFillAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    gdi.graphicsSurfaceFillHistoryBounded !== true ||
    gdi.graphicsSurfaceFillCapacity !== 16_384 ||
    gdi.graphicsSurfaceContentGeneration !== true ||
    gdi.graphicsSurfacePixelStorageCreated !== false ||
    gdi.releaseLifetime !== "retire-only-matching-acquisition-hdc" ||
    gdi.liveGraphicsSurfaceDeviceContextBlocksComRetirement !== true ||
    gdi.mutation !== "owner-plan-stale-check-targeted-commit" ||
    gdi.failureAtomicity !== true ||
    gdiTeardownOrder.join(",") !== "gdi,graphics,user32" ||
    gdi.browserOwnsWindowsState !== false ||
    gdi.runtimeGraphicsCreated !== true ||
    gdi.browserRendererResourceCreated !== false ||
    gdi.sharedGraphicsBoundary !==
      "RuntimeGraphics-logical-surface-fill-command-history" ||
    gdi.gdiAndDirect3dFrontendsRemainSeparate !== true ||
    gdiImplementedApis.join(",") !==
      "GetDC,GetStockObject,FillRect,ReleaseDC" ||
    gdi.generalGdiRenderingClaimed !== false
  ) {
    errors.push("active GDI frontend boundary is invalid");
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
    modules.subsystemTests < 8 ||
    modules.crossSubsystemAtomicTests < 5 ||
    modules.registrationRollbackMutation !== false ||
    modules.dynamicRuntimeSystemModuleLoad !==
      "owner-plan-object-preflight-targeted-commit" ||
    modules.dynamicRuntimeSystemModuleHandleAllocation !==
      "module-owner-monotonic-slot" ||
    modules.dynamicRuntimeSystemModuleStaleGuard !==
      "nonserialized-module-graph-mutation-generation" ||
    modules.dynamicRuntimeSystemModuleGraphRootCopies !== false ||
    modules.dynamicRuntimeSystemModuleExistingIdentityReuse !== true ||
    modules.dynamicRuntimeSystemModuleThreadReferenceCounting !== true ||
    modules.dynamicRuntimeSystemModuleCallbackExecution !== false ||
    modules.dynamicRuntimeSystemModuleFailureAtomicity !== true ||
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
  const sourceFiles = requireRecord(
    kernel32Policy.sourceFiles,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.sourceFiles",
    errors,
  );
  const loadLibraryA = requireRecord(
    kernel32Policy.loadLibraryA,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.loadLibraryA",
    errors,
  );
  if (
    kernel32Policy.schema !== "vine.kernel32-compatibility-policy" ||
    kernel32Policy.schemaVersion !== 2 ||
    kernel32Policy.id !== "kernel32-d3dx-runtime-module-v1" ||
    kernel32Policy.source !==
      "fixtures/runtime-policies/kernel32-d3dx-runtime-module-v1.json" ||
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
    sourceFiles.authority !== "runtime/files.rs::RuntimeFiles" ||
    sourceFiles.sourceManifestAuthority !== "rust-runtime-files" ||
    sourceFiles.browserRole !== "same-origin-byte-service" ||
    sourceFiles.allowedDesiredAccess?.join(",") !== `${0x8000_0000}` ||
    sourceFiles.allowedShareModes?.join(",") !== "1" ||
    sourceFiles.allowedCreationDispositions?.join(",") !== "3" ||
    sourceFiles.allowedFlagsAndAttributes?.join(",") !== `${0x0800_0020}` ||
    sourceFiles.maximumPathBytes !== 32_768 ||
    sourceFiles.maximumReadBytes !== 1_048_576 ||
    sourceFiles.handleNamespace !== "vine.runtime-source-file.v1" ||
    sourceFiles.mutation !== "owner-plan-stale-check-targeted-commit" ||
    sourceFiles.completionValidation !==
      "request-thread-generation-lifetime-before-commit" ||
    sourceFiles.immutableSourceAndWritableOverlaySeparated !== true ||
    sourceFiles.browserOwnsWindowsFileState !== false ||
    loadLibraryA.maximumModuleNameBytes !== 260 ||
    JSON.stringify(loadLibraryA.runtimeSystemModules) !==
      JSON.stringify([
        {
          canonicalName: "d3dx9_43.dll",
          exports: ["D3DXCreateEffect"],
        },
      ]) ||
    loadLibraryA.moduleAuthority !== "runtime/modules.rs::RuntimeModules" ||
    loadLibraryA.objectAuthority !== "runtime/objects.rs::RuntimeObjectTable" ||
    loadLibraryA.exportAuthority !== "runtime/registries.rs::RuntimeRegistries" ||
    loadLibraryA.browserOwnsWindowsModuleState !== false ||
    loadLibraryA.nativeImageMapping !== false ||
    loadLibraryA.generalSearch !== false ||
    loadLibraryA.dllMainCallbacks !== false ||
    loadLibraryA.unload !== false ||
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
  const sourceSnapshot = requireRecord(
    active.sourceSnapshot,
    "activeRuntimeCheckpoint.sourceSnapshot",
    errors,
  );
  if (
    sourceSnapshot.schema !== "vine.source-snapshot-manifest" ||
    sourceSnapshot.schemaVersion !== 1 ||
    sourceSnapshot.source !== "fixtures/source-snapshots/bgi-gdb-v1.json" ||
    sourceSnapshot.titleFingerprint !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    sourceSnapshot.snapshotId !== "bgi-gdb-e14899243887737b-v1" ||
    sourceSnapshot.sourceId !== 1 ||
    sourceSnapshot.rootPathA?.join(",") !== "67,58,92,86,105,110,101,0" ||
    sourceSnapshot.fileCount !== 1 ||
    sourceSnapshot.objectId !== 1 ||
    sourceSnapshot.relativePathA?.join(",") !== "66,71,73,46,103,100,98" ||
    sourceSnapshot.byteLength !== 125_461 ||
    sourceSnapshot.sha256 !==
      "e14899243887737b5fd1bddcf35d9fb3e74dc5f7ff0eea7198e1821f9a4d0f92" ||
    sourceSnapshot.immutableAcrossRun !== true ||
    sourceSnapshot.privateSourcePathCommitted !== false
  ) {
    errors.push("active immutable source snapshot boundary is invalid");
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
  const monitorFromWindowVariants = requireStringArray(
    user32Policy.monitorFromWindowVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.monitorFromWindowVariants",
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
  const loadImageAVariants = requireStringArray(
    user32Policy.loadImageAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadImageAVariants",
    errors,
  );
  const sendMessageAVariants = requireStringArray(
    user32Policy.sendMessageAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.sendMessageAVariants",
    errors,
  );
  const setWindowPosVariants = requireStringArray(
    user32Policy.setWindowPosVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.setWindowPosVariants",
    errors,
  );
  const setWindowLongAVariants = requireStringArray(
    user32Policy.setWindowLongAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.setWindowLongAVariants",
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
  const createWindowExAVariants = requireStringArray(
    user32Policy.createWindowExAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.createWindowExAVariants",
    errors,
  );
  const windowCreationMessages = requireStringArray(
    user32Policy.windowCreationMessages,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.windowCreationMessages",
    errors,
  );
  const callbackScratchArena = requireRecord(
    user32Policy.callbackScratchArena,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.callbackScratchArena",
    errors,
  );
  const defWindowProcAImplementedMessages = requireIntegerArray(
    user32Policy.defWindowProcAImplementedMessages,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.defWindowProcAImplementedMessages",
    errors,
  );
  const getSystemMenuFailure = requireRecord(
    user32Policy.getSystemMenuFailure,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.getSystemMenuFailure",
    errors,
  );
  const enableMenuItemFailure = requireRecord(
    user32Policy.enableMenuItemFailure,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.enableMenuItemFailure",
    errors,
  );
  if (
    user32Policy.schema !== "vine.user32-compatibility-policy" ||
    user32Policy.schemaVersion !== 16 ||
    user32Policy.id !==
      "user32-single-monitor-normal-mouse-icon-cursor-class-metrics-window-menu-small-icon-position-style-activation-monitor-v16" ||
    user32Policy.source !==
      "crates/vine-core/src/user32.rs::User32CompatibilityPolicy::controlled_v16" ||
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
    user32Policy.monitorFromWindowProfileId !==
      "live-window-default-to-primary-v1" ||
    user32Policy.monitorFromWindowFlags !== 1 ||
    monitorFromWindowVariants.join(",") !== "live-window-default-to-primary" ||
    user32Policy.monitorFromWindowReturnNamespace !==
      "vine.logical-monitor.v1" ||
    user32Policy.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    user32Policy.monitorFromWindowMutation !==
      "logical-display-authority-read-only" ||
    user32Policy.monitorFromWindowFailureAtomicity !== true ||
    user32Policy.monitorFromWindowHostDisplayTopologyImported !== false ||
    user32Policy.monitorFromWindowBrowserDisplayTopologyImported !== false ||
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
    user32Policy.loadImageAProfileId !== "module-shared-small-icon-16x16-v1" ||
    user32Policy.loadImageAImageType !== 1 ||
    user32Policy.loadImageATargetWidth !== 16 ||
    user32Policy.loadImageATargetHeight !== 16 ||
    user32Policy.loadImageAFlags !== 32_768 ||
    user32Policy.loadImageAMaximumGroupEntries !== 64 ||
    loadImageAVariants.join(",") !==
      "mapped-module-integer-resource-image-icon-lr-shared" ||
    user32Policy.loadImageALastErrorPolicy !==
      "handler-success-zero-specific-failure-code" ||
    user32Policy.sendMessageAProfileId !== "same-thread-wm-seticon-small-v1" ||
    sendMessageAVariants.join(",") !==
      "same-thread-created-window-wm-seticon-icon-small-live-shared-icon" ||
    user32Policy.sendMessageAMessage !== 128 ||
    user32Policy.sendMessageAWParam !== 0 ||
    user32Policy.sendMessageAIconWidth !== 16 ||
    user32Policy.sendMessageAIconHeight !== 16 ||
    user32Policy.sendMessageACallbackContract !==
      "registered-class-wndproc-stdcall-four-u32-no-scratch-return-u32" ||
    user32Policy.sendMessageALastErrorPolicy !== "preserve" ||
    user32Policy.sendMessageAMutation !==
      "defwindowproc-wm-seticon-owner-plan-targeted-commit" ||
    user32Policy.sendMessageAFailureAtomicity !== true ||
    user32Policy.setWindowPosProfileId !==
      "initial-offscreen-show-hide-same-geometry-activation-v2" ||
    setWindowPosVariants.join(",") !==
      "initial-offscreen-show,initial-hide-preserve-position,same-geometry-activation" ||
    user32Policy.setWindowPosInsertAfter !== "0xfffffffe" ||
    user32Policy.setWindowPosInitialShowScreenMultiplier !== 8 ||
    user32Policy.setWindowPosInitialShowFlags !== 1_369 ||
    user32Policy.setWindowPosInitialHideFlags !== 1_435 ||
    user32Policy.setWindowPosInitialShowCallbackFlags !== 268_438_873 ||
    user32Policy.setWindowPosInitialHideCallbackFlags !== 7_583 ||
    user32Policy.setWindowPosGetIconWParam !== 2 ||
    user32Policy.setWindowPosActivationFlags !== 266 ||
    user32Policy.setWindowPosActivationInternalFlags !== 3 ||
    user32Policy.setWindowPosActivationInsertAfter !== "0x00000000" ||
    user32Policy.setWindowPosActivateAppOtherThreadId !== 0 ||
    user32Policy.setWindowPosImeSetContextFlags !== "0xc000000f" ||
    user32Policy.setWindowPosImeNotifyCommand !== 2 ||
    user32Policy.setWindowPosCallbackContract !==
      "initial-wm-windowposchanged-depth-two-wm-move-same-depth-wm-geticon-or-activation-two-wm-windowposchanging-wm-activateapp-wm-ncactivate-wm-activate-depth-three-ime-and-focus" ||
    user32Policy.setWindowPosMutation !==
      "owner-operation-generation-callback-plan-targeted-commit" ||
    user32Policy.setWindowPosActivationAuthority !==
      "runtime/user32.rs::RuntimeUser32-thread-active-focus-process-foreground-relations" ||
    user32Policy.setWindowPosActivationCommitOrder !==
      "first-windowposchanging-return-active-and-foreground-then-wm-activate-default-focus" ||
    user32Policy.setWindowPosHostIdentityNormalization !==
      "native-z-order-hwnd-and-external-thread-id-not-imported" ||
    user32Policy.setWindowPosFailureAtomicity !== true ||
    user32Policy.setWindowLongAProfileId !==
      "main-window-gwl-style-visible-v1" ||
    setWindowLongAVariants.join(",") !==
      "created-window-gwl-style-reached-value,invalid-window,invalid-index" ||
    user32Policy.setWindowLongAStyleIndex !== -16 ||
    user32Policy.setWindowLongARequestedStyle !== "0x90ca0000" ||
    user32Policy.setWindowLongARetainedStyleMask !== "0x04000000" ||
    user32Policy.setWindowLongAInvalidIndex !== -100 ||
    user32Policy.setWindowLongASuccessLastErrorPolicy !== "preserve" ||
    user32Policy.setWindowLongAMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.setWindowLongAIdempotentUpdatePreservesGeneration !== true ||
    user32Policy.setWindowLongAFailureAtomicity !== true ||
    user32Policy.secondIconHandle !== "0x49010002" ||
    user32Policy.largeAndSmallIconResourcesUseDistinctIdentity !== true ||
    user32Policy.sharedIconMutation !== "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedResourceSharesIdentity !== true ||
    user32Policy.processExitRetiresIcons !== true ||
    user32Policy.hostIconImported !== false ||
    user32Policy.browserIconImported !== false ||
    user32Policy.systemIconImplemented !== false ||
    user32Policy.stringResourceNameImplemented !== false ||
    user32Policy.pngIconImplemented !== false ||
    user32Policy.logicalCursorAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.loadCursorProfileId !==
      "system-arrow-and-module-shared-cursor-v2" ||
    user32Policy.loadCursorArrowResourceId !== 32_512 ||
    user32Policy.loadCursorTargetWidth !== 32 ||
    user32Policy.loadCursorTargetHeight !== 32 ||
    user32Policy.loadCursorMaximumGroupEntries !== 64 ||
    loadCursorVariants.join(",") !==
      "null-instance-idc-arrow,mapped-module-integer-resource" ||
    user32Policy.loadCursorSuccessLastErrorPolicy !==
      "system-preserve-module-zero" ||
    user32Policy.loadCursorMissingResourceId !== 65_535 ||
    user32Policy.loadCursorMissingResourceLastError !== 1_814 ||
    user32Policy.loadCursorModuleMissingResourceId !== 65_535 ||
    user32Policy.loadCursorModuleMissingResourceLastError !== 1_813 ||
    user32Policy.loadCursorModuleWrongTypeResourceId !== 101 ||
    user32Policy.loadCursorModuleWrongTypeLastError !== 1_813 ||
    user32Policy.cursorHandleNamespace !== "vine.logical-cursor.v1" ||
    user32Policy.firstCursorHandle !== "0x43010001" ||
    user32Policy.secondCursorHandle !== "0x43010002" ||
    user32Policy.sharedCursorMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedSystemCursorSharesIdentity !== true ||
    user32Policy.repeatedModuleCursorSharesIdentity !== true ||
    user32Policy.processExitRetiresCursors !== true ||
    user32Policy.hostCursorImported !== false ||
    user32Policy.browserCursorImported !== false ||
    user32Policy.moduleCursorImplemented !== true ||
    user32Policy.moduleCursorResourceAuthority !==
      "mapped-module-pe-resource-catalog" ||
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
    user32Policy.logicalWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.createWindowExAProfileId !==
      "main-local-window-800x600-v1" ||
    createWindowExAVariants.join(",") !==
      "mapped-local-main-class-top-level-null-relations" ||
    user32Policy.createWindowExARequestedStyle !== "0x80ca0000" ||
    user32Policy.createWindowExAEffectiveStyle !== "0x84ca0000" ||
    user32Policy.windowHandleNamespace !== "vine.logical-window.v1" ||
    user32Policy.firstWindowHandle !== "0x48010001" ||
    user32Policy.windowCreationMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    windowCreationMessages.join(",") !==
      "wm-nccreate,wm-nccalcsize,wm-create,wm-size,wm-move" ||
    user32Policy.windowCreationCallbackReplacement !==
      "same-depth-retire-and-enter-after-preflight" ||
    callbackScratchArena.base !== "0x00501000" ||
    callbackScratchArena.bytes !== 65_536 ||
    callbackScratchArena.slotBytes !== 4_096 ||
    user32Policy.callbackScratchMutation !==
      "address-memory-stack-and-frame-preflight-transaction" ||
    defWindowProcAImplementedMessages.join(",") !==
      "129,131,5,3,128,71,70,28,134,6,641,642,7" ||
    user32Policy.defWindowProcALparamContract !==
      "bounded-message-dependent-scalar-or-pointer-union" ||
    user32Policy.invalidMenuProfileId !==
      "null-window-system-menu-close-item-v1" ||
    getSystemMenuFailure.result !== "0x00000000" ||
    getSystemMenuFailure.lastError !== 1_400 ||
    enableMenuItemFailure.item !== "0x0000f060" ||
    enableMenuItemFailure.flags !== 0 ||
    enableMenuItemFailure.result !== "0xffffffff" ||
    enableMenuItemFailure.lastError !== 1_401 ||
    user32Policy.processExitRetiresWindows !== true ||
    user32Policy.hostWindowImported !== false ||
    user32Policy.browserWindowImported !== false ||
    user32Policy.generalMenuIdentityImplemented !== false ||
    user32Policy.windowVisibilityImplemented !== true ||
    user32Policy.windowEffectiveStyleMutationImplemented !== true ||
    user32Policy.windowGeometryMutationImplemented !== true ||
    user32Policy.windowTopmostStateImplemented !== true ||
    user32Policy.windowActivationRelationsImplemented !== true ||
    user32Policy.generalWindowActivationImplemented !== false ||
    user32Policy.multiWindowActivationImplemented !== false ||
    user32Policy.generalWindowResizeImplemented !== false ||
    user32Policy.generalWindowZOrderImplemented !== false ||
    user32Policy.windowMessageQueueIntegrated !== false ||
    user32Policy.logicalWindowIconMutationImplemented !== true ||
    user32Policy.browserWindowIconMutationImplemented !== false ||
    user32Policy.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-small-icon-16x16-v3" ||
    getSystemMetricsIndices.join(",") !== "0,1,4,7,8,49,50" ||
    user32Policy.getSystemMetricsCaptionHeight !== 23 ||
    user32Policy.getSystemMetricsFixedFrameWidth !== 3 ||
    user32Policy.getSystemMetricsFixedFrameHeight !== 3 ||
    user32Policy.getSystemMetricsSmallIconWidth !== 16 ||
    user32Policy.getSystemMetricsSmallIconHeight !== 16 ||
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
    winmmPolicy.id !== "winmm-timecaps-1-1000000-period-1-tick-1-v1" ||
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
    winmmPolicy.timeGetTimeProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    winmmPolicy.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    winmmPolicy.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    winmmPolicy.timeGetTimeIncrementPerRead !== 1 ||
    winmmPolicy.timerResolutionAffectsSampledClock !== false ||
    winmmPolicy.lastErrorPolicy !== "preserve" ||
    winmmPolicy.hostTimerCapabilitiesImported !== false ||
    winmmPolicy.browserTimerCapabilitiesImported !== false ||
    winmmPolicy.hostTimerResolutionImported !== false ||
    winmmPolicy.browserTimerResolutionImported !== false ||
    winmmPolicy.hostClockImported !== false ||
    winmmPolicy.browserClockImported !== false ||
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
  for (const field of [
    "nativeTimeGetTimePrimaryTraceSha256",
    "nativeTimeGetTimePrimarySummarySha256",
    "nativeTimeGetTimeConfirmationTraceSha256",
    "nativeTimeGetTimeConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
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
    latestEvidence.vfsBackendImplemented !== true
  ) {
    errors.push("latest drive-type authority evidence is invalid");
  }
  requireSha256(
    latestEvidence.sourceFileReadPayloadSha256,
    "activeRuntimeCheckpoint.latestEvidence.sourceFileReadPayloadSha256",
    errors,
  );
  if (
    latestEvidence.sourceFileCreateImplemented !== true ||
    latestEvidence.sourceFileGetSizeImplemented !== true ||
    latestEvidence.sourceFileReadImplemented !== true ||
    latestEvidence.sourceFileSetPointerImplemented !== true ||
    latestEvidence.sourceFileCloseImplemented !== true ||
    latestEvidence.sourceFileCreateCallCount !== 2 ||
    latestEvidence.sourceFileGetSizeCallCount !== 2 ||
    latestEvidence.sourceFileReadCallCount !== 2 ||
    latestEvidence.sourceFileSetPointerCallCount !== 0 ||
    latestEvidence.sourceFileCloseCallCount !== 2 ||
    latestEvidence.sourceFileReadSequences?.join(",") !== "1362,1378" ||
    latestEvidence.sourceFileReadRequestedBytes !== 125_461 ||
    latestEvidence.sourceFileReadTransferredBytes !== 125_461 ||
    latestEvidence.sourceFileReadOffsets?.join(",") !== "0,0" ||
    latestEvidence.sourceFileReadRequestIds?.join(",") !== "1,2" ||
    latestEvidence.sourceFileReadObjectId !== 1 ||
    latestEvidence.sourceFileReadSourceId !== 1 ||
    latestEvidence.sourceFileMappingOperationCount !== 43 ||
    latestEvidence.sourceFileHostDirtyPageCount !== 958 ||
    latestEvidence.sourceFileHeapPageMaterializationCount !== 31 ||
    latestEvidence.sourceFileCallbackEventCount !== 2 ||
    latestEvidence.sourceFileReadFailureAtomicity !== true ||
    latestEvidence.sourceFileReadPendingCancellation !== true ||
    latestEvidence.sourceFileReadProcessExitTeardown !== true ||
    latestEvidence.sourceFileReadBrowserOwnsWindowsState !== false ||
    latestEvidence.sourceSnapshotId !== "bgi-gdb-e14899243887737b-v1" ||
    latestEvidence.sourceSnapshotManifest !== "fixtures/source-snapshots/bgi-gdb-v1.json" ||
    latestEvidence.sourceSnapshotImmutableAcrossRun !== true ||
    latestEvidence.sourceSnapshotPrivatePathCommitted !== false
  ) {
    errors.push("latest immutable source-file evidence is invalid");
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
    "nativeGetSystemMetricsSmallIconPrimaryTraceSha256",
    "nativeGetSystemMetricsSmallIconConfirmationTraceSha256",
    "nativeLoadImageAPrimaryTraceSha256",
    "nativeLoadImageAConfirmationTraceSha256",
    "nativeSendMessageAPrimaryTraceSha256",
    "nativeSendMessageAConfirmationTraceSha256",
    "nativeSetWindowPosPrimaryTraceSha256",
    "nativeSetWindowPosConfirmationTraceSha256",
    "nativeSetWindowPosActivationPrimaryTraceSha256",
    "nativeSetWindowPosActivationConfirmationTraceSha256",
    "nativeMonitorFromWindowOracleSourceSha256",
    "nativeMonitorFromWindowOracleExecutableSha256",
    "nativeD3d9GetAdapterCountTraceSha256",
    "nativeD3d9GetAdapterCountSummarySha256",
    "nativeD3d9GetAdapterMonitorTraceSha256",
    "nativeD3d9GetAdapterMonitorSummarySha256",
    "nativeD3d9GetAdapterMonitorOracleSourceSha256",
    "nativeD3d9GetAdapterMonitorOracleExecutableSha256",
    "nativeLargeHeapPrimaryTraceSha256",
    "nativeLargeHeapConfirmationTraceSha256",
    "nativeCreateWindowExAPrimaryTraceSha256",
    "nativeCreateWindowExAConfirmationTraceSha256",
    "nativeCreateWindowExAPayloadTraceSha256",
    "nativeInvalidMenuTraceSha256",
    "nativeDefWindowProcTraceSha256",
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
    latestEvidence.timerResolutionProcessExitTeardown !== true ||
    latestEvidence.nativeTimeGetTimePrimaryTraceBytes !== 304_054 ||
    latestEvidence.nativeTimeGetTimePrimaryTraceLines !== 854 ||
    latestEvidence.nativeTimeGetTimePrimaryTraceSha256 !==
      "8560ea8389627cfc2c85b4a98f30f835ab53ff3ec13038ac55b25ad736790a29" ||
    latestEvidence.nativeTimeGetTimePrimaryBalancedApiPairs !== 390 ||
    latestEvidence.nativeTimeGetTimePrimarySummaryBytes !== 51_478 ||
    latestEvidence.nativeTimeGetTimePrimarySummarySha256 !==
      "5fa32c69a605e1cd173eed042522669f253534a1d993d943ee239b12d16fc777" ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceBytes !== 303_051 ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceLines !== 851 ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceSha256 !==
      "8f3a8847793282b4eadb4f4e45c6a142610b9f3ea1f3e4437e0b8349b314cc5e" ||
    latestEvidence.nativeTimeGetTimeConfirmationBalancedApiPairs !== 388 ||
    latestEvidence.nativeTimeGetTimeConfirmationSummaryBytes !== 51_652 ||
    latestEvidence.nativeTimeGetTimeConfirmationSummarySha256 !==
      "83d7a4ca65a6b109379b796838cc1d97718083a28657bc70d395d636ef9604c3" ||
    latestEvidence.nativeTimeGetTimeTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimeGetTimeProfile !== "x86-bgi-time-get-time-v1" ||
    latestEvidence.nativeTimeGetTimeOracleProfile !==
      "x86-time-get-time-shared-tick-epoch-v1" ||
    latestEvidence.nativeTimeGetTimeOracleSentinels?.join(",") !==
      "0,87,305419896" ||
    latestEvidence.nativeTimeGetTimeOracleCallsPerTrace !== 3 ||
    latestEvidence.nativeTimeGetTimeOracleResultsMatchHooks !== true ||
    latestEvidence.nativeTimeGetTimeOraclePreservesLastError !== true ||
    latestEvidence.nativeTimeGetTimeOracleResultsWithinGetTickCountBrackets !==
      true ||
    latestEvidence.nativeTimeGetTimeMainCallsPerTrace !== 1 ||
    latestEvidence.nativeTimeGetTimeCallerRva !== "0x00091ebf" ||
    latestEvidence.nativeTimeGetTimeLastError !== 0 ||
    latestEvidence.nativeTimeGetTimeConsumerProfile !==
      "wrap-aware-eax-delta-v1" ||
    latestEvidence.nativeTimeGetTimeConsumerFrameCallerRva !== "0x00083144" ||
    latestEvidence.nativeTimeGetTimeExecutableHashUnchanged !== true ||
    latestEvidence.timeGetTimeImplemented !== true ||
    latestEvidence.timeGetTimeReached !== false ||
    latestEvidence.timeGetTimeImplementationId !== "winmm.time-get-time.v1" ||
    latestEvidence.timeGetTimePolicyProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    latestEvidence.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    latestEvidence.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    latestEvidence.timeGetTimeIncrementPerRead !== 1 ||
    latestEvidence.timeGetTimeTimerResolutionAffectsClock !== false ||
    latestEvidence.timeGetTimeHostClockImported !== false ||
    latestEvidence.timeGetTimeBrowserClockImported !== false ||
    latestEvidence.timeGetTimeFailureAtomicity !== true ||
    latestEvidence.timeGetTimeNodeCompletions !== 0 ||
    latestEvidence.timeGetTimeChromeCompletions !== 0 ||
    latestEvidence.timeGetTimeNodeChromeReachabilityMatched !== true ||
    latestEvidence.tickCountNextValue !== 3_601_004
  ) {
    errors.push("latest WinMM timer and shared-clock evidence is invalid");
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
    latestEvidence.schema !== "vine.v86-bgi-d3d9-initial-render-states.v1" ||
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
    latestEvidence.nativeLoadCursorAPrimaryTraceBytes !== 336_703 ||
    latestEvidence.nativeLoadCursorAPrimaryTraceLines !== 812 ||
    latestEvidence.nativeLoadCursorAPrimaryBalancedApiPairs !== 370 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceBytes !== 338_341 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceLines !== 815 ||
    latestEvidence.nativeLoadCursorAConfirmationBalancedApiPairs !== 371 ||
    latestEvidence.nativeLoadCursorATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadCursorAOracleProfile !==
      "x86-system-arrow-and-module-cursor-shared-identity-v2" ||
    latestEvidence.nativeLoadCursorAOracleArrowResourceId !== 32_512 ||
    latestEvidence.nativeLoadCursorAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadCursorAOracleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadCursorAOracleSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceLastError !== 1_814 ||
    latestEvidence.nativeLoadCursorAOracleModuleResourceId !== 106 ||
    latestEvidence.nativeLoadCursorAOracleModuleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadCursorAOracleModuleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadCursorAOracleModuleSuccessLastError !== 0 ||
    latestEvidence.nativeLoadCursorAOracleModuleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadCursorAOracleModuleMissingResourceLastError !== 1_813 ||
    latestEvidence.nativeLoadCursorAOracleModuleWrongTypeResourceId !== 101 ||
    latestEvidence.nativeLoadCursorAOracleModuleWrongTypeLastError !== 1_813 ||
    latestEvidence.nativeLoadCursorASystemMetrics?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadCursorAModuleGroupResourceType !== 12 ||
    latestEvidence.nativeLoadCursorAModuleGroupResourceId !== 106 ||
    latestEvidence.nativeLoadCursorAModuleLanguageId !== 1_041 ||
    latestEvidence.nativeLoadCursorAModuleGroupDataRva !== "0x0016e9f4" ||
    latestEvidence.nativeLoadCursorAModuleGroupDataBytes !== 20 ||
    latestEvidence.nativeLoadCursorAModuleCursorResourceType !== 1 ||
    latestEvidence.nativeLoadCursorAModuleCursorResourceId !== 5 ||
    latestEvidence.nativeLoadCursorAModuleCursorDataRva !== "0x00154f88" ||
    latestEvidence.nativeLoadCursorAModuleCursorDataBytes !== 2_220 ||
    latestEvidence.nativeLoadCursorAModuleDimensions?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadCursorAModulePlanes !== 1 ||
    latestEvidence.nativeLoadCursorAModuleBitsPerPixel !== 8 ||
    latestEvidence.nativeLoadCursorAModuleHotspot?.join(",") !== "0,0" ||
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
    latestEvidence.loadCursorAModuleImplemented !== true ||
    latestEvidence.loadCursorAModuleSequence !== 17_815 ||
    latestEvidence.loadCursorAModuleOccurrence !== 4 ||
    latestEvidence.loadCursorAModuleInstructionCount !== 11_806_421 ||
    latestEvidence.loadCursorAModuleArguments?.join(",") !==
      "0x10000000,0x0000006a" ||
    latestEvidence.loadCursorAModuleReturnAddress !== "0x10067fe1" ||
    latestEvidence.loadCursorAModuleResult !== "0x43010002" ||
    latestEvidence.loadCursorAModuleLastError !== 0 ||
    latestEvidence.loadCursorAModuleSuccessLastErrorPolicy !== "zero" ||
    latestEvidence.loadCursorAModuleGuestWrites !== 0 ||
    latestEvidence.loadCursorAModuleCursorId !== 2 ||
    latestEvidence.loadCursorAModuleHandleNamespace !== "vine.logical-cursor.v1" ||
    latestEvidence.loadCursorAModuleShared !== true ||
    latestEvidence.loadCursorAModuleSourceKind !== "module-resource" ||
    latestEvidence.loadCursorAModuleId !== 15 ||
    latestEvidence.loadCursorAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadCursorAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadCursorAModuleGroupResourceId !== 106 ||
    latestEvidence.loadCursorAModuleLanguageId !== 1_041 ||
    latestEvidence.loadCursorAModuleCursorResourceId !== 5 ||
    latestEvidence.loadCursorAModuleGroupDataRva !== "0x0016e9f4" ||
    latestEvidence.loadCursorAModuleGroupDataBytes !== 20 ||
    latestEvidence.loadCursorAModuleCursorDataRva !== "0x00154f88" ||
    latestEvidence.loadCursorAModuleCursorDataBytes !== 2_220 ||
    latestEvidence.loadCursorAModuleWidth !== 32 ||
    latestEvidence.loadCursorAModuleHeight !== 32 ||
    latestEvidence.loadCursorAModulePlanes !== 1 ||
    latestEvidence.loadCursorAModuleBitsPerPixel !== 8 ||
    latestEvidence.loadCursorAModuleHotspotX !== 0 ||
    latestEvidence.loadCursorAModuleHotspotY !== 0 ||
    latestEvidence.loadCursorAModuleDistinctFromSystemIdentity !== true ||
    latestEvidence.loadCursorAModuleRepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadCursorAModuleFailureAtomicity !== true ||
    latestEvidence.loadCursorAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadCursorAHostCursorImported !== false ||
    latestEvidence.loadCursorABrowserCursorImported !== false
  ) {
    errors.push("latest system and module cursor identity evidence is invalid");
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
    latestEvidence.nativeGetSystemMetricsSmallIconPrimaryTraceBytes !== 372_414 ||
    latestEvidence.nativeGetSystemMetricsSmallIconPrimaryTraceLines !== 865 ||
    latestEvidence.nativeGetSystemMetricsSmallIconConfirmationTraceBytes !== 372_020 ||
    latestEvidence.nativeGetSystemMetricsSmallIconConfirmationTraceLines !== 864 ||
    latestEvidence.nativeGetSystemMetricsSmallIconProfile !==
      "x86-bgi-small-icon-metrics-v1" ||
    latestEvidence.nativeGetSystemMetricsSmallIconIndices?.join(",") !== "50,49" ||
    latestEvidence.nativeGetSystemMetricsSmallIconValues?.join(",") !== "16,16" ||
    latestEvidence.nativeGetSystemMetricsSmallIconCallerReturnRvas?.join(",") !==
      "0x00067f2d,0x00067f32" ||
    latestEvidence.nativeGetSystemMetricsSmallIconLastErrors?.join(",") !==
      "1401,1401" ||
    latestEvidence.nativeGetSystemMetricsSmallIconConsumer !==
      "loadimagea-resource-101-image-icon-width-height-lr-shared" ||
    latestEvidence.nativeGetSystemMetricsSmallIconExecutableHashUnchanged !== true ||
    latestEvidence.getSystemMetricsImplemented !== true ||
    latestEvidence.getSystemMetricsCallCount !== 11 ||
    latestEvidence.getSystemMetricsSequences?.join(",") !==
      "1345,1346,1347,1348,1349,1395,1396,1400,1401,1405,1406" ||
    latestEvidence.getSystemMetricsInstructionCounts?.join(",") !==
      "49990,49995,50000,50020,50024,11476518,11476522,11476595,11476602,11476761,11476765" ||
    latestEvidence.getSystemMetricsArguments?.join(",") !==
      "7,8,4,0,1,50,49,1,0,50,49" ||
    latestEvidence.getSystemMetricsReturnAddresses?.join(",") !==
      "0x10067ddb,0x10067de6,0x10067dee,0x10067e26,0x10067e2e,0x10067f2d,0x10067f32,0x10067f65,0x10067f70,0x100929d2,0x100929d7" ||
    latestEvidence.getSystemMetricsResults?.join(",") !==
      "3,3,23,1920,1080,16,16,1080,1920,16,16" ||
    latestEvidence.getSystemMetricsLastErrors?.join(",") !==
      "0,0,0,0,0,0,0,0,0,0,0" ||
    latestEvidence.getSystemMetricsSuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.getSystemMetricsGuestWrites !== 0 ||
    latestEvidence.getSystemMetricsDisplayStateOwner !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.getSystemMetricsNonclientPolicyOwner !==
      "User32CompatibilityPolicy.get_system_metrics" ||
    latestEvidence.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-small-icon-16x16-v3" ||
    latestEvidence.getSystemMetricsSupportedIndices?.join(",") !==
      "0,1,4,7,8,49,50" ||
    latestEvidence.getSystemMetricsSmallIconWidth !== 16 ||
    latestEvidence.getSystemMetricsSmallIconWidthConsumer !==
      "paired-loadimagea-small-icon-width" ||
    latestEvidence.getSystemMetricsSmallIconHeight !== 16 ||
    latestEvidence.getSystemMetricsSmallIconHeightConsumer !==
      "paired-loadimagea-small-icon-height" ||
    latestEvidence.getSystemMetricsHostValuesImported !== false ||
    latestEvidence.getSystemMetricsBrowserValuesImported !== false
  ) {
    errors.push("latest controlled system-metrics evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadImageAPrimaryTraceBytes !== 295_589 ||
    latestEvidence.nativeLoadImageAPrimaryTraceLines !== 803 ||
    latestEvidence.nativeLoadImageAPrimaryBalancedApiPairs !== 365 ||
    latestEvidence.nativeLoadImageAConfirmationTraceBytes !== 295_572 ||
    latestEvidence.nativeLoadImageAConfirmationTraceLines !== 803 ||
    latestEvidence.nativeLoadImageAConfirmationBalancedApiPairs !== 365 ||
    latestEvidence.nativeLoadImageATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadImageABgiProfile !==
      "x86-bgi-load-image-a-small-icon-v1" ||
    latestEvidence.nativeLoadImageABgiCallCount !== 2 ||
    latestEvidence.nativeLoadImageABgiCallerReturnRvas?.join(",") !==
      "0x00067f43,0x000929e9" ||
    latestEvidence.nativeLoadImageABgiArguments?.join(",") !==
      "main-module,101,1,16,16,0x00008000" ||
    latestEvidence.nativeLoadImageABgiResultsNonNull !== true ||
    latestEvidence.nativeLoadImageABgiIdentityShared !== true ||
    latestEvidence.nativeLoadImageABgiEntryLastErrors?.join(",") !== "1401,0" ||
    latestEvidence.nativeLoadImageABgiExitLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLoadImageABgiConsumer !==
      "sendmessagea-wm-seticon-icon-small-then-repeat-null-check" ||
    latestEvidence.nativeLoadImageAOracleProfile !==
      "x86-module-small-icon-load-image-shared-identity-v1" ||
    latestEvidence.nativeLoadImageAOracleSuccessLastError !== 0 ||
    latestEvidence.nativeLoadImageAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadImageAOracleMissingResourceLastError !== 1_813 ||
    latestEvidence.nativeLoadImageASelectedWidth !== 16 ||
    latestEvidence.nativeLoadImageASelectedHeight !== 16 ||
    latestEvidence.nativeLoadImageASelectedPlanes !== 1 ||
    latestEvidence.nativeLoadImageASelectedBitsPerPixel !== 32 ||
    latestEvidence.nativeLoadImageAExecutableHashUnchanged !== true ||
    latestEvidence.loadImageAImplemented !== true ||
    latestEvidence.loadImageACallCount !== 2 ||
    latestEvidence.loadImageASequences?.join(",") !== "1397,1407" ||
    latestEvidence.loadImageASequence !== 1_397 ||
    latestEvidence.loadImageAInstructionCount !== 11_476_529 ||
    latestEvidence.loadImageAArguments?.join(",") !==
      "0x10000000,0x00000065,0x00000001,0x00000010,0x00000010,0x00008000" ||
    latestEvidence.loadImageAReturnAddress !== "0x10067f43" ||
    latestEvidence.loadImageAResult !== "0x49010002" ||
    latestEvidence.loadImageALastError !== 0 ||
    latestEvidence.loadImageAGuestWrites !== 0 ||
    latestEvidence.loadImageAStateOwner !== "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadImageAIconId !== 2 ||
    latestEvidence.loadImageAHandleNamespace !== "vine.logical-icon.v1" ||
    latestEvidence.loadImageAShared !== true ||
    latestEvidence.loadImageAModuleId !== 15 ||
    latestEvidence.loadImageAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadImageAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadImageAGroupResourceId !== 101 ||
    latestEvidence.loadImageALanguageId !== 1_041 ||
    latestEvidence.loadImageAIconResourceId !== 4 ||
    latestEvidence.loadImageAGroupDataRva !== "0x0016ea08" ||
    latestEvidence.loadImageAGroupDataBytes !== 62 ||
    latestEvidence.loadImageAIconDataRva !== "0x0016d004" ||
    latestEvidence.loadImageAIconDataBytes !== 1_128 ||
    latestEvidence.loadImageAWidth !== 16 ||
    latestEvidence.loadImageAHeight !== 16 ||
    latestEvidence.loadImageAPlanes !== 1 ||
    latestEvidence.loadImageABitsPerPixel !== 32 ||
    latestEvidence.loadImageALargeIconUsesDistinctIdentity !== true ||
    latestEvidence.loadImageARepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadImageAMissingResourceLastError !== 1_813 ||
    latestEvidence.loadImageAUnsupportedVariantsFailBeforeMutation !== true ||
    latestEvidence.loadImageAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadImageAHostIconImported !== false ||
    latestEvidence.loadImageABrowserIconImported !== false ||
    latestEvidence.writebackBatches !== 17_920 ||
    latestEvidence.writebackDirtyPages !== 36_075 ||
    latestEvidence.writebackChangedBytes !== 1_807_844 ||
    latestEvidence.hostDeltaBatches !== 16_573 ||
    latestEvidence.finalMemoryGeneration !== 34_924
  ) {
    errors.push("latest LoadImageA small-icon evidence is invalid");
  }
  if (
    latestEvidence.nativeSendMessageAPrimaryTraceBytes !== 320_348 ||
    latestEvidence.nativeSendMessageAPrimaryTraceLines !== 808 ||
    latestEvidence.nativeSendMessageAPrimaryBalancedApiPairs !== 367 ||
    latestEvidence.nativeSendMessageAConfirmationTraceBytes !== 320_408 ||
    latestEvidence.nativeSendMessageAConfirmationTraceLines !== 806 ||
    latestEvidence.nativeSendMessageAConfirmationBalancedApiPairs !== 366 ||
    latestEvidence.nativeSendMessageATraceDiagnostics !== 0 ||
    latestEvidence.nativeSendMessageAProfile !==
      "x86-bgi-send-message-a-wm-seticon-v1" ||
    latestEvidence.nativeSendMessageACallerReturnRva !== "0x00067f58" ||
    latestEvidence.nativeSendMessageAMessage !== 128 ||
    latestEvidence.nativeSendMessageAWParam !== 0 ||
    latestEvidence.nativeSendMessageAResult !== 0 ||
    latestEvidence.nativeSendMessageAEntryLastError !== 0 ||
    latestEvidence.nativeSendMessageAExitLastError !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureProfile !==
      "bgi-main-window-procedure-rva-00092550-v1" ||
    latestEvidence.nativeSendMessageAWindowProcedureRva !== "0x00092550" ||
    latestEvidence.nativeSendMessageAWindowProcedureParentRelation !==
      "direct-child-same-thread" ||
    latestEvidence.nativeSendMessageAWindowProcedureArgumentsMatch !== true ||
    latestEvidence.nativeSendMessageAWindowProcedureResult !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureEntryLastError !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureExitLastError !== 0 ||
    latestEvidence.nativeSendMessageAConsumer !==
      "lresult-ignored-before-window-position-argument-setup" ||
    latestEvidence.nativeSendMessageASecondLoadImageAfterCallback !== true ||
    latestEvidence.nativeSendMessageAExecutableHashUnchanged !== true ||
    latestEvidence.sendMessageAImplemented !== true ||
    latestEvidence.sendMessageASequence !== 1_398 ||
    latestEvidence.sendMessageAInstructionCount !== 11_476_536 ||
    latestEvidence.sendMessageAArguments?.join(",") !==
      "0x48010001,0x00000080,0x00000000,0x49010002" ||
    latestEvidence.sendMessageAReturnAddress !== "0x10067f58" ||
    latestEvidence.sendMessageAResult !== "0x00000000" ||
    latestEvidence.sendMessageALastError !== 0 ||
    latestEvidence.sendMessageAGuestWrites !== 0 ||
    latestEvidence.sendMessageAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.sendMessageAWindowId !== 1 ||
    latestEvidence.sendMessageAWindowHandle !== "0x48010001" ||
    latestEvidence.sendMessageAOwnerThreadId !== 8_192 ||
    latestEvidence.sendMessageAClassId !== 1 ||
    latestEvidence.sendMessageAIconId !== 2 ||
    latestEvidence.sendMessageAIconHandle !== "0x49010002" ||
    latestEvidence.sendMessageACallbackFrameId !== 7 ||
    latestEvidence.sendMessageACallbackTarget !== "0x10092550" ||
    latestEvidence.sendMessageACallbackArguments?.join(",") !==
      "0x48010001,0x00000080,0x00000000,0x49010002" ||
    latestEvidence.sendMessageACallbackAllocationStart !== "0x006ffac8" ||
    latestEvidence.sendMessageACallbackStackPointer !== "0x006ffac8" ||
    latestEvidence.sendMessageACallbackScratchAddress !== "0x00000000" ||
    latestEvidence.sendMessageACallbackScratchBytes !== 0 ||
    latestEvidence.sendMessageACallbackFrameBytes !== 20 ||
    latestEvidence.sendMessageACallbackReturnSentinel !== "0x7f010000" ||
    latestEvidence.sendMessageACallbackReturnInstructionCount !== 11_476_589 ||
    latestEvidence.sendMessageACallbackResult !== "0x00000000" ||
    latestEvidence.sendMessageACallbackDepthAfterReturn !== 0 ||
    latestEvidence.sendMessageANestedDefWindowProcSequence !== 1_399 ||
    latestEvidence.sendMessageANestedDefWindowProcInstructionCount !== 11_476_576 ||
    latestEvidence.sendMessageANestedDefWindowProcResult !== "0x00000000" ||
    latestEvidence.sendMessageAFailureAtomicity !== true ||
    latestEvidence.sendMessageAWindowIconMutationImplemented !== true ||
    latestEvidence.sendMessageAMessageQueueIntegrated !== false ||
    latestEvidence.sendMessageAHostDispatchImported !== false ||
    latestEvidence.sendMessageABrowserDispatchImported !== false
  ) {
    errors.push("latest synchronous small-icon message evidence is invalid");
  }
  const setWindowPosShowGeometry = requireRecord(
    latestEvidence.setWindowPosShowGeometry,
    "activeRuntimeCheckpoint.latestEvidence.setWindowPosShowGeometry",
    errors,
  );
  if (
    latestEvidence.nativeSetWindowPosPrimaryTraceBytes !== 95_895 ||
    latestEvidence.nativeSetWindowPosPrimaryTraceLines !== 132 ||
    latestEvidence.nativeSetWindowPosPrimaryBalancedApiPairs !== 39 ||
    latestEvidence.nativeSetWindowPosConfirmationTraceBytes !== 95_912 ||
    latestEvidence.nativeSetWindowPosConfirmationTraceLines !== 132 ||
    latestEvidence.nativeSetWindowPosConfirmationBalancedApiPairs !== 39 ||
    latestEvidence.nativeSetWindowPosTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowPosProfile !==
      "x86-bgi-set-window-pos-initial-placement-v1" ||
    latestEvidence.nativeSetWindowPosCallerRvas?.join(",") !==
      "0x00067f88,0x00067f9f" ||
    latestEvidence.nativeSetWindowPosResults?.join(",") !== "1,1" ||
    latestEvidence.nativeSetWindowPosEntryLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeSetWindowPosExitLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeSetWindowPosWindowPosFlags?.join(",") !==
      "0x10000d59,0x00001d9f" ||
    latestEvidence.nativeSetWindowPosShowWindowPos?.join(",") !==
      "15360,8640,806,629" ||
    latestEvidence.nativeSetWindowPosMoveLparam !== "0x21da3c03" ||
    latestEvidence.nativeSetWindowPosCallbackMessages?.join(",") !==
      "71,3,127,71" ||
    latestEvidence.nativeSetWindowPosGetIconWParam !== 2 ||
    latestEvidence.nativeSetWindowPosGetIconRelation !==
      "same-shared-16x16-loadimagea-hicon" ||
    latestEvidence.nativeSetWindowPosConsumer !==
      "bool-results-ignored-and-second-call-returns" ||
    latestEvidence.nativeSetWindowPosExecutableHashUnchanged !== true ||
    latestEvidence.nativeSetWindowPosActivationPrimaryTraceBytes !== 83_322 ||
    latestEvidence.nativeSetWindowPosActivationPrimaryTraceLines !== 97 ||
    latestEvidence.nativeSetWindowPosActivationPrimaryBalancedApiPairs !== 17 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationTraceBytes !== 83_309 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationTraceLines !== 97 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationBalancedApiPairs !== 17 ||
    latestEvidence.nativeSetWindowPosActivationTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowPosActivationProfile !==
      "x86-bgi-first-set-window-pos-size-v1" ||
    latestEvidence.nativeSetWindowPosActivationCallsPerTrace !== 1 ||
    latestEvidence.nativeSetWindowPosActivationCallerRva !== "0x0005cb4b" ||
    latestEvidence.nativeSetWindowPosActivationResult !== 1 ||
    latestEvidence.nativeSetWindowPosActivationEntryLastError !== 0 ||
    latestEvidence.nativeSetWindowPosActivationExitLastError !== 0 ||
    latestEvidence.nativeSetWindowPosActivationInvalidWindowResult !== 0 ||
    latestEvidence.nativeSetWindowPosActivationInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeSetWindowPosActivationCallbackMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    latestEvidence.nativeSetWindowPosActivationCallbackResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    latestEvidence.nativeSetWindowPosActivationInitialRelations?.join(",") !==
      "null,null,external" ||
    latestEvidence.nativeSetWindowPosActivationSecondWindowPosRelations?.join(",") !==
      "target,null,target" ||
    latestEvidence.nativeSetWindowPosActivationImeAndFocusRelations?.join(",") !==
      "target,target,target" ||
    latestEvidence.nativeSetWindowPosActivationGeometryStyleVisibilityUnchanged !== true ||
    latestEvidence.nativeSetWindowPosActivationWindowPosChangedSizeMoveAbsent !== true ||
    latestEvidence.nativeSetWindowPosActivationConsumer !==
      "bool-overwritten-by-immediate-helper-call" ||
    latestEvidence.nativeSetWindowPosActivationExecutableHashUnchanged !== true ||
    latestEvidence.setWindowPosImplemented !== true ||
    latestEvidence.setWindowPosCallCount !== 3 ||
    latestEvidence.setWindowPosSequences?.join(",") !== "1402,1408,17831" ||
    latestEvidence.setWindowPosInstructionCounts?.join(",") !==
      "11476612,11476797,11806645" ||
    latestEvidence.setWindowPosReturnAddresses?.join(",") !==
      "0x10067f88,0x10067f9f,0x1005cb4b" ||
    latestEvidence.setWindowPosResults?.join(",") !== "1,1,1" ||
    latestEvidence.setWindowPosLastErrors?.join(",") !== "0,0,0" ||
    latestEvidence.setWindowPosGuestWrites !== 0 ||
    latestEvidence.setWindowPosImplementationId !== "user32.set-window-pos.v2" ||
    latestEvidence.setWindowPosStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.setWindowPosWindowId !== 1 ||
    latestEvidence.setWindowPosWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosOwnerThreadId !== 8_192 ||
    latestEvidence.setWindowPosSmallIconId !== 2 ||
    latestEvidence.setWindowPosSmallIconHandle !== "0x49010002" ||
    setWindowPosShowGeometry.x !== 15_360 ||
    setWindowPosShowGeometry.y !== 8_640 ||
    setWindowPosShowGeometry.width !== 806 ||
    setWindowPosShowGeometry.height !== 629 ||
    setWindowPosShowGeometry.clientX !== 15_363 ||
    setWindowPosShowGeometry.clientY !== 8_666 ||
    setWindowPosShowGeometry.clientWidth !== 800 ||
    setWindowPosShowGeometry.clientHeight !== 600 ||
    latestEvidence.setWindowPosFinalVisible !== false ||
    latestEvidence.setWindowPosFinalTopmost !== false ||
    latestEvidence.setWindowPosCallbackFrameIds?.join(",") !== "8,9,10,11" ||
    latestEvidence.setWindowPosCallbackMessages?.join(",") !== "71,3,127,71" ||
    latestEvidence.setWindowPosCallbackResults?.join(",") !==
      "0,0,0x49010002,0" ||
    latestEvidence.setWindowPosCallbackReturnInstructionCounts?.join(",") !==
      "11476718,11476705,11476787,11476850" ||
    latestEvidence.setWindowPosMaximumCallbackDepth !== 2 ||
    latestEvidence.setWindowPosSameDepthReplacement !== true ||
    latestEvidence.setWindowPosActivationCallbackFrameIds?.join(",") !==
      "12,13,14,15,16,17,18,19" ||
    latestEvidence.setWindowPosActivationCallbackMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    latestEvidence.setWindowPosActivationCallbackResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    latestEvidence.setWindowPosActivationCallbackReturnInstructionCounts?.join(",") !==
      "11806700,11806755,11806808,11806864,11807100,11807034,11807021,11807087" ||
    latestEvidence.setWindowPosActivationMaximumCallbackDepth !== 3 ||
    latestEvidence.setWindowPosActivationReplacementCount !== 5 ||
    latestEvidence.setWindowPosActivationGeometryStyleVisibilityUnchanged !== true ||
    latestEvidence.setWindowPosActivationActiveWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosActivationFocusWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosActivationForegroundWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosActivationHostZOrderIdentityImported !== false ||
    latestEvidence.setWindowPosActivationExternalThreadIdentityImported !== false ||
    latestEvidence.setWindowPosGeneralResizeImplemented !== false ||
    latestEvidence.setWindowPosGeneralZOrderImplemented !== false ||
    latestEvidence.setWindowPosMultiWindowActivationImplemented !== false ||
    latestEvidence.setWindowPosFailureAtomicity !== true ||
    latestEvidence.setWindowPosBrowserOwnsLogicalWindowState !== false ||
    latestEvidence.nativeCriticalSectionCapacityTraceBytes !== 66_177_215 ||
    latestEvidence.nativeCriticalSectionCapacityTraceLines !== 116_186 ||
    latestEvidence.nativeCriticalSectionCapacityTraceSha256 !==
      "119b818f617206c9abcb63c2777279aaf8596f1b522b094c99b5f65960358b45" ||
    latestEvidence.nativeCriticalSectionCapacityProfile !==
      "bgi-main-module-live-critical-section-capacity-v1" ||
    latestEvidence.nativeCriticalSectionCapacityDurationMilliseconds !== 10_000 ||
    latestEvidence.nativeCriticalSectionCapacitySpinCountCalls !== 17 ||
    latestEvidence.nativeCriticalSectionCapacityPlainCalls !== 16_517 ||
    latestEvidence.nativeCriticalSectionCapacityDeleteCalls !== 0 ||
    latestEvidence.nativeCriticalSectionCapacityEnterCalls !== 4_156 ||
    latestEvidence.nativeCriticalSectionCapacityLeaveCalls !== 4_066 ||
    latestEvidence.nativeCriticalSectionCapacityMaximumLive !== 16_534 ||
    latestEvidence.nativeCriticalSectionCapacityFinalLive !== 16_534 ||
    latestEvidence.nativeCriticalSectionCapacityLastInitializeAfterModuleLoadMilliseconds !==
      8_966 ||
    latestEvidence.nativeCriticalSectionCapacityRepeatMaximumLive?.join(",") !==
      "5325,5147" ||
    latestEvidence.nativeCriticalSectionCapacityExecutableHashUnchanged !== true ||
    latestEvidence.criticalSectionCapacityLimit !== 32_768 ||
    latestEvidence.criticalSectionNodeLiveCount !== 16_408 ||
    latestEvidence.criticalSectionNodeSpinCountCalls !== 17 ||
    latestEvidence.criticalSectionNodePlainCalls !== 16_391 ||
    latestEvidence.criticalSectionNodeRemainingCapacity !== 16_360 ||
    latestEvidence.criticalSectionCapacityBounded !== true ||
    latestEvidence.criticalSectionCapacityFailureAtomicity !== true
  ) {
    errors.push("latest SetWindowPos and critical-section capacity evidence is invalid");
  }
  const nativeMonitorFromWindowInvalidWindowFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowInvalidWindowFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowInvalidWindowFlags",
    errors,
  );
  const nativeMonitorFromWindowValidTargets = requireStringArray(
    latestEvidence.nativeMonitorFromWindowValidTargets,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowValidTargets",
    errors,
  );
  const nativeMonitorFromWindowValidFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowValidFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowValidFlags",
    errors,
  );
  const nativeMonitorFromWindowNullFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowNullFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowNullFlags",
    errors,
  );
  if (
    latestEvidence.nativeMonitorFromWindowOracleArchitecture !== "coff-i386" ||
    latestEvidence.nativeMonitorFromWindowOracleSourceBytes !== 1_920 ||
    latestEvidence.nativeMonitorFromWindowOracleSourceSha256 !==
      "9443dcc8744ff0a39ee22485f7ad3f15a3413a1c51306361f117605bb82d271a" ||
    latestEvidence.nativeMonitorFromWindowOracleExecutableBytes !== 78_848 ||
    latestEvidence.nativeMonitorFromWindowOracleExecutableSha256 !==
      "c499b514839719a8536b663b02f09d98ed098c66cc27b6a8bd8979f53e1cdda9" ||
    latestEvidence.nativeMonitorFromWindowSentinelLastError !== 23_130 ||
    nativeMonitorFromWindowInvalidWindowFlags.join(",") !== "0,1,2" ||
    latestEvidence.nativeMonitorFromWindowInvalidWindowResult !== "0x00000000" ||
    latestEvidence.nativeMonitorFromWindowInvalidWindowLastError !== 1_400 ||
    nativeMonitorFromWindowValidTargets.join(",") !==
      "hidden-window,desktop-window" ||
    nativeMonitorFromWindowValidFlags.join(",") !== "0,1,2" ||
    latestEvidence.nativeMonitorFromWindowValidResultsNonNull !== true ||
    latestEvidence.nativeMonitorFromWindowValidIdentityShared !== true ||
    latestEvidence.nativeMonitorFromWindowValidLastErrorPreserved !== true ||
    latestEvidence.nativeMonitorFromWindowInvalidFlags !== 3 ||
    latestEvidence.nativeMonitorFromWindowInvalidFlagsResult !== "0x00000000" ||
    latestEvidence.nativeMonitorFromWindowInvalidFlagsLastError !== 1_004 ||
    nativeMonitorFromWindowNullFlags.join(",") !== "0,1,2" ||
    JSON.stringify(latestEvidence.nativeMonitorFromWindowNullResultsNonNull) !==
      JSON.stringify([false, true, true]) ||
    latestEvidence.nativeMonitorFromWindowNullLastErrorPreserved !== true ||
    latestEvidence.nativeMonitorFromWindowNullOutsideReachedContract !== true ||
    latestEvidence.nativeMonitorFromWindowCallerReturnRva !== "0x00059d19" ||
    latestEvidence.nativeMonitorFromWindowReachedFlags !== 1 ||
    latestEvidence.nativeMonitorFromWindowConsumer !==
      "return-hmonitor-store-ebx-compare-with-idirect3d9-get-adapter-monitor" ||
    latestEvidence.nativeMonitorFromWindowExecutableHashUnchanged !== true ||
    latestEvidence.monitorFromWindowImplemented !== true ||
    latestEvidence.monitorFromWindowSequence !== 17_842 ||
    latestEvidence.monitorFromWindowInstructionCount !== 11_807_189 ||
    latestEvidence.monitorFromWindowArguments?.join(",") !==
      "0x48010001,0x00000001" ||
    latestEvidence.monitorFromWindowReturnAddress !== "0x10059d19" ||
    latestEvidence.monitorFromWindowResult !== "0x4f010001" ||
    latestEvidence.monitorFromWindowLastError !== 0 ||
    latestEvidence.monitorFromWindowGuestWrites !== 0 ||
    latestEvidence.monitorFromWindowImplementationId !==
      "user32.monitor-from-window.v1" ||
    latestEvidence.monitorFromWindowPolicyId !==
      "live-window-default-to-primary-v1" ||
    latestEvidence.monitorFromWindowStateAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.monitorFromWindowWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.monitorFromWindowWindowHandle !== "0x48010001" ||
    latestEvidence.monitorFromWindowMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    latestEvidence.monitorFromWindowPrimaryMonitorHandle !== "0x4f010001" ||
    latestEvidence.monitorFromWindowDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.monitorFromWindowFlags !== 1 ||
    latestEvidence.monitorFromWindowInvalidWindowResult !== "0x00000000" ||
    latestEvidence.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    latestEvidence.monitorFromWindowUnsupportedFlagsFault !==
      "monitor-from-window-flags-unsupported" ||
    latestEvidence.monitorFromWindowFailureAtomicity !== true ||
    latestEvidence.monitorFromWindowHostDisplayTopologyImported !== false ||
    latestEvidence.monitorFromWindowBrowserDisplayTopologyImported !== false
  ) {
    errors.push("latest MonitorFromWindow evidence is invalid");
  }
  if (
    latestEvidence.nativeD3d9GetAdapterCountTraceBytes !== 2_267_972 ||
    latestEvidence.nativeD3d9GetAdapterCountTraceLines !== 7_133 ||
    latestEvidence.nativeD3d9GetAdapterCountTraceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    latestEvidence.nativeD3d9GetAdapterCountSummaryBytes !== 58_021 ||
    latestEvidence.nativeD3d9GetAdapterCountSummarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    latestEvidence.nativeD3d9GetAdapterCountTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetAdapterCountCallsPerTrace !== 6 ||
    latestEvidence.nativeD3d9GetAdapterCountCallerRva !== "0x00059d36" ||
    latestEvidence.nativeD3d9GetAdapterCountResult !== 1 ||
    latestEvidence.nativeD3d9GetAdapterCountLastError !== 0 ||
    latestEvidence.nativeD3d9GetAdapterCountConsumer !==
      "store-count-in-edi-loop-adapter-indices-and-compare-monitor-identity" ||
    latestEvidence.nativeD3d9GetAdapterCountExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterCountImplemented !== true ||
    latestEvidence.d3d9GetAdapterCountSequence !== 17_843 ||
    latestEvidence.d3d9GetAdapterCountInstructionCount !== 11_807_197 ||
    latestEvidence.d3d9GetAdapterCountArguments?.join(",") !== "0x01000004" ||
    latestEvidence.d3d9GetAdapterCountReturnAddress !== "0x10059d36" ||
    latestEvidence.d3d9GetAdapterCountResult !== 1 ||
    latestEvidence.d3d9GetAdapterCountLastError !== 0 ||
    latestEvidence.d3d9GetAdapterCountGuestWriteBytes !== 0 ||
    latestEvidence.d3d9GetAdapterCountPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterCountPolicyVersion !== 14 ||
    latestEvidence.d3d9GetAdapterCountPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetAdapterCountProfileId !==
      "single-logical-adapter-v1" ||
    latestEvidence.d3d9GetAdapterCountDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9GetAdapterCountCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterCountDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterCountReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterCountReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterCountFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterCountBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterCountRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterCountRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterCountHostGpuTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterCountBrowserGpuTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterCountNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterCountChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterCountNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 adapter-count evidence is invalid");
  }
  const nativeD3d9GetAdapterMonitorOracleAdapters = requireIntegerArray(
    latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapters,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapters",
    errors,
  );
  const d3d9GetAdapterMonitorSequences = requireIntegerArray(
    latestEvidence.d3d9GetAdapterMonitorSequences,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterMonitorSequences",
    errors,
  );
  const d3d9GetAdapterMonitorInstructionCounts = requireIntegerArray(
    latestEvidence.d3d9GetAdapterMonitorInstructionCounts,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterMonitorInstructionCounts",
    errors,
  );
  if (
    latestEvidence.nativeD3d9GetAdapterMonitorTraceBytes !== 2_267_972 ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceLines !== 7_133 ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    latestEvidence.nativeD3d9GetAdapterMonitorSummaryBytes !== 58_021 ||
    latestEvidence.nativeD3d9GetAdapterMonitorSummarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorCallsPerTrace !== 6 ||
    latestEvidence.nativeD3d9GetAdapterMonitorCallerRva !== "0x00059d5e" ||
    latestEvidence.nativeD3d9GetAdapterMonitorAdapter !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorResultOpaqueNonNull !== true ||
    latestEvidence.nativeD3d9GetAdapterMonitorLastError !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleArchitecture !== "coff-i386" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleSourceBytes !== 5_089 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleSourceSha256 !==
      "337ddf83ec68ba80a25d70f1f9a9d0361a83833657ca7c852075043d16b57c67" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleExecutableBytes !== 3_584 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleExecutableSha256 !==
      "1d2681032fad258d9930c1a4a88ac6d4cf67d53b7d26ff447a17d5129bb436d5" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapterCount !== 1 ||
    nativeD3d9GetAdapterMonitorOracleAdapters.join(",") !== "0,1,4294967295" ||
    JSON.stringify(latestEvidence.nativeD3d9GetAdapterMonitorOracleResultsNonNull) !==
      JSON.stringify([true, false, false]) ||
    latestEvidence.nativeD3d9GetAdapterMonitorOraclePrimaryIdentityStable !== true ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleLastErrorSentinel !== 23_130 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleLastErrorPreserved !== true ||
    latestEvidence.nativeD3d9GetAdapterMonitorConsumer !==
      "compare-result-with-monitor-from-window-primary-identity" ||
    latestEvidence.nativeD3d9GetAdapterMonitorExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterMonitorImplemented !== true ||
    d3d9GetAdapterMonitorSequences.join(",") !== "17844,17848" ||
    d3d9GetAdapterMonitorInstructionCounts.join(",") !== "11807216,11807374" ||
    JSON.stringify(latestEvidence.d3d9GetAdapterMonitorArguments) !==
      JSON.stringify([["0x01000004", "0x00000000"], ["0x01000004", "0x00000000"]]) ||
    latestEvidence.d3d9GetAdapterMonitorReturnAddresses?.join(",") !==
      "0x10059d5e,0x10059d5e" ||
    latestEvidence.d3d9GetAdapterMonitorResults?.join(",") !==
      "0x4f010001,0x4f010001" ||
    latestEvidence.d3d9GetAdapterMonitorLastError !== 0 ||
    latestEvidence.d3d9GetAdapterMonitorGuestWriteBytes !== 0 ||
    latestEvidence.d3d9GetAdapterMonitorPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterMonitorPolicyVersion !== 14 ||
    latestEvidence.d3d9GetAdapterMonitorPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetAdapterMonitorProfileId !==
      "primary-adapter-monitor-v1" ||
    latestEvidence.d3d9GetAdapterMonitorDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9GetAdapterMonitorAdapterMappingAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterMonitorIdentityAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    latestEvidence.d3d9GetAdapterMonitorPrimaryHandle !== "0x4f010001" ||
    latestEvidence.d3d9GetAdapterMonitorMatchesMonitorFromWindowIdentity !== true ||
    latestEvidence.d3d9GetAdapterMonitorReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterMonitorReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterMonitorFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterMonitorBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterMonitorRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterMonitorRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterMonitorHostIdentityImported !== false ||
    latestEvidence.d3d9GetAdapterMonitorBrowserTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterMonitorNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterMonitorChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterMonitorNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 adapter-monitor evidence is invalid");
  }
  if (
    latestEvidence.d3d9RootFactoryImplemented !== true ||
    latestEvidence.d3d9RootFactorySequence !== 17_816 ||
    latestEvidence.d3d9RootFactoryInstructionCount !== 11_806_445 ||
    latestEvidence.d3d9RootFactoryArguments?.join(",") !== "0x00000020" ||
    latestEvidence.d3d9RootFactoryReturnAddress !== "0x1005b892" ||
    latestEvidence.d3d9RootFactoryResult !== "0x01000004" ||
    latestEvidence.d3d9RootFactoryLastError !== 0 ||
    latestEvidence.d3d9RootFactoryGuestWrites !== 0 ||
    latestEvidence.d3d9RootFactoryImplementationId !==
      "d3d9.direct3d-create9.v1" ||
    latestEvidence.d3d9RootObjectId !== "1" ||
    latestEvidence.d3d9RootClassIdentity !== "d3d9.idirect3d9-root.v1" ||
    latestEvidence.d3d9RootReferenceCount !== 1 ||
    latestEvidence.d3d9RootCanonicalIunknownInterfaceId !== "1" ||
    latestEvidence.d3d9RootInterfaceId !== "2" ||
    latestEvidence.d3d9RootInterfaceName !== "IDirect3D9" ||
    latestEvidence.d3d9RootInterfacePointer !== "0x01000004" ||
    latestEvidence.d3d9RootInterfaceVtable !== "0x0100001c" ||
    latestEvidence.d3d9RootInterfaceSlots !== 17 ||
    latestEvidence.d3d9RootBackingAddressAllocationId !== 41 ||
    latestEvidence.d3d9RootBackingMemoryOwner !== 13 ||
    latestEvidence.d3d9RootBackingBase !== "0x01000000" ||
    latestEvidence.d3d9RootBackingBytes !== 8_192 ||
    latestEvidence.d3d9RootBackingMappingOperations !== 5 ||
    latestEvidence.d3d9RootBackingDirtyPages !== 2 ||
    latestEvidence.d3d9RootBackingFailureAtomicity !== true ||
    latestEvidence.d3d9RootBrowserOwnsComState !== false
  ) {
    errors.push("latest D3D9 root factory evidence is invalid");
  }
  requireSha256(
    latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256",
    errors,
  );
  requireSha256(
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256",
    errors,
  );
  const nativeD3d9DisplayModeCallerRvas = requireStringArray(
    latestEvidence.nativeD3d9DisplayModeCallerRvas,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModeCallerRvas",
    errors,
  );
  const d3d9GetAdapterDisplayModeArguments = requireStringArray(
    latestEvidence.d3d9GetAdapterDisplayModeArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterDisplayModeArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9DisplayModePrimaryTraceBytes !== 2_192_729 ||
    latestEvidence.nativeD3d9DisplayModePrimaryTraceLines !== 6_886 ||
    latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256 !==
      "6b9bd17809ef235f1817fb08ccd4bfb92f27f3cd424bd0f15500470d73b05208" ||
    latestEvidence.nativeD3d9DisplayModePrimaryBalancedApiPairs !== 3_405 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceBytes !== 2_249_188 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256 !==
      "fac93ab62effd0228dc8c8a88b617f3aced5fcff9429c9e90c9347979c660f92" ||
    latestEvidence.nativeD3d9DisplayModeConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9DisplayModeTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9DisplayModeCallsPerTrace !== 4 ||
    nativeD3d9DisplayModeCallerRvas.join(",") !==
      "0x0005b8d6,0x00059da5" ||
    latestEvidence.nativeD3d9DisplayModeCallerCounts?.join(",") !== "1,3" ||
    latestEvidence.nativeD3d9DisplayModeAdapter !== 0 ||
    latestEvidence.nativeD3d9DisplayModeResult !== "0x00000000" ||
    latestEvidence.nativeD3d9DisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    latestEvidence.nativeD3d9DisplayModeFirstInputZeroed !== true ||
    latestEvidence.nativeD3d9DisplayModeFirstCallFullOverwrite !== true ||
    latestEvidence.nativeD3d9DisplayModeHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9DisplayModeAllFieldsConsumed !== true ||
    latestEvidence.nativeD3d9DisplayModeExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeImplemented !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeSequence !== 17_817 ||
    latestEvidence.d3d9GetAdapterDisplayModeInstructionCount !== 11_806_455 ||
    d3d9GetAdapterDisplayModeArguments.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    latestEvidence.d3d9GetAdapterDisplayModeReturnAddress !== "0x1005b8d6" ||
    latestEvidence.d3d9GetAdapterDisplayModeResult !== "0x00000000" ||
    latestEvidence.d3d9GetAdapterDisplayModeLastError !== 0 ||
    latestEvidence.d3d9GetAdapterDisplayModeGuestWriteBytes !== 16 ||
    latestEvidence.d3d9GetAdapterDisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    latestEvidence.d3d9GetAdapterDisplayModeWidth !== 1_920 ||
    latestEvidence.d3d9GetAdapterDisplayModeHeight !== 1_080 ||
    latestEvidence.d3d9GetAdapterDisplayModeRefreshRate !== 60 ||
    latestEvidence.d3d9GetAdapterDisplayModeFormat !== 22 ||
    latestEvidence.d3d9GetAdapterDisplayModePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterDisplayModePolicyVersion !== 14 ||
    latestEvidence.d3d9GetAdapterDisplayModePolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetAdapterDisplayModeProfileId !==
      "primary-adapter-1920x1080-60-x8r8g8b8-v1" ||
    latestEvidence.d3d9GetAdapterDisplayModeDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9GetAdapterDisplayModeGeometryAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterDisplayModeReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterDisplayModeReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterDisplayModeChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterDisplayModeNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 display-mode evidence is invalid");
  }
  for (const field of [
    "nativeD3d9AdapterIdentifierPrimaryTraceSha256",
    "nativeD3d9AdapterIdentifierConfirmationTraceSha256",
    "nativeD3d9AdapterIdentifierInputSha256",
    "nativeD3d9AdapterIdentifierOutputSha256",
    "d3d9GetAdapterIdentifierOutputSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const d3d9GetAdapterIdentifierArguments = requireOrderedStringArray(
    latestEvidence.d3d9GetAdapterIdentifierArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterIdentifierArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceBytes !== 2_207_106 ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceLines !== 6_888 ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceSha256 !==
      "c635bde508a727e5c42dd0d17b75d3e82e30d62f77ad06bf497133cb4f8bd7d3" ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryBalancedApiPairs !== 3_406 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceBytes !== 2_263_088 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceSha256 !==
      "d13217c0bc87789d61da46af87339e63bb0a92ac11dd406698a6b75982574da5" ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9AdapterIdentifierTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9AdapterIdentifierCallerRva !== "0x0005b91c" ||
    latestEvidence.nativeD3d9AdapterIdentifierAdapter !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierFlags !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierResult !== "0x00000000" ||
    latestEvidence.nativeD3d9AdapterIdentifierInputZeroed !== true ||
    latestEvidence.nativeD3d9AdapterIdentifierInputSha256 !==
      "a805fd6f7ad225e289321ba6e2b079e92c3accc1675a7ba6626101da38f727f2" ||
    latestEvidence.nativeD3d9AdapterIdentifierOutputSha256 !==
      "773a5d417dceeab588a8724b951f9926543e39b42fad71b58d1ba6efb6a8497c" ||
    latestEvidence.nativeD3d9AdapterIdentifierChangedBytes !== 169 ||
    latestEvidence.nativeD3d9AdapterIdentifierHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9AdapterIdentifierOutputFieldsConsumed !== false ||
    latestEvidence.nativeD3d9AdapterIdentifierHostGpuIdentityImported !== false ||
    latestEvidence.nativeD3d9AdapterIdentifierExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterIdentifierImplemented !== true ||
    latestEvidence.d3d9GetAdapterIdentifierSequence !== 17_818 ||
    latestEvidence.d3d9GetAdapterIdentifierInstructionCount !== 11_806_466 ||
    d3d9GetAdapterIdentifierArguments.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    latestEvidence.d3d9GetAdapterIdentifierReturnAddress !== "0x1005b91c" ||
    latestEvidence.d3d9GetAdapterIdentifierResult !== "0x00000000" ||
    latestEvidence.d3d9GetAdapterIdentifierLastError !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierGuestWriteBytes !== 1_100 ||
    latestEvidence.d3d9GetAdapterIdentifierOutputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    latestEvidence.d3d9GetAdapterIdentifierDriver !== "vine-d3d9.dll" ||
    latestEvidence.d3d9GetAdapterIdentifierDescription !==
      "Vine Virtual Direct3D9 Adapter" ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceName !== "\\\\.\\DISPLAY1" ||
    latestEvidence.d3d9GetAdapterIdentifierDriverVersion !==
      "0x0001000000000000" ||
    latestEvidence.d3d9GetAdapterIdentifierVendorId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierSubSysId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierRevision !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceIdentifier !==
      "56494e45-4433-4439-8000-000000000001" ||
    latestEvidence.d3d9GetAdapterIdentifierWhqlLevel !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterIdentifierPolicyVersion !== 14 ||
    latestEvidence.d3d9GetAdapterIdentifierPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetAdapterIdentifierProfileId !==
      "primary-virtual-adapter-identifier-v1" ||
    latestEvidence.d3d9GetAdapterIdentifierDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9GetAdapterIdentifierIdentityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterIdentifierDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterIdentifierReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterIdentifierFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterIdentifierBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterIdentifierRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterIdentifierRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterIdentifierHostGpuIdentityImported !== false ||
    latestEvidence.d3d9GetAdapterIdentifierNodeCompletions !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierChromeCompletions !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 adapter-identifier evidence is invalid");
  }
  for (const field of [
    "nativeD3d9DeviceCapsPrimaryTraceSha256",
    "nativeD3d9DeviceCapsConfirmationTraceSha256",
    "nativeD3d9DeviceCapsOutputSha256",
    "d3d9GetDeviceCapsOutputSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9DeviceCapsConsumedFields = requireStringArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedFields,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedFields",
    errors,
  );
  const nativeD3d9DeviceCapsConsumedFieldOffsets = requireIntegerArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedFieldOffsets,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedFieldOffsets",
    errors,
  );
  const nativeD3d9DeviceCapsConsumedValues = requireStringArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedValues,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedValues",
    errors,
  );
  const d3d9GetDeviceCapsArguments = requireOrderedStringArray(
    latestEvidence.d3d9GetDeviceCapsArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetDeviceCapsArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryBalancedApiPairs !== 3_495 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceBytes !== 2_282_110 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9DeviceCapsTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9DeviceCapsCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9DeviceCapsCallerRva !== "0x0005b949" ||
    latestEvidence.nativeD3d9DeviceCapsAdapter !== 0 ||
    latestEvidence.nativeD3d9DeviceCapsDeviceType !== 1 ||
    latestEvidence.nativeD3d9DeviceCapsResult !== "0x00000000" ||
    latestEvidence.nativeD3d9DeviceCapsOutputBytes !== 304 ||
    latestEvidence.nativeD3d9DeviceCapsOutputSha256 !==
      "5c48557292c6b254d6e9eb9612e3b24aae72f183bcbbcaf803d7a5ac16b41803" ||
    latestEvidence.nativeD3d9DeviceCapsOutputEqualsCallerStackPointerAfterReturn !== true ||
    latestEvidence.nativeD3d9DeviceCapsHresultSignConsumed !== true ||
    nativeD3d9DeviceCapsConsumedFields.join(",") !==
      "Caps,Caps2,PixelShaderVersion" ||
    nativeD3d9DeviceCapsConsumedFieldOffsets.join(",") !== "8,12,204" ||
    nativeD3d9DeviceCapsConsumedValues.join(",") !==
      "0x00020000,0xe0010000,0xffff0300" ||
    latestEvidence.nativeD3d9DeviceCapsConsumerInstructionCount !== 256 ||
    latestEvidence.nativeD3d9DeviceCapsHostGpuCapsImported !== false ||
    latestEvidence.nativeD3d9DeviceCapsExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetDeviceCapsImplemented !== true ||
    latestEvidence.d3d9GetDeviceCapsSequence !== 17_819 ||
    latestEvidence.d3d9GetDeviceCapsInstructionCount !== 11_806_478 ||
    d3d9GetDeviceCapsArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    latestEvidence.d3d9GetDeviceCapsReturnAddress !== "0x1005b949" ||
    latestEvidence.d3d9GetDeviceCapsResult !== "0x00000000" ||
    latestEvidence.d3d9GetDeviceCapsLastError !== 0 ||
    latestEvidence.d3d9GetDeviceCapsGuestWriteBytes !== 304 ||
    latestEvidence.d3d9GetDeviceCapsOutputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    latestEvidence.d3d9GetDeviceCapsDeviceType !== 1 ||
    latestEvidence.d3d9GetDeviceCapsAdapterOrdinal !== 0 ||
    latestEvidence.d3d9GetDeviceCapsCaps !== "0x00020000" ||
    latestEvidence.d3d9GetDeviceCapsCaps2 !== "0x20000000" ||
    latestEvidence.d3d9GetDeviceCapsPixelShaderVersion !== "0xffff0300" ||
    latestEvidence.d3d9GetDeviceCapsNumberOfAdaptersInGroup !== 1 ||
    latestEvidence.d3d9GetDeviceCapsOtherFieldsZero !== true ||
    latestEvidence.d3d9GetDeviceCapsPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetDeviceCapsPolicyVersion !== 14 ||
    latestEvidence.d3d9GetDeviceCapsPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetDeviceCapsProfileId !==
      "primary-hal-readscanline-dynamic-textures-ps3-v1" ||
    latestEvidence.d3d9GetDeviceCapsDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9GetDeviceCapsCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetDeviceCapsDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetDeviceCapsReferenceCount !== 1 ||
    latestEvidence.d3d9GetDeviceCapsReferenceCountMutation !== false ||
    latestEvidence.d3d9GetDeviceCapsFailureAtomicity !== true ||
    latestEvidence.d3d9GetDeviceCapsBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetDeviceCapsRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetDeviceCapsRendererResourceCreated !== false ||
    latestEvidence.d3d9GetDeviceCapsHostGpuCapsImported !== false ||
    latestEvidence.d3d9GetDeviceCapsNodeCompletions !== 1 ||
    latestEvidence.d3d9GetDeviceCapsChromeCompletions !== 1 ||
    latestEvidence.d3d9GetDeviceCapsNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 device-caps evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CheckDeviceTypePrimaryTraceSha256",
    "nativeD3d9CheckDeviceTypeConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9CheckDeviceTypeCallerRvas = requireStringArray(
    latestEvidence.nativeD3d9CheckDeviceTypeDirectCallerRvas,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceTypeDirectCallerRvas",
    errors,
  );
  const nativeD3d9CheckDeviceTypeWindowedValues = requireIntegerArray(
    latestEvidence.nativeD3d9CheckDeviceTypeDirectWindowedValues,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceTypeDirectWindowedValues",
    errors,
  );
  const d3d9CheckDeviceTypeSequences = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeSequences,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeSequences",
    errors,
  );
  const d3d9CheckDeviceTypeInstructionCounts = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeInstructionCounts,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeInstructionCounts",
    errors,
  );
  const d3d9CheckDeviceTypeWindowedValues = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeWindowedValues,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeWindowedValues",
    errors,
  );
  if (
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryBalancedApiPairs !== 3_495 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceBytes !== 2_282_110 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9CheckDeviceTypeTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceTypeCallsPerTrace !== 5 ||
    latestEvidence.nativeD3d9CheckDeviceTypeDirectCallsPerTrace !== 2 ||
    latestEvidence.nativeD3d9CheckDeviceTypeInternalCallsPerTrace !== 3 ||
    nativeD3d9CheckDeviceTypeCallerRvas.join(",") !==
      "0x0005b9d6,0x0005ba1d" ||
    nativeD3d9CheckDeviceTypeWindowedValues.join(",") !== "1,0" ||
    latestEvidence.nativeD3d9CheckDeviceTypeAdapter !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceTypeDeviceType !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceTypeAdapterFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceTypeBackBufferFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceTypeResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CheckDeviceTypeHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CheckDeviceTypeConsumerOracle !==
      "immutable-pe-disassembly-and-device-caps-consumer-snapshot" ||
    latestEvidence.nativeD3d9CheckDeviceTypeExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CheckDeviceTypeImplemented !== true ||
    d3d9CheckDeviceTypeSequences.join(",") !== "17820,17821" ||
    d3d9CheckDeviceTypeInstructionCounts.join(",") !== "11806497,11806510" ||
    JSON.stringify(latestEvidence.d3d9CheckDeviceTypeArguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000001"],
        ["0x01000004", "0x00000000", "0x00000001", "0x00000016", "0x00000016", "0x00000000"],
      ]) ||
    latestEvidence.d3d9CheckDeviceTypeReturnAddresses?.join(",") !==
      "0x1005b9d6,0x1005ba1d" ||
    latestEvidence.d3d9CheckDeviceTypeResults?.join(",") !==
      "0x00000000,0x00000000" ||
    latestEvidence.d3d9CheckDeviceTypeLastError !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeGuestWriteBytes !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeAdapter !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeDeviceType !== 1 ||
    latestEvidence.d3d9CheckDeviceTypeAdapterFormat !== 22 ||
    latestEvidence.d3d9CheckDeviceTypeBackBufferFormat !== 22 ||
    d3d9CheckDeviceTypeWindowedValues.join(",") !== "1,0" ||
    latestEvidence.d3d9CheckDeviceTypeInvalidCallResult !== "0x8876086c" ||
    latestEvidence.d3d9CheckDeviceTypeNotAvailableResult !== "0x8876086a" ||
    latestEvidence.d3d9CheckDeviceTypeBoolNormalization !==
      "zero-fullscreen-nonzero-windowed" ||
    latestEvidence.d3d9CheckDeviceTypePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CheckDeviceTypePolicyVersion !== 14 ||
    latestEvidence.d3d9CheckDeviceTypePolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9CheckDeviceTypeProfileId !==
      "primary-hal-x8r8g8b8-windowed-fullscreen-v1" ||
    latestEvidence.d3d9CheckDeviceTypeDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9CheckDeviceTypeCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9CheckDeviceTypeDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CheckDeviceTypeReferenceCount !== 1 ||
    latestEvidence.d3d9CheckDeviceTypeReferenceCountMutation !== false ||
    latestEvidence.d3d9CheckDeviceTypeFailureAtomicity !== true ||
    latestEvidence.d3d9CheckDeviceTypeBrowserOwnsComState !== false ||
    latestEvidence.d3d9CheckDeviceTypeRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9CheckDeviceTypeRendererResourceCreated !== false ||
    latestEvidence.d3d9CheckDeviceTypeNodeCompletions !== 2 ||
    latestEvidence.d3d9CheckDeviceTypeChromeCompletions !== 2 ||
    latestEvidence.d3d9CheckDeviceTypeNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 device-type evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CheckDeviceFormatPrimaryTraceSha256",
    "nativeD3d9CheckDeviceFormatConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    latestEvidence.nativeD3d9CheckDeviceFormatSupportedUsages,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceFormatSupportedUsages",
    errors,
  );
  const latestD3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceFormatSupportedUsages,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceFormatSupportedUsages",
    errors,
  );
  if (
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryBalancedApiPairs !== 3_495 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceBytes !== 2_282_110 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9CheckDeviceFormatTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceFormatCallsPerTrace !== 13 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceFormatCreateDeviceParentedCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9CheckDeviceFormatOtherInternalCallsPerTrace !== 9 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCallerRva !== "0x0005ba69" ||
    latestEvidence.nativeD3d9CheckDeviceFormatAdapter !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDeviceType !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceFormatAdapterFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatResourceType !== 3 ||
    nativeD3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    latestEvidence.nativeD3d9CheckDeviceFormatSupportedCheckFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatBasicUsageSuccessCallsPerTrace !== 6 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDynamicUsageSuccessCallsPerTrace !== 4 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedCheckFormat !== 118 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedResult !== "0x8876086a" ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectUsage !== 512 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCheckFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CheckDeviceFormatHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CheckDeviceFormatConsumerOracle !==
      "immutable-pe-disassembly" ||
    latestEvidence.nativeD3d9CheckDeviceFormatExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CheckDeviceFormatImplemented !== true ||
    latestEvidence.d3d9CheckDeviceFormatSequence !== 17_822 ||
    latestEvidence.d3d9CheckDeviceFormatInstructionCount !== 11_806_524 ||
    latestEvidence.d3d9CheckDeviceFormatArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    latestEvidence.d3d9CheckDeviceFormatReturnAddress !== "0x1005ba69" ||
    latestEvidence.d3d9CheckDeviceFormatResult !== "0x00000000" ||
    latestEvidence.d3d9CheckDeviceFormatLastError !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatGuestWriteBytes !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatAdapter !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatDeviceType !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatAdapterFormat !== 22 ||
    latestEvidence.d3d9CheckDeviceFormatUsage !== 512 ||
    latestEvidence.d3d9CheckDeviceFormatResourceType !== 3 ||
    latestEvidence.d3d9CheckDeviceFormatCheckFormat !== 22 ||
    latestD3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    latestEvidence.d3d9CheckDeviceFormatInvalidCallResult !== "0x8876086c" ||
    latestEvidence.d3d9CheckDeviceFormatNotAvailableResult !== "0x8876086a" ||
    latestEvidence.d3d9CheckDeviceFormatPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CheckDeviceFormatPolicyVersion !== 14 ||
    latestEvidence.d3d9CheckDeviceFormatPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9CheckDeviceFormatProfileId !==
      "primary-hal-x8r8g8b8-texture-basic-dynamic-v1" ||
    latestEvidence.d3d9CheckDeviceFormatDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9CheckDeviceFormatCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9CheckDeviceFormatDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CheckDeviceFormatReferenceCount !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatReferenceCountMutation !== false ||
    latestEvidence.d3d9CheckDeviceFormatFailureAtomicity !== true ||
    latestEvidence.d3d9CheckDeviceFormatBrowserOwnsComState !== false ||
    latestEvidence.d3d9CheckDeviceFormatRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9CheckDeviceFormatRendererResourceCreated !== false ||
    latestEvidence.d3d9CheckDeviceFormatNodeCompletions !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatChromeCompletions !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 device-format evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CreateDevicePrimaryTraceSha256",
    "nativeD3d9CreateDevicePrimarySummarySha256",
    "nativeD3d9CreateDeviceConfirmationTraceSha256",
    "nativeD3d9CreateDeviceConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceBytes !== 2_333_677 ||
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceLines !== 7_075 ||
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    latestEvidence.nativeD3d9CreateDevicePrimaryBalancedApiPairs !== 3_498 ||
    latestEvidence.nativeD3d9CreateDevicePrimarySummaryBytes !== 275_843 ||
    latestEvidence.nativeD3d9CreateDevicePrimarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceBytes !== 2_355_307 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceLines !== 7_121 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationBalancedApiPairs !== 3_521 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationSummaryBytes !== 275_855 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    latestEvidence.nativeD3d9CreateDeviceTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9CreateDeviceCallerRva !== "0x0005aca5" ||
    latestEvidence.nativeD3d9CreateDeviceAdapter !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceDeviceType !== 1 ||
    latestEvidence.nativeD3d9CreateDeviceBehaviorFlags !== "0x00000044" ||
    JSON.stringify(latestEvidence.nativeD3d9CreateDeviceInputWidthHeightFormat) !==
      JSON.stringify([0, 0, 0]) ||
    JSON.stringify(latestEvidence.nativeD3d9CreateDeviceResolvedWidthHeightFormat) !==
      JSON.stringify([[800, 600, 22], [1280, 720, 22], [1280, 720, 22]]) ||
    latestEvidence.nativeD3d9CreateDevicePreservedBackBufferCount !== 1 ||
    latestEvidence.nativeD3d9CreateDevicePreservedSwapEffect !== 3 ||
    latestEvidence.nativeD3d9CreateDevicePreservedWindowed !== 1 ||
    latestEvidence.nativeD3d9CreateDevicePreservedAutoDepthStencilFormat !== 80 ||
    latestEvidence.nativeD3d9CreateDeviceResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CreateDeviceLastError !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceOutputNonNull !== true ||
    latestEvidence.nativeD3d9CreateDeviceHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CreateDeviceFirstReachedDeviceMethod !==
      "CreateTexture" ||
    latestEvidence.nativeD3d9CreateDeviceFirstReachedDeviceSlot !== 23 ||
    latestEvidence.nativeD3d9CreateDeviceNestedAddRefResult !== 2 ||
    latestEvidence.nativeD3d9CreateDeviceExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CreateDeviceImplemented !== true ||
    latestEvidence.d3d9CreateDeviceSequence !== 17_849 ||
    latestEvidence.d3d9CreateDeviceInstructionCount !== 11_807_387 ||
    latestEvidence.d3d9CreateDeviceArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    latestEvidence.d3d9CreateDeviceReturnAddress !== "0x1005aca5" ||
    latestEvidence.d3d9CreateDeviceResult !== "0x00000000" ||
    latestEvidence.d3d9CreateDeviceLastError !== 0 ||
    latestEvidence.d3d9CreateDeviceGuestWriteBytes !== 16 ||
    latestEvidence.d3d9CreateDevicePresentationPrefixHex !==
      "200300005802000016000000" ||
    latestEvidence.d3d9CreateDeviceOutputPointer !== "0x01010004" ||
    latestEvidence.d3d9CreateDevicePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CreateDevicePolicyVersion !== 14 ||
    latestEvidence.d3d9CreateDevicePolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9CreateDeviceProfileId !==
      "primary-windowed-hal-device-v1" ||
    latestEvidence.d3d9CreateDeviceDisplayProfileId !==
      "single-monitor-1920x1080-v1" ||
    latestEvidence.d3d9CreateDeviceWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.d3d9CreateDeviceDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CreateDeviceGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9CreateDeviceComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9CreateDeviceDeviceClassIdentity !==
      "d3d9.idirect3ddevice9.v1" ||
    latestEvidence.d3d9CreateDeviceDeviceInterface !== "IDirect3DDevice9" ||
    latestEvidence.d3d9CreateDeviceDeviceVtableSlots !== 119 ||
    latestEvidence.d3d9CreateDeviceDeviceReferenceCount !== 1 ||
    latestEvidence.d3d9CreateDeviceRootReferenceCount !== 2 ||
    latestEvidence.d3d9CreateDeviceRetainsRootReference !== true ||
    latestEvidence.d3d9CreateDeviceLogicalGraphicsDeviceId !== 1 ||
    latestEvidence.d3d9CreateDeviceLogicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    JSON.stringify(latestEvidence.d3d9CreateDeviceLogicalPresentation) !==
      JSON.stringify([800, 600, 22, 1, 1, 3, 1]) ||
    latestEvidence.d3d9CreateDeviceGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9CreateDeviceFailureAtomicity !== true ||
    latestEvidence.d3d9CreateDeviceFinalReleaseRetiresGraphicsAndMayCascadeRoot !==
      true ||
    latestEvidence.d3d9CreateDeviceBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9CreateDeviceRendererResourceCreated !== false ||
    latestEvidence.d3d9CreateDeviceNodeCompletions !== 1 ||
    latestEvidence.d3d9CreateDeviceChromeCompletions !== 1 ||
    latestEvidence.d3d9CreateDeviceNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 CreateDevice evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CreateTexturePrimaryTraceSha256",
    "nativeD3d9CreateTexturePrimarySummarySha256",
    "nativeD3d9CreateTextureConfirmationTraceSha256",
    "nativeD3d9CreateTextureConfirmationSummarySha256",
    "nativeD3d9InitialTexturesPrimaryTraceSha256",
    "nativeD3d9InitialTexturesPrimarySummarySha256",
    "nativeD3d9InitialTexturesConfirmationTraceSha256",
    "nativeD3d9InitialTexturesConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceBytes !== 2_333_677 ||
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceLines !== 7_075 ||
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    latestEvidence.nativeD3d9CreateTexturePrimaryBalancedApiPairs !== 3_498 ||
    latestEvidence.nativeD3d9CreateTexturePrimarySummaryBytes !== 275_843 ||
    latestEvidence.nativeD3d9CreateTexturePrimarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceBytes !== 2_355_307 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceLines !== 7_121 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    latestEvidence.nativeD3d9CreateTextureConfirmationBalancedApiPairs !== 3_521 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationSummaryBytes !== 275_855 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    latestEvidence.nativeD3d9CreateTextureTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CreateTextureCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9CreateTextureCallerRva !== "0x0005a715" ||
    latestEvidence.nativeD3d9CreateTextureArguments?.join(",") !==
      "1024,1024,1,0,22,2" ||
    latestEvidence.nativeD3d9CreateTextureSharedHandleNull !== true ||
    latestEvidence.nativeD3d9CreateTextureResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CreateTextureLastError !== 0 ||
    latestEvidence.nativeD3d9CreateTextureHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CreateTextureOutputSnapshotCaptured !== false ||
    latestEvidence.nativeD3d9CreateTexturePointerClaimed !== false ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextMethod !==
      "GetSurfaceLevel" ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextSlot !== 18 ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextLevel !== 0 ||
    latestEvidence.nativeD3d9CreateTextureExecutableHashUnchanged !== true ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceBytes !== 261_943 ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceLines !== 505 ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceSha256 !==
      "2fe16a38d7a8c8ee18b673d06f759ef3bfa3d412d3f688689692edb4d6d55ad4" ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryBalancedApiPairs !== 219 ||
    latestEvidence.nativeD3d9InitialTexturesPrimarySummaryBytes !== 328_531 ||
    latestEvidence.nativeD3d9InitialTexturesPrimarySummarySha256 !==
      "85db2bcea0fb895c1481a3af7330a3c79b430cbed997761cdb23d30e0fbf875d" ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceBytes !== 262_766 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceLines !== 508 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceSha256 !==
      "cdf646718a1a187e72129d9ae5562d44b4c5432c7c9c60ee1c217d30ae90179f" ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationBalancedApiPairs !== 220 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationSummaryBytes !== 328_698 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationSummarySha256 !==
      "20eb05e775a84075aec1c48b2137c9d237f684e2e19d580b1aac5671a7039837" ||
    latestEvidence.nativeD3d9InitialTexturesTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9InitialTexturesProfileId !==
      "x86-bgi-initial-textures-v1" ||
    latestEvidence.nativeD3d9InitialTexturesCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9InitialTexturesCallerRvas?.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    JSON.stringify(latestEvidence.nativeD3d9InitialTexturesVariants) !==
      JSON.stringify([[1_024, 1_024, 1, 0, 22, 2], [1_024, 1_024, 1, 0, 22, 0], [1_024, 1_024, 1, 512, 22, 0]]) ||
    latestEvidence.nativeD3d9InitialTexturesSharedHandlesNull !== true ||
    latestEvidence.nativeD3d9InitialTexturesResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    latestEvidence.nativeD3d9InitialTexturesLastErrors?.join(",") !== "0,0,0" ||
    latestEvidence.nativeD3d9InitialTexturesSameDevicePerTrace !== true ||
    latestEvidence.nativeD3d9InitialTexturesOutputAddresses?.join(",") !==
      "0x00d117cc,0x00d117d0,0x00d117d4" ||
    latestEvidence.nativeD3d9InitialTexturesReturnedPointersDistinct !== true ||
    latestEvidence.nativeD3d9InitialTexturesPointersOpaqueArtifactIdentity !== true ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextMethod !==
      "CreateVertexBuffer" ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextSlot !== 26 ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextArguments?.join(",") !==
      "112,8,324,1" ||
    latestEvidence.nativeD3d9InitialTexturesExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CreateTextureImplemented !== true ||
    latestEvidence.d3d9CreateTextureSequence !== 17_850 ||
    latestEvidence.d3d9CreateTextureInstructionCount !== 11_807_772 ||
    latestEvidence.d3d9CreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    latestEvidence.d3d9CreateTextureReturnAddress !== "0x1005a715" ||
    latestEvidence.d3d9CreateTextureResult !== "0x00000000" ||
    latestEvidence.d3d9CreateTextureLastError !== 0 ||
    latestEvidence.d3d9CreateTextureGuestWriteBytes !== 4 ||
    latestEvidence.d3d9CreateTextureOutputPointer !== "0x01020004" ||
    latestEvidence.d3d9CreateTexturePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CreateTexturePolicyVersion !== 14 ||
    latestEvidence.d3d9CreateTexturePolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9CreateTextureProfileId !==
      "primary-initial-x8r8g8b8-textures-v1" ||
    latestEvidence.d3d9CreateTextureGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9CreateTextureComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9CreateTextureClassIdentity !==
      "d3d9.idirect3dtexture9.v1" ||
    latestEvidence.d3d9CreateTextureInterface !== "IDirect3DTexture9" ||
    latestEvidence.d3d9CreateTextureVtableSlots !== 22 ||
    latestEvidence.d3d9CreateTextureDeviceReferenceCount !== 2 ||
    latestEvidence.d3d9CreateTextureReferenceCount !== 1 ||
    latestEvidence.d3d9CreateTextureRetainsDeviceReference !== true ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsDeviceId !== 1 ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsTextureId !== 1 ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsTextureNamespace !==
      "vine.logical-graphics-texture.v1" ||
    latestEvidence.d3d9CreateTextureLogicalDescriptor?.join(",") !==
      "1024,1024,1,0,22,2" ||
    latestEvidence.d3d9CreateTextureGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9CreateTextureFailureAtomicity !== true ||
    latestEvidence.d3d9CreateTextureFinalReleaseRetiresGraphicsAndMayCascadeDeviceRoot !==
      true ||
    latestEvidence.d3d9CreateTextureBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9CreateTextureRendererResourceCreated !== false ||
    latestEvidence.d3d9CreateTextureNodeCompletions !== 3 ||
    latestEvidence.d3d9CreateTextureChromeCompletions !== 3 ||
    latestEvidence.d3d9CreateTextureNodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.d3d9InitialTexturesNodeSequences?.join(",") !==
      "17850,17857,17858" ||
    latestEvidence.d3d9InitialTexturesNodeInstructionCounts?.join(",") !==
      "11807772,11807840,11807856" ||
    latestEvidence.d3d9InitialTexturesChromeSequences?.join(",") !==
      "17850,17857,17858" ||
    latestEvidence.d3d9InitialTexturesCallerReturnAddresses?.join(",") !==
      "0x1005a715,0x1005a7b7,0x1005a7dc" ||
    JSON.stringify(latestEvidence.d3d9InitialTexturesArguments) !==
      JSON.stringify([
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000000", "0x00000016", "0x00000002", "0x101517cc", "0x00000000"],
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000000", "0x00000016", "0x00000000", "0x101517d0", "0x00000000"],
        ["0x01010004", "0x00000400", "0x00000400", "0x00000001", "0x00000200", "0x00000016", "0x00000000", "0x101517d4", "0x00000000"],
      ]) ||
    latestEvidence.d3d9InitialTexturesResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    latestEvidence.d3d9InitialTexturesOutputAddresses?.join(",") !==
      "0x101517cc,0x101517d0,0x101517d4" ||
    latestEvidence.d3d9InitialTexturesPointers?.join(",") !==
      "0x01020004,0x01030004,0x01040004" ||
    latestEvidence.d3d9InitialTexturesGuestWriteBytes?.join(",") !== "4,4,4" ||
    JSON.stringify(latestEvidence.d3d9InitialTexturesLogicalDescriptors) !==
      JSON.stringify([[1_024, 1_024, 1, 0, 22, 2], [1_024, 1_024, 1, 0, 22, 0], [1_024, 1_024, 1, 512, 22, 0]]) ||
    latestEvidence.d3d9InitialTexturesLogicalGraphicsDeviceCount !== 1 ||
    latestEvidence.d3d9InitialTexturesLogicalGraphicsTextureCount !== 3 ||
    latestEvidence.d3d9InitialTexturesDeviceReferenceCountAtStop !== 4 ||
    latestEvidence.d3d9InitialTexturesBrowserRendererCreated !== false ||
    latestEvidence.d3d9InitialTexturesPixelStorageCreated !== false ||
    latestEvidence.d3d9InitialTexturesNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 CreateTexture evidence is invalid");
  }
  for (const field of [
    "nativeD3d9GetSurfaceLevelPrimaryTraceSha256",
    "nativeD3d9GetSurfaceLevelPrimarySummarySha256",
    "nativeD3d9GetSurfaceLevelConfirmationTraceSha256",
    "nativeD3d9GetSurfaceLevelConfirmationSummarySha256",
    "d3d9GetSurfaceLevelOracleSourceSha256",
    "d3d9GetSurfaceLevelOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceBytes !== 228_637 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceLines !== 514 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceSha256 !==
      "54aa56bade26d675f864162bb7a7a2f8b4fc30f8df8dbd0835681cc09f9eced2" ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryBalancedApiPairs !== 222 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimarySummaryBytes !== 230_104 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimarySummarySha256 !==
      "c68ef403d864c89ad466ed3bda3cef750065d7555b815fb23f3ad99a43697c8e" ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceBytes !== 229_203 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceLines !== 514 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceSha256 !==
      "e3f6d853293cfa8f395695c2de21cb4675cc71f27efa4b3715f2171575ad1422" ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationBalancedApiPairs !== 222 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationSummaryBytes !== 230_131 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationSummarySha256 !==
      "1ed3b5f247a8b6dd82efd2cdaa7cb0393e68d7db9639d07eaa1afa2244b4a81f" ||
    latestEvidence.nativeD3d9GetSurfaceLevelTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9GetSurfaceLevelCallerRva !== "0x0005a731" ||
    latestEvidence.nativeD3d9GetSurfaceLevelLevel !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelResult !== "0x00000000" ||
    latestEvidence.nativeD3d9GetSurfaceLevelLastError !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelOutputNonNull !== true ||
    latestEvidence.nativeD3d9GetSurfaceLevelHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextMethod !== "GetDC" ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextSlot !== 15 ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextCallerRva !== "0x0005a746" ||
    latestEvidence.nativeD3d9GetSurfaceLevelExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleArchitecture !== "coff-i386" ||
    latestEvidence.d3d9GetSurfaceLevelOracleSourceBytes !== 13_977 ||
    latestEvidence.d3d9GetSurfaceLevelOracleSourceSha256 !==
      "1ec0c191bb9783abcbfae7b92ba6d7136018111c4b1da7c860d7dd343daca562" ||
    latestEvidence.d3d9GetSurfaceLevelOracleExecutableBytes !== 12_288 ||
    latestEvidence.d3d9GetSurfaceLevelOracleExecutableSha256 !==
      "7ca28333d06b7352b032b0af9ceb22a016e03d5830f17a74231569ef2b2fee9e" ||
    latestEvidence.d3d9GetSurfaceLevelOracleRepeatSamePointer !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleRepeatSameIunknown !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleSurfaceIunknownDistinctFromTexture !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleContainerIsTexture !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleInvalidLevelResult !== "0x8876086c" ||
    latestEvidence.d3d9GetSurfaceLevelOracleInvalidLevelZerosOutput !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleNullOutputException !== "0xc0000005" ||
    latestEvidence.d3d9GetSurfaceLevelOracleSuccessPreservesLastError !== true ||
    latestEvidence.d3d9GetSurfaceLevelImplemented !== true ||
    latestEvidence.d3d9GetSurfaceLevelSequence !== 17_851 ||
    latestEvidence.d3d9GetSurfaceLevelInstructionCount !== 11_807_783 ||
    latestEvidence.d3d9GetSurfaceLevelArguments?.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    latestEvidence.d3d9GetSurfaceLevelReturnAddress !== "0x1005a731" ||
    latestEvidence.d3d9GetSurfaceLevelResult !== "0x00000000" ||
    latestEvidence.d3d9GetSurfaceLevelLastError !== 0 ||
    latestEvidence.d3d9GetSurfaceLevelGuestWriteBytes !== 4 ||
    latestEvidence.d3d9GetSurfaceLevelOutputPointer !== "0x01030004" ||
    latestEvidence.d3d9GetSurfaceLevelPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetSurfaceLevelPolicyVersion !== 14 ||
    latestEvidence.d3d9GetSurfaceLevelPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9GetSurfaceLevelProfileId !==
      "primary-texture-level-zero-surface-v1" ||
    latestEvidence.d3d9GetSurfaceLevelGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9GetSurfaceLevelComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9GetSurfaceLevelClassIdentity !==
      "d3d9.idirect3dsurface9.v1" ||
    latestEvidence.d3d9GetSurfaceLevelInterface !== "IDirect3DSurface9" ||
    latestEvidence.d3d9GetSurfaceLevelVtableSlots !== 17 ||
    latestEvidence.d3d9GetSurfaceLevelTextureReferenceCount !== 2 ||
    latestEvidence.d3d9GetSurfaceLevelReferenceCount !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelRetainsTextureReference !== true ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsTextureId !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceId !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceNamespace !==
      "vine.logical-graphics-surface.v1" ||
    latestEvidence.d3d9GetSurfaceLevelLogicalDescriptor?.join(",") !==
      "0,1024,1024,22" ||
    latestEvidence.d3d9GetSurfaceLevelRepeatReusesIdentityAndAddsReference !== true ||
    latestEvidence.d3d9GetSurfaceLevelGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9GetSurfaceLevelFailureAtomicity !== true ||
    latestEvidence.d3d9GetSurfaceLevelFinalReleaseRetiresGraphicsAndMayCascadeTextureDeviceRoot !==
      true ||
    latestEvidence.d3d9GetSurfaceLevelBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9GetSurfaceLevelRendererResourceCreated !== false ||
    latestEvidence.d3d9GetSurfaceLevelNodeCompletions !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelChromeCompletions !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 GetSurfaceLevel evidence is invalid");
  }
  for (const field of [
    "nativeD3d9SurfaceDcPrimaryTraceSha256",
    "nativeD3d9SurfaceDcPrimarySummarySha256",
    "nativeD3d9SurfaceDcConfirmationTraceSha256",
    "nativeD3d9SurfaceDcConfirmationSummarySha256",
    "d3d9SurfaceDcOracleSourceSha256",
    "d3d9SurfaceDcOracleExecutableSha256",
    "d3d9SurfaceDcOracleNullOutputExecutableSha256",
    "d3d9SurfaceDcOracleReadOnlyOutputExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const latestSurfaceDcEvidencePairs = [
    [latestEvidence.nativeD3d9SurfaceDcPrimaryTraceBytes, d3d9NativeSurfaceDc.primaryTraceBytes],
    [latestEvidence.nativeD3d9SurfaceDcPrimaryTraceLines, d3d9NativeSurfaceDc.primaryTraceLines],
    [latestEvidence.nativeD3d9SurfaceDcPrimaryTraceSha256, d3d9NativeSurfaceDc.primaryTraceSha256],
    [latestEvidence.nativeD3d9SurfaceDcPrimaryBalancedApiPairs, d3d9NativeSurfaceDc.primaryBalancedApiPairs],
    [latestEvidence.nativeD3d9SurfaceDcPrimarySummaryBytes, d3d9NativeSurfaceDc.primarySummaryBytes],
    [latestEvidence.nativeD3d9SurfaceDcPrimarySummarySha256, d3d9NativeSurfaceDc.primarySummarySha256],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationTraceBytes, d3d9NativeSurfaceDc.confirmationTraceBytes],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationTraceLines, d3d9NativeSurfaceDc.confirmationTraceLines],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationTraceSha256, d3d9NativeSurfaceDc.confirmationTraceSha256],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationBalancedApiPairs, d3d9NativeSurfaceDc.confirmationBalancedApiPairs],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationSummaryBytes, d3d9NativeSurfaceDc.confirmationSummaryBytes],
    [latestEvidence.nativeD3d9SurfaceDcConfirmationSummarySha256, d3d9NativeSurfaceDc.confirmationSummarySha256],
    [latestEvidence.nativeD3d9SurfaceDcTraceDiagnostics, d3d9NativeSurfaceDc.diagnostics],
    [latestEvidence.nativeD3d9SurfaceDcProfileId, d3d9NativeSurfaceDc.profileId],
    [latestEvidence.nativeD3d9SurfaceDcCallsPerTrace, d3d9NativeSurfaceDc.callsPerTrace],
    [latestEvidence.nativeD3d9SurfaceDcCallerRvas, d3d9NativeSurfaceDc.callerRvas],
    [latestEvidence.nativeD3d9SurfaceDcOrderedMethods, d3d9NativeSurfaceDc.orderedMethods],
    [latestEvidence.nativeD3d9SurfaceDcGetDcResult, d3d9NativeSurfaceDc.getDcResult],
    [latestEvidence.nativeD3d9SurfaceDcGetDcOutputNonNull, d3d9NativeSurfaceDc.getDcOutputNonNull],
    [latestEvidence.nativeD3d9SurfaceDcStockObjectIndex, d3d9NativeSurfaceDc.stockObjectIndex],
    [latestEvidence.nativeD3d9SurfaceDcStockObjectResultNonNull, d3d9NativeSurfaceDc.stockObjectResultNonNull],
    [latestEvidence.nativeD3d9SurfaceDcRect, d3d9NativeSurfaceDc.rect],
    [latestEvidence.nativeD3d9SurfaceDcFillRectResult, d3d9NativeSurfaceDc.fillRectResult],
    [latestEvidence.nativeD3d9SurfaceDcReleaseDcResult, d3d9NativeSurfaceDc.releaseDcResult],
    [latestEvidence.nativeD3d9SurfaceDcSurfaceReleaseResult, d3d9NativeSurfaceDc.surfaceReleaseResult],
    [latestEvidence.nativeD3d9SurfaceDcLastErrors, d3d9NativeSurfaceDc.lastErrors],
    [latestEvidence.nativeD3d9SurfaceDcExecutableHashUnchanged, d3d9NativeSurfaceDc.sourceExecutableHashUnchanged],
    [latestEvidence.d3d9SurfaceDcOracleArchitecture, d3d9ControlledSurfaceDc.architecture],
    [latestEvidence.d3d9SurfaceDcOracleSourceBytes, d3d9ControlledSurfaceDc.sourceBytes],
    [latestEvidence.d3d9SurfaceDcOracleSourceSha256, d3d9ControlledSurfaceDc.sourceSha256],
    [latestEvidence.d3d9SurfaceDcOracleExecutableBytes, d3d9ControlledSurfaceDc.executableBytes],
    [latestEvidence.d3d9SurfaceDcOracleExecutableSha256, d3d9ControlledSurfaceDc.executableSha256],
    [latestEvidence.d3d9SurfaceDcOracleNullOutputExecutableSha256, d3d9ControlledSurfaceDc.nullOutputExecutableSha256],
    [latestEvidence.d3d9SurfaceDcOracleReadOnlyOutputExecutableSha256, d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256],
    [latestEvidence.d3d9SurfaceDcOracleFirstGetDcResult, d3d9ControlledSurfaceDc.firstGetDcResult],
    [latestEvidence.d3d9SurfaceDcOracleRepeatGetDcResult, d3d9ControlledSurfaceDc.repeatGetDcResult],
    [latestEvidence.d3d9SurfaceDcOracleRepeatOutputPreserved, d3d9ControlledSurfaceDc.repeatGetDcPreservesOutput],
    [latestEvidence.d3d9SurfaceDcOracleReacquireResult, d3d9ControlledSurfaceDc.reacquireGetDcResult],
    [latestEvidence.d3d9SurfaceDcOracleReferenceCounts, d3d9ControlledSurfaceDc.surfaceReferenceCounts],
    [latestEvidence.d3d9SurfaceDcOracleWrongReleaseResult, d3d9ControlledSurfaceDc.wrongReleaseDcResult],
    [latestEvidence.d3d9SurfaceDcOracleRepeatedReleaseResult, d3d9ControlledSurfaceDc.repeatedReleaseDcResult],
    [latestEvidence.d3d9SurfaceDcOracleCorrectReleaseResult, d3d9ControlledSurfaceDc.correctReleaseDcResult],
    [latestEvidence.d3d9SurfaceDcOracleLockWhileDcResult, d3d9ControlledSurfaceDc.lockRectWhileDcResult],
    [latestEvidence.d3d9SurfaceDcOracleLockAfterReleaseResult, d3d9ControlledSurfaceDc.lockRectAfterReleaseResult],
    [latestEvidence.d3d9SurfaceDcOracleNullOutputException, d3d9ControlledSurfaceDc.nullOutputException],
    [latestEvidence.d3d9SurfaceDcOracleReadOnlyOutputException, d3d9ControlledSurfaceDc.readOnlyOutputException],
    [latestEvidence.d3d9SurfaceDcOracleSuccessPreservesLastError, d3d9ControlledSurfaceDc.successPreservesLastError],
    [latestEvidence.d3d9SurfaceDcProfileId, d3d9SurfaceDc.profileId],
    [latestEvidence.d3d9SurfaceDcNodeSequences, d3d9PrivateSurfaceDc.nodeSequences],
    [latestEvidence.d3d9SurfaceDcNodeInstructionCounts, d3d9PrivateSurfaceDc.nodeInstructionCounts],
    [latestEvidence.d3d9SurfaceDcChromeSequences, d3d9PrivateSurfaceDc.chromeSequences],
    [latestEvidence.d3d9SurfaceDcCallerReturnAddresses, d3d9PrivateSurfaceDc.callerReturnAddresses],
    [latestEvidence.d3d9SurfaceDcSurfacePointer, d3d9PrivateSurfaceDc.surfacePointer],
    [latestEvidence.d3d9SurfaceDcOutputAddress, d3d9PrivateSurfaceDc.getDcOutputAddress],
    [latestEvidence.d3d9SurfaceDcHandle, d3d9PrivateSurfaceDc.deviceContextHandle],
    [latestEvidence.d3d9SurfaceDcDeviceContextId, d3d9PrivateSurfaceDc.deviceContextId],
    [latestEvidence.d3d9SurfaceDcHandleNamespace, d3d9PrivateSurfaceDc.deviceContextNamespace],
    [latestEvidence.d3d9SurfaceDcTargetKind, d3d9PrivateSurfaceDc.deviceContextTargetKind],
    [latestEvidence.d3d9SurfaceDcGraphicsAuthority, d3d9SurfaceDc.graphicsSurfaceAuthority],
    [latestEvidence.d3d9SurfaceDcGdiAuthority, d3d9SurfaceDc.deviceContextAuthority],
    [latestEvidence.d3d9SurfaceDcLogicalGraphicsSurfaceId, d3d9PrivateSurfaceDc.logicalGraphicsSurfaceId],
    [latestEvidence.d3d9SurfaceDcSurfaceComObjectId, d3d9PrivateSurfaceDc.surfaceComObjectId],
    [latestEvidence.d3d9SurfaceDcSurfaceGeometry, d3d9PrivateSurfaceDc.surfaceGeometry],
    [latestEvidence.d3d9SurfaceDcColorFormat, d3d9PrivateSurfaceDc.surfaceColorFormat],
    [latestEvidence.d3d9SurfaceDcFillRect, d3d9PrivateSurfaceDc.fillRect],
    [latestEvidence.d3d9SurfaceDcFillColorRef, d3d9PrivateSurfaceDc.fillColorRef],
    [latestEvidence.d3d9SurfaceDcContentGeneration, d3d9PrivateSurfaceDc.surfaceContentGeneration],
    [latestEvidence.d3d9SurfaceDcSurfaceGeneration, d3d9PrivateSurfaceDc.surfaceGeneration],
    [latestEvidence.d3d9SurfaceDcSingleOutstandingPerSurface, d3d9SurfaceDc.singleOutstandingDeviceContextPerSurface],
    [latestEvidence.d3d9SurfaceDcGuestWriteAndGdiPublicationAtomic, d3d9SurfaceDc.acquisitionAndGuestWriteAtomic],
    [latestEvidence.d3d9SurfaceDcLiveHdcBlocksComRetirement, d3d9SurfaceDc.liveDeviceContextBlocksSurfaceRetirement],
    [latestEvidence.d3d9SurfaceDcFinalReferenceCount, d3d9PrivateSurfaceDc.surfaceReleaseReferenceCount],
    [latestEvidence.d3d9SurfaceDcObjectRetired, d3d9PrivateSurfaceDc.surfaceObjectRetired],
    [latestEvidence.d3d9SurfaceDcLogicalGraphicsSurfaceCountAfterRelease, d3d9PrivateSurfaceDc.logicalGraphicsSurfaceCountAfterRelease],
    [latestEvidence.d3d9SurfaceDcSecondCreateTextureSequence, d3d9PrivateSurfaceDc.secondCreateTextureSequence],
    [latestEvidence.d3d9SurfaceDcSecondCreateTextureInstructionCount, d3d9PrivateSurfaceDc.secondCreateTextureInstructionCount],
    [latestEvidence.d3d9SurfaceDcSecondCreateTexturePool, d3d9PrivateSurfaceDc.secondCreateTexturePool],
    [latestEvidence.d3d9SurfaceDcSecondCreateTextureResult, d3d9PrivateSurfaceDc.secondCreateTextureResult],
    [latestEvidence.d3d9SurfaceDcRendererResourceCreated, d3d9PrivateSurfaceDc.browserRendererCreated],
    [latestEvidence.d3d9SurfaceDcPixelStorageCreated, d3d9PrivateSurfaceDc.pixelStorageCreated],
    [latestEvidence.d3d9SurfaceDcNodeChromeOrderedSemanticsMatched, d3d9PrivateSurfaceDc.nodeChromeOrderedSemanticsMatched],
  ];
  if (
    latestSurfaceDcEvidencePairs.some(
      ([actual, expected]) => JSON.stringify(actual) !== JSON.stringify(expected),
    ) ||
    latestEvidence.d3d9SurfaceDcImplemented !== true ||
    latestEvidence.d3d9SurfaceDcPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9SurfaceDcPolicyVersion !== 14 ||
    latestEvidence.d3d9SurfaceDcPolicyId !==
      "d3d9-primary-virtual-adapter-v14" ||
    latestEvidence.d3d9SurfaceDcFailureAtomicity !== true ||
    latestEvidence.d3d9SurfaceDcBrowserOwnsWindowsOrGraphicsState !== false
  ) {
    errors.push("latest D3D9 Surface DC evidence is invalid");
  }
  requireSha256(
    latestEvidence.nativeD3dxSystemDllSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3dxSystemDllSha256",
    errors,
  );
  if (
    latestEvidence.nativeD3dxRuntimeModuleProfile !==
      "x86-d3dx9-43-load-and-export-v1" ||
    latestEvidence.nativeD3dxGameDirectoryDllPresent !== false ||
    latestEvidence.nativeD3dxSystemDllBytes !== 1_998_168 ||
    latestEvidence.nativeD3dxSystemDllSha256 !==
      "0b28546be22c71834501f7d7185ede5d79742457331c7ee09efc14490dd64f5f" ||
    latestEvidence.nativeD3dxSystemDllEntryPointRva !== "0x001bec0d" ||
    latestEvidence.nativeD3dxSystemDllImports?.join(",") !==
      "msvcrt.dll,GDI32.dll,KERNEL32.dll,ADVAPI32.dll" ||
    latestEvidence.nativeLoadLibraryARequestedName !== "d3dx9_43.dll" ||
    latestEvidence.nativeLoadLibraryACallerReturnRva !== "0x0005bab0" ||
    latestEvidence.nativeLoadLibraryAResultNonNull !== true ||
    latestEvidence.nativeLoadLibraryAEntryLastError !== 0 ||
    latestEvidence.nativeLoadLibraryAExitLastError !== 0 ||
    latestEvidence.nativeLoadLibraryANullLastError !== 87 ||
    latestEvidence.nativeLoadLibraryAEmptyLastError !== 87 ||
    latestEvidence.nativeLoadLibraryAMissingBareDllLastError !== 126 ||
    latestEvidence.nativeLoadLibraryAConsumer !==
      "store-global-test-nonzero-pass-hmodule-to-get-proc-address" ||
    latestEvidence.nativeD3dxGetProcAddressName !== "D3DXCreateEffect" ||
    latestEvidence.nativeD3dxGetProcAddressCallerReturnRva !== "0x0005bac5" ||
    latestEvidence.nativeD3dxGetProcAddressResultNonNull !== true ||
    latestEvidence.nativeD3dxGetProcAddressLastErrorPreserved !== true ||
    latestEvidence.nativeD3dxGetProcAddressConsumer !==
      "store-global-not-invoked-before-next-structured-stop" ||
    latestEvidence.nativeLoadLibraryAExecutableHashUnchanged !== true ||
    latestEvidence.loadLibraryAImplemented !== true ||
    latestEvidence.loadLibraryASequence !== 17_823 ||
    latestEvidence.loadLibraryAInstructionCount !== 11_806_531 ||
    latestEvidence.loadLibraryAArguments?.join(",") !== "0x100dc9c8" ||
    latestEvidence.loadLibraryAReturnAddress !== "0x1005bab0" ||
    latestEvidence.loadLibraryARequestedName !== "d3dx9_43.dll" ||
    latestEvidence.loadLibraryAResult !== "0x4d010010" ||
    latestEvidence.loadLibraryALastError !== 0 ||
    latestEvidence.loadLibraryAGuestWriteBytes !== 0 ||
    latestEvidence.loadLibraryAImplementationId !==
      "kernel32.load-library-a.runtime-system.v1" ||
    latestEvidence.loadLibraryAPolicyId !== "kernel32-d3dx-runtime-module-v1" ||
    latestEvidence.loadLibraryAModuleAuthority !==
      "runtime/modules.rs::RuntimeModules" ||
    latestEvidence.loadLibraryAObjectAuthority !==
      "runtime/objects.rs::RuntimeObjectTable" ||
    latestEvidence.loadLibraryAModuleHandleNamespace !==
      "vine.logical-loaded-module.v1" ||
    latestEvidence.loadLibraryAModuleHandle !== "0x4d010010" ||
    latestEvidence.loadLibraryAModuleId !== 16 ||
    latestEvidence.loadLibraryAModuleClass !== "runtime-system-module" ||
    latestEvidence.loadLibraryAModuleState !== "process-attached" ||
    latestEvidence.loadLibraryAAttachSequence !== 16 ||
    latestEvidence.loadLibraryARuntimeServiceReferenceCount !== 1 ||
    latestEvidence.loadLibraryADynamicThreadId !== 8_192 ||
    latestEvidence.loadLibraryADynamicThreadReferenceCount !== 1 ||
    latestEvidence.loadLibraryAObjectId !== 23 ||
    latestEvidence.loadLibraryAObjectOpenHandles !== 1 ||
    latestEvidence.loadLibraryARepeatedLoadReusesIdentity !== true ||
    latestEvidence.loadLibraryAFailureAtomicity !== true ||
    latestEvidence.loadLibraryABrowserOwnsWindowsModuleState !== false ||
    latestEvidence.loadLibraryANativeImageMapped !== false ||
    latestEvidence.loadLibraryADllMainInvoked !== false ||
    latestEvidence.loadLibraryAGeneralSearchImplemented !== false ||
    latestEvidence.loadLibraryAUnloadImplemented !== false ||
    latestEvidence.d3dxDynamicEndpointId !== 5 ||
    latestEvidence.d3dxDynamicEndpointAddress !== "0x7f020040" ||
    latestEvidence.d3dxDynamicEndpointNamespace !==
      "vine.dynamic-export-endpoint.v1" ||
    latestEvidence.d3dxDynamicEndpointModule !== "d3dx9_43.dll" ||
    latestEvidence.d3dxDynamicEndpointName !== "D3DXCreateEffect" ||
    latestEvidence.d3dxGetProcAddressSequence !== 17_824 ||
    latestEvidence.d3dxGetProcAddressInstructionCount !== 11_806_538 ||
    latestEvidence.d3dxGetProcAddressArguments?.join(",") !==
      "0x4d010010,0x100dc9d8" ||
    latestEvidence.d3dxGetProcAddressReturnAddress !== "0x1005bac5" ||
    latestEvidence.d3dxGetProcAddressResult !== "0x7f020040" ||
    latestEvidence.d3dxGetProcAddressLastError !== 0 ||
    latestEvidence.d3dxGetProcAddressGuestWriteBytes !== 0 ||
    latestEvidence.d3dxCreateEffectInvoked !== false ||
    latestEvidence.loadLibraryANodeCompletions !== 1 ||
    latestEvidence.loadLibraryAChromeCompletions !== 1 ||
    latestEvidence.loadLibraryANodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.reflectionSha256 !==
      "65f3b728ff9570a0f3081571c05996df74232711142bbfc5045d2c780dcecf49"
  ) {
    errors.push("latest runtime-system module load evidence is invalid");
  }
  for (const field of [
    "nativeGetClientRectPrimaryTraceSha256",
    "nativeGetClientRectConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeGetClientRectPrimaryTraceBytes !== 98_812 ||
    latestEvidence.nativeGetClientRectPrimaryTraceLines !== 259 ||
    latestEvidence.nativeGetClientRectPrimaryTraceSha256 !==
      "f1fc35c2afb505d39ed8514c8bf4a969d78bc013ee2fb15c890677ad8afe42a1" ||
    latestEvidence.nativeGetClientRectPrimaryBalancedApiPairs !== 99 ||
    latestEvidence.nativeGetClientRectConfirmationTraceBytes !== 98_817 ||
    latestEvidence.nativeGetClientRectConfirmationTraceLines !== 259 ||
    latestEvidence.nativeGetClientRectConfirmationTraceSha256 !==
      "151395f4a93715778a75d0262627cf63958a532cf3b26a478dc2c47ce408f05a" ||
    latestEvidence.nativeGetClientRectConfirmationBalancedApiPairs !== 99 ||
    latestEvidence.nativeGetClientRectTraceDiagnostics !== 0 ||
    latestEvidence.nativeGetClientRectCallsPerTrace !== 1 ||
    latestEvidence.nativeGetClientRectCallerRva !== "0x0005c997" ||
    latestEvidence.nativeGetClientRectResult !== "0x00000001" ||
    latestEvidence.nativeGetClientRectLastErrorBefore !== 0 ||
    latestEvidence.nativeGetClientRectLastErrorAfter !== 0 ||
    latestEvidence.nativeGetClientRectOutputRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.nativeGetClientRectSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeGetClientRectInvalidWindowResult !== "0x00000000" ||
    latestEvidence.nativeGetClientRectInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeGetClientRectInvalidWindowOutputPreserved !== true ||
    latestEvidence.nativeGetClientRectConsumer !==
      "bool-result-ignored-immediate-getdc-call" ||
    latestEvidence.nativeGetClientRectExecutableHashUnchanged !== true ||
    latestEvidence.getClientRectImplemented !== true ||
    latestEvidence.getClientRectSequence !== 17_825 ||
    latestEvidence.getClientRectInstructionCount !== 11_806_582 ||
    latestEvidence.getClientRectArguments?.join(",") !==
      "0x48010001,0x006ffdc4" ||
    latestEvidence.getClientRectReturnAddress !== "0x1005c997" ||
    latestEvidence.getClientRectResult !== "0x00000001" ||
    latestEvidence.getClientRectLastError !== 0 ||
    latestEvidence.getClientRectGuestWriteBytes !== 16 ||
    latestEvidence.getClientRectOutputHex !==
      "00000000000000002003000058020000" ||
    latestEvidence.getClientRectStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.getClientRectWindowId !== 1 ||
    latestEvidence.getClientRectWindowHandleNamespace !==
      "vine.logical-window.v1" ||
    latestEvidence.getClientRectGeometrySource !==
      "live-window-client-geometry" ||
    latestEvidence.getClientRectPreservesLastError !== true ||
    latestEvidence.getClientRectWindowStateMutation !== false ||
    latestEvidence.getClientRectFailureAtomicity !== true ||
    latestEvidence.getClientRectBrowserOwnsLogicalWindowState !== false ||
    latestEvidence.getClientRectHostGeometryImported !== false ||
    latestEvidence.getClientRectNodeCompletions !== 1 ||
    latestEvidence.getClientRectChromeCompletions !== 1 ||
    latestEvidence.getClientRectNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest GetClientRect evidence is invalid");
  }
  for (const field of [
    "nativeWindowDcPrimaryTraceSha256",
    "nativeWindowDcConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeWindowDcPrimaryTraceBytes !== 63_019 ||
    latestEvidence.nativeWindowDcPrimaryTraceLines !== 95 ||
    latestEvidence.nativeWindowDcPrimaryTraceSha256 !==
      "7a793d995671f44e083df46ce77da221600f3cce546ee21b03c3a4e163fbbdae" ||
    latestEvidence.nativeWindowDcPrimaryBalancedApiPairs !== 17 ||
    latestEvidence.nativeWindowDcConfirmationTraceBytes !== 63_231 ||
    latestEvidence.nativeWindowDcConfirmationTraceLines !== 96 ||
    latestEvidence.nativeWindowDcConfirmationTraceSha256 !==
      "9c554473a4fda0abc78f4bf4cdb65170db0ea6c3972d76b64f95b47478dd4d23" ||
    latestEvidence.nativeWindowDcConfirmationBalancedApiPairs !== 17 ||
    latestEvidence.nativeWindowDcTraceDiagnostics !== 0 ||
    latestEvidence.nativeWindowDcProfile !== "x86-bgi-window-dc-fill-v1" ||
    latestEvidence.nativeWindowDcCallerReturnRvas?.join(",") !==
      "0x0005c9a4,0x0005c9ae,0x0005c9bb,0x0005c9cb" ||
    latestEvidence.nativeWindowDcClientRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.nativeWindowDcStockObjectIndex !== 4 ||
    latestEvidence.nativeWindowDcGetDcResultNonNull !== true ||
    latestEvidence.nativeWindowDcFillRectResult !== 1 ||
    latestEvidence.nativeWindowDcReleaseDcResult !== 1 ||
    latestEvidence.nativeWindowDcSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeWindowDcConcurrentAcquisitionsDistinct !== true ||
    latestEvidence.nativeWindowDcNumericHandleReuseImported !== false ||
    latestEvidence.nativeWindowDcInvalidWindowGetDcLastError !== 1_400 ||
    latestEvidence.nativeWindowDcInvalidHdcFillRectLastError !== 6 ||
    latestEvidence.nativeWindowDcInvalidHdcReleaseDcLastError !== 1_425 ||
    latestEvidence.nativeWindowDcInvalidStockObjectResult !== "0x00000000" ||
    latestEvidence.nativeWindowDcInvalidStockObjectLastErrorPreserved !== true ||
    latestEvidence.nativeWindowDcExecutableHashUnchanged !== true ||
    latestEvidence.getDcImplemented !== true ||
    latestEvidence.getDcSequence !== 17_826 ||
    latestEvidence.getDcInstructionCount !== 11_806_586 ||
    latestEvidence.getDcArguments?.join(",") !== "0x48010001" ||
    latestEvidence.getDcReturnAddress !== "0x1005c9a4" ||
    latestEvidence.getDcResult !== "0x47010001" ||
    latestEvidence.getDcLastError !== 0 ||
    latestEvidence.getDcGuestWriteBytes !== 0 ||
    latestEvidence.getDcDeviceContextId !== 1 ||
    latestEvidence.getDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    latestEvidence.getDcSurfaceId !== 1 ||
    latestEvidence.getDcSurfaceCreated !== true ||
    latestEvidence.getStockObjectImplemented !== true ||
    latestEvidence.getStockObjectSequence !== 17_827 ||
    latestEvidence.getStockObjectInstructionCount !== 11_806_590 ||
    latestEvidence.getStockObjectArguments?.join(",") !== "0x00000004" ||
    latestEvidence.getStockObjectReturnAddress !== "0x1005c9ae" ||
    latestEvidence.getStockObjectResult !== "0x47020001" ||
    latestEvidence.getStockObjectLastError !== 0 ||
    latestEvidence.getStockObjectGuestWriteBytes !== 0 ||
    latestEvidence.getStockObjectId !== 1 ||
    latestEvidence.getStockObjectHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    latestEvidence.getStockObjectShared !== true ||
    latestEvidence.getStockObjectColorRef !== 0 ||
    latestEvidence.fillRectImplemented !== true ||
    latestEvidence.fillRectSequence !== 17_828 ||
    latestEvidence.fillRectInstructionCount !== 11_806_596 ||
    latestEvidence.fillRectArguments?.join(",") !==
      "0x47010001,0x006ffdc4,0x47020001" ||
    latestEvidence.fillRectReturnAddress !== "0x1005c9bb" ||
    latestEvidence.fillRectResult !== "0x00000001" ||
    latestEvidence.fillRectLastError !== 0 ||
    latestEvidence.fillRectGuestWriteBytes !== 0 ||
    latestEvidence.fillRectSurfaceId !== 1 ||
    latestEvidence.fillRectSurfaceGeometry?.join(",") !== "800,600" ||
    latestEvidence.fillRectSurfaceGeneration !== 1 ||
    latestEvidence.fillRectRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.fillRectColorRef !== 0 ||
    latestEvidence.releaseDcImplemented !== true ||
    latestEvidence.releaseDcSequence !== 17_829 ||
    latestEvidence.releaseDcInstructionCount !== 11_806_602 ||
    latestEvidence.releaseDcArguments?.join(",") !==
      "0x48010001,0x47010001" ||
    latestEvidence.releaseDcReturnAddress !== "0x1005c9cb" ||
    latestEvidence.releaseDcResult !== "0x00000001" ||
    latestEvidence.releaseDcLastError !== 0 ||
    latestEvidence.releaseDcGuestWriteBytes !== 0 ||
    latestEvidence.releaseDcLiveDeviceContexts !== 0 ||
    latestEvidence.releaseDcLiveSurfaces !== 1 ||
    latestEvidence.gdiStateAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.gdiWindowRelationOwnership !==
      "non-owning-revalidated-through-runtime-user32" ||
    latestEvidence.gdiMutation !== "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.gdiProcessExitTeardown !== true ||
    latestEvidence.gdiFailureAtomicity !== true ||
    latestEvidence.gdiBrowserOwnsWindowsState !== false ||
    latestEvidence.gdiRuntimeGraphicsCreated !== false ||
    latestEvidence.gdiBrowserRendererResourceCreated !== false ||
    latestEvidence.windowDcNodeCompletions !== 4 ||
    latestEvidence.windowDcChromeCompletions !== 4 ||
    latestEvidence.windowDcNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest window DC fill evidence is invalid");
  }
  for (const field of [
    "nativeSetWindowLongAPrimaryTraceSha256",
    "nativeSetWindowLongAConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeSetWindowLongAPrimaryTraceBytes !== 67_283 ||
    latestEvidence.nativeSetWindowLongAPrimaryTraceLines !== 88 ||
    latestEvidence.nativeSetWindowLongAPrimaryTraceSha256 !==
      "c7ec66fdcce747246943c4cc98b1edfebb9f5b8748ccfd023916a8d4ed1c6283" ||
    latestEvidence.nativeSetWindowLongAPrimaryBalancedApiPairs !== 6 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceBytes !== 67_579 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceLines !== 89 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceSha256 !==
      "1b7d6816599493a0db1b5b7d4ff716a5fdc6192ed96f3c2fe51856f81d0c18a6" ||
    latestEvidence.nativeSetWindowLongAConfirmationBalancedApiPairs !== 6 ||
    latestEvidence.nativeSetWindowLongATraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowLongAProfile !==
      "x86-bgi-first-gwl-style-update-v1" ||
    latestEvidence.nativeSetWindowLongAOracleProfile !==
      "x86-live-gwl-style-and-invalid-arguments-v1" ||
    latestEvidence.nativeSetWindowLongAMainCallsPerTrace !== 3 ||
    latestEvidence.nativeSetWindowLongACallerRva !== "0x0005cae3" ||
    latestEvidence.nativeSetWindowLongAStyleIndex !== -16 ||
    latestEvidence.nativeSetWindowLongARequestedStyle !== "0x90ca0000" ||
    latestEvidence.nativeSetWindowLongAInitialEffectiveStyle !==
      "0x84ca0000" ||
    latestEvidence.nativeSetWindowLongAFirstResult !== "0x84ca0000" ||
    latestEvidence.nativeSetWindowLongAFinalEffectiveStyle !==
      "0x94ca0000" ||
    latestEvidence.nativeSetWindowLongAVisibilityTransition?.join(",") !==
      "false,true" ||
    latestEvidence.nativeSetWindowLongASuccessLastErrorPreserved !== true ||
    latestEvidence.nativeSetWindowLongARepeatResult !== "0x94ca0000" ||
    latestEvidence.nativeSetWindowLongARepeatStateChanged !== false ||
    latestEvidence.nativeSetWindowLongAInvalidWindowResult !== "0x00000000" ||
    latestEvidence.nativeSetWindowLongAInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeSetWindowLongAInvalidIndex !== -100 ||
    latestEvidence.nativeSetWindowLongAInvalidIndexResult !== "0x00000000" ||
    latestEvidence.nativeSetWindowLongAInvalidIndexLastError !== 1_413 ||
    latestEvidence.nativeSetWindowLongAConsumer !==
      "return-eax-unread-before-set-window-pos-argument-setup" ||
    latestEvidence.nativeSetWindowLongAExecutableHashUnchanged !== true ||
    latestEvidence.setWindowLongAImplemented !== true ||
    latestEvidence.setWindowLongASequence !== 17_830 ||
    latestEvidence.setWindowLongAInstructionCount !== 11_806_624 ||
    latestEvidence.setWindowLongAArguments?.join(",") !==
      "0x48010001,0xfffffff0,0x90ca0000" ||
    latestEvidence.setWindowLongAReturnAddress !== "0x1005cae3" ||
    latestEvidence.setWindowLongAResult !== "0x84ca0000" ||
    latestEvidence.setWindowLongALastError !== 0 ||
    latestEvidence.setWindowLongAGuestWriteBytes !== 0 ||
    latestEvidence.setWindowLongAImplementationId !==
      "user32.set-window-long-a.gwl-style.v1" ||
    latestEvidence.setWindowLongAPolicyId !==
      "main-window-gwl-style-visible-v1" ||
    latestEvidence.setWindowLongAStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.setWindowLongAWindowId !== 1 ||
    latestEvidence.setWindowLongAWindowHandleNamespace !==
      "vine.logical-window.v1" ||
    latestEvidence.setWindowLongARequestedCreationStyle !== "0x80ca0000" ||
    latestEvidence.setWindowLongAPreviousEffectiveStyle !== "0x84ca0000" ||
    latestEvidence.setWindowLongARetainedStyleMask !== "0x04000000" ||
    latestEvidence.setWindowLongAEffectiveStyle !== "0x94ca0000" ||
    latestEvidence.setWindowLongAVisibleBefore !== false ||
    latestEvidence.setWindowLongAVisibleAfter !== true ||
    latestEvidence.setWindowLongATopmostAfter !== false ||
    latestEvidence.setWindowLongAMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.setWindowLongAIdempotentUpdatePreservesGeneration !== true ||
    latestEvidence.setWindowLongAFailureAtomicity !== true ||
    latestEvidence.setWindowLongABrowserOwnsLogicalWindowState !== false ||
    latestEvidence.setWindowLongAHostStyleImported !== false ||
    latestEvidence.setWindowLongANodeCompletions !== 1 ||
    latestEvidence.setWindowLongAChromeCompletions !== 1 ||
    latestEvidence.setWindowLongANodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest SetWindowLongA GWL_STYLE evidence is invalid");
  }
  const createWindowExAGeometry = requireRecord(
    latestEvidence.createWindowExAGeometry,
    "activeRuntimeCheckpoint.latestEvidence.createWindowExAGeometry",
    errors,
  );
  if (
    latestEvidence.nativeCreateWindowExAPrimaryTraceBytes !== 316_971 ||
    latestEvidence.nativeCreateWindowExAPrimaryTraceLines !== 793 ||
    latestEvidence.nativeCreateWindowExAConfirmationTraceBytes !== 316_932 ||
    latestEvidence.nativeCreateWindowExAConfirmationTraceLines !== 791 ||
    latestEvidence.nativeCreateWindowExAPayloadTraceBytes !== 319_849 ||
    latestEvidence.nativeCreateWindowExAPayloadTraceLines !== 794 ||
    latestEvidence.nativeInvalidMenuTraceBytes !== 330_210 ||
    latestEvidence.nativeInvalidMenuTraceLines !== 794 ||
    latestEvidence.nativeDefWindowProcTraceBytes !== 342_059 ||
    latestEvidence.nativeDefWindowProcTraceLines !== 833 ||
    latestEvidence.nativeCreateWindowExAProfile !==
      "x86-bgi-create-window-ex-a-v1" ||
    latestEvidence.nativeCreateWindowExACallerReturnRva !== "0x00067efa" ||
    latestEvidence.nativeCreateWindowExAArguments?.join(",") !==
      "0x00040000,BGI - Main window,Ethornell title,0x80ca0000,317,154,806,629,0x00000000,0x00000000,main-module,0x00000000" ||
    latestEvidence.nativeCreateWindowExAResultNonNull !== true ||
    latestEvidence.nativeCreateWindowExAConsumer !==
      "store-global-test-nonzero-and-continue-initialization" ||
    latestEvidence.nativeCreateWindowExACallbackMessages?.join(",") !==
      "129,131,1,5,3" ||
    latestEvidence.nativeCreateWindowExACallbackResults?.join(",") !==
      "1,0,0,0,0" ||
    latestEvidence.nativeCreateWindowExANonClientRectBefore?.join(",") !==
      "317,154,1123,783" ||
    latestEvidence.nativeCreateWindowExANonClientRectAfter?.join(",") !==
      "320,180,1120,780" ||
    latestEvidence.nativeCreateWindowExASizeLparam !== "0x02580320" ||
    latestEvidence.nativeCreateWindowExAMoveLparam !== "0x00b40140" ||
    latestEvidence.nativeDefWindowProcCallerRva !== "0x000933bf" ||
    latestEvidence.nativeDefWindowProcCreationMessages?.join(",") !==
      "129,131,5,3" ||
    latestEvidence.nativeDefWindowProcCreationResults?.join(",") !==
      "1,0,0,0" ||
    latestEvidence.nativeGetSystemMenuArguments?.join(",") !==
      "0x00000000,0x00000000" ||
    latestEvidence.nativeGetSystemMenuResult !== "0x00000000" ||
    latestEvidence.nativeGetSystemMenuLastError !== 1_400 ||
    latestEvidence.nativeEnableMenuItemArguments?.join(",") !==
      "0x00000000,0x0000f060,0x00000000" ||
    latestEvidence.nativeEnableMenuItemResult !== "0xffffffff" ||
    latestEvidence.nativeEnableMenuItemLastError !== 1_401 ||
    latestEvidence.nativeInvalidMenuConsumer !==
      "result-ignored-global-flag-set-and-continue" ||
    latestEvidence.nativeCreateWindowExAExecutableHashUnchanged !== true ||
    latestEvidence.implementation !==
      "bounded D3D9 managed vertex-buffer creation and single-lock lifecycle with atomic COM, address-space, graphics, and guest-memory publication" ||
    latestEvidence.createWindowExAImplemented !== true ||
    latestEvidence.createWindowExASequence !== 1_388 ||
    latestEvidence.createWindowExAInstructionCount !== 11_476_236 ||
    latestEvidence.createWindowExAArguments?.join(",") !==
      "0x00040000,0x100fba4c,0x006ffb48,0x80ca0000,317,154,806,629,0x00000000,0x00000000,0x10000000,0x00000000" ||
    latestEvidence.createWindowExAReturnAddress !== "0x10067efa" ||
    latestEvidence.createWindowExAResult !== "0x48010001" ||
    latestEvidence.createWindowExALastError !== 0 ||
    latestEvidence.createWindowExAGuestWrites !== 0 ||
    latestEvidence.createWindowExAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.createWindowExAWindowId !== 1 ||
    latestEvidence.createWindowExAHandleNamespace !== "vine.logical-window.v1" ||
    latestEvidence.createWindowExAOwnerThreadId !== 8_192 ||
    latestEvidence.createWindowExAClassId !== 1 ||
    latestEvidence.createWindowExAClassAtom !== "0x0000c000" ||
    latestEvidence.createWindowExANameEncoding !== "windows-932-v1" ||
    latestEvidence.createWindowExAEffectiveExtendedStyle !== "0x00040100" ||
    latestEvidence.createWindowExARequestedStyle !== "0x80ca0000" ||
    latestEvidence.createWindowExAEffectiveStyle !== "0x84ca0000" ||
    createWindowExAGeometry.x !== 317 ||
    createWindowExAGeometry.y !== 154 ||
    createWindowExAGeometry.width !== 806 ||
    createWindowExAGeometry.height !== 629 ||
    createWindowExAGeometry.clientX !== 320 ||
    createWindowExAGeometry.clientY !== 180 ||
    createWindowExAGeometry.clientWidth !== 800 ||
    createWindowExAGeometry.clientHeight !== 600 ||
    latestEvidence.createWindowExAFinalLifecycle !== "created" ||
    latestEvidence.createWindowExAVisible !== false ||
    latestEvidence.createWindowExACallbackFrameIds?.join(",") !== "2,3,4,5,6" ||
    latestEvidence.createWindowExACallbackMessages?.join(",") !==
      "129,131,1,5,3" ||
    latestEvidence.createWindowExACallbackResults?.join(",") !== "1,0,0,0,0" ||
    latestEvidence.createWindowExACallbackReturnInstructionCounts?.join(",") !==
      "11476289,11476342,11476388,11476458,11476511" ||
    latestEvidence.createWindowExACallbackDepthAfterReturn !== 0 ||
    latestEvidence.callbackScratchArenaBase !== "0x00501000" ||
    latestEvidence.callbackScratchArenaBytes !== 65_536 ||
    latestEvidence.callbackScratchSlotBytes !== 4_096 ||
    latestEvidence.callbackScratchAddressAllocationPurpose !== "callback-scratch" ||
    latestEvidence.callbackScratchSeparatedFromDescendingStack !== true ||
    latestEvidence.defWindowProcAImplemented !== true ||
    latestEvidence.defWindowProcACallCount !== 16 ||
    latestEvidence.defWindowProcASequences?.join(",") !==
      "1389,1390,1393,1394,1399,1403,1404,1409,17832,17833,17834,17835,17836,17837,17838,17839" ||
    latestEvidence.defWindowProcAMessages?.join(",") !==
      "129,131,5,3,128,71,3,71,70,70,28,134,6,641,642,7" ||
    latestEvidence.defWindowProcAResults?.join(",") !==
      "1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0" ||
    latestEvidence.defWindowProcALastErrors?.join(",") !==
      "0,0,1401,1401,0,0,0,0,0,0,0,0,0,0,0,0" ||
    latestEvidence.defWindowProcANonClientGuestWriteBytes !== 16 ||
    latestEvidence.defWindowProcAScalarLparamsDereferenced !== false ||
    latestEvidence.getSystemMenuImplemented !== true ||
    latestEvidence.getSystemMenuSequence !== 1_391 ||
    latestEvidence.getSystemMenuResult !== "0x00000000" ||
    latestEvidence.getSystemMenuLastError !== 1_400 ||
    latestEvidence.enableMenuItemImplemented !== true ||
    latestEvidence.enableMenuItemSequence !== 1_392 ||
    latestEvidence.enableMenuItemResult !== "0xffffffff" ||
    latestEvidence.enableMenuItemLastError !== 1_401 ||
    latestEvidence.generalMenuIdentityAllocated !== false ||
    latestEvidence.windowCreationFailureAtomicity !== true ||
    latestEvidence.windowCallbackReplacementFailureAtomicity !== true ||
    latestEvidence.browserOwnsLogicalWindowState !== false
  ) {
    errors.push("latest logical window creation evidence is invalid");
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
  const currentStop = root.nextStructuredStop ?? {};
  const currentStopEndpoint =
    currentStop.kind === "com-method"
      ? `${currentStop.interface}::${currentStop.method}`
      : currentStop.api;
  if (
    active.canonicalReproductionCommand !== "npm run check:runtime:bgi-browser" ||
    bgiBrowserEvidence.supportedCalls !== active.genericDispatcherCallOccurrenceCount ||
    bgiBrowserEvidence.guestInstructions !== currentStop.instructionCount ||
    bgiBrowserEvidence.nodeFinalGeneration !== latestEvidence.finalMemoryGeneration ||
    bgiBrowserEvidence.memoryContentSha256 !== latestEvidence.memoryContentSha256 ||
    bgiBrowserEvidence.nodeOracleReportBytes !== latestEvidence.reportBytes ||
    bgiBrowserEvidence.nodeOracleReportSha256 !== latestEvidence.reportSha256 ||
    bgiBrowserEvidence.nextCapabilityCluster !== currentStop.capabilityCluster ||
    bgiBrowserEvidence.nextEndpoint?.toLowerCase() !== currentStopEndpoint?.toLowerCase() ||
    bgiBrowserEvidence.nextFaultCode !== currentStop.reason ||
    bgiBrowserEvidence.nextEndpointAddress !== currentStop.endpointAddress ||
    bgiBrowserEvidence.nextArguments?.join(",") !==
      currentStop.arguments?.join(",") ||
    latestEvidence.firstStructuredStopInstructionCount !==
      currentStop.instructionCount ||
    latestEvidence.firstStructuredStopCapabilityCluster !==
      currentStop.capabilityCluster ||
    latestEvidence.firstStructuredStopEndpoint?.toLowerCase() !==
      currentStopEndpoint?.toLowerCase() ||
    latestEvidence.firstStructuredStopEndpointKind !== currentStop.endpointKind ||
    latestEvidence.firstStructuredStopEndpointAddress !==
      currentStop.endpointAddress ||
    latestEvidence.firstStructuredStopArguments?.join(",") !==
      currentStop.arguments?.join(",") ||
    latestEvidence.firstStructuredStopReturnAddress !== currentStop.returnAddress ||
    latestEvidence.firstStructuredStopFault !== currentStop.reason
  ) {
    errors.push("browser BGI evidence must match the active runtime checkpoint");
  }
  if (
    currentStop.kind === "api-endpoint" &&
    (latestEvidence.firstStructuredStopEndpointId !== currentStop.endpointId ||
      latestEvidence.firstStructuredStopIatAddress !== currentStop.iatAddress)
  ) {
    errors.push("browser BGI API stop identity must match the active runtime checkpoint");
  }
  if (
    currentStop.kind === "com-method" &&
    (latestEvidence.firstStructuredStopInterface !== currentStop.interface ||
      latestEvidence.firstStructuredStopVtableSlot !== currentStop.vtableSlot ||
      latestEvidence.firstStructuredStopMethod !== currentStop.method ||
      latestEvidence.firstStructuredStopThisPointer !== currentStop.thisPointer)
  ) {
    errors.push("browser BGI COM stop identity must match the active runtime checkpoint");
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
  } else if (next.kind === "com-method") {
    requireString(next.endpointKind, "nextStructuredStop.endpointKind", errors);
    requireString(next.interface, "nextStructuredStop.interface", errors);
    requireInteger(next.vtableSlot, "nextStructuredStop.vtableSlot", errors);
    requireString(next.method, "nextStructuredStop.method", errors);
    requireOrderedStringArray(next.arguments, "nextStructuredStop.arguments", errors);
    for (const field of ["thisPointer", "endpointAddress", "returnAddress"]) {
      if (typeof next[field] !== "string" || !HEX32.test(next[field])) {
        errors.push(`nextStructuredStop.${field} must be a 32-bit hex string`);
      }
    }
    if (next.endpointKind !== "com-method" || next.vtableSlot < 0) {
      errors.push("nextStructuredStop COM method identity is inconsistent");
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
    errors.push(
      "nextStructuredStop.kind must be api-endpoint, com-method, or execution-fault",
    );
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
  const scenario = manifest.scenarioMilestones;
  const historical = manifest.historicalBaseline;
  const active = manifest.activeRuntimeCheckpoint;
  const kernelBoundary = active.runtimeKernelBoundary;
  const latest = active.latestEvidence;
  const browser = manifest.browserCheckpoint;
  const engine = manifest.engineDecisionEvidence.v86EngineGate;
  const next = manifest.nextStructuredStop;
  const APIs = active.genericDispatcherApis.map((api) => `- \`${api}\``).join("\n");
  const scenarioRows = scenario.milestones
    .map(
      ({ id, label, status, evidenceBoundary }) =>
        `| \`${id}\` | ${label} | \`${status}\` | ${evidenceBoundary} |`,
    )
    .join("\n");
  const nextSummary =
    next.kind === "execution-fault"
      ? `\`${next.capabilityCluster}\` at instruction ${next.instructionCount}: ` +
        `\`${next.reason}\` at \`${next.fault.faultAddress}\``
      : next.kind === "com-method"
        ? `\`${next.interface}::${next.method}(${next.arguments.join(", ")})\` at instruction ` +
          `${next.instructionCount}: \`${next.reason}\``
      : `\`${next.api}(${next.arguments.join(", ")})\` at instruction ` +
        `${next.instructionCount}: \`${next.reason}\``;
  const blockerSummary = browser.bgiBenchmarkBlocker
    ? `| Active blocker | \`${browser.bgiBenchmarkBlocker.id}\`: \`${browser.bgiBenchmarkBlocker.firstFailure.thresholdId}\`; ${browser.bgiBenchmarkBlocker.resumeCondition} |\n`
    : "";

  return `${README_STATUS_START}
## Current Status

This section is generated from [\`docs/current-checkpoint.json\`](docs/current-checkpoint.json).
Edit the manifest and run \`npm run docs:generate\`; do not edit this block by hand.

| Area | Current state |
|---|---|
| Historical baseline | \`${historical.checkpoint}\`: ${historical.supportedCallCount} calls, ${historical.entryInstructionCount} instructions |
| Active scenario | \`${scenario.activeMilestone}\` |
| Runtime diagnostics | \`${active.runtimeKernelCheckpoint}\`: ${active.genericDispatcherCallOccurrenceCount} calls across ${active.genericDispatcherApiCount} API contracts, memory generation ${latest.finalMemoryGeneration} |
| Runtime architecture | \`${kernelBoundary.checkpoint}\`: \`${kernelBoundary.aggregateType}\`; \`${kernelBoundary.compatibilityAlias}\` remains a compatibility alias |
| Browser | \`${browser.workload}\` ${browser.status}; BGI session \`${browser.bgiSession}\`; benchmark \`${browser.bgiBenchmark}\` (${browser.bgiBenchmarkEvidence.completedSessions} sessions) |
${blockerSummary}| Engine evidence | v86 \`${engine.decision}\` from \`${engine.configuration}\` |
| Next structured stop | ${nextSummary} |

The active semantic report is ${latest.reportBytes.toLocaleString("en-US")} bytes with SHA-256
\`${latest.reportSha256}\`. The hash identifies the artifact; semantic checks are the gate.

### Scenario Milestones

| ID | Milestone | Status | Evidence boundary |
|---|---|---|---|
${scenarioRows}

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
