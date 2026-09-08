import { observePageLifecycle } from "./page-lifecycle.js";
import {
  keyboardSourceEvent,
  pointerPositionSourceEvent,
  pointerSourceEvent,
} from "./keyboard-input.js";
import {
  BrowserPcmRenderer,
  type BrowserAudioPacingSnapshot,
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
  audioPacing: BrowserAudioPacingSnapshot;
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

const searchParams = new URL(location.href).searchParams;
const executionEngine = searchParams.get("engine");
if (executionEngine !== null && executionEngine !== "interpreter") {
  throw new Error("engine must be interpreter when specified");
}
const executionMode = searchParams.get("executionMode");
if (executionMode !== null && executionMode !== "play-performance") {
  throw new Error("executionMode must be play-performance when specified");
}
const workerUrl = new URL("./runtime-worker.js", import.meta.url);
if (executionEngine === "interpreter") {
  workerUrl.searchParams.set("executionJit", "off");
}
if (executionMode === "play-performance") {
  workerUrl.searchParams.set("executionMode", executionMode);
}
const worker = new Worker(workerUrl, {
  type: "module",
});
const status = requiredElement<HTMLOutputElement>("#status");
const scenario = parseScenario(searchParams.get("scenario"));
const forwardDevelopment = searchParams.get("development") === "forward";
const forwardInput = searchParams.get("forwardInput");
if (forwardInput !== null && forwardInput !== "title-observe") {
  throw new Error("forwardInput must be title-observe when specified");
}
const forwardInputMode =
  forwardInput === "title-observe" ? "title-observe" : "campaign";
const forwardStop = searchParams.get("forwardStop");
const forwardPointerStopMatch =
  forwardStop === null
    ? null
    : /^pointer-action:([1-9][0-9]*)$/u.exec(forwardStop);
if (
  forwardStop !== null &&
  forwardStop !== "title-action" &&
  forwardStop !== "looping-audio-update" &&
  forwardPointerStopMatch === null
) {
  throw new Error(
    "forwardStop must be title-action, looping-audio-update, or pointer-action:N when specified",
  );
}
const stopAfterForwardTitleAction = forwardStop === "title-action";
const stopAfterForwardLoopingAudioUpdate =
  forwardStop === "looping-audio-update";
const stopAfterForwardPointerAction =
  forwardPointerStopMatch === null ? null : Number(forwardPointerStopMatch[1]);
const forwardPointerOverrides = parseForwardPointerOverrides(
  searchParams.get("forwardPointerOverrides"),
);
const forwardPointerHoldOverrides = parseForwardPointerHoldOverrides(
  searchParams.get("forwardPointerHoldOverrides"),
);
const forwardPointerStaticActions = parseForwardPointerStaticActions(
  searchParams.get("forwardPointerStaticActions"),
);
const forwardEnterAfterPointerAction = parseOptionalPositiveInteger(
  searchParams.get("forwardEnterAfterPointerAction"),
  "forwardEnterAfterPointerAction",
);
const forwardHoldControlAfterPointerAction = parseOptionalPositiveInteger(
  searchParams.get("forwardHoldControlAfterPointerAction"),
  "forwardHoldControlAfterPointerAction",
);
const forwardHoldControlMilliseconds = parseOptionalPositiveInteger(
  searchParams.get("forwardHoldControlMilliseconds"),
  "forwardHoldControlMilliseconds",
);
if (
  forwardHoldControlMilliseconds !== null &&
  forwardHoldControlMilliseconds > 600_000
) {
  throw new Error(
    "forwardHoldControlMilliseconds must be from 1 through 600000",
  );
}
const forwardStopSettleMilliseconds = Number(
  searchParams.get("forwardStopSettleMilliseconds") ?? 10_000,
);
if (
  !Number.isInteger(forwardStopSettleMilliseconds) ||
  forwardStopSettleMilliseconds < 0 ||
  forwardStopSettleMilliseconds > 600_000
) {
  throw new Error(
    "forwardStopSettleMilliseconds must be an integer from 0 through 600000",
  );
}
const forwardSeconds = Number(searchParams.get("forwardSeconds") ?? 540);
if (
  !Number.isInteger(forwardSeconds) ||
  forwardSeconds < 1 ||
  forwardSeconds > 600
) {
  throw new Error("forwardSeconds must be an integer from 1 through 600");
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
  forwardDevelopment ||
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
  capabilityProfile === "user32-def-window-proc-a-action-continuation" ||
  capabilityProfile === "user32-peek-message-a-pointer-release-continuation" ||
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
let forwardVisibleFrameObserved = false;
let forwardPointerMoveSent = false;
let forwardHoverPresented = false;
let forwardActiveAction = 0;
let forwardCompletedActions = 0;
let forwardButtonDown = false;
let forwardNextInputSequence = 1;
let forwardKeyboardCampaignReady = false;
let forwardKeyboardButtonDown = false;
let forwardControlButtonDown = false;
let forwardCompletedKeyboardActivations = 0;
let forwardPrimaryActionTargetReady = false;
let forwardTitleActionDelayScheduled = false;
let forwardTitleActionDelayElapsed = false;
let forwardCampaignPhase:
  | "startup-pointer"
  | "startup-keyboard"
  | "title-start-pointer"
  | "title-response"
  | "scenario-pointer"
  | "complete" = "startup-pointer";
let forwardKeyboardActivationLimit = 2;
const forwardStartupKeyboardActivationCount = 0;
// Keep advancing dialogue for the whole bounded forward run. The event and
// semantic reports are independently ring-bounded, so this is only a safety
// ceiling and not a checkpoint endpoint.
const forwardScenarioPointerActionCount = 4_096;
const forwardScenarioPointerCoordinates = forwardPointerOverrides.get(0) ?? {
  normalizedX: 0x8000,
  normalizedY: 0x8000,
};
const defaultForwardPointerActions = [
  { normalizedX: 0x3a1a, normalizedY: 0xcfd2 },
  // Native title trace: client (276, 592) after moving onto Start.
  { normalizedX: 0x374d, normalizedY: 0xd2a2 },
  // The scenario action binding uses the primary pointer button. Default to
  // the viewport center; action-zero configuration can select a measured safe
  // scenario point without repeating it for the whole campaign.
  ...Array.from(
    { length: forwardScenarioPointerActionCount },
    () => forwardScenarioPointerCoordinates,
  ),
] as const;
const forwardPointerActions = defaultForwardPointerActions.map(
  (coordinates, index) => forwardPointerOverrides.get(index + 1) ?? coordinates,
);
for (const action of forwardPointerOverrides.keys()) {
  if (action > forwardPointerActions.length) {
    throw new Error(
      `forwardPointerOverrides action ${action} exceeds the ${forwardPointerActions.length}-action campaign`,
    );
  }
}
for (const action of forwardPointerHoldOverrides.keys()) {
  if (action > forwardPointerActions.length) {
    throw new Error(
      `forwardPointerHoldOverrides action ${action} exceeds the ${forwardPointerActions.length}-action campaign`,
    );
  }
}
for (const action of forwardPointerStaticActions) {
  if (action > forwardPointerActions.length) {
    throw new Error(
      `forwardPointerStaticActions action ${action} exceeds the ${forwardPointerActions.length}-action campaign`,
    );
  }
}
if (
  stopAfterForwardPointerAction !== null &&
  stopAfterForwardPointerAction > forwardPointerActions.length
) {
  throw new Error(
    `forwardStop action ${stopAfterForwardPointerAction} exceeds the ${forwardPointerActions.length}-action campaign`,
  );
}
if (
  forwardEnterAfterPointerAction !== null &&
  forwardEnterAfterPointerAction !== stopAfterForwardPointerAction &&
  !(
    forwardHoldControlMilliseconds !== null &&
    forwardEnterAfterPointerAction === forwardHoldControlAfterPointerAction &&
    stopAfterForwardPointerAction !== null &&
    forwardEnterAfterPointerAction < stopAfterForwardPointerAction
  )
) {
  throw new Error(
    "forwardEnterAfterPointerAction must match the terminal stop or precede it with a finite Control hold",
  );
}
if (
  forwardHoldControlAfterPointerAction !== null &&
  (forwardHoldControlMilliseconds === null
    ? forwardHoldControlAfterPointerAction !== stopAfterForwardPointerAction
    : stopAfterForwardPointerAction === null ||
      forwardHoldControlAfterPointerAction > stopAfterForwardPointerAction)
) {
  throw new Error(
    "forwardHoldControlAfterPointerAction must match the terminal stop or precede it when a hold duration is set",
  );
}
if (
  forwardHoldControlMilliseconds !== null &&
  forwardHoldControlAfterPointerAction === null
) {
  throw new Error(
    "forwardHoldControlMilliseconds requires forwardHoldControlAfterPointerAction",
  );
}
if (
  forwardEnterAfterPointerAction !== null &&
  forwardHoldControlAfterPointerAction !== null &&
  forwardHoldControlMilliseconds === null
) {
  throw new Error(
    "combined forward Enter and Control activation requires a finite Control hold",
  );
}
const forwardStartupPointerActionCount = 1;
const forwardTitlePointerActionCount = 1;
const forwardTitleActionDelayMilliseconds = 1_250;
const forwardPointerHoldMilliseconds = 117;
const heartbeatIntervalMilliseconds = forwardDevelopment ? 50 : 5;
const heartbeat = window.setInterval(() => {
  const now = performance.now();
  maximumHeartbeatGapMilliseconds = Math.max(
    maximumHeartbeatGapMilliseconds,
    now - lastHeartbeat,
  );
  lastHeartbeat = now;
  heartbeatSamples += 1;
}, heartbeatIntervalMilliseconds);

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
  const nextAction = forwardCompletedActions + 1;
  const phaseAcceptsNextAction =
    (forwardCampaignPhase === "startup-pointer" &&
      nextAction <= forwardStartupPointerActionCount) ||
    (forwardInputMode === "campaign" &&
      forwardCampaignPhase === "title-start-pointer" &&
      nextAction ===
        forwardStartupPointerActionCount + forwardTitlePointerActionCount) ||
    (forwardInputMode === "campaign" &&
      forwardCampaignPhase === "scenario-pointer" &&
      nextAction <= forwardPointerActions.length);
  if (
    !forwardDevelopment ||
    (!forwardVisibleFrameObserved &&
      !forwardPointerStaticActions.has(nextAction)) ||
    !phaseAcceptsNextAction ||
    forwardPointerMoveSent
  ) {
    return;
  }
  const coordinates = forwardPointerActions[forwardCompletedActions];
  if (coordinates === undefined) {
    return;
  }
  forwardPointerMoveSent = true;
  worker.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event: pointerPositionSourceEvent(
      forwardNextInputSequence++,
      coordinates.normalizedX,
      coordinates.normalizedY,
    ),
  } satisfies MainToWorkerMessage);
  if (forwardPointerStaticActions.has(nextAction)) {
    // Worker messages from this producer are FIFO: queue the Rust-owned cursor
    // move before the button event, but do not require a cosmetic WM_MOUSEMOVE
    // callback or Present from menu surfaces that poll GetCursorPos instead.
    forwardHoverPresented = true;
    pressForwardAction();
  }
}

function pressForwardAction(): void {
  const nextAction = forwardCompletedActions + 1;
  if (
    !forwardPointerMoveSent ||
    !forwardHoverPresented ||
    (!forwardVisibleFrameObserved &&
      !forwardPointerStaticActions.has(nextAction)) ||
    (forwardCampaignPhase !== "startup-pointer" &&
      forwardCampaignPhase !== "title-start-pointer" &&
      forwardCampaignPhase !== "scenario-pointer") ||
    forwardButtonDown ||
    forwardActiveAction !== forwardCompletedActions ||
    forwardCompletedActions >= forwardPointerActions.length
  ) {
    return;
  }
  forwardActiveAction = forwardCompletedActions + 1;
  const coordinates = forwardPointerActions[forwardActiveAction - 1];
  if (coordinates === undefined) {
    return;
  }
  const down = pointerSourceEvent(
    forwardNextInputSequence++,
    "button-down",
    0,
    coordinates.normalizedX,
    coordinates.normalizedY,
  );
  if (down === null) {
    fail(
      new Error("forward pointer action is outside the bounded source mapping"),
    );
    return;
  }
  forwardButtonDown = true;
  worker.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event: down,
  } satisfies MainToWorkerMessage);
}

function releaseForwardAction(action: number): void {
  if (!forwardButtonDown || action !== forwardActiveAction) {
    return;
  }
  const coordinates = forwardPointerActions[action - 1];
  if (coordinates === undefined) {
    return;
  }
  const up = pointerSourceEvent(
    forwardNextInputSequence++,
    "button-up",
    0,
    coordinates.normalizedX,
    coordinates.normalizedY,
  );
  if (up === null) {
    fail(
      new Error(
        "forward pointer release is outside the bounded source mapping",
      ),
    );
    return;
  }
  forwardButtonDown = false;
  worker.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event: up,
  } satisfies MainToWorkerMessage);
  const nextCoordinates = forwardPointerActions[action];
  if (
    action !== stopAfterForwardPointerAction &&
    forwardPointerStaticActions.has(action) &&
    forwardPointerStaticActions.has(action + 1) &&
    nextCoordinates !== undefined &&
    coordinates.normalizedX === nextCoordinates.normalizedX &&
    coordinates.normalizedY === nextCoordinates.normalizedY
  ) {
    // FIFO preserves UP before the next DOWN. Explicit identical static
    // actions model a real double-click without waiting for a cosmetic
    // release Present that would exceed the Windows double-click interval.
    scheduleNextForwardAction(action);
  }
}

function startForwardControlHold(action: number): void {
  if (forwardControlButtonDown) {
    return;
  }
  const down = keyboardSourceEvent(
    forwardNextInputSequence++,
    "key-down",
    "ControlLeft",
    false,
  );
  if (down === null) {
    fail(new Error("forward Control input is outside the keyboard mapping"));
    return;
  }
  forwardControlButtonDown = true;
  worker.postMessage({
    type: "keyboard-input",
    protocolVersion,
    sessionId,
    event: down,
  } satisfies MainToWorkerMessage);
  if (forwardHoldControlMilliseconds !== null) {
    window.setTimeout(() => {
      const up = keyboardSourceEvent(
        forwardNextInputSequence++,
        "key-up",
        "ControlLeft",
        false,
      );
      if (up === null) {
        fail(new Error("forward Control release is outside the keyboard mapping"));
        return;
      }
      forwardControlButtonDown = false;
      worker.postMessage({
        type: "keyboard-input",
        protocolVersion,
        sessionId,
        event: up,
      } satisfies MainToWorkerMessage);
      if (action === stopAfterForwardPointerAction) {
        forwardCampaignPhase = "complete";
        window.setTimeout(
          () => window.__vineBgiCheck.cancel(),
          forwardStopSettleMilliseconds,
        );
      } else {
        forwardCampaignPhase = "scenario-pointer";
        scheduleNextForwardAction(action);
      }
    }, forwardHoldControlMilliseconds);
    return;
  }
  forwardCampaignPhase = "complete";
  window.setTimeout(
    () => window.__vineBgiCheck.cancel(),
    forwardStopSettleMilliseconds,
  );
}

function handleForwardPointerActionCompletion(action: number): boolean {
  if (action === forwardEnterAfterPointerAction) {
    if (forwardCampaignPhase === "complete") {
      return true;
    }
    forwardCampaignPhase = "startup-keyboard";
    forwardKeyboardActivationLimit = forwardCompletedKeyboardActivations + 1;
    forwardKeyboardCampaignReady = true;
    forwardVisibleFrameObserved = true;
    pressForwardKeyboardActivation();
    forwardCampaignPhase = "complete";
    return true;
  }
  if (action === forwardHoldControlAfterPointerAction) {
    if (forwardCampaignPhase !== "complete") {
      startForwardControlHold(action);
    }
    return true;
  }
  if (action !== stopAfterForwardPointerAction) {
    return false;
  }
  if (forwardCampaignPhase === "complete") {
    return true;
  }
  forwardCampaignPhase = "complete";
  window.setTimeout(
    () => window.__vineBgiCheck.cancel(),
    forwardStopSettleMilliseconds,
  );
  return true;
}

function scheduleNextForwardAction(action: number): void {
  if (action !== forwardActiveAction || action <= forwardCompletedActions) {
    return;
  }
  forwardCompletedActions = action;
  forwardPointerMoveSent = false;
  forwardHoverPresented = false;
  if (forwardCampaignPhase === "title-start-pointer") {
    forwardCampaignPhase = "title-response";
    return;
  }
  if (forwardCampaignPhase === "scenario-pointer") {
    const completedScenarioActions =
      action -
      forwardStartupPointerActionCount -
      forwardTitlePointerActionCount;
    if (completedScenarioActions >= forwardScenarioPointerActionCount) {
      forwardCampaignPhase = "complete";
      return;
    }
    const previousCoordinates = forwardPointerActions[action - 1];
    const nextCoordinates = forwardPointerActions[action];
    if (
      previousCoordinates !== undefined &&
      nextCoordinates !== undefined &&
      previousCoordinates.normalizedX === nextCoordinates.normalizedX &&
      previousCoordinates.normalizedY === nextCoordinates.normalizedY
    ) {
      // The cursor remains at the same guest-visible position after release.
      // Reuse that observed hover instead of forcing another cosmetic Present
      // between identical scenario actions.
      forwardPointerMoveSent = true;
      forwardHoverPresented = true;
      pressForwardAction();
    } else {
      maybePostForwardAction();
    }
    return;
  }
  if (
    forwardCampaignPhase !== "startup-pointer" ||
    action < forwardStartupPointerActionCount
  ) {
    return;
  }
  if (forwardPrimaryActionTargetReady) {
    schedulePostStartupPointerAction();
    return;
  }
  if (
    forwardCompletedKeyboardActivations < forwardStartupKeyboardActivationCount
  ) {
    forwardCampaignPhase = "startup-keyboard";
    forwardKeyboardCampaignReady = true;
    forwardKeyboardActivationLimit = forwardStartupKeyboardActivationCount;
    scheduleForwardKeyboardActivation();
    return;
  }
  // The fingerprinted target has one click-through warning before the title.
  // Its D3D transition can lag the release by many Presents, so wait for the
  // title signature instead of blindly clicking the next visible frame.
}

function pressForwardKeyboardActivation(): void {
  if (
    !forwardKeyboardCampaignReady ||
    !forwardVisibleFrameObserved ||
    forwardCampaignPhase !== "startup-keyboard" ||
    forwardKeyboardButtonDown ||
    forwardCompletedKeyboardActivations >= forwardKeyboardActivationLimit
  ) {
    return;
  }
  const down = keyboardSourceEvent(
    forwardNextInputSequence++,
    "key-down",
    "Enter",
    false,
  );
  if (down === null) {
    fail(new Error("forward Enter activation is outside the keyboard mapping"));
    return;
  }
  forwardKeyboardButtonDown = true;
  worker.postMessage({
    type: "keyboard-input",
    protocolVersion,
    sessionId,
    event: down,
  } satisfies MainToWorkerMessage);
}

function releaseForwardKeyboardActivation(action: number): void {
  if (
    !forwardKeyboardButtonDown ||
    action !== forwardCompletedKeyboardActivations + 1
  ) {
    return;
  }
  const up = keyboardSourceEvent(
    forwardNextInputSequence++,
    "key-up",
    "Enter",
    false,
  );
  if (up === null) {
    fail(new Error("forward Enter release is outside the keyboard mapping"));
    return;
  }
  forwardKeyboardButtonDown = false;
  worker.postMessage({
    type: "keyboard-input",
    protocolVersion,
    sessionId,
    event: up,
  } satisfies MainToWorkerMessage);
}

function scheduleForwardKeyboardActivation(): void {
  if (
    forwardKeyboardButtonDown ||
    forwardCompletedKeyboardActivations >= forwardKeyboardActivationLimit
  ) {
    return;
  }
  forwardKeyboardCampaignReady = true;
  pressForwardKeyboardActivation();
}

function schedulePostStartupPointerAction(): void {
  if (
    forwardCampaignPhase !== "startup-pointer" ||
    forwardCompletedActions !== forwardStartupPointerActionCount ||
    forwardCompletedActions !== forwardActiveAction ||
    forwardButtonDown ||
    forwardCompletedKeyboardActivations !==
      forwardStartupKeyboardActivationCount ||
    !forwardPrimaryActionTargetReady ||
    forwardCompletedActions >= forwardPointerActions.length
  ) {
    return;
  }
  if (!forwardTitleActionDelayElapsed) {
    if (forwardTitleActionDelayScheduled) {
      return;
    }
    forwardTitleActionDelayScheduled = true;
    window.setTimeout(() => {
      forwardTitleActionDelayScheduled = false;
      forwardTitleActionDelayElapsed = true;
      schedulePostStartupPointerAction();
    }, forwardTitleActionDelayMilliseconds);
    return;
  }
  forwardKeyboardCampaignReady = false;
  forwardCampaignPhase = "title-start-pointer";
  forwardPointerMoveSent = false;
  forwardHoverPresented = false;
  maybePostForwardAction();
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
          "Forward stable visible frame observed; accepting live development input"
      ) {
        forwardVisibleFrameObserved = true;
        maybePostForwardAction();
        pressForwardAction();
        if (forwardCampaignPhase === "startup-keyboard") {
          pressForwardKeyboardActivation();
        }
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail ===
          "Forward visible frame lost; waiting for stable output"
      ) {
        forwardVisibleFrameObserved = false;
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail === "Forward primary pointer hover frame presented"
      ) {
        forwardHoverPresented = true;
        pressForwardAction();
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail === "Forward primary pointer hover consumed by guest" &&
        (forwardCampaignPhase === "title-start-pointer" ||
          forwardPointerStaticActions.has(forwardCompletedActions + 1))
      ) {
        // An explicitly static surface does not necessarily Present after a
        // hover-only update. Its guest-consumed mouse move is sufficient to
        // deliver the primary button without deadlocking on a cosmetic frame.
        forwardHoverPresented = true;
        pressForwardAction();
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail === "Forward primary action target ready"
      ) {
        // The title-specific signature is stronger than the generic populated-
        // frame streak, so it is safe to accept input without waiting for eight
        // additional Presents on the slow private-target main loop.
        forwardVisibleFrameObserved = true;
        forwardPrimaryActionTargetReady = true;
        schedulePostStartupPointerAction();
      }
      if (
        forwardDevelopment &&
        message.phase === "running" &&
        message.detail === "Forward primary action visible response observed"
      ) {
        if (stopAfterForwardTitleAction) {
          forwardCampaignPhase = "complete";
          window.__vineBgiCheck.cancel();
          return;
        }
        // The first response can be the title's black transition frame. Wait for
        // the worker to announce the next stable visible frame before sending
        // scenario input.
        forwardVisibleFrameObserved = false;
        forwardCampaignPhase = "scenario-pointer";
        forwardKeyboardCampaignReady = false;
        maybePostForwardAction();
      }
      if (forwardDevelopment && message.phase === "running") {
        const enteredPress =
          /^Forward pointer action (\d+) press entered guest$/u.exec(
            message.detail,
          );
        if (enteredPress !== null) {
          const action = Number(enteredPress[1]);
          window.setTimeout(
            () => releaseForwardAction(action),
            forwardPointerHoldOverrides.get(action) ??
              forwardPointerHoldMilliseconds,
          );
        }
        const consumedRelease =
          /^Forward pointer action (\d+) release consumed by guest$/u.exec(
            message.detail,
          );
        if (consumedRelease !== null) {
          const action = Number(consumedRelease[1]);
          if (handleForwardPointerActionCompletion(action)) {
            return;
          }
          if (forwardPointerStaticActions.has(action + 1)) {
            scheduleNextForwardAction(action);
          }
        }
        const release =
          /^Forward pointer action (\d+) release frame presented$/u.exec(
            message.detail,
          );
        if (release !== null) {
          const action = Number(release[1]);
          if (handleForwardPointerActionCompletion(action)) {
            return;
          }
          scheduleNextForwardAction(action);
        }
        const activationPress =
          /^Forward activation action (\d+) press frame presented$/u.exec(
            message.detail,
          );
        const activationObserved =
          /^Forward activation action (\d+) press observed by guest$/u.exec(
            message.detail,
          );
        if (
          activationObserved !== null &&
          forwardEnterAfterPointerAction !== null &&
          forwardCampaignPhase === "complete"
        ) {
          releaseForwardKeyboardActivation(Number(activationObserved[1]));
        }
        if (activationPress !== null) {
          releaseForwardKeyboardActivation(Number(activationPress[1]));
        }
        const activationRelease =
          /^Forward activation action (\d+) release frame presented$/u.exec(
            message.detail,
          );
        if (activationRelease !== null) {
          forwardCompletedKeyboardActivations = Number(activationRelease[1]);
          if (
            forwardEnterAfterPointerAction !== null &&
            forwardCampaignPhase === "complete"
          ) {
            if (
              forwardHoldControlAfterPointerAction ===
              forwardEnterAfterPointerAction
            ) {
              startForwardControlHold(forwardEnterAfterPointerAction);
            } else {
              window.setTimeout(
                () => window.__vineBgiCheck.cancel(),
                forwardStopSettleMilliseconds,
              );
            }
            return;
          }
          if (
            forwardCompletedKeyboardActivations ===
            forwardStartupKeyboardActivationCount
          ) {
            schedulePostStartupPointerAction();
          } else {
            scheduleForwardKeyboardActivation();
          }
        }
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
      void pcmRenderer
        .play(message)
        .then((receipt) => {
          if (!receipt.contentUpdated && !receipt.staleMessageIgnored) {
            audioPlaybacksStarted += 1;
          }
          latestAudioPlayback = receipt;
          document.body.dataset.audioPlayback = JSON.stringify(receipt);
          if (
            stopAfterForwardLoopingAudioUpdate &&
            message.playback === "playing-looping" &&
            receipt.contentUpdated &&
            !receipt.staleMessageIgnored
          ) {
            window.__vineBgiCheck.cancel();
          }
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

function parseForwardPointerOverrides(
  value: string | null,
): Map<number, { normalizedX: number; normalizedY: number }> {
  const overrides = new Map<
    number,
    { normalizedX: number; normalizedY: number }
  >();
  if (value === null) {
    return overrides;
  }
  if (value.length === 0) {
    throw new Error("forwardPointerOverrides must not be empty");
  }
  for (const entry of value.split(",")) {
    const match = /^(\d+):(\d+):(\d+)$/u.exec(entry);
    if (match === null) {
      throw new Error(
        "forwardPointerOverrides entries must be action:normalized-x:normalized-y",
      );
    }
    const action = Number(match[1]);
    const normalizedX = Number(match[2]);
    const normalizedY = Number(match[3]);
    if (!Number.isSafeInteger(action) || action < 0) {
      throw new Error(
        "forwardPointerOverrides action must be zero for the scenario default or a positive integer",
      );
    }
    if (
      !Number.isInteger(normalizedX) ||
      normalizedX < 0 ||
      normalizedX > 0xffff
    ) {
      throw new Error(
        "forwardPointerOverrides normalized x must be from 0 through 65535",
      );
    }
    if (
      !Number.isInteger(normalizedY) ||
      normalizedY < 0 ||
      normalizedY > 0xffff
    ) {
      throw new Error(
        "forwardPointerOverrides normalized y must be from 0 through 65535",
      );
    }
    if (overrides.has(action)) {
      throw new Error(
        `forwardPointerOverrides action ${action} must be unique`,
      );
    }
    overrides.set(action, { normalizedX, normalizedY });
  }
  return overrides;
}

function parseForwardPointerHoldOverrides(
  value: string | null,
): Map<number, number> {
  const overrides = new Map<number, number>();
  if (value === null) {
    return overrides;
  }
  if (value.length === 0) {
    throw new Error("forwardPointerHoldOverrides must not be empty");
  }
  for (const entry of value.split(",")) {
    const match = /^(\d+):(\d+)$/u.exec(entry);
    if (match === null) {
      throw new Error(
        "forwardPointerHoldOverrides entries must be action:milliseconds",
      );
    }
    const action = Number(match[1]);
    const milliseconds = Number(match[2]);
    if (!Number.isSafeInteger(action) || action <= 0) {
      throw new Error(
        "forwardPointerHoldOverrides action must be a positive integer",
      );
    }
    if (
      !Number.isInteger(milliseconds) ||
      milliseconds < 1 ||
      milliseconds > 60_000
    ) {
      throw new Error(
        "forwardPointerHoldOverrides milliseconds must be from 1 through 60000",
      );
    }
    if (overrides.has(action)) {
      throw new Error(
        `forwardPointerHoldOverrides action ${action} must be unique`,
      );
    }
    overrides.set(action, milliseconds);
  }
  return overrides;
}

function parseForwardPointerStaticActions(value: string | null): Set<number> {
  const actions = new Set<number>();
  if (value === null) {
    return actions;
  }
  if (value.length === 0) {
    throw new Error("forwardPointerStaticActions must not be empty");
  }
  for (const entry of value.split(",")) {
    if (!/^(?:0|[1-9]\d*)$/u.test(entry)) {
      throw new Error(
        "forwardPointerStaticActions entries must be positive action numbers",
      );
    }
    const action = Number(entry);
    if (!Number.isSafeInteger(action) || action <= 0) {
      throw new Error(
        "forwardPointerStaticActions action must be a positive integer",
      );
    }
    if (actions.has(action)) {
      throw new Error(
        `forwardPointerStaticActions action ${action} must be unique`,
      );
    }
    actions.add(action);
  }
  return actions;
}

function parseOptionalPositiveInteger(
  value: string | null,
  label: string,
): number | null {
  if (value === null) {
    return null;
  }
  if (!/^[1-9]\d*$/u.test(value)) {
    throw new Error(`${label} must be a positive integer`);
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${label} exceeds the safe integer range`);
  }
  return parsed;
}

const startMessage = {
  type: "start-session",
  protocolVersion,
  sessionId,
  configuration: {
    mode: scenario,
    evidenceLevel,
    workload: "private-bgi",
    ...(!forwardDevelopment && capabilityProfile !== undefined
      ? { capabilityProfile }
      : {}),
    ...(forwardDevelopment ? { developmentMode: "forward" as const } : {}),
    executableUrl: "./private/BGI.exe",
    policyUrl: "./private/kernel32-policy.json",
    instructionBudget:
      scenario === "watchdog" ? 4 : forwardDevelopment ? 524_288 : 16_384,
    // The release JIT sustains more than 80M guest instructions/second on the
    // private target. Keep the wall deadline authoritative for forward runs.
    watchdogInstructionLimit: forwardDevelopment
      ? Math.max(12_000_000_000, forwardSeconds * 100_000_000)
      : scenario === "watchdog"
        ? 1
        : capabilityProfile === "user32-get-cursor-info" ||
            capabilityProfile === "d3d9-update-texture-partial-dirty-region" ||
            capabilityProfile ===
              "d3d9-begin-scene-post-partial-dirty-region" ||
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
                          capabilityProfile ===
                            "primary-main-loop-input-progress-gateway" ||
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
                          capabilityProfile ===
                            "user32-set-focus-action-continuation" ||
                          capabilityProfile ===
                            "winmm-time-get-time-action-continuation" ||
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
                        : capabilityProfile ===
                              "runtime-trapless-execution-budget" ||
                            capabilityProfile ===
                              "kernel32-heap-free-large-buffer" ||
                            capabilityProfile ===
                              "kernel32-heap-alloc-24-byte-buffer" ||
                            capabilityProfile ===
                              "winmm-time-get-time-post-heap-reuse" ||
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
                            : capabilityProfile ===
                                  "user32-async-key-state-initial-scan" ||
                                capabilityProfile ===
                                  "user32-async-key-state-initial-scan-continuation" ||
                                capabilityProfile === "user32-peek-message-a" ||
                                capabilityProfile === "user32-is-iconic"
                              ? 180_000_000
                              : capabilityProfile ===
                                    "kernel32-create-directory-a" ||
                                  capabilityProfile ===
                                    "user32-set-window-pos-repeat-activation" ||
                                  capabilityProfile ===
                                    "d3d9-recreated-textures" ||
                                  capabilityProfile ===
                                    "user32-set-window-pos-frame-change-move" ||
                                  capabilityProfile ===
                                    "user32-show-window-normal" ||
                                  capabilityProfile ===
                                    "user32-update-window" ||
                                  capabilityProfile === "d3d9-clear" ||
                                  capabilityProfile === "user32-set-rect" ||
                                  capabilityProfile === "d3d9-add-dirty-rect" ||
                                  capabilityProfile === "d3d9-update-texture" ||
                                  capabilityProfile === "d3d9-begin-scene" ||
                                  capabilityProfile ===
                                    "d3d9-initial-sampler-states" ||
                                  capabilityProfile ===
                                    "d3d9-initial-texture-stage-states" ||
                                  capabilityProfile === "d3d9-set-texture" ||
                                  capabilityProfile === "d3d9-set-fvf" ||
                                  capabilityProfile ===
                                    "d3d9-set-stream-source" ||
                                  capabilityProfile === "d3d9-draw-primitive" ||
                                  capabilityProfile === "d3d9-end-scene" ||
                                  capabilityProfile ===
                                    "d3d9-post-end-scene-raster-status" ||
                                  capabilityProfile ===
                                    "d3d9-post-raster-status-waitable-timer" ||
                                  capabilityProfile ===
                                    "d3d9-post-scene-present-gateway" ||
                                  capabilityProfile === "d3d9-first-present" ||
                                  capabilityProfile ===
                                    "d3d9-post-present-continuation" ||
                                  capabilityProfile ===
                                    "user32-async-key-state-input" ||
                                  capabilityProfile ===
                                    "user32-async-key-state-input-continuation"
                                ? 35_000_000
                                : 20_000_000,
    watchdogWallMilliseconds: forwardDevelopment
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
                "primary-main-loop-presentation-progress-gateway" ||
              capabilityProfile ===
                "primary-main-loop-input-progress-gateway" ||
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
    capabilityProfile ===
      "primary-main-loop-pointer-message-action-progress-gateway" ||
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
    capabilityProfile ===
      "user32-is-iconic-post-pointer-release-continuation" ||
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
    capabilityProfile ===
      "primary-main-loop-pointer-message-action-progress-gateway" ||
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
    capabilityProfile ===
      "user32-is-iconic-post-pointer-release-continuation" ||
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
    const downEvent = pointerSourceEvent(1, "button-down", 0, 0x8000, 0x8000);
    const upEvent = pointerSourceEvent(2, "button-up", 0, 0x8000, 0x8000);
    if (downEvent === null || upEvent === null) {
      throw new Error(
        "primary pointer button is outside the bounded source mapping",
      );
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
        throw new Error(
          "primary action pointer button is outside the bounded source mapping",
        );
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
      report.outcome === "watchdog" ||
      (forwardStop !== null && report.outcome === "cancelled");
    if (!accepted) {
      fail(
        new Error(
          report.error ?? `unexpected forward outcome ${report.outcome}`,
        ),
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
      audioPacing: pcmRenderer.pacingSnapshot(),
    };
    window.__vineBgiCheck.result = evidence;
    document.body.dataset.checkStatus = "passed";
    document.body.dataset.report = JSON.stringify(evidence);
    status.textContent = report.outcome;
    // The profiling runner closes the page after collecting the stopped Worker's CPU profile.
    if (searchParams.get("cpuProfile") !== "1") {
      worker.terminate();
    }
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
    audioPacing: pcmRenderer.pacingSnapshot(),
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
  return value === "user32-set-window-text-a-post-scheduled-shared-clock" ||
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
    value === "user32-peek-message-a-post-test-cooperative-level-timer-wait" ||
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
    value === "user32-get-async-key-state-post-is-iconic-clock-continuation" ||
    value === "d3d9-test-cooperative-level-post-input-scan-continuation" ||
    value === "d3d9-post-input-scan-waitable-timer-cycle" ||
    value === "user32-window-timer-callback-post-waitable-timer-cycle" ||
    value === "user32-peek-message-a-post-window-timer-callback-continuation" ||
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
