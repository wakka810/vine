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
import { loadChromeForTestingArtifact } from "./chrome-for-testing.mjs";
import {
  loadBrowserPerformanceEnvironmentPolicy,
  qualifyBenchmarkDecision,
  startBrowserPerformanceEnvironmentMonitor,
} from "./browser-performance-environment.mjs";
import {
  BROWSER_BGI_REPORT_SCHEMA,
  BROWSER_BGI_REPORT_SCHEMA_VERSION,
} from "./browser-bgi-report-contract.mjs";
import {
  handleSourceSnapshotHttp,
  registerSourceSnapshot,
} from "./source-snapshot.mjs";
import {
  handleWritableProfileHttp,
  openWritableProfile,
  readWritableProfile,
  snapshotWritableProfile,
} from "./writable-profile.mjs";
import {
  SEMANTIC_CALL_DIGEST_ALGORITHM,
  normalizeSemanticCall,
} from "../browser/runtime-worker/semantic-call.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, ".work", "browser-runtime");
const serveOnly = process.argv.includes("--serve");
const fullEvidence = process.argv.includes("--full-evidence");
const bgiBenchmarkMilliseconds = Number(optionValue("--bgi-benchmark-ms") ?? 0);
const bgiMode = process.argv.includes("--bgi") || bgiBenchmarkMilliseconds > 0;
const bgiCpuProfile = process.argv.includes("--bgi-cpu-profile");
const stressMilliseconds = Number(optionValue("--stress-ms") ?? 0);
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const bgiProfileOverride = optionValue("--bgi-profile");
const bgiPolicyOverride = optionValue("--bgi-policy");
const bgiBenchmarkConfigurationOverride = optionValue("--bgi-benchmark-config");
assert(
  (bgiProfileOverride === null && bgiPolicyOverride === null) ||
    (bgiMode &&
      (bgiBenchmarkMilliseconds === 0 ||
        bgiBenchmarkConfigurationOverride !== null)),
  "benchmark profile overrides require an explicit benchmark configuration",
);
assert.equal(
  bgiProfileOverride === null,
  bgiPolicyOverride === null,
  "--bgi-profile and --bgi-policy must be provided together",
);
assert(
  bgiBenchmarkConfigurationOverride === null ||
    (bgiBenchmarkMilliseconds > 0 && bgiProfileOverride !== null),
  "--bgi-benchmark-config requires benchmark profile and policy overrides",
);
assert(
  !bgiCpuProfile || bgiBenchmarkMilliseconds > 0,
  "--bgi-cpu-profile requires a BGI benchmark",
);
const bgiProfile =
  bgiProfileOverride ?? checkpoint.activeRuntimeCheckpoint.profile;
const d3d9InitialTextureStageStatesProfile =
  bgiProfile === "d3d9-initial-texture-stage-states";
const d3d9InitialSamplerStatesProfile =
  d3d9InitialTextureStageStatesProfile ||
  bgiProfile === "d3d9-initial-sampler-states";
const d3d9BeginSceneProfile =
  d3d9InitialSamplerStatesProfile || bgiProfile === "d3d9-begin-scene";
const d3d9UpdateTextureProfile =
  d3d9BeginSceneProfile || bgiProfile === "d3d9-update-texture";
const d3d9AddDirtyRectProfile =
  d3d9UpdateTextureProfile || bgiProfile === "d3d9-add-dirty-rect";
const setRectProfile =
  d3d9AddDirtyRectProfile || bgiProfile === "user32-set-rect";
const d3d9ClearProfile = setRectProfile || bgiProfile === "d3d9-clear";
const updateWindowProfile =
  d3d9ClearProfile || bgiProfile === "user32-update-window";
const showWindowNormalProfile =
  updateWindowProfile || bgiProfile === "user32-show-window-normal";
const setWindowPosFrameChangeMoveProfile =
  showWindowNormalProfile ||
  bgiProfile === "user32-set-window-pos-frame-change-move";
const d3d9RecreatedTexturesProfile =
  setWindowPosFrameChangeMoveProfile ||
  bgiProfile === "d3d9-recreated-textures";
const setWindowPosRepeatActivationProfile =
  d3d9RecreatedTexturesProfile ||
  bgiProfile === "user32-set-window-pos-repeat-activation";
const createDirectoryAProfile =
  setWindowPosRepeatActivationProfile ||
  bgiProfile === "kernel32-create-directory-a";
const createDirectoryAInstructionOffset = createDirectoryAProfile ? 521 : 0;
const getFileAttributesAProfile =
  createDirectoryAProfile || bgiProfile === "kernel32-get-file-attributes-a";
const globalMemoryStatusProfile =
  getFileAttributesAProfile || bgiProfile === "kernel32-global-memory-status";
const sleepZeroProfile =
  globalMemoryStatusProfile || bgiProfile === "kernel32-sleep-zero";
const archiveSourceReadProfile =
  sleepZeroProfile || bgiProfile === "kernel32-archive-source-read";
const runtimeProtocol = "vine.browser-runtime-worker.v35";
assert.equal(
  checkpoint.browserCheckpoint.browserRuntimeProtocol,
  runtimeProtocol,
  "current checkpoint does not identify the built Runtime Worker protocol",
);
const bgiPath = path.resolve(
  process.env.VINE_BGI_EXE ??
    "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const bgiPolicyPath = path.resolve(
  root,
  bgiPolicyOverride ??
    checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
);
const bgiGdbPath = path.join(path.dirname(bgiPath), "BGI.gdb");
const sourceManifestPath = path.join(
  root,
  "fixtures",
  "source-snapshots",
  "controlled-source-read-v1.json",
);
const sourceObjectPath = path.join(
  root,
  "fixtures",
  "storage",
  "source-read-v1.dat",
);
const bgiOracleReportPath = path.join(
  root,
  ".work",
  bgiProfileOutputDirectory(bgiProfile),
  "report.json",
);
const bgiBenchmarkConfigurationPath = path.join(
  root,
  bgiBenchmarkConfigurationOverride ??
    checkpoint.browserCheckpoint.bgiBenchmarkEvidence.configuration,
);
const browserPerformanceEnvironmentPolicyPath = path.join(
  root,
  "fixtures",
  "benchmarks",
  "browser-performance-environment-windows-v1.json",
);

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index < 0 ? null : (process.argv[index + 1] ?? null);
}

function wait(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function hash(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function run(
  command,
  arguments_,
  { env = process.env, timeoutMs = 30_000 } = {},
) {
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
      reject(
        new Error(
          `${path.basename(command)} timed out\n${stdout}${stderr}`.trim(),
        ),
      );
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
        reject(
          new Error(
            `${path.basename(command)} failed (${signal ?? code})\n${stdout}${stderr}`,
          ),
        );
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
      () =>
        fail(new Error(`Chrome did not expose DevTools\n${stdout}${stderr}`)),
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
        resolve({
          browserWebSocketUrl: match[1],
          child,
          exit,
          logs: () => `${stdout}${stderr}`,
        });
      }
    });
    child.once("error", fail);
    child.once("close", (code, signal) => {
      resolveExit({ code, signal });
      fail(
        new Error(
          `Chrome exited before DevTools (${signal ?? code})\n${stdout}${stderr}`,
        ),
      );
    });
  });
}

async function connectDevTools(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("DevTools connection timed out")),
      10_000,
    );
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
      request.reject(
        new Error(`DevTools ${request.method}: ${message.error.message}`),
      );
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
        JSON.stringify({
          id,
          method,
          params,
          ...(sessionId === null ? {} : { sessionId }),
        }),
      );
    });
  }
}

async function createDebugTarget(browserWebSocketUrl, url) {
  const browserEndpoint = new URL(browserWebSocketUrl);
  const response = await fetch(
    `http://${browserEndpoint.host}/json/new?${encodeURIComponent(url)}`,
    {
      method: "PUT",
    },
  );
  assert(response.ok, `Chrome target creation failed with ${response.status}`);
  const target = await response.json();
  assert.equal(typeof target.webSocketDebuggerUrl, "string");
  return target;
}

async function evaluate(
  client,
  expression,
  { awaitPromise = false, timeoutMs = 10_000 } = {},
) {
  const result = await client.send(
    "Runtime.evaluate",
    { expression, awaitPromise, returnByValue: true },
    timeoutMs,
  );
  if (result.exceptionDetails !== undefined) {
    throw new Error(
      `browser evaluation failed: ${JSON.stringify(result.exceptionDetails)}`,
    );
  }
  return result.result.value;
}

async function waitForRuntime(client) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      if (
        await evaluate(
          client,
          `Boolean(window.__vineRuntime && document.querySelector("#run-button"))`,
        )
      ) {
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
    if (
      snapshot.checkStatus === "passed" ||
      snapshot.checkStatus === "failed"
    ) {
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
  return {
    file: path.relative(root, file),
    bytes: bytes.byteLength,
    sha256: hash(bytes),
  };
}

function assertExecutionStateMachine(report, scenario) {
  assert.equal(report.executionState, "stopped");
  assert(
    report.executionTransitionCount > 0,
    "execution state machine did not transition",
  );
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
    assert(
      report.watchdogContext.lastTransition !== null,
      "watchdog transition is missing",
    );
  } else {
    assert.equal(
      report.watchdogContext,
      null,
      "non-watchdog report retained watchdog context",
    );
  }
}

function assertExecutionEngine(report) {
  assert.equal(report.executionEngine?.schema, "vine.execution-engine-profile");
  assert.equal(report.executionEngine?.schemaVersion, 1);
  assert.equal(report.executionEngine?.engine, "v86");
  assert.equal(report.executionEngineLinearMemoryBytes > 0, true);
}

async function runProbe(
  browserWebSocketUrl,
  baseUrl,
  mode,
  screenshots = false,
) {
  const target = await createDebugTarget(
    browserWebSocketUrl,
    `${baseUrl}?scenario=${mode}`,
  );
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    await waitForRuntime(client);
    await clickRun(client);
    const snapshot = await waitForCheckpoint(client);
    assert.equal(
      snapshot.checkStatus,
      "passed",
      snapshot.error === null
        ? `${mode} failed`
        : `${snapshot.error}\n${snapshot.report ?? "no session report"}`,
    );
    const evidence = JSON.parse(snapshot.report);
    assert.equal(evidence.report.pendingHostCalls, 0);
    assert.equal(evidence.report.pendingCallbacks, 0);
    assertExecutionStateMachine(evidence.report, mode);
    assertExecutionEngine(evidence.report);
    if (mode === "watchdog") {
      assert.notEqual(evidence.report.watchdogContext.requestId, null);
      assert.equal(
        evidence.report.watchdogContext.lastTransition?.to,
        "waiting-host",
      );
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
      assert.deepEqual(evidence.report.registeredThreadIds, [8192, 8193]);
      assert.deepEqual(evidence.report.executedThreadIds, [8192, 8193]);
      assert.equal(evidence.report.contextSwitches, 2);
      assert.deepEqual(evidence.report.hostWaitExecutions, [
        { requestId: "1", waitingThreadId: 8192, executedThreadId: 8193 },
      ]);
      assert.deepEqual(evidence.report.threadExits, [
        {
          threadId: 8193,
          exitCode: 7,
          markerAddress: 0x1000_3000,
          markerValue: 0x5448_5244,
        },
      ]);
      assert.equal(evidence.report.callbackEntries, 2);
      assert.equal(evidence.report.maximumCallbackDepth, 2);
      assert(
        evidence.report.budgetYields > 0,
        "success path did not yield periodically",
      );
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
      assert.equal(
        evidence.report.outcome,
        mode === "watchdog" ? "watchdog" : "cancelled",
      );
    }
    const captures = [];
    if (screenshots) {
      captures.push(
        await captureScreenshot(client, "browser-runtime-desktop.png"),
      );
      await client.send("Emulation.setDeviceMetricsOverride", {
        width: 390,
        height: 844,
        deviceScaleFactor: 1,
        mobile: true,
      });
      await wait(200);
      captures.push(
        await captureScreenshot(client, "browser-runtime-mobile.png"),
      );
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
    try {
      snapshot = await evaluate(client, `window.__vineBgiCheck`);
    } catch (error) {
      if (
        error instanceof Error &&
        error.message.includes("Cannot find default execution context")
      ) {
        await wait(50);
        continue;
      }
      throw error;
    }
    if (snapshot?.error !== null && snapshot?.error !== undefined) {
      throw new Error(`BGI browser runtime failed: ${snapshot.error}`);
    }
    if (snapshot?.result !== null && snapshot?.result !== undefined) {
      return snapshot.result;
    }
    await wait(50);
  }
  const diagnostics = await evaluate(
    client,
    `({
    check: window.__vineBgiCheck,
    status: document.querySelector("#status")?.textContent ?? null,
    body: { ...document.body.dataset },
  })`,
  );
  throw new Error(
    `BGI browser runtime timed out after ${timeoutMs}ms: ${JSON.stringify(diagnostics)}`,
  );
}

async function runBgiProbe(browserWebSocketUrl, baseUrl, scenario) {
  const target = await createDebugTarget(
    browserWebSocketUrl,
    `${baseUrl}bgi-check.html?scenario=${scenario}&profile=${encodeURIComponent(bgiProfile)}${fullEvidence ? "&evidence=full" : ""}`,
  );
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    const evidence = await waitForBgiCheckpoint(
      client,
      scenario === "success" && sleepZeroProfile
        ? 300_000
        : scenario === "success" &&
            (bgiProfile === "shell32-drag-accept-files" ||
              bgiProfile === "kernel32-archive-source-read")
          ? 180_000
          : 30_000,
    );
    assert.equal(evidence.scenario, scenario);
    assert.equal(evidence.report.pendingHostCalls, 0);
    assert.equal(evidence.report.pendingCallbacks, 0);
    assertExecutionStateMachine(evidence.report, scenario);
    assertExecutionEngine(evidence.report);
    assert.deepEqual(
      evidence.report.registeredThreadIds,
      [
        "kernel32-create-thread",
        "kernel32-set-thread-priority",
        "gdi-screen-dc",
        "gdi-font-enumeration",
        "gdi-font-creation",
        "gdi-compatible-dc",
        "gdi-dib-lifecycle",
        "gdi-outline-text-metrics",
        "gdi-text-raster",
        "gdiplus-startup",
        "kernel32-virtual-alloc",
        "imm32-default-ime-context",
        "kernel32-set-waitable-timer",
        "kernel32-wait-for-single-object-ex",
        "kernel32-enter-critical-section",
        "kernel32-enter-critical-section-continuation",
        "kernel32-leave-critical-section-continuation",
        "primary-main-loop-progression",
        "kernel32-leave-critical-section",
        "shell32-drag-accept-files",
        "kernel32-archive-source-read",
        "kernel32-sleep-zero",
        "kernel32-global-memory-status",
        "kernel32-get-file-attributes-a",
        "kernel32-create-directory-a",
        "user32-set-window-pos-repeat-activation",
        "d3d9-recreated-textures",
        "user32-set-window-pos-frame-change-move",
        "user32-show-window-normal",
        "user32-update-window",
        "d3d9-clear",
        "user32-set-rect",
        "d3d9-add-dirty-rect",
        "d3d9-update-texture",
        "d3d9-begin-scene",
        "d3d9-initial-sampler-states",
        "d3d9-initial-texture-stage-states",
      ].includes(bgiProfile) && scenario === "success"
        ? [8192, 8193]
        : [8192],
    );
    assert.deepEqual(
      evidence.report.executedThreadIds,
      [
        "imm32-default-ime-context",
        "kernel32-set-waitable-timer",
        "kernel32-wait-for-single-object-ex",
        "kernel32-enter-critical-section",
        "kernel32-enter-critical-section-continuation",
        "kernel32-leave-critical-section-continuation",
        "primary-main-loop-progression",
        "kernel32-leave-critical-section",
        "shell32-drag-accept-files",
        "kernel32-archive-source-read",
        "kernel32-sleep-zero",
        "kernel32-global-memory-status",
        "kernel32-get-file-attributes-a",
        "kernel32-create-directory-a",
        "user32-set-window-pos-repeat-activation",
        "d3d9-recreated-textures",
        "user32-set-window-pos-frame-change-move",
        "user32-show-window-normal",
        "user32-update-window",
        "d3d9-clear",
        "user32-set-rect",
        "d3d9-add-dirty-rect",
        "d3d9-update-texture",
        "d3d9-begin-scene",
        "d3d9-initial-sampler-states",
        "d3d9-initial-texture-stage-states",
      ].includes(bgiProfile) && scenario === "success"
        ? [8192, 8193]
        : [8192],
    );
    assert.equal(
      evidence.report.contextSwitches,
      [
        "imm32-default-ime-context",
        "kernel32-set-waitable-timer",
        "kernel32-wait-for-single-object-ex",
        "kernel32-enter-critical-section",
        "kernel32-enter-critical-section-continuation",
        "kernel32-leave-critical-section-continuation",
        "primary-main-loop-progression",
        "kernel32-leave-critical-section",
        "shell32-drag-accept-files",
        "kernel32-archive-source-read",
        "kernel32-sleep-zero",
        "kernel32-global-memory-status",
        "kernel32-get-file-attributes-a",
        "kernel32-create-directory-a",
        "user32-set-window-pos-repeat-activation",
        "d3d9-recreated-textures",
        "user32-set-window-pos-frame-change-move",
        "user32-show-window-normal",
        "user32-update-window",
        "d3d9-clear",
        "user32-set-rect",
        "d3d9-add-dirty-rect",
        "d3d9-update-texture",
        "d3d9-begin-scene",
        "d3d9-initial-sampler-states",
        "d3d9-initial-texture-stage-states",
      ].includes(bgiProfile) && scenario === "success"
        ? d3d9RecreatedTexturesProfile
          ? 2_010
          : createDirectoryAProfile
            ? 1_526
            : getFileAttributesAProfile
              ? 1_518
              : sleepZeroProfile
                ? 1_512
                : bgiProfile === "kernel32-archive-source-read"
                  ? 1_032
                  : bgiProfile === "shell32-drag-accept-files"
                    ? 1_030
                    : bgiProfile === "primary-main-loop-progression" ||
                        bgiProfile === "kernel32-leave-critical-section" ||
                        bgiProfile ===
                          "kernel32-enter-critical-section-continuation" ||
                        bgiProfile ===
                          "kernel32-leave-critical-section-continuation"
                      ? 2
                      : 1
        : 0,
    );
    assert.deepEqual(evidence.report.initialCurrentDirectory, {
      directory: {
        sourceAuthority: "browser-private-process-bootstrap",
        bytesWithNul: [67, 58, 92, 86, 105, 110, 101, 0],
        encoding: "windows-932-v1",
      },
      generation: 0,
    });
    assert(evidence.statuses.some(({ phase }) => phase === "running"));
    if (scenario === "success") {
      const capabilityCheckpoint =
        bgiProfile === "kernel32-wait-for-single-object-ex" ||
        bgiProfile === "kernel32-enter-critical-section" ||
        bgiProfile === "kernel32-enter-critical-section-continuation" ||
        bgiProfile === "kernel32-leave-critical-section-continuation" ||
        bgiProfile === "primary-main-loop-progression" ||
        bgiProfile === "kernel32-leave-critical-section";
      assert.equal(
        evidence.report.outcome,
        capabilityCheckpoint ? "checkpoint-complete" : "structured-stop",
      );
      assert(evidence.report.supportedCallCount > 0);
      assert.equal(
        evidence.report.evidenceLevel,
        fullEvidence ? "full" : "summary",
      );
      assert.equal(
        evidence.report.supportedCalls.length,
        fullEvidence ? evidence.report.supportedCallCount : 0,
      );
      assert.equal(
        evidence.report.supportedCallSemanticsAlgorithm,
        SEMANTIC_CALL_DIGEST_ALGORITHM,
      );
      assert.match(
        evidence.report.supportedCallSemanticsSha256,
        /^[0-9a-f]{64}$/u,
      );
      assert.equal(
        evidence.report.firstUnsupported === null,
        capabilityCheckpoint,
      );
      assert.equal(
        evidence.report.checkpointCompletion !== null,
        capabilityCheckpoint,
      );
      if (
        capabilityCheckpoint ||
        bgiProfile === "shell32-drag-accept-files" ||
        bgiProfile === "kernel32-archive-source-read" ||
        bgiProfile === "kernel32-sleep-zero" ||
        bgiProfile === "kernel32-global-memory-status" ||
        bgiProfile === "kernel32-get-file-attributes-a" ||
        bgiProfile === "kernel32-create-directory-a" ||
        setWindowPosRepeatActivationProfile
      ) {
        const pacingCount = d3d9RecreatedTexturesProfile
          ? 9_186
          : createDirectoryAProfile
            ? 6_974
            : getFileAttributesAProfile
              ? 6_937
              : sleepZeroProfile
                ? 6_910
                : bgiProfile === "kernel32-archive-source-read"
                  ? 4_716
                  : bgiProfile === "shell32-drag-accept-files"
                    ? 4_706
                    : bgiProfile === "primary-main-loop-progression" ||
                        bgiProfile === "kernel32-leave-critical-section" ||
                        bgiProfile ===
                          "kernel32-enter-critical-section-continuation" ||
                        bgiProfile ===
                          "kernel32-leave-critical-section-continuation"
                      ? 7
                      : 3;
        assert.deepEqual(evidence.report.timerPacing, {
          requests: pacingCount,
          completions: pacingCount,
          cancellations: 0,
          freezes: 0,
          resumes: 0,
          staleCallbacks: 0,
          boundedCatchUps: 0,
          pending: 0,
        });
      }
      if (
        bgiProfile === "shell32-drag-accept-files" ||
        bgiProfile === "kernel32-archive-source-read" ||
        bgiProfile === "kernel32-sleep-zero" ||
        bgiProfile === "kernel32-global-memory-status" ||
        bgiProfile === "kernel32-get-file-attributes-a" ||
        bgiProfile === "kernel32-create-directory-a" ||
        setWindowPosRepeatActivationProfile
      ) {
        const dragAcceptFiles = evidence.report.dragAcceptFiles;
        assert(
          dragAcceptFiles !== null,
          "Chrome Worker omitted DragAcceptFiles evidence",
        );
        assert.equal(dragAcceptFiles.call.sequence, 89_249);
        assert.equal(
          dragAcceptFiles.call.trapInstructionCount,
          15_601_759 + createDirectoryAInstructionOffset,
        );
        assert.deepEqual(dragAcceptFiles.call.arguments, [0x4801_0001, 0]);
        assert.deepEqual(dragAcceptFiles.call.result, { action: "preserve" });
        assert.equal(dragAcceptFiles.call.returnAddress, 0x1005_f272);
        assert.equal(
          dragAcceptFiles.call.stackPointerAfter -
            dragAcceptFiles.call.stackPointerBefore,
          12,
        );
        assert.equal(dragAcceptFiles.call.lastError, 0);
        assert.deepEqual(dragAcceptFiles.call.guestWrites, []);
        assert.equal(dragAcceptFiles.call.window.handle, 0x4801_0001);
        assert.equal(
          dragAcceptFiles.call.window.effectiveExtendedStyle,
          0x0004_0100,
        );
        assert.equal(
          dragAcceptFiles.call.window.effectiveExtendedStyle & 0x0000_0010,
          0,
        );
        assert.equal(dragAcceptFiles.call.window.lifecycle.state, "created");
        assert.equal(dragAcceptFiles.browserResourceCreated, false);
        assert.equal(dragAcceptFiles.wmDropFilesImplemented, false);
      } else {
        assert.equal(evidence.report.dragAcceptFiles, null);
      }
      if (archiveSourceReadProfile) {
        const archiveReads = evidence.report.sourceReads
          .filter((read) => read.request.sourceId === 1)
          .map((read) => ({
            objectId: read.request.objectId,
            offset: read.request.offset,
            requestedBytes: read.request.requestedBytes,
            transferredBytes: read.transferredBytes,
            succeeded: read.succeeded,
          }));
        assert.deepEqual(
          archiveReads,
          [
            {
              objectId: 1,
              offset: 0,
              requestedBytes: 16,
              transferredBytes: 16,
              succeeded: true,
            },
            {
              objectId: 1,
              offset: 16,
              requestedBytes: 384,
              transferredBytes: 384,
              succeeded: true,
            },
            {
              objectId: 1,
              offset: 400,
              requestedBytes: 2_328,
              transferredBytes: 2_328,
              succeeded: true,
            },
            ...(createDirectoryAProfile
              ? [
                  {
                    objectId: 2,
                    offset: 0,
                    requestedBytes: 16,
                    transferredBytes: 16,
                    succeeded: true,
                  },
                  {
                    objectId: 2,
                    offset: 16,
                    requestedBytes: 7_296,
                    transferredBytes: 7_296,
                    succeeded: true,
                  },
                  {
                    objectId: 2,
                    offset: 7_312,
                    requestedBytes: 1_530,
                    transferredBytes: 1_530,
                    succeeded: true,
                  },
                ]
              : []),
          ],
          "Chrome Worker primary boot-archive reads differ from the frozen slice",
        );
      }
      if (sleepZeroProfile) {
        const sleepZero = evidence.report.sleepZero;
        assert(sleepZero !== null, "Chrome Worker omitted Sleep(0) evidence");
        assert.equal(
          sleepZero.callCount,
          d3d9RecreatedTexturesProfile ? 480 : 240,
        );
        assert.equal(sleepZero.firstCall.sequence, 89_430);
        assert.equal(sleepZero.firstCall.occurrence, 1);
        assert.equal(
          sleepZero.firstCall.trapInstructionCount,
          15_953_766 + createDirectoryAInstructionOffset,
        );
        assert.deepEqual(sleepZero.firstCall.arguments, [0]);
        assert.deepEqual(sleepZero.firstCall.result, { action: "preserve" });
        assert.equal(sleepZero.firstCall.returnAddress, 0x1006_a2f5);
        assert.equal(
          sleepZero.firstCall.stackPointerAfter -
            sleepZero.firstCall.stackPointerBefore,
          8,
        );
        assert.equal(sleepZero.firstCall.lastError, 0);
        assert.deepEqual(sleepZero.firstCall.guestWrites, []);
        assert.equal(
          sleepZero.firstCall.executionBoundary,
          "yield-current-thread",
        );
        assert.equal(
          sleepZero.lastCall.sequence,
          d3d9RecreatedTexturesProfile ? 126_996 : 107_670,
        );
        assert.equal(
          sleepZero.lastCall.occurrence,
          d3d9RecreatedTexturesProfile ? 480 : 240,
        );
        assert.equal(
          sleepZero.lastCall.trapInstructionCount,
          d3d9RecreatedTexturesProfile
            ? 28_612_007
            : 16_290_005 + createDirectoryAInstructionOffset,
        );
        assert.equal(
          sleepZero.schedulerSelections.selectionCount,
          d3d9RecreatedTexturesProfile ? 480 : 240,
        );
        assert.equal(
          sleepZero.schedulerSelections.firstSelection?.sequence,
          89_430,
        );
        assert.equal(
          sleepZero.schedulerSelections.lastSelection?.sequence,
          d3d9RecreatedTexturesProfile ? 126_996 : 107_670,
        );
        assert.equal(sleepZero.noCompetitorCompletions, 0);
        assert.equal(sleepZero.schedulerAuthority, "rust-runtime-scheduler");
        assert.equal(sleepZero.hostWaitCreated, false);
      } else {
        assert.equal(evidence.report.sleepZero, null);
      }
      if (globalMemoryStatusProfile) {
        const memoryStatus = evidence.report.globalMemoryStatus;
        assert(
          memoryStatus !== null,
          "Chrome Worker omitted GlobalMemoryStatus evidence",
        );
        assert.equal(memoryStatus.call.sequence, 107_757);
        assert.equal(memoryStatus.call.occurrence, 1);
        assert.equal(
          memoryStatus.call.trapInstructionCount,
          16_384_108 + createDirectoryAInstructionOffset,
        );
        assert.deepEqual(memoryStatus.call.arguments, [0x006f_f674]);
        assert.deepEqual(memoryStatus.call.result, { action: "preserve" });
        assert.equal(memoryStatus.call.returnAddress, 0x1008_1fae);
        assert.equal(
          memoryStatus.call.stackPointerAfter -
            memoryStatus.call.stackPointerBefore,
          8,
        );
        assert.equal(memoryStatus.call.lastError, 0);
        assert.deepEqual(memoryStatus.call.guestWrites, [
          {
            address: 0x006f_f674,
            bytes: [
              0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
              0x7f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
              0xff, 0xff, 0x00, 0x00, 0xfe, 0x7f, 0x00, 0x00, 0x00, 0x60,
            ],
          },
        ]);
        assert.equal(
          memoryStatus.stateAuthority,
          "kernel32-compatibility-policy",
        );
        assert.equal(
          memoryStatus.virtualSpanAuthority,
          "kernel32-system-info-policy",
        );
        assert.equal(memoryStatus.hostMemoryImported, false);
        assert.equal(memoryStatus.browserMemoryImported, false);
        assert.equal(memoryStatus.allocatorQuotaAuthority, false);
      } else {
        assert.equal(evidence.report.globalMemoryStatus, null);
      }
      if (getFileAttributesAProfile) {
        const fileAttributes = evidence.report.getFileAttributesA;
        assert(
          fileAttributes !== null,
          "Chrome Worker omitted GetFileAttributesA evidence",
        );
        assert.equal(fileAttributes.callCount, 23);
        assert.deepEqual(fileAttributes.firstCall, {
          sequence: 107_788,
          module: "kernel32.dll",
          symbol: "GetFileAttributesA",
          occurrence: 1,
          trapInstructionCount: 16_396_079 + createDirectoryAInstructionOffset,
          arguments: [0x006f_f048],
          result: { action: "set", value: 0xffff_ffff },
          implementationId: "kernel32.get-file-attributes-a.runtime-vfs.v1",
          returnAddress: 0x1006_1c80,
          stackPointerBefore: 0x006f_f02c,
          stackPointerAfter: 0x006f_f034,
          lastError: 2,
          guestWrites: [],
        });
        assert.equal(fileAttributes.lastCall.sequence, 108_205);
        assert.equal(fileAttributes.lastCall.occurrence, 23);
        assert.equal(
          fileAttributes.lastCall.trapInstructionCount,
          16_445_360 + createDirectoryAInstructionOffset,
        );
        assert.deepEqual(fileAttributes.lastCall.arguments, [
          createDirectoryAProfile ? 0x202e_2448 : 0x202e_2418,
        ]);
        assert.equal(fileAttributes.lastCall.returnAddress, 0x1008_2648);
        assert.equal(fileAttributes.lastCall.stackPointerBefore, 0x006f_f688);
        assert.equal(fileAttributes.lastCall.stackPointerAfter, 0x006f_f690);
        assert.equal(fileAttributes.lastCall.lastError, 2);
        assert.deepEqual(fileAttributes.lastCall.guestWrites, []);
        assert.equal(fileAttributes.pathAuthority, "rust-runtime-vfs");
        assert.equal(
          fileAttributes.fileExistenceAuthority,
          "registered-source-and-writable-manifests",
        );
        assert.equal(
          fileAttributes.directoryExistence,
          createDirectoryAProfile
            ? "registered-path-ancestors-and-writable-directory-state"
            : "registered-path-ancestor-derivation",
        );
        assert.equal(fileAttributes.hostFileSystemImported, false);
        assert.equal(fileAttributes.browserFileSystemImported, false);
        assert.equal(fileAttributes.mutation, false);
      } else {
        assert.equal(evidence.report.getFileAttributesA, null);
      }
      if (createDirectoryAProfile) {
        const directory = evidence.report.createDirectoryA;
        assert(
          directory !== null,
          "Chrome Worker omitted CreateDirectoryA evidence",
        );
        assert.deepEqual(directory.call, {
          sequence: 108_206,
          module: "kernel32.dll",
          symbol: "CreateDirectoryA",
          occurrence: 1,
          trapInstructionCount: 16_446_219,
          arguments: [0x202e_2448, 0],
          result: { action: "set", value: 1 },
          implementationId: "kernel32.create-directory-a.runtime-vfs.v1",
          returnAddress: 0x1008_25e8,
          stackPointerBefore: 0x006f_f684,
          stackPointerAfter: 0x006f_f690,
          lastError: 2,
          guestWrites: [],
          writableDirectoryCreate: {
            request: {
              requestId: 6_943,
              directoryId: 1,
              relativePathA: [85, 115, 101, 114, 68, 97, 116, 97],
              baseGeneration: 0,
            },
            succeeded: true,
            committedGeneration: 1,
            win32Error: 0,
          },
        });
        assert.equal(
          directory.directoryAuthority,
          "rust-runtime-vfs-writable-profile",
        );
        assert.equal(
          directory.durableAuthority,
          "sqlite-writable-profile-directory-generation",
        );
        assert.equal(
          directory.transaction,
          "validate-plan-host-publish-targeted-commit",
        );
        assert.equal(directory.sourceMutation, false);
        assert.equal(directory.hostPathCreated, false);
        assert.equal(directory.browserFileSystemImported, false);
        assert.deepEqual(evidence.report.writableDirectoryCreates, [
          directory.call.writableDirectoryCreate,
        ]);
      } else {
        assert.equal(evidence.report.createDirectoryA, null);
        assert.deepEqual(evidence.report.writableDirectoryCreates, []);
      }
      if (setWindowPosRepeatActivationProfile) {
        const repeat = evidence.report.setWindowPosRepeatActivation;
        assert(
          repeat !== null,
          "Chrome Worker omitted SetWindowPos repeat-activation evidence",
        );
        assert.equal(repeat.callCount, 5);
        assert.equal(repeat.repeatCall.sequence, 108_718);
        assert.equal(repeat.repeatCall.module.toLowerCase(), "user32.dll");
        assert.equal(repeat.repeatCall.symbol, "SetWindowPos");
        assert.equal(repeat.repeatCall.occurrence, 5);
        assert.equal(repeat.repeatCall.trapInstructionCount, 28_273_977);
        assert.deepEqual(
          repeat.repeatCall.arguments,
          [0x4801_0001, 0xffff_fffe, 0, 0, 1_286, 749, 266],
        );
        assert.deepEqual(repeat.repeatCall.result, { action: "set", value: 1 });
        assert.equal(
          repeat.repeatCall.implementationId,
          "user32.set-window-pos.v5",
        );
        assert.equal(repeat.repeatCall.returnAddress, 0x1005_cb4b);
        assert.equal(repeat.repeatCall.stackPointerBefore, 0x006f_f5e0);
        assert.equal(repeat.repeatCall.stackPointerAfter, 0x006f_f600);
        assert.equal(repeat.repeatCall.lastError, 0);
        assert.deepEqual(repeat.repeatCall.guestWrites, []);
        assert.deepEqual(repeat.repeatCall.window.geometry, {
          x: 15_360,
          y: 8_640,
          width: 1_286,
          height: 749,
          clientX: 15_363,
          clientY: 8_666,
          clientWidth: 1_280,
          clientHeight: 720,
        });
        assert.deepEqual(repeat.repeatCall.callback, {
          frameId: 25,
          target: 0x1009_2550,
          allocationStart: 0x006f_f5cc,
          stackPointer: 0x006f_f5cc,
          scratchAddress: 0x0050_1000,
          scratchBytes: 28,
          frameBytes: 20,
          returnSentinel: 0x7f01_0000,
          returnInstructionCount: 28_274_079,
          result: 0,
        });
        assert.deepEqual(repeat.callbackEvents, [
          {
            action: "enter",
            sequence: 108_718,
            frameId: 25,
            instructionCount: 28_273_977,
            target: 0x1009_2550,
            stackPointer: 0x006f_f5cc,
            arguments: [0x4801_0001, 0x0046, 0, 0x0050_1000],
          },
          {
            action: "return",
            sequence: 108_718,
            frameId: 25,
            instructionCount: 28_274_079,
            result: 0,
          },
        ]);
        assert.equal(
          repeat.windowAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(
          repeat.relationAuthority,
          "rust-runtime-user32-thread-and-process-window-relations",
        );
        assert.equal(repeat.callbackDisposition, "validate-only");
        assert.equal(repeat.mutation, false);
        assert.equal(repeat.browserResourceCreated, false);
      } else {
        assert.equal(evidence.report.setWindowPosRepeatActivation, null);
      }
      if (setWindowPosFrameChangeMoveProfile) {
        const frameChange = evidence.report.setWindowPosFrameChangeMove;
        assert(
          frameChange !== null,
          "Chrome Worker omitted SetWindowPos frame-change move evidence",
        );
        assert.equal(frameChange.callCount, 6);
        assert.equal(frameChange.call.sequence, 127_346);
        assert.equal(frameChange.call.module.toLowerCase(), "user32.dll");
        assert.equal(frameChange.call.symbol, "SetWindowPos");
        assert.equal(frameChange.call.occurrence, 6);
        assert.equal(frameChange.call.trapInstructionCount, 28_760_801);
        assert.deepEqual(
          frameChange.call.arguments,
          [0x4801_0001, 0, 317, 154, 0, 0, 0x25],
        );
        assert.deepEqual(frameChange.call.result, { action: "set", value: 1 });
        assert.equal(
          frameChange.call.implementationId,
          "user32.set-window-pos.v5",
        );
        assert.equal(frameChange.call.returnAddress, 0x1009_38c2);
        assert.equal(frameChange.call.stackPointerBefore, 0x006f_f670);
        assert.equal(frameChange.call.stackPointerAfter, 0x006f_f690);
        assert.equal(frameChange.call.lastError, 0);
        assert.deepEqual(frameChange.call.guestWrites, []);
        assert.deepEqual(frameChange.call.window.geometry, {
          x: 317,
          y: 154,
          width: 1_286,
          height: 749,
          clientX: 320,
          clientY: 180,
          clientWidth: 1_280,
          clientHeight: 720,
        });
        assert.deepEqual(
          frameChange.callbackEvents.map((event) => [
            event.action,
            event.sequence,
            event.frameId,
            event.instructionCount,
            event.arguments ?? event.result,
          ]),
          [
            [
              "enter",
              127_346,
              26,
              28_760_801,
              [0x4801_0001, 0x0046, 0, 0x0050_1000],
            ],
            ["return", 127_346, 26, 28_760_858, 0],
            [
              "replace",
              127_346,
              27,
              28_760_858,
              [0x4801_0001, 0x0083, 1, 0x0050_1000],
            ],
            ["return", 127_346, 27, 28_760_911, 0],
            [
              "replace",
              127_346,
              28,
              28_760_911,
              [0x4801_0001, 0x0047, 0, 0x0050_1000],
            ],
            ["return", 127_346, 28, 28_761_017, 0],
          ],
        );
        assert.equal(frameChange.nestedMoveCallbackEvents.length, 2);
        assert.equal(frameChange.nestedMoveCallbackEvents[0].action, "enter");
        assert.equal(frameChange.nestedMoveCallbackEvents[0].sequence, 127_349);
        assert.deepEqual(
          frameChange.nestedMoveCallbackEvents[0].arguments,
          [0x4801_0001, 0x0003, 0, 0x00b4_0140],
        );
        assert.equal(frameChange.nestedMoveCallbackEvents[1].action, "return");
        assert.equal(frameChange.nestedMoveCallbackEvents[1].sequence, 127_349);
        assert.equal(frameChange.nestedMoveCallbackEvents[1].result, 0);
        assert.equal(
          frameChange.windowAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(
          frameChange.relationAuthority,
          "rust-runtime-user32-thread-and-process-window-relations",
        );
        assert.equal(
          frameChange.geometryCommit,
          "nccalcsize-completion-targeted-commit",
        );
        assert.equal(frameChange.relationMutation, false);
        assert.equal(frameChange.gdiResourceMutation, false);
        assert.equal(frameChange.browserResourceCreated, false);
        assert.equal(frameChange.nonClientPaintAndEraseModeled, false);
      } else {
        assert.equal(evidence.report.setWindowPosFrameChangeMove, null);
      }
      if (showWindowNormalProfile) {
        const show = evidence.report.showWindowNormal;
        assert(
          show !== null,
          "Chrome Worker omitted ShowWindow(SW_SHOWNORMAL) evidence",
        );
        assert.equal(show.callCount, 1);
        assert.equal(show.call.sequence, 127_351);
        assert.equal(show.call.module.toLowerCase(), "user32.dll");
        assert.equal(show.call.symbol, "ShowWindow");
        assert.equal(show.call.occurrence, 1);
        assert.equal(show.call.trapInstructionCount, 28_761_027);
        assert.deepEqual(show.call.arguments, [0x4801_0001, 1]);
        assert.deepEqual(show.call.result, { action: "set", value: 1 });
        assert.equal(
          show.call.implementationId,
          "user32.show-window.visible-normal-noop.v1",
        );
        assert.equal(show.call.returnAddress, 0x1009_38da);
        assert.equal(show.call.stackPointerBefore, 0x006f_f684);
        assert.equal(show.call.stackPointerAfter, 0x006f_f690);
        assert.equal(show.call.lastError, 0);
        assert.deepEqual(show.call.guestWrites, []);
        assert.equal(
          show.windowAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(
          show.relationAuthority,
          "rust-runtime-user32-thread-and-process-window-relations",
        );
        assert.equal(show.mutation, false);
        assert.equal(show.callbackCount, 0);
        assert.equal(show.browserResourceCreated, false);
      } else {
        assert.equal(evidence.report.showWindowNormal, null);
      }
      if (updateWindowProfile) {
        const update = evidence.report.updateWindow;
        assert(
          update !== null,
          "Chrome Worker omitted UpdateWindow paint lifecycle evidence",
        );
        assert.equal(update.callCount, 1);
        assert.deepEqual(update.call, {
          sequence: 127_352,
          module: "USER32.dll",
          symbol: "UpdateWindow",
          occurrence: 1,
          trapInstructionCount: 28_761_031,
          arguments: [0x4801_0001],
          pendingCallback: true,
        });
        assert.deepEqual(update.callbackEvents, [
          {
            action: "enter",
            sequence: 127_352,
            frameId: 30,
            instructionCount: 28_761_031,
            target: 0x1009_2550,
            stackPointer: 0x006f_f674,
            arguments: [0x4801_0001, 0x000f, 0, 0],
          },
        ]);
        assert.deepEqual(
          {
            sequence: update.beginPaint.sequence,
            module: update.beginPaint.module.toLowerCase(),
            symbol: update.beginPaint.symbol,
            occurrence: update.beginPaint.occurrence,
            trapInstructionCount: update.beginPaint.trapInstructionCount,
            arguments: update.beginPaint.arguments,
            result: update.beginPaint.result,
            implementationId: update.beginPaint.implementationId,
            returnAddress: update.beginPaint.returnAddress,
            stackPointerBefore: update.beginPaint.stackPointerBefore,
            stackPointerAfter: update.beginPaint.stackPointerAfter,
            lastError: update.beginPaint.lastError,
          },
          {
            sequence: 127_353,
            module: "user32.dll",
            symbol: "BeginPaint",
            occurrence: 1,
            trapInstructionCount: 28_761_068,
            arguments: [0x4801_0001, 0x006f_f318],
            result: { action: "set", value: 0x4701_0009 },
            implementationId:
              "user32.begin-paint.full-client-logical-dc.v1",
            returnAddress: 0x1009_28d9,
            stackPointerBefore: 0x006f_f2cc,
            stackPointerAfter: 0x006f_f2d8,
            lastError: 0,
          },
        );
        const paintStructBytes = expectedUpdateWindowPaintStructBytes();
        assert.deepEqual(update.beginPaint.guestWrites, [
          { address: 0x006f_f318, bytes: paintStructBytes },
        ]);
        assert.equal(update.beginPaint.paint.action, "begun");
        assert.equal(update.beginPaint.paint.window.id, 1);
        assert.equal(update.beginPaint.paint.window.handle, 0x4801_0001);
        assert.equal(update.beginPaint.paint.window.paintGeneration, 0);
        assert.equal(
          update.beginPaint.paint.window.pendingUpdateRect,
          undefined,
        );
        assert.deepEqual(update.beginPaint.paint.session.updateRect, {
          left: 0,
          top: 0,
          right: 1_280,
          bottom: 720,
        });
        assert.equal(update.beginPaint.paint.session.paintGeneration, 1);
        assert.equal(
          update.beginPaint.paint.session.deviceContextHandle,
          0x4701_0009,
        );
        assert.equal(
          update.beginPaint.paint.session.paintStructAddress,
          0x006f_f318,
        );
        assert.deepEqual(
          update.beginPaint.paint.session.paintStructBytes,
          paintStructBytes,
        );
        assert.equal(update.beginPaint.paint.deviceContext.id, 9);
        assert.equal(
          update.beginPaint.paint.deviceContext.handle,
          0x4701_0009,
        );
        assert.equal(update.beginPaint.paint.livePaintSessions, 1);
        assert.equal(update.beginPaint.paint.liveDeviceContexts, 1);
        assert.deepEqual(
          {
            sequence: update.endPaint.sequence,
            module: update.endPaint.module.toLowerCase(),
            symbol: update.endPaint.symbol,
            occurrence: update.endPaint.occurrence,
            trapInstructionCount: update.endPaint.trapInstructionCount,
            arguments: update.endPaint.arguments,
            result: update.endPaint.result,
            implementationId: update.endPaint.implementationId,
            returnAddress: update.endPaint.returnAddress,
            stackPointerBefore: update.endPaint.stackPointerBefore,
            stackPointerAfter: update.endPaint.stackPointerAfter,
            lastError: update.endPaint.lastError,
            guestWrites: update.endPaint.guestWrites,
          },
          {
            sequence: 127_354,
            module: "user32.dll",
            symbol: "EndPaint",
            occurrence: 1,
            trapInstructionCount: 28_761_073,
            arguments: [0x4801_0001, 0x006f_f318],
            result: { action: "set", value: 1 },
            implementationId: "user32.end-paint.matching-session.v1",
            returnAddress: 0x1009_28e5,
            stackPointerBefore: 0x006f_f2cc,
            stackPointerAfter: 0x006f_f2d8,
            lastError: 0,
            guestWrites: [],
          },
        );
        assert.equal(update.endPaint.paint.action, "ended");
        assert.equal(update.endPaint.paint.window.paintGeneration, 1);
        assert.equal(update.endPaint.paint.paintStructAddress, 0x006f_f318);
        assert.equal(update.endPaint.paint.livePaintSessions, 0);
        assert.equal(update.endPaint.paint.liveDeviceContexts, 0);
        assert.equal(
          update.windowAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(
          update.updateRegionAuthority,
          "rust-runtime-user32-window-update-region",
        );
        assert.equal(
          update.updateRegionSource,
          "set-window-pos-frame-change-nccalcsize-completion",
        );
        assert.equal(
          update.updateRegionConsumption,
          "begin-paint-targeted-commit",
        );
        assert.equal(
          update.paintSessionAuthority,
          "rust-runtime-user32-paint-session",
        );
        assert.equal(
          update.deviceContextAuthority,
          "rust-runtime-gdi-device-context",
        );
        assert.equal(update.paintSessionRetired, true);
        assert.equal(update.deviceContextRetired, true);
        assert.equal(update.failureAtomicity, true);
        assert.equal(update.callbackActiveAtStop, true);
        assert.equal(update.browserResourceCreated, false);
      } else {
        assert.equal(evidence.report.updateWindow, null);
      }
      if (d3d9ClearProfile) {
        const clear = evidence.report.d3d9Clear;
        assert(clear !== null, "Chrome Worker omitted D3D9 Clear evidence");
        assert.deepEqual(clear.call.arguments, [
          0x0101_0004,
          0,
          0,
          1,
          0xff00_0000,
          0x3f80_0000,
          0,
        ]);
        assert.deepEqual(clear.call.result, { kind: "h-result", value: 0 });
        assert.equal(clear.call.sequence, 127_355);
        assert.equal(clear.call.symbol, "IDirect3DDevice9.Clear");
        assert.equal(clear.call.interface, "IDirect3DDevice9");
        assert.equal(clear.call.vtableSlot, 43);
        assert.equal(clear.call.returnAddress, 0x1005_af9e);
        assert.equal(clear.call.referenceCount, 5);
        assert.equal(clear.call.objectRetired, false);
        assert.deepEqual(clear.call.guestWrites, []);
        assert.deepEqual(clear.call.graphicsDevice?.presentation, {
          width: 1_280,
          height: 720,
          colorFormat: "b8g8r8x8-unorm",
          backBufferCount: 1,
          sampleCount: 1,
          presentMode: "discard",
          windowed: true,
        });
        assert.deepEqual(clear.call.graphicsDevice?.backBuffer, {
          contentGeneration: 1,
          uniformClear: {
            sequence: 1,
            rect: { left: 0, top: 0, right: 1_280, bottom: 720 },
            colorArgb: 0xff00_0000,
          },
        });
        assert.equal(clear.call.graphicsDevice?.renderer, undefined);
        assert.equal(
          clear.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(clear.clearProfileId, "primary-full-render-target-black-v1");
        assert.equal(
          clear.targetAuthority,
          "rust-runtime-graphics-logical-device-back-buffer",
        );
        assert.equal(clear.targetScope, "full-presentation-back-buffer");
        assert.equal(clear.mutation, "validate-plan-preflight-targeted-commit");
        assert.equal(clear.failureAtomicity, true);
        assert.equal(clear.depthStencilCleared, false);
        assert.equal(clear.partialRectsImplemented, false);
        assert.equal(clear.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9Clear, null);
      }
      if (setRectProfile) {
        const setRect = evidence.report.setRect;
        assert(setRect !== null, "Chrome Worker omitted SetRect evidence");
        assert.equal(setRect.call.sequence, 127_356);
        assert.equal(setRect.call.module.toLowerCase(), "user32.dll");
        assert.equal(setRect.call.symbol, "SetRect");
        assert.deepEqual(setRect.call.arguments, [
          0x006f_f29c,
          0,
          0,
          1_280,
          720,
        ]);
        assert.deepEqual(setRect.call.result, { action: "set", value: 1 });
        assert.equal(setRect.call.implementationId, "user32.set-rect.v1");
        assert.equal(setRect.call.returnAddress, 0x1005_b00b);
        assert.equal(setRect.call.lastError, 0);
        assert.deepEqual(setRect.call.guestWrites, [
          {
            address: 0x006f_f29c,
            bytes: [
              0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 208, 2, 0, 0,
            ],
          },
        ]);
        assert.equal(setRect.implementationId, "user32.set-rect.v1");
        assert.equal(setRect.outputType, "RECT");
        assert.equal(setRect.coordinateEncoding, "signed-long32");
        assert.equal(setRect.lastErrorPolicy, "preserve");
        assert.equal(setRect.mutation, "guest-memory-preflight-atomic-write");
        assert.equal(setRect.nullOutputReturnsFalse, true);
        assert.equal(setRect.browserOwnsRectState, false);
      } else {
        assert.equal(evidence.report.setRect, null);
      }
      if (d3d9AddDirtyRectProfile) {
        const dirty = evidence.report.d3d9AddDirtyRect;
        assert(
          dirty !== null,
          "Chrome Worker omitted D3D9 AddDirtyRect evidence",
        );
        assert.equal(dirty.call.sequence, 127_357);
        assert.equal(dirty.call.symbol, "IDirect3DTexture9.AddDirtyRect");
        assert.equal(dirty.call.interface, "IDirect3DTexture9");
        assert.equal(dirty.call.vtableSlot, 21);
        assert.deepEqual(dirty.call.arguments, [0x0102_0004, 0x006f_f29c]);
        assert.deepEqual(dirty.call.result, { kind: "h-result", value: 0 });
        assert.equal(dirty.call.returnAddress, 0x1005_b01d);
        assert.equal(dirty.call.referenceCount, 1);
        assert.equal(dirty.call.objectRetired, false);
        assert.deepEqual(dirty.call.guestWrites, []);
        assert.deepEqual(dirty.call.graphicsTexture?.descriptor, {
          width: 2_048,
          height: 1_024,
          levels: 1,
          usage: "none",
          colorFormat: "b8g8r8x8-unorm",
          pool: "system-memory",
        });
        assert.equal(dirty.call.graphicsTexture?.comObjectId, "22");
        assert.deepEqual(dirty.call.graphicsTexture?.dirtyRegion, {
          kind: "whole-texture",
        });
        assert.equal(dirty.call.graphicsTexture?.content?.contentGeneration, 1);
        assert.equal(dirty.call.graphicsTexture?.content?.fills.length, 1);
        assert.equal(dirty.call.graphicsTexture?.content?.fills[0]?.sequence, 3);
        assert.equal(dirty.call.graphicsTexture?.generation, 41);
        assert.equal(
          dirty.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          dirty.addDirtyRectProfileId,
          "primary-recreated-system-memory-texture-dirty-rect-v1",
        );
        assert.equal(
          dirty.textureAuthority,
          "rust-runtime-graphics-logical-texture",
        );
        assert.equal(dirty.initialDirtyRegion, "whole-texture");
        assert.equal(
          dirty.mutation,
          "validate-plan-preflight-idempotent-completion",
        );
        assert.equal(dirty.textureGenerationChanged, false);
        assert.equal(dirty.failureAtomicity, true);
        assert.equal(dirty.dirtyRegionConsumed, false);
        assert.equal(dirty.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9AddDirtyRect, null);
      }
      if (d3d9UpdateTextureProfile) {
        const update = evidence.report.d3d9UpdateTexture;
        assert(
          update !== null,
          "Chrome Worker omitted D3D9 UpdateTexture evidence",
        );
        assert.equal(update.call.sequence, 127_358);
        assert.equal(update.call.symbol, "IDirect3DDevice9.UpdateTexture");
        assert.equal(update.call.interface, "IDirect3DDevice9");
        assert.equal(update.call.vtableSlot, 31);
        assert.deepEqual(update.call.arguments, [
          0x0101_0004,
          0x0102_0004,
          0x0103_0004,
        ]);
        assert.deepEqual(update.call.result, { kind: "h-result", value: 0 });
        assert.equal(update.call.returnAddress, 0x1005_b038);
        assert.equal(update.call.stackPointerBefore, 0x006f_f258);
        assert.equal(update.call.stackPointerAfter, 0x006f_f268);
        assert.equal(update.call.referenceCount, 5);
        assert.equal(update.call.objectRetired, false);
        assert.deepEqual(update.call.guestWrites, []);
        const pair = update.call.graphicsTextureUpdate;
        assert(pair !== undefined, "UpdateTexture omitted its texture pair");
        assert.equal(pair.source.id, 7);
        assert.equal(pair.source.comObjectId, "22");
        assert.equal(pair.source.descriptor.pool, "system-memory");
        assert.deepEqual(pair.source.dirtyRegion, { kind: "clean" });
        assert.equal(pair.source.content?.contentGeneration, 1);
        assert.equal(pair.source.content?.fills[0]?.sequence, 3);
        assert.equal(pair.source.generation, 42);
        assert.equal(pair.destination.id, 8);
        assert.equal(pair.destination.comObjectId, "24");
        assert.equal(pair.destination.descriptor.pool, "default");
        assert.deepEqual(pair.destination.dirtyRegion, {
          kind: "whole-texture",
        });
        assert.deepEqual(pair.destination.content?.fills, pair.source.content?.fills);
        assert.deepEqual(pair.destination.content?.lastUpdate, {
          sequence: 1,
          sourceTextureId: 7,
          sourceContentGeneration: 1,
          dirtyRegion: { kind: "whole-texture" },
        });
        assert.equal(pair.destination.generation, 45);
        assert.equal(
          update.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          update.updateTextureProfileId,
          "primary-recreated-system-memory-to-static-default-texture-update-v1",
        );
        assert.equal(
          update.contentAuthority,
          "rust-runtime-graphics-logical-texture",
        );
        assert.equal(update.sourceDirtyRegionBefore, "whole-texture");
        assert.equal(update.sourceDirtyRegionAfter, "clean");
        assert.equal(update.dirtyRegionConsumed, true);
        assert.equal(
          update.transferScope,
          "whole-texture-level-zero-logical-content",
        );
        assert.equal(update.mutation, "dual-texture-preflight-targeted-commit");
        assert.equal(update.failureAtomicity, true);
        assert.equal(update.partialDirtyRegionsImplemented, false);
        assert.equal(update.dynamicDestinationImplemented, false);
        assert.equal(update.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9UpdateTexture, null);
      }
      if (d3d9BeginSceneProfile) {
        const begin = evidence.report.d3d9BeginScene;
        assert(begin !== null, "Chrome Worker omitted D3D9 BeginScene evidence");
        assert.equal(begin.call.sequence, 127_359);
        assert.equal(begin.call.symbol, "IDirect3DDevice9.BeginScene");
        assert.equal(begin.call.interface, "IDirect3DDevice9");
        assert.equal(begin.call.vtableSlot, 41);
        assert.deepEqual(begin.call.arguments, [0x0101_0004]);
        assert.deepEqual(begin.call.result, { kind: "h-result", value: 0 });
        assert.equal(begin.call.returnAddress, 0x1005_b048);
        assert.equal(begin.call.stackPointerBefore, 0x006f_f260);
        assert.equal(begin.call.stackPointerAfter, 0x006f_f268);
        assert.equal(begin.call.referenceCount, 5);
        assert.equal(begin.call.objectRetired, false);
        assert.deepEqual(begin.call.guestWrites, []);
        assert.equal(begin.call.graphicsDevice?.id, 3);
        assert.equal(begin.call.graphicsDevice?.comObjectId, "21");
        assert.deepEqual(begin.call.graphicsDevice?.activeScene, { sequence: 1 });
        assert.equal(begin.call.graphicsDevice?.generation, 45);
        assert.equal(begin.call.graphicsDevice?.backBuffer.contentGeneration, 1);
        assert.equal(begin.call.graphicsDevice?.backBuffer.uniformClear?.sequence, 1);
        assert.equal(begin.call.graphicsDevice?.renderer, undefined);
        assert.equal(
          begin.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          begin.beginSceneProfileId,
          "primary-first-frame-scene-begin-v1",
        );
        assert.equal(
          begin.sceneAuthority,
          "rust-runtime-graphics-logical-device",
        );
        assert.equal(begin.lifecycle, "inactive-to-active");
        assert.equal(begin.mutation, "validate-plan-preflight-targeted-commit");
        assert.equal(begin.nestedBeginReturnsInvalidCall, true);
        assert.equal(begin.failureAtomicity, true);
        assert.equal(begin.endSceneImplemented, false);
        assert.equal(begin.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9BeginScene, null);
      }
      if (d3d9InitialSamplerStatesProfile) {
        const sampler = evidence.report.d3d9InitialSamplerStates;
        assert(
          sampler !== null,
          "Chrome Worker omitted D3D9 SetSamplerState evidence",
        );
        assert.deepEqual(
          sampler.calls.map((call) => [
            call.sequence,
            call.symbol,
            call.vtableSlot,
            call.arguments,
            call.returnAddress,
          ]),
          [
            [
              127_360,
              "IDirect3DDevice9.SetSamplerState",
              69,
              [0x0101_0004, 0, 6, 2],
              0x1005_b18a,
            ],
            [
              127_361,
              "IDirect3DDevice9.SetSamplerState",
              69,
              [0x0101_0004, 0, 5, 2],
              0x1005_b1a0,
            ],
          ],
        );
        for (const [index, call] of sampler.calls.entries()) {
          assert.equal(call.interface, "IDirect3DDevice9");
          assert.deepEqual(call.result, { kind: "h-result", value: 0 });
          assert.equal(call.stackPointerBefore, 0x006f_f254);
          assert.equal(call.stackPointerAfter, 0x006f_f268);
          assert.equal(call.referenceCount, 5);
          assert.equal(call.objectRetired, false);
          assert.deepEqual(call.guestWrites, []);
          assert.equal(
            call.graphicsDevice?.samplerStates?.samplerZero.minFilter,
            "linear",
          );
          assert.equal(
            call.graphicsDevice?.samplerStates?.samplerZero.magFilter,
            index === 0 ? undefined : "linear",
          );
          assert.equal(call.graphicsDevice?.generation, 46 + index);
          assert.equal(call.graphicsDevice?.renderer, undefined);
        }
        assert.deepEqual(
          sampler.postSamplerRenderStateRejections.map((call) => [
            call.sequence,
            call.symbol,
            call.vtableSlot,
            call.arguments,
            call.result,
            call.returnAddress,
          ]),
          [
            [
              127_362,
              "IDirect3DDevice9.SetRenderState",
              57,
              [0x0101_0004, 171, 1],
              { kind: "h-result", value: -2_005_530_518 },
              0x1005_b1b7,
            ],
            [
              127_363,
              "IDirect3DDevice9.SetRenderState",
              57,
              [0x0101_0004, 20, 6],
              { kind: "h-result", value: -2_005_530_518 },
              0x1005_b1cb,
            ],
            [
              127_364,
              "IDirect3DDevice9.SetRenderState",
              57,
              [0x0101_0004, 19, 5],
              { kind: "h-result", value: -2_005_530_518 },
              0x1005_b1df,
            ],
          ],
        );
        assert.equal(
          sampler.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          sampler.samplerStateProfileId,
          "primary-first-frame-linear-sampler-states-v1",
        );
        assert.equal(
          sampler.stateAuthority,
          "rust-runtime-graphics-logical-device-sampler-states",
        );
        assert.equal(
          sampler.mutation,
          "validate-plan-preflight-targeted-idempotent-commit",
        );
        assert.equal(sampler.sameValueGenerationChange, 0);
        assert.equal(sampler.unsupportedVariantResult, "0x8876086a");
        assert.equal(sampler.failureAtomicity, true);
        assert.equal(sampler.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9InitialSamplerStates, null);
      }
      if (d3d9InitialTextureStageStatesProfile) {
        const textureStage = evidence.report.d3d9InitialTextureStageStates;
        assert(
          textureStage !== null,
          "Chrome Worker omitted D3D9 SetTextureStageState evidence",
        );
        const expectedStates = [1, 2, 3, 4, 5, 6];
        const expectedValues = [4, 2, 0, 2, 2, 0];
        const expectedReturns = [
          0x1005_b1f5, 0x1005_b20b, 0x1005_b221, 0x1005_b237,
          0x1005_b24d, 0x1005_b263,
        ];
        assert.equal(textureStage.calls.length, 6);
        for (const [index, call] of textureStage.calls.entries()) {
          assert.equal(call.sequence, 127_365 + index);
          assert.equal(call.symbol, "IDirect3DDevice9.SetTextureStageState");
          assert.equal(call.interface, "IDirect3DDevice9");
          assert.equal(call.vtableSlot, 67);
          assert.deepEqual(call.arguments, [
            0x0101_0004,
            0,
            expectedStates[index],
            expectedValues[index],
          ]);
          assert.deepEqual(call.result, { kind: "h-result", value: 0 });
          assert.equal(call.returnAddress, expectedReturns[index]);
          assert.equal(call.stackPointerBefore, 0x006f_f254);
          assert.equal(call.stackPointerAfter, 0x006f_f268);
          assert.equal(call.referenceCount, 5);
          assert.equal(call.objectRetired, false);
          assert.deepEqual(call.guestWrites, []);
          assert.deepEqual(call.graphicsDevice?.samplerStates, {
            samplerZero: { minFilter: "linear", magFilter: "linear" },
          });
          assert.deepEqual(call.graphicsDevice?.textureStageStates, {
            stageZero: {
              colorOperation: "modulate",
              ...(index >= 1 ? { colorArgumentOne: "texture" } : {}),
              ...(index >= 2 ? { colorArgumentTwo: "diffuse" } : {}),
              ...(index >= 3
                ? { alphaOperation: "select-argument-one" }
                : {}),
              ...(index >= 4 ? { alphaArgumentOne: "texture" } : {}),
              ...(index >= 5 ? { alphaArgumentTwo: "diffuse" } : {}),
            },
          });
          assert.equal(call.graphicsDevice?.generation, 48 + index);
          assert.equal(call.graphicsDevice?.renderer, undefined);
        }
        assert.equal(
          textureStage.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          textureStage.textureStageStateProfileId,
          "primary-first-frame-fixed-function-texture-stage-zero-v1",
        );
        assert.equal(
          textureStage.stateAuthority,
          "rust-runtime-graphics-logical-device-texture-stage-states",
        );
        assert.equal(
          textureStage.mutation,
          "validate-plan-preflight-targeted-idempotent-commit",
        );
        assert.equal(textureStage.sameValueGenerationChange, 0);
        assert.equal(textureStage.unsupportedVariantResult, "0x8876086a");
        assert.equal(textureStage.failureAtomicity, true);
        assert.equal(textureStage.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9InitialTextureStageStates, null);
      }
      if (d3d9RecreatedTexturesProfile) {
        const recreated = evidence.report.d3d9RecreatedTextures;
        assert(
          recreated !== null,
          "Chrome Worker omitted D3D9 recreated texture evidence",
        );
        assert.equal(
          recreated.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v23",
        );
        assert.equal(
          recreated.createTextureProfileId,
          "primary-initial-and-recreated-x8r8g8b8-textures-v2",
        );
        assert.equal(
          recreated.getSurfaceLevelProfileId,
          "primary-system-memory-texture-level-zero-surfaces-v2",
        );
        assert.deepEqual(
          recreated.calls.map((call) => [call.sequence, call.symbol]),
          [
            [108_735, "IDirect3DDevice9.CreateTexture"],
            [108_736, "IDirect3DTexture9.GetSurfaceLevel"],
            [108_737, "IDirect3DSurface9.GetDC"],
            [108_738, "GetStockObject"],
            [108_739, "FillRect"],
            [108_740, "IDirect3DSurface9.ReleaseDC"],
            [108_741, "IDirect3DSurface9.Release"],
            [108_742, "IDirect3DDevice9.CreateTexture"],
            [108_743, "IDirect3DDevice9.CreateTexture"],
          ],
        );
        const textureCalls = recreated.calls.filter(
          (call) => call.symbol === "IDirect3DDevice9.CreateTexture",
        );
        assert.deepEqual(
          textureCalls.map((call) => [
            call.arguments.slice(1),
            call.result,
            call.referenceCount,
            call.guestWrites,
          ]),
          [
            [
              [2_048, 1_024, 1, 0, 22, 2, 0x1015_17cc, 0],
              { kind: "h-result", value: 0 },
              1,
              [{ address: 0x1015_17cc, bytes: [4, 0, 2, 1] }],
            ],
            [
              [2_048, 1_024, 1, 0, 22, 0, 0x1015_17d0, 0],
              { kind: "h-result", value: 0 },
              2,
              [{ address: 0x1015_17d0, bytes: [4, 0, 3, 1] }],
            ],
            [
              [2_048, 1_024, 1, 0x200, 22, 0, 0x1015_17d4, 0],
              { kind: "h-result", value: 0 },
              3,
              [{ address: 0x1015_17d4, bytes: [4, 0, 4, 1] }],
            ],
          ],
        );
        assert.equal(
          recreated.textureAuthority,
          "rust-runtime-graphics-logical-texture",
        );
        assert.equal(
          recreated.comAuthority,
          "rust-runtime-com-object-and-interface",
        );
        assert.equal(recreated.browserRendererResourceCreated, false);
        assert.equal(
          recreated.mutation,
          "cross-owner-preflight-transactional-commit",
        );
      } else {
        assert.equal(evidence.report.d3d9RecreatedTextures, null);
      }
      assert(evidence.report.maximumSliceMilliseconds > 0);
      assert(evidence.report.wasmBridgeCalls > 0);
      assert(
        evidence.heartbeatSamples > 0,
        "main heartbeat did not advance during BGI",
      );
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

async function waitForSourceCheckpoint(client, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  let snapshot = null;
  while (Date.now() < deadline) {
    snapshot = await evaluate(client, `window.__vineSourceCheck`);
    if (snapshot?.error !== null && snapshot?.error !== undefined) {
      throw new Error(
        `source gateway browser runtime failed: ${snapshot.error}`,
      );
    }
    if (snapshot?.result !== null && snapshot?.result !== undefined) {
      return snapshot.result;
    }
    await wait(50);
  }
  throw new Error(
    `source gateway browser runtime timed out: ${JSON.stringify(snapshot)}`,
  );
}

async function runSourceProbe(browserWebSocketUrl, baseUrl, scenario) {
  const target = await createDebugTarget(
    browserWebSocketUrl,
    `${baseUrl}source-check.html?scenario=${scenario}`,
  );
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await client.send("Runtime.enable");
    await client.send("Page.enable");
    const evidence = await waitForSourceCheckpoint(client);
    assert.equal(evidence.scenario, scenario);
    assert.equal(evidence.report.pendingHostCalls, 0);
    assert.equal(evidence.report.pendingCallbacks, 0);
    assertExecutionStateMachine(evidence.report, scenario);
    assertExecutionEngine(evidence.report);
    assert(
      evidence.statuses.some(({ phase }) => phase === "waiting-host"),
      `${scenario} source status history has no waiting-host phase: ${JSON.stringify(evidence.statuses)}`,
    );
    if (scenario === "success") {
      assert.equal(evidence.report.outcome, "completed");
      assert.equal(evidence.report.exitCode, 0);
      assert.equal(evidence.report.instructions, 217);
      assert.equal(evidence.report.hostRequests, 2);
      assert.equal(evidence.report.rustSchedulerTransitions, 2);
      assert.deepEqual(evidence.report.sourceReads, [
        {
          request: {
            requestId: 1,
            fileId: 1,
            sourceId: 1,
            objectId: 1,
            offset: 0,
            requestedBytes: 8,
          },
          succeeded: true,
          transferredBytes: 8,
          win32Error: 0,
          cursorBefore: 0,
          cursorAfter: 8,
          contentSha256:
            "202034b246570abc8e0e549637dbeb3ce7990de1fedeef6bcbf795934a3bb2da",
        },
        {
          request: {
            requestId: 2,
            fileId: 1,
            sourceId: 1,
            objectId: 1,
            offset: 50,
            requestedBytes: 11,
          },
          succeeded: true,
          transferredBytes: 11,
          win32Error: 0,
          cursorBefore: 50,
          cursorAfter: 61,
          contentSha256:
            "71c02fe83d1ee0855a1875c47a53372209e4892b3b8bd7bcdc5d1ea9610d0dc3",
        },
      ]);
    } else if (scenario === "wrong-origin") {
      assert.equal(evidence.report.outcome, "fault");
      assert.equal(evidence.report.hostRequests, 1);
      assert.equal(evidence.report.instructions, 32);
      assert.equal(evidence.report.rustSchedulerTransitions, 0);
      assert.deepEqual(evidence.report.sourceReads, []);
      assert.equal(
        evidence.report.error,
        "source API URL must use the Runtime Worker origin",
      );
    } else {
      assert.equal(
        evidence.report.outcome,
        scenario === "watchdog" ? "watchdog" : "cancelled",
      );
      assert.equal(evidence.report.hostRequests, 1);
      assert.equal(evidence.report.instructions, 32);
      assert.equal(evidence.report.rustSchedulerTransitions, 0);
      assert.deepEqual(evidence.report.sourceReads, []);
      if (scenario === "watchdog") {
        assert.equal(evidence.report.watchdogContext.requestId, "1");
        assert.equal(
          evidence.report.watchdogContext.lastTransition?.to,
          "waiting-host",
        );
      }
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
  candidateWorkload = null,
) {
  const searchParameters = new URLSearchParams({
    durationMs: String(durationMilliseconds),
    sampleIntervalMs: String(sampleIntervalMilliseconds),
  });
  if (candidateWorkload !== null) {
    for (const field of [
      "profile",
      "guestInstructions",
      "supportedCalls",
      "sourceReadCompletions",
      "nextApi",
      "nextFaultCode",
      "callSemanticsAlgorithm",
      "callSemanticsSha256",
      "memoryContentSha256",
      "wasmBridgeCalls",
    ]) {
      assert(
        candidateWorkload[field] !== null &&
          candidateWorkload[field] !== undefined,
        `candidate benchmark workload is missing ${field}`,
      );
      searchParameters.set(field, String(candidateWorkload[field]));
    }
  }
  const benchmarkUrl = `${baseUrl}bgi-benchmark.html?${searchParameters}`;
  const target = await createDebugTarget(browserWebSocketUrl, "about:blank");
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  let workerSessionId = null;
  let workerCpuProfileActive = false;
  const workerHeapSamples = [];
  try {
    client.on("Target.attachedToTarget", ({ sessionId, targetInfo }) => {
      if (
        targetInfo.type === "worker" &&
        targetInfo.url.endsWith("/runtime-worker.js")
      ) {
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
    assert(
      workerSessionId !== null,
      "BGI benchmark Worker target was not discoverable",
    );
    if (bgiCpuProfile) {
      await client.sendToSession(workerSessionId, "Profiler.enable");
      await client.sendToSession(
        workerSessionId,
        "Profiler.setSamplingInterval",
        {
          interval: 100,
        },
      );
      await client.sendToSession(workerSessionId, "Profiler.start");
      workerCpuProfileActive = true;
    }
    const deadline = Date.now() + durationMilliseconds + 60_000;
    let nextHeapSampleAt = 0;
    let snapshot = null;
    while (Date.now() < deadline) {
      snapshot = await evaluate(
        client,
        `window.__vineBgiBenchmark?.snapshot()`,
      );
      if (snapshot?.status === "failed") {
        throw new Error(`BGI benchmark failed: ${snapshot.error}`);
      }
      if (workerCpuProfileActive && snapshot?.completedSessions >= 1) {
        const { profile } = await client.sendToSession(
          workerSessionId,
          "Profiler.stop",
          {},
          30_000,
        );
        workerCpuProfileActive = false;
        await writeFile(
          path.join(assetsRoot, "bgi-browser-cpu-profile.json"),
          `${JSON.stringify(profile)}\n`,
        );
        await client.sendToSession(workerSessionId, "Profiler.disable");
      }
      if (snapshot?.status === "passed") {
        assert(
          snapshot.result.completedSessions > 0,
          "BGI benchmark completed no sessions",
        );
        assert.equal(
          snapshot.result.failedSessions,
          0,
          "BGI benchmark contained failures",
        );
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
    if (workerSessionId !== null && workerCpuProfileActive) {
      try {
        await client.sendToSession(
          workerSessionId,
          "Profiler.stop",
          {},
          30_000,
        );
      } catch {
        // The benchmark Worker may have terminated before a complete session.
      }
    }
    if (workerSessionId !== null) {
      try {
        await client.send("Target.detachFromTarget", {
          sessionId: workerSessionId,
        });
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
  const index = Math.min(
    sorted.length - 1,
    Math.ceil(percentileValue * sorted.length) - 1,
  );
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
  const denominator = points.reduce(
    (sum, point) => sum + (point.x - meanX) ** 2,
    0,
  );
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
  const sessionDurations = result.sessions.map(
    ({ elapsedMilliseconds }) => elapsedMilliseconds,
  );
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
  const workerEmbedderHeap = heapMetrics(
    result.workerHeapSamples,
    "embedderHeapUsedSize",
  );
  const workerBackingStorage = heapMetrics(
    result.workerHeapSamples,
    "backingStorageSize",
  );
  const runtimeCoreSizes = new Set(
    result.sessions.map(
      ({ runtimeCoreLinearMemoryBytes }) => runtimeCoreLinearMemoryBytes,
    ),
  );
  const engineSizes = new Set(
    result.sessions.map(
      ({ executionEngineLinearMemoryBytes }) =>
        executionEngineLinearMemoryBytes,
    ),
  );
  const wasmBridgeCallCounts = new Set(
    result.sessions.map(({ wasmBridgeCalls }) => wasmBridgeCalls),
  );
  assert.equal(
    runtimeCoreSizes.size,
    1,
    "Rust runtime linear memory size was unstable",
  );
  assert.equal(
    engineSizes.size,
    1,
    "execution engine linear memory size was unstable",
  );
  assert.equal(
    wasmBridgeCallCounts.size,
    1,
    "Wasm crossing count was unstable",
  );
  const metrics = {
    durationMilliseconds: result.durationMilliseconds,
    completedSessions: result.completedSessions,
    failedSessions: result.failedSessions,
    sessionsPerMinute:
      (result.completedSessions * 60_000) / result.durationMilliseconds,
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
    thresholdEvaluations.push({
      name,
      observed,
      comparison,
      threshold,
      passed,
    });
    if (!passed) {
      thresholdFailures.push(name);
    }
  }
}

function normalizeCurrentDirectorySnapshot(snapshot) {
  assert(
    snapshot !== null && snapshot !== undefined,
    "current-directory snapshot is missing",
  );
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

function normalizeHeapPageMaterialization(materialization) {
  return {
    instructionCount: materialization.instructionCount,
    handle: materialization.handle,
    allocation: materialization.allocation,
    memoryOwner: materialization.memoryOwner,
    addressAllocationId: materialization.addressAllocationId,
    faultAddress: materialization.faultAddress,
    pageFaultErrorCode: materialization.pageFaultErrorCode,
    committedPage: materialization.committedPage,
    materializedPages: materialization.materializedPages,
  };
}

function normalizeDragAcceptFiles(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    ...snapshot,
    call: {
      ...snapshot.call,
      module: snapshot.call.module.toLowerCase(),
    },
  };
}

function normalizeSleepZero(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const normalizeCall = (call) => ({
    sequence: call.sequence,
    module: call.module.toLowerCase(),
    symbol: call.symbol,
    occurrence: call.occurrence,
    trapInstructionCount: call.trapInstructionCount,
    arguments: call.arguments,
    result: call.result,
    implementationId: call.implementationId,
    returnAddress: call.returnAddress,
    stackPointerBefore: call.stackPointerBefore,
    stackPointerAfter: call.stackPointerAfter,
    lastError: call.lastError,
    guestWrites: call.guestWrites,
    executionBoundary: call.executionBoundary,
  });
  const normalizeSelection = (selection) =>
    selection === null || selection === undefined
      ? null
      : {
          sequence: selection.sequence,
          instructionCount: selection.instructionCount,
          previousThreadId: selection.previousThreadId,
          threadId: selection.threadId,
          state: selection.state,
          executionBoundary: selection.executionBoundary,
        };
  return {
    callCount: snapshot.callCount,
    firstCall: normalizeCall(snapshot.firstCall),
    lastCall: normalizeCall(snapshot.lastCall),
    executionBoundary: snapshot.executionBoundary,
    schedulerAuthority: snapshot.schedulerAuthority,
    schedulerSelections: {
      selectionCount: snapshot.schedulerSelections.selectionCount,
      firstSelection: normalizeSelection(
        snapshot.schedulerSelections.firstSelection,
      ),
      lastSelection: normalizeSelection(
        snapshot.schedulerSelections.lastSelection,
      ),
    },
    noCompetitorCompletions: snapshot.noCompetitorCompletions,
    hostWaitCreated: snapshot.hostWaitCreated,
  };
}

function normalizeGlobalMemoryStatus(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      sequence: snapshot.call.sequence,
      module: snapshot.call.module.toLowerCase(),
      symbol: snapshot.call.symbol,
      occurrence: snapshot.call.occurrence,
      trapInstructionCount: snapshot.call.trapInstructionCount,
      arguments: snapshot.call.arguments,
      result: snapshot.call.result,
      implementationId: snapshot.call.implementationId,
      returnAddress: snapshot.call.returnAddress,
      stackPointerBefore: snapshot.call.stackPointerBefore,
      stackPointerAfter: snapshot.call.stackPointerAfter,
      lastError: snapshot.call.lastError,
      guestWrites: snapshot.call.guestWrites.map(({ address, bytes }) => ({
        address,
        bytes,
      })),
    },
    stateAuthority: snapshot.stateAuthority,
    virtualSpanAuthority: snapshot.virtualSpanAuthority,
    hostMemoryImported: snapshot.hostMemoryImported,
    browserMemoryImported: snapshot.browserMemoryImported,
    allocatorQuotaAuthority: snapshot.allocatorQuotaAuthority,
  };
}

function normalizeGetFileAttributesA(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const normalizeCall = (call) => ({
    ...call,
    module: call.module.toLowerCase(),
  });
  return {
    callCount: snapshot.callCount,
    firstCall: normalizeCall(snapshot.firstCall),
    lastCall: normalizeCall(snapshot.lastCall),
    pathAuthority: snapshot.pathAuthority,
    fileExistenceAuthority: snapshot.fileExistenceAuthority,
    directoryExistence: snapshot.directoryExistence,
    hostFileSystemImported: snapshot.hostFileSystemImported,
    browserFileSystemImported: snapshot.browserFileSystemImported,
    mutation: snapshot.mutation,
  };
}

function normalizeCreateDirectoryA(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    ...snapshot,
    call: {
      ...snapshot.call,
      module: snapshot.call.module.toLowerCase(),
    },
  };
}

function normalizeSetWindowPosRepeatActivation(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const { calls: legacyCalls, ...rest } = snapshot;
  return {
    ...rest,
    callCount: snapshot.callCount ?? legacyCalls?.length,
    repeatCall: {
      ...snapshot.repeatCall,
      module: snapshot.repeatCall.module.toLowerCase(),
    },
  };
}

function normalizeSetWindowPosFrameChangeMove(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const call = snapshot.call;
  return {
    callCount: snapshot.callCount,
    call: {
      sequence: call.sequence,
      module: call.module.toLowerCase(),
      symbol: call.symbol,
      occurrence: call.occurrence,
      trapInstructionCount: call.trapInstructionCount,
      arguments: call.arguments,
      result: call.result,
      implementationId: call.implementationId,
      returnAddress: call.returnAddress,
      stackPointerBefore: call.stackPointerBefore,
      stackPointerAfter: call.stackPointerAfter,
      lastError: call.lastError,
      guestWrites: call.guestWrites,
      geometry: call.window.geometry,
    },
    callbackEvents: snapshot.callbackEvents,
    nestedMoveCallbackEvents: snapshot.nestedMoveCallbackEvents,
    windowAuthority: snapshot.windowAuthority,
    relationAuthority: snapshot.relationAuthority,
    geometryCommit: snapshot.geometryCommit,
    relationMutation: snapshot.relationMutation,
    gdiResourceMutation: snapshot.gdiResourceMutation,
    browserResourceCreated: snapshot.browserResourceCreated,
    nonClientPaintAndEraseModeled: snapshot.nonClientPaintAndEraseModeled,
  };
}

function expectedUpdateWindowPaintStructBytes() {
  return [
    0x09, 0x00, 0x01, 0x47,
    0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x05, 0x00, 0x00,
    0xd0, 0x02, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
  ];
}

function normalizeShowWindowNormal(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    ...snapshot,
    call: {
      ...snapshot.call,
      module: snapshot.call.module.toLowerCase(),
      guestWrites: snapshot.call.guestWrites.map(({ address, bytes }) => ({
        address,
        bytes,
      })),
    },
  };
}

function normalizePaintSnapshot(snapshot) {
  if (snapshot.action === "begun") {
    return {
      action: snapshot.action,
      window: {
        id: snapshot.window.id,
        handle: snapshot.window.handle,
        ownerThreadId: snapshot.window.ownerThreadId,
        geometry: snapshot.window.geometry,
        visible: snapshot.window.visible,
        pendingUpdateRect: snapshot.window.pendingUpdateRect,
        paintGeneration: snapshot.window.paintGeneration,
      },
      session: snapshot.session,
      deviceContext: snapshot.deviceContext,
      livePaintSessions: snapshot.livePaintSessions,
      liveDeviceContexts: snapshot.liveDeviceContexts,
    };
  }
  return {
    action: snapshot.action,
    window: {
      id: snapshot.window.id,
      handle: snapshot.window.handle,
      ownerThreadId: snapshot.window.ownerThreadId,
      geometry: snapshot.window.geometry,
      visible: snapshot.window.visible,
      pendingUpdateRect: snapshot.window.pendingUpdateRect,
      paintGeneration: snapshot.window.paintGeneration,
    },
    paintStructAddress: snapshot.paintStructAddress,
    livePaintSessions: snapshot.livePaintSessions,
    liveDeviceContexts: snapshot.liveDeviceContexts,
  };
}

function normalizePaintCall(call) {
  return {
    sequence: call.sequence,
    module: call.module.toLowerCase(),
    symbol: call.symbol,
    occurrence: call.occurrence,
    trapInstructionCount: call.trapInstructionCount,
    arguments: call.arguments,
    result: call.result,
    implementationId: call.implementationId,
    returnAddress: call.returnAddress,
    stackPointerBefore: call.stackPointerBefore,
    stackPointerAfter: call.stackPointerAfter,
    lastError: call.lastError,
    guestWrites: call.guestWrites.map(({ address, bytes }) => ({
      address,
      bytes,
    })),
    paint: normalizePaintSnapshot(call.paint),
  };
}

function normalizeUpdateWindow(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    callCount: snapshot.callCount,
    call: {
      ...snapshot.call,
      module: snapshot.call.module.toLowerCase(),
    },
    callbackEvents: snapshot.callbackEvents,
    beginPaint: normalizePaintCall(snapshot.beginPaint),
    endPaint: normalizePaintCall(snapshot.endPaint),
    windowAuthority: snapshot.windowAuthority,
    updateRegionAuthority: snapshot.updateRegionAuthority,
    updateRegionSource: snapshot.updateRegionSource,
    updateRegionConsumption: snapshot.updateRegionConsumption,
    paintSessionAuthority: snapshot.paintSessionAuthority,
    deviceContextAuthority: snapshot.deviceContextAuthority,
    paintSessionRetired: snapshot.paintSessionRetired,
    deviceContextRetired: snapshot.deviceContextRetired,
    failureAtomicity: snapshot.failureAtomicity,
    callbackActiveAtStop: snapshot.callbackActiveAtStop,
    browserResourceCreated: snapshot.browserResourceCreated,
  };
}

function normalizeD3d9RecreatedTextures(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    calls: snapshot.calls.map(normalizeSemanticCall),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    createTextureProfileId: snapshot.createTextureProfileId,
    getSurfaceLevelProfileId: snapshot.getSurfaceLevelProfileId,
    textureAuthority: snapshot.textureAuthority,
    comAuthority: snapshot.comAuthority,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
    mutation: snapshot.mutation,
  };
}

function normalizeD3d9Clear(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    clearProfileId: snapshot.clearProfileId,
    targetAuthority: snapshot.targetAuthority,
    targetScope: snapshot.targetScope,
    mutation: snapshot.mutation,
    failureAtomicity: snapshot.failureAtomicity,
    depthStencilCleared: snapshot.depthStencilCleared,
    partialRectsImplemented: snapshot.partialRectsImplemented,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9AddDirtyRect(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsTexture: snapshot.call.graphicsTexture,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    addDirtyRectProfileId: snapshot.addDirtyRectProfileId,
    textureAuthority: snapshot.textureAuthority,
    initialDirtyRegion: snapshot.initialDirtyRegion,
    mutation: snapshot.mutation,
    textureGenerationChanged: snapshot.textureGenerationChanged,
    failureAtomicity: snapshot.failureAtomicity,
    dirtyRegionConsumed: snapshot.dirtyRegionConsumed,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9UpdateTexture(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsTextureUpdate: snapshot.call.graphicsTextureUpdate,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    updateTextureProfileId: snapshot.updateTextureProfileId,
    contentAuthority: snapshot.contentAuthority,
    sourceDirtyRegionBefore: snapshot.sourceDirtyRegionBefore,
    sourceDirtyRegionAfter: snapshot.sourceDirtyRegionAfter,
    dirtyRegionConsumed: snapshot.dirtyRegionConsumed,
    transferScope: snapshot.transferScope,
    mutation: snapshot.mutation,
    failureAtomicity: snapshot.failureAtomicity,
    partialDirtyRegionsImplemented: snapshot.partialDirtyRegionsImplemented,
    dynamicDestinationImplemented: snapshot.dynamicDestinationImplemented,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9BeginScene(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    beginSceneProfileId: snapshot.beginSceneProfileId,
    sceneAuthority: snapshot.sceneAuthority,
    lifecycle: snapshot.lifecycle,
    mutation: snapshot.mutation,
    nestedBeginReturnsInvalidCall: snapshot.nestedBeginReturnsInvalidCall,
    failureAtomicity: snapshot.failureAtomicity,
    endSceneImplemented: snapshot.endSceneImplemented,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9InitialSamplerStates(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    calls: snapshot.calls.map((call) => ({
      ...normalizeSemanticCall(call),
      graphicsDevice: call.graphicsDevice,
    })),
    postSamplerRenderStateRejections:
      snapshot.postSamplerRenderStateRejections.map(normalizeSemanticCall),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    samplerStateProfileId: snapshot.samplerStateProfileId,
    stateAuthority: snapshot.stateAuthority,
    mutation: snapshot.mutation,
    sameValueGenerationChange: snapshot.sameValueGenerationChange,
    unsupportedVariantResult: snapshot.unsupportedVariantResult,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9InitialTextureStageStates(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    calls: snapshot.calls.map((call) => ({
      ...normalizeSemanticCall(call),
      graphicsDevice: call.graphicsDevice,
    })),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    textureStageStateProfileId: snapshot.textureStageStateProfileId,
    stateAuthority: snapshot.stateAuthority,
    mutation: snapshot.mutation,
    sameValueGenerationChange: snapshot.sameValueGenerationChange,
    unsupportedVariantResult: snapshot.unsupportedVariantResult,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeSetRect(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: normalizeSemanticCall(snapshot.call),
    implementationId: snapshot.implementationId,
    outputType: snapshot.outputType,
    coordinateEncoding: snapshot.coordinateEncoding,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    mutation: snapshot.mutation,
    nullOutputReturnsFalse: snapshot.nullOutputReturnsFalse,
    browserOwnsRectState: snapshot.browserOwnsRectState,
  };
}

function validateBgiEquivalence(browser, oracle) {
  const evidenceLevel = fullEvidence ? "full" : "summary";
  assert.equal(
    browser.report.supportedCallCount,
    oracle.supportedCallCount,
    "Chrome Worker supported-call count differs from the Node oracle",
  );
  assert.equal(browser.report.evidenceLevel, evidenceLevel);
  assert.equal(oracle.contract.evidenceLevel, evidenceLevel);
  assert.equal(
    browser.report.supportedCalls.length,
    fullEvidence ? browser.report.supportedCallCount : 0,
  );
  assert.equal(
    oracle.supportedContinuation.length,
    fullEvidence ? oracle.supportedCallCount : 0,
  );
  assert.equal(
    browser.report.supportedCallSemanticsAlgorithm,
    SEMANTIC_CALL_DIGEST_ALGORITHM,
  );
  assert.equal(
    oracle.supportedCallSemanticsAlgorithm,
    SEMANTIC_CALL_DIGEST_ALGORITHM,
  );
  assert.equal(
    browser.report.supportedCallSemanticsSha256,
    oracle.supportedCallSemanticsSha256,
    "Chrome Worker call semantics digest differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeDragAcceptFiles(browser.report.dragAcceptFiles),
    normalizeDragAcceptFiles(oracle.dragAcceptFiles),
    "Chrome Worker DragAcceptFiles evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeSleepZero(browser.report.sleepZero),
    normalizeSleepZero(oracle.sleepZero),
    "Chrome Worker Sleep(0) evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGlobalMemoryStatus(browser.report.globalMemoryStatus),
    normalizeGlobalMemoryStatus(oracle.globalMemoryStatus),
    "Chrome Worker GlobalMemoryStatus evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGetFileAttributesA(browser.report.getFileAttributesA),
    normalizeGetFileAttributesA(oracle.getFileAttributesA),
    "Chrome Worker GetFileAttributesA evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeCreateDirectoryA(browser.report.createDirectoryA),
    normalizeCreateDirectoryA(oracle.createDirectoryA),
    "Chrome Worker CreateDirectoryA evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeSetWindowPosRepeatActivation(
      browser.report.setWindowPosRepeatActivation,
    ),
    normalizeSetWindowPosRepeatActivation(oracle.setWindowPosRepeatActivation),
    "Chrome Worker SetWindowPos repeat activation differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeSetWindowPosFrameChangeMove(
      browser.report.setWindowPosFrameChangeMove,
    ),
    normalizeSetWindowPosFrameChangeMove(oracle.setWindowPosFrameChangeMove),
    "Chrome Worker SetWindowPos frame-change move differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeShowWindowNormal(browser.report.showWindowNormal),
    normalizeShowWindowNormal(oracle.showWindowNormal),
    "Chrome Worker ShowWindow(SW_SHOWNORMAL) evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeUpdateWindow(browser.report.updateWindow),
    normalizeUpdateWindow(oracle.updateWindow),
    "Chrome Worker UpdateWindow paint lifecycle differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9RecreatedTextures(browser.report.d3d9RecreatedTextures),
    normalizeD3d9RecreatedTextures(oracle.d3d9RecreatedTextures),
    "Chrome Worker D3D9 recreated texture evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9Clear(browser.report.d3d9Clear),
    normalizeD3d9Clear(oracle.d3d9Clear),
    "Chrome Worker D3D9 Clear evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9AddDirtyRect(browser.report.d3d9AddDirtyRect),
    normalizeD3d9AddDirtyRect(oracle.d3d9AddDirtyRect),
    "Chrome Worker D3D9 AddDirtyRect evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9UpdateTexture(browser.report.d3d9UpdateTexture),
    normalizeD3d9UpdateTexture(oracle.d3d9UpdateTexture),
    "Chrome Worker D3D9 UpdateTexture evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9BeginScene(browser.report.d3d9BeginScene),
    normalizeD3d9BeginScene(oracle.d3d9BeginScene),
    "Chrome Worker D3D9 BeginScene evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9InitialSamplerStates(
      browser.report.d3d9InitialSamplerStates,
    ),
    normalizeD3d9InitialSamplerStates(oracle.d3d9InitialSamplerStates),
    "Chrome Worker D3D9 SetSamplerState evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9InitialTextureStageStates(
      browser.report.d3d9InitialTextureStageStates,
    ),
    normalizeD3d9InitialTextureStageStates(
      oracle.d3d9InitialTextureStageStates,
    ),
    "Chrome Worker D3D9 SetTextureStageState evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeSetRect(browser.report.setRect),
    normalizeSetRect(oracle.setRect),
    "Chrome Worker SetRect evidence differs from the Node oracle",
  );
  if (fullEvidence) {
    assert.deepEqual(
      browser.report.supportedCalls.map(normalizeSemanticCall),
      oracle.supportedContinuation.map(normalizeSemanticCall),
      "Chrome Worker full call semantics differ from the Node oracle",
    );
  }
  assert.deepEqual(
    normalizeCurrentDirectorySnapshot(browser.report.initialCurrentDirectory),
    normalizeCurrentDirectorySnapshot({
      directory: oracle.process.currentDirectoryA,
      generation: oracle.process.currentDirectoryGeneration,
    }),
    "Chrome Worker initial current directory differs from the Node oracle",
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
  assert.deepEqual(
    (browser.report.heapPageMaterializations ?? []).map(
      normalizeHeapPageMaterialization,
    ),
    (oracle.synchronization.heapPageMaterializations ?? []).map(
      normalizeHeapPageMaterialization,
    ),
    "Chrome Worker heap page-materialization semantics differ from the Node oracle",
  );
  assert.equal(
    browser.report.sourceReads.length,
    oracle.synchronization.sourceReads.length,
    "Chrome Worker source-read completion count differs from the Node oracle",
  );
  const browserStop = browser.report.firstUnsupported;
  const oracleStop = oracle.firstUnsupported;
  const browserCheckpoint = browser.report.checkpointCompletion;
  const oracleCheckpoint = oracle.checkpointCompletion ?? null;
  assert.equal(
    browser.report.initialGeneration,
    oracle.process.memory.generation,
  );
  assert(browser.report.finalGeneration >= browser.report.initialGeneration);
  if (oracleCheckpoint !== null) {
    assert.equal(
      browser.report.timerPacing.requests,
      oracle.waitableTimerWait.pacingRequests.length,
      "Chrome Worker pacing request count differs from the Node transport oracle",
    );
    assert.equal(
      browser.report.timerPacing.completions,
      browser.report.timerPacing.requests,
    );
    assert.equal(browser.report.timerPacing.pending, 0);
    assert.equal(browser.report.timerPacing.cancellations, 0);
    assert.equal(browser.report.timerPacing.staleCallbacks, 0);
    assert.equal(browserStop, null);
    assert(
      browserCheckpoint !== null,
      "Chrome Worker omitted its capability checkpoint",
    );
    assert.equal(browser.report.outcome, "checkpoint-complete");
    assert.equal(
      browser.report.instructions,
      oracleCheckpoint.instructionCount,
    );
    assert.deepEqual(
      {
        capability: browserCheckpoint.capability,
        implementationId: browserCheckpoint.implementationId,
        requiredCompletions: browserCheckpoint.requiredCompletions,
        completionCount: browserCheckpoint.completionCount,
        lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
        lastCompletionInstructionCount:
          browserCheckpoint.lastCompletionInstructionCount,
        instructionCount: browserCheckpoint.instructionCount,
        memoryContentSha256: browserCheckpoint.memoryContentSha256,
      },
      {
        capability: oracleCheckpoint.capability,
        implementationId: oracleCheckpoint.implementationId,
        requiredCompletions: oracleCheckpoint.requiredCompletions,
        completionCount: oracleCheckpoint.completionCount,
        lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
        lastCompletionInstructionCount:
          oracleCheckpoint.lastCompletionInstructionCount,
        instructionCount: oracleCheckpoint.instructionCount,
        memoryContentSha256: oracleCheckpoint.memoryContentSha256,
      },
    );
    assert.equal(
      browserCheckpoint.nextEndpoint.id,
      oracleCheckpoint.nextEndpoint.id,
    );
    assert.equal(
      browserCheckpoint.nextEndpoint.address,
      oracleCheckpoint.nextEndpoint.address,
    );
    assert.equal(
      browserCheckpoint.nextEndpoint.module?.toLowerCase(),
      oracleCheckpoint.nextEndpoint.module?.toLowerCase(),
    );
    assert.deepEqual(
      browserCheckpoint.nextEndpoint.symbol,
      oracleCheckpoint.nextEndpoint.symbol,
    );
    if (oracleCheckpoint.capability === "kernel32-enter-critical-section") {
      assert.deepEqual(
        browserCheckpoint.roundtrips,
        oracleCheckpoint.roundtrips,
        "Chrome Worker critical-section checkpoint differs from the Node oracle",
      );
    }
    if (oracleCheckpoint.capability === "primary-main-loop-progression") {
      assert.deepEqual(
        browserCheckpoint.threadSelections,
        oracleCheckpoint.threadSelections,
        "Chrome Worker scheduler selections differ from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability === "kernel32-leave-critical-section" ||
      oracleCheckpoint.capability ===
        "kernel32-leave-critical-section-continuation"
    ) {
      assert.deepEqual(
        {
          criticalSectionAddress: browserCheckpoint.criticalSectionAddress,
          releaseThreadId: browserCheckpoint.releaseThreadId,
          lastError: browserCheckpoint.lastError,
        },
        {
          criticalSectionAddress: oracleCheckpoint.criticalSectionAddress,
          releaseThreadId: oracleCheckpoint.releaseThreadId,
          lastError: oracleCheckpoint.lastError,
        },
        "Chrome Worker critical-section release differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "kernel32-enter-critical-section-continuation"
    ) {
      assert.deepEqual(
        {
          criticalSectionAddress: browserCheckpoint.criticalSectionAddress,
          acquisitionThreadId: browserCheckpoint.acquisitionThreadId,
          lastError: browserCheckpoint.lastError,
        },
        {
          criticalSectionAddress: oracleCheckpoint.criticalSectionAddress,
          acquisitionThreadId: oracleCheckpoint.acquisitionThreadId,
          lastError: oracleCheckpoint.lastError,
        },
        "Chrome Worker critical-section acquisition differs from the Node oracle",
      );
    }
    return;
  }
  if (oracle.dragAcceptFiles !== undefined) {
    assert.equal(
      browser.report.timerPacing.requests,
      oracle.waitableTimerWait.pacingRequestCount,
      "Chrome Worker pacing request count differs from the long Node oracle",
    );
    assert.equal(
      browser.report.timerPacing.completions,
      browser.report.timerPacing.requests,
    );
    assert.equal(browser.report.timerPacing.pending, 0);
    assert.equal(browser.report.timerPacing.cancellations, 0);
    assert.equal(browser.report.timerPacing.staleCallbacks, 0);
  }
  assert.equal(browserCheckpoint, null);
  assert(browserStop !== null, "Chrome Worker omitted its structured stop");
  assert.equal(browserStop.instructionCount, oracleStop.instructionCount);
  assert.equal(browser.report.instructions, oracleStop.instructionCount);
  assert.equal(browserStop.memoryContentSha256, oracleStop.memoryContentSha256);
  assert.equal(browserStop.capabilityCluster, oracleStop.capabilityCluster);
  if (oracleStop.endpoint === null) {
    assert.equal(browserStop.endpoint, null);
    assert.equal(
      browserStop.fault.instructionPointer,
      oracleStop.fault.instructionPointer,
    );
    assert.equal(
      browserStop.fault.previousInstructionPointer,
      oracleStop.fault.previousInstructionPointer,
    );
    assert.equal(browserStop.fault.faultAddress, oracleStop.fault.faultAddress);
    assert.equal(
      browserStop.fault.pageFaultErrorCode,
      oracleStop.fault.pageFaultErrorCode,
    );
  } else if (oracleStop.endpoint.kind === "com-method") {
    assert.equal(browserStop.endpoint.kind, "com-method");
    assert.equal(
      browserStop.endpoint.endpointAddress,
      oracleStop.endpoint.target.method.endpoint,
    );
    assert.deepEqual(browserStop.endpoint.target, oracleStop.fault.target);
    assert.equal(
      oracleStop.endpoint.target.interface.id,
      oracleStop.fault.target.interfaceId,
    );
    assert.equal(
      oracleStop.endpoint.target.object.id,
      oracleStop.fault.target.objectId,
    );
  } else {
    assert.equal(browserStop.endpoint.id, oracleStop.endpoint.endpoint.id);
    assert.equal(
      browserStop.endpoint.address,
      oracleStop.endpoint.endpoint.address,
    );
    assert.equal(
      browserStop.endpoint.module,
      oracleStop.endpoint.endpoint.module,
    );
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
  assert.equal(
    result.failedSessions,
    0,
    "stress run contained failed sessions",
  );
  assert(
    result.samples.length >= 2,
    "stress run did not record enough memory samples",
  );
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

function createServer(sourceRegistration, writableProfile) {
  return http.createServer(async (request, response) => {
    try {
      if (
        sourceRegistration !== null &&
        handleSourceSnapshotHttp(request, response, sourceRegistration)
      ) {
        return;
      }
      if (
        writableProfile !== null &&
        (await handleWritableProfileHttp(request, response, writableProfile))
      ) {
        return;
      }
      const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
      let file;
      if (bgiMode && pathname === "/private/BGI.exe") {
        file = bgiPath;
      } else if (bgiMode && pathname === "/private/kernel32-policy.json") {
        file = bgiPolicyPath;
      } else {
        const name = pathname === "/" ? "index.html" : pathname.slice(1);
        file = path.resolve(assetsRoot, name);
        assert(
          file.startsWith(`${path.resolve(assetsRoot)}${path.sep}`),
          "invalid asset path",
        );
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

async function startServer(preferredPort, sourceRegistration, writableProfile) {
  const ports =
    preferredPort === 0
      ? [0]
      : Array.from({ length: 20 }, (_, index) => preferredPort + index);
  let lastError = null;
  for (const port of ports) {
    const server = createServer(sourceRegistration, writableProfile);
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

async function checkSourceApi(baseUrl, registration) {
  const manifestResponse = await fetch(`${baseUrl}api/source/v1/manifest`, {
    cache: "no-store",
  });
  assert.equal(manifestResponse.status, 200);
  assert.deepEqual(
    Buffer.from(await manifestResponse.arrayBuffer()),
    registration.manifestBytes,
    "source API manifest bytes changed after registration",
  );
  const file = registration.manifest.files[0];
  const rangeUrl =
    `${baseUrl}api/source/v1/titles/${encodeURIComponent(registration.manifest.titleFingerprint)}` +
    `/snapshots/${encodeURIComponent(registration.manifest.snapshotId)}` +
    `/sources/${registration.manifest.mount.sourceId}/objects/${file.objectId}` +
    "?offset=0&length=7";
  const rangeResponse = await fetch(rangeUrl, { cache: "no-store" });
  assert.equal(rangeResponse.status, 206);
  assert.equal(
    rangeResponse.headers.get("x-vine-title-fingerprint"),
    registration.manifest.titleFingerprint,
  );
  assert.equal(
    rangeResponse.headers.get("x-vine-snapshot-id"),
    registration.manifest.snapshotId,
  );
  assert.equal(
    rangeResponse.headers.get("x-vine-object-id"),
    String(file.objectId),
  );
  assert.equal(rangeResponse.headers.get("x-vine-transferred-bytes"), "7");
  assert.deepEqual(
    Buffer.from(await rangeResponse.arrayBuffer()),
    registration.objects.get(file.objectId).bytes.subarray(0, 7),
    "source API returned different registered bytes",
  );
  const wrongSnapshot = await fetch(
    rangeUrl.replace(
      encodeURIComponent(registration.manifest.snapshotId),
      "wrong-snapshot",
    ),
  );
  assert.equal(wrongSnapshot.status, 404);
  const wrongTitle = await fetch(
    rangeUrl.replace(
      encodeURIComponent(registration.manifest.titleFingerprint),
      "wrong-title",
    ),
  );
  assert.equal(wrongTitle.status, 404);
  const wrongObject = await fetch(
    rangeUrl.replace(`/objects/${file.objectId}`, "/objects/999"),
  );
  assert.equal(wrongObject.status, 404);
  const wrongRange = await fetch(
    rangeUrl.replace("offset=0", `offset=${file.byteLength + 1}`),
  );
  assert.equal(wrongRange.status, 416);
}

async function checkWritableProfileApi(baseUrl, profile) {
  const leaseResponse = await fetch(`${baseUrl}api/profile/v3/sessions`, {
    method: "POST",
    cache: "no-store",
    headers: { "X-Vine-Session-Key": "browser-runner-preflight" },
  });
  assert.equal(leaseResponse.status, 201);
  const sessionEpoch = leaseResponse.headers.get("x-vine-session-epoch");
  assert.match(sessionEpoch, /^[1-9][0-9]*$/u);
  const leaseHeaders = {
    "X-Vine-Session-Key": "browser-runner-preflight",
    "X-Vine-Session-Epoch": sessionEpoch,
  };
  const snapshotResponse = await fetch(`${baseUrl}api/profile/v3/snapshot`, {
    cache: "no-store",
    headers: leaseHeaders,
  });
  assert.equal(snapshotResponse.status, 200);
  const snapshot = JSON.parse(
    Buffer.from(await snapshotResponse.arrayBuffer()),
  );
  assert.deepEqual(snapshot, snapshotWritableProfile(profile));
  const file = snapshot.files[0];
  if (file.exists) {
    const length = Math.min(7, file.byteLength);
    const rangeUrl =
      `${baseUrl}api/profile/v3/titles/${encodeURIComponent(snapshot.titleFingerprint)}` +
      `/profiles/${encodeURIComponent(snapshot.profileId)}/objects/${file.objectId}` +
      `?generation=${file.generation}&offset=0&length=${length}`;
    const rangeResponse = await fetch(rangeUrl, {
      cache: "no-store",
      headers: leaseHeaders,
    });
    assert.equal(rangeResponse.status, 206);
    assert.equal(
      rangeResponse.headers.get("x-vine-profile-id"),
      snapshot.profileId,
    );
    assert.equal(
      rangeResponse.headers.get("x-vine-generation"),
      String(file.generation),
    );
    const expected = readWritableProfile(profile, {
      sourceId: 0,
      objectId: file.objectId,
      offset: 0n,
      requestedBytes: length,
    });
    assert.deepEqual(
      Buffer.from(await rangeResponse.arrayBuffer()),
      expected.bytes,
    );
    assert.equal(
      (
        await fetch(
          rangeUrl.replace(
            `generation=${file.generation}`,
            "generation=999999",
          ),
          {
            headers: leaseHeaders,
          },
        )
      ).status,
      409,
    );
    assert.equal(
      (
        await fetch(
          rangeUrl.replace(
            encodeURIComponent(snapshot.titleFingerprint),
            "wrong-title",
          ),
          { headers: leaseHeaders },
        )
      ).status,
      404,
    );
  }
  const revokeResponse = await fetch(
    `${baseUrl}api/profile/v3/sessions/${sessionEpoch}`,
    {
      method: "DELETE",
      cache: "no-store",
      headers: { "X-Vine-Session-Key": "browser-runner-preflight" },
    },
  );
  assert.equal(revokeResponse.status, 204);
  assert.equal(revokeResponse.headers.get("x-vine-session-revoked"), "true");
}

function assertWritableProfileLeaseReleased(profile) {
  const lease = profile.database
    .prepare(
      "SELECT session_key, active FROM profile_lease WHERE singleton = 1",
    )
    .get();
  assert.equal(lease?.active, 0);
  assert.equal(lease?.session_key, null);
}

function findChrome() {
  const candidates = [
    process.env.VINE_CHROME,
    process.env.ProgramFiles &&
      path.join(
        process.env.ProgramFiles,
        "Google",
        "Chrome",
        "Application",
        "chrome.exe",
      ),
    process.env.LOCALAPPDATA &&
      path.join(
        process.env.LOCALAPPDATA,
        "Google",
        "Chrome",
        "Application",
        "chrome.exe",
      ),
    process.env["ProgramFiles(x86)"] &&
      path.join(
        process.env["ProgramFiles(x86)"],
        "Google",
        "Chrome",
        "Application",
        "chrome.exe",
      ),
  ].filter(Boolean);
  const chrome = candidates.find((candidate) => fs.existsSync(candidate));
  assert(chrome, "Chrome stable was not found; set VINE_CHROME to chrome.exe");
  return chrome;
}

async function resolveChromeSelection() {
  if (bgiBenchmarkMilliseconds <= 0) {
    const executablePath = findChrome();
    return {
      executablePath,
      version: await chromeVersion(executablePath),
      artifact: null,
    };
  }
  assert.equal(
    process.platform,
    "win32",
    "canonical BGI performance acceptance is Windows-only",
  );
  assert.equal(
    process.arch,
    "x64",
    "canonical BGI performance acceptance requires Windows x64",
  );
  const artifact = await loadChromeForTestingArtifact(root);
  const version = await chromeVersion(artifact.executablePath);
  assert.equal(
    version,
    artifact.manifest.executable.productVersion,
    "Chrome product version differs from the SHA-pinned artifact manifest",
  );
  return { executablePath: artifact.executablePath, version, artifact };
}

async function chromeVersion(chrome) {
  if (process.platform !== "win32") {
    return (await run(chrome, ["--version"])).stdout.trim();
  }
  return (
    await run(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        "(Get-Item -LiteralPath $env:VINE_CHROME_PATH).VersionInfo.ProductVersion",
      ],
      { env: { ...process.env, VINE_CHROME_PATH: chrome } },
    )
  ).stdout.trim();
}

async function stopChrome(chrome) {
  if (chrome === null) {
    return;
  }
  terminateProcess(chrome.child);
  if (
    !(await Promise.race([
      chrome.exit.then(() => true),
      wait(5_000).then(() => false),
    ]))
  ) {
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

assert(
  fs.existsSync(path.join(assetsRoot, "index.html")),
  "browser runtime is not built",
);
assert(
  fs.existsSync(path.join(assetsRoot, "source-check.html")),
  "source gateway browser check is not built",
);
assert(
  fs.existsSync(sourceManifestPath),
  `source manifest is missing at ${sourceManifestPath}`,
);
assert(
  fs.existsSync(sourceObjectPath),
  `source object is missing at ${sourceObjectPath}`,
);
if (bgiMode) {
  assert(
    fs.existsSync(path.join(assetsRoot, "bgi-check.html")),
    "BGI browser check is not built",
  );
  assert(fs.existsSync(bgiPath), `BGI source is missing at ${bgiPath}`);
  assert(
    fs.existsSync(bgiPolicyPath),
    `BGI policy is missing at ${bgiPolicyPath}`,
  );
  assert(
    fs.existsSync(bgiOracleReportPath),
    "Node BGI oracle report is missing",
  );
  assert(
    fs.existsSync(bgiBenchmarkConfigurationPath),
    "BGI benchmark configuration is missing",
  );
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
assert(
  Number.isFinite(stressMilliseconds) && stressMilliseconds >= 0,
  "--stress-ms is invalid",
);
assert(
  Number.isFinite(bgiBenchmarkMilliseconds) && bgiBenchmarkMilliseconds >= 0,
  "--bgi-benchmark-ms is invalid",
);
const requestedPort = serveOnly ? Number(optionValue("--port") ?? 4173) : 0;
const sourceRegistration = bgiMode
  ? archiveSourceReadProfile
    ? await registerSourceSnapshot({
        manifestPath: path.join(
          root,
          "fixtures",
          "source-snapshots",
          "primary-boot-archives-v1.json",
        ),
        objectFiles: {
          1: path.join(path.dirname(bgiPath), "system.arc"),
          2: path.join(path.dirname(bgiPath), "sysprg.arc"),
          3: path.join(path.dirname(bgiPath), "sysgrp.arc"),
        },
      })
    : null
  : await registerSourceSnapshot({
      manifestPath: sourceManifestPath,
      objectFiles: { 1: sourceObjectPath },
    });
const bgiGdbBefore =
  bgiMode && fs.existsSync(bgiGdbPath) ? await readFile(bgiGdbPath) : null;
const writableProfileBaseDirectory = bgiMode
  ? path.join(
      root,
      ".work",
      "private-profiles",
      checkpoint.browserCheckpoint.bgiRuntimeEvidence.sourceSha256,
    )
  : null;
if (writableProfileBaseDirectory !== null) {
  fs.mkdirSync(writableProfileBaseDirectory, { recursive: true });
}
const writableProfileRunDirectory =
  bgiMode && createDirectoryAProfile
    ? await mkdtemp(
        path.join(
          writableProfileBaseDirectory,
          "browser-create-directory-run-",
        ),
      )
    : null;
const writableProfile = bgiMode
  ? openWritableProfile({
      databasePath: path.join(
        writableProfileRunDirectory ?? writableProfileBaseDirectory,
        "default.sqlite",
      ),
      titleFingerprint:
        checkpoint.browserCheckpoint.bgiRuntimeEvidence.sourceSha256,
      profileId: "default",
      rootPathA: Buffer.from("C:\\Vine\0", "binary"),
      quotaBytes: 16 * 1024 * 1024,
      files: [
        {
          objectId: 1,
          relativePathA: Buffer.from("BGI.gdb", "ascii"),
          ...(bgiGdbBefore === null ? {} : { seedBytes: bgiGdbBefore }),
        },
      ],
      directories: createDirectoryAProfile
        ? [
            {
              directoryId: 1,
              relativePathA: Buffer.from("UserData", "ascii"),
            },
          ]
        : [],
    })
  : null;
const { server, port } = await startServer(
  requestedPort,
  sourceRegistration,
  writableProfile,
);
const url = `http://127.0.0.1:${port}/`;
if (sourceRegistration !== null) {
  await checkSourceApi(url, sourceRegistration);
}
if (writableProfile !== null) {
  await checkWritableProfileApi(url, writableProfile);
}

if (serveOnly) {
  console.log(`Vine browser runtime listening at ${url}`);
  await waitForShutdown();
  await closeServer(server);
  writableProfile?.close();
  await cleanupWritableProfileRunDirectory();
} else {
  const temporaryRoot = path.resolve(os.tmpdir());
  let profile = null;
  let chrome = null;
  try {
    const chromeSelection = await resolveChromeSelection();
    const chromePath = chromeSelection.executablePath;
    const version = chromeSelection.version;
    profile = await mkdtemp(path.join(temporaryRoot, "vine-browser-runtime-"));
    chrome = await launchChrome(chromePath, [
      "--headless=new",
      ...(process.platform === "linux"
        ? ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
        : []),
      ...(bgiBenchmarkMilliseconds > 0
        ? [
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
            "--disable-renderer-backgrounding",
          ]
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
        const configurationBytes = await readFile(
          bgiBenchmarkConfigurationPath,
        );
        const configuration = JSON.parse(configurationBytes);
        assert.equal(configuration.schema, "vine.bgi-browser-benchmark");
        assert.equal(configuration.schemaVersion, 1);
        if (bgiBenchmarkConfigurationOverride === null) {
          const evidence = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
          assert.deepEqual(configuration.workload, {
            profile: checkpoint.activeRuntimeCheckpoint.profile,
            guestInstructions: evidence.guestInstructions,
            supportedCalls: evidence.supportedCalls,
            sourceReadCompletions: evidence.sourceReadCompletions,
            nextApi: evidence.nextEndpoint,
            nextFaultCode: evidence.nextFaultCode,
            callSemanticsAlgorithm: evidence.callSemanticsAlgorithm,
            callSemanticsSha256: evidence.callSemanticsSha256,
            memoryContentSha256: evidence.memoryContentSha256,
            wasmBridgeCalls: evidence.wasmBridgeCalls,
          });
        } else {
          assert.equal(
            configuration.workload.profile,
            bgiProfile,
            "candidate benchmark profile differs from its versioned configuration",
          );
        }
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
        const environmentPolicy = await loadBrowserPerformanceEnvironmentPolicy(
          browserPerformanceEnvironmentPolicyPath,
        );
        const environmentMonitor =
          await startBrowserPerformanceEnvironmentMonitor({
            root,
            rootProcessId: chrome.child.pid,
            policy: environmentPolicy.value,
            durationMilliseconds: bgiBenchmarkMilliseconds,
          });
        let result;
        let performanceEnvironment;
        try {
          result = await runBgiBenchmark(
            chrome.browserWebSocketUrl,
            url,
            bgiBenchmarkMilliseconds,
            configuration.sampleIntervalMilliseconds,
            bgiBenchmarkConfigurationOverride === null
              ? null
              : configuration.workload,
          );
        } finally {
          performanceEnvironment = await environmentMonitor.stop();
        }
        const {
          metrics,
          thresholdEvaluations,
          thresholdFailures,
          decision: workloadDecision,
        } = summarizeBgiBenchmark(result, configuration);
        const decision = qualifyBenchmarkDecision(
          workloadDecision,
          performanceEnvironment.decision,
        );
        const sourceAfter = await readFile(bgiPath);
        assert.deepEqual(
          sourceAfter,
          sourceBefore,
          "BGI source changed during the benchmark",
        );
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
          schemaVersion: 3,
          measuredAt: new Date().toISOString(),
          evidenceClassification: structuredClone(EVIDENCE_CLASSIFICATION),
          decision,
          workloadDecision,
          environment: {
            host: `${os.platform()} ${os.release()} ${os.arch()}`,
            cpu: os.cpus()[0]?.model ?? "unknown",
            logicalCpus: os.cpus().length,
            totalMemoryBytes: os.totalmem(),
            chrome: version,
            chromeArtifact:
              chromeSelection.artifact === null
                ? null
                : {
                    id: chromeSelection.artifact.manifest.id,
                    version: chromeSelection.artifact.manifest.version,
                    platform: chromeSelection.artifact.manifest.platform,
                    manifest: {
                      path: path.relative(
                        root,
                        chromeSelection.artifact.manifestPath,
                      ),
                      ...chromeSelection.artifact.manifestIdentity,
                    },
                    archive: chromeSelection.artifact.archiveIdentity,
                    executable: chromeSelection.artifact.executableIdentity,
                  },
            node: process.version,
          },
          performanceEnvironment: {
            configuration: {
              path: path.relative(
                root,
                browserPerformanceEnvironmentPolicyPath,
              ),
              bytes: environmentPolicy.bytes.byteLength,
              sha256: hash(environmentPolicy.bytes),
              value: environmentPolicy.value,
            },
            monitor: performanceEnvironment.monitor,
            decision: performanceEnvironment.decision,
            observation: performanceEnvironment.observation,
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
        await writeFile(
          path.join(assetsRoot, "bgi-browser-benchmark-report.json"),
          reportBytes,
        );
        if (decision.status === "invalid-environment") {
          throw new Error(
            `BGI benchmark discarded as invalid environment: ${decision.invalidEnvironmentReasonIds.join(",")}`,
          );
        }
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
        const success = await runBgiProbe(
          chrome.browserWebSocketUrl,
          url,
          "success",
        );
        assertWritableProfileLeaseReleased(writableProfile);
        const cancellation = await runBgiProbe(
          chrome.browserWebSocketUrl,
          url,
          "cancel-pending",
        );
        assertWritableProfileLeaseReleased(writableProfile);
        const watchdog = await runBgiProbe(
          chrome.browserWebSocketUrl,
          url,
          "watchdog",
        );
        assertWritableProfileLeaseReleased(writableProfile);
        console.log(
          `BGI browser generations ${success.report.initialGeneration}->${success.report.finalGeneration}; ` +
            `writebacks=${success.report.writebackBatches}/${success.report.writebackPages}, ` +
            `mappingDeltas=${success.report.mappingDeltas}`,
        );
        await writeFile(
          path.join(assetsRoot, "bgi-browser-latest-success.json"),
          Buffer.from(`${JSON.stringify(success, null, 2)}\n`),
        );
        validateBgiEquivalence(success, oracle);
        const sourceAfter = await readFile(bgiPath);
        assert.deepEqual(
          sourceAfter,
          sourceBefore,
          "BGI source changed during the browser session",
        );
        const report = {
          schema: BROWSER_BGI_REPORT_SCHEMA,
          schemaVersion: BROWSER_BGI_REPORT_SCHEMA_VERSION,
          runtimeProtocol,
          measuredAt: new Date().toISOString(),
          chromeVersion: version,
          source: {
            bytes: sourceBefore.byteLength,
            sha256: hash(sourceBefore),
          },
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
        await writeFile(
          path.join(assetsRoot, "bgi-browser-report.json"),
          reportBytes,
        );
        console.log(
          `BGI Chrome Worker equivalence passed: calls=${success.report.supportedCallCount}, ` +
            `instructions=${success.report.instructions}, generation=${success.report.finalGeneration}, ` +
            `cancel/watchdog=clean, Chrome=${version}, reportSha256=${hash(reportBytes)}`,
        );
      }
    } else {
      const success = await runProbe(
        chrome.browserWebSocketUrl,
        url,
        "success",
        true,
      );
      const cancellation = await runProbe(
        chrome.browserWebSocketUrl,
        url,
        "cancel-pending",
      );
      const watchdog = await runProbe(
        chrome.browserWebSocketUrl,
        url,
        "watchdog",
      );
      const sourceSuccess = await runSourceProbe(
        chrome.browserWebSocketUrl,
        url,
        "success",
      );
      const sourceCancellation = await runSourceProbe(
        chrome.browserWebSocketUrl,
        url,
        "cancel-pending",
      );
      const sourceWatchdog = await runSourceProbe(
        chrome.browserWebSocketUrl,
        url,
        "watchdog",
      );
      await runSourceProbe(chrome.browserWebSocketUrl, url, "wrong-origin");
      let stress = null;
      if (stressMilliseconds > 0) {
        const target = await createDebugTarget(
          chrome.browserWebSocketUrl,
          `${url}?scenario=success`,
        );
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
        schemaVersion: 6,
        runtimeProtocol,
        measuredAt: new Date().toISOString(),
        chromeVersion: version,
        success,
        cancellation,
        watchdog,
        sourceGateway: {
          success: sourceSuccess,
          cancellation: sourceCancellation,
          watchdog: sourceWatchdog,
        },
        stress,
      };
      const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
      await writeFile(
        path.join(assetsRoot, "browser-report.json"),
        reportBytes,
      );
      console.log(
        `browser Worker vertical slice passed: instructions=${success.evidence.report.instructions}, ` +
          `yields=${success.evidence.report.budgetYields}, callbacks=2/depth2, ` +
          `pixel=rgba(255,0,0,255), audio=880Hz, storage=0x5a, ` +
          `sourceReads=${sourceSuccess.report.sourceReads.length}, ` +
          `cancel/watchdog=clean, Chrome=${version}, reportSha256=${hash(reportBytes)}`,
      );
    }
  } finally {
    try {
      await stopChrome(chrome);
    } finally {
      try {
        if (writableProfile !== null) {
          assertWritableProfileLeaseReleased(writableProfile);
        }
      } finally {
        try {
          await closeServer(server);
        } finally {
          writableProfile?.close();
          await cleanupWritableProfileRunDirectory();
          if (bgiMode) {
            const bgiGdbAfter = fs.existsSync(bgiGdbPath)
              ? await readFile(bgiGdbPath)
              : null;
            assert.deepEqual(
              bgiGdbAfter,
              bgiGdbBefore,
              "distribution BGI.gdb changed outside the writable profile",
            );
          }
          if (profile !== null) {
            const safePrefix = path.join(
              temporaryRoot,
              "vine-browser-runtime-",
            );
            assert(
              profile.startsWith(safePrefix),
              "refusing to remove unexpected Chrome profile",
            );
            await rm(profile, {
              force: true,
              maxRetries: 5,
              recursive: true,
              retryDelay: 100,
            });
          }
        }
      }
    }
  }
}

async function cleanupWritableProfileRunDirectory() {
  if (writableProfileRunDirectory === null) {
    return;
  }
  const safePrefix = path.join(
    writableProfileBaseDirectory,
    "browser-create-directory-run-",
  );
  assert(
    writableProfileRunDirectory.startsWith(safePrefix),
    "refusing to remove an unexpected writable profile run directory",
  );
  await rm(writableProfileRunDirectory, {
    force: true,
    maxRetries: 5,
    recursive: true,
    retryDelay: 100,
  });
}
