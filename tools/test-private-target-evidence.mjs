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
  validatePrivateTargetEvidenceDescendantPaths,
  validatePrivateTargetEvidence,
} from "./private-target-evidence.mjs";
import { projectWaitableTimerPacingRequests } from "./private-target-report-shape.mjs";
import {
  formatManifestRuntimeEndpointIdentity,
  formatRuntimeEndpointSymbol,
} from "./runtime-endpoint-symbol.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
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
assert.equal(BROWSER_BGI_REPORT_SCHEMA_VERSION, 21);

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
  /exactly one full or compact pacing shape/u,
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
  const browser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  const policy = checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
  const capabilityCheckpoint =
    checkpoint.nextStructuredStop.kind === "capability-checkpoint";
  const outcome = capabilityCheckpoint
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
        return {
          ...structuredClone(checkpoint.nextStructuredStop.completion),
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
        checkpoint.browserCheckpoint.browserRuntimeCoreWasm.sha256,
        checkpoint.browserCheckpoint.browserRuntimeCoreWasm.bytes,
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
