import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import {
  assertValidPrivateTargetEvidence,
  finalizePrivateTargetEvidence,
  PRIVATE_TARGET_EVIDENCE_SCHEMA,
  PRIVATE_TARGET_EVIDENCE_VERSION,
} from "./private-target-evidence.mjs";
import { formatRuntimeEndpointSymbol } from "./runtime-endpoint-symbol.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultOutput = path.join(root, "docs", "private-target-evidence.json");
const maximumSemanticReportBytes = 2 * 1024 * 1024;
const defaultBgi =
  process.platform === "win32" ? "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe" : null;
const protectedInputs = [
  "browser/",
  "crates/",
  "fixtures/",
  "tools/",
  "Cargo.lock",
  "Cargo.toml",
  "package-lock.json",
  "package.json",
  "rust-toolchain.toml",
  "docs/current-checkpoint.json",
];

function fail(message) {
  throw new Error(`private target gate failed: ${message}`);
}

function normalizeRepositoryPath(value) {
  const relative = path.relative(root, path.resolve(root, value)).replaceAll("\\", "/");
  if (relative === "" || relative === ".." || relative.startsWith("../")) {
    fail(`path is outside the repository: ${value}`);
  }
  return relative;
}

function parseArguments(arguments_) {
  let output = defaultOutput;
  const allowedDirtyPaths = [];
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--output" && index + 1 < arguments_.length) {
      output = path.resolve(root, arguments_[index + 1]);
      index += 1;
    } else if (argument === "--allow-dirty" && index + 1 < arguments_.length) {
      allowedDirtyPaths.push(normalizeRepositoryPath(arguments_[index + 1]));
      index += 1;
    } else {
      fail(`unsupported argument: ${argument}`);
    }
  }
  return {
    output,
    outputRelative: normalizeRepositoryPath(output),
    allowedDirtyPaths: [...new Set(allowedDirtyPaths)].sort(),
  };
}

function capture(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    encoding: options.encoding ?? "utf8",
    windowsHide: true,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail(
      `${command} ${arguments_.join(" ")} exited ${result.status}\n${result.stdout ?? ""}${result.stderr ?? ""}`.trim(),
    );
  }
  return result.stdout;
}

function runInherited(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail(`${command} ${arguments_.join(" ")} exited ${result.status}`);
  }
}

function git(arguments_, options = {}) {
  return capture("git", arguments_, options);
}

function splitNullTerminated(value) {
  return value
    .split("\0")
    .filter((entry) => entry.length > 0)
    .map((entry) => entry.replaceAll("\\", "/"));
}

function dirtyPaths() {
  const tracked = splitNullTerminated(git(["diff", "--name-only", "-z", "HEAD", "--"]));
  const untracked = splitNullTerminated(
    git(["ls-files", "--others", "--exclude-standard", "-z"]),
  );
  return [...new Set([...tracked, ...untracked])].sort();
}

function isProtectedInput(relative) {
  return protectedInputs.some((entry) =>
    entry.endsWith("/") ? relative.startsWith(entry) : relative === entry,
  );
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function artifact(file) {
  const bytes = fs.readFileSync(file);
  return {
    path: path.relative(root, file).replaceAll("\\", "/"),
    bytes: bytes.byteLength,
    sha256: sha256(bytes),
  };
}

function executionEngineIdentity(profile, actualArtifact) {
  assert(Array.isArray(profile.artifacts), "execution engine profile has no artifact allowlist");
  const matched = profile.artifacts.find(
    ({ bytes, sha256: digest }) =>
      bytes === actualArtifact.bytes && digest === actualArtifact.sha256,
  );
  assert(matched !== undefined, "execution engine artifact is not in the pinned profile");
  return {
    schema: profile.schema,
    schemaVersion: profile.schemaVersion,
    engine: profile.engine,
    sourceRevision: profile.sourceRevision,
    cpuContextAbi: structuredClone(profile.cpuContextAbi),
    artifact: {
      bytes: matched.bytes,
      sha256: matched.sha256,
    },
  };
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function firstLine(command, arguments_ = []) {
  return capture(command, arguments_).trim().split(/\r?\n/u)[0];
}

function hex32(value) {
  assert(Number.isSafeInteger(value) && value >= 0 && value <= 0xffff_ffff);
  return `0x${value.toString(16).padStart(8, "0")}`;
}

function optionalHex32(value) {
  return value === undefined ? undefined : hex32(value);
}

function executionStateEvidence(report) {
  const context = report.watchdogContext;
  return {
    finalState: report.executionState,
    transitionCount: report.executionTransitionCount,
    lastTransitionTo: report.lastExecutionTransition?.to ?? null,
    pendingHostRequestRecords: report.pendingHostRequestRecords,
    watchdogContext:
      context === null
        ? null
        : {
            threadId: context.threadId,
            requestId: context.requestId,
            lastTransition:
              context.lastTransition === null
                ? null
                : {
                    from: context.lastTransition.from,
                    to: context.lastTransition.to,
                    cause: context.lastTransition.cause,
                  },
          },
  };
}

function assertExecutionStateReport(report, scenario) {
  assert.equal(report.executionState, "stopped");
  assert(report.executionTransitionCount > 0, "execution state machine did not transition");
  assert.equal(report.lastExecutionTransition?.to, "stopped");
  assert.equal(report.pendingHostRequestRecords, 0);
  if (scenario === "watchdog") {
    assert(report.watchdogContext !== null, "watchdog context is missing");
    assert(Number.isSafeInteger(report.watchdogContext.threadId));
    assert(report.watchdogContext.threadId > 0);
    assert(report.watchdogContext.lastTransition !== null, "watchdog transition is missing");
  } else {
    assert.equal(report.watchdogContext, null, "non-watchdog report retained watchdog context");
  }
}

function assertReportSemantics({
  checkpoint,
  nodeReport,
  browserReport,
  policy,
  executionEngine,
  source,
}) {
  const active = checkpoint.activeRuntimeCheckpoint;
  const expectedBrowser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  const expectedNext = checkpoint.nextStructuredStop;
  const sourceIdentity = { bytes: source.bytes, sha256: source.sha256 };
  assert.deepEqual(nodeReport.source, sourceIdentity, "Node source fingerprint mismatch");
  assert.deepEqual(browserReport.source, sourceIdentity, "browser source fingerprint mismatch");
  assert.deepEqual(
    nodeReport.contract.compatibilityPolicy,
    policy,
    "runtime policy mismatch",
  );
  assert.equal(nodeReport.contract.enabledCapability, active.profile);
  assert.equal(nodeReport.contract.evidenceLevel, "summary");
  assert.equal(nodeReport.supportedContinuation.length, 0);
  assert.equal(nodeReport.supportedCallCount, active.genericDispatcherCallOccurrenceCount);
  assert.equal(nodeReport.supportedCallSemanticsAlgorithm, "sha256-canonical-json-v1");
  assert.match(nodeReport.supportedCallSemanticsSha256, /^[0-9a-f]{64}$/u);
  assert.equal(
    nodeReport.supportedCallSemanticsSha256,
    expectedBrowser.callSemanticsSha256,
  );
  assert.equal(nodeReport.process.implementedApiContracts, active.genericDispatcherApiCount);
  assert.equal(nodeReport.firstUnsupported.instructionCount, expectedNext.instructionCount);
  assert.equal(nodeReport.synchronization.finalGeneration, expectedBrowser.nodeFinalGeneration);
  assert.equal(
    nodeReport.firstUnsupported.memoryContentSha256,
    expectedBrowser.memoryContentSha256,
  );

  assert.equal(nodeReport.firstUnsupported.capabilityCluster, expectedNext.capabilityCluster);
  assert.equal(nodeReport.firstUnsupported.fault.code, expectedNext.reason);
  if (expectedNext.kind === "execution-fault") {
    assert.equal(nodeReport.firstUnsupported.endpoint, null);
    assert.equal(
      nodeReport.firstUnsupported.fault.instructionPointer,
      Number.parseInt(expectedNext.fault.instructionPointer, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.previousInstructionPointer,
      Number.parseInt(expectedNext.fault.previousInstructionPointer, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.faultAddress,
      Number.parseInt(expectedNext.fault.faultAddress, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.pageFaultErrorCode,
      expectedNext.fault.pageFaultErrorCode,
    );
  } else if (expectedNext.kind === "com-method") {
    const nodeEndpoint = nodeReport.firstUnsupported.endpoint;
    const nodeTarget = nodeEndpoint.target;
    assert.equal(nodeEndpoint.kind, "com-method");
    assert.equal(nodeTarget.interface.name, expectedNext.interface);
    assert.equal(nodeTarget.method.slot, expectedNext.vtableSlot);
    assert.equal(nodeTarget.method.name, expectedNext.method);
    assert.equal(hex32(nodeTarget.method.endpoint), expectedNext.endpointAddress);
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.stackWords[1]),
      expectedNext.thisPointer,
    );
    assert.deepEqual(
      nodeReport.firstUnsupported.terminal.stackWords
        .slice(2, 2 + expectedNext.arguments.length)
        .map(hex32),
      expectedNext.arguments,
    );
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.returnAddress),
      expectedNext.returnAddress,
    );
  } else {
    const nodeEndpoint = nodeReport.firstUnsupported.endpoint.endpoint;
    assert.equal(nodeReport.firstUnsupported.endpoint.kind, expectedNext.endpointKind);
    assert.equal(nodeEndpoint.id, expectedNext.endpointId);
    assert.equal(hex32(nodeEndpoint.address), expectedNext.endpointAddress);
    assert.equal(optionalHex32(nodeEndpoint.iatAddress), expectedNext.iatAddress);
    assert.equal(
      `${nodeEndpoint.module}!${formatRuntimeEndpointSymbol(
        nodeEndpoint,
      )}`.toLowerCase(),
      expectedNext.api.toLowerCase(),
    );
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.returnAddress),
      expectedNext.returnAddress,
    );
  }

  assert.equal(browserReport.runtimeProtocol, checkpoint.browserCheckpoint.browserRuntimeProtocol);
  assert.equal(browserReport.schema, "vine.browser-bgi-runtime-worker");
  assert.equal(browserReport.schemaVersion, 5);
  assert.equal(browserReport.success.report.outcome, "structured-stop");
  assert.equal(browserReport.success.report.evidenceLevel, "summary");
  assert.equal(
    browserReport.success.report.supportedCallCount,
    active.genericDispatcherCallOccurrenceCount,
  );
  assert.equal(browserReport.success.report.supportedCalls.length, 0);
  assert.equal(
    browserReport.success.report.supportedCallSemanticsAlgorithm,
    "sha256-canonical-json-v1",
  );
  assert.equal(
    browserReport.success.report.supportedCallSemanticsSha256,
    nodeReport.supportedCallSemanticsSha256,
    "Node/browser call semantics digest mismatch",
  );
  assert.equal(browserReport.success.report.instructions, expectedNext.instructionCount);
  assert.equal(browserReport.success.report.finalGeneration, expectedBrowser.finalGeneration);
  assert.equal(
    browserReport.success.report.firstUnsupported.memoryContentSha256,
    expectedBrowser.memoryContentSha256,
  );
  const browserUnsupported = browserReport.success.report.firstUnsupported;
  assert.equal(browserUnsupported.capabilityCluster, expectedNext.capabilityCluster);
  assert.equal(browserUnsupported.fault.code, expectedNext.reason);
  if (expectedNext.kind === "execution-fault") {
    assert.equal(browserUnsupported.endpoint, null);
    assert.equal(
      browserUnsupported.fault.instructionPointer,
      Number.parseInt(expectedNext.fault.instructionPointer, 16),
    );
    assert.equal(
      browserUnsupported.fault.previousInstructionPointer,
      Number.parseInt(expectedNext.fault.previousInstructionPointer, 16),
    );
    assert.equal(
      browserUnsupported.fault.faultAddress,
      Number.parseInt(expectedNext.fault.faultAddress, 16),
    );
    assert.equal(
      browserUnsupported.fault.pageFaultErrorCode,
      expectedNext.fault.pageFaultErrorCode,
    );
  } else if (expectedNext.kind === "com-method") {
    assert.equal(browserUnsupported.endpoint.kind, "com-method");
    assert.equal(
      hex32(browserUnsupported.endpoint.endpointAddress),
      expectedNext.endpointAddress,
    );
    assert.equal(browserUnsupported.endpoint.target.interface, expectedNext.interface);
    assert.equal(browserUnsupported.endpoint.target.vtableSlot, expectedNext.vtableSlot);
    assert.equal(browserUnsupported.endpoint.target.method, expectedNext.method);
    assert.equal(
      hex32(browserUnsupported.endpoint.target.endpointAddress),
      expectedNext.endpointAddress,
    );
    assert.equal(hex32(browserUnsupported.fault.callSite), expectedNext.returnAddress);
  } else {
    assert.equal(browserUnsupported.endpoint.id, expectedNext.endpointId);
    assert.equal(hex32(browserUnsupported.endpoint.address), expectedNext.endpointAddress);
    assert.equal(
      optionalHex32(browserUnsupported.endpoint.iatAddress),
      expectedNext.iatAddress,
    );
    assert.equal(
      `${browserUnsupported.endpoint.module}!${formatRuntimeEndpointSymbol(
        browserUnsupported.endpoint,
      )}`.toLowerCase(),
      expectedNext.api.toLowerCase(),
    );
  }
  assert.equal(browserReport.success.report.pendingHostCalls, 0);
  assert.equal(browserReport.success.report.pendingCallbacks, 0);
  assert.equal(browserReport.cancellation.report.pendingHostCalls, 0);
  assert.equal(browserReport.cancellation.report.pendingCallbacks, 0);
  assert.equal(browserReport.watchdog.report.pendingHostCalls, 0);
  assert.equal(browserReport.watchdog.report.pendingCallbacks, 0);
  assertExecutionStateReport(browserReport.success.report, "success");
  assertExecutionStateReport(browserReport.cancellation.report, "cancellation");
  assertExecutionStateReport(browserReport.watchdog.report, "watchdog");
  for (const report of [
    browserReport.success.report,
    browserReport.cancellation.report,
    browserReport.watchdog.report,
  ]) {
    assert.deepEqual(report.executionEngine, executionEngine);
  }
  assert.equal(
    browserReport.success.report.executionEngineLinearMemoryBytes,
    expectedBrowser.executionEngineLinearMemoryBytes,
  );
  assert.equal(
    nodeReport.firstUnsupported.memoryContentSha256,
    browserUnsupported.memoryContentSha256,
    "Node/browser final memory mismatch",
  );
}

function invocation(allowedDirtyPaths, outputRelative) {
  const parts = ["npm run check:private-target"];
  if (allowedDirtyPaths.length > 0 || outputRelative !== "docs/private-target-evidence.json") {
    parts.push("--");
  }
  for (const entry of allowedDirtyPaths) {
    parts.push(`--allow-dirty ${JSON.stringify(entry)}`);
  }
  if (outputRelative !== "docs/private-target-evidence.json") {
    parts.push(`--output ${JSON.stringify(outputRelative)}`);
  }
  return parts.join(" ");
}

const options = parseArguments(process.argv.slice(2));
const checkpoint = loadCurrentCheckpoint(path.join(root, "docs", "current-checkpoint.json"));
const headCommit = git(["rev-parse", "HEAD"]).trim();
const implementationTree = git(["rev-parse", "HEAD^{tree}"]).trim();
const beforeDirtyPaths = dirtyPaths();
const allowed = new Set([...options.allowedDirtyPaths, options.outputRelative]);
const disallowedDirtyPaths = beforeDirtyPaths.filter(
  (entry) => !allowed.has(entry) || isProtectedInput(entry),
);
if (disallowedDirtyPaths.length > 0) {
  fail(
    `execution inputs are dirty:\n${disallowedDirtyPaths.map((entry) => `- ${entry}`).join("\n")}`,
  );
}

const bgiPath = process.env.VINE_BGI_EXE ?? defaultBgi;
if (bgiPath === null || !fs.existsSync(bgiPath)) {
  fail("BGI.exe is unavailable; set VINE_BGI_EXE to the private target path");
}
const bgiBytes = fs.readFileSync(bgiPath);
const source = {
  fileName: path.basename(bgiPath),
  bytes: bgiBytes.byteLength,
  sha256: sha256(bgiBytes),
};
const expectedSource = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
if (
  source.bytes !== expectedSource.sourceBytes ||
  source.sha256 !== expectedSource.sourceSha256
) {
  fail(
    `BGI source fingerprint mismatch: expected ${expectedSource.sourceBytes} bytes/${expectedSource.sourceSha256}, got ${source.bytes} bytes/${source.sha256}`,
  );
}

console.log(
  `private target preflight passed: commit ${headCommit.slice(0, 12)}, ` +
    `${beforeDirtyPaths.length} recorded dirty path(s), source ${source.sha256}`,
);
if (process.platform === "win32") {
  runInherited(process.env.ComSpec ?? "cmd.exe", [
    "/d",
    "/s",
    "/c",
    "npm run check:runtime:bgi-browser",
  ]);
} else {
  runInherited("npm", ["run", "check:runtime:bgi-browser"]);
}

if (git(["rev-parse", "HEAD"]).trim() !== headCommit) {
  fail("HEAD changed while the private target gate was running");
}
const afterDirtyPaths = dirtyPaths();
const afterDisallowed = afterDirtyPaths.filter(
  (entry) => !allowed.has(entry) || isProtectedInput(entry),
);
if (afterDisallowed.length > 0) {
  fail(
    `the gate changed execution inputs:\n${afterDisallowed.map((entry) => `- ${entry}`).join("\n")}`,
  );
}

const browserReportPath = path.join(root, ".work", "browser-runtime", "bgi-browser-report.json");
const browserReport = readJson(browserReportPath);
const nodeReportPath = path.resolve(root, browserReport.nodeOracle.path);
const nodeReport = readJson(nodeReportPath);
const toolchainReportPath = path.join(root, ".work", "toolchain-spike", "report.json");
const v86AdapterReportPath = path.join(root, ".work", "v86-adapter-spike", "report.json");
const browserBuildManifestPath = path.join(
  root,
  ".work",
  "browser-runtime",
  "build-manifest.json",
);
const toolchainReport = readJson(toolchainReportPath);
const v86AdapterReport = readJson(v86AdapterReportPath);
const browserBuildManifest = readJson(browserBuildManifestPath);
const policyPath = path.join(
  root,
  checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
);
const policy = readJson(policyPath);
const executionEngineProfilePath = path.join(
  root,
  checkpoint.browserCheckpoint.executionEngineProfile,
);
const executionEngineProfile = readJson(executionEngineProfilePath);
const builtAdapterArtifact = browserBuildManifest.assets["v86-adapter.wasm"];
const executionEngine = executionEngineIdentity(
  executionEngineProfile,
  builtAdapterArtifact,
);
assertReportSemantics({
  checkpoint,
  nodeReport,
  browserReport,
  policy,
  executionEngine,
  source,
});

const runtimeCoreWasmPath = path.join(root, ".work", "browser-runtime", "runtime-core.wasm");
const v86AdapterWasmPath = path.join(root, ".work", "browser-runtime", "v86-adapter.wasm");
const artifacts = {
  nodeReport: artifact(nodeReportPath),
  browserReport: artifact(browserReportPath),
  toolchainReport: artifact(toolchainReportPath),
  v86AdapterReport: artifact(v86AdapterReportPath),
  browserBuildManifest: artifact(browserBuildManifestPath),
  runtimeCoreWasm: artifact(runtimeCoreWasmPath),
  v86AdapterWasm: artifact(v86AdapterWasmPath),
  cargoLock: artifact(path.join(root, "Cargo.lock")),
  npmLock: artifact(path.join(root, "package-lock.json")),
};
assert(
  artifacts.nodeReport.bytes <= maximumSemanticReportBytes,
  `summary Node report exceeds ${maximumSemanticReportBytes} bytes`,
);
assert(
  artifacts.browserReport.bytes <= maximumSemanticReportBytes,
  `summary browser report exceeds ${maximumSemanticReportBytes} bytes`,
);
assert.deepEqual(
  browserBuildManifest.assets["runtime-core.wasm"],
  { bytes: artifacts.runtimeCoreWasm.bytes, sha256: artifacts.runtimeCoreWasm.sha256 },
);
assert.equal(browserBuildManifest.schemaVersion, 2);
assert.deepEqual(browserBuildManifest.executionEngine, {
  profile: checkpoint.browserCheckpoint.executionEngineProfile,
  ...executionEngine,
});
assert.deepEqual(
  browserBuildManifest.assets["v86-adapter.wasm"],
  { bytes: artifacts.v86AdapterWasm.bytes, sha256: artifacts.v86AdapterWasm.sha256 },
);
assert.equal(browserReport.nodeOracle.bytes, artifacts.nodeReport.bytes);
assert.equal(browserReport.nodeOracle.sha256, artifacts.nodeReport.sha256);

const expectedBrowser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
const actualAllowedDirtyPaths = beforeDirtyPaths.filter((entry) => allowed.has(entry));
const evidence = finalizePrivateTargetEvidence({
  schema: PRIVATE_TARGET_EVIDENCE_SCHEMA,
  schemaVersion: PRIVATE_TARGET_EVIDENCE_VERSION,
  status: "passed",
  recordedAt: new Date().toISOString(),
  repository: {
    implementationCommit: headCommit,
    implementationTree,
    headCommit,
    dirty: beforeDirtyPaths.length > 0,
    dirtyPaths: beforeDirtyPaths,
    allowedDirtyPaths: actualAllowedDirtyPaths,
    executionInputsClean: true,
    disallowedDirtyPaths: [],
  },
  command: {
    canonical: "npm run check:private-target",
    invocation: invocation(options.allowedDirtyPaths, options.outputRelative),
    semanticGate: "required",
    performanceGate: "separate-observation",
  },
  source,
  profile: {
    currentCheckpointSchema: checkpoint.schema,
    currentCheckpointSchemaVersion: checkpoint.schemaVersion,
    runtimeCheckpoint: checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
    runtimeProfile: checkpoint.activeRuntimeCheckpoint.profile,
    policySchema: policy.schema,
    policySchemaVersion: policy.schemaVersion,
    policyId: policy.id,
    policySource: checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
    nodeReportSchema: nodeReport.schema,
    nodeReportSchemaVersion: nodeReport.schemaVersion,
    browserRuntimeProtocol: browserReport.runtimeProtocol,
    executionEngineProfile: checkpoint.browserCheckpoint.executionEngineProfile,
  },
  environment: {
    platform: `${process.platform}-${os.version()}`,
    osRelease: os.release(),
    architecture: process.arch,
    node: process.version,
    rustc: firstLine("rustc", ["--version"]),
    cargo: firstLine("cargo", ["--version"]),
    chrome: browserReport.chromeVersion,
    llvm: {
      clang: toolchainReport.tools.clang,
      wasmClang: toolchainReport.tools.wasmClang,
      lldLink: toolchainReport.tools.lldLink,
      llvmReadobj: toolchainReport.tools.llvmReadobj,
    },
    executionEngine: {
      engine: executionEngine.engine,
      sourceRevision: v86AdapterReport.sourceRevision,
      cpuContextAbi: {
        version: v86AdapterReport.results.cpuState.version,
        bytes: v86AdapterReport.results.cpuState.size,
      },
      adapterArtifact: {
        bytes: artifacts.v86AdapterWasm.bytes,
        sha256: artifacts.v86AdapterWasm.sha256,
      },
    },
  },
  semantic: {
    outcome: "passed",
    node: {
      outcome: "structured-stop",
      implementedApiContracts: nodeReport.process.implementedApiContracts,
      supportedCalls: nodeReport.supportedCallCount,
      callSemanticsSha256: nodeReport.supportedCallSemanticsSha256,
      instructions: nodeReport.firstUnsupported.instructionCount,
      finalGeneration: nodeReport.synchronization.finalGeneration,
      memoryContentSha256: nodeReport.firstUnsupported.memoryContentSha256,
      writebackBatches: nodeReport.synchronization.writebackBatches,
      dirtyPages: nodeReport.synchronization.dirtyPages,
      changedBytes: nodeReport.synchronization.changedBytes,
      hostDeltaBatches: nodeReport.synchronization.hostDeltaBatches,
    },
    browser: {
      outcome: browserReport.success.report.outcome,
      supportedCalls: browserReport.success.report.supportedCallCount,
      callSemanticsSha256:
        browserReport.success.report.supportedCallSemanticsSha256,
      instructions: browserReport.success.report.instructions,
      finalGeneration: browserReport.success.report.finalGeneration,
      memoryContentSha256:
        browserReport.success.report.firstUnsupported.memoryContentSha256,
      wasmBridgeCalls: browserReport.success.report.wasmBridgeCalls,
      executionStateMachine: {
        success: executionStateEvidence(browserReport.success.report),
        cancellation: executionStateEvidence(browserReport.cancellation.report),
        watchdog: executionStateEvidence(browserReport.watchdog.report),
      },
      executionEngine: structuredClone(browserReport.success.report.executionEngine),
      executionEngineLinearMemoryBytes:
        browserReport.success.report.executionEngineLinearMemoryBytes,
    },
    memoryEquivalent: true,
    nextStructuredStop: structuredClone(checkpoint.nextStructuredStop),
    cleanup: {
      pendingHostCalls: browserReport.success.report.pendingHostCalls,
      pendingCallbacks: browserReport.success.report.pendingCallbacks,
      cancelInstructions: browserReport.cancellation.report.instructions,
      cancelPendingHostCalls: browserReport.cancellation.report.pendingHostCalls,
      cancelPendingCallbacks: browserReport.cancellation.report.pendingCallbacks,
      watchdogInstructions: browserReport.watchdog.report.instructions,
      watchdogPendingHostCalls: browserReport.watchdog.report.pendingHostCalls,
      watchdogPendingCallbacks: browserReport.watchdog.report.pendingCallbacks,
    },
  },
  artifacts,
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
assertValidPrivateTargetEvidence(evidence, checkpoint);

fs.mkdirSync(path.dirname(options.output), { recursive: true });
const temporary = `${options.output}.tmp-${process.pid}`;
fs.writeFileSync(temporary, `${JSON.stringify(evidence, null, 2)}\n`, { flag: "wx" });
fs.renameSync(temporary, options.output);
console.log(
  `private target gate passed: ${evidence.semantic.node.supportedCalls} Node/Chrome calls, ` +
    `${
      evidence.semantic.nextStructuredStop.kind === "execution-fault"
        ? evidence.semantic.nextStructuredStop.capabilityCluster
        : evidence.semantic.nextStructuredStop.kind === "com-method"
          ? `${evidence.semantic.nextStructuredStop.interface}::${evidence.semantic.nextStructuredStop.method}`
          : evidence.semantic.nextStructuredStop.api
    } structured stop; ` +
    `${options.outputRelative} payload ${evidence.payloadSha256}`,
);
