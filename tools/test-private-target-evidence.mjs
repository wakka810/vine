import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  BROWSER_BGI_REPORT_SCHEMA,
  BROWSER_BGI_REPORT_SCHEMA_VERSION,
} from "./browser-bgi-report-contract.mjs";
import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import {
  finalizePrivateTargetEvidence,
  PRIVATE_TARGET_EVIDENCE_SCHEMA,
  PRIVATE_TARGET_EVIDENCE_VERSION,
  projectPrivateTargetExpectedBrowserEvidence,
  validatePrivateTargetEvidenceDescendantPaths,
  validatePrivateTargetEvidence,
} from "./private-target-evidence.mjs";
import {
  projectCapabilityCheckpointCompletion,
  projectWaitableTimerPacingRequests,
} from "./private-target-report-shape.mjs";
import {
  formatManifestRuntimeEndpointIdentity,
  formatRuntimeEndpointSymbol,
} from "./runtime-endpoint-symbol.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const privateTargetRunner = fs.readFileSync(
  path.join(root, "tools", "run-private-target-check.mjs"),
  "utf8",
);
assert.match(
  privateTargetRunner,
  /if \(expected\.completion\.requiredEntries !== undefined\) \{[\s\S]*?actual\.releaseCallbackEntry\.depth,[\s\S]*?actual\.queueDepthAtDispatchEntry,[\s\S]*?if \(expected\.completion\.requiredCompletions !== undefined\) \{[\s\S]*?postReleasePeek[\s\S]*?actual\.peekMessagePostRelease\.sequence[\s\S]*?actual\.postReleaseQueueDepthAfter, 0[\s\S]*?actual\.defWindowProcARelease\.sequence[\s\S]*?actual\.releaseCallbackReturn\.frameId,[\s\S]*?actual\.activeCallbackDepthAtStop, 0[\s\S]*?\} else \{[\s\S]*?actual\.activeCallbackDepthAtStop, 1[\s\S]*?\}[\s\S]*?\} else if \(expected\.completion\.requiredPointerMessages !== undefined\)/u,
);
assert.match(
  privateTargetRunner,
  /nodeExecution\.presentations\.thirdPresentReached,\s*scheduled\.nodePrimaryEvidence\.thirdPresentReached/u,
  "private target gate must derive third-Present acceptance from the active manifest",
);
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const activeCapability = checkpoint.nextStructuredStop.completion?.capability;
const postInputScanOrScan = [
  "user32-peek-message-a-post-window-timer-callback-continuation",
  "user32-window-timer-callback-post-waitable-timer-cycle",
  "d3d9-post-input-scan-waitable-timer-cycle",
  "d3d9-test-cooperative-level-post-input-scan-continuation",
  "user32-get-async-key-state-post-is-iconic-clock-continuation",
].includes(activeCapability);
const executionEngineProfile = JSON.parse(
  fs.readFileSync(
    path.join(root, checkpoint.browserCheckpoint.executionEngineProfile),
    "utf8",
  ),
);
const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
const executionEngineArtifact = executionEngineProfile.artifacts.find(
  ({ bytes, sha256 }) =>
    bytes === benchmark.v86AdapterWasmBytes &&
    sha256 === benchmark.v86AdapterWasmSha256,
);
assert(
  executionEngineArtifact !== undefined,
  "checkpoint adapter artifact is not in the execution engine profile",
);
const executionEngineIdentity = {
  schema: executionEngineProfile.schema,
  schemaVersion: executionEngineProfile.schemaVersion,
  engine: executionEngineProfile.engine,
  sourceRevision: executionEngineProfile.sourceRevision,
  cpuContextAbi: structuredClone(executionEngineProfile.cpuContextAbi),
  artifact: {
    bytes: executionEngineArtifact.bytes,
    sha256: executionEngineArtifact.sha256,
  },
};
const sha = (character) => character.repeat(64);
const artifact = (name, digest = sha("a"), bytes = 1) => ({
  path: `.work/${name}`,
  bytes,
  sha256: digest,
});

assert.equal(BROWSER_BGI_REPORT_SCHEMA, "vine.browser-bgi-runtime-worker");
assert.equal(BROWSER_BGI_REPORT_SCHEMA_VERSION, 87);

const firstPacingRequest = { requestId: "3", sequence: 18_682 };
const lastPacingRequest = { requestId: "4708", sequence: 89_218 };
assert.deepEqual(
  projectWaitableTimerPacingRequests({
    pacingRequests: [firstPacingRequest, lastPacingRequest],
  }),
  {
    totalCount: 2,
    sampledRequests: [firstPacingRequest, lastPacingRequest],
  },
);
assert.deepEqual(
  projectWaitableTimerPacingRequests({
    pacingRequestCount: 4_706,
    firstPacingRequest,
    lastPacingRequest,
  }),
  {
    totalCount: 4_706,
    sampledRequests: [firstPacingRequest, lastPacingRequest],
  },
);
assert.deepEqual(
  projectWaitableTimerPacingRequests({
    count: 4_706,
    first: firstPacingRequest,
    last: lastPacingRequest,
  }),
  {
    totalCount: 4_706,
    sampledRequests: [firstPacingRequest, lastPacingRequest],
  },
);
assert.deepEqual(
  projectWaitableTimerPacingRequests({
    pacingRequestCount: 1,
    firstPacingRequest,
    lastPacingRequest: structuredClone(firstPacingRequest),
  }),
  {
    totalCount: 1,
    sampledRequests: [firstPacingRequest],
  },
);
assert.throws(
  () =>
    projectWaitableTimerPacingRequests({
      pacingRequests: [],
      pacingRequestCount: 0,
      firstPacingRequest: null,
      lastPacingRequest: null,
    }),
  /exactly one full, compact, or scheduled shape/u,
);
assert.throws(
  () =>
    projectWaitableTimerPacingRequests({
      pacingRequestCount: 1,
      firstPacingRequest,
      lastPacingRequest,
    }),
  /matching first and last samples/u,
);

const callBackedCompletion = {
  capability: "user32-set-focus-action-continuation",
  requiredCompletions: 1,
  completionCount: 1,
  setFocus: {
    sequence: 161_946,
    trapInstructionCount: 321_462_324,
    runtimeThreadId: 8_192,
    window: { ownerThreadId: 8_192 },
  },
};
assert.deepEqual(projectCapabilityCheckpointCompletion(callBackedCompletion), {
  ...callBackedCompletion,
  lastCompletionSequence: 161_946,
  lastCompletionInstructionCount: 321_462_324,
});
assert.equal(
  Object.hasOwn(callBackedCompletion, "lastCompletionSequence"),
  false,
);
const wrongOwnerCompletion = structuredClone(callBackedCompletion);
wrongOwnerCompletion.setFocus.window.ownerThreadId = 8_193;
assert.throws(
  () => projectCapabilityCheckpointCompletion(wrongOwnerCompletion),
  /requires the SetFocus owner thread/u,
);

const expectedBrowserEvidence =
  projectPrivateTargetExpectedBrowserEvidence(checkpoint);
if (
  checkpoint.activeRuntimeCheckpoint.scheduledSharedClock?.status === "passed"
) {
  const scheduled = checkpoint.activeRuntimeCheckpoint.scheduledSharedClock;
  assert.equal(
    expectedBrowserEvidence.callSemanticsSha256,
    scheduled.nodePrimaryEvidence.supportedCallSemanticsSha256,
  );
  assert.equal(
    expectedBrowserEvidence.nodeFinalGeneration,
    scheduled.nodePrimaryEvidence.finalMemoryGeneration,
  );
  assert.equal(
    expectedBrowserEvidence.finalGeneration,
    scheduled.browserBoundaryEvidence.privatePrimaryFinalMemoryGeneration,
  );
  assert.equal(
    expectedBrowserEvidence.runtimeCoreWasmSha256,
    scheduled.browserBoundaryEvidence.runtimeCoreWasmSha256,
  );
}

assert.equal(
  formatRuntimeEndpointSymbol({ name: "DirectSoundCreate8" }),
  "DirectSoundCreate8",
);
assert.equal(
  formatRuntimeEndpointSymbol({
    symbol: { kind: "name", name: "DirectInput8Create" },
  }),
  "DirectInput8Create",
);
assert.equal(
  formatRuntimeEndpointSymbol({ symbol: { kind: "ordinal", ordinal: 11 } }),
  "#11",
);
assert.throws(
  () =>
    formatRuntimeEndpointSymbol({
      symbol: { kind: "ordinal", ordinal: 0x1_0000 },
    }),
  /unsigned 16-bit integer/u,
);
assert.equal(
  formatManifestRuntimeEndpointIdentity({ api: "kernel32.dll!Sleep" }),
  "kernel32.dll!Sleep",
);
assert.equal(
  formatManifestRuntimeEndpointIdentity({
    api: "IDirect3DDevice9::GetRasterStatus",
    observedModule: "<com>",
    observedSymbol: "interface-1-slot-19",
  }),
  "<com>!interface-1-slot-19",
);
assert.throws(
  () =>
    formatManifestRuntimeEndpointIdentity({
      api: "IDirect3DDevice9::GetRasterStatus",
      observedModule: "<com>",
    }),
  /requires a non-empty module and symbol/u,
);

function validEvidence() {
  const browser = projectPrivateTargetExpectedBrowserEvidence(checkpoint);
  const policy = checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
  const capabilityCheckpoint =
    checkpoint.nextStructuredStop.kind === "capability-checkpoint";
  const completedCapabilityBeforeStructuredStop =
    checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint ===
      "user32-get-cursor-info-post-outline-text-metrics-224x112" &&
    checkpoint.nextStructuredStop.kind === "com-method" &&
    checkpoint.nextStructuredStop.completion !== undefined;
  const outcome =
    capabilityCheckpoint || completedCapabilityBeforeStructuredStop
    ? "checkpoint-complete"
    : "structured-stop";
  const checkpointCompletion = capabilityCheckpoint
    ? (() => {
        const endpointIdentity = formatManifestRuntimeEndpointIdentity(
          checkpoint.nextStructuredStop.nextEndpoint,
        );
        const separator = endpointIdentity.indexOf("!");
        assert(
          separator > 0,
          "checkpoint endpoint identity must include a module",
        );
        const manifestCompletion = structuredClone(
          checkpoint.nextStructuredStop.completion ??
            checkpoint.activeRuntimeCheckpoint.latestEvidence
              .capabilityCheckpointCompletion,
        );
        const { dispatchSequence, dispatchInstructionCount, ...completion } =
          manifestCompletion;
        return {
          ...completion,
          ...(dispatchSequence === undefined
            ? {}
            : {
                dispatchEntry: {
                  sequence: dispatchSequence,
                  trapInstructionCount: dispatchInstructionCount,
                },
              }),
          instructionCount: checkpoint.nextStructuredStop.instructionCount,
          nextEndpoint: {
            id: checkpoint.nextStructuredStop.nextEndpoint.endpointId,
            address: Number.parseInt(
              checkpoint.nextStructuredStop.nextEndpoint.endpointAddress,
              16,
            ),
            module: endpointIdentity.slice(0, separator),
            symbol: {
              kind: "name",
              name: endpointIdentity.slice(separator + 1),
            },
          },
          memoryContentSha256: browser.memoryContentSha256,
        };
      })()
    : null;
  return finalizePrivateTargetEvidence({
    schema: PRIVATE_TARGET_EVIDENCE_SCHEMA,
    schemaVersion: PRIVATE_TARGET_EVIDENCE_VERSION,
    status: "passed",
    recordedAt: "2026-07-13T00:00:00.000Z",
    repository: {
      implementationCommit: "1".repeat(40),
      implementationTree: "2".repeat(40),
      headCommit: "1".repeat(40),
      dirty: true,
      dirtyPaths: ["local-notes.md"],
      allowedDirtyPaths: ["local-notes.md"],
      executionInputsClean: true,
      disallowedDirtyPaths: [],
    },
    command: {
      canonical: "npm run check:private-target",
      invocation:
        'npm run check:private-target -- --allow-dirty "local-notes.md"',
      semanticGate: "required",
      performanceGate: "separate-observation",
    },
    source: {
      fileName: "BGI.exe",
      bytes: browser.sourceBytes,
      sha256: browser.sourceSha256,
    },
    profile: {
      currentCheckpointSchema: checkpoint.schema,
      currentCheckpointSchemaVersion: checkpoint.schemaVersion,
      runtimeCheckpoint:
        checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
      runtimeProfile: checkpoint.activeRuntimeCheckpoint.profile,
      policySchema: policy.schema,
      policySchemaVersion: policy.schemaVersion,
      policyId: policy.id,
      policySource: policy.source,
      nodeReportSchema: "vine.test-private-target",
      nodeReportSchemaVersion: 1,
      browserRuntimeProtocol:
        checkpoint.browserCheckpoint.browserRuntimeProtocol,
      executionEngineProfile:
        checkpoint.browserCheckpoint.executionEngineProfile,
    },
    environment: {
      platform: "win32-test",
      osRelease: "test",
      architecture: "x64",
      node: "v24.0.0",
      rustc: "rustc test",
      cargo: "cargo test",
      chrome: "150.0.0.0",
      llvm: {
        clang: "clang test",
        wasmClang: "wasm clang test",
        lldLink: "lld-link test",
        llvmReadobj: "llvm-readobj test",
      },
      executionEngine: {
        engine: executionEngineIdentity.engine,
        sourceRevision: executionEngineIdentity.sourceRevision,
        cpuContextAbi: structuredClone(executionEngineIdentity.cpuContextAbi),
        adapterArtifact: structuredClone(executionEngineIdentity.artifact),
      },
    },
    semantic: {
      outcome: "passed",
      node: {
        outcome,
        implementedApiContracts:
          checkpoint.activeRuntimeCheckpoint.genericDispatcherContractCount,
        supportedCalls:
          checkpoint.activeRuntimeCheckpoint
            .genericDispatcherCallOccurrenceCount,
        callSemanticsSha256: browser.callSemanticsSha256,
        instructions: checkpoint.nextStructuredStop.instructionCount,
        finalGeneration: browser.nodeFinalGeneration,
        memoryContentSha256: browser.memoryContentSha256,
        ...(checkpointCompletion === null
          ? {}
          : { checkpointCompletion: structuredClone(checkpointCompletion) }),
        writebackBatches: 1,
        dirtyPages: 1,
        changedBytes: 1,
        hostDeltaBatches: 1,
      },
      browser: {
        outcome,
        supportedCalls:
          checkpoint.activeRuntimeCheckpoint
            .genericDispatcherCallOccurrenceCount,
        callSemanticsSha256: browser.callSemanticsSha256,
        instructions: checkpoint.nextStructuredStop.instructionCount,
        finalGeneration: browser.finalGeneration,
        memoryContentSha256: browser.memoryContentSha256,
        ...(checkpointCompletion === null
          ? {}
          : { checkpointCompletion: structuredClone(checkpointCompletion) }),
        wasmBridgeCalls: 1,
        executionStateMachine: structuredClone(browser.executionStateMachine),
        executionEngine: structuredClone(executionEngineIdentity),
        executionEngineLinearMemoryBytes:
          browser.executionEngineLinearMemoryBytes,
      },
      memoryEquivalent: true,
      nextStructuredStop: structuredClone(checkpoint.nextStructuredStop),
      cleanup: {
        pendingHostCalls: 0,
        pendingCallbacks: 0,
        cancelInstructions: browser.cancelInstructions,
        cancelPendingHostCalls: 0,
        cancelPendingCallbacks: 0,
        watchdogInstructions: browser.watchdogInstructions,
        watchdogPendingHostCalls: 0,
        watchdogPendingCallbacks: 0,
      },
    },
    artifacts: {
      nodeReport: artifact("node-report.json"),
      browserReport: artifact("browser-report.json"),
      toolchainReport: artifact("toolchain-report.json"),
      v86AdapterReport: artifact("adapter-report.json"),
      browserBuildManifest: artifact("build-manifest.json"),
      runtimeCoreWasm: artifact(
        "runtime-core.wasm",
        browser.runtimeCoreWasmSha256,
        browser.runtimeCoreWasmBytes,
      ),
      v86AdapterWasm: artifact(
        "v86-adapter.wasm",
        benchmark.v86AdapterWasmSha256,
        benchmark.v86AdapterWasmBytes,
      ),
      cargoLock: artifact("Cargo.lock"),
      npmLock: artifact("package-lock.json"),
    },
    performanceObservation: {
      classification: "observation-not-semantic-gate",
      status: checkpoint.browserCheckpoint.bgiBenchmark,
      configuration: benchmark.configuration,
      benchmarkClassification: benchmark.performanceClassification,
      decisionStatus: benchmark.decisionStatus,
      decisionReason: benchmark.decisionReason,
      thresholdEvaluationCount: benchmark.thresholdEvaluationCount,
      durationMilliseconds: benchmark.durationMilliseconds,
      completedSessions: benchmark.completedSessions,
      failedSessions: benchmark.failedSessions,
      thresholdFailureIds: benchmark.thresholdFailureIds,
      reportSha256: benchmark.reportSha256,
    },
  });
}

function mutated(mutate) {
  const evidence = validEvidence();
  delete evidence.payloadSha256;
  mutate(evidence);
  return finalizePrivateTargetEvidence(evidence);
}

assert.deepEqual(
  validatePrivateTargetEvidence(validEvidence(), checkpoint),
  [],
);
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation"
) {
  const projectedBrowserWrite = validEvidence();
  delete projectedBrowserWrite.payloadSha256;
  delete projectedBrowserWrite.semantic.browser.checkpointCompletion
    .getWindowRectPostRelease.guestWrites[0].result;
  assert.deepEqual(
    validatePrivateTargetEvidence(
      finalizePrivateTargetEvidence(projectedBrowserWrite),
      checkpoint,
    ),
    [],
  );

  const missingNodeWriteReceipt = mutated((value) => {
    delete value.semantic.node.checkpointCompletion.getWindowRectPostRelease
      .guestWrites[0].result;
  });
  assert(
    validatePrivateTargetEvidence(missingNodeWriteReceipt, checkpoint).some(
      (error) =>
        error.includes("getWindowRectPostRelease.guestWrites[0].result"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation"
) {
  const projectedBrowserWrite = validEvidence();
  delete projectedBrowserWrite.payloadSha256;
  delete projectedBrowserWrite.semantic.browser.checkpointCompletion
    .getCursorPosPostRelease.guestWrites[0].result;
  assert.deepEqual(
    validatePrivateTargetEvidence(
      finalizePrivateTargetEvidence(projectedBrowserWrite),
      checkpoint,
    ),
    [],
  );

  const missingNodeWriteReceipt = mutated((value) => {
    delete value.semantic.node.checkpointCompletion.getCursorPosPostRelease
      .guestWrites[0].result;
  });
  assert(
    validatePrivateTargetEvidence(missingNodeWriteReceipt, checkpoint).some(
      (error) =>
        error.includes("getCursorPosPostRelease.guestWrites[0].result"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-peek-message-a-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-pointer-release-repeat-batch" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseQueue = mutated((value) => {
    value.semantic.node.checkpointCompletion.postReleaseQueueDepthAfter = 1;
  });
  assert(
    validatePrivateTargetEvidence(stalePostReleaseQueue, checkpoint).some(
      (error) => error.includes("postReleaseQueueDepthAfter"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-pointer-release-repeat-batch" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseTick = mutated((value) => {
    value.semantic.browser.checkpointCompletion.timeGetTimePostRelease.tickCount.nextTickCount =
      3_601_224;
  });
  assert(
    validatePrivateTargetEvidence(stalePostReleaseTick, checkpoint).some(
      (error) => error.includes("timeGetTimePostRelease.tickCount.nextTickCount"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseRect = mutated((value) => {
    value.semantic.node.checkpointCompletion.getWindowRectPostRelease.guestWrites[0].bytes[0] =
      62;
  });
  assert(
    validatePrivateTargetEvidence(stalePostReleaseRect, checkpoint).some(
      (error) => error.includes("getWindowRectPostRelease.guestWrites[0].bytes"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleasePoint = mutated((value) => {
    value.semantic.node.checkpointCompletion.getCursorPosPostRelease.guestWrites[0].bytes[0] =
      191;
  });
  assert(
    validatePrivateTargetEvidence(stalePostReleasePoint, checkpoint).some(
      (error) => error.includes("getCursorPosPostRelease.guestWrites[0].bytes"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseMetric = mutated((value) => {
    value.semantic.node.checkpointCompletion.getSystemMetricsPostRelease.result.value =
      4;
  });
  assert(
    validatePrivateTargetEvidence(stalePostReleaseMetric, checkpoint).some(
      (error) => error.includes("getSystemMetricsPostRelease.result.value"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseCaptionMetric = mutated((value) => {
    value.semantic.node.checkpointCompletion.getSystemMetricsCaptionPostRelease.result.value =
      24;
  });
  assert(
    validatePrivateTargetEvidence(
      stalePostReleaseCaptionMetric,
      checkpoint,
    ).some((error) =>
      error.includes("getSystemMetricsCaptionPostRelease.result.value"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-is-iconic-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-is-iconic-continuation" ||
  postInputScanOrScan
) {
  const stalePostReleaseFixedFrameHeightMetric = mutated((value) => {
    value.semantic.node.checkpointCompletion.getSystemMetricsFixedFrameHeightPostRelease.result.value =
      4;
  });
  assert(
    validatePrivateTargetEvidence(
      stalePostReleaseFixedFrameHeightMetric,
      checkpoint,
    ).some((error) =>
      error.includes(
        "getSystemMetricsFixedFrameHeightPostRelease.result.value",
      ),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "user32-is-iconic-post-pointer-release-continuation" ||
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-is-iconic-continuation" ||
  postInputScanOrScan
) {
  const staleIsIconicResult = mutated((value) => {
    value.semantic.node.checkpointCompletion.isIconicPostRelease.result.value =
      1;
  });
  assert(
    validatePrivateTargetEvidence(staleIsIconicResult, checkpoint).some(
      (error) => error.includes("isIconicPostRelease.result.value"),
    ),
  );
}
if (
  checkpoint.nextStructuredStop.completion.capability ===
    "winmm-time-get-time-post-is-iconic-continuation" ||
  postInputScanOrScan
) {
  const staleTimeGetTimeTick = mutated((value) => {
    value.semantic.node.checkpointCompletion.timeGetTimePostIsIconic.tickCount.nextTickCount =
      3_601_230;
  });
  assert(
    validatePrivateTargetEvidence(staleTimeGetTimeTick, checkpoint).some(
      (error) =>
        error.includes("timeGetTimePostIsIconic.tickCount.nextTickCount"),
    ),
  );
}
if (
  postInputScanOrScan
) {
  const staleScanOrder = mutated((value) => {
    value.semantic.browser.checkpointCompletion.scanVirtualKeys[0] = 15;
  });
  assert(
    validatePrivateTargetEvidence(staleScanOrder, checkpoint).some((error) =>
      error.includes("getAsyncKeyStatePostIsIconicClock.scanVirtualKeys"),
    ),
  );
  const staleLastScanResult = mutated((value) => {
    value.semantic.node.checkpointCompletion.getAsyncKeyStatePostIsIconicClockLast.result.value =
      1;
  });
  assert(
    validatePrivateTargetEvidence(staleLastScanResult, checkpoint).some(
      (error) =>
        error.includes("getAsyncKeyStatePostIsIconicClock.last.result.value"),
    ),
  );
  if (
    [
      "user32-peek-message-a-post-window-timer-callback-continuation",
      "user32-window-timer-callback-post-waitable-timer-cycle",
      "d3d9-post-input-scan-waitable-timer-cycle",
      "d3d9-test-cooperative-level-post-input-scan-continuation",
    ].includes(activeCapability)
  ) {
    const staleTestCooperativeLevelResult = mutated((value) => {
      value.semantic.browser.checkpointCompletion.d3d9TestCooperativeLevelPostInputScan.result.value =
        0x88760868;
    });
    assert(
      validatePrivateTargetEvidence(
        staleTestCooperativeLevelResult,
        checkpoint,
      ).some((error) =>
        error.includes(
          "d3d9TestCooperativeLevelPostInputScan.result.value",
        ),
      ),
    );
  }
  if (
    [
      "user32-peek-message-a-post-window-timer-callback-continuation",
      "user32-window-timer-callback-post-waitable-timer-cycle",
      "d3d9-post-input-scan-waitable-timer-cycle",
    ].includes(activeCapability)
  ) {
    const staleTimerArmGeneration = mutated((value) => {
      value.semantic.browser.checkpointCompletion.timerArm.waitableTimer.timer.arm.generation =
        8;
    });
    assert(
      validatePrivateTargetEvidence(staleTimerArmGeneration, checkpoint).some(
        (error) =>
          error.includes(
            "timerArm.waitableTimer.timer.arm.generation",
          ),
      ),
    );
    const staleNextWaitRegistration = mutated((value) => {
      value.semantic.node.checkpointCompletion.timerWait.waitableTimer.nextWaitRegistrationId =
        11_474;
    });
    assert(
      validatePrivateTargetEvidence(
        staleNextWaitRegistration,
        checkpoint,
      ).some((error) =>
        error.includes("timerWait.waitableTimer.nextWaitRegistrationId"),
      ),
    );
  }
  if (
    [
      "user32-peek-message-a-post-window-timer-callback-continuation",
      "user32-window-timer-callback-post-waitable-timer-cycle",
    ].includes(activeCapability)
  ) {
    const staleWindowTimerDelivery = mutated((value) => {
      value.semantic.browser.checkpointCompletion.windowTimerPeek.windowTimer.deliveryCount =
        2;
    });
    assert(
      validatePrivateTargetEvidence(
        staleWindowTimerDelivery,
        checkpoint,
      ).some((error) =>
        error.includes("windowTimerPeek.windowTimer.deliveryCount"),
      ),
    );
    const staleCallbackFrame = mutated((value) => {
      value.semantic.node.checkpointCompletion.windowTimerDispatch.callback.frameId =
        34;
    });
    assert(
      validatePrivateTargetEvidence(staleCallbackFrame, checkpoint).some(
        (error) => error.includes("windowTimerDispatch.callback.frameId"),
      ),
    );
    const staleNestedTick = mutated((value) => {
      value.semantic.browser.checkpointCompletion.windowTimerCallbackNestedCalls[0].tickCount.nextTickCount =
        3_601_229;
    });
    assert(
      validatePrivateTargetEvidence(staleNestedTick, checkpoint).some(
        (error) =>
          error.includes(
            "windowTimerCallbackNestedCalls[0].tickCount.nextTickCount",
          ),
      ),
    );
    if (
      activeCapability ===
      "user32-peek-message-a-post-window-timer-callback-continuation"
    ) {
      const mutatedPostCallbackPeek = mutated((value) => {
        value.semantic.node.checkpointCompletion.peekMessagePostWindowTimerCallback.result.value =
          1;
      });
      assert(
        validatePrivateTargetEvidence(
          mutatedPostCallbackPeek,
          checkpoint,
        ).some((error) =>
          error.includes(
            "peekMessagePostWindowTimerCallback.result.value",
          ),
        ),
      );
    }
  }
}
if (
  checkpoint.nextStructuredStop.kind === "capability-checkpoint" &&
  checkpoint.nextStructuredStop.completion.requiredEntries !== undefined &&
  checkpoint.nextStructuredStop.completion.requiredCompletions === undefined
) {
  const inheritedCompletionEvidence = validEvidence();
  for (const result of [
    inheritedCompletionEvidence.semantic.node,
    inheritedCompletionEvidence.semantic.browser,
  ]) {
    Object.assign(result.checkpointCompletion, {
      requiredCompletions: 1,
      completionCount: 1,
      lastCompletionSequence: 161_950,
      lastCompletionInstructionCount: 321_462_431,
    });
  }
  delete inheritedCompletionEvidence.payloadSha256;
  assert.deepEqual(
    validatePrivateTargetEvidence(
      finalizePrivateTargetEvidence(inheritedCompletionEvidence),
      checkpoint,
    ),
    [],
  );
}
assert.deepEqual(
  validatePrivateTargetEvidenceDescendantPaths([
    "docs/private-target-evidence.json",
  ]),
  [],
);
for (const disguisedPath of [
  "docs\\private-target-evidence.json",
  "\ndocs/private-target-evidence.json",
]) {
  assert.match(
    validatePrivateTargetEvidenceDescendantPaths([disguisedPath])[0],
    /evidence is stale/u,
  );
}
assert.match(
  validatePrivateTargetEvidenceDescendantPaths([
    "browser/runtime-worker/runtime-worker.ts",
  ])[0],
  /evidence is stale.*runtime-worker\.ts/u,
);
assert.match(
  validatePrivateTargetEvidenceDescendantPaths([
    "docs/current-checkpoint.json",
  ])[0],
  /evidence is stale.*current-checkpoint\.json/u,
);

const tamperedDigest = validEvidence();
tamperedDigest.semantic.node.supportedCalls += 1;
assert(
  validatePrivateTargetEvidence(tamperedDigest, checkpoint).some((error) =>
    error.includes("payloadSha256"),
  ),
);

for (const [label, evidence, expected] of [
  [
    "source mismatch",
    mutated((value) => {
      value.source.sha256 = sha("b");
    }),
    "evidence.source.sha256",
  ],
  [
    "semantic mismatch",
    mutated((value) => {
      value.semantic.browser.supportedCalls -= 1;
    }),
    "evidence.semantic.browser.supportedCalls",
  ],
  [
    "call semantics mismatch",
    mutated((value) => {
      value.semantic.browser.callSemanticsSha256 = sha("d");
    }),
    "semantic.callSemanticsSha256",
  ],
  [
    "checkpoint call semantics mismatch",
    mutated((value) => {
      value.semantic.node.callSemanticsSha256 = sha("c");
      value.semantic.browser.callSemanticsSha256 = sha("c");
    }),
    "checkpoint digest",
  ],
  [
    "runtime core Wasm artifact mismatch",
    mutated((value) => {
      value.artifacts.runtimeCoreWasm.sha256 = sha("runtime-core-mismatch");
      value.artifacts.runtimeCoreWasm.bytes += 1;
    }),
    "evidence.artifacts.runtimeCoreWasm",
  ],
  [
    "structured stop mismatch",
    mutated((value) => {
      value.semantic.nextStructuredStop.reason = "test-mismatch";
    }),
    "evidence.semantic.nextStructuredStop.reason",
  ],
  [
    "dirty execution input",
    mutated((value) => {
      value.repository.executionInputsClean = false;
      value.repository.disallowedDirtyPaths = [
        "crates/vine-core/src/runtime.rs",
      ];
    }),
    "executionInputsClean",
  ],
  [
    "pending host work",
    mutated((value) => {
      value.semantic.cleanup.pendingHostCalls = 1;
    }),
    "pendingHostCalls",
  ],
  [
    "unfinished browser execution lane",
    mutated((value) => {
      value.semantic.browser.executionStateMachine.success.finalState =
        "running";
    }),
    "executionStateMachine.success.finalState",
  ],
  [
    "missing watchdog context",
    mutated((value) => {
      value.semantic.browser.executionStateMachine.watchdog.watchdogContext =
        null;
    }),
    "watchdogContext",
  ],
  [
    "execution engine revision mismatch",
    mutated((value) => {
      value.semantic.browser.executionEngine.sourceRevision = "0".repeat(40);
    }),
    "executionEngine.sourceRevision",
  ],
  [
    "performance promoted to semantic gate",
    mutated((value) => {
      value.performanceObservation.classification = "semantic-gate";
    }),
    "observation-not-semantic-gate",
  ],
  [
    "benchmark decision reason mismatch",
    mutated((value) => {
      value.performanceObservation.decisionReason = "single-sample-passed";
    }),
    "performanceObservation.decisionReason",
  ],
]) {
  assert(
    validatePrivateTargetEvidence(evidence, checkpoint).some((error) =>
      error.includes(expected),
    ),
    `${label} was not rejected`,
  );
}

console.log("private target evidence contract passed");
