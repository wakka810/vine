import { Cpu, createIcons, Database, Play, RotateCcw, Square, Volume2 } from "lucide";

import {
  assertProtocolVersion,
  type HostRequestMessage,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionMode,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";

interface RuntimeEvidence {
  report: SessionReport;
  heartbeatSamples: number;
  maximumHeartbeatGapMilliseconds: number;
  pixelRgba: string | null;
  toneHz: number | null;
  persistentByte: number | null;
}

interface StressSample {
  elapsedMilliseconds: number;
  usedJsHeapBytes: number | null;
  sessions: number;
}

interface StressEvidence {
  durationMilliseconds: number;
  sessions: number;
  samples: StressSample[];
  heapSlopeBytesPerMinute: number | null;
  maximumHeartbeatGapMilliseconds: number;
  failedSessions: number;
}

declare global {
  interface Window {
    __vineRuntime: {
      start(mode?: SessionMode): Promise<RuntimeEvidence>;
      cancel(): void;
      runStress(durationMilliseconds: number): Promise<StressEvidence>;
      snapshot(): Record<string, string | null>;
    };
  }

  interface Performance {
    memory?: { usedJSHeapSize: number };
  }
}

const worker = new Worker(new URL("./runtime-worker.js", import.meta.url), { type: "module" });
const runButton = requiredElement<HTMLButtonElement>("run-button");
const cancelButton = requiredElement<HTMLButtonElement>("cancel-button");
const resetButton = requiredElement<HTMLButtonElement>("reset-button");
const statusLabel = requiredElement<HTMLElement>("session-status");
const statusDot = requiredElement<HTMLElement>("status-dot");
const timeline = requiredElement<HTMLOListElement>("timeline");
const canvas = requiredElement<HTMLCanvasElement>("surface");
const instructionMetric = requiredElement<HTMLElement>("metric-instructions");
const yieldMetric = requiredElement<HTMLElement>("metric-yields");
const callbackMetric = requiredElement<HTMLElement>("metric-callbacks");
const generationMetric = requiredElement<HTMLElement>("metric-generation");
const audioMetric = requiredElement<HTMLElement>("audio-value");
const storageMetric = requiredElement<HTMLElement>("storage-value");
const heartbeatMetric = requiredElement<HTMLElement>("heartbeat-value");
const modeFromUrl = parseMode(new URL(location.href).searchParams.get("scenario"));

let nextSessionId = 1;
let activeSessionId: number | null = null;
let activeMode: SessionMode = modeFromUrl;
let resolveSession: ((evidence: RuntimeEvidence) => void) | null = null;
let rejectSession: ((error: Error) => void) | null = null;
let audioContext: AudioContext | null = null;
let heartbeatTimer: number | null = null;
let heartbeatSamples = 0;
let lastHeartbeat = 0;
let maximumHeartbeatGap = 0;
let pixelRgba: string | null = null;
let toneHz: number | null = null;
let persistentByte: number | null = null;
let hostServiceError: Error | null = null;

createIcons({
  icons: { Cpu, Database, Play, RotateCcw, Square, Volume2 },
  attrs: { "aria-hidden": "true", "stroke-width": 1.8 },
});
resetSurface();

runButton.addEventListener("click", () => {
  void startSession(modeFromUrl).catch(showFatalError);
});
cancelButton.addEventListener("click", cancelSession);
resetButton.addEventListener("click", resetView);

worker.addEventListener("message", (event: MessageEvent<WorkerToMainMessage>) => {
  const message = event.data;
  assertProtocolVersion(message.protocolVersion);
  if (message.sessionId !== activeSessionId) {
    return;
  }
  switch (message.type) {
    case "session-status":
      setStatus(message.phase, message.detail);
      addTimeline(message.phase, message.detail);
      break;
    case "host-request":
      void handleHostRequest(message);
      break;
    case "session-finished":
      finishSession(message.report);
      break;
  }
});

worker.addEventListener("error", (event) => {
  showFatalError(new Error(event.message));
});

async function startSession(mode: SessionMode = "success"): Promise<RuntimeEvidence> {
  if (activeSessionId !== null) {
    throw new Error("a runtime session is already active");
  }
  await ensureAudioContext();
  const sessionId = nextSessionId;
  nextSessionId += 1;
  activeSessionId = sessionId;
  activeMode = mode;
  hostServiceError = null;
  pixelRgba = null;
  toneHz = null;
  persistentByte = null;
  document.body.dataset.checkStatus = "running";
  document.body.dataset.error = "";
  document.body.dataset.outcome = "";
  document.body.dataset.report = "";
  timeline.replaceChildren();
  resetSurface();
  audioMetric.textContent = "Idle";
  storageMetric.textContent = "--";
  startHeartbeat();
  setControls(true);
  setStatus("starting", `Session ${sessionId}`);
  addTimeline("starting", `Mode: ${mode}`);

  const configuration = {
    mode,
    instructionBudget: mode === "watchdog" ? 4 : 8,
    watchdogInstructionLimit: mode === "watchdog" ? 1 : 100_000,
    watchdogWallMilliseconds: 10_000,
  };
  const message: MainToWorkerMessage = {
    type: "start-session",
    protocolVersion,
    sessionId,
    configuration,
  };
  worker.postMessage(message);
  return new Promise<RuntimeEvidence>((resolve, reject) => {
    resolveSession = resolve;
    rejectSession = reject;
  });
}

function cancelSession(): void {
  if (activeSessionId === null) {
    return;
  }
  worker.postMessage({
    type: "cancel-session",
    protocolVersion,
    sessionId: activeSessionId,
  } satisfies MainToWorkerMessage);
}

async function handleHostRequest(message: HostRequestMessage): Promise<void> {
  addTimeline("host", `${message.service} [${message.arguments.join(", ")}]`);
  if (activeMode === "cancel-pending") {
    window.setTimeout(cancelSession, 20);
    return;
  }
  let result = 0;
  try {
    switch (message.service) {
      case "browser.deferred-add":
        await delay(35);
        invariant(message.arguments.length === 2, "deferred add expects two arguments");
        result = ((message.arguments[0] ?? 0) + (message.arguments[1] ?? 0)) >>> 0;
        break;
      case "browser.commit-outputs":
        result = await commitOutputs(message.arguments);
        break;
      default:
        throw new Error(`unsupported browser service ${message.service}`);
    }
  } catch (error) {
    hostServiceError = error instanceof Error ? error : new Error(String(error));
  }
  if (activeSessionId !== message.sessionId) {
    return;
  }
  worker.postMessage({
    type: "host-completion",
    protocolVersion,
    sessionId: message.sessionId,
    requestId: message.requestId,
    result,
  } satisfies MainToWorkerMessage);
}

async function commitOutputs(arguments_: number[]): Promise<number> {
  invariant(arguments_.length === 3, "output commit expects three arguments");
  const rgba = arguments_[0] ?? 0;
  const frequency = arguments_[1] ?? 0;
  const byte = arguments_[2] ?? 0;
  invariant(rgba === 0xff00_00ff, "unexpected fixture RGBA value");
  invariant(frequency === 880, "unexpected fixture tone frequency");
  invariant(byte === 0x5a, "unexpected fixture persistent byte");
  presentPixel(rgba);
  await Promise.all([playPulse(frequency), persistByte(byte)]);
  return 1;
}

function presentPixel(rgba: number): void {
  const context = canvas.getContext("2d", { alpha: false });
  invariant(context !== null, "Canvas 2D is unavailable");
  const image = context.createImageData(2, 2);
  for (let offset = 0; offset < image.data.length; offset += 4) {
    image.data.set([20, 24, 29, 255], offset);
  }
  image.data.set(
    [(rgba >>> 24) & 0xff, (rgba >>> 16) & 0xff, (rgba >>> 8) & 0xff, rgba & 0xff],
    12,
  );
  context.putImageData(image, 0, 0);
  pixelRgba = [...context.getImageData(1, 1, 1, 1).data].join(",");
  document.body.dataset.pixelRgba = pixelRgba;
}

async function ensureAudioContext(): Promise<AudioContext> {
  audioContext ??= new AudioContext({ latencyHint: "interactive" });
  if (audioContext.state === "suspended") {
    await audioContext.resume();
  }
  return audioContext;
}

async function playPulse(frequency: number): Promise<void> {
  const context = await ensureAudioContext();
  const oscillator = new OscillatorNode(context, { frequency, type: "sine" });
  const gain = new GainNode(context, { gain: 0.0001 });
  oscillator.connect(gain).connect(context.destination);
  const start = context.currentTime;
  gain.gain.exponentialRampToValueAtTime(0.045, start + 0.008);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.07);
  oscillator.start(start);
  oscillator.stop(start + 0.075);
  await new Promise<void>((resolve) => {
    oscillator.addEventListener("ended", () => resolve(), { once: true });
  });
  oscillator.disconnect();
  gain.disconnect();
  toneHz = frequency;
  audioMetric.textContent = `${frequency} Hz`;
  document.body.dataset.toneHz = String(frequency);
}

async function persistByte(byte: number): Promise<void> {
  const database = await openDatabase();
  try {
    await transactionComplete(database, "readwrite", (store) => store.put(byte, "output-byte"));
    const readback = await requestResult(
      database.transaction("runtime", "readonly").objectStore("runtime").get("output-byte"),
    );
    invariant(readback === byte, "IndexedDB byte readback mismatch");
    persistentByte = byte;
    storageMetric.textContent = `0x${byte.toString(16).padStart(2, "0")}`;
    document.body.dataset.persistentByte = String(byte);
  } finally {
    database.close();
  }
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open("vine-browser-runtime-v1", 1);
    request.addEventListener("upgradeneeded", () => {
      if (!request.result.objectStoreNames.contains("runtime")) {
        request.result.createObjectStore("runtime");
      }
    });
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
  });
}

function transactionComplete(
  database: IDBDatabase,
  mode: IDBTransactionMode,
  operation: (store: IDBObjectStore) => IDBRequest,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const transaction = database.transaction("runtime", mode);
    operation(transaction.objectStore("runtime"));
    transaction.addEventListener("complete", () => resolve(), { once: true });
    transaction.addEventListener("error", () => reject(transaction.error), { once: true });
    transaction.addEventListener("abort", () => reject(transaction.error), { once: true });
  });
}

function requestResult(request: IDBRequest): Promise<unknown> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
  });
}

function finishSession(report: SessionReport): void {
  stopHeartbeat();
  const evidence: RuntimeEvidence = {
    report,
    heartbeatSamples,
    maximumHeartbeatGapMilliseconds: maximumHeartbeatGap,
    pixelRgba,
    toneHz,
    persistentByte,
  };
  instructionMetric.textContent = report.instructions.toLocaleString();
  yieldMetric.textContent = String(report.budgetYields);
  callbackMetric.textContent = `${report.callbackEntries} / ${report.maximumCallbackDepth}`;
  generationMetric.textContent = String(report.finalGeneration);
  heartbeatMetric.textContent = `${heartbeatSamples} / ${maximumHeartbeatGap.toFixed(1)} ms`;
  document.body.dataset.outcome = report.outcome;
  document.body.dataset.report = JSON.stringify(evidence);
  const validationError = validateEvidence(activeMode, evidence);
  if (validationError === null) {
    document.body.dataset.checkStatus = "passed";
    setStatus(report.outcome, outcomeLabel(report.outcome));
    addTimeline("complete", outcomeLabel(report.outcome));
  } else {
    document.body.dataset.checkStatus = "failed";
    document.body.dataset.error = validationError.message;
    setStatus("failed", validationError.message);
    addTimeline("failed", validationError.message);
  }
  activeSessionId = null;
  setControls(false);
  const resolve = resolveSession;
  const reject = rejectSession;
  resolveSession = null;
  rejectSession = null;
  if (validationError === null) {
    resolve?.(evidence);
  } else {
    reject?.(validationError);
  }
}

function validateEvidence(mode: SessionMode, evidence: RuntimeEvidence): Error | null {
  const { report } = evidence;
  if (report.pendingHostCalls !== 0 || report.pendingCallbacks !== 0) {
    return new Error("runtime cleanup retained suspended work");
  }
  if (mode === "cancel-pending") {
    return report.outcome === "cancelled" ? null : new Error("pending request was not cancelled");
  }
  if (mode === "watchdog") {
    return report.outcome === "watchdog" ? null : new Error("watchdog did not terminate the guest");
  }
  if (hostServiceError !== null) {
    return hostServiceError;
  }
  if (report.outcome !== "completed" || report.exitCode !== 0) {
    return new Error(report.error ?? `guest exited with ${String(report.exitCode)}`);
  }
  if (report.hostRequests !== 2) {
    return new Error("guest did not complete both deferred browser services");
  }
  if (report.callbackEntries !== 2 || report.maximumCallbackDepth !== 2) {
    return new Error("guest did not complete the nested callback sequence");
  }
  if (evidence.pixelRgba !== "255,0,0,255") {
    return new Error("Canvas pixel readback mismatch");
  }
  if (evidence.toneHz !== 880 || evidence.persistentByte !== 0x5a) {
    return new Error("audio or persistent output mismatch");
  }
  if (evidence.heartbeatSamples < 2) {
    return new Error("main-thread heartbeat did not advance during the session");
  }
  return null;
}

function startHeartbeat(): void {
  stopHeartbeat();
  heartbeatSamples = 0;
  maximumHeartbeatGap = 0;
  lastHeartbeat = performance.now();
  heartbeatTimer = window.setInterval(() => {
    const now = performance.now();
    maximumHeartbeatGap = Math.max(maximumHeartbeatGap, now - lastHeartbeat);
    lastHeartbeat = now;
    heartbeatSamples += 1;
  }, 5);
}

function stopHeartbeat(): void {
  if (heartbeatTimer !== null) {
    clearInterval(heartbeatTimer);
    heartbeatTimer = null;
  }
}

async function runStress(durationMilliseconds: number): Promise<StressEvidence> {
  invariant(
    Number.isFinite(durationMilliseconds) && durationMilliseconds >= 1_000,
    "stress duration must be at least one second",
  );
  const startedAt = performance.now();
  let sessions = 0;
  let failedSessions = 0;
  let maximumGap = 0;
  const samples: StressSample[] = [];
  let nextSampleAt = 0;
  document.body.dataset.stressStatus = "running";
  await delay(20);
  while (performance.now() - startedAt < durationMilliseconds) {
    try {
      const evidence = await startSession("success");
      sessions += 1;
      maximumGap = Math.max(maximumGap, evidence.maximumHeartbeatGapMilliseconds);
    } catch {
      failedSessions += 1;
    }
    const elapsed = performance.now() - startedAt;
    if (elapsed >= nextSampleAt) {
      samples.push({
        elapsedMilliseconds: elapsed,
        usedJsHeapBytes: performance.memory?.usedJSHeapSize ?? null,
        sessions,
      });
      nextSampleAt += 60_000;
    }
    await delay(20);
  }
  const finalElapsed = performance.now() - startedAt;
  if (samples.at(-1)?.sessions !== sessions) {
    samples.push({
      elapsedMilliseconds: finalElapsed,
      usedJsHeapBytes: performance.memory?.usedJSHeapSize ?? null,
      sessions,
    });
  }
  const evidence: StressEvidence = {
    durationMilliseconds: finalElapsed,
    sessions,
    samples,
    heapSlopeBytesPerMinute: linearHeapSlope(samples),
    maximumHeartbeatGapMilliseconds: maximumGap,
    failedSessions,
  };
  document.body.dataset.stressStatus = failedSessions === 0 ? "passed" : "failed";
  document.body.dataset.stressReport = JSON.stringify(evidence);
  return evidence;
}

function linearHeapSlope(samples: StressSample[]): number | null {
  const points = samples.filter(
    (sample): sample is StressSample & { usedJsHeapBytes: number } =>
      sample.usedJsHeapBytes !== null,
  );
  if (points.length < 2) {
    return null;
  }
  const meanX = points.reduce((sum, point) => sum + point.elapsedMilliseconds, 0) / points.length;
  const meanY = points.reduce((sum, point) => sum + point.usedJsHeapBytes, 0) / points.length;
  const numerator = points.reduce(
    (sum, point) => sum + (point.elapsedMilliseconds - meanX) * (point.usedJsHeapBytes - meanY),
    0,
  );
  const denominator = points.reduce(
    (sum, point) => sum + (point.elapsedMilliseconds - meanX) ** 2,
    0,
  );
  return denominator === 0 ? 0 : (numerator / denominator) * 60_000;
}

function setControls(running: boolean): void {
  runButton.disabled = running;
  cancelButton.disabled = !running;
  resetButton.disabled = running;
}

function setStatus(phase: string, detail: string): void {
  statusLabel.textContent = detail;
  statusDot.dataset.phase = phase;
}

function addTimeline(phase: string, detail: string): void {
  const item = document.createElement("li");
  const time = document.createElement("time");
  const label = document.createElement("span");
  time.textContent = new Date().toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  label.textContent = detail;
  item.dataset.phase = phase;
  item.append(time, label);
  timeline.append(item);
  while (timeline.childElementCount > 12) {
    timeline.firstElementChild?.remove();
  }
  timeline.scrollTop = timeline.scrollHeight;
}

function resetView(): void {
  timeline.replaceChildren();
  resetSurface();
  instructionMetric.textContent = "0";
  yieldMetric.textContent = "0";
  callbackMetric.textContent = "0 / 0";
  generationMetric.textContent = "0";
  heartbeatMetric.textContent = "0 / 0 ms";
  audioMetric.textContent = "Idle";
  storageMetric.textContent = "--";
  setStatus("idle", "Ready");
  document.body.dataset.checkStatus = "idle";
  document.body.dataset.error = "";
}

function resetSurface(): void {
  const context = canvas.getContext("2d", { alpha: false });
  invariant(context !== null, "Canvas 2D is unavailable");
  context.fillStyle = "#14181d";
  context.fillRect(0, 0, canvas.width, canvas.height);
  document.body.dataset.pixelRgba = "";
}

function showFatalError(error: unknown): void {
  const fatal = error instanceof Error ? error : new Error(String(error));
  stopHeartbeat();
  setControls(false);
  setStatus("failed", fatal.message);
  addTimeline("failed", fatal.message);
  document.body.dataset.checkStatus = "failed";
  document.body.dataset.error = fatal.message;
  activeSessionId = null;
  rejectSession?.(fatal);
  resolveSession = null;
  rejectSession = null;
}

function outcomeLabel(outcome: SessionReport["outcome"]): string {
  switch (outcome) {
    case "completed":
      return "Process exited cleanly";
    case "cancelled":
      return "Session cancelled cleanly";
    case "watchdog":
      return "Watchdog cleanup complete";
    case "fault":
      return "Runtime fault";
  }
}

function parseMode(value: string | null): SessionMode {
  return value === "cancel-pending" || value === "watchdog" ? value : "success";
}

function requiredElement<T extends Element>(id: string): T {
  const element = document.getElementById(id);
  invariant(element !== null, `missing #${id}`);
  return element as unknown as T;
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

window.__vineRuntime = {
  start: startSession,
  cancel: cancelSession,
  runStress,
  snapshot: () => ({
    checkStatus: document.body.dataset.checkStatus ?? null,
    error: document.body.dataset.error ?? null,
    outcome: document.body.dataset.outcome ?? null,
    pixelRgba: document.body.dataset.pixelRgba ?? null,
    toneHz: document.body.dataset.toneHz ?? null,
    persistentByte: document.body.dataset.persistentByte ?? null,
    report: document.body.dataset.report ?? null,
    stressStatus: document.body.dataset.stressStatus ?? null,
    stressReport: document.body.dataset.stressReport ?? null,
  }),
};
