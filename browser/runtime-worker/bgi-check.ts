import { observePageLifecycle } from "./page-lifecycle.js";
import {
  assertProtocolVersion,
  type BgiCapabilityProfile,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionMode,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";

interface BgiBrowserEvidence {
  scenario: SessionMode;
  report: SessionReport;
  heartbeatSamples: number;
  maximumHeartbeatGapMilliseconds: number;
  usedJsHeapBytesBefore: number | null;
  usedJsHeapBytesAfter: number | null;
  statuses: Array<{ phase: string; detail: string }>;
}

declare global {
  interface Window {
    __vineBgiCheck: { result: BgiBrowserEvidence | null; error: string | null };
  }

  interface Performance {
    memory?: { usedJSHeapSize: number };
  }
}

const worker = new Worker(new URL("./runtime-worker.js", import.meta.url), {
  type: "module",
});
const status = requiredElement<HTMLOutputElement>("#status");
const searchParams = new URL(location.href).searchParams;
const scenario = parseScenario(searchParams.get("scenario"));
const evidenceLevel =
  searchParams.get("evidence") === "full" ? "full" : "summary";
const capabilityProfile = parseCapabilityProfile(searchParams.get("profile"));
const sessionId = 1;
const pageLifecycle = observePageLifecycle((lifecycle) => {
  worker.postMessage({
    type: "page-lifecycle",
    protocolVersion,
    sessionId,
    lifecycle,
  } satisfies MainToWorkerMessage);
});
const statuses: Array<{ phase: string; detail: string }> = [];
const usedJsHeapBytesBefore = performance.memory?.usedJSHeapSize ?? null;
let heartbeatSamples = 0;
let maximumHeartbeatGapMilliseconds = 0;
let lastHeartbeat = performance.now();
let cancellationSent = false;
const heartbeat = window.setInterval(() => {
  const now = performance.now();
  maximumHeartbeatGapMilliseconds = Math.max(
    maximumHeartbeatGapMilliseconds,
    now - lastHeartbeat,
  );
  lastHeartbeat = now;
  heartbeatSamples += 1;
}, 5);

window.__vineBgiCheck = { result: null, error: null };

worker.addEventListener(
  "message",
  (event: MessageEvent<WorkerToMainMessage>) => {
    const message = event.data;
    assertProtocolVersion(message.protocolVersion);
    if (message.sessionId !== sessionId) {
      return;
    }
    if (message.type === "session-status") {
      statuses.push({ phase: message.phase, detail: message.detail });
      status.textContent = message.detail;
      if (
        scenario === "cancel-pending" &&
        message.phase === "running" &&
        !cancellationSent
      ) {
        cancellationSent = true;
        worker.postMessage({
          type: "cancel-session",
          protocolVersion,
          sessionId,
        } satisfies MainToWorkerMessage);
      }
      return;
    }
    if (message.type === "host-request") {
      fail(
        new Error(
          `BGI emitted unexpected browser host request ${message.service}`,
        ),
      );
      return;
    }
    finish(message.report);
  },
);

worker.addEventListener("error", (event) => fail(new Error(event.message)));

worker.postMessage({
  type: "start-session",
  protocolVersion,
  sessionId,
  configuration: {
    mode: scenario,
    evidenceLevel,
    workload: "private-bgi",
    ...(capabilityProfile === undefined ? {} : { capabilityProfile }),
    executableUrl: "./private/BGI.exe",
    policyUrl: "./private/kernel32-policy.json",
    instructionBudget: scenario === "watchdog" ? 4 : 16_384,
    watchdogInstructionLimit:
      scenario === "watchdog"
        ? 1
        : capabilityProfile === "kernel32-create-directory-a" ||
            capabilityProfile === "user32-set-window-pos-repeat-activation" ||
            capabilityProfile === "d3d9-recreated-textures" ||
            capabilityProfile === "user32-set-window-pos-frame-change-move" ||
            capabilityProfile === "user32-show-window-normal" ||
            capabilityProfile === "user32-update-window"
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
      capabilityProfile === "user32-update-window"
        ? 300_000
        : capabilityProfile === "shell32-drag-accept-files" ||
            capabilityProfile === "kernel32-archive-source-read"
          ? 180_000
          : 20_000,
    pageLifecycle: pageLifecycle.snapshot(),
  },
} satisfies MainToWorkerMessage);

function finish(report: SessionReport): void {
  window.clearInterval(heartbeat);
  const expectedOutcome =
    scenario === "success"
      ? report.checkpointCompletion === null
        ? "structured-stop"
        : "checkpoint-complete"
      : scenario === "watchdog"
        ? "watchdog"
        : "cancelled";
  if (report.outcome !== expectedOutcome) {
    fail(new Error(report.error ?? `unexpected BGI outcome ${report.outcome}`));
    return;
  }
  const evidence: BgiBrowserEvidence = {
    scenario,
    report,
    heartbeatSamples,
    maximumHeartbeatGapMilliseconds,
    usedJsHeapBytesBefore,
    usedJsHeapBytesAfter: performance.memory?.usedJSHeapSize ?? null,
    statuses,
  };
  window.__vineBgiCheck.result = evidence;
  document.body.dataset.checkStatus = "passed";
  document.body.dataset.report = JSON.stringify(evidence);
  status.textContent = expectedOutcome;
  worker.terminate();
}

function fail(error: Error): void {
  window.clearInterval(heartbeat);
  window.__vineBgiCheck.error = error.message;
  document.body.dataset.checkStatus = "failed";
  document.body.dataset.error = error.message;
  status.textContent = error.message;
  worker.terminate();
}

function parseScenario(value: string | null): SessionMode {
  return value === "cancel-pending" || value === "watchdog" ? value : "success";
}

function parseCapabilityProfile(
  value: string | null,
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
    value === "user32-update-window"
    ? value
    : undefined;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  invariant(element !== null, `missing ${selector}`);
  return element;
}
