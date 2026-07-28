import {
  assertProtocolVersion,
  type BgiCapabilityProfile,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";
import type { ExecutionEngineIdentity } from "./execution-engine-contract.js";
import { observePageLifecycle } from "./page-lifecycle.js";

interface MemorySample {
  elapsedMilliseconds: number;
  completedSessions: number;
  usedJsHeapBytes: number | null;
}

interface SessionSample {
  sequence: number;
  benchmarkElapsedMilliseconds: number;
  elapsedMilliseconds: number;
  maximumSliceMilliseconds: number;
  wasmBridgeCalls: number;
  runtimeCoreLinearMemoryBytes: number;
  executionEngineLinearMemoryBytes: number;
  writebackBatches: number;
  writebackPages: number;
  finalGeneration: number;
}

interface BgiBenchmarkResult {
  requestedDurationMilliseconds: number;
  durationMilliseconds: number;
  completedSessions: number;
  failedSessions: number;
  heartbeatSamples: number;
  heartbeatGapMilliseconds: number[];
  maximumHeartbeatGapMilliseconds: number;
  executionEngine: ExecutionEngineIdentity;
  memorySamples: MemorySample[];
  sessions: SessionSample[];
}

interface BenchmarkSnapshot {
  status: "running" | "passed" | "failed";
  elapsedMilliseconds: number;
  completedSessions: number;
  result: BgiBenchmarkResult | null;
  error: string | null;
}

declare global {
  interface Window {
    __vineBgiBenchmark: { snapshot(): BenchmarkSnapshot };
  }

  interface Performance {
    memory?: { usedJSHeapSize: number };
  }
}

const worker = new Worker(new URL("./runtime-worker.js", import.meta.url), {
  type: "module",
});
const statusOutput = requiredElement<HTMLOutputElement>("#status");
const searchParameters = new URL(location.href).searchParams;
const requestedDurationMilliseconds = Number(
  searchParameters.get("durationMs") ?? 60_000,
);
const sampleIntervalMilliseconds = Number(
  searchParameters.get("sampleIntervalMs") ?? 5_000,
);
invariant(
  Number.isFinite(requestedDurationMilliseconds) &&
    requestedDurationMilliseconds >= 10_000,
  "BGI benchmark duration must be at least 10 seconds",
);
invariant(
  Number.isFinite(sampleIntervalMilliseconds) &&
    sampleIntervalMilliseconds >= 1_000,
  "BGI benchmark sample interval must be at least 1 second",
);

const startedAt = performance.now();
const memorySamples: MemorySample[] = [];
const sessions: SessionSample[] = [];
let failedSessions = 0;
let nextSessionId = 1;
let activeSession: {
  id: number;
  resolve: (report: SessionReport) => void;
  reject: (error: Error) => void;
} | null = null;
let heartbeatSamples = 0;
const heartbeatGapMilliseconds: number[] = [];
let maximumHeartbeatGapMilliseconds = 0;
let lastHeartbeat = performance.now();
let finalResult: BgiBenchmarkResult | null = null;
let finalError: string | null = null;
let executionEngineIdentity: ExecutionEngineIdentity | null = null;
const pageLifecycle = observePageLifecycle((lifecycle) => {
  if (activeSession !== null) {
    worker.postMessage({
      type: "page-lifecycle",
      protocolVersion,
      sessionId: activeSession.id,
      lifecycle,
    } satisfies MainToWorkerMessage);
  }
});

const heartbeat = window.setInterval(() => {
  const now = performance.now();
  const gap = now - lastHeartbeat;
  heartbeatGapMilliseconds.push(gap);
  maximumHeartbeatGapMilliseconds = Math.max(
    maximumHeartbeatGapMilliseconds,
    gap,
  );
  lastHeartbeat = now;
  heartbeatSamples += 1;
}, 5);
const memorySampler = window.setInterval(
  recordMemorySample,
  sampleIntervalMilliseconds,
);
recordMemorySample();

window.__vineBgiBenchmark = {
  snapshot: () => ({
    status:
      finalError === null
        ? finalResult === null
          ? "running"
          : "passed"
        : "failed",
    elapsedMilliseconds: performance.now() - startedAt,
    completedSessions: sessions.length,
    result: finalResult,
    error: finalError,
  }),
};

worker.addEventListener(
  "message",
  (event: MessageEvent<WorkerToMainMessage>) => {
    const message = event.data;
    assertProtocolVersion(message.protocolVersion);
    if (activeSession === null || message.sessionId !== activeSession.id) {
      return;
    }
    if (message.type === "session-status") {
      statusOutput.textContent = `${sessions.length} sessions; ${message.detail}`;
      return;
    }
    if (message.type === "host-request") {
      activeSession.reject(
        new Error(`BGI emitted unexpected host request ${message.service}`),
      );
      activeSession = null;
      return;
    }
    const active = activeSession;
    activeSession = null;
    active.resolve(message.report);
  },
);
worker.addEventListener("error", (event) => {
  activeSession?.reject(new Error(event.message));
  activeSession = null;
});

void runBenchmark();

async function runBenchmark(): Promise<void> {
  try {
    const checkpoint = await fetchJson("./current-checkpoint.json");
    const candidateProfile = searchParameters.get("profile");
    const expected =
      candidateProfile === null
        ? checkpointExpectedEvidence(
            checkpoint.browserCheckpoint.bgiRuntimeEvidence,
          )
        : candidateExpectedEvidence();
    const capabilityProfile = parseCapabilityProfile(
      candidateProfile ?? checkpoint.activeRuntimeCheckpoint.profile,
    );
    invariant(
      capabilityProfile !== undefined,
      "BGI benchmark checkpoint profile is unsupported",
    );
    while (performance.now() - startedAt < requestedDurationMilliseconds) {
      const sequence = nextSessionId;
      const report = await runSession(sequence, capabilityProfile);
      validateSession(report, expected);
      invariant(
        report.executionEngine !== null,
        "BGI execution engine identity is missing",
      );
      if (executionEngineIdentity === null) {
        executionEngineIdentity = structuredClone(report.executionEngine);
      } else {
        invariant(
          JSON.stringify(report.executionEngine) ===
            JSON.stringify(executionEngineIdentity),
          "BGI execution engine identity changed between sessions",
        );
      }
      sessions.push({
        sequence,
        benchmarkElapsedMilliseconds: performance.now() - startedAt,
        elapsedMilliseconds: report.elapsedMilliseconds,
        maximumSliceMilliseconds: report.maximumSliceMilliseconds,
        wasmBridgeCalls: report.wasmBridgeCalls,
        runtimeCoreLinearMemoryBytes: report.runtimeCoreLinearMemoryBytes,
        executionEngineLinearMemoryBytes:
          report.executionEngineLinearMemoryBytes,
        writebackBatches: report.writebackBatches,
        writebackPages: report.writebackPages,
        finalGeneration: report.finalGeneration,
      });
      releaseCompletedSessionPayload(report);
      nextSessionId += 1;
      await delay(20);
    }
    recordMemorySample();
    invariant(
      executionEngineIdentity !== null,
      "BGI benchmark completed no engine session",
    );
    finalResult = {
      requestedDurationMilliseconds,
      durationMilliseconds: performance.now() - startedAt,
      completedSessions: sessions.length,
      failedSessions,
      heartbeatSamples,
      heartbeatGapMilliseconds,
      maximumHeartbeatGapMilliseconds,
      executionEngine: executionEngineIdentity,
      memorySamples,
      sessions,
    };
    document.body.dataset.checkStatus = "passed";
    statusOutput.textContent = `${sessions.length} sessions completed`;
  } catch (error) {
    failedSessions += 1;
    finalError = error instanceof Error ? error.message : String(error);
    document.body.dataset.checkStatus = "failed";
    document.body.dataset.error = finalError;
    statusOutput.textContent = finalError;
  } finally {
    window.clearInterval(heartbeat);
    window.clearInterval(memorySampler);
    worker.terminate();
  }
}

function checkpointExpectedEvidence(evidence: any): any {
  return {
    ...evidence,
    nextApi: evidence.nextApi ?? evidence.nextEndpoint,
  };
}

function candidateExpectedEvidence(): any {
  return {
    guestInstructions: requiredIntegerSearchParameter("guestInstructions"),
    supportedCalls: requiredIntegerSearchParameter("supportedCalls"),
    sourceReadCompletions: requiredIntegerSearchParameter(
      "sourceReadCompletions",
    ),
    nextApi: requiredSearchParameter("nextApi"),
    nextEndpoint: {},
    nextFaultCode: requiredSearchParameter("nextFaultCode"),
    callSemanticsAlgorithm: requiredSearchParameter("callSemanticsAlgorithm"),
    callSemanticsSha256: requiredSearchParameter("callSemanticsSha256"),
    memoryContentSha256: requiredSearchParameter("memoryContentSha256"),
    wasmBridgeCalls: requiredIntegerSearchParameter("wasmBridgeCalls"),
    checkpointCompletion: null,
  };
}

function requiredSearchParameter(name: string): string {
  const value = searchParameters.get(name);
  invariant(value !== null && value.length > 0, `missing benchmark ${name}`);
  return value;
}

function requiredIntegerSearchParameter(name: string): number {
  const value = Number(requiredSearchParameter(name));
  invariant(
    Number.isSafeInteger(value) && value >= 0,
    `invalid benchmark ${name}`,
  );
  return value;
}

function releaseCompletedSessionPayload(report: SessionReport): void {
  report.supportedCalls.length = 0;
  report.sourceReads.length = 0;
  report.stackGrowths.length = 0;
  report.registeredThreadIds.length = 0;
  report.executedThreadIds.length = 0;
  report.hostWaitExecutions.length = 0;
  report.threadExits.length = 0;
}

function runSession(
  sessionId: number,
  capabilityProfile: BgiCapabilityProfile | undefined,
): Promise<SessionReport> {
  invariant(activeSession === null, "BGI benchmark session overlap");
  worker.postMessage({
    type: "start-session",
    protocolVersion,
    sessionId,
    configuration: {
      mode: "success",
      evidenceLevel: "summary",
      workload: "private-bgi",
      ...(capabilityProfile === undefined ? {} : { capabilityProfile }),
      executableUrl: "./private/BGI.exe",
      policyUrl: "./private/kernel32-policy.json",
      instructionBudget: 16_384,
      watchdogInstructionLimit:
        capabilityProfile === "kernel32-create-directory-a" ||
        capabilityProfile === "user32-set-window-pos-repeat-activation" ||
        capabilityProfile === "d3d9-recreated-textures" ||
        capabilityProfile === "user32-set-window-pos-frame-change-move" ||
        capabilityProfile === "user32-show-window-normal" ||
        capabilityProfile === "user32-update-window" ||
        capabilityProfile === "d3d9-clear" ||
        capabilityProfile === "user32-set-rect" ||
        capabilityProfile === "d3d9-add-dirty-rect" ||
        capabilityProfile === "d3d9-update-texture" ||
        capabilityProfile === "d3d9-begin-scene" ||
        capabilityProfile === "d3d9-initial-sampler-states" ||
        (capabilityProfile === "d3d9-initial-texture-stage-states" ||
          capabilityProfile === "d3d9-set-texture" ||
          capabilityProfile === "d3d9-set-fvf" ||
          capabilityProfile === "d3d9-set-stream-source" ||
          capabilityProfile === "d3d9-draw-primitive" ||
          capabilityProfile === "d3d9-end-scene" ||
          capabilityProfile === "d3d9-post-end-scene-raster-status")
          ? 35_000_000
          : 20_000_000,
      watchdogWallMilliseconds:
        capabilityProfile === "kernel32-sleep-zero" ||
        capabilityProfile === "kernel32-global-memory-status" ||
        capabilityProfile === "kernel32-get-file-attributes-a" ||
        capabilityProfile === "kernel32-create-directory-a" ||
        capabilityProfile === "user32-set-window-pos-repeat-activation" ||
        capabilityProfile === "d3d9-recreated-textures" ||
        capabilityProfile === "user32-set-window-pos-frame-change-move" ||
        capabilityProfile === "user32-show-window-normal" ||
        capabilityProfile === "user32-update-window" ||
        capabilityProfile === "d3d9-clear" ||
        capabilityProfile === "user32-set-rect" ||
        capabilityProfile === "d3d9-add-dirty-rect" ||
        capabilityProfile === "d3d9-update-texture" ||
        capabilityProfile === "d3d9-begin-scene" ||
        capabilityProfile === "d3d9-initial-sampler-states" ||
        (capabilityProfile === "d3d9-initial-texture-stage-states" ||
          capabilityProfile === "d3d9-set-texture" ||
          capabilityProfile === "d3d9-set-fvf" ||
        capabilityProfile === "d3d9-set-stream-source" ||
        capabilityProfile === "d3d9-draw-primitive" ||
        capabilityProfile === "d3d9-end-scene" ||
        capabilityProfile === "d3d9-post-end-scene-raster-status")
          ? 300_000
          : capabilityProfile === "shell32-drag-accept-files" ||
              capabilityProfile === "kernel32-archive-source-read"
            ? 180_000
            : 20_000,
      pageLifecycle: pageLifecycle.snapshot(),
    },
  } satisfies MainToWorkerMessage);
  return new Promise((resolve, reject) => {
    activeSession = { id: sessionId, resolve, reject };
  });
}

function parseCapabilityProfile(
  value: unknown,
): BgiCapabilityProfile | undefined {
  return value === "kernel32-wait-for-single-object-ex" ||
    value === "kernel32-enter-critical-section" ||
    value === "primary-main-loop-progression" ||
    value === "kernel32-leave-critical-section" ||
    value === "kernel32-enter-critical-section-continuation" ||
    value === "kernel32-leave-critical-section-continuation" ||
    value === "shell32-drag-accept-files" ||
    value === "kernel32-archive-source-read" ||
    value === "kernel32-sleep-zero" ||
    value === "kernel32-global-memory-status" ||
    value === "kernel32-get-file-attributes-a" ||
    value === "kernel32-create-directory-a" ||
    value === "user32-set-window-pos-repeat-activation" ||
    value === "d3d9-recreated-textures" ||
    value === "user32-set-window-pos-frame-change-move" ||
    value === "user32-show-window-normal" ||
    value === "user32-update-window" ||
    value === "d3d9-clear" ||
    value === "user32-set-rect" ||
    value === "d3d9-add-dirty-rect" ||
    value === "d3d9-update-texture" ||
    value === "d3d9-begin-scene" ||
    value === "d3d9-initial-sampler-states" ||
    (value === "d3d9-initial-texture-stage-states" ||
      value === "d3d9-set-texture" ||
      value === "d3d9-set-fvf" ||
      value === "d3d9-set-stream-source" ||
      value === "d3d9-draw-primitive" ||
      value === "d3d9-end-scene" ||
      value === "d3d9-post-end-scene-raster-status")
    ? value
    : undefined;
}

function validateSession(report: SessionReport, expected: any): void {
  const endpoint = report.firstUnsupported?.endpoint;
  const fault = report.firstUnsupported?.fault;
  const checkpoint = report.checkpointCompletion;
  const expectedCheckpoint = expected.checkpointCompletion ?? null;
  invariant(
    report.outcome ===
      (expectedCheckpoint === null ? "structured-stop" : "checkpoint-complete"),
    report.error ?? "BGI session did not stop cleanly",
  );
  invariant(
    report.pendingHostCalls === 0 && report.pendingCallbacks === 0,
    "BGI retained work",
  );
  invariant(
    report.executionState === "stopped" &&
      report.lastExecutionTransition?.to === "stopped" &&
      report.pendingHostRequestRecords === 0,
    "BGI execution state did not stop cleanly",
  );
  invariant(
    report.executionEngine?.schema === "vine.execution-engine-profile" &&
      report.executionEngine.schemaVersion === 1,
    "BGI execution engine identity is missing",
  );
  invariant(
    report.supportedCallCount === expected.supportedCalls,
    "BGI call count changed",
  );
  invariant(
    report.supportedCalls.length === 0,
    "BGI benchmark retained detailed call evidence",
  );
  invariant(
    report.supportedCallSemanticsAlgorithm === expected.callSemanticsAlgorithm,
    "BGI semantic-call digest algorithm changed",
  );
  invariant(
    report.supportedCallSemanticsSha256 === expected.callSemanticsSha256,
    "BGI semantic-call digest changed",
  );
  invariant(
    report.sourceReads.length === expected.sourceReadCompletions,
    "BGI source-read completion count changed",
  );
  invariant(
    report.instructions === expected.guestInstructions,
    "BGI instruction count changed",
  );
  invariant(
    report.wasmBridgeCalls === expected.wasmBridgeCalls,
    "BGI Wasm crossing count changed",
  );
  if (expectedCheckpoint !== null) {
    invariant(
      report.firstUnsupported === null,
      "BGI checkpoint retained an unsupported fault",
    );
    invariant(isRecord(checkpoint), "BGI capability checkpoint is malformed");
    invariant(
      checkpoint.capability === expectedCheckpoint.capability &&
        checkpoint.implementationId === expectedCheckpoint.implementationId &&
        checkpoint.requiredCompletions ===
          expectedCheckpoint.requiredCompletions &&
        checkpoint.completionCount === expectedCheckpoint.completionCount &&
        checkpoint.lastCompletionSequence ===
          expectedCheckpoint.lastCompletionSequence &&
        checkpoint.lastCompletionInstructionCount ===
          expectedCheckpoint.lastCompletionInstructionCount &&
        checkpoint.instructionCount === expectedCheckpoint.instructionCount &&
        checkpoint.memoryContentSha256 ===
          expectedCheckpoint.memoryContentSha256,
      "BGI capability checkpoint changed",
    );
    return;
  }
  invariant(isRecord(fault), "BGI next fault is malformed");
  invariant(
    checkpoint === null,
    "BGI structured stop retained a capability checkpoint",
  );
  if (expected.nextEndpoint === null) {
    invariant(
      endpoint === null,
      "BGI execution fault unexpectedly retained an endpoint",
    );
    invariant(
      fault.instructionPointer ===
        Number.parseInt(expected.nextInstructionPointer, 16),
      "BGI fault instruction changed",
    );
    invariant(
      fault.previousInstructionPointer ===
        Number.parseInt(expected.nextPreviousInstructionPointer, 16),
      "BGI previous instruction changed",
    );
    invariant(
      fault.faultAddress === Number.parseInt(expected.nextFaultAddress, 16),
      "BGI fault address changed",
    );
    invariant(
      fault.pageFaultErrorCode === expected.nextPageFaultErrorCode,
      "BGI page-fault error code changed",
    );
  } else {
    invariant(isRecord(endpoint), "BGI next endpoint is malformed");
    if (endpoint.kind === "com-method") {
      const target = endpoint.target;
      invariant(
        isRecord(target) &&
          typeof target.interface === "string" &&
          typeof target.method === "string",
        "BGI next COM endpoint is malformed",
      );
      invariant(
        `${target.interface}::${target.method}`.toLowerCase() ===
          expected.nextApi.toLowerCase(),
        "BGI next COM method changed",
      );
    } else {
      invariant(
        isRecord(endpoint.symbol),
        "BGI next API endpoint is malformed",
      );
      const symbol =
        endpoint.symbol.kind === "ordinal"
          ? `#${endpoint.symbol.ordinal}`
          : endpoint.symbol.name;
      invariant(
        `${endpoint.module}!${symbol}`.toLowerCase() ===
          expected.nextApi.toLowerCase(),
        "BGI next API changed",
      );
    }
  }
  invariant(fault.code === expected.nextFaultCode, "BGI next fault changed");
  invariant(
    report.firstUnsupported?.memoryContentSha256 ===
      expected.memoryContentSha256,
    "BGI memory content changed",
  );
}

function isRecord(value: unknown): value is Record<string, any> {
  return typeof value === "object" && value !== null;
}

function recordMemorySample(): void {
  memorySamples.push({
    elapsedMilliseconds: performance.now() - startedAt,
    completedSessions: sessions.length,
    usedJsHeapBytes: performance.memory?.usedJSHeapSize ?? null,
  });
}

async function fetchJson(url: string): Promise<any> {
  const response = await fetch(url);
  invariant(
    response.ok,
    `benchmark manifest fetch failed with ${response.status}`,
  );
  return response.json();
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  invariant(element !== null, `missing ${selector}`);
  return element;
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
