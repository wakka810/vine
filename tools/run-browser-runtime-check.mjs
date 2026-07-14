import assert from "node:assert/strict";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { bgiProfileOutputDirectory } from "./bgi-checkpoint-command.mjs";
import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import {
  classifyBenchmarkDecision,
  EVIDENCE_CLASSIFICATION,
} from "./evidence-classification.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, ".work", "browser-runtime");
const serveOnly = process.argv.includes("--serve");
const bgiBenchmarkMilliseconds = Number(optionValue("--bgi-benchmark-ms") ?? 0);
const bgiMode = process.argv.includes("--bgi") || bgiBenchmarkMilliseconds > 0;
const stressMilliseconds = Number(optionValue("--stress-ms") ?? 0);
const checkpoint = loadCurrentCheckpoint(path.join(root, "docs", "current-checkpoint.json"));
const runtimeProtocol = "vine.browser-runtime-worker.v4";
assert.equal(
  checkpoint.browserCheckpoint.browserRuntimeProtocol,
  runtimeProtocol,
  "current checkpoint does not identify the built Runtime Worker protocol",
);
const bgiPath = path.resolve(
  process.env.VINE_BGI_EXE ?? "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const bgiPolicyPath = path.join(
  root,
  checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
);
const bgiOracleReportPath = path.join(
  root,
  ".work",
  bgiProfileOutputDirectory(checkpoint.activeRuntimeCheckpoint.profile),
  "report.json",
);
const bgiBenchmarkConfigurationPath = path.join(
  root,
  "fixtures",
  "benchmarks",
  "bgi-browser-v1.json",
);

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? null : process.argv[index + 1] ?? null;
}

function wait(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function hash(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function run(command, arguments_, { env = process.env, timeoutMs = 30_000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, arguments_, {
      cwd: root,
      env,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error(`${path.basename(command)} timed out\n${stdout}${stderr}`.trim()));
    }, timeoutMs);
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code, signal) => {
      clearTimeout(timeout);
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`${path.basename(command)} failed (${signal ?? code})\n${stdout}${stderr}`));
      }
    });
  });
}

function terminateProcess(child, signal = "SIGTERM") {
  try {
    if (process.platform === "win32") {
      child.kill(signal);
    } else {
      process.kill(-child.pid, signal);
    }
  } catch (error) {
    if (error.code !== "ESRCH") {
      throw error;
    }
  }
}

function launchChrome(command, arguments_, timeoutMs = 15_000) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, arguments_, {
      cwd: root,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    let settled = false;
    let resolveExit;
    const exit = new Promise((resolve_) => {
      resolveExit = resolve_;
    });
    const fail = (error) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        terminateProcess(child);
        reject(error);
      }
    };
    const timeout = setTimeout(
      () => fail(new Error(`Chrome did not expose DevTools\n${stdout}${stderr}`)),
      timeoutMs,
    );
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
      const match = /DevTools listening on (ws:\/\/\S+)/u.exec(stderr);
      if (match !== null && !settled) {
        settled = true;
        clearTimeout(timeout);
        resolve({ browserWebSocketUrl: match[1], child, exit, logs: () => `${stdout}${stderr}` });
      }
    });
    child.once("error", fail);
    child.once("close", (code, signal) => {
      resolveExit({ code, signal });
      fail(new Error(`Chrome exited before DevTools (${signal ?? code})\n${stdout}${stderr}`));
    });
  });
}

async function connectDevTools(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("DevTools connection timed out")), 10_000);
    socket.addEventListener(
      "open",
      () => {
        clearTimeout(timeout);
        resolve();
      },
      { once: true },
    );
    socket.addEventListener(
      "error",
      () => {
        clearTimeout(timeout);
        reject(new Error("DevTools connection failed"));
      },
      { once: true },
    );
  });
  let nextId = 1;
  const pending = new Map();
  const eventListeners = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!Number.isInteger(message.id)) {
      for (const listener of eventListeners.get(message.method) ?? []) {
        listener(message.params, message.sessionId ?? null);
      }
      return;
    }
    const request = pending.get(message.id);
    if (request === undefined) {
      return;
    }
    pending.delete(message.id);
    clearTimeout(request.timeout);
    if (message.error === undefined) {
      request.resolve(message.result);
    } else {
      request.reject(new Error(`DevTools ${request.method}: ${message.error.message}`));
    }
  });
  socket.addEventListener("close", () => {
    for (const request of pending.values()) {
      clearTimeout(request.timeout);
      request.reject(new Error(`DevTools closed during ${request.method}`));
    }
    pending.clear();
  });
  return {
    close() {
      if (socket.readyState === WebSocket.OPEN) {
        socket.close();
      }
    },
    send(method, params = {}, timeoutMs = 10_000) {
      return sendMessage(method, params, timeoutMs, null);
    },
    sendToSession(sessionId, method, params = {}, timeoutMs = 10_000) {
      return sendMessage(method, params, timeoutMs, sessionId);
    },
    on(method, listener) {
      const listeners = eventListeners.get(method) ?? new Set();
      listeners.add(listener);
      eventListeners.set(method, listeners);
      return () => listeners.delete(listener);
    },
  };

  function sendMessage(method, params, timeoutMs, sessionId) {
    const id = nextId;
    nextId += 1;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`DevTools ${method} timed out`));
      }, timeoutMs);
      pending.set(id, { method, reject, resolve, timeout });
      socket.send(
        JSON.stringify({ id, method, params, ...(sessionId === null ? {} : { sessionId }) }),
      );
    });
  }
}

async function createDebugTarget(browserWebSocketUrl, url) {
  const browserEndpoint = new URL(browserWebSocketUrl);
  const response = await fetch(`http://${browserEndpoint.host}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  assert(response.ok, `Chrome target creation failed with ${response.status}`);
  const target = await response.json();
  assert.equal(typeof target.webSocketDebuggerUrl, "string");
  return target;
}

async function evaluate(client, expression, { awaitPromise = false, timeoutMs = 10_000 } = {}) {
  const result = await client.send(
    "Runtime.evaluate",
    { expression, awaitPromise, returnByValue: true },
    timeoutMs,
  );
  if (result.exceptionDetails !== undefined) {
    throw new Error(`browser evaluation failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return result.result.value;
}

async function waitForRuntime(client) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      if (await evaluate(client, `Boolean(window.__vineRuntime && document.querySelector("#run-button"))`)) {
        return;
      }
    } catch {
      // Navigation may still be replacing the execution context.
    }
    await wait(100);
  }
  throw new Error("browser runtime did not become ready");
}

async function clickRun(client) {
  const point = await evaluate(
    client,
    `(() => { const r = document.querySelector("#run-button").getBoundingClientRect(); ` +
      `return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`,
  );
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button: "left",
    clickCount: 1,
  });
  await client.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: point.x,
    y: point.y,
    button: "left",
    clickCount: 1,
  });
}

async function waitForCheckpoint(client, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  let snapshot = null;
  while (Date.now() < deadline) {
    snapshot = await evaluate(client, `window.__vineRuntime.snapshot()`);
    if (snapshot.checkStatus === "passed" || snapshot.checkStatus === "failed") {
      return snapshot;
    }
    await wait(100);
  }
  throw new Error(`browser runtime timed out: ${JSON.stringify(snapshot)}`);
}

async function captureScreenshot(client, name) {
  const result = await client.send("Page.captureScreenshot", {
    format: "png",
    fromSurface: true,
    captureBeyondViewport: false,
  });
  const bytes = Buffer.from(result.data, "base64");
  const file = path.join(assetsRoot, name);
  await writeFile(file, bytes);
  return { file: path.relative(root, file), bytes: bytes.byteLength, sha256: hash(bytes) };
}

function assertExecutionStateMachine(report, scenario) {
  assert.equal(report.executionState, "stopped");
  assert(report.executionTransitionCount > 0, "execution state machine did not transition");
  assert.equal(report.lastExecutionTransition?.to, "stopped");
  assert.equal(report.pendingHostRequestRecords, 0);
  if (scenario === "watchdog") {
    assert(report.watchdogContext !== null, "watchdog context is missing");
    assert(
      Number.isSafeInteger(report.watchdogContext.threadId) &&
        report.watchdogContext.threadId > 0,
      "watchdog thread identity is invalid",
    );
    assert(
      report.watchdogContext.requestId === null ||
        /^(0|[1-9][0-9]*)$/u.test(report.watchdogContext.requestId),
      "watchdog request identity is invalid",
    );
    assert(report.watchdogContext.lastTransition !== null, "watchdog transition is missing");
  } else {
    assert.equal(report.watchdogContext, null, "non-watchdog report retained watchdog context");
  }
}

function assertExecutionEngine(report) {
  assert.equal(report.executionEngine?.schema, "vine.execution-engine-profile");
  assert.equal(report.executionEngine?.schemaVersion, 1);
  assert.equal(report.executionEngine?.engine, "v86");
  assert.equal(report.executionEngineLinearMemoryBytes > 0, true);
}

async function runProbe(browserWebSocketUrl, baseUrl, mode, screenshots = false) {
  const target = await createDebugTarget(browserWebSocketUrl, `${baseUrl}?scenario=${mode}`);
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    await waitForRuntime(client);
    await clickRun(client);
    const snapshot = await waitForCheckpoint(client);
    assert.equal(snapshot.checkStatus, "passed", snapshot.error ?? `${mode} failed`);
    const evidence = JSON.parse(snapshot.report);
    assert.equal(evidence.report.pendingHostCalls, 0);
    assert.equal(evidence.report.pendingCallbacks, 0);
    assertExecutionStateMachine(evidence.report, mode);
    assertExecutionEngine(evidence.report);
    if (mode === "watchdog") {
      assert.notEqual(evidence.report.watchdogContext.requestId, null);
      assert.equal(evidence.report.watchdogContext.lastTransition?.to, "waiting-host");
    }
    assert.equal(evidence.report.mappingDeltas, 7);
    assert.deepEqual(evidence.report.mappingOperations, {
      reserve: 1,
      commit: 2,
      protect: 2,
      decommit: 1,
      release: 1,
    });
    assert.equal(evidence.report.mappingDirtyPages, 3);
    assert.equal(evidence.report.mappingAtomicRejections, 1);
    assert.equal(evidence.report.mappingStaleAccessStops, 2);
    assert.deepEqual(evidence.report.mappingCacheEffects, {
      tlbFlushes: 6,
      instructionFetchInvalidations: 6,
      generatedCodeInvalidations: 6,
    });
    assert.equal(evidence.report.mirrorRebuilds, 1);
    if (mode === "success") {
      assert.equal(evidence.report.outcome, "completed");
      assert.equal(evidence.report.exitCode, 0);
      assert.equal(evidence.report.hostRequests, 2);
      assert.equal(evidence.report.rustSchedulerTransitions, 2);
      assert.equal(evidence.report.callbackEntries, 2);
      assert.equal(evidence.report.maximumCallbackDepth, 2);
      assert(evidence.report.budgetYields > 0, "success path did not yield periodically");
      assert(evidence.heartbeatSamples >= 2, "main heartbeat did not advance");
      assert.equal(snapshot.pixelRgba, "255,0,0,255");
      assert.equal(snapshot.toneHz, "880");
      assert.equal(snapshot.persistentByte, "90");
      const pixel = await evaluate(
        client,
        `[...document.querySelector("#surface").getContext("2d").getImageData(1,1,1,1).data]`,
      );
      assert.deepEqual(pixel, [255, 0, 0, 255]);
    } else {
      assert.equal(evidence.report.outcome, mode === "watchdog" ? "watchdog" : "cancelled");
    }
    const captures = [];
    if (screenshots) {
      captures.push(await captureScreenshot(client, "browser-runtime-desktop.png"));
      await client.send("Emulation.setDeviceMetricsOverride", {
        width: 390,
        height: 844,
        deviceScaleFactor: 1,
        mobile: true,
      });
      await wait(200);
      captures.push(await captureScreenshot(client, "browser-runtime-mobile.png"));
    }
    return { mode, evidence, screenshots: captures };
  } finally {
    client.close();
  }
}

async function waitForBgiCheckpoint(client, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  let snapshot = null;
  while (Date.now() < deadline) {
    snapshot = await evaluate(client, `window.__vineBgiCheck`);
    if (snapshot?.error !== null && snapshot?.error !== undefined) {
      throw new Error(`BGI browser runtime failed: ${snapshot.error}`);
    }
    if (snapshot?.result !== null && snapshot?.result !== undefined) {
      return snapshot.result;
    }
    await wait(50);
  }
  throw new Error(`BGI browser runtime timed out: ${JSON.stringify(snapshot)}`);
}

async function runBgiProbe(browserWebSocketUrl, baseUrl, scenario) {
  const target = await createDebugTarget(
    browserWebSocketUrl,
    `${baseUrl}bgi-check.html?scenario=${scenario}`,
  );
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    const evidence = await waitForBgiCheckpoint(client);
    assert.equal(evidence.scenario, scenario);
    assert.equal(evidence.report.pendingHostCalls, 0);
    assert.equal(evidence.report.pendingCallbacks, 0);
    assertExecutionStateMachine(evidence.report, scenario);
    assertExecutionEngine(evidence.report);
    assert(evidence.statuses.some(({ phase }) => phase === "running"));
    if (scenario === "success") {
      assert.equal(evidence.report.outcome, "structured-stop");
      assert.equal(
        evidence.report.supportedCalls.length,
        checkpoint.activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount,
      );
      assert.equal(
        evidence.report.firstUnsupported?.capabilityCluster,
        checkpoint.nextStructuredStop.capabilityCluster,
      );
      assert(evidence.report.maximumSliceMilliseconds > 0);
      assert(evidence.report.wasmBridgeCalls > 0);
      assert(evidence.heartbeatSamples > 0, "main heartbeat did not advance during BGI");
    } else {
      assert.equal(
        evidence.report.outcome,
        scenario === "watchdog" ? "watchdog" : "cancelled",
      );
    }
    return evidence;
  } finally {
    client.close();
  }
}

async function runBgiBenchmark(
  browserWebSocketUrl,
  baseUrl,
  durationMilliseconds,
  sampleIntervalMilliseconds,
) {
  const benchmarkUrl =
    `${baseUrl}bgi-benchmark.html?durationMs=${durationMilliseconds}` +
    `&sampleIntervalMs=${sampleIntervalMilliseconds}`;
  const target = await createDebugTarget(browserWebSocketUrl, "about:blank");
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  let workerSessionId = null;
  const workerHeapSamples = [];
  try {
    client.on("Target.attachedToTarget", ({ sessionId, targetInfo }) => {
      if (targetInfo.type === "worker" && targetInfo.url.endsWith("/runtime-worker.js")) {
        workerSessionId = sessionId;
      }
    });
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    await client.send("Target.setAutoAttach", {
      autoAttach: true,
      flatten: true,
      waitForDebuggerOnStart: false,
    });
    await client.send("Page.navigate", { url: benchmarkUrl });
    const workerDeadline = Date.now() + 10_000;
    while (workerSessionId === null && Date.now() < workerDeadline) {
      await wait(100);
    }
    assert(workerSessionId !== null, "BGI benchmark Worker target was not discoverable");
    const deadline = Date.now() + durationMilliseconds + 60_000;
    let nextHeapSampleAt = 0;
    let snapshot = null;
    while (Date.now() < deadline) {
      snapshot = await evaluate(client, `window.__vineBgiBenchmark?.snapshot()`);
      if (snapshot?.status === "failed") {
        throw new Error(`BGI benchmark failed: ${snapshot.error}`);
      }
      if (snapshot?.status === "passed") {
        assert(snapshot.result.completedSessions > 0, "BGI benchmark completed no sessions");
        assert.equal(snapshot.result.failedSessions, 0, "BGI benchmark contained failures");
        return { ...snapshot.result, workerHeapSamples };
      }
      if (snapshot?.elapsedMilliseconds >= nextHeapSampleAt) {
        const usage = await client.sendToSession(
          workerSessionId,
          "Runtime.getHeapUsage",
        );
        workerHeapSamples.push({
          elapsedMilliseconds: snapshot.elapsedMilliseconds,
          completedSessions: snapshot.completedSessions,
          usedSize: usage.usedSize,
          totalSize: usage.totalSize,
          embedderHeapUsedSize: usage.embedderHeapUsedSize ?? null,
          backingStorageSize: usage.backingStorageSize ?? null,
        });
        nextHeapSampleAt += sampleIntervalMilliseconds;
      }
      await wait(500);
    }
    throw new Error(`BGI benchmark timed out: ${JSON.stringify(snapshot)}`);
  } finally {
    if (workerSessionId !== null) {
      try {
        await client.send("Target.detachFromTarget", { sessionId: workerSessionId });
      } catch {
        // The benchmark page terminates its Worker after publishing the result.
      }
    }
    client.close();
  }
}

function percentile(values, percentileValue) {
  assert(values.length > 0, "percentile requires samples");
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(sorted.length - 1, Math.ceil(percentileValue * sorted.length) - 1);
  return sorted[index];
}

function linearSlope(points) {
  if (points.length < 2) {
    return null;
  }
  const meanX = points.reduce((sum, point) => sum + point.x, 0) / points.length;
  const meanY = points.reduce((sum, point) => sum + point.y, 0) / points.length;
  const numerator = points.reduce(
    (sum, point) => sum + (point.x - meanX) * (point.y - meanY),
    0,
  );
  const denominator = points.reduce((sum, point) => sum + (point.x - meanX) ** 2, 0);
  return denominator === 0 ? 0 : (numerator / denominator) * 60_000;
}

function heapMetrics(samples, field) {
  const points = samples
    .filter((sample) => sample[field] !== null)
    .map((sample) => ({ x: sample.elapsedMilliseconds, y: sample[field] }));
  const values = points.map(({ y }) => y);
  return {
    samples: points.length,
    minimumBytes: values.length === 0 ? null : Math.min(...values),
    maximumBytes: values.length === 0 ? null : Math.max(...values),
    finalBytes: values.length === 0 ? null : values.at(-1),
    slopeBytesPerMinute: linearSlope(points),
  };
}

function summarizeBgiBenchmark(result, configuration) {
  const sessionDurations = result.sessions.map(({ elapsedMilliseconds }) => elapsedMilliseconds);
  const sliceDurations = result.sessions.map(
    ({ maximumSliceMilliseconds }) => maximumSliceMilliseconds,
  );
  assert.equal(
    result.heartbeatGapMilliseconds.length,
    result.heartbeatSamples,
    "heartbeat sample count differs from its distribution",
  );
  const mainHeap = heapMetrics(result.memorySamples, "usedJsHeapBytes");
  const workerHeap = heapMetrics(result.workerHeapSamples, "usedSize");
  const workerHeapCapacity = heapMetrics(result.workerHeapSamples, "totalSize");
  const workerEmbedderHeap = heapMetrics(result.workerHeapSamples, "embedderHeapUsedSize");
  const workerBackingStorage = heapMetrics(result.workerHeapSamples, "backingStorageSize");
  const runtimeCoreSizes = new Set(
    result.sessions.map(({ runtimeCoreLinearMemoryBytes }) => runtimeCoreLinearMemoryBytes),
  );
  const engineSizes = new Set(
    result.sessions.map(
      ({ executionEngineLinearMemoryBytes }) => executionEngineLinearMemoryBytes,
    ),
  );
  const wasmBridgeCallCounts = new Set(
    result.sessions.map(({ wasmBridgeCalls }) => wasmBridgeCalls),
  );
  assert.equal(runtimeCoreSizes.size, 1, "Rust runtime linear memory size was unstable");
  assert.equal(engineSizes.size, 1, "execution engine linear memory size was unstable");
  assert.equal(wasmBridgeCallCounts.size, 1, "Wasm crossing count was unstable");
  const metrics = {
    durationMilliseconds: result.durationMilliseconds,
    completedSessions: result.completedSessions,
    failedSessions: result.failedSessions,
    sessionsPerMinute: (result.completedSessions * 60_000) / result.durationMilliseconds,
    sessionLatencyMilliseconds: {
      p50: percentile(sessionDurations, 0.5),
      p95: percentile(sessionDurations, 0.95),
      p99: percentile(sessionDurations, 0.99),
      maximum: Math.max(...sessionDurations),
    },
    maximumSliceMilliseconds: {
      p50: percentile(sliceDurations, 0.5),
      p95: percentile(sliceDurations, 0.95),
      p99: percentile(sliceDurations, 0.99),
      maximum: Math.max(...sliceDurations),
    },
    heartbeatGapMilliseconds: {
      p50: percentile(result.heartbeatGapMilliseconds, 0.5),
      p95: percentile(result.heartbeatGapMilliseconds, 0.95),
      p99: percentile(result.heartbeatGapMilliseconds, 0.99),
      maximum: result.maximumHeartbeatGapMilliseconds,
    },
    maximumHeartbeatGapMilliseconds: result.maximumHeartbeatGapMilliseconds,
    heartbeatSamples: result.heartbeatSamples,
    mainHeap,
    workerHeap,
    workerHeapCapacity,
    workerEmbedderHeap,
    workerBackingStorage,
    runtimeCoreLinearMemoryBytes: [...runtimeCoreSizes][0],
    executionEngineLinearMemoryBytes: [...engineSizes][0],
    wasmBridgeCallsPerSession: [...wasmBridgeCallCounts][0],
  };
  const thresholdFailures = [];
  const thresholdEvaluations = [];
  const thresholds = configuration.thresholds;
  evaluateThreshold(
    "minimum-completed-sessions",
    metrics.completedSessions,
    ">=",
    thresholds.minimumCompletedSessions,
  );
  evaluateThreshold(
    "maximum-failed-sessions",
    metrics.failedSessions,
    "<=",
    thresholds.maximumFailedSessions,
  );
  evaluateThreshold(
    "minimum-main-heap-samples",
    metrics.mainHeap.samples,
    ">=",
    thresholds.minimumMainHeapSamples,
  );
  evaluateThreshold(
    "minimum-worker-heap-samples",
    metrics.workerHeap.samples,
    ">=",
    thresholds.minimumWorkerHeapSamples,
  );
  evaluateThreshold(
    "maximum-p95-session-milliseconds",
    metrics.sessionLatencyMilliseconds.p95,
    "<=",
    thresholds.maximumP95SessionMilliseconds,
  );
  evaluateThreshold(
    "maximum-p99-slice-milliseconds",
    metrics.maximumSliceMilliseconds.p99,
    "<=",
    thresholds.maximumP99SliceMilliseconds,
  );
  evaluateThreshold(
    "maximum-p99-heartbeat-gap-milliseconds",
    metrics.heartbeatGapMilliseconds.p99,
    "<=",
    thresholds.maximumP99HeartbeatGapMilliseconds,
  );
  evaluateThreshold(
    "maximum-heartbeat-gap-milliseconds",
    metrics.maximumHeartbeatGapMilliseconds,
    "<=",
    thresholds.maximumHeartbeatGapMilliseconds,
  );
  evaluateThreshold(
    "maximum-main-heap",
    metrics.mainHeap.maximumBytes,
    "<=",
    thresholds.maximumMainHeapBytes,
  );
  evaluateThreshold(
    "maximum-main-heap-slope",
    metrics.mainHeap.slopeBytesPerMinute,
    "<=",
    thresholds.maximumMainHeapSlopeBytesPerMinute,
  );
  evaluateThreshold(
    "maximum-worker-heap",
    metrics.workerHeap.maximumBytes,
    "<=",
    thresholds.maximumWorkerHeapBytes,
  );
  evaluateThreshold(
    "maximum-worker-heap-slope",
    metrics.workerHeap.slopeBytesPerMinute,
    "<=",
    thresholds.maximumWorkerHeapSlopeBytesPerMinute,
  );
  evaluateThreshold(
    "maximum-worker-heap-capacity",
    metrics.workerHeapCapacity.maximumBytes,
    "<=",
    thresholds.maximumWorkerHeapCapacityBytes,
  );
  evaluateThreshold(
    "maximum-worker-backing-storage",
    metrics.workerBackingStorage.maximumBytes,
    "<=",
    thresholds.maximumWorkerBackingStorageBytes,
  );
  evaluateThreshold(
    "maximum-worker-backing-storage-slope",
    metrics.workerBackingStorage.slopeBytesPerMinute,
    "<=",
    thresholds.maximumWorkerBackingStorageSlopeBytesPerMinute,
  );
  evaluateThreshold(
    "maximum-runtime-core-linear-memory",
    metrics.runtimeCoreLinearMemoryBytes,
    "<=",
    thresholds.maximumRuntimeCoreLinearMemoryBytes,
  );
  evaluateThreshold(
    "maximum-execution-engine-linear-memory",
    metrics.executionEngineLinearMemoryBytes,
    "<=",
    thresholds.maximumExecutionEngineLinearMemoryBytes,
  );
  const decision = classifyBenchmarkDecision(thresholdEvaluations);
  assert.deepEqual(decision.failureIds, thresholdFailures);
  return { metrics, thresholdEvaluations, thresholdFailures, decision };

  function evaluateThreshold(name, observed, comparison, threshold) {
    const passed =
      observed !== null &&
      (comparison === ">=" ? observed >= threshold : observed <= threshold);
    thresholdEvaluations.push({ name, observed, comparison, threshold, passed });
    if (!passed) {
      thresholdFailures.push(name);
    }
  }
}

function normalizeBgiCall(call) {
  return {
    sequence: call.sequence,
    module: call.module.toLowerCase(),
    symbol: call.symbol,
    arguments: call.arguments,
    result: call.result,
    implementationId: call.implementationId,
    returnAddress: call.returnAddress,
    stackPointerBefore: call.stackPointerBefore,
    stackPointerAfter: call.stackPointerAfter,
    lastError: call.lastError,
    guestWrites: call.guestWrites.map(({ address, bytes }) => ({ address, bytes })),
    currentDirectory: call.currentDirectory ?? null,
  };
}

function normalizeCurrentDirectorySnapshot(snapshot) {
  assert(snapshot !== null && snapshot !== undefined, "current-directory snapshot is missing");
  assert.equal(typeof snapshot.directory.sourceAuthority, "string");
  assert(snapshot.directory.sourceAuthority.length > 0);
  return {
    bytesWithNul: snapshot.directory.bytesWithNul,
    encoding: snapshot.directory.encoding,
    generation: snapshot.generation,
  };
}

function normalizeStackGrowth(growth) {
  return {
    instructionCount: growth.instructionCount,
    faultAddress: growth.faultAddress,
    pageFaultErrorCode: growth.pageFaultErrorCode,
    addressAllocationId: growth.addressAllocationId,
    memoryOwner: growth.memoryOwner,
    committedPage: growth.committedPage,
    previousGuardPage: growth.previousGuardPage,
    nextGuardPage: growth.nextGuardPage,
    committedPages: growth.committedPages,
  };
}

function validateBgiEquivalence(browser, oracle) {
  assert.deepEqual(
    normalizeCurrentDirectorySnapshot(browser.report.initialCurrentDirectory),
    normalizeCurrentDirectorySnapshot({
      directory: oracle.process.currentDirectoryA,
      generation: oracle.process.currentDirectoryGeneration,
    }),
    "Chrome Worker initial current directory differs from the Node oracle",
  );
  assert.deepEqual(
    browser.report.supportedCalls.map(normalizeBgiCall),
    oracle.supportedContinuation.map(normalizeBgiCall),
    "Chrome Worker call semantics differ from the Node oracle",
  );
  assert.deepEqual(
    browser.report.stackGrowths.map(normalizeStackGrowth),
    oracle.synchronization.stackGrowths.map(normalizeStackGrowth),
    "Chrome Worker stack-growth semantics differ from the Node oracle",
  );
  for (const growths of [
    browser.report.stackGrowths,
    oracle.synchronization.stackGrowths,
  ]) {
    for (const growth of growths) {
      assert.equal(growth.generation, growth.fromGeneration + 1);
    }
  }
  const browserStop = browser.report.firstUnsupported;
  const oracleStop = oracle.firstUnsupported;
  assert(browserStop !== null, "Chrome Worker omitted its structured stop");
  assert.equal(browserStop.instructionCount, oracleStop.instructionCount);
  assert.equal(browser.report.instructions, oracleStop.instructionCount);
  assert.equal(browser.report.initialGeneration, oracle.process.memory.generation);
  assert(browser.report.finalGeneration >= browser.report.initialGeneration);
  assert.equal(browserStop.memoryContentSha256, oracleStop.memoryContentSha256);
  assert.equal(browserStop.capabilityCluster, oracleStop.capabilityCluster);
  if (oracleStop.endpoint === null) {
    assert.equal(browserStop.endpoint, null);
    assert.equal(browserStop.fault.instructionPointer, oracleStop.fault.instructionPointer);
    assert.equal(
      browserStop.fault.previousInstructionPointer,
      oracleStop.fault.previousInstructionPointer,
    );
    assert.equal(browserStop.fault.faultAddress, oracleStop.fault.faultAddress);
    assert.equal(
      browserStop.fault.pageFaultErrorCode,
      oracleStop.fault.pageFaultErrorCode,
    );
  } else {
    assert.equal(browserStop.endpoint.id, oracleStop.endpoint.endpoint.id);
    assert.equal(browserStop.endpoint.address, oracleStop.endpoint.endpoint.address);
    assert.equal(browserStop.endpoint.module, oracleStop.endpoint.endpoint.module);
    const oracleSymbol = oracleStop.endpoint.endpoint.symbol ?? {
      kind: "name",
      name: oracleStop.endpoint.endpoint.name,
    };
    assert.deepEqual(browserStop.endpoint.symbol, oracleSymbol);
  }
  assert.equal(browserStop.fault.class, oracleStop.fault.class);
  assert.equal(browserStop.fault.stage, oracleStop.fault.stage);
  assert.equal(browserStop.fault.code, oracleStop.fault.code);
}

async function runStress(client, durationMilliseconds) {
  const result = await evaluate(
    client,
    `window.__vineRuntime.runStress(${durationMilliseconds})`,
    { awaitPromise: true, timeoutMs: durationMilliseconds + 120_000 },
  );
  assert(result.sessions > 0, "stress run completed no sessions");
  assert.equal(result.failedSessions, 0, "stress run contained failed sessions");
  assert(result.samples.length >= 2, "stress run did not record enough memory samples");
  return result;
}

function mimeType(file) {
  switch (path.extname(file)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    case ".json":
      return "application/json; charset=utf-8";
    case ".png":
      return "image/png";
    default:
      return "application/octet-stream";
  }
}

function createServer() {
  return http.createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
      let file;
      if (bgiMode && pathname === "/private/BGI.exe") {
        file = bgiPath;
      } else if (bgiMode && pathname === "/private/kernel32-policy.json") {
        file = bgiPolicyPath;
      } else {
        const name = pathname === "/" ? "index.html" : pathname.slice(1);
        file = path.resolve(assetsRoot, name);
        assert(file.startsWith(`${path.resolve(assetsRoot)}${path.sep}`), "invalid asset path");
      }
      const bytes = await readFile(file);
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": mimeType(file),
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Resource-Policy": "same-origin",
      });
      response.end(bytes);
    } catch {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("not found\n");
    }
  });
}

function listen(server, port) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server.address().port));
  });
}

async function startServer(preferredPort) {
  const ports = preferredPort === 0 ? [0] : Array.from({ length: 20 }, (_, index) => preferredPort + index);
  let lastError = null;
  for (const port of ports) {
    const server = createServer();
    try {
      return { server, port: await listen(server, port) };
    } catch (error) {
      lastError = error;
      if (error.code !== "EADDRINUSE") {
        throw error;
      }
    }
  }
  throw lastError;
}

function closeServer(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function findChrome() {
  const candidates = [
    process.env.VINE_CHROME,
    process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Google", "Chrome", "Application", "chrome.exe"),
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Google", "Chrome", "Application", "chrome.exe"),
    process.env["ProgramFiles(x86)"] &&
      path.join(process.env["ProgramFiles(x86)"], "Google", "Chrome", "Application", "chrome.exe"),
  ].filter(Boolean);
  const chrome = candidates.find((candidate) => fs.existsSync(candidate));
  assert(chrome, "Chrome stable was not found; set VINE_CHROME to chrome.exe");
  return chrome;
}

async function chromeVersion(chrome) {
  if (process.platform !== "win32") {
    return (await run(chrome, ["--version"])).stdout.trim();
  }
  return (
    await run(
      "powershell.exe",
      ["-NoProfile", "-Command", "(Get-Item -LiteralPath $env:VINE_CHROME_PATH).VersionInfo.ProductVersion"],
      { env: { ...process.env, VINE_CHROME_PATH: chrome } },
    )
  ).stdout.trim();
}

async function stopChrome(chrome) {
  if (chrome === null) {
    return;
  }
  terminateProcess(chrome.child);
  if (!(await Promise.race([chrome.exit.then(() => true), wait(5_000).then(() => false)]))) {
    terminateProcess(chrome.child, "SIGKILL");
    await chrome.exit;
  }
}

async function waitForShutdown() {
  await new Promise((resolve) => {
    process.once("SIGINT", resolve);
    process.once("SIGTERM", resolve);
  });
}

assert(fs.existsSync(path.join(assetsRoot, "index.html")), "browser runtime is not built");
if (bgiMode) {
  assert(fs.existsSync(path.join(assetsRoot, "bgi-check.html")), "BGI browser check is not built");
  assert(fs.existsSync(bgiPath), `BGI source is missing at ${bgiPath}`);
  assert(fs.existsSync(bgiPolicyPath), `BGI policy is missing at ${bgiPolicyPath}`);
  assert(fs.existsSync(bgiOracleReportPath), "Node BGI oracle report is missing");
  assert(fs.existsSync(bgiBenchmarkConfigurationPath), "BGI benchmark configuration is missing");
  assert(
    fs.existsSync(path.join(assetsRoot, "bgi-benchmark.html")),
    "BGI browser benchmark is not built",
  );
  assert(
    !fs.existsSync(path.join(assetsRoot, "BGI.exe")) &&
      !fs.existsSync(path.join(assetsRoot, "private", "BGI.exe")),
    "private BGI source must not be copied into the browser build",
  );
}
assert(Number.isFinite(stressMilliseconds) && stressMilliseconds >= 0, "--stress-ms is invalid");
assert(
  Number.isFinite(bgiBenchmarkMilliseconds) && bgiBenchmarkMilliseconds >= 0,
  "--bgi-benchmark-ms is invalid",
);
const requestedPort = serveOnly ? Number(optionValue("--port") ?? 4173) : 0;
const { server, port } = await startServer(requestedPort);
const url = `http://127.0.0.1:${port}/`;

if (serveOnly) {
  console.log(`Vine browser runtime listening at ${url}`);
  await waitForShutdown();
  await closeServer(server);
} else {
  const temporaryRoot = path.resolve(os.tmpdir());
  let profile = null;
  let chrome = null;
  try {
    const chromePath = findChrome();
    const version = await chromeVersion(chromePath);
    profile = await mkdtemp(path.join(temporaryRoot, "vine-browser-runtime-"));
    chrome = await launchChrome(chromePath, [
      "--headless=new",
      ...(process.platform === "linux"
        ? ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
        : []),
      "--disable-background-networking",
      "--disable-breakpad",
      "--disable-component-update",
      "--disable-default-apps",
      "--disable-extensions",
      "--disable-sync",
      "--enable-precise-memory-info",
      "--metrics-recording-only",
      "--mute-audio",
      "--no-default-browser-check",
      "--no-first-run",
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "--window-size=1280,800",
      "about:blank",
    ]);
    if (bgiMode) {
      const sourceBefore = await readFile(bgiPath);
      if (bgiBenchmarkMilliseconds > 0) {
        const configurationBytes = await readFile(bgiBenchmarkConfigurationPath);
        const configuration = JSON.parse(configurationBytes);
        assert.equal(configuration.schema, "vine.bgi-browser-benchmark");
        assert.equal(configuration.schemaVersion, 1);
        assert.equal(
          bgiBenchmarkMilliseconds,
          configuration.durationMilliseconds,
          "BGI benchmark duration differs from its versioned configuration",
        );
        assert(
          Number.isFinite(configuration.sampleIntervalMilliseconds) &&
            configuration.sampleIntervalMilliseconds >= 1_000,
          "BGI benchmark sample interval is invalid",
        );
        const result = await runBgiBenchmark(
          chrome.browserWebSocketUrl,
          url,
          bgiBenchmarkMilliseconds,
          configuration.sampleIntervalMilliseconds,
        );
        const {
          metrics,
          thresholdEvaluations,
          thresholdFailures,
          decision,
        } = summarizeBgiBenchmark(result, configuration);
        const sourceAfter = await readFile(bgiPath);
        assert.deepEqual(sourceAfter, sourceBefore, "BGI source changed during the benchmark");
        const artifactPaths = {
          runtimeWorker: path.join(assetsRoot, "runtime-worker.js"),
          benchmarkMain: path.join(assetsRoot, "bgi-benchmark.js"),
          runtimeCoreWasm: path.join(assetsRoot, "runtime-core.wasm"),
          v86AdapterWasm: path.join(assetsRoot, "v86-adapter.wasm"),
        };
        const [commit, status, artifactEntries] = await Promise.all([
          run("git", ["rev-parse", "HEAD"]),
          run("git", ["status", "--short"]),
          Promise.all(
            Object.entries(artifactPaths).map(async ([name, artifactPath]) => {
              const bytes = await readFile(artifactPath);
              return [name, { bytes: bytes.byteLength, sha256: hash(bytes) }];
            }),
          ),
        ]);
        const buildManifest = JSON.parse(
          await readFile(path.join(assetsRoot, "build-manifest.json")),
        );
        const { profile: executionEngineProfile, ...builtExecutionEngine } =
          buildManifest.executionEngine;
        assert.deepEqual(
          result.executionEngine,
          builtExecutionEngine,
          "benchmark engine identity differs from the browser build",
        );
        const report = {
          schema: "vine.browser-bgi-benchmark-report",
          schemaVersion: 2,
          measuredAt: new Date().toISOString(),
          evidenceClassification: structuredClone(EVIDENCE_CLASSIFICATION),
          decision,
          environment: {
            host: `${os.platform()} ${os.release()} ${os.arch()}`,
            cpu: os.cpus()[0]?.model ?? "unknown",
            logicalCpus: os.cpus().length,
            totalMemoryBytes: os.totalmem(),
            chrome: version,
            node: process.version,
          },
          source: {
            repository: {
              commit: commit.stdout.trim(),
              dirty: status.stdout.trim().length > 0,
            },
            privateExecutable: {
              bytes: sourceBefore.byteLength,
              sha256: hash(sourceBefore),
            },
            executionEngine: {
              profile: executionEngineProfile,
              ...result.executionEngine,
            },
            artifacts: Object.fromEntries(artifactEntries),
          },
          configuration: {
            path: path.relative(root, bgiBenchmarkConfigurationPath),
            bytes: configurationBytes.byteLength,
            sha256: hash(configurationBytes),
            value: configuration,
          },
          metrics,
          thresholdEvaluations,
          thresholdFailures,
          samples: result,
        };
        const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
        await writeFile(path.join(assetsRoot, "bgi-browser-benchmark-report.json"), reportBytes);
        assert.equal(
          decision.status,
          "passed",
          `BGI benchmark failed (${decision.reason}): ${thresholdFailures}`,
        );
        console.log(
          `BGI browser benchmark passed: sessions=${metrics.completedSessions}, ` +
            `p95=${metrics.sessionLatencyMilliseconds.p95.toFixed(2)}ms, ` +
            `sliceP99=${metrics.maximumSliceMilliseconds.p99.toFixed(2)}ms, ` +
            `mainSlope=${metrics.mainHeap.slopeBytesPerMinute?.toFixed(2)}, ` +
            `workerSlope=${metrics.workerHeap.slopeBytesPerMinute?.toFixed(2)}, ` +
            `decision=${decision.reason}, reportSha256=${hash(reportBytes)}`,
        );
      } else {
        const oracleBytes = await readFile(bgiOracleReportPath);
        const oracle = JSON.parse(oracleBytes);
        const success = await runBgiProbe(chrome.browserWebSocketUrl, url, "success");
        const cancellation = await runBgiProbe(
          chrome.browserWebSocketUrl,
          url,
          "cancel-pending",
        );
        const watchdog = await runBgiProbe(chrome.browserWebSocketUrl, url, "watchdog");
        console.log(
          `BGI browser generations ${success.report.initialGeneration}->${success.report.finalGeneration}; ` +
            `writebacks=${success.report.writebackBatches}/${success.report.writebackPages}, ` +
            `mappingDeltas=${success.report.mappingDeltas}`,
        );
        validateBgiEquivalence(success, oracle);
        const sourceAfter = await readFile(bgiPath);
        assert.deepEqual(sourceAfter, sourceBefore, "BGI source changed during the browser session");
        const report = {
          schema: "vine.browser-bgi-runtime-worker",
          schemaVersion: 3,
          runtimeProtocol,
          measuredAt: new Date().toISOString(),
          chromeVersion: version,
          source: { bytes: sourceBefore.byteLength, sha256: hash(sourceBefore) },
          nodeOracle: {
            path: path.relative(root, bgiOracleReportPath),
            bytes: oracleBytes.byteLength,
            sha256: hash(oracleBytes),
          },
          success,
          cancellation,
          watchdog,
        };
        const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
        await writeFile(path.join(assetsRoot, "bgi-browser-report.json"), reportBytes);
        console.log(
          `BGI Chrome Worker equivalence passed: calls=${success.report.supportedCalls.length}, ` +
            `instructions=${success.report.instructions}, generation=${success.report.finalGeneration}, ` +
            `cancel/watchdog=clean, Chrome=${version}, reportSha256=${hash(reportBytes)}`,
        );
      }
    } else {
      const success = await runProbe(chrome.browserWebSocketUrl, url, "success", true);
      const cancellation = await runProbe(chrome.browserWebSocketUrl, url, "cancel-pending");
      const watchdog = await runProbe(chrome.browserWebSocketUrl, url, "watchdog");
      let stress = null;
      if (stressMilliseconds > 0) {
        const target = await createDebugTarget(chrome.browserWebSocketUrl, `${url}?scenario=success`);
        const client = await connectDevTools(target.webSocketDebuggerUrl);
        try {
          await client.send("Runtime.enable");
          await client.send("Page.enable");
          await waitForRuntime(client);
          await clickRun(client);
          await waitForCheckpoint(client);
          stress = await runStress(client, stressMilliseconds);
        } finally {
          client.close();
        }
      }
      const report = {
        schema: "vine.browser-runtime-worker",
        schemaVersion: 4,
        runtimeProtocol,
        measuredAt: new Date().toISOString(),
        chromeVersion: version,
        success,
        cancellation,
        watchdog,
        stress,
      };
      const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
      await writeFile(path.join(assetsRoot, "browser-report.json"), reportBytes);
      console.log(
        `browser Worker vertical slice passed: instructions=${success.evidence.report.instructions}, ` +
          `yields=${success.evidence.report.budgetYields}, callbacks=2/depth2, ` +
          `pixel=rgba(255,0,0,255), audio=880Hz, storage=0x5a, ` +
          `cancel/watchdog=clean, Chrome=${version}, reportSha256=${hash(reportBytes)}`,
      );
    }
  } finally {
    try {
      await stopChrome(chrome);
    } finally {
      await closeServer(server);
      if (profile !== null) {
        const safePrefix = path.join(temporaryRoot, "vine-browser-runtime-");
        assert(profile.startsWith(safePrefix), "refusing to remove unexpected Chrome profile");
        await rm(profile, { force: true, maxRetries: 5, recursive: true, retryDelay: 100 });
      }
    }
  }
}
