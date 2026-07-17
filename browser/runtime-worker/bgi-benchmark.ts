import {
  assertProtocolVersion,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";
import type { ExecutionEngineIdentity } from "./execution-engine-contract.js";

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

const worker = new Worker(new URL("./runtime-worker.js", import.meta.url), { type: "module" });
const statusOutput = requiredElement<HTMLOutputElement>("#status");
const requestedDurationMilliseconds = Number(
  new URL(location.href).searchParams.get("durationMs") ?? 60_000,
);
const sampleIntervalMilliseconds = Number(
  new URL(location.href).searchParams.get("sampleIntervalMs") ?? 5_000,
);
invariant(
  Number.isFinite(requestedDurationMilliseconds) && requestedDurationMilliseconds >= 10_000,
  "BGI benchmark duration must be at least 10 seconds",
);
invariant(
  Number.isFinite(sampleIntervalMilliseconds) && sampleIntervalMilliseconds >= 1_000,
  "BGI benchmark sample interval must be at least 1 second",
);

const startedAt = performance.now();
const memorySamples: MemorySample[] = [];
const sessions: SessionSample[] = [];
let failedSessions = 0;
let nextSessionId = 1;
let activeSession:
  | { id: number; resolve: (report: SessionReport) => void; reject: (error: Error) => void }
  | null = null;
let heartbeatSamples = 0;
const heartbeatGapMilliseconds: number[] = [];
let maximumHeartbeatGapMilliseconds = 0;
let lastHeartbeat = performance.now();
let finalResult: BgiBenchmarkResult | null = null;
let finalError: string | null = null;
let executionEngineIdentity: ExecutionEngineIdentity | null = null;

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
const memorySampler = window.setInterval(recordMemorySample, sampleIntervalMilliseconds);
recordMemorySample();

window.__vineBgiBenchmark = {
  snapshot: () => ({
    status: finalError === null ? (finalResult === null ? "running" : "passed") : "failed",
    elapsedMilliseconds: performance.now() - startedAt,
    completedSessions: sessions.length,
    result: finalResult,
    error: finalError,
  }),
};

worker.addEventListener("message", (event: MessageEvent<WorkerToMainMessage>) => {
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
    activeSession.reject(new Error(`BGI emitted unexpected host request ${message.service}`));
    activeSession = null;
    return;
  }
  const active = activeSession;
  activeSession = null;
  active.resolve(message.report);
});
worker.addEventListener("error", (event) => {
  activeSession?.reject(new Error(event.message));
  activeSession = null;
});

void runBenchmark();

async function runBenchmark(): Promise<void> {
  try {
    const checkpoint = await fetchJson("./current-checkpoint.json");
    const expected = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
    while (performance.now() - startedAt < requestedDurationMilliseconds) {
      const sequence = nextSessionId;
      const report = await runSession(sequence);
      validateSession(report, expected);
      invariant(report.executionEngine !== null, "BGI execution engine identity is missing");
      if (executionEngineIdentity === null) {
        executionEngineIdentity = structuredClone(report.executionEngine);
      } else {
        invariant(
          JSON.stringify(report.executionEngine) === JSON.stringify(executionEngineIdentity),
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
        executionEngineLinearMemoryBytes: report.executionEngineLinearMemoryBytes,
        writebackBatches: report.writebackBatches,
        writebackPages: report.writebackPages,
        finalGeneration: report.finalGeneration,
      });
      releaseCompletedSessionPayload(report);
      nextSessionId += 1;
      await delay(20);
    }
    recordMemorySample();
    invariant(executionEngineIdentity !== null, "BGI benchmark completed no engine session");
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

function releaseCompletedSessionPayload(report: SessionReport): void {
  report.supportedCalls.length = 0;
  report.sourceReads.length = 0;
  report.stackGrowths.length = 0;
  report.registeredThreadIds.length = 0;
  report.executedThreadIds.length = 0;
  report.hostWaitExecutions.length = 0;
  report.threadExits.length = 0;
}

function runSession(sessionId: number): Promise<SessionReport> {
  invariant(activeSession === null, "BGI benchmark session overlap");
  worker.postMessage({
    type: "start-session",
    protocolVersion,
    sessionId,
    configuration: {
      mode: "success",
      workload: "private-bgi",
      executableUrl: "./private/BGI.exe",
      policyUrl: "./private/kernel32-policy.json",
      instructionBudget: 256,
      watchdogInstructionLimit: 20_000_000,
      watchdogWallMilliseconds: 10_000,
    },
  } satisfies MainToWorkerMessage);
  return new Promise((resolve, reject) => {
    activeSession = { id: sessionId, resolve, reject };
  });
}

function validateSession(report: SessionReport, expected: any): void {
  const endpoint = report.firstUnsupported?.endpoint;
  const fault = report.firstUnsupported?.fault;
  invariant(isRecord(fault), "BGI next fault is malformed");
  invariant(report.outcome === "structured-stop", report.error ?? "BGI session did not stop cleanly");
  invariant(report.pendingHostCalls === 0 && report.pendingCallbacks === 0, "BGI retained work");
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
  invariant(report.supportedCalls.length === expected.supportedCalls, "BGI call count changed");
  invariant(report.instructions === expected.guestInstructions, "BGI instruction count changed");
  invariant(report.wasmBridgeCalls === expected.wasmBridgeCalls, "BGI Wasm crossing count changed");
  if (expected.nextEndpoint === null) {
    invariant(endpoint === null, "BGI execution fault unexpectedly retained an endpoint");
    invariant(
      fault.instructionPointer === Number.parseInt(expected.nextInstructionPointer, 16),
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
    invariant(isRecord(endpoint) && isRecord(endpoint.symbol), "BGI next endpoint is malformed");
    invariant(
      `${endpoint.module}!${endpoint.symbol.name}`.toLowerCase() === expected.nextApi.toLowerCase(),
      "BGI next API changed",
    );
  }
  invariant(fault.code === expected.nextFaultCode, "BGI next fault changed");
  invariant(
    report.firstUnsupported?.memoryContentSha256 === expected.memoryContentSha256,
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
  invariant(response.ok, `benchmark manifest fetch failed with ${response.status}`);
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
