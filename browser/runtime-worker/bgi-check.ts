import { observePageLifecycle } from "./page-lifecycle.js";
import { pointerSourceEvent } from "./keyboard-input.js";
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
  statusMessagesObserved: number;
  statusMessagesRetained: number;
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
const scheduledSharedClockSetWindowText =
  capabilityProfile ===
    "user32-set-window-text-a-post-scheduled-shared-clock" ||
  capabilityProfile === "kernel32-find-first-file-a" ||
  capabilityProfile === "kernel32-delete-file-a" ||
  capabilityProfile === "kernel32-private-heap-ordinary-segment-growth" ||
  capabilityProfile === "gdi-create-font-a-224x112" ||
  capabilityProfile === "gdi-create-dib-section-448x336" ||
  capabilityProfile === "gdi-outline-text-metrics-224x112";
const d3d9Canvas =
  scheduledSharedClockSetWindowText ||
  capabilityProfile === "d3d9-first-present" ||
  capabilityProfile === "d3d9-post-present-continuation" ||
  capabilityProfile === "user32-async-key-state-input" ||
  capabilityProfile === "user32-async-key-state-input-continuation" ||
  capabilityProfile === "user32-async-key-state-initial-scan" ||
  capabilityProfile === "user32-async-key-state-initial-scan-continuation" ||
  capabilityProfile === "user32-peek-message-a" ||
  capabilityProfile === "user32-is-iconic" ||
  capabilityProfile === "runtime-thread-stack-growth" ||
  capabilityProfile === "runtime-trapless-execution-budget" ||
  capabilityProfile === "kernel32-heap-free-large-buffer" ||
  capabilityProfile === "kernel32-heap-alloc-24-byte-buffer" ||
  capabilityProfile === "winmm-time-get-time-post-heap-reuse" ||
  capabilityProfile === "kernel32-enter-critical-section-post-clock" ||
  capabilityProfile === "kernel32-enter-critical-section-post-wait" ||
  capabilityProfile ===
    "kernel32-enter-critical-section-post-timer-wait-batch" ||
  capabilityProfile ===
    "kernel32-enter-critical-section-post-timer-rearm-wait-batch" ||
  capabilityProfile ===
    "kernel32-set-waitable-timer-post-critical-section-batch" ||
  capabilityProfile === "kernel32-set-waitable-timer-post-timer-wait-batch" ||
  capabilityProfile ===
    "kernel32-set-waitable-timer-post-timer-rearm-wait-batch" ||
  capabilityProfile === "primary-main-loop-recurring-timer-cycle-batch" ||
  capabilityProfile === "primary-main-loop-recurring-timer-multi-cycle-batch" ||
  capabilityProfile === "primary-main-loop-recurring-timer-window-batch" ||
  capabilityProfile === "primary-main-loop-presentation-progress-gateway" ||
  capabilityProfile === "primary-main-loop-input-progress-gateway" ||
  capabilityProfile === "primary-main-loop-action-input-progress-gateway" ||
  capabilityProfile ===
    "primary-main-loop-window-timer-callback-delivery-gateway" ||
  capabilityProfile ===
    "primary-main-loop-window-timer-callback-recurring-batch" ||
  capabilityProfile ===
    "primary-main-loop-window-timer-callback-action-input-progress-gateway" ||
  capabilityProfile ===
    "primary-main-loop-pointer-message-action-progress-gateway" ||
  capabilityProfile === "user32-set-focus-action-continuation" ||
  capabilityProfile === "winmm-time-get-time-action-continuation" ||
  capabilityProfile ===
    "user32-def-window-proc-a-action-continuation" ||
  capabilityProfile ===
    "user32-peek-message-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-translate-message-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-dispatch-message-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-def-window-proc-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-peek-message-a-post-pointer-release-continuation" ||
  capabilityProfile ===
    "winmm-time-get-time-post-pointer-release-continuation" ||
  capabilityProfile ===
    "winmm-time-get-time-post-pointer-release-repeat-batch" ||
  capabilityProfile ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  capabilityProfile === "user32-is-iconic-post-pointer-release-continuation" ||
  capabilityProfile === "winmm-time-get-time-post-is-iconic-continuation" ||
  capabilityProfile ===
    "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
  capabilityProfile ===
    "d3d9-test-cooperative-level-post-input-scan-continuation" ||
  capabilityProfile === "d3d9-post-input-scan-waitable-timer-cycle" ||
  capabilityProfile ===
    "user32-window-timer-callback-post-waitable-timer-cycle" ||
  capabilityProfile ===
    "user32-peek-message-a-post-window-timer-callback-continuation" ||
  capabilityProfile ===
    "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
  capabilityProfile ===
    "kernel32-wait-for-single-object-ex-post-timer-rearm-batch" ||
  capabilityProfile ===
    "kernel32-wait-for-single-object-ex-post-critical-section"
    ? requiredElement<HTMLCanvasElement>(
        "#surface",
      ).transferControlToOffscreen()
    : undefined;
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
const maximumRetainedStatuses = 4_096;
let statusMessagesObserved = 0;
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
      statusMessagesObserved += 1;
      const observation = { phase: message.phase, detail: message.detail };
      if (statuses.length < maximumRetainedStatuses) {
        statuses.push(observation);
      } else {
        statuses[maximumRetainedStatuses - 1] = observation;
      }
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

const startMessage = {
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
        : capabilityProfile === "gdi-outline-text-metrics-224x112"
          ? 385_000_000
        : capabilityProfile === "kernel32-delete-file-a" ||
            capabilityProfile ===
              "kernel32-private-heap-ordinary-segment-growth" ||
            capabilityProfile === "gdi-create-font-a-224x112" ||
            capabilityProfile === "gdi-create-dib-section-448x336"
          ? 350_000_000
        : scheduledSharedClockSetWindowText
          ? 340_000_000
          : capabilityProfile ===
              "primary-main-loop-presentation-progress-gateway" ||
            capabilityProfile === "primary-main-loop-input-progress-gateway" ||
            capabilityProfile ===
              "primary-main-loop-action-input-progress-gateway" ||
            capabilityProfile ===
              "primary-main-loop-window-timer-callback-delivery-gateway" ||
            capabilityProfile ===
              "primary-main-loop-window-timer-callback-recurring-batch" ||
            capabilityProfile ===
              "primary-main-loop-window-timer-callback-action-input-progress-gateway" ||
            capabilityProfile ===
              "primary-main-loop-pointer-message-action-progress-gateway" ||
            capabilityProfile === "user32-set-focus-action-continuation" ||
            capabilityProfile === "winmm-time-get-time-action-continuation" ||
            capabilityProfile ===
              "user32-def-window-proc-a-action-continuation" ||
            capabilityProfile ===
              "user32-peek-message-a-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-translate-message-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-dispatch-message-a-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-def-window-proc-a-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-peek-message-a-post-pointer-release-continuation" ||
            capabilityProfile ===
              "winmm-time-get-time-post-pointer-release-continuation" ||
            capabilityProfile ===
              "winmm-time-get-time-post-pointer-release-repeat-batch" ||
            capabilityProfile ===
              "user32-get-window-rect-post-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-get-cursor-pos-post-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-get-system-metrics-post-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
            capabilityProfile ===
              "user32-is-iconic-post-pointer-release-continuation" ||
            capabilityProfile ===
              "winmm-time-get-time-post-is-iconic-continuation" ||
            capabilityProfile ===
              "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
            capabilityProfile ===
              "d3d9-test-cooperative-level-post-input-scan-continuation" ||
            capabilityProfile ===
              "d3d9-post-input-scan-waitable-timer-cycle" ||
            capabilityProfile ===
              "user32-window-timer-callback-post-waitable-timer-cycle" ||
            capabilityProfile ===
              "user32-peek-message-a-post-window-timer-callback-continuation"
          ? 340_000_000
          : capabilityProfile === "runtime-trapless-execution-budget" ||
              capabilityProfile === "kernel32-heap-free-large-buffer" ||
              capabilityProfile === "kernel32-heap-alloc-24-byte-buffer" ||
              capabilityProfile === "winmm-time-get-time-post-heap-reuse" ||
              capabilityProfile ===
                "kernel32-enter-critical-section-post-clock" ||
              capabilityProfile ===
                "kernel32-enter-critical-section-post-wait" ||
              capabilityProfile ===
                "kernel32-enter-critical-section-post-timer-wait-batch" ||
              capabilityProfile ===
                "kernel32-enter-critical-section-post-timer-rearm-wait-batch" ||
              capabilityProfile ===
                "kernel32-set-waitable-timer-post-critical-section-batch" ||
              capabilityProfile ===
                "kernel32-set-waitable-timer-post-timer-wait-batch" ||
              capabilityProfile ===
                "kernel32-set-waitable-timer-post-timer-rearm-wait-batch" ||
              capabilityProfile ===
                "primary-main-loop-recurring-timer-cycle-batch" ||
              capabilityProfile ===
                "primary-main-loop-recurring-timer-multi-cycle-batch" ||
              capabilityProfile ===
                "primary-main-loop-recurring-timer-window-batch" ||
              capabilityProfile ===
                "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
              capabilityProfile ===
                "kernel32-wait-for-single-object-ex-post-timer-rearm-batch" ||
              capabilityProfile ===
                "kernel32-wait-for-single-object-ex-post-critical-section"
            ? 320_000_000
            : capabilityProfile === "runtime-thread-stack-growth"
              ? 300_000_000
              : capabilityProfile === "user32-async-key-state-initial-scan" ||
                  capabilityProfile ===
                    "user32-async-key-state-initial-scan-continuation" ||
                  capabilityProfile === "user32-peek-message-a" ||
                  capabilityProfile === "user32-is-iconic"
                ? 180_000_000
                : capabilityProfile === "kernel32-create-directory-a" ||
                    capabilityProfile ===
                      "user32-set-window-pos-repeat-activation" ||
                    capabilityProfile === "d3d9-recreated-textures" ||
                    capabilityProfile ===
                      "user32-set-window-pos-frame-change-move" ||
                    capabilityProfile === "user32-show-window-normal" ||
                    capabilityProfile === "user32-update-window" ||
                    capabilityProfile === "d3d9-clear" ||
                    capabilityProfile === "user32-set-rect" ||
                    capabilityProfile === "d3d9-add-dirty-rect" ||
                    capabilityProfile === "d3d9-update-texture" ||
                    capabilityProfile === "d3d9-begin-scene" ||
                    capabilityProfile === "d3d9-initial-sampler-states" ||
                    capabilityProfile === "d3d9-initial-texture-stage-states" ||
                    capabilityProfile === "d3d9-set-texture" ||
                    capabilityProfile === "d3d9-set-fvf" ||
                    capabilityProfile === "d3d9-set-stream-source" ||
                    capabilityProfile === "d3d9-draw-primitive" ||
                    capabilityProfile === "d3d9-end-scene" ||
                    capabilityProfile === "d3d9-post-end-scene-raster-status" ||
                    capabilityProfile ===
                      "d3d9-post-raster-status-waitable-timer" ||
                    capabilityProfile === "d3d9-post-scene-present-gateway" ||
                    capabilityProfile === "d3d9-first-present" ||
                    capabilityProfile === "d3d9-post-present-continuation" ||
                    capabilityProfile === "user32-async-key-state-input" ||
                    capabilityProfile ===
                      "user32-async-key-state-input-continuation"
                  ? 35_000_000
                  : 20_000_000,
    watchdogWallMilliseconds:
      scheduledSharedClockSetWindowText ||
      capabilityProfile === "user32-async-key-state-initial-scan" ||
      capabilityProfile ===
        "user32-async-key-state-initial-scan-continuation" ||
      capabilityProfile === "user32-peek-message-a" ||
      capabilityProfile === "user32-is-iconic" ||
      capabilityProfile === "runtime-thread-stack-growth" ||
      capabilityProfile === "runtime-trapless-execution-budget" ||
      capabilityProfile === "kernel32-heap-free-large-buffer" ||
      capabilityProfile === "kernel32-heap-alloc-24-byte-buffer" ||
      capabilityProfile === "winmm-time-get-time-post-heap-reuse" ||
      capabilityProfile === "kernel32-enter-critical-section-post-clock" ||
      capabilityProfile === "kernel32-enter-critical-section-post-wait" ||
      capabilityProfile ===
        "kernel32-enter-critical-section-post-timer-wait-batch" ||
      capabilityProfile ===
        "kernel32-enter-critical-section-post-timer-rearm-wait-batch" ||
      capabilityProfile ===
        "kernel32-set-waitable-timer-post-critical-section-batch" ||
      capabilityProfile ===
        "kernel32-set-waitable-timer-post-timer-wait-batch" ||
      capabilityProfile ===
        "kernel32-set-waitable-timer-post-timer-rearm-wait-batch" ||
      capabilityProfile === "primary-main-loop-recurring-timer-cycle-batch" ||
      capabilityProfile ===
        "primary-main-loop-recurring-timer-multi-cycle-batch" ||
      capabilityProfile === "primary-main-loop-recurring-timer-window-batch" ||
      capabilityProfile === "primary-main-loop-presentation-progress-gateway" ||
      capabilityProfile === "primary-main-loop-input-progress-gateway" ||
      capabilityProfile ===
        "primary-main-loop-action-input-progress-gateway" ||
      capabilityProfile ===
        "primary-main-loop-window-timer-callback-delivery-gateway" ||
      capabilityProfile ===
        "primary-main-loop-window-timer-callback-recurring-batch" ||
      capabilityProfile ===
        "primary-main-loop-window-timer-callback-action-input-progress-gateway" ||
      capabilityProfile ===
        "primary-main-loop-pointer-message-action-progress-gateway" ||
      capabilityProfile === "user32-set-focus-action-continuation" ||
      capabilityProfile === "winmm-time-get-time-action-continuation" ||
      capabilityProfile ===
        "user32-def-window-proc-a-action-continuation" ||
      capabilityProfile ===
        "user32-peek-message-a-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-translate-message-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-dispatch-message-a-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-def-window-proc-a-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-peek-message-a-post-pointer-release-continuation" ||
      capabilityProfile ===
        "winmm-time-get-time-post-pointer-release-continuation" ||
      capabilityProfile ===
        "winmm-time-get-time-post-pointer-release-repeat-batch" ||
      capabilityProfile ===
        "user32-get-window-rect-post-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-get-cursor-pos-post-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-get-system-metrics-post-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
      capabilityProfile ===
        "user32-is-iconic-post-pointer-release-continuation" ||
      capabilityProfile ===
        "winmm-time-get-time-post-is-iconic-continuation" ||
      capabilityProfile ===
        "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
      capabilityProfile ===
        "d3d9-test-cooperative-level-post-input-scan-continuation" ||
      capabilityProfile === "d3d9-post-input-scan-waitable-timer-cycle" ||
      capabilityProfile ===
        "user32-window-timer-callback-post-waitable-timer-cycle" ||
      capabilityProfile ===
        "user32-peek-message-a-post-window-timer-callback-continuation" ||
      capabilityProfile ===
        "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
      capabilityProfile ===
        "kernel32-wait-for-single-object-ex-post-timer-rearm-batch" ||
      capabilityProfile ===
        "kernel32-wait-for-single-object-ex-post-critical-section"
        ? 600_000
        : capabilityProfile === "kernel32-sleep-zero" ||
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
            capabilityProfile === "d3d9-initial-texture-stage-states" ||
            capabilityProfile === "d3d9-set-texture" ||
            capabilityProfile === "d3d9-set-fvf" ||
            capabilityProfile === "d3d9-set-stream-source" ||
            capabilityProfile === "d3d9-draw-primitive" ||
            capabilityProfile === "d3d9-end-scene" ||
            capabilityProfile === "d3d9-post-end-scene-raster-status" ||
            capabilityProfile === "d3d9-post-raster-status-waitable-timer" ||
            capabilityProfile === "d3d9-post-scene-present-gateway" ||
            capabilityProfile === "d3d9-first-present" ||
            capabilityProfile === "d3d9-post-present-continuation" ||
            capabilityProfile === "user32-async-key-state-input" ||
            capabilityProfile === "user32-async-key-state-input-continuation"
          ? 300_000
          : capabilityProfile === "shell32-drag-accept-files" ||
              capabilityProfile === "kernel32-archive-source-read"
            ? 180_000
            : 20_000,
    pageLifecycle: pageLifecycle.snapshot(),
  },
  ...(d3d9Canvas === undefined ? {} : { d3d9Canvas }),
} satisfies MainToWorkerMessage;
worker.postMessage(startMessage, d3d9Canvas === undefined ? [] : [d3d9Canvas]);
if (
  capabilityProfile === "user32-async-key-state-input" ||
  capabilityProfile === "user32-async-key-state-input-continuation" ||
  capabilityProfile === "primary-main-loop-input-progress-gateway" ||
  capabilityProfile === "primary-main-loop-action-input-progress-gateway" ||
  capabilityProfile ===
    "primary-main-loop-window-timer-callback-action-input-progress-gateway" ||
  capabilityProfile === "primary-main-loop-pointer-message-action-progress-gateway" ||
  capabilityProfile === "user32-set-focus-action-continuation" ||
  capabilityProfile === "winmm-time-get-time-action-continuation" ||
  capabilityProfile === "user32-def-window-proc-a-action-continuation" ||
  capabilityProfile ===
    "user32-peek-message-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-translate-message-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-dispatch-message-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-def-window-proc-a-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-peek-message-a-post-pointer-release-continuation" ||
  capabilityProfile ===
    "winmm-time-get-time-post-pointer-release-continuation" ||
  capabilityProfile ===
    "winmm-time-get-time-post-pointer-release-repeat-batch" ||
  capabilityProfile ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
  capabilityProfile ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
  capabilityProfile === "user32-is-iconic-post-pointer-release-continuation" ||
  capabilityProfile === "winmm-time-get-time-post-is-iconic-continuation" ||
  capabilityProfile ===
    "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
  capabilityProfile ===
    "d3d9-test-cooperative-level-post-input-scan-continuation" ||
  capabilityProfile === "d3d9-post-input-scan-waitable-timer-cycle" ||
  capabilityProfile ===
    "user32-window-timer-callback-post-waitable-timer-cycle" ||
  capabilityProfile ===
    "user32-peek-message-a-post-window-timer-callback-continuation" ||
  scheduledSharedClockSetWindowText
) {
  const pointerMessageInput =
    capabilityProfile === "primary-main-loop-pointer-message-action-progress-gateway" ||
    capabilityProfile === "user32-set-focus-action-continuation" ||
    capabilityProfile === "winmm-time-get-time-action-continuation" ||
    capabilityProfile === "user32-def-window-proc-a-action-continuation" ||
    capabilityProfile ===
      "user32-peek-message-a-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-translate-message-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-dispatch-message-a-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-def-window-proc-a-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-peek-message-a-post-pointer-release-continuation" ||
    capabilityProfile ===
      "winmm-time-get-time-post-pointer-release-continuation" ||
    capabilityProfile ===
      "winmm-time-get-time-post-pointer-release-repeat-batch" ||
    capabilityProfile ===
      "user32-get-window-rect-post-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-get-cursor-pos-post-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-get-system-metrics-post-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
    capabilityProfile ===
      "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
    capabilityProfile === "user32-is-iconic-post-pointer-release-continuation" ||
    capabilityProfile === "winmm-time-get-time-post-is-iconic-continuation" ||
    capabilityProfile ===
      "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
    capabilityProfile ===
      "d3d9-test-cooperative-level-post-input-scan-continuation" ||
    capabilityProfile === "d3d9-post-input-scan-waitable-timer-cycle" ||
    capabilityProfile ===
      "user32-window-timer-callback-post-waitable-timer-cycle" ||
    capabilityProfile ===
      "user32-peek-message-a-post-window-timer-callback-continuation" ||
    scheduledSharedClockSetWindowText;
  const actionInput =
    capabilityProfile === "primary-main-loop-action-input-progress-gateway" ||
    capabilityProfile ===
      "primary-main-loop-window-timer-callback-action-input-progress-gateway";
  if (pointerMessageInput) {
    const downEvent = pointerSourceEvent(
      1,
      "button-down",
      0,
      0x8000,
      0x8000,
    );
    const upEvent = pointerSourceEvent(
      2,
      "button-up",
      0,
      0x8000,
      0x8000,
    );
    if (downEvent === null || upEvent === null) {
      throw new Error("primary pointer button is outside the bounded source mapping");
    }
    worker.postMessage({
      type: "pointer-input",
      protocolVersion,
      sessionId,
      event: downEvent,
    } satisfies MainToWorkerMessage);
    worker.postMessage({
      type: "pointer-input",
      protocolVersion,
      sessionId,
      event: upEvent,
    } satisfies MainToWorkerMessage);
  } else {
    const downEvent = {
      sequence: "1",
      kind: "key-down" as const,
      virtualKey: actionInput ? 0x01 : 0xa2,
      repeat: false,
    };
    worker.postMessage({
      type: "keyboard-input",
      protocolVersion,
      sessionId,
      event: downEvent,
    } satisfies MainToWorkerMessage);
    if (
      capabilityProfile === "primary-main-loop-input-progress-gateway" ||
      actionInput
    ) {
      const upEvent = {
        sequence: "2",
        kind: "key-up" as const,
        virtualKey: actionInput ? 0x01 : 0xa2,
        repeat: false,
      };
      worker.postMessage({
        type: "keyboard-input",
        protocolVersion,
        sessionId,
        event: upEvent,
      } satisfies MainToWorkerMessage);
    }
  }
}

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
    statusMessagesObserved,
    statusMessagesRetained: statuses.length,
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
  return value ===
    "user32-set-window-text-a-post-scheduled-shared-clock" ||
    value === "kernel32-find-first-file-a" ||
    value === "kernel32-delete-file-a" ||
    value === "kernel32-private-heap-ordinary-segment-growth" ||
    value === "gdi-create-font-a-224x112" ||
    value === "gdi-create-dib-section-448x336" ||
    value === "gdi-outline-text-metrics-224x112" ||
    value === "kernel32-wait-for-single-object-ex" ||
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
    value === "d3d9-initial-texture-stage-states" ||
    value === "d3d9-set-texture" ||
    value === "d3d9-set-fvf" ||
    value === "d3d9-set-stream-source" ||
    value === "d3d9-draw-primitive" ||
    value === "d3d9-end-scene" ||
    value === "d3d9-post-end-scene-raster-status" ||
    value === "d3d9-post-raster-status-waitable-timer" ||
    value === "d3d9-post-scene-present-gateway" ||
    value === "d3d9-first-present" ||
    value === "d3d9-post-present-continuation" ||
    value === "user32-async-key-state-input" ||
    value === "user32-async-key-state-input-continuation" ||
    value === "user32-async-key-state-initial-scan" ||
    value === "user32-async-key-state-initial-scan-continuation" ||
    value === "user32-peek-message-a" ||
    value === "user32-is-iconic" ||
    value === "runtime-thread-stack-growth" ||
    value === "runtime-trapless-execution-budget" ||
    value === "kernel32-heap-free-large-buffer" ||
    value === "kernel32-heap-alloc-24-byte-buffer" ||
    value === "winmm-time-get-time-post-heap-reuse" ||
    value === "kernel32-enter-critical-section-post-clock" ||
    value === "kernel32-enter-critical-section-post-wait" ||
    value === "kernel32-enter-critical-section-post-timer-wait-batch" ||
    value === "kernel32-enter-critical-section-post-timer-rearm-wait-batch" ||
    value === "kernel32-set-waitable-timer-post-critical-section-batch" ||
    value === "kernel32-set-waitable-timer-post-timer-wait-batch" ||
    value === "kernel32-set-waitable-timer-post-timer-rearm-wait-batch" ||
    value === "primary-main-loop-recurring-timer-cycle-batch" ||
    value === "primary-main-loop-recurring-timer-multi-cycle-batch" ||
    value === "primary-main-loop-recurring-timer-window-batch" ||
    value === "primary-main-loop-presentation-progress-gateway" ||
    value === "primary-main-loop-input-progress-gateway" ||
    value === "primary-main-loop-action-input-progress-gateway" ||
    value === "primary-main-loop-window-timer-callback-delivery-gateway" ||
    value === "primary-main-loop-window-timer-callback-recurring-batch" ||
    value ===
      "primary-main-loop-window-timer-callback-action-input-progress-gateway" ||
    value === "primary-main-loop-pointer-message-action-progress-gateway" ||
    value === "user32-set-focus-action-continuation" ||
    value === "winmm-time-get-time-action-continuation" ||
    value === "user32-def-window-proc-a-action-continuation" ||
    value === "user32-peek-message-a-pointer-release-continuation" ||
    value === "user32-translate-message-pointer-release-continuation" ||
    value === "user32-dispatch-message-a-pointer-release-continuation" ||
    value === "user32-def-window-proc-a-pointer-release-continuation" ||
    value === "user32-peek-message-a-post-pointer-release-continuation" ||
    value === "winmm-time-get-time-post-pointer-release-continuation" ||
    value === "winmm-time-get-time-post-pointer-release-repeat-batch" ||
    value === "user32-get-window-rect-post-pointer-release-continuation" ||
    value === "user32-get-cursor-pos-post-pointer-release-continuation" ||
    value === "user32-get-system-metrics-post-pointer-release-continuation" ||
    value ===
      "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
    value ===
      "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
    value === "user32-is-iconic-post-pointer-release-continuation" ||
    value === "winmm-time-get-time-post-is-iconic-continuation" ||
    value ===
      "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
    value === "d3d9-test-cooperative-level-post-input-scan-continuation" ||
    value === "d3d9-post-input-scan-waitable-timer-cycle" ||
    value ===
      "user32-window-timer-callback-post-waitable-timer-cycle" ||
    value ===
      "user32-peek-message-a-post-window-timer-callback-continuation" ||
    value === "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
    value === "kernel32-wait-for-single-object-ex-post-timer-rearm-batch" ||
    value === "kernel32-wait-for-single-object-ex-post-critical-section"
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
