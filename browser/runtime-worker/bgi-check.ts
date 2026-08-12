import { observePageLifecycle } from "./page-lifecycle.js";
import { pointerSourceEvent } from "./keyboard-input.js";
import {
  BrowserPcmRenderer,
  type BrowserAudioPlaybackReceipt,
  type BrowserAudioStopReceipt,
} from "./audio-renderer.js";
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
  audioPlaybackRequests: number;
  audioPlaybacksStarted: number;
  latestAudioPlayback: BrowserAudioPlaybackReceipt | null;
  audioStopRequests: number;
  audioStopsApplied: number;
  latestAudioStop: BrowserAudioStopReceipt | null;
}

declare global {
  interface Window {
    __vineBgiCheck: {
      result: BgiBrowserEvidence | null;
      error: string | null;
      cancel: () => void;
    };
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
const forwardDevelopment = searchParams.get("development") === "forward";
const forwardSeconds = Number(searchParams.get("forwardSeconds") ?? 60);
if (!Number.isInteger(forwardSeconds) || forwardSeconds < 1 || forwardSeconds > 540) {
  throw new Error("forwardSeconds must be an integer from 1 through 540");
}
const evidenceLevel =
  !forwardDevelopment && searchParams.get("evidence") === "full"
    ? "full"
    : "summary";
const capabilityProfile = parseCapabilityProfile(searchParams.get("profile"));
const scheduledSharedClockSetWindowText =
  capabilityProfile ===
    "user32-set-window-text-a-post-scheduled-shared-clock" ||
  capabilityProfile === "kernel32-find-first-file-a" ||
  capabilityProfile === "kernel32-delete-file-a" ||
  capabilityProfile === "kernel32-private-heap-ordinary-segment-growth" ||
  capabilityProfile === "gdi-create-font-a-224x112" ||
  capabilityProfile === "gdi-create-dib-section-448x336" ||
  capabilityProfile === "gdi-outline-text-metrics-224x112" ||
  capabilityProfile === "user32-get-cursor-info" ||
  capabilityProfile === "d3d9-update-texture-partial-dirty-region" ||
  capabilityProfile === "d3d9-begin-scene-post-partial-dirty-region" ||
  capabilityProfile === "d3d9-set-sampler-state-post-begin-scene" ||
  capabilityProfile === "d3d9-set-texture-stage-state-post-sampler-state" ||
  capabilityProfile === "d3d9-set-texture-post-texture-stage-state" ||
  capabilityProfile === "d3d9-set-fvf-post-texture-binding" ||
  capabilityProfile === "d3d9-set-stream-source-post-fvf" ||
  capabilityProfile === "d3d9-draw-primitive-post-stream-source" ||
  capabilityProfile === "d3d9-end-scene-post-draw-primitive" ||
  capabilityProfile === "d3d9-get-raster-status-post-end-scene" ||
  capabilityProfile === "d3d9-present-post-raster-status" ||
  capabilityProfile === "winmm-time-get-time-post-present" ||
  capabilityProfile === "d3d9-test-cooperative-level-post-time-get-time" ||
  capabilityProfile ===
    "kernel32-set-waitable-timer-post-test-cooperative-level" ||
  capabilityProfile ===
    "kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm" ||
  capabilityProfile ===
    "user32-peek-message-a-post-test-cooperative-level-timer-wait" ||
  capabilityProfile ===
    "user32-translate-message-post-test-cooperative-level-timer-peek" ||
  capabilityProfile ===
    "user32-dispatch-message-a-post-test-cooperative-level-timer-translate" ||
  capabilityProfile ===
    "kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch" ||
  capabilityProfile ===
    "user32-peek-message-a-post-test-cooperative-level-timer-callback" ||
  capabilityProfile ===
    "winmm-time-get-time-post-test-cooperative-level-timer-callback" ||
  capabilityProfile ===
    "user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time" ||
  capabilityProfile ===
    "user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect" ||
  capabilityProfile ===
    "primary-main-loop-post-callback-pointer-message-consumption-batch" ||
  capabilityProfile ===
    "primary-main-loop-post-callback-pointer-action-continuation-batch" ||
  capabilityProfile ===
    "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
  capabilityProfile ===
    "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch" ||
  capabilityProfile ===
    "primary-main-loop-post-callback-pointer-release-message-consumption-batch";
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
const pcmRenderer = new BrowserPcmRenderer();
const maximumRetainedStatuses = forwardDevelopment ? 128 : 4_096;
let statusMessagesObserved = 0;
let audioPlaybackRequests = 0;
let audioPlaybacksStarted = 0;
let latestAudioPlayback: BrowserAudioPlaybackReceipt | null = null;
let audioStopRequests = 0;
let audioStopsApplied = 0;
let latestAudioStop: BrowserAudioStopReceipt | null = null;
const usedJsHeapBytesBefore = performance.memory?.usedJSHeapSize ?? null;
let heartbeatSamples = 0;
let maximumHeartbeatGapMilliseconds = 0;
let lastHeartbeat = performance.now();
let cancellationSent = false;
let forwardAudioObserved = false;
let forwardVisibleFrameObserved = false;
let forwardPostAudioActionSent = false;
let forwardPostAudioActionReleased = false;
const heartbeat = window.setInterval(() => {
  const now = performance.now();
  maximumHeartbeatGapMilliseconds = Math.max(
    maximumHeartbeatGapMilliseconds,
    now - lastHeartbeat,
  );
  lastHeartbeat = now;
  heartbeatSamples += 1;
}, 5);

window.__vineBgiCheck = {
  result: null,
  error: null,
  cancel: () => {
    if (cancellationSent) {
      return;
    }
    cancellationSent = true;
    worker.postMessage({
      type: "cancel-session",
      protocolVersion,
      sessionId,
    } satisfies MainToWorkerMessage);
  },
};

function maybePostForwardAction(): void {
  if (
    !forwardDevelopment ||
    !forwardAudioObserved ||
    !forwardVisibleFrameObserved ||
    forwardPostAudioActionSent
  ) {
    return;
  }
  const down = pointerSourceEvent(1, "button-down", 0, 0x3b00, 0xd200);
  if (down === null) {
    fail(new Error("post-audio pointer action is outside the bounded source mapping"));
    return;
  }
  forwardPostAudioActionSent = true;
  worker.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event: down,
  } satisfies MainToWorkerMessage);
}

function releaseForwardAction(): void {
  if (!forwardPostAudioActionSent || forwardPostAudioActionReleased) {
    return;
  }
  const up = pointerSourceEvent(2, "button-up", 0, 0x3b00, 0xd200);
  if (up === null) {
    fail(new Error("post-audio pointer release is outside the bounded source mapping"));
    return;
  }
  forwardPostAudioActionReleased = true;
  worker.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event: up,
  } satisfies MainToWorkerMessage);
}

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
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail ===
          "Forward visible frame observed; accepting live development input"
      ) {
        forwardVisibleFrameObserved = true;
        maybePostForwardAction();
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail === "Forward primary pointer press observed by guest"
      ) {
        releaseForwardAction();
      }
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
    if (message.type === "audio-playback") {
      audioPlaybackRequests += 1;
      forwardAudioObserved = true;
      maybePostForwardAction();
      void pcmRenderer
        .play(message)
        .then((receipt) => {
          audioPlaybacksStarted += 1;
          latestAudioPlayback = receipt;
          document.body.dataset.audioPlayback = JSON.stringify(receipt);
        })
        .catch((error: unknown) =>
          fail(error instanceof Error ? error : new Error(String(error))),
        );
      return;
    }
    if (message.type === "audio-stop") {
      audioStopRequests += 1;
      latestAudioStop = pcmRenderer.stop(message);
      if (latestAudioStop.sourceStopped) {
        audioStopsApplied += 1;
      }
      document.body.dataset.audioStop = JSON.stringify(latestAudioStop);
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
    ...(forwardDevelopment ? { developmentMode: "forward" as const } : {}),
    executableUrl: "./private/BGI.exe",
    policyUrl: "./private/kernel32-policy.json",
    instructionBudget:
      scenario === "watchdog" ? 4 : forwardDevelopment ? 131_072 : 16_384,
    watchdogInstructionLimit:
      forwardDevelopment
        ? 10_300_000_000
        : scenario === "watchdog"
        ? 1
        : capabilityProfile === "user32-get-cursor-info" ||
            capabilityProfile === "d3d9-update-texture-partial-dirty-region" ||
            capabilityProfile === "d3d9-begin-scene-post-partial-dirty-region" ||
            capabilityProfile === "d3d9-set-sampler-state-post-begin-scene" ||
            capabilityProfile ===
              "d3d9-set-texture-stage-state-post-sampler-state" ||
            capabilityProfile === "d3d9-set-texture-post-texture-stage-state" ||
            capabilityProfile === "d3d9-set-fvf-post-texture-binding" ||
            capabilityProfile === "d3d9-set-stream-source-post-fvf" ||
            capabilityProfile === "d3d9-draw-primitive-post-stream-source" ||
            capabilityProfile === "d3d9-end-scene-post-draw-primitive" ||
            capabilityProfile === "d3d9-get-raster-status-post-end-scene" ||
            capabilityProfile === "d3d9-present-post-raster-status" ||
            capabilityProfile === "winmm-time-get-time-post-present" ||
            capabilityProfile ===
              "d3d9-test-cooperative-level-post-time-get-time" ||
            capabilityProfile ===
              "kernel32-set-waitable-timer-post-test-cooperative-level" ||
            capabilityProfile ===
              "kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm" ||
            capabilityProfile ===
              "user32-peek-message-a-post-test-cooperative-level-timer-wait" ||
            capabilityProfile ===
              "user32-translate-message-post-test-cooperative-level-timer-peek" ||
            capabilityProfile ===
              "user32-dispatch-message-a-post-test-cooperative-level-timer-translate" ||
            capabilityProfile ===
              "kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch" ||
            capabilityProfile ===
              "user32-peek-message-a-post-test-cooperative-level-timer-callback" ||
            capabilityProfile ===
              "winmm-time-get-time-post-test-cooperative-level-timer-callback" ||
            capabilityProfile ===
              "user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time"
          ? 405_000_000
        : capabilityProfile ===
              "user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect" ||
            capabilityProfile ===
              "primary-main-loop-post-callback-pointer-message-consumption-batch" ||
            capabilityProfile ===
              "primary-main-loop-post-callback-pointer-action-continuation-batch"
          ? 455_000_000
        : capabilityProfile ===
            "primary-main-loop-post-callback-pointer-release-message-consumption-batch"
          ? 8_000_000_000
        : capabilityProfile ===
              "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
            capabilityProfile ===
              "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch"
          ? 3_200_000_000
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
      forwardDevelopment
        ? forwardSeconds * 1_000
        : capabilityProfile ===
        "primary-main-loop-post-callback-pointer-release-message-consumption-batch"
        ? 3_600_000
        : capabilityProfile ===
        "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch"
        ? 1_800_000
        : scheduledSharedClockSetWindowText ||
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
  !forwardDevelopment &&
  (capabilityProfile === "user32-async-key-state-input" ||
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
  scheduledSharedClockSetWindowText)
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
    if (
      capabilityProfile ===
        "user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-message-consumption-batch" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-action-continuation-batch" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch" ||
      capabilityProfile ===
        "primary-main-loop-post-callback-pointer-release-message-consumption-batch"
    ) {
      const actionDownEvent = pointerSourceEvent(
        3,
        "button-down",
        0,
        0x8000,
        0x8000,
      );
      if (actionDownEvent === null) {
        throw new Error("primary action pointer button is outside the bounded source mapping");
      }
      worker.postMessage({
        type: "pointer-input",
        protocolVersion,
        sessionId,
        event: actionDownEvent,
      } satisfies MainToWorkerMessage);
      if (
        capabilityProfile ===
          "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
        capabilityProfile ===
          "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch" ||
        capabilityProfile ===
          "primary-main-loop-post-callback-pointer-release-message-consumption-batch"
      ) {
        const actionUpEvent = pointerSourceEvent(
          4,
          "button-up",
          0,
          0x8000,
          0x8000,
        );
        if (actionUpEvent === null) {
          throw new Error(
            "primary action release is outside the bounded source mapping",
          );
        }
        worker.postMessage({
          type: "pointer-input",
          protocolVersion,
          sessionId,
          event: actionUpEvent,
        } satisfies MainToWorkerMessage);
      }
    }
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
  if (forwardDevelopment) {
    const accepted =
      report.outcome === "structured-stop" ||
      report.outcome === "checkpoint-complete" ||
      report.outcome === "completed" ||
      report.outcome === "fault" ||
      report.outcome === "watchdog";
    if (!accepted) {
      fail(
        new Error(report.error ?? `unexpected forward outcome ${report.outcome}`),
      );
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
      audioPlaybackRequests,
      audioPlaybacksStarted,
      latestAudioPlayback,
      audioStopRequests,
      audioStopsApplied,
      latestAudioStop,
    };
    window.__vineBgiCheck.result = evidence;
    document.body.dataset.checkStatus = "passed";
    document.body.dataset.report = JSON.stringify(evidence);
    status.textContent = report.outcome;
    worker.terminate();
    return;
  }
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
    audioPlaybackRequests,
    audioPlaybacksStarted,
    latestAudioPlayback,
    audioStopRequests,
    audioStopsApplied,
    latestAudioStop,
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
    value === "user32-get-cursor-info" ||
    value === "d3d9-update-texture-partial-dirty-region" ||
    value === "d3d9-begin-scene-post-partial-dirty-region" ||
    value === "d3d9-set-sampler-state-post-begin-scene" ||
    value === "d3d9-set-texture-stage-state-post-sampler-state" ||
    value === "d3d9-set-texture-post-texture-stage-state" ||
    value === "d3d9-set-fvf-post-texture-binding" ||
    value === "d3d9-set-stream-source-post-fvf" ||
    value === "d3d9-draw-primitive-post-stream-source" ||
    value === "d3d9-end-scene-post-draw-primitive" ||
    value === "d3d9-get-raster-status-post-end-scene" ||
    value === "d3d9-present-post-raster-status" ||
    value === "winmm-time-get-time-post-present" ||
    value === "d3d9-test-cooperative-level-post-time-get-time" ||
    value === "kernel32-set-waitable-timer-post-test-cooperative-level" ||
    value ===
      "kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm" ||
    value ===
      "user32-peek-message-a-post-test-cooperative-level-timer-wait" ||
    value ===
      "user32-translate-message-post-test-cooperative-level-timer-peek" ||
    value ===
      "user32-dispatch-message-a-post-test-cooperative-level-timer-translate" ||
    value ===
      "kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch" ||
    value ===
      "user32-peek-message-a-post-test-cooperative-level-timer-callback" ||
    value ===
      "winmm-time-get-time-post-test-cooperative-level-timer-callback" ||
    value ===
      "user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time" ||
    value ===
      "user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect" ||
    value ===
      "primary-main-loop-post-callback-pointer-message-consumption-batch" ||
    value ===
      "primary-main-loop-post-callback-pointer-action-continuation-batch" ||
    value ===
      "primary-main-loop-post-callback-pointer-action-next-iteration-batch" ||
    value ===
      "primary-main-loop-post-callback-pointer-action-next-iteration-input-scan-batch" ||
    value ===
      "primary-main-loop-post-callback-pointer-release-message-consumption-batch" ||
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
