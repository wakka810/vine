import { BrowserPcmRenderer } from "./audio-renderer.js";
import {
  focusSourceEvent,
  keyboardSourceEvent,
  pointerButtonForBrowserButton,
  pointerPositionSourceEvent,
  pointerSourceEvent,
} from "./keyboard-input.js";
import { observePageLifecycle } from "./page-lifecycle.js";
import {
  assertProtocolVersion,
  type MainToWorkerMessage,
  protocolVersion,
  type RuntimeKeyboardInputEvent,
  type RuntimePointerButtonInputEvent,
  type RuntimePointerPositionInputEvent,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";

type PlayMode = "normal" | "performance";

declare global {
  interface Window {
    __vinePlay: {
      mode: PlayMode;
      started: boolean;
      status: string;
      error: string | null;
      result: SessionReport | null;
      audioPlaybacks: number;
      audioStops: number;
      start(): Promise<void>;
      cancel(): void;
    };
  }
}

const mode = parseMode(new URL(location.href).searchParams.get("mode"));
const canvas = requiredElement<HTMLCanvasElement>("#surface");
const startButton = requiredElement<HTMLButtonElement>("#start");
const stopButton = requiredElement<HTMLButtonElement>("#stop");
const status = requiredElement<HTMLOutputElement>("#status");
requiredElement<HTMLAnchorElement>(`#${mode}-mode`).ariaCurrent = "page";
document.body.dataset.mode = mode;

const sessionId = 1;
const pcmRenderer = new BrowserPcmRenderer();
const pressedPointerButtons = new Set<number>();
let worker: Worker | null = null;
let nextInputSequence = 1;
let inputSourceFocused = true;
let pendingPointerPosition: { normalizedX: number; normalizedY: number } | null = null;
let pointerAnimationFrame = 0;

const play: Window["__vinePlay"] = {
  mode,
  started: false,
  status: status.textContent ?? "",
  error: null,
  result: null,
  audioPlaybacks: 0,
  audioStops: 0,
  start,
  cancel,
};
window.__vinePlay = play;

const pageLifecycle = observePageLifecycle((lifecycle) => {
  worker?.postMessage({
    type: "page-lifecycle",
    protocolVersion,
    sessionId,
    lifecycle,
  } satisfies MainToWorkerMessage);
});

startButton.addEventListener("click", () => void start());
stopButton.addEventListener("click", cancel);
window.addEventListener("keydown", (event) => handleKeyboard(event, "key-down"));
window.addEventListener("keyup", (event) => handleKeyboard(event, "key-up"));
window.addEventListener("blur", () => sendFocus(false));
window.addEventListener("focus", () => sendFocus(true));
canvas.addEventListener("pointerdown", (event) => handlePointerButton(event, "button-down"));
canvas.addEventListener("pointerup", (event) => handlePointerButton(event, "button-up"));
canvas.addEventListener("pointermove", handlePointerMove);
canvas.addEventListener("pointercancel", handlePointerCancel);
canvas.addEventListener("contextmenu", (event) => event.preventDefault());

async function start(): Promise<void> {
  if (worker !== null) {
    return;
  }
  startButton.disabled = true;
  setStatus("音声とRuntimeを準備中");
  try {
    await pcmRenderer.activate();
    const workerUrl = new URL("./runtime-worker.js", import.meta.url);
    workerUrl.searchParams.set("manualPlay", "1");
    if (mode === "performance") {
      workerUrl.searchParams.set("executionMode", "play-performance");
    }
    worker = new Worker(workerUrl, { type: "module" });
    worker.addEventListener("message", handleWorkerMessage);
    worker.addEventListener("error", (event) => fail(new Error(event.message)));
    const d3d9Canvas = canvas.transferControlToOffscreen();
    worker.postMessage(
      {
        type: "start-session",
        protocolVersion,
        sessionId,
        configuration: {
          mode: "success",
          evidenceLevel: "summary",
          workload: "private-bgi",
          developmentMode: "forward",
          executableUrl: "./private/BGI.exe",
          policyUrl: "./private/kernel32-policy.json",
          instructionBudget: 524_288,
          watchdogInstructionLimit: 8_000_000_000_000,
          watchdogWallMilliseconds: 12 * 60 * 60 * 1_000,
          pageLifecycle: pageLifecycle.snapshot(),
        },
        d3d9Canvas,
      } satisfies MainToWorkerMessage,
      [d3d9Canvas],
    );
    play.started = true;
    stopButton.disabled = false;
    canvas.focus({ preventScroll: true });
  } catch (error) {
    fail(error instanceof Error ? error : new Error(String(error)));
  }
}

function handleWorkerMessage(event: MessageEvent<WorkerToMainMessage>): void {
  const message = event.data;
  assertProtocolVersion(message.protocolVersion);
  if (message.sessionId !== sessionId) {
    return;
  }
  switch (message.type) {
    case "session-status":
      setStatus(message.detail);
      break;
    case "audio-playback":
      void pcmRenderer.play(message).then((receipt) => {
        if (!receipt.staleMessageIgnored) {
          play.audioPlaybacks += 1;
        }
      }).catch(fail);
      break;
    case "audio-stop":
      if (pcmRenderer.stop(message).sourceStopped) {
        play.audioStops += 1;
      }
      break;
    case "host-request":
      fail(new Error(`unsupported browser service ${message.service}`));
      break;
    case "session-finished":
      play.result = message.report;
      stopButton.disabled = true;
      setStatus(message.report.error ?? `停止: ${message.report.outcome}`);
      break;
  }
}

function handleKeyboard(event: KeyboardEvent, kind: "key-down" | "key-up"): void {
  if (worker === null || !inputSourceFocused) {
    return;
  }
  const input = keyboardSourceEvent(nextInputSequence, kind, event.code, event.repeat);
  if (input !== null) {
    event.preventDefault();
    sendKeyboard(input);
  }
}

function handlePointerMove(event: PointerEvent): void {
  if (worker === null || !inputSourceFocused) {
    return;
  }
  pendingPointerPosition = normalizedPointerPosition(event);
  if (pendingPointerPosition === null || pointerAnimationFrame !== 0) {
    return;
  }
  pointerAnimationFrame = requestAnimationFrame(() => {
    pointerAnimationFrame = 0;
    flushPointerPosition();
  });
  event.preventDefault();
}

function handlePointerButton(
  event: PointerEvent,
  kind: "button-down" | "button-up",
): void {
  if (
    worker === null ||
    !inputSourceFocused ||
    pointerButtonForBrowserButton(event.button) === null
  ) {
    return;
  }
  const position = normalizedPointerPosition(event);
  if (position === null) {
    return;
  }
  flushPointerPosition();
  const input = pointerSourceEvent(
    nextInputSequence,
    kind,
    event.button,
    position.normalizedX,
    position.normalizedY,
  );
  if (input === null) {
    return;
  }
  if (kind === "button-down") {
    if (pressedPointerButtons.has(event.button)) {
      return;
    }
    pressedPointerButtons.add(event.button);
    canvas.setPointerCapture(event.pointerId);
    canvas.focus({ preventScroll: true });
  } else if (!pressedPointerButtons.delete(event.button)) {
    return;
  }
  event.preventDefault();
  sendPointer(input);
}

function handlePointerCancel(event: PointerEvent): void {
  for (const button of [...pressedPointerButtons]) {
    const position = normalizedPointerPosition(event);
    if (position !== null) {
      const input = pointerSourceEvent(
        nextInputSequence,
        "button-up",
        button,
        position.normalizedX,
        position.normalizedY,
      );
      if (input !== null) {
        sendPointer(input);
      }
    }
    pressedPointerButtons.delete(button);
  }
}

function normalizedPointerPosition(
  event: PointerEvent,
): { normalizedX: number; normalizedY: number } | null {
  const rect = canvas.getBoundingClientRect();
  if (!(rect.width > 0 && rect.height > 0)) {
    return null;
  }
  return {
    normalizedX: normalizedPointerCoordinate(event.clientX, rect.left, rect.width),
    normalizedY: normalizedPointerCoordinate(event.clientY, rect.top, rect.height),
  };
}

function normalizedPointerCoordinate(coordinate: number, start: number, extent: number): number {
  return Math.max(0, Math.min(0xffff, Math.floor(((coordinate - start) * 0x1_0000) / extent)));
}

function flushPointerPosition(): void {
  if (pointerAnimationFrame !== 0) {
    cancelAnimationFrame(pointerAnimationFrame);
    pointerAnimationFrame = 0;
  }
  const position = pendingPointerPosition;
  pendingPointerPosition = null;
  if (position !== null) {
    sendPointer(
      pointerPositionSourceEvent(
        nextInputSequence,
        position.normalizedX,
        position.normalizedY,
      ),
    );
  }
}

function sendFocus(focused: boolean): void {
  if (worker === null || inputSourceFocused === focused) {
    return;
  }
  inputSourceFocused = focused;
  if (!focused) {
    pressedPointerButtons.clear();
    pendingPointerPosition = null;
  }
  sendKeyboard(focusSourceEvent(nextInputSequence, focused));
}

function sendKeyboard(event: RuntimeKeyboardInputEvent): void {
  worker?.postMessage({
    type: "keyboard-input",
    protocolVersion,
    sessionId,
    event,
  } satisfies MainToWorkerMessage);
  nextInputSequence += 1;
}

function sendPointer(
  event: RuntimePointerButtonInputEvent | RuntimePointerPositionInputEvent,
): void {
  worker?.postMessage({
    type: "pointer-input",
    protocolVersion,
    sessionId,
    event,
  } satisfies MainToWorkerMessage);
  nextInputSequence += 1;
}

function cancel(): void {
  worker?.postMessage({
    type: "cancel-session",
    protocolVersion,
    sessionId,
  } satisfies MainToWorkerMessage);
}

function setStatus(detail: string): void {
  status.textContent = detail;
  play.status = detail;
}

function fail(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  play.error = message;
  setStatus(message);
  cancel();
}

function parseMode(value: string | null): PlayMode {
  if (value === null || value === "performance") {
    return value ?? "performance";
  }
  if (value === "normal") {
    return value;
  }
  throw new Error("mode must be normal or performance");
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) {
    throw new Error(`missing element ${selector}`);
  }
  return element;
}
