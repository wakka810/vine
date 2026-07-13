interface ThroughputResult {
  instructionsPerSecond: number;
}

interface SliceResult {
  targetMilliseconds: number;
  relativeThroughput: number;
}

interface BandwidthResult {
  pages: number;
  mebibytesPerSecond: number;
}

interface SuiteResult {
  integer: ThroughputResult;
  x87: ThroughputResult;
  sse: ThroughputResult;
  traps: {
    roundTripsPerSecond: number;
    wasmExportCallsPerRoundTrip: number;
  };
  productionApi: {
    roundTripsPerSecond: number;
    medianMicroseconds: number;
    p95Microseconds: number;
    wasmExportCallsPerRoundTrip: number;
  };
  callbacks: { microsecondsPerRoundTrip: number };
  context: { saveRestoreMicroseconds: number };
  slices: SliceResult[];
  reflection: BandwidthResult[];
  mirrorWriteback: BandwidthResult[];
  productionWriteback: { mebibytesPerSecond: number };
  invalidation: { microsecondsPerInvalidation: number };
  environment: Record<string, unknown>;
}

interface BenchmarkThresholds {
  integerInstructionsPerSecondMinimum: number;
  x87InstructionsPerSecondMinimum: number;
  sseInstructionsPerSecondMinimum: number;
  apiTrapRoundTripsPerSecondMinimum: number;
  productionApiRoundTripsPerSecondMinimum: number;
  callbackRoundTripMicrosecondsMaximum: number;
  contextSaveRestoreMicrosecondsMaximum: number;
  sliceRelativeThroughputMinimum: Record<string, number>;
  reflectionMiBPerSecondMinimum: number;
  mirrorWritebackMiBPerSecondMinimum: number;
  productionWritebackMiBPerSecondMinimum: number;
  codeInvalidationMicrosecondsMaximum: number;
  workerColdStartP95MillisecondsMaximum: number;
  steadyMemoryBytesMaximum: number;
  stressHeapSlopeBytesPerMinuteMaximum: number;
  backgroundThroughputRatioMinimum: number;
  audioProbeMaximumGapMilliseconds: number;
  wasmExportCallsPerApiTrapMaximum: number;
}

interface BenchmarkConfiguration {
  schema: string;
  schemaVersion: number;
  id: string;
  workloadDurationMilliseconds: number;
  coldStartSamples: number;
  thresholds: BenchmarkThresholds;
}

interface AudioProbeResult {
  sampleRate: number;
  messages: number;
  expectedMessageIntervalMilliseconds: number;
  maximumDeliveryGapMilliseconds: number | null;
  frameDiscontinuities: number;
}

interface MemoryMeasurement {
  method: "measureUserAgentSpecificMemory" | "performance.memory" | "unavailable";
  bytes: number | null;
}

interface ForegroundResult {
  configuration: BenchmarkConfiguration;
  suite: SuiteResult;
  coldStart: {
    samplesMilliseconds: number[];
    internalSamplesMilliseconds: number[];
    medianMilliseconds: number;
    p95Milliseconds: number;
  };
  audio: AudioProbeResult;
  memoryBefore: MemoryMeasurement;
  memorySteady: MemoryMeasurement;
}

interface BackgroundResult {
  visibilityBefore: DocumentVisibilityState;
  visibilityAfter: DocumentVisibilityState;
  throughput: ThroughputResult;
  foregroundRatio: number;
}

interface BenchmarkReport {
  schema: "vine.v86-browser-benchmark";
  schemaVersion: 1;
  measuredAt: string;
  foreground: ForegroundResult;
  background: BackgroundResult;
  stressEvidence: Record<string, unknown> | null;
  failures: string[];
  decision: "go" | "conditional-go" | "no-go";
}

type WorkerMessage =
  | { type: "progress"; phase: string }
  | { type: "cold-start-complete"; internalMilliseconds: number }
  | { type: "suite-complete"; result: SuiteResult }
  | { type: "integer-probe-complete"; result: ThroughputResult }
  | { type: "benchmark-error"; message: string };

declare global {
  interface Window {
    __vineBenchmark: {
      run(): Promise<ForegroundResult>;
      runBackgroundProbe(): Promise<BenchmarkReport>;
      snapshot(): Record<string, unknown>;
    };
  }

  interface Performance {
    memory?: { usedJSHeapSize: number };
    measureUserAgentSpecificMemory?: () => Promise<{ bytes: number }>;
  }
}

const runButton = requiredElement<HTMLButtonElement>("run-benchmark");
const phaseLabel = requiredElement<HTMLElement>("benchmark-phase");
const resultsBody = requiredElement<HTMLTableSectionElement>("benchmark-results");
const benchmarkWorkerUrl = new URL("./benchmark-worker.js", import.meta.url);
let suiteWorker: Worker | null = null;
let foregroundResult: ForegroundResult | null = null;
let finalReport: BenchmarkReport | null = null;

runButton.addEventListener("click", () => {
  void runForeground().catch(fail);
});

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function requiredElement<T extends Element>(id: string): T {
  const element = document.getElementById(id);
  invariant(element !== null, `missing #${id}`);
  return element as unknown as T;
}

function percentile(values: number[], percentile_: number): number {
  invariant(values.length > 0, "percentile requires samples");
  const ordered = [...values].sort((left, right) => left - right);
  const index = Math.min(ordered.length - 1, Math.ceil(percentile_ * ordered.length) - 1);
  return ordered[index] ?? 0;
}

function median(values: number[]): number {
  invariant(values.length > 0, "median requires samples");
  const ordered = [...values].sort((left, right) => left - right);
  const middle = Math.floor(ordered.length / 2);
  return ordered.length % 2 === 0
    ? ((ordered[middle - 1] ?? 0) + (ordered[middle] ?? 0)) / 2
    : (ordered[middle] ?? 0);
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  invariant(response.ok, `${url} fetch failed with ${response.status}`);
  return (await response.json()) as T;
}

async function memoryMeasurement(): Promise<MemoryMeasurement> {
  if (typeof performance.measureUserAgentSpecificMemory === "function") {
    try {
      const result = await performance.measureUserAgentSpecificMemory();
      return { method: "measureUserAgentSpecificMemory", bytes: result.bytes };
    } catch {
      // Fall through to the Chromium-specific heap counter.
    }
  }
  if (performance.memory !== undefined) {
    return { method: "performance.memory", bytes: performance.memory.usedJSHeapSize };
  }
  return { method: "unavailable", bytes: null };
}

async function coldStartSample(): Promise<{ outer: number; internal: number }> {
  const startedAt = performance.now();
  const worker = new Worker(benchmarkWorkerUrl, { type: "module" });
  try {
    const message = await new Promise<WorkerMessage>((resolve, reject) => {
      const timeout = window.setTimeout(() => reject(new Error("cold Worker timed out")), 10_000);
      worker.addEventListener(
        "message",
        (event: MessageEvent<WorkerMessage>) => {
          clearTimeout(timeout);
          resolve(event.data);
        },
        { once: true },
      );
      worker.addEventListener("error", (event) => reject(new Error(event.message)), { once: true });
      worker.postMessage({ type: "cold-start" });
    });
    invariant(message.type === "cold-start-complete", "cold Worker returned the wrong message");
    return { outer: performance.now() - startedAt, internal: message.internalMilliseconds };
  } finally {
    worker.terminate();
  }
}

async function startAudioProbe(): Promise<{
  stop(): Promise<AudioProbeResult>;
}> {
  const context = new AudioContext({ latencyHint: "interactive" });
  await context.audioWorklet.addModule("./audio-probe-worklet.js");
  await context.resume();
  const node = new AudioWorkletNode(context, "vine-audio-deadline-probe");
  const arrivals: number[] = [];
  const frames: number[] = [];
  node.port.addEventListener("message", (event: MessageEvent<{ currentFrame: number }>) => {
    arrivals.push(performance.now());
    frames.push(event.data.currentFrame);
  });
  node.port.start();
  node.connect(context.destination);
  return {
    async stop() {
      node.disconnect();
      node.port.close();
      await context.close();
      const gaps = arrivals.slice(1).map((arrival, index) => arrival - (arrivals[index] ?? arrival));
      let frameDiscontinuities = 0;
      for (let index = 1; index < frames.length; index += 1) {
        if ((frames[index] ?? 0) - (frames[index - 1] ?? 0) !== 16 * 128) {
          frameDiscontinuities += 1;
        }
      }
      return {
        sampleRate: context.sampleRate,
        messages: arrivals.length,
        expectedMessageIntervalMilliseconds: (16 * 128 * 1000) / context.sampleRate,
        maximumDeliveryGapMilliseconds: gaps.length > 0 ? Math.max(...gaps) : null,
        frameDiscontinuities,
      };
    },
  };
}

function suiteRequest(worker: Worker, request: Record<string, unknown>): Promise<WorkerMessage> {
  return new Promise((resolve, reject) => {
    const onMessage = (event: MessageEvent<WorkerMessage>) => {
      if (event.data.type === "progress") {
        phaseLabel.textContent = `Running ${event.data.phase} metrics`;
        return;
      }
      worker.removeEventListener("message", onMessage);
      if (event.data.type === "benchmark-error") {
        reject(new Error(event.data.message));
      } else {
        resolve(event.data);
      }
    };
    worker.addEventListener("message", onMessage);
    worker.addEventListener("error", (event) => reject(new Error(event.message)), { once: true });
    worker.postMessage(request);
  });
}

async function runForeground(): Promise<ForegroundResult> {
  if (foregroundResult !== null) {
    return foregroundResult;
  }
  runButton.disabled = true;
  document.body.dataset.benchmarkStatus = "running";
  phaseLabel.textContent = "Measuring Worker cold start";
  const configuration = await fetchJson<BenchmarkConfiguration>("./v86-browser-v1.json");
  const memoryBefore = await memoryMeasurement();
  const coldSamples = [];
  for (let index = 0; index < configuration.coldStartSamples; index += 1) {
    coldSamples.push(await coldStartSample());
  }
  const audio = await startAudioProbe();
  suiteWorker = new Worker(benchmarkWorkerUrl, { type: "module" });
  const message = await suiteRequest(suiteWorker, {
    type: "run-suite",
    configuration: {
      workloadDurationMilliseconds: configuration.workloadDurationMilliseconds,
    },
  });
  invariant(message.type === "suite-complete", "benchmark Worker returned the wrong result");
  const memorySteady = await memoryMeasurement();
  const audioResult = await audio.stop();
  foregroundResult = {
    configuration,
    suite: message.result,
    coldStart: {
      samplesMilliseconds: coldSamples.map(({ outer }) => outer),
      internalSamplesMilliseconds: coldSamples.map(({ internal }) => internal),
      medianMilliseconds: median(coldSamples.map(({ outer }) => outer)),
      p95Milliseconds: percentile(coldSamples.map(({ outer }) => outer), 0.95),
    },
    audio: audioResult,
    memoryBefore,
    memorySteady,
  };
  renderForeground(foregroundResult);
  phaseLabel.textContent = "Foreground complete; background probe pending";
  document.body.dataset.benchmarkStatus = "foreground-complete";
  document.body.dataset.foregroundReport = JSON.stringify(foregroundResult);
  return foregroundResult;
}

async function runBackgroundProbe(): Promise<BenchmarkReport> {
  const foreground = await runForeground();
  invariant(suiteWorker !== null, "benchmark Worker is unavailable");
  const visibilityBefore = document.visibilityState;
  const message = await suiteRequest(suiteWorker, {
    type: "integer-probe",
    durationMilliseconds: 3_000,
  });
  invariant(message.type === "integer-probe-complete", "background probe result mismatch");
  const background: BackgroundResult = {
    visibilityBefore,
    visibilityAfter: document.visibilityState,
    throughput: message.result,
    foregroundRatio:
      message.result.instructionsPerSecond / foreground.suite.integer.instructionsPerSecond,
  };
  const checkpoint = await fetchJson<{
    browserCheckpoint?: { browserStressEvidence?: Record<string, unknown> };
  }>("./current-checkpoint.json");
  finalReport = finalize(
    foreground,
    background,
    checkpoint.browserCheckpoint?.browserStressEvidence ?? null,
  );
  document.body.dataset.benchmarkStatus =
    finalReport.decision === "no-go" ? "failed" : "passed";
  document.body.dataset.report = JSON.stringify(finalReport);
  phaseLabel.textContent = `Decision: ${finalReport.decision}`;
  renderDecision(finalReport);
  return finalReport;
}

function finalize(
  foreground: ForegroundResult,
  background: BackgroundResult,
  stressEvidence: Record<string, unknown> | null,
): BenchmarkReport {
  const { thresholds } = foreground.configuration;
  const { suite } = foreground;
  const failures: string[] = [];
  const minimum = (value: number, threshold: number, id: string) => {
    if (value < threshold) failures.push(`${id}: ${value} < ${threshold}`);
  };
  const maximum = (value: number | null, threshold: number, id: string) => {
    if (value === null || value > threshold) failures.push(`${id}: ${String(value)} > ${threshold}`);
  };
  minimum(
    suite.integer.instructionsPerSecond,
    thresholds.integerInstructionsPerSecondMinimum,
    "integer-ips",
  );
  minimum(suite.x87.instructionsPerSecond, thresholds.x87InstructionsPerSecondMinimum, "x87-ips");
  minimum(suite.sse.instructionsPerSecond, thresholds.sseInstructionsPerSecondMinimum, "sse-ips");
  minimum(
    suite.traps.roundTripsPerSecond,
    thresholds.apiTrapRoundTripsPerSecondMinimum,
    "api-traps",
  );
  minimum(
    suite.productionApi.roundTripsPerSecond,
    thresholds.productionApiRoundTripsPerSecondMinimum,
    "production-api-round-trips",
  );
  maximum(
    suite.callbacks.microsecondsPerRoundTrip,
    thresholds.callbackRoundTripMicrosecondsMaximum,
    "callback-cost",
  );
  maximum(
    suite.context.saveRestoreMicroseconds,
    thresholds.contextSaveRestoreMicrosecondsMaximum,
    "context-cost",
  );
  for (const slice of suite.slices) {
    minimum(
      slice.relativeThroughput,
      thresholds.sliceRelativeThroughputMinimum[String(slice.targetMilliseconds)] ?? 1,
      `slice-${slice.targetMilliseconds}ms`,
    );
  }
  minimum(
    suite.reflection.at(-1)?.mebibytesPerSecond ?? 0,
    thresholds.reflectionMiBPerSecondMinimum,
    "reflection-bandwidth",
  );
  minimum(
    suite.mirrorWriteback.at(-1)?.mebibytesPerSecond ?? 0,
    thresholds.mirrorWritebackMiBPerSecondMinimum,
    "mirror-writeback-bandwidth",
  );
  minimum(
    suite.productionWriteback.mebibytesPerSecond,
    thresholds.productionWritebackMiBPerSecondMinimum,
    "production-writeback-bandwidth",
  );
  maximum(
    suite.invalidation.microsecondsPerInvalidation,
    thresholds.codeInvalidationMicrosecondsMaximum,
    "invalidation-cost",
  );
  maximum(
    foreground.coldStart.p95Milliseconds,
    thresholds.workerColdStartP95MillisecondsMaximum,
    "worker-cold-start",
  );
  maximum(
    foreground.memorySteady.bytes,
    thresholds.steadyMemoryBytesMaximum,
    "steady-memory",
  );
  minimum(
    background.foregroundRatio,
    thresholds.backgroundThroughputRatioMinimum,
    "background-throughput",
  );
  maximum(
    foreground.audio.maximumDeliveryGapMilliseconds,
    thresholds.audioProbeMaximumGapMilliseconds,
    "audio-gap",
  );
  if (foreground.audio.frameDiscontinuities !== 0) {
    failures.push(`audio-frame-discontinuities: ${foreground.audio.frameDiscontinuities}`);
  }
  maximum(
    suite.productionApi.wasmExportCallsPerRoundTrip,
    thresholds.wasmExportCallsPerApiTrapMaximum,
    "wasm-crossings",
  );
  const stressSlope = Number(stressEvidence?.heapSlopeBytesPerMinute ?? Number.NaN);
  maximum(
    Number.isFinite(stressSlope) ? stressSlope : null,
    thresholds.stressHeapSlopeBytesPerMinuteMaximum,
    "stress-memory-slope",
  );
  const severe = failures.some((failure) => {
    const [id] = failure.split(":", 1);
    return id === "integer-ips" || id === "x87-ips" || id === "sse-ips" || id === "audio-gap";
  });
  return {
    schema: "vine.v86-browser-benchmark",
    schemaVersion: 1,
    measuredAt: new Date().toISOString(),
    foreground,
    background,
    stressEvidence,
    failures,
    decision: failures.length === 0 ? "go" : severe ? "no-go" : "conditional-go",
  };
}

function addRow(label: string, value: string): void {
  const row = document.createElement("tr");
  const heading = document.createElement("th");
  const cell = document.createElement("td");
  heading.scope = "row";
  heading.textContent = label;
  cell.textContent = value;
  row.append(heading, cell);
  resultsBody.append(row);
}

function renderForeground(result: ForegroundResult): void {
  resultsBody.replaceChildren();
  addRow("Integer", `${Math.round(result.suite.integer.instructionsPerSecond).toLocaleString()} ips`);
  addRow("x87", `${Math.round(result.suite.x87.instructionsPerSecond).toLocaleString()} ips`);
  addRow("SSE2", `${Math.round(result.suite.sse.instructionsPerSecond).toLocaleString()} ips`);
  addRow("API traps", `${Math.round(result.suite.traps.roundTripsPerSecond).toLocaleString()} /s`);
  addRow(
    "Production API",
    `${Math.round(result.suite.productionApi.roundTripsPerSecond).toLocaleString()} /s (${result.suite.productionApi.medianMicroseconds.toFixed(2)} us median)`,
  );
  addRow("Callback", `${result.suite.callbacks.microsecondsPerRoundTrip.toFixed(2)} us`);
  addRow("Cold start p95", `${result.coldStart.p95Milliseconds.toFixed(1)} ms`);
  addRow("Steady memory", `${String(result.memorySteady.bytes)} bytes`);
  addRow("Audio max gap", `${String(result.audio.maximumDeliveryGapMilliseconds)} ms`);
}

function renderDecision(report: BenchmarkReport): void {
  addRow("Background ratio", report.background.foregroundRatio.toFixed(3));
  addRow("Decision", report.decision);
  if (report.failures.length > 0) {
    addRow("Threshold gaps", report.failures.join("; "));
  }
}

function fail(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  document.body.dataset.benchmarkStatus = "failed";
  document.body.dataset.error = message;
  phaseLabel.textContent = message;
  runButton.disabled = false;
}

window.__vineBenchmark = {
  run: runForeground,
  runBackgroundProbe,
  snapshot: () => ({
    status: document.body.dataset.benchmarkStatus ?? null,
    error: document.body.dataset.error ?? null,
    foregroundReport: document.body.dataset.foregroundReport ?? null,
    report: document.body.dataset.report ?? null,
    visibilityState: document.visibilityState,
  }),
};
