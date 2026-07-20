import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  README_STATUS_END,
  README_STATUS_START,
  loadCurrentCheckpoint,
  renderReadmeStatus,
  replaceReadmeStatus,
  validateCurrentCheckpoint,
} from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
assert.deepEqual(validateCurrentCheckpoint(manifest), []);

const rendered = renderReadmeStatus(manifest);
assert.ok(rendered.startsWith(README_STATUS_START));
assert.ok(rendered.endsWith(README_STATUS_END));
assert.ok(rendered.includes(manifest.activeRuntimeCheckpoint.runtimeKernelCheckpoint));
assert.ok(
  rendered.includes(manifest.activeRuntimeCheckpoint.runtimeKernelBoundary.checkpoint),
);
assert.ok(
  rendered.includes(manifest.activeRuntimeCheckpoint.runtimeKernelBoundary.aggregateType),
);
assert.ok(rendered.includes(manifest.nextStructuredStop.reason));
assert.ok(rendered.includes(manifest.browserCheckpoint.bgiBenchmark));
assert.equal(manifest.browserCheckpoint.bgiBenchmarkBlocker, undefined);
assert.ok(!rendered.includes("| Active blocker |"));
for (const milestone of manifest.scenarioMilestones.milestones) {
  assert.ok(rendered.includes(`\`${milestone.id}\``));
  assert.ok(rendered.includes(`\`${milestone.status}\``));
}
for (const api of manifest.activeRuntimeCheckpoint.genericDispatcherApis) {
  assert.ok(rendered.includes(`\`${api}\``));
}

const stale = structuredClone(manifest);
stale.activeRuntimeCheckpoint.genericDispatcherApiCount += 1;
assert.ok(
  validateCurrentCheckpoint(stale).some((error) => error.includes("must match")),
);

const invalidScenarioStatus = structuredClone(manifest);
invalidScenarioStatus.scenarioMilestones.milestones[0].status = "complete";
assert.ok(
  validateCurrentCheckpoint(invalidScenarioStatus).some((error) =>
    error.includes("scenarioMilestones.milestones[0].status"),
  ),
);

const duplicateScenario = structuredClone(manifest);
duplicateScenario.scenarioMilestones.milestones[1].id =
  duplicateScenario.scenarioMilestones.milestones[0].id;
assert.ok(
  validateCurrentCheckpoint(duplicateScenario).some((error) =>
    error.includes("canonical scenario order"),
  ),
);

const missingLatestEvidence = structuredClone(manifest);
delete missingLatestEvidence.activeRuntimeCheckpoint.latestEvidence;
assert.ok(
  validateCurrentCheckpoint(missingLatestEvidence).some((error) =>
    error.includes("latestEvidence"),
  ),
);

const bypassedScheduler = structuredClone(manifest);
bypassedScheduler.browserCheckpoint.browserRuntimeEvidence.rustSchedulerTransitions = 1;
assert.ok(
  validateCurrentCheckpoint(bypassedScheduler).some((error) =>
    error.includes("scheduler transitions"),
  ),
);

const missingGuestThreadExecution = structuredClone(manifest);
missingGuestThreadExecution.browserCheckpoint.browserRuntimeEvidence.executedThreadIds = [8192];
assert.ok(
  validateCurrentCheckpoint(missingGuestThreadExecution).some((error) =>
    error.includes("multi-thread execution evidence"),
  ),
);

const missingBrowserRelease = structuredClone(manifest);
missingBrowserRelease.browserCheckpoint.browserRuntimeEvidence.mappingOperations.release = 0;
assert.ok(
  validateCurrentCheckpoint(missingBrowserRelease).some((error) =>
    error.includes("mapping-operation evidence"),
  ),
);

const sourceAdapterLosesTitleIdentity = structuredClone(manifest);
sourceAdapterLosesTitleIdentity.browserCheckpoint.browserSourceReadEvidence.titleSnapshotObjectHttpIdentity =
  false;
assert.ok(
  validateCurrentCheckpoint(sourceAdapterLosesTitleIdentity).some((error) =>
    error.includes("immutable source-read evidence"),
  ),
);

const fixedExecutionMirrorPool = structuredClone(manifest);
fixedExecutionMirrorPool.memoryAuthorityEvidence.executionMirrorCapacity.growthPolicy =
  "fixed-two-mebibytes";
assert.ok(
  validateCurrentCheckpoint(fixedExecutionMirrorPool).some((error) =>
    error.includes("execution mirror capacity contract"),
  ),
);

const browserAuthorityMovedToNode = structuredClone(manifest);
browserAuthorityMovedToNode.browserCheckpoint.bgiRuntimeEvidence.runtimeStateAuthority =
  "node";
assert.ok(
  validateCurrentCheckpoint(browserAuthorityMovedToNode).some((error) =>
    error.includes("authority or teardown"),
  ),
);

const mismatchedBrowserCalls = structuredClone(manifest);
mismatchedBrowserCalls.browserCheckpoint.bgiRuntimeEvidence.supportedCalls -= 1;
assert.ok(
  validateCurrentCheckpoint(mismatchedBrowserCalls).some((error) =>
    error.includes("active runtime checkpoint"),
  ),
);

const incompleteEndpointIdentity = structuredClone(manifest);
delete incompleteEndpointIdentity.nextStructuredStop.endpointKind;
assert.ok(
  validateCurrentCheckpoint(incompleteEndpointIdentity).some((error) =>
    error.includes("nextStructuredStop.endpointKind"),
  ),
);

const failedBgiBenchmark = structuredClone(manifest);
failedBgiBenchmark.browserCheckpoint.bgiBenchmarkEvidence.thresholdFailures = 2;
assert.ok(
  validateCurrentCheckpoint(failedBgiBenchmark).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const structuredBenchmarkBlocker = structuredClone(manifest);
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmark = "threshold-blocked";
Object.assign(structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence, {
  decisionStatus: "failed",
  decisionReason: "1-of-17-versioned-thresholds-failed",
  completedSessions: 26,
  sessionP95Milliseconds: 8437.465000033379,
  thresholdFailures: 1,
  thresholdFailureIds: ["maximum-p95-session-milliseconds"],
  reportBytes: 930_082,
  reportSha256: "7b599da1fb0fa2e75234553cf69e16e740f6388522d97c1438bd9fe2c8bcf742",
});
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkBlocker = {
  schema: "vine.structured-blocker",
  schemaVersion: 1,
  id: "d3d9-create-texture-performance-oracle-host-contention-v1",
  kind: "feasibility-blocker",
  reason: "repeatable-performance-oracle-unavailable-with-current-host-load",
  status: "blocked",
  blockedWork: "advance-from-create-texture-to-get-surface-level",
  exactReproduction: "npm run check:runtime:bgi-browser-benchmark:prepared",
  firstFailure: {
    thresholdId: "maximum-p95-session-milliseconds",
    observed: 8437.465000033379,
    comparison: "<=",
    threshold: 8_000,
  },
  evidence: {
    benchmarkReport: ".work/browser-runtime/bgi-browser-benchmark-report.json",
    benchmarkReportBytes: 930_082,
    benchmarkReportSha256:
      "7b599da1fb0fa2e75234553cf69e16e740f6388522d97c1438bd9fe2c8bcf742",
    passingThresholds: 16,
    thresholdEvaluations: 17,
    completedSessions: 26,
    failedSessions: 0,
    nodeComparisonReport: ".work/hyperfine-d3d9-create-texture-v1.json",
    nodeComparisonReportBytes: 1_082,
    nodeComparisonReportSha256:
      "dc3a36b16d9b0a31252be5e99a8cc79887eb1020a8d17282048fc12d971ccb67",
    createDeviceMedianMilliseconds: 36_326.809418,
    createTextureMedianMilliseconds: 36_623.290318,
    nodeMedianIncreasePercent: 0.816149022581365,
    ambientCpuConsumers: [
      { processName: "MuMuVMMHeadless", observedLogicalCpuPercent: 234 },
      { processName: "MaaGakumasu", observedLogicalCpuPercent: 123 },
    ],
  },
  attemptedOptions: [
    "same-host-node-create-device-create-texture-comparison",
    "ambient-process-cpu-attribution",
    "preserve-versioned-duration-and-thresholds-without-retry-acceptance",
  ],
  protectedInvariants: [
    "do-not-relax-versioned-performance-thresholds",
    "do-not-accept-a-retry-as-performance-evidence",
    "do-not-stop-unrelated-user-processes-without-authorization",
  ],
  requiredAuthority: "user-controlled-external-process-lifecycle-or-isolated-host",
  resumeCondition:
    "Pause the external CPU workloads or use an isolated host, then rerun with unchanged thresholds before GetSurfaceLevel.",
};
assert.deepEqual(validateCurrentCheckpoint(structuredBenchmarkBlocker), []);

const missingStructuredBenchmarkBlocker = structuredClone(structuredBenchmarkBlocker);
delete missingStructuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkBlocker;
assert.ok(
  validateCurrentCheckpoint(missingStructuredBenchmarkBlocker).some((error) =>
    error.includes("structured performance blocker"),
  ),
);

const mismatchedStructuredBenchmarkBlocker = structuredClone(structuredBenchmarkBlocker);
mismatchedStructuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkBlocker.firstFailure.observed =
  8_000;
assert.ok(
  validateCurrentCheckpoint(mismatchedStructuredBenchmarkBlocker).some((error) =>
    error.includes("structured performance blocker"),
  ),
);

const mismatchedBenchmarkProfile = structuredClone(manifest);
mismatchedBenchmarkProfile.browserCheckpoint.bgiBenchmarkEvidence.runtimeProfile =
  "user32-get-system-metrics";
assert.ok(
  validateCurrentCheckpoint(mismatchedBenchmarkProfile).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const mismatchedRuntimeCoreArtifact = structuredClone(manifest);
mismatchedRuntimeCoreArtifact.browserCheckpoint.bgiBenchmarkEvidence.runtimeCoreWasmBytes += 1;
assert.ok(
  validateCurrentCheckpoint(mismatchedRuntimeCoreArtifact).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const staleRetainedBenchmark = structuredClone(manifest);
staleRetainedBenchmark.browserCheckpoint.bgiBenchmarkEvidence.observationStatus =
  "retained-semantic-leaf-not-rerun";
assert.ok(
  validateCurrentCheckpoint(staleRetainedBenchmark).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const invalidNativeMonitorFallback = structuredClone(manifest);
invalidNativeMonitorFallback.activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowInvalidWindowLastError =
  0;
assert.ok(
  validateCurrentCheckpoint(invalidNativeMonitorFallback).some((error) =>
    error.includes("latest MonitorFromWindow evidence"),
  ),
);

const invalidAdapterCountPolicy = structuredClone(manifest);
invalidAdapterCountPolicy.activeRuntimeCheckpoint.d3d9Frontend.getAdapterCount.adapterCount =
  2;
assert.ok(
  validateCurrentCheckpoint(invalidAdapterCountPolicy).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const browserOwnedAdapterCount = structuredClone(manifest);
browserOwnedAdapterCount.activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterCountBrowserGpuTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedAdapterCount).some((error) =>
    error.includes("latest D3D9 adapter-count evidence"),
  ),
);

const duplicatedAdapterMonitorIdentity = structuredClone(manifest);
duplicatedAdapterMonitorIdentity.activeRuntimeCheckpoint.d3d9Frontend.getAdapterMonitor.monitorIdentityAuthority =
  "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy";
assert.ok(
  validateCurrentCheckpoint(duplicatedAdapterMonitorIdentity).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const browserOwnedAdapterMonitor = structuredClone(manifest);
browserOwnedAdapterMonitor.activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterMonitorBrowserTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedAdapterMonitor).some((error) =>
    error.includes("latest D3D9 adapter-monitor evidence"),
  ),
);

const duplicatedGraphicsAuthority = structuredClone(manifest);
duplicatedGraphicsAuthority.activeRuntimeCheckpoint.d3d9Frontend.runtimeGraphics.stateOwner =
  "runtime/com.rs::RuntimeCom";
assert.ok(
  validateCurrentCheckpoint(duplicatedGraphicsAuthority).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const browserOwnedCreateDeviceState = structuredClone(manifest);
browserOwnedCreateDeviceState.activeRuntimeCheckpoint.latestEvidence.d3d9CreateDeviceBrowserOwnsComOrGraphicsState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedCreateDeviceState).some((error) =>
    error.includes("latest D3D9 CreateDevice evidence"),
  ),
);

const partialCreateDevicePublication = structuredClone(manifest);
partialCreateDevicePublication.activeRuntimeCheckpoint.latestEvidence.d3d9CreateDeviceGuestComGraphicsAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialCreateDevicePublication).some((error) =>
    error.includes("latest D3D9 CreateDevice evidence"),
  ),
);

const duplicatedTextureAuthority = structuredClone(manifest);
duplicatedTextureAuthority.activeRuntimeCheckpoint.d3d9Frontend.runtimeGraphics.textureDeviceRelation =
  "browser-owned-device-relation";
assert.ok(
  validateCurrentCheckpoint(duplicatedTextureAuthority).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const browserOwnedCreateTextureState = structuredClone(manifest);
browserOwnedCreateTextureState.activeRuntimeCheckpoint.latestEvidence.d3d9CreateTextureBrowserOwnsComOrGraphicsState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedCreateTextureState).some((error) =>
    error.includes("latest D3D9 CreateTexture evidence"),
  ),
);

const partialCreateTexturePublication = structuredClone(manifest);
partialCreateTexturePublication.activeRuntimeCheckpoint.latestEvidence.d3d9CreateTextureGuestComGraphicsAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialCreateTexturePublication).some((error) =>
    error.includes("latest D3D9 CreateTexture evidence"),
  ),
);

const aggregateMutatesSubsystemCollection = structuredClone(manifest);
aggregateMutatesSubsystemCollection.activeRuntimeCheckpoint.runtimeKernelBoundary.directSubsystemCollectionMutation =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesSubsystemCollection).some((error) =>
    error.includes("kernel responsibility boundary"),
  ),
);

const exposedProcessCollections = structuredClone(manifest);
exposedProcessCollections.activeRuntimeCheckpoint.runtimeKernelBoundary.processMutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedProcessCollections).some((error) =>
    error.includes("kernel responsibility boundary"),
  ),
);

const incompleteRepresentativeAtomicCoverage = structuredClone(manifest);
incompleteRepresentativeAtomicCoverage.activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests.pop();
assert.ok(
  validateCurrentCheckpoint(incompleteRepresentativeAtomicCoverage).some((error) =>
    error.includes("kernel responsibility boundary"),
  ),
);

const exposedMessages = structuredClone(manifest);
exposedMessages.activeRuntimeCheckpoint.runtimeMessageQueue.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedMessages).some((error) =>
    error.includes("message subsystem boundary"),
  ),
);

const exposedObjects = structuredClone(manifest);
exposedObjects.activeRuntimeCheckpoint.runtimeObjectModel.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedObjects).some((error) =>
    error.includes("object subsystem boundary"),
  ),
);

const missingWaitableTimerObjectKind = structuredClone(manifest);
missingWaitableTimerObjectKind.activeRuntimeCheckpoint.runtimeObjectModel.objectKinds =
  missingWaitableTimerObjectKind.activeRuntimeCheckpoint.runtimeObjectModel.objectKinds.filter(
    (kind) => kind !== "waitable-timer",
  );
assert.ok(
  validateCurrentCheckpoint(missingWaitableTimerObjectKind).some((error) =>
    error.includes("object subsystem boundary"),
  ),
);

const missingSourceFileObjectKind = structuredClone(manifest);
missingSourceFileObjectKind.activeRuntimeCheckpoint.runtimeObjectModel.objectKinds =
  missingSourceFileObjectKind.activeRuntimeCheckpoint.runtimeObjectModel.objectKinds.filter(
    (kind) => kind !== "file",
  );
assert.ok(
  validateCurrentCheckpoint(missingSourceFileObjectKind).some((error) =>
    error.includes("object subsystem boundary"),
  ),
);

const mutexOnlyObjectPlan = structuredClone(manifest);
mutexOnlyObjectPlan.activeRuntimeCheckpoint.runtimeKernelBoundary.closeHandleObjectPlan =
  "runtime/objects.rs::RuntimeMutexObjectMutationPlan";
assert.ok(
  validateCurrentCheckpoint(mutexOnlyObjectPlan).some((error) =>
    error.includes("kernel responsibility boundary"),
  ),
);

const browserOwnedComIdentity = structuredClone(manifest);
browserOwnedComIdentity.activeRuntimeCheckpoint.runtimeComFoundation.logicalIdentityAuthority =
  "browser";
assert.ok(
  validateCurrentCheckpoint(browserOwnedComIdentity).some((error) =>
    error.includes("COM foundation boundary"),
  ),
);

const unsafeNumericComIdentity = structuredClone(manifest);
unsafeNumericComIdentity.activeRuntimeCheckpoint.runtimeComFoundation.logicalIdentitySerialization =
  "json-number";
assert.ok(
  validateCurrentCheckpoint(unsafeNumericComIdentity).some((error) =>
    error.includes("COM foundation boundary"),
  ),
);

const postMutationComCompletionValidation = structuredClone(manifest);
postMutationComCompletionValidation.activeRuntimeCheckpoint.runtimeComFoundation.completionPreflightBeforeMutation =
  false;
assert.ok(
  validateCurrentCheckpoint(postMutationComCompletionValidation).some((error) =>
    error.includes("COM foundation boundary"),
  ),
);

const browserOwnsComReference = structuredClone(manifest);
browserOwnsComReference.activeRuntimeCheckpoint.runtimeComFoundation.browserAssociationOwnsComReference =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnsComReference).some((error) =>
    error.includes("COM foundation boundary"),
  ),
);

const browserOwnsComApartment = structuredClone(manifest);
browserOwnsComApartment.activeRuntimeCheckpoint.runtimeComFoundation.apartmentStateOwner =
  "browser/runtime-worker/runtime-worker.ts";
assert.ok(
  validateCurrentCheckpoint(browserOwnsComApartment).some((error) =>
    error.includes("COM foundation boundary"),
  ),
);

const apartmentLeaksOnThreadExit = structuredClone(manifest);
apartmentLeaksOnThreadExit.activeRuntimeCheckpoint.runtimeThreadRegistry.comApartmentThreadExitCleanup =
  false;
assert.ok(
  validateCurrentCheckpoint(apartmentLeaksOnThreadExit).some((error) =>
    error.includes("thread subsystem boundary"),
  ),
);

const exposedRegistries = structuredClone(manifest);
exposedRegistries.activeRuntimeCheckpoint.runtimeRegistries.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedRegistries).some((error) =>
    error.includes("registry subsystem boundary"),
  ),
);

const aggregateMutatesRegistries = structuredClone(manifest);
aggregateMutatesRegistries.activeRuntimeCheckpoint.runtimeRegistries.directCollectionMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesRegistries).some((error) =>
    error.includes("registry subsystem boundary"),
  ),
);

const splitEncodedPointerIndexes = structuredClone(manifest);
splitEncodedPointerIndexes.activeRuntimeCheckpoint.runtimeRegistries.encodedPointerIndexAtomic =
  false;
assert.ok(
  validateCurrentCheckpoint(splitEncodedPointerIndexes).some((error) =>
    error.includes("registry subsystem boundary"),
  ),
);

const aggregateMutatesModules = structuredClone(manifest);
aggregateMutatesModules.activeRuntimeCheckpoint.runtimeModules.directCollectionMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesModules).some((error) =>
    error.includes("module subsystem boundary"),
  ),
);

const aggregateMutatesDllLifecycle = structuredClone(manifest);
aggregateMutatesDllLifecycle.activeRuntimeCheckpoint.runtimeModules.directLifecycleMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesDllLifecycle).some((error) =>
    error.includes("module subsystem boundary"),
  ),
);

const splitDllCallbackCommit = structuredClone(manifest);
splitDllCallbackCommit.activeRuntimeCheckpoint.runtimeModules.callbackReplacementAtomic = false;
assert.ok(
  validateCurrentCheckpoint(splitDllCallbackCommit).some((error) =>
    error.includes("module subsystem boundary"),
  ),
);

const exposedThreads = structuredClone(manifest);
exposedThreads.activeRuntimeCheckpoint.runtimeThreadRegistry.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedThreads).some((error) =>
    error.includes("thread subsystem boundary"),
  ),
);

const aggregateMutatesThreadResources = structuredClone(manifest);
aggregateMutatesThreadResources.activeRuntimeCheckpoint.runtimeThreadRegistry.directResourceMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesThreadResources).some((error) =>
    error.includes("thread subsystem boundary"),
  ),
);

const splitCallbackAllocator = structuredClone(manifest);
splitCallbackAllocator.activeRuntimeCheckpoint.runtimeThreadRegistry.callbackAllocatorAtomic =
  false;
assert.ok(
  validateCurrentCheckpoint(splitCallbackAllocator).some((error) =>
    error.includes("thread subsystem boundary"),
  ),
);

const exposedScheduler = structuredClone(manifest);
exposedScheduler.activeRuntimeCheckpoint.runtimeScheduler.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedScheduler).some((error) =>
    error.includes("scheduler subsystem boundary"),
  ),
);

const aggregateMutatesHostRequests = structuredClone(manifest);
aggregateMutatesHostRequests.activeRuntimeCheckpoint.runtimeScheduler.directHostRequestMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesHostRequests).some((error) =>
    error.includes("scheduler subsystem boundary"),
  ),
);

const splitHostRequestCancellation = structuredClone(manifest);
splitHostRequestCancellation.activeRuntimeCheckpoint.runtimeScheduler.hostRequestCancellationPlan =
  false;
assert.ok(
  validateCurrentCheckpoint(splitHostRequestCancellation).some((error) =>
    error.includes("scheduler subsystem boundary"),
  ),
);

const incompleteWaitValidation = structuredClone(manifest);
incompleteWaitValidation.activeRuntimeCheckpoint.runtimeWaitQueue.excludedWakeCandidatesValidated =
  false;
assert.ok(
  validateCurrentCheckpoint(incompleteWaitValidation).some((error) =>
    error.includes("wait subsystem boundary"),
  ),
);

const aggregateMutatesWaitAllocator = structuredClone(manifest);
aggregateMutatesWaitAllocator.activeRuntimeCheckpoint.runtimeWaitQueue.directAllocatorMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesWaitAllocator).some((error) =>
    error.includes("wait subsystem boundary"),
  ),
);

const splitWaitAllocatorCommit = structuredClone(manifest);
splitWaitAllocatorCommit.activeRuntimeCheckpoint.runtimeWaitQueue.allocatorOverflowMutationFree =
  false;
assert.ok(
  validateCurrentCheckpoint(splitWaitAllocatorCommit).some((error) =>
    error.includes("wait subsystem boundary"),
  ),
);

const exposedSynchronization = structuredClone(manifest);
exposedSynchronization.activeRuntimeCheckpoint.runtimeSynchronization.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedSynchronization).some((error) =>
    error.includes("synchronization subsystem boundary"),
  ),
);

const splitHeapOwnership = structuredClone(manifest);
splitHeapOwnership.activeRuntimeCheckpoint.runtimeHeapBacking.logicalAndBackingCollectionsCoOwned =
  false;
assert.ok(
  validateCurrentCheckpoint(splitHeapOwnership).some((error) =>
    error.includes("heap subsystem boundary"),
  ),
);

const aggregateMutatesHeapAllocator = structuredClone(manifest);
aggregateMutatesHeapAllocator.activeRuntimeCheckpoint.runtimeHeapBacking.directAllocatorMutationFromAggregate =
  true;
assert.ok(
  validateCurrentCheckpoint(aggregateMutatesHeapAllocator).some((error) =>
    error.includes("heap subsystem boundary"),
  ),
);

const splitHeapAllocatorCommit = structuredClone(manifest);
splitHeapAllocatorCommit.activeRuntimeCheckpoint.runtimeHeapBacking.automaticAllocatorCommitAtomic =
  false;
assert.ok(
  validateCurrentCheckpoint(splitHeapAllocatorCommit).some((error) =>
    error.includes("heap subsystem boundary"),
  ),
);

const restoredAggregateScalarAllocator = structuredClone(manifest);
restoredAggregateScalarAllocator.activeRuntimeCheckpoint.runtimeHeapBacking.runtimeStateDirectScalarAllocators =
  1;
assert.ok(
  validateCurrentCheckpoint(restoredAggregateScalarAllocator).some((error) =>
    error.includes("heap subsystem boundary"),
  ),
);

const disabledActiveCodePageSelector = structuredClone(manifest);
disabledActiveCodePageSelector.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.multiByteAllowActiveCodePageSelector =
  false;
assert.ok(
  validateCurrentCheckpoint(disabledActiveCodePageSelector).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const exactActiveHeapSizes = structuredClone(manifest);
exactActiveHeapSizes.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.heapAllocator = {
  allowedFlags: [0, 8],
  byteLengths: [1, 4, 16, 128],
};
assert.ok(
  validateCurrentCheckpoint(exactActiveHeapSizes).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const unboundedActiveHeap = structuredClone(manifest);
unboundedActiveHeap.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.heapAllocator.maximumAllocationBytes =
  0xffffffff;
assert.ok(
  validateCurrentCheckpoint(unboundedActiveHeap).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const hostShapedAffinityPolicy = structuredClone(manifest);
hostShapedAffinityPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.threadAffinityProcessMask =
  0xffff;
assert.ok(
  validateCurrentCheckpoint(hostShapedAffinityPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const hostShapedSystemInfoPolicy = structuredClone(manifest);
hostShapedSystemInfoPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.systemInfo.processorLevel =
  25;
assert.ok(
  validateCurrentCheckpoint(hostShapedSystemInfoPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const duplicatedSystemInfoMask = structuredClone(manifest);
duplicatedSystemInfoMask.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.systemInfoActiveProcessorMaskSource =
  "system-info-policy-field";
assert.ok(
  validateCurrentCheckpoint(duplicatedSystemInfoMask).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const fixedActiveTickPolicy = structuredClone(manifest);
fixedActiveTickPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.tickCountIncrementPerRead =
  0;
assert.ok(
  validateCurrentCheckpoint(fixedActiveTickPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const derivedSystemDefaultLangId = structuredClone(manifest);
derivedSystemDefaultLangId.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.systemDefaultLangIdDerivedFromAllowedLocales =
  true;
assert.ok(
  validateCurrentCheckpoint(derivedSystemDefaultLangId).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const hostShapedOsVersionPolicy = structuredClone(manifest);
hostShapedOsVersionPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.osVersionInfoA.buildNumber =
  26200;
assert.ok(
  validateCurrentCheckpoint(hostShapedOsVersionPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const browserOwnedOsVersionPolicy = structuredClone(manifest);
browserOwnedOsVersionPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.osVersionInfoABrowserVersionImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedOsVersionPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const hostOwnedOsVersionEvidence = structuredClone(manifest);
hostOwnedOsVersionEvidence.activeRuntimeCheckpoint.latestEvidence.getVersionExAHostVersionImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostOwnedOsVersionEvidence).some((error) =>
    error.includes("ANSI OS-version evidence"),
  ),
);

const enabledAutoResetWaitableTimer = structuredClone(manifest);
enabledAutoResetWaitableTimer.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.createWaitableTimerAllowAutoReset =
  true;
assert.ok(
  validateCurrentCheckpoint(enabledAutoResetWaitableTimer).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const browserOwnedWaitableTimer = structuredClone(manifest);
browserOwnedWaitableTimer.activeRuntimeCheckpoint.latestEvidence.createWaitableTimerABrowserTimerImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedWaitableTimer).some((error) =>
    error.includes("waitable-timer identity and lifetime evidence"),
  ),
);

const partialWaitableTimerTeardown = structuredClone(manifest);
partialWaitableTimerTeardown.activeRuntimeCheckpoint.latestEvidence.createWaitableTimerAProcessExitRetiresPayloadAndObject =
  false;
assert.ok(
  validateCurrentCheckpoint(partialWaitableTimerTeardown).some((error) =>
    error.includes("waitable-timer identity and lifetime evidence"),
  ),
);

const hostShapedDrivePolicy = structuredClone(manifest);
hostShapedDrivePolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.hostDriveTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostShapedDrivePolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const duplicateDriveAuthority = structuredClone(manifest);
duplicateDriveAuthority.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.getDriveTypeAuthority =
  "browser-drive-profile";
assert.ok(
  validateCurrentCheckpoint(duplicateDriveAuthority).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const browserOwnedDriveEvidence = structuredClone(manifest);
browserOwnedDriveEvidence.activeRuntimeCheckpoint.latestEvidence.getDriveTypeABrowserDriveTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDriveEvidence).some((error) =>
    error.includes("drive-type authority evidence"),
  ),
);

const hostShapedDisplayPolicy = structuredClone(manifest);
hostShapedDisplayPolicy.activeRuntimeCheckpoint.user32CompatibilityPolicy.browserDisplayTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostShapedDisplayPolicy).some((error) =>
    error.includes("User32 compatibility policy boundary"),
  ),
);

const sharedMonitorHandleNamespace = structuredClone(manifest);
sharedMonitorHandleNamespace.activeRuntimeCheckpoint.user32CompatibilityPolicy.monitorHandleNamespace =
  "vine.logical-window.v1";
assert.ok(
  validateCurrentCheckpoint(sharedMonitorHandleNamespace).some((error) =>
    error.includes("User32 compatibility policy boundary"),
  ),
);

const mutableDisplayProfile = structuredClone(manifest);
mutableDisplayProfile.activeRuntimeCheckpoint.user32CompatibilityPolicy.configuration =
  "replace-on-enumeration";
assert.ok(
  validateCurrentCheckpoint(mutableDisplayProfile).some((error) =>
    error.includes("User32 compatibility policy boundary"),
  ),
);

const browserOwnedMonitorLookup = structuredClone(manifest);
browserOwnedMonitorLookup.activeRuntimeCheckpoint.latestEvidence.monitorFromWindowBrowserDisplayTopologyImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedMonitorLookup).some((error) =>
    error.includes("latest MonitorFromWindow evidence"),
  ),
);

const browserOwnedMouseMapping = structuredClone(manifest);
browserOwnedMouseMapping.activeRuntimeCheckpoint.user32CompatibilityPolicy.browserMouseButtonMappingImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedMouseMapping).some((error) =>
    error.includes("User32 compatibility policy boundary"),
  ),
);

const duplicateMouseMappingAuthority = structuredClone(manifest);
duplicateMouseMappingAuthority.activeRuntimeCheckpoint.user32CompatibilityPolicy.logicalInputAuthority =
  "browser/runtime-worker/runtime-worker.ts";
assert.ok(
  validateCurrentCheckpoint(duplicateMouseMappingAuthority).some((error) =>
    error.includes("User32 compatibility policy boundary"),
  ),
);

const hostOwnedMouseMappingEvidence = structuredClone(manifest);
hostOwnedMouseMappingEvidence.activeRuntimeCheckpoint.latestEvidence.swapMouseButtonHostStateImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostOwnedMouseMappingEvidence).some((error) =>
    error.includes("mouse-button mapping authority evidence"),
  ),
);

const browserOwnedClientRect = structuredClone(manifest);
browserOwnedClientRect.activeRuntimeCheckpoint.latestEvidence.getClientRectBrowserOwnsLogicalWindowState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedClientRect).some((error) =>
    error.includes("latest GetClientRect evidence"),
  ),
);

const hostOwnedTimerCapabilities = structuredClone(manifest);
hostOwnedTimerCapabilities.activeRuntimeCheckpoint.winmmCompatibilityPolicy.hostTimerCapabilitiesImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostOwnedTimerCapabilities).some((error) =>
    error.includes("WinMM compatibility policy boundary"),
  ),
);

const invalidTimerCapabilityRange = structuredClone(manifest);
invalidTimerCapabilityRange.activeRuntimeCheckpoint.winmmCompatibilityPolicy.periodMinimumMilliseconds =
  0;
assert.ok(
  validateCurrentCheckpoint(invalidTimerCapabilityRange).some((error) =>
    error.includes("WinMM compatibility policy boundary"),
  ),
);

const browserOwnedTimerCapabilityEvidence = structuredClone(manifest);
browserOwnedTimerCapabilityEvidence.activeRuntimeCheckpoint.latestEvidence.timeGetDevCapsBrowserCapabilitiesImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedTimerCapabilityEvidence).some((error) =>
    error.includes("WinMM timer-resolution evidence"),
  ),
);

const hostOwnedTimerResolution = structuredClone(manifest);
hostOwnedTimerResolution.activeRuntimeCheckpoint.winmmCompatibilityPolicy.hostTimerResolutionImported =
  true;
assert.ok(
  validateCurrentCheckpoint(hostOwnedTimerResolution).some((error) =>
    error.includes("WinMM compatibility policy boundary"),
  ),
);

const duplicateTimerResolutionAuthority = structuredClone(manifest);
duplicateTimerResolutionAuthority.activeRuntimeCheckpoint.latestEvidence.timeBeginPeriodBrowserTimerResolutionImported =
  true;
assert.ok(
  validateCurrentCheckpoint(duplicateTimerResolutionAuthority).some((error) =>
    error.includes("WinMM timer-resolution evidence"),
  ),
);

const unmatchedTimerEndSilentlySucceeds = structuredClone(manifest);
unmatchedTimerEndSilentlySucceeds.activeRuntimeCheckpoint.winmmCompatibilityPolicy.unmatchedEndResult =
  0;
assert.ok(
  validateCurrentCheckpoint(unmatchedTimerEndSilentlySucceeds).some((error) =>
    error.includes("WinMM compatibility policy boundary"),
  ),
);

const staleThreadRegistryShape = structuredClone(manifest);
staleThreadRegistryShape.activeRuntimeCheckpoint.runtimeThreadRegistry.schema =
  "vine.runtime-thread-registry.v4";
assert.ok(
  validateCurrentCheckpoint(staleThreadRegistryShape).some((error) =>
    error.includes("thread subsystem boundary"),
  ),
);

const detachedHeapAddress = structuredClone(manifest);
detachedHeapAddress.activeRuntimeCheckpoint.processAddressSpaceAllocator.heapBackingUsesAllocationId =
  false;
assert.ok(
  validateCurrentCheckpoint(detachedHeapAddress).some((error) =>
    error.includes("address-space allocator boundary"),
  ),
);

const detachedComAddress = structuredClone(manifest);
detachedComAddress.activeRuntimeCheckpoint.processAddressSpaceAllocator.comBackingUsesAllocationId =
  false;
assert.ok(
  validateCurrentCheckpoint(detachedComAddress).some((error) =>
    error.includes("address-space allocator boundary"),
  ),
);

const evidenceSelectedHandler = structuredClone(manifest);
evidenceSelectedHandler.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.evidenceSelectsImplementationBranch =
  true;
assert.ok(
  validateCurrentCheckpoint(evidenceSelectedHandler).some((error) =>
    error.includes("compatibility policy boundary"),
  ),
);

const sample = `before\n${README_STATUS_START}\nstale\n${README_STATUS_END}\nafter\n`;
assert.equal(
  replaceReadmeStatus(sample, rendered),
  `before\n${rendered}\nafter\n`,
);
assert.throws(() => replaceReadmeStatus("no markers", rendered), /markers/);

console.log(
  `current checkpoint schema passed: ${manifest.activeRuntimeCheckpoint.genericDispatcherApiCount} APIs`,
);
