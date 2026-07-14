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
assert.ok(rendered.includes(manifest.nextStructuredStop.reason));
assert.ok(rendered.includes(manifest.browserCheckpoint.bgiBenchmark));
for (const api of manifest.activeRuntimeCheckpoint.genericDispatcherApis) {
  assert.ok(rendered.includes(`\`${api}\``));
}

const stale = structuredClone(manifest);
stale.activeRuntimeCheckpoint.genericDispatcherApiCount += 1;
assert.ok(
  validateCurrentCheckpoint(stale).some((error) => error.includes("must match")),
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

const missingBrowserRelease = structuredClone(manifest);
missingBrowserRelease.browserCheckpoint.browserRuntimeEvidence.mappingOperations.release = 0;
assert.ok(
  validateCurrentCheckpoint(missingBrowserRelease).some((error) =>
    error.includes("mapping-operation evidence"),
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
failedBgiBenchmark.browserCheckpoint.bgiBenchmarkEvidence.thresholdFailures = 1;
assert.ok(
  validateCurrentCheckpoint(failedBgiBenchmark).some((error) =>
    error.includes("benchmark evidence"),
  ),
);

const structuredBenchmarkBlocker = structuredClone(manifest);
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmark = "threshold-blocked";
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.completedSessions = 100;
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.thresholdFailures = 1;
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.thresholdFailureIds = [
  "minimum-completed-sessions",
];
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.decisionStatus = "failed";
structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.decisionReason =
  `1-of-${structuredBenchmarkBlocker.browserCheckpoint.bgiBenchmarkEvidence.thresholdEvaluationCount}` +
  "-versioned-thresholds-failed";
assert.deepEqual(validateCurrentCheckpoint(structuredBenchmarkBlocker), []);

const mismatchedBenchmarkCrossings = structuredClone(manifest);
mismatchedBenchmarkCrossings.browserCheckpoint.bgiBenchmarkEvidence.wasmBridgeCallsPerSession -= 1;
assert.ok(
  validateCurrentCheckpoint(mismatchedBenchmarkCrossings).some((error) =>
    error.includes("benchmark evidence"),
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

const detachedHeapAddress = structuredClone(manifest);
detachedHeapAddress.activeRuntimeCheckpoint.processAddressSpaceAllocator.heapBackingUsesAllocationId =
  false;
assert.ok(
  validateCurrentCheckpoint(detachedHeapAddress).some((error) =>
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
