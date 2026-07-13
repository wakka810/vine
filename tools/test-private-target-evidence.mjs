import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import {
  finalizePrivateTargetEvidence,
  PRIVATE_TARGET_EVIDENCE_SCHEMA,
  PRIVATE_TARGET_EVIDENCE_VERSION,
  validatePrivateTargetEvidence,
} from "./private-target-evidence.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const sha = (character) => character.repeat(64);
const artifact = (name, digest = sha("a"), bytes = 1) => ({
  path: `.work/${name}`,
  bytes,
  sha256: digest,
});

function validEvidence() {
  const browser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
  const policy = checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
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
      dirtyPaths: ["review.md"],
      allowedDirtyPaths: ["review.md"],
      executionInputsClean: true,
      disallowedDirtyPaths: [],
    },
    command: {
      canonical: "npm run check:private-target",
      invocation: "npm run check:private-target -- --allow-dirty \"review.md\"",
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
      runtimeCheckpoint: checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
      runtimeProfile: checkpoint.activeRuntimeCheckpoint.profile,
      policySchema: policy.schema,
      policySchemaVersion: policy.schemaVersion,
      policyId: policy.id,
      policySource: policy.source,
      nodeReportSchema: "vine.test-private-target",
      nodeReportSchemaVersion: 1,
      browserRuntimeProtocol: checkpoint.browserCheckpoint.browserRuntimeProtocol,
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
      v86: {
        sourceRevision: "test-revision",
        stateAbiVersion: 1,
        stateAbiBytes: 640,
      },
    },
    semantic: {
      outcome: "passed",
      node: {
        outcome: "structured-stop",
        implementedApiContracts: checkpoint.activeRuntimeCheckpoint.genericDispatcherApiCount,
        supportedCalls: checkpoint.activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount,
        instructions: checkpoint.nextStructuredStop.instructionCount,
        finalGeneration: browser.nodeFinalGeneration,
        memoryContentSha256: browser.memoryContentSha256,
        writebackBatches: 1,
        dirtyPages: 1,
        changedBytes: 1,
        hostDeltaBatches: 1,
      },
      browser: {
        outcome: "structured-stop",
        supportedCalls: checkpoint.activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount,
        instructions: checkpoint.nextStructuredStop.instructionCount,
        finalGeneration: browser.finalGeneration,
        memoryContentSha256: browser.memoryContentSha256,
        wasmBridgeCalls: 1,
        executionStateMachine: structuredClone(browser.executionStateMachine),
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
      ),
      cargoLock: artifact("Cargo.lock"),
      npmLock: artifact("package-lock.json"),
    },
    performanceObservation: {
      classification: "observation-not-semantic-gate",
      status: checkpoint.browserCheckpoint.bgiBenchmark,
      configuration: benchmark.configuration,
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

assert.deepEqual(validatePrivateTargetEvidence(validEvidence(), checkpoint), []);

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
    "dirty execution input",
    mutated((value) => {
      value.repository.executionInputsClean = false;
      value.repository.disallowedDirtyPaths = ["crates/vine-core/src/runtime.rs"];
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
      value.semantic.browser.executionStateMachine.success.finalState = "running";
    }),
    "executionStateMachine.success.finalState",
  ],
  [
    "missing watchdog context",
    mutated((value) => {
      value.semantic.browser.executionStateMachine.watchdog.watchdogContext = null;
    }),
    "watchdogContext",
  ],
  [
    "performance promoted to semantic gate",
    mutated((value) => {
      value.performanceObservation.classification = "semantic-gate";
    }),
    "observation-not-semantic-gate",
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
