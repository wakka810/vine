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
for (const api of manifest.activeRuntimeCheckpoint.genericDispatcherApis) {
  assert.ok(rendered.includes(`\`${api}\``));
}

const stale = structuredClone(manifest);
stale.activeRuntimeCheckpoint.genericDispatcherApiCount += 1;
assert.ok(
  validateCurrentCheckpoint(stale).some((error) => error.includes("must match")),
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

const exposedMessages = structuredClone(manifest);
exposedMessages.activeRuntimeCheckpoint.runtimeMessageQueue.mutableCollectionVisibility =
  "public";
assert.ok(
  validateCurrentCheckpoint(exposedMessages).some((error) =>
    error.includes("message subsystem boundary"),
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
