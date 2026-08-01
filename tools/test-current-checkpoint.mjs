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
assert.ok(
  rendered.includes(
    "Runtime VFS search enumerates all 45 data0????.arc entries, both missing-file DeleteFileA calls complete, the third ordinary private-heap segment succeeds, the 224 x 112 selected-font GetOutlineTextMetricsA pair completes, and two GetCursorInfo calls project the Rust-owned system-arrow identity and (960, 540) cursor point before the next partial dirty-region UpdateTexture variant",
  ),
);
assert.ok(
  rendered.includes(
    "Node primary recorded 100,556 samples / 40 TIMERPROC callbacks",
  ),
);
assert.ok(rendered.includes("| Shared clock |"));
assert.ok(rendered.includes(manifest.browserCheckpoint.bgiBenchmark));
assert.ok(
  ["threshold-passed", "threshold-blocked", "environment-blocked"].includes(
    manifest.browserCheckpoint.bgiBenchmark,
  ),
);
assert.equal(manifest.schemaVersion, 4);
assert.equal(manifest.activeBlocker, null);
if (manifest.browserCheckpoint.bgiBenchmark === "environment-blocked") {
  assert.equal(
    manifest.browserCheckpoint.bgiBenchmarkEvidence.environmentStatus,
    "invalid-environment",
  );
  assert.ok(manifest.browserCheckpoint.bgiBenchmarkBlocker);
  assert.ok(rendered.includes("| Active blocker |"));
} else if (manifest.browserCheckpoint.bgiBenchmark === "threshold-blocked") {
  assert.equal(
    manifest.browserCheckpoint.bgiBenchmarkEvidence.environmentStatus,
    "valid",
  );
  assert.ok(manifest.browserCheckpoint.bgiBenchmarkBlocker);
  assert.ok(rendered.includes("| Active blocker |"));
} else {
  assert.equal(manifest.browserCheckpoint.bgiBenchmarkBlocker, undefined);
  assert.ok(!rendered.includes("| Active blocker |"));
  assert.equal(
    manifest.browserCheckpoint.bgiBenchmarkEvidence.environmentStatus,
    "valid",
  );
}
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

const staleContractCount = structuredClone(manifest);
staleContractCount.activeRuntimeCheckpoint.genericDispatcherContractCount =
  staleContractCount.activeRuntimeCheckpoint.genericDispatcherApiCount - 1;
assert.ok(
  validateCurrentCheckpoint(staleContractCount).some((error) =>
    error.includes("genericDispatcherContractCount must cover"),
  ),
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

const mismatchedScheduledCalls = structuredClone(manifest);
mismatchedScheduledCalls.activeRuntimeCheckpoint.scheduledSharedClock.nodePrimaryEvidence.completedSupportedCalls -=
  1;
assert.ok(
  validateCurrentCheckpoint(mismatchedScheduledCalls).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

const incompleteEndpointIdentity = structuredClone(manifest);
const structuredStop = incompleteEndpointIdentity.nextStructuredStop;
if (
  structuredStop.kind === "execution-fault" ||
  structuredStop.kind === "execution-budget"
) {
  delete structuredStop.fault.class;
} else {
  const endpointIdentity =
    structuredStop.kind === "capability-checkpoint"
      ? structuredStop.nextEndpoint
      : structuredStop;
  delete endpointIdentity.endpointKind;
}
assert.ok(
  validateCurrentCheckpoint(incompleteEndpointIdentity).some((error) =>
    error.includes(
      manifest.nextStructuredStop.kind === "execution-fault" ||
        manifest.nextStructuredStop.kind === "execution-budget"
        ? "nextStructuredStop.fault.class"
        : manifest.nextStructuredStop.kind === "capability-checkpoint"
        ? "nextStructuredStop.nextEndpoint.endpointKind"
        : "nextStructuredStop.endpointKind",
    ),
  ),
);

const failedBgiBenchmark = structuredClone(manifest);
failedBgiBenchmark.browserCheckpoint.bgiBenchmarkEvidence.thresholdFailures += 1;
assert.ok(
  validateCurrentCheckpoint(failedBgiBenchmark).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const unexpectedStructuredBenchmarkBlocker = structuredClone(manifest);
unexpectedStructuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkBlocker = {};
assert.ok(
  validateCurrentCheckpoint(unexpectedStructuredBenchmarkBlocker).some((error) =>
    error.includes(
      manifest.browserCheckpoint.bgiBenchmark === "environment-blocked"
        ? "performance-environment blocker"
        : manifest.browserCheckpoint.bgiBenchmark === "threshold-blocked"
          ? "DragAcceptFiles structured performance blocker"
          : "passed browser benchmark",
    ),
  ),
);

const relaxedStructuredBenchmarkThreshold = structuredClone(manifest);
if (
  relaxedStructuredBenchmarkThreshold.browserCheckpoint.bgiBenchmark ===
  "threshold-blocked"
) {
  relaxedStructuredBenchmarkThreshold.browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.threshold =
    20_000;
  assert.ok(
    validateCurrentCheckpoint(relaxedStructuredBenchmarkThreshold).some((error) =>
      error.includes("DragAcceptFiles structured performance blocker"),
    ),
  );
}

const browserOwnedDragAcceptance = structuredClone(manifest);
browserOwnedDragAcceptance.browserCheckpoint.bgiRuntimeEvidence.dragAcceptFilesBrowserOwnsLogicalWindowState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDragAcceptance).some((error) =>
    error.includes("DragAcceptFiles evidence"),
  ),
);

const incompleteActiveBlocker = structuredClone(manifest);
incompleteActiveBlocker.activeBlocker = {};
assert.ok(
  validateCurrentCheckpoint(incompleteActiveBlocker).some((error) =>
    error.includes("active structured blocker"),
  ),
);

const clearedActiveBlocker = structuredClone(manifest);
clearedActiveBlocker.activeBlocker = null;
assert.deepEqual(validateCurrentCheckpoint(clearedActiveBlocker), []);
assert.ok(
  manifest.browserCheckpoint.bgiBenchmark !== "threshold-passed" ||
    !renderReadmeStatus(clearedActiveBlocker).includes("| Active blocker |"),
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

const mismatchedEnvironmentMonitor = structuredClone(manifest);
mismatchedEnvironmentMonitor.browserCheckpoint.bgiBenchmarkEvidence.environmentMonitorSha256 =
  "0".repeat(64);
assert.ok(
  validateCurrentCheckpoint(mismatchedEnvironmentMonitor).some((error) =>
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

const duplicatedSurfaceAuthority = structuredClone(manifest);
duplicatedSurfaceAuthority.activeRuntimeCheckpoint.d3d9Frontend.runtimeGraphics.surfaceTextureRelation =
  "browser-owned-texture-relation";
assert.ok(
  validateCurrentCheckpoint(duplicatedSurfaceAuthority).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const unstableSurfaceIdentity = structuredClone(manifest);
unstableSurfaceIdentity.activeRuntimeCheckpoint.d3d9Frontend.getSurfaceLevel.repeatReturnsSameIdentityAndAddsReference =
  false;
assert.ok(
  validateCurrentCheckpoint(unstableSurfaceIdentity).some((error) =>
    error.includes("active D3D9 frontend boundary"),
  ),
);

const partialGetSurfaceLevelPublication = structuredClone(manifest);
partialGetSurfaceLevelPublication.activeRuntimeCheckpoint.latestEvidence.d3d9GetSurfaceLevelGuestComGraphicsAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialGetSurfaceLevelPublication).some((error) =>
    error.includes("latest D3D9 GetSurfaceLevel evidence"),
  ),
);

const partialSurfaceDcPublication = structuredClone(manifest);
partialSurfaceDcPublication.activeRuntimeCheckpoint.latestEvidence.d3d9SurfaceDcGuestWriteAndGdiPublicationAtomic =
  false;
assert.ok(
  validateCurrentCheckpoint(partialSurfaceDcPublication).some((error) =>
    error.includes("latest D3D9 Surface DC evidence"),
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
    error.includes("waitable-timer identity, arm, and lifetime evidence"),
  ),
);

const partialWaitableTimerTeardown = structuredClone(manifest);
partialWaitableTimerTeardown.activeRuntimeCheckpoint.latestEvidence.createWaitableTimerAProcessExitRetiresPayloadAndObject =
  false;
assert.ok(
  validateCurrentCheckpoint(partialWaitableTimerTeardown).some((error) =>
    error.includes("waitable-timer identity, arm, and lifetime evidence"),
  ),
);

const enabledWaitableTimerCompletionRoutine = structuredClone(manifest);
enabledWaitableTimerCompletionRoutine.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.setWaitableTimerAllowCompletionRoutine =
  true;
assert.ok(
  validateCurrentCheckpoint(enabledWaitableTimerCompletionRoutine).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const enabledWaitableTimerResume = structuredClone(manifest);
enabledWaitableTimerResume.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.setWaitableTimerAllowResume =
  true;
assert.ok(
  validateCurrentCheckpoint(enabledWaitableTimerResume).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const browserOwnedWaitableTimerArm = structuredClone(manifest);
browserOwnedWaitableTimerArm.activeRuntimeCheckpoint.latestEvidence.setWaitableTimerBrowserTimerImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedWaitableTimerArm).some((error) =>
    error.includes("waitable-timer identity, arm, and lifetime evidence"),
  ),
);

const partialWaitableTimerArmCommit = structuredClone(manifest);
partialWaitableTimerArmCommit.activeRuntimeCheckpoint.latestEvidence.setWaitableTimerFailureAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialWaitableTimerArmCommit).some((error) =>
    error.includes("waitable-timer identity, arm, and lifetime evidence"),
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
    error.includes("WinMM timer and shared-clock evidence"),
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
    error.includes("WinMM timer and shared-clock evidence"),
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

const duplicatedTimeGetTimeAuthority = structuredClone(manifest);
duplicatedTimeGetTimeAuthority.activeRuntimeCheckpoint.winmmCompatibilityPolicy.timeGetTimeClockAuthority =
  "browser-performance-now";
assert.ok(
  validateCurrentCheckpoint(duplicatedTimeGetTimeAuthority).some((error) =>
    error.includes("WinMM compatibility policy boundary"),
  ),
);

const browserOwnedTimeGetTime = structuredClone(manifest);
browserOwnedTimeGetTime.activeRuntimeCheckpoint.latestEvidence.timeGetTimeBrowserClockImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedTimeGetTime).some((error) =>
    error.includes("WinMM timer and shared-clock evidence"),
  ),
);

const detachedTimeGetTimeClock = structuredClone(manifest);
detachedTimeGetTimeClock.activeRuntimeCheckpoint.latestEvidence.timeGetTimeClockRelation =
  "winmm-private-clock";
assert.ok(
  validateCurrentCheckpoint(detachedTimeGetTimeClock).some((error) =>
    error.includes("WinMM timer and shared-clock evidence"),
  ),
);

const browserOwnedPerformanceCounterPolicy = structuredClone(manifest);
browserOwnedPerformanceCounterPolicy.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.performanceCounterBrowserClockImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedPerformanceCounterPolicy).some((error) =>
    error.includes("Kernel32 compatibility policy boundary"),
  ),
);

const detachedPerformanceCounterAuthority = structuredClone(manifest);
detachedPerformanceCounterAuthority.activeRuntimeCheckpoint.latestEvidence.queryPerformanceCounterStateAuthority =
  "browser-performance-now";
assert.ok(
  validateCurrentCheckpoint(detachedPerformanceCounterAuthority).some((error) =>
    error.includes("high-resolution performance-clock evidence"),
  ),
);

const browserOwnedPerformanceFrequency = structuredClone(manifest);
browserOwnedPerformanceFrequency.browserCheckpoint.bgiRuntimeEvidence.queryPerformanceFrequencyBrowserClockImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedPerformanceFrequency).some((error) =>
    error.includes("browser BGI authority or teardown evidence"),
  ),
);

const browserOwnedDirectInputComState = structuredClone(manifest);
browserOwnedDirectInputComState.browserCheckpoint.bgiRuntimeEvidence.directInput8CreateBrowserOwnsComState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDirectInputComState).some((error) =>
    error.includes("browser BGI authority or teardown evidence"),
  ),
);

const partialDirectInputPublication = structuredClone(manifest);
partialDirectInputPublication.activeRuntimeCheckpoint.latestEvidence.directInput8CreateFailureAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialDirectInputPublication).some((error) =>
    error.includes("DirectInput 8 root-factory evidence"),
  ),
);

const browserOwnedDirectInputEnumeration = structuredClone(manifest);
browserOwnedDirectInputEnumeration.browserCheckpoint.bgiRuntimeEvidence.directInput8EnumDevicesBrowserInputImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDirectInputEnumeration).some((error) =>
    error.includes("browser BGI authority or teardown evidence"),
  ),
);

const partialDirectInputEnumeration = structuredClone(manifest);
partialDirectInputEnumeration.activeRuntimeCheckpoint.latestEvidence.directInput8EnumDevicesFailureAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialDirectInputEnumeration).some((error) =>
    error.includes("DirectInput 8 EnumDevices evidence"),
  ),
);

const unsupportedCallbackClaim = structuredClone(manifest);
unsupportedCallbackClaim.activeRuntimeCheckpoint.latestEvidence.directInput8EnumDevicesCallbackProducingVariantsImplemented =
  true;
assert.ok(
  validateCurrentCheckpoint(unsupportedCallbackClaim).some((error) =>
    error.includes("DirectInput 8 EnumDevices evidence"),
  ),
);

const missingDirectInputBoundaryTest = structuredClone(manifest);
missingDirectInputBoundaryTest.activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests =
  missingDirectInputBoundaryTest.activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests.filter(
    (test) => !test.includes("factory_can_publish_iunknown"),
  );
assert.ok(
  validateCurrentCheckpoint(missingDirectInputBoundaryTest).some((error) =>
    error.includes("runtime kernel responsibility boundary"),
  ),
);

const browserOwnedDirectSoundState = structuredClone(manifest);
browserOwnedDirectSoundState.browserCheckpoint.bgiRuntimeEvidence.directSoundCreate8BrowserOwnsComOrAudioState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDirectSoundState).some((error) =>
    error.includes("browser BGI authority or teardown evidence"),
  ),
);

const partialDirectSoundPublication = structuredClone(manifest);
partialDirectSoundPublication.activeRuntimeCheckpoint.latestEvidence.directSoundCreate8GuestComAudioAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialDirectSoundPublication).some((error) =>
    error.includes("DirectSound 8 root and cooperative-level evidence"),
  ),
);

const browserOwnedDirectSoundCooperativeState = structuredClone(manifest);
browserOwnedDirectSoundCooperativeState.activeRuntimeCheckpoint.latestEvidence.directSoundSetCooperativeLevelBrowserAudioImported =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedDirectSoundCooperativeState).some((error) =>
    error.includes("DirectSound 8 root and cooperative-level evidence"),
  ),
);

const partialDirectSoundCooperativeCommit = structuredClone(manifest);
partialDirectSoundCooperativeCommit.activeRuntimeCheckpoint.latestEvidence.directSoundSetCooperativeLevelFailureAtomicity =
  false;
assert.ok(
  validateCurrentCheckpoint(partialDirectSoundCooperativeCommit).some((error) =>
    error.includes("DirectSound 8 root and cooperative-level evidence"),
  ),
);

const missingDirectSoundBoundaryTest = structuredClone(manifest);
missingDirectSoundBoundaryTest.activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests =
  missingDirectSoundBoundaryTest.activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests.filter(
    (test) => !test.includes("factory_resource_exhaustion_rolls_back_guest_com_and_audio_state"),
  );
assert.ok(
  validateCurrentCheckpoint(missingDirectSoundBoundaryTest).some((error) =>
    error.includes("runtime kernel responsibility boundary"),
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

const partialDibPublication = structuredClone(manifest);
partialDibPublication.activeRuntimeCheckpoint.latestEvidence.dibFailureAtomicity = false;
assert.ok(
  validateCurrentCheckpoint(partialDibPublication).some((error) =>
    error.includes("latest bounded DIB-section lifecycle evidence"),
  ),
);

const browserOwnedDibBytes = structuredClone(manifest);
browserOwnedDibBytes.browserCheckpoint.bgiRuntimeEvidence.dibByteAuthority =
  "browser-array-buffer";
assert.ok(
  validateCurrentCheckpoint(browserOwnedDibBytes).some((error) =>
    error.includes("browser BGI DIB-section lifecycle evidence"),
  ),
);

const browserOwnedSharedClock = structuredClone(manifest);
browserOwnedSharedClock.activeRuntimeCheckpoint.scheduledSharedClock.browserBoundaryEvidence.browserOwnsGuestClockState =
  true;
assert.ok(
  validateCurrentCheckpoint(browserOwnedSharedClock).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

const advancingScheduledApiRead = structuredClone(manifest);
advancingScheduledApiRead.activeRuntimeCheckpoint.scheduledSharedClock.apiReadsAdvanceClock =
  true;
assert.ok(
  validateCurrentCheckpoint(advancingScheduledApiRead).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

const staleScheduledClockSampleCount = structuredClone(manifest);
staleScheduledClockSampleCount.activeRuntimeCheckpoint.scheduledSharedClock.nodePrimaryEvidence.clockSampleCount -= 1;
assert.ok(
  validateCurrentCheckpoint(staleScheduledClockSampleCount).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

const staleScheduledNextEndpoint = structuredClone(manifest);
staleScheduledNextEndpoint.nextStructuredStop.vtableSlot -= 1;
assert.ok(
  validateCurrentCheckpoint(staleScheduledNextEndpoint).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

const falseScheduledClockConclusion = structuredClone(manifest);
falseScheduledClockConclusion.activeRuntimeCheckpoint.scheduledSharedClock.proximateCauseConclusion.clockFreezeWasProximateCause =
  true;
assert.ok(
  validateCurrentCheckpoint(falseScheduledClockConclusion).some((error) =>
    error.includes("scheduled shared clock checkpoint evidence"),
  ),
);

// These assertions describe the immediately preceding pointer-release
// checkpoint and remain useful when replaying an older manifest profile.
if (!manifest.activeRuntimeCheckpoint.scheduledSharedClock) {
const stalePostIsIconicAsyncKeyScanOrder = structuredClone(manifest);
stalePostIsIconicAsyncKeyScanOrder.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.scanVirtualKeys[0] =
  15;
assert.ok(
  validateCurrentCheckpoint(stalePostIsIconicAsyncKeyScanOrder).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
    ),
  ),
);

const stalePostIsIconicAsyncKeyScanResult = structuredClone(manifest);
stalePostIsIconicAsyncKeyScanResult.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.getAsyncKeyStatePostIsIconicClockLast.result.value =
  1;
assert.ok(
  validateCurrentCheckpoint(stalePostIsIconicAsyncKeyScanResult).some(
    (error) =>
      error.includes(
        "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const mutatedPostInputScanTestCooperativeLevel = structuredClone(manifest);
mutatedPostInputScanTestCooperativeLevel.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.d3d9TestCooperativeLevelPostInputScan.result.value =
  0x88760868;
assert.ok(
  validateCurrentCheckpoint(mutatedPostInputScanTestCooperativeLevel).some(
    (error) =>
      error.includes(
        "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const staleTestCooperativeLevelCumulativeCount = structuredClone(manifest);
staleTestCooperativeLevelCumulativeCount.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.cumulativeCompletions =
  8;
assert.ok(
  validateCurrentCheckpoint(staleTestCooperativeLevelCumulativeCount).some(
    (error) =>
      error.includes(
        "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const stalePostInputTimerArmGeneration = structuredClone(manifest);
stalePostInputTimerArmGeneration.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.timerArm.waitableTimer.timer.arm.generation =
  8;
assert.ok(
  validateCurrentCheckpoint(stalePostInputTimerArmGeneration).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
    ),
  ),
);

const stalePostInputTimerExpirationGeneration = structuredClone(manifest);
stalePostInputTimerExpirationGeneration.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.timerWait.waitableTimer.timer.lastExpiration.armGeneration =
  8;
assert.ok(
  validateCurrentCheckpoint(stalePostInputTimerExpirationGeneration).some(
    (error) =>
      error.includes(
        "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const stalePostInputTimerCompletionBoundary = structuredClone(manifest);
stalePostInputTimerCompletionBoundary.nextStructuredStop.completion.cumulativeTimerWaitCompletions =
  11_474;
assert.ok(
  validateCurrentCheckpoint(stalePostInputTimerCompletionBoundary).some(
    (error) =>
      error.includes(
        "post-window-timer-callback PeekMessageA continuation pointer-release continuation checkpoint stop is invalid",
      ),
  ),
);

const staleWindowTimerDelivery = structuredClone(manifest);
staleWindowTimerDelivery.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.windowTimerPeek.windowTimer.deliveryCount =
  2;
assert.ok(
  validateCurrentCheckpoint(staleWindowTimerDelivery).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
    ),
  ),
);

const staleWindowTimerCallbackFrame = structuredClone(manifest);
staleWindowTimerCallbackFrame.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.windowTimerDispatch.callback.frameId =
  34;
assert.ok(
  validateCurrentCheckpoint(staleWindowTimerCallbackFrame).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
    ),
  ),
);

const staleWindowTimerNestedTick = structuredClone(manifest);
staleWindowTimerNestedTick.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.windowTimerCallbackNestedCalls[0].tickCount.nextTickCount =
  3_601_229;
assert.ok(
  validateCurrentCheckpoint(staleWindowTimerNestedTick).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
    ),
  ),
);

const nonEmptyPostWindowTimerCallbackPeek = structuredClone(manifest);
nonEmptyPostWindowTimerCallbackPeek.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.peekMessagePostWindowTimerCallback.result.value =
  1;
assert.ok(
  validateCurrentCheckpoint(nonEmptyPostWindowTimerCallbackPeek).some(
    (error) =>
      error.includes(
        "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const mutatedPostWindowTimerCallbackQueue = structuredClone(manifest);
mutatedPostWindowTimerCallbackQueue.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.postWindowTimerCallbackQueueDepthAfter =
  1;
assert.ok(
  validateCurrentCheckpoint(mutatedPostWindowTimerCallbackQueue).some(
    (error) =>
      error.includes(
        "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const injectedPostWindowTimerCallbackWrite = structuredClone(manifest);
injectedPostWindowTimerCallbackWrite.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.peekMessagePostWindowTimerCallback.guestWrites.push(
  { address: 7_338_216, bytes: [0] },
);
assert.ok(
  validateCurrentCheckpoint(injectedPostWindowTimerCallbackWrite).some(
    (error) =>
      error.includes(
        "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence is invalid",
      ),
  ),
);

const staleCombinedBrowserGeneration = structuredClone(manifest);
staleCombinedBrowserGeneration.browserCheckpoint.bgiRuntimeEvidence.finalGeneration =
  324_436;
assert.ok(
  validateCurrentCheckpoint(staleCombinedBrowserGeneration).some((error) =>
    error.includes("browser BGI authority or teardown evidence"),
  ),
);

const detachedWindowTimerQueue = structuredClone(manifest);
detachedWindowTimerQueue.activeRuntimeCheckpoint.latestEvidence.windowTimerQueueAuthority =
  "typescript-worker-message-queue";
assert.ok(
  validateCurrentCheckpoint(detachedWindowTimerQueue).some((error) =>
    error.includes(
      "queue authority evidence is invalid for the post-window-timer-callback PeekMessageA continuation pointer-release checkpoint",
    ),
  ),
);

const consumedTimerProcResult = structuredClone(manifest);
consumedTimerProcResult.activeRuntimeCheckpoint.latestEvidence.dispatchMessageACallbackResultPolicy =
  "return-guest-result";
assert.ok(
  validateCurrentCheckpoint(consumedTimerProcResult).some((error) =>
    error.includes(
      "queue authority evidence is invalid for the post-window-timer-callback PeekMessageA continuation pointer-release checkpoint",
    ),
  ),
);

const detachedSetFocusThread = structuredClone(manifest);
detachedSetFocusThread.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.setFocus.runtimeThreadId =
  8_193;
assert.ok(
  validateCurrentCheckpoint(detachedSetFocusThread).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const detachedBrowserSetFocusThread = structuredClone(manifest);
detachedBrowserSetFocusThread.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.setFocus.runtimeThreadId =
  8_193;
assert.ok(
  validateCurrentCheckpoint(detachedBrowserSetFocusThread).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const detachedTimeGetTimeThread = structuredClone(manifest);
detachedTimeGetTimeThread.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.timeGetTime.runtimeThreadId =
  8_193;
assert.ok(
  validateCurrentCheckpoint(detachedTimeGetTimeThread).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const staleBrowserTimeGetTimeTick = structuredClone(manifest);
staleBrowserTimeGetTimeTick.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.timeGetTime.tickCount.nextTickCount =
  3_601_225;
assert.ok(
  validateCurrentCheckpoint(staleBrowserTimeGetTimeTick).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const detachedDefWindowProcAThread = structuredClone(manifest);
detachedDefWindowProcAThread.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.defWindowProcA.runtimeThreadId =
  8_193;
assert.ok(
  validateCurrentCheckpoint(detachedDefWindowProcAThread).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedBrowserDefWindowProcAResult = structuredClone(manifest);
mutatedBrowserDefWindowProcAResult.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.defWindowProcA.result.value =
  1;
assert.ok(
  validateCurrentCheckpoint(mutatedBrowserDefWindowProcAResult).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const retainedPointerRelease = structuredClone(manifest);
retainedPointerRelease.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.releaseMessageQueued =
  true;
assert.ok(
  validateCurrentCheckpoint(retainedPointerRelease).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const staleBrowserReleaseQueueDepth = structuredClone(manifest);
staleBrowserReleaseQueueDepth.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.queueDepthAfter =
  1;
assert.ok(
  validateCurrentCheckpoint(staleBrowserReleaseQueueDepth).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const stalePointerPeekTimerMetadata = structuredClone(manifest);
stalePointerPeekTimerMetadata.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.peekMessage.windowTimer =
  { deliveryCount: 2 };
assert.ok(
  validateCurrentCheckpoint(stalePointerPeekTimerMetadata).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedReleaseMessageBytes = structuredClone(manifest);
mutatedReleaseMessageBytes.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.peekMessageRelease.guestWrites[0].bytes[4] =
  1;
assert.ok(
  validateCurrentCheckpoint(mutatedReleaseMessageBytes).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedInheritedPointerInput = structuredClone(manifest);
mutatedInheritedPointerInput.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.inputEvents[0].event.normalizedX =
  32_767;
assert.ok(
  validateCurrentCheckpoint(mutatedInheritedPointerInput).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedInheritedDispatchEntry = structuredClone(manifest);
mutatedInheritedDispatchEntry.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.dispatchEntry.pendingCallback =
  false;
assert.ok(
  validateCurrentCheckpoint(mutatedInheritedDispatchEntry).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedInheritedCallbackEntry = structuredClone(manifest);
mutatedInheritedCallbackEntry.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.callbackEntry.arguments[1] =
  514;
assert.ok(
  validateCurrentCheckpoint(mutatedInheritedCallbackEntry).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedTranslateMessageResult = structuredClone(manifest);
mutatedTranslateMessageResult.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.translateMessageRelease.result.value =
  1;
assert.ok(
  validateCurrentCheckpoint(mutatedTranslateMessageResult).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const injectedTranslateTimerMetadata = structuredClone(manifest);
injectedTranslateTimerMetadata.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.translateMessageRelease.windowTimer =
  { deliveryCount: 1 };
assert.ok(
  validateCurrentCheckpoint(injectedTranslateTimerMetadata).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const injectedTranslateGuestWrite = structuredClone(manifest);
injectedTranslateGuestWrite.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.translateMessageRelease.guestWrites.push(
  { address: 7_338_216, bytes: [0] },
);
assert.ok(
  validateCurrentCheckpoint(injectedTranslateGuestWrite).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const staleTranslateQueueDepth = structuredClone(manifest);
staleTranslateQueueDepth.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.queueDepthAtTranslateCompletion =
  1;
assert.ok(
  validateCurrentCheckpoint(staleTranslateQueueDepth).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const unpreservedTranslateMessageBytes = structuredClone(manifest);
unpreservedTranslateMessageBytes.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.translateMessageReleaseBytesPreserved =
  false;
assert.ok(
  validateCurrentCheckpoint(unpreservedTranslateMessageBytes).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const staleDefWindowProcAPolicy = structuredClone(manifest);
staleDefWindowProcAPolicy.activeRuntimeCheckpoint.user32CompatibilityPolicy.defWindowProcAPointerDown.requireCursorDerivedClientLParam =
  false;
assert.ok(
  validateCurrentCheckpoint(staleDefWindowProcAPolicy).some((error) =>
    error.includes("active User32 compatibility policy boundary"),
  ),
);

const staleDefWindowProcAReleasePolicy = structuredClone(manifest);
staleDefWindowProcAReleasePolicy.activeRuntimeCheckpoint.user32CompatibilityPolicy.defWindowProcAPointerRelease.wParam =
  1;
assert.ok(
  validateCurrentCheckpoint(staleDefWindowProcAReleasePolicy).some((error) =>
    error.includes("active User32 compatibility policy boundary"),
  ),
);

const stalePointerReleaseStop = structuredClone(manifest);
stalePointerReleaseStop.nextStructuredStop.nextEndpoint.endpointId = 182;
assert.ok(
  validateCurrentCheckpoint(stalePointerReleaseStop).some((error) =>
    error.includes(
      "post-window-timer-callback PeekMessageA continuation pointer-release continuation checkpoint stop is invalid",
    ),
  ),
);

const stalePostReleaseQueue = structuredClone(manifest);
stalePostReleaseQueue.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.postReleaseQueueDepthAfter =
  1;
assert.ok(
  validateCurrentCheckpoint(stalePostReleaseQueue).some((error) =>
    error.includes("latest post-release PeekMessageA evidence"),
  ),
);

const mutatedBrowserPostReleasePeek = structuredClone(manifest);
mutatedBrowserPostReleasePeek.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.peekMessagePostRelease.result.value =
  1;
assert.ok(
  validateCurrentCheckpoint(mutatedBrowserPostReleasePeek).some((error) =>
    error.includes("browser BGI post-release PeekMessageA evidence"),
  ),
);

const unpreservedPostReleaseMessage = structuredClone(manifest);
unpreservedPostReleaseMessage.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.postReleaseMessageBytesPreserved =
  false;
assert.ok(
  validateCurrentCheckpoint(unpreservedPostReleaseMessage).some((error) =>
    error.includes("latest post-release PeekMessageA evidence"),
  ),
);

const dispatchReleaseMutations = [
  {
    label: "pending callback",
    mutate(candidate) {
      candidate.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.dispatchMessageRelease.pendingCallback =
        false;
    },
    expected: "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "queue depth",
    mutate(candidate) {
      candidate.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.queueDepthAtDispatchEntry =
        1;
    },
    expected:
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "MSG preservation",
    mutate(candidate) {
      candidate.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.dispatchMessageReleaseBytesPreserved =
        false;
    },
    expected: "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "callback frame identity",
    mutate(candidate) {
      candidate.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.releaseCallbackEntry.frameId =
        35;
    },
    expected:
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "active callback depth",
    mutate(candidate) {
      candidate.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.activeCallbackDepthAtStop =
        1;
    },
    expected: "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "release default result",
    mutate(candidate) {
      candidate.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.defWindowProcARelease.result.value =
        1;
    },
    expected: "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "release callback return",
    mutate(candidate) {
      candidate.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.releaseCallbackReturn.result =
        1;
    },
    expected:
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
  },
  {
    label: "next endpoint",
    mutate(candidate) {
      candidate.nextStructuredStop.nextEndpoint.endpointId = 182;
    },
    expected:
      "post-window-timer-callback PeekMessageA continuation pointer-release continuation checkpoint stop is invalid",
  },
];
for (const { label, mutate, expected } of dispatchReleaseMutations) {
  const candidate = structuredClone(manifest);
  mutate(candidate);
  assert.ok(
    validateCurrentCheckpoint(candidate).some((error) =>
      error.includes(expected),
    ),
    `post-window-timer-callback PeekMessageA continuation pointer-release validator accepted mutated ${label}`,
  );
}

const stalePostReleaseTick = structuredClone(manifest);
stalePostReleaseTick.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.timeGetTimePostRelease.tickCount.nextTickCount =
  3_601_224;
assert.ok(
  validateCurrentCheckpoint(stalePostReleaseTick).some((error) =>
    error.includes("latest post-release timeGetTime evidence"),
  ),
);

const browserOwnedPostReleaseClock = structuredClone(manifest);
browserOwnedPostReleaseClock.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.timeGetTimePostRelease.result.value =
  3_601_225;
assert.ok(
  validateCurrentCheckpoint(browserOwnedPostReleaseClock).some((error) =>
    error.includes("browser BGI post-release timeGetTime evidence"),
  ),
);

const stalePostReleaseClockStack = structuredClone(manifest);
stalePostReleaseClockStack.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.timeGetTimePostRelease.stackPointerAfter =
  7_338_244;
assert.ok(
  validateCurrentCheckpoint(stalePostReleaseClockStack).some((error) =>
    error.includes("latest post-release timeGetTime evidence"),
  ),
);

const staleSecondPostReleaseTick = structuredClone(manifest);
staleSecondPostReleaseTick.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.timeGetTimePostReleaseCompletions[1].tickCount.nextTickCount =
  3_601_225;
assert.ok(
  validateCurrentCheckpoint(staleSecondPostReleaseTick).some((error) =>
    error.includes("latest post-release timeGetTime repeat batch evidence"),
  ),
);

const truncatedBrowserPostReleaseBatch = structuredClone(manifest);
truncatedBrowserPostReleaseBatch.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.timeGetTimePostReleaseCompletions.pop();
assert.ok(
  validateCurrentCheckpoint(truncatedBrowserPostReleaseBatch).some((error) =>
    error.includes("browser BGI post-release timeGetTime repeat batch evidence"),
  ),
);

const unboundedPostReleaseBatch = structuredClone(manifest);
unboundedPostReleaseBatch.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.maximumRepeatCompletions =
  17;
assert.ok(
  validateCurrentCheckpoint(unboundedPostReleaseBatch).some((error) =>
    error.includes("latest post-release timeGetTime repeat batch evidence"),
  ),
);

const stalePostReleaseRect = structuredClone(manifest);
stalePostReleaseRect.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.getWindowRectPostRelease.guestWrites[0].bytes[0] =
  62;
assert.ok(
  validateCurrentCheckpoint(stalePostReleaseRect).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const stalePostReleasePoint = structuredClone(manifest);
stalePostReleasePoint.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.getCursorPosPostRelease.guestWrites[0].bytes[0] =
  191;
assert.ok(
  validateCurrentCheckpoint(stalePostReleasePoint).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedPostReleaseSystemMetric = structuredClone(manifest);
mutatedPostReleaseSystemMetric.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.getSystemMetricsPostRelease.result.value =
  4;
assert.ok(
  validateCurrentCheckpoint(mutatedPostReleaseSystemMetric).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const injectedBrowserSystemMetricWrite = structuredClone(manifest);
injectedBrowserSystemMetricWrite.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.getSystemMetricsPostRelease.guestWrites.push(
  { address: 7_338_164, bytes: [3, 0, 0, 0] },
);
assert.ok(
  validateCurrentCheckpoint(injectedBrowserSystemMetricWrite).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedPostReleaseCaptionMetric = structuredClone(manifest);
mutatedPostReleaseCaptionMetric.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.getSystemMetricsCaptionPostRelease.result.value =
  24;
assert.ok(
  validateCurrentCheckpoint(mutatedPostReleaseCaptionMetric).some((error) =>
    error.includes(
      "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const injectedBrowserCaptionMetricWrite = structuredClone(manifest);
injectedBrowserCaptionMetricWrite.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.getSystemMetricsCaptionPostRelease.guestWrites.push(
  { address: 7_338_164, bytes: [23, 0, 0, 0] },
);
assert.ok(
  validateCurrentCheckpoint(injectedBrowserCaptionMetricWrite).some((error) =>
    error.includes(
      "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
    ),
  ),
);

const mutatedPostReleaseFixedFrameHeightMetric = structuredClone(manifest);
mutatedPostReleaseFixedFrameHeightMetric.activeRuntimeCheckpoint.latestEvidence.capabilityCheckpointCompletion.getSystemMetricsFixedFrameHeightPostRelease.result.value =
  4;
assert.ok(
  validateCurrentCheckpoint(mutatedPostReleaseFixedFrameHeightMetric).some(
    (error) =>
      error.includes(
        "latest post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
      ),
  ),
);

const injectedBrowserFixedFrameHeightMetricWrite = structuredClone(manifest);
injectedBrowserFixedFrameHeightMetricWrite.browserCheckpoint.bgiRuntimeEvidence.checkpointCompletion.getSystemMetricsFixedFrameHeightPostRelease.guestWrites.push(
  { address: 7_338_164, bytes: [3, 0, 0, 0] },
);
assert.ok(
  validateCurrentCheckpoint(injectedBrowserFixedFrameHeightMetricWrite).some(
    (error) =>
      error.includes(
        "browser BGI post-window-timer-callback PeekMessageA continuation pointer-release checkpoint evidence",
      ),
  ),
);

const staleSystemMetricAggregate = structuredClone(manifest);
staleSystemMetricAggregate.activeRuntimeCheckpoint.latestEvidence.getSystemMetricsResults[11] =
  4;
assert.ok(
  validateCurrentCheckpoint(staleSystemMetricAggregate).some((error) =>
    error.includes("latest controlled system-metrics evidence"),
  ),
);

const staleCaptionMetricAggregate = structuredClone(manifest);
staleCaptionMetricAggregate.activeRuntimeCheckpoint.latestEvidence.getSystemMetricsResults[12] =
  24;
assert.ok(
  validateCurrentCheckpoint(staleCaptionMetricAggregate).some((error) =>
    error.includes("latest controlled system-metrics evidence"),
  ),
);

const staleFixedFrameHeightMetricAggregate = structuredClone(manifest);
staleFixedFrameHeightMetricAggregate.activeRuntimeCheckpoint.latestEvidence.getSystemMetricsResults[13] =
  4;
assert.ok(
  validateCurrentCheckpoint(staleFixedFrameHeightMetricAggregate).some(
    (error) => error.includes("latest controlled system-metrics evidence"),
  ),
);

const staleFixedFrameHeightMetricStop = structuredClone(manifest);
staleFixedFrameHeightMetricStop.nextStructuredStop.nextEndpoint.arguments[0] =
  "0x00000007";
assert.ok(
  validateCurrentCheckpoint(staleFixedFrameHeightMetricStop).some((error) =>
    error.includes(
      "post-window-timer-callback PeekMessageA continuation pointer-release continuation checkpoint stop is invalid",
    ),
  ),
);
}

const sample = `before\n${README_STATUS_START}\nstale\n${README_STATUS_END}\nafter\n`;
assert.equal(
  replaceReadmeStatus(sample, rendered),
  `before\n${rendered}\nafter\n`,
);
assert.throws(() => replaceReadmeStatus("no markers", rendered), /markers/);

console.log(
  `current checkpoint schema passed: ${manifest.activeRuntimeCheckpoint.genericDispatcherApiCount} APIs`,
);
