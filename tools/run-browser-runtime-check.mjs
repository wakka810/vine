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
const d3d9PostRasterStatusWaitableTimerProfile =
  bgiProfile === "d3d9-post-raster-status-waitable-timer";
const d3d9PostScenePresentGatewayProfile =
  bgiProfile === "d3d9-post-scene-present-gateway";
const user32AsyncKeyStateInputContinuationProfile =
  bgiProfile === "user32-async-key-state-input-continuation";
const user32AsyncKeyStateInputProfile =
  bgiProfile === "user32-async-key-state-input" ||
  user32AsyncKeyStateInputContinuationProfile;
const user32AsyncKeyStateInitialScanCheckpointProfile =
  bgiProfile === "user32-async-key-state-initial-scan";
const recurringTimerCycleProfile =
  bgiProfile === "primary-main-loop-recurring-timer-cycle-batch";
const recurringTimerMultiCycleProfile =
  bgiProfile === "primary-main-loop-recurring-timer-multi-cycle-batch";
const recurringTimerWindowProfile =
  bgiProfile === "primary-main-loop-recurring-timer-window-batch";
const presentationProgressGatewayProfile =
  bgiProfile === "primary-main-loop-presentation-progress-gateway";
const inputProgressGatewayProfile =
  bgiProfile === "primary-main-loop-input-progress-gateway";
const windowTimerCallbackActionInputProgressGatewayProfile =
  bgiProfile ===
    "primary-main-loop-window-timer-callback-action-input-progress-gateway";
const pointerMessageActionProgressGatewayProfile =
  bgiProfile === "primary-main-loop-pointer-message-action-progress-gateway";
const setFocusActionContinuationProfile =
  bgiProfile === "user32-set-focus-action-continuation";
const timeGetTimeActionContinuationProfile =
  bgiProfile === "winmm-time-get-time-action-continuation";
const getAsyncKeyStatePostIsIconicClockContinuationProfile =
  bgiProfile ===
  "user32-get-async-key-state-post-is-iconic-clock-continuation";
const timeGetTimePostIsIconicContinuationProfile =
  getAsyncKeyStatePostIsIconicClockContinuationProfile ||
  bgiProfile === "winmm-time-get-time-post-is-iconic-continuation";
const isIconicPostPointerReleaseContinuationProfile =
  timeGetTimePostIsIconicContinuationProfile ||
  bgiProfile === "user32-is-iconic-post-pointer-release-continuation";
const getSystemMetricsFixedFrameHeightPostPointerReleaseContinuationProfile =
  isIconicPostPointerReleaseContinuationProfile ||
  bgiProfile ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation";
const getSystemMetricsCaptionPostPointerReleaseContinuationProfile =
  getSystemMetricsFixedFrameHeightPostPointerReleaseContinuationProfile ||
  bgiProfile ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation";
const getSystemMetricsPostPointerReleaseContinuationProfile =
  getSystemMetricsCaptionPostPointerReleaseContinuationProfile ||
  bgiProfile === "user32-get-system-metrics-post-pointer-release-continuation";
const getCursorPosPostPointerReleaseContinuationProfile =
  bgiProfile ===
    "user32-get-cursor-pos-post-pointer-release-continuation" ||
  getSystemMetricsPostPointerReleaseContinuationProfile;
const getWindowRectPostPointerReleaseContinuationProfile =
  bgiProfile ===
    "user32-get-window-rect-post-pointer-release-continuation" ||
  getCursorPosPostPointerReleaseContinuationProfile;
const timeGetTimePostPointerReleaseRepeatBatchProfile =
  bgiProfile === "winmm-time-get-time-post-pointer-release-repeat-batch" ||
  getWindowRectPostPointerReleaseContinuationProfile;
const timeGetTimePostPointerReleaseContinuationProfile =
  bgiProfile === "winmm-time-get-time-post-pointer-release-continuation" ||
  timeGetTimePostPointerReleaseRepeatBatchProfile;
const peekMessageAPostPointerReleaseContinuationProfile =
  bgiProfile === "user32-peek-message-a-post-pointer-release-continuation" ||
  timeGetTimePostPointerReleaseContinuationProfile;
const defWindowProcAPointerReleaseContinuationProfile =
  bgiProfile === "user32-def-window-proc-a-pointer-release-continuation" ||
  peekMessageAPostPointerReleaseContinuationProfile;
const dispatchMessageAPointerReleaseContinuationProfile =
  bgiProfile === "user32-dispatch-message-a-pointer-release-continuation" ||
  defWindowProcAPointerReleaseContinuationProfile;
const translateMessagePointerReleaseContinuationProfile =
  bgiProfile === "user32-translate-message-pointer-release-continuation" ||
  dispatchMessageAPointerReleaseContinuationProfile;
const peekMessageAPointerReleaseContinuationProfile =
  bgiProfile === "user32-peek-message-a-pointer-release-continuation" ||
  translateMessagePointerReleaseContinuationProfile;
const defWindowProcAActionContinuationProfile =
  bgiProfile === "user32-def-window-proc-a-action-continuation" ||
  peekMessageAPointerReleaseContinuationProfile;
const timeGetTimeEvidenceProfile =
  timeGetTimeActionContinuationProfile ||
  defWindowProcAActionContinuationProfile;
const setFocusEvidenceProfile =
  setFocusActionContinuationProfile || timeGetTimeEvidenceProfile;
const pointerMessageEvidenceProfile =
  pointerMessageActionProgressGatewayProfile || setFocusEvidenceProfile;
const actionInputProgressGatewayProfile =
  bgiProfile === "primary-main-loop-action-input-progress-gateway" ||
  windowTimerCallbackActionInputProgressGatewayProfile;
const windowTimerCallbackDeliveryGatewayProfile =
  bgiProfile === "primary-main-loop-window-timer-callback-delivery-gateway";
const windowTimerCallbackRecurringBatchProfile =
  bgiProfile === "primary-main-loop-window-timer-callback-recurring-batch";
const windowTimerCallbackDeliveryProfile =
  windowTimerCallbackDeliveryGatewayProfile ||
  windowTimerCallbackRecurringBatchProfile ||
  windowTimerCallbackActionInputProgressGatewayProfile ||
  pointerMessageEvidenceProfile;
const inputProgressEvidenceProfile =
  inputProgressGatewayProfile || actionInputProgressGatewayProfile;
const presentationProgressEvidenceProfile =
  presentationProgressGatewayProfile ||
  inputProgressEvidenceProfile ||
  windowTimerCallbackDeliveryProfile ||
  pointerMessageEvidenceProfile;
const runtimeTraplessExecutionBudgetProfile =
  bgiProfile === "runtime-trapless-execution-budget";
const largeHeapFreeProfile = bgiProfile === "kernel32-heap-free-large-buffer";
const heapAlloc24Profile = bgiProfile === "kernel32-heap-alloc-24-byte-buffer";
const postHeapTimeGetTimeProfile =
  bgiProfile === "winmm-time-get-time-post-heap-reuse";
const postClockCriticalSectionProfile =
  bgiProfile === "kernel32-enter-critical-section-post-clock";
const postWaitCriticalSectionProfile =
  bgiProfile === "kernel32-enter-critical-section-post-wait";
const postTimerRearmWaitCriticalSectionProfile =
  bgiProfile === "kernel32-enter-critical-section-post-timer-rearm-wait-batch";
const postTimerWaitCriticalSectionProfile =
  postTimerRearmWaitCriticalSectionProfile ||
  bgiProfile === "kernel32-enter-critical-section-post-timer-wait-batch";
const postTimerRearmWaitSetWaitableTimerProfile =
  bgiProfile === "kernel32-set-waitable-timer-post-timer-rearm-wait-batch";
const postTimerWaitSetWaitableTimerProfile =
  postTimerRearmWaitSetWaitableTimerProfile ||
  bgiProfile === "kernel32-set-waitable-timer-post-timer-wait-batch";
const postCriticalSectionSetWaitableTimerProfile =
  bgiProfile === "kernel32-set-waitable-timer-post-critical-section-batch" ||
  postTimerWaitSetWaitableTimerProfile;
const postTimerRearmWaitProfile =
  bgiProfile === "kernel32-wait-for-single-object-ex-post-timer-rearm-batch";
const postTimerArmWaitProfile =
  bgiProfile === "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
  bgiProfile === "kernel32-wait-for-single-object-ex-post-timer-rearm-batch";
const postCriticalSectionWaitProfile =
  bgiProfile === "kernel32-wait-for-single-object-ex-post-critical-section";
const postHeapTimeGetTimeEvidenceProfile =
  recurringTimerCycleProfile ||
  recurringTimerMultiCycleProfile ||
  recurringTimerWindowProfile ||
  presentationProgressEvidenceProfile ||
  windowTimerCallbackDeliveryProfile ||
  postHeapTimeGetTimeProfile ||
  postClockCriticalSectionProfile ||
  postWaitCriticalSectionProfile ||
  postTimerWaitCriticalSectionProfile ||
  postCriticalSectionSetWaitableTimerProfile ||
  postTimerArmWaitProfile ||
  postCriticalSectionWaitProfile;
const runtimeThreadStackGrowthProfile =
  !windowTimerCallbackDeliveryGatewayProfile &&
  (bgiProfile === "runtime-thread-stack-growth" ||
    recurringTimerCycleProfile ||
    recurringTimerMultiCycleProfile ||
    recurringTimerWindowProfile ||
    presentationProgressEvidenceProfile ||
    runtimeTraplessExecutionBudgetProfile ||
    largeHeapFreeProfile ||
    heapAlloc24Profile ||
    postHeapTimeGetTimeEvidenceProfile);
const user32IsIconicProfile =
  bgiProfile === "user32-is-iconic" || runtimeThreadStackGrowthProfile;
const user32PeekMessageAProfile =
  bgiProfile === "user32-peek-message-a" ||
  user32IsIconicProfile ||
  windowTimerCallbackDeliveryProfile;
const user32AsyncKeyStateInitialScanContinuationProfile =
  bgiProfile === "user32-async-key-state-initial-scan-continuation" ||
  user32PeekMessageAProfile;
const user32AsyncKeyStateInitialScanProfile =
  user32AsyncKeyStateInitialScanCheckpointProfile ||
  user32AsyncKeyStateInitialScanContinuationProfile;
const d3d9CompatibilityPolicyId = user32AsyncKeyStateInitialScanProfile
  ? "d3d9-primary-virtual-adapter-v32"
  : "d3d9-primary-virtual-adapter-v31";
const user32AsyncKeyStateInitialScanVirtualKeys = [
  14, 15, 13, 16, 27, 38, 33, 40, 34, 37, 39, 49, 97, 50, 98, 51, 99, 52, 100,
  53, 101, 54, 102, 55, 103, 56, 104, 57, 105, 48, 96, 9, 17, 1, 2, 4, 5, 6,
];
const user32AsyncKeyStateGatewayProfile =
  user32AsyncKeyStateInputProfile || user32AsyncKeyStateInitialScanProfile;
const d3d9PostPresentContinuationProfile =
  bgiProfile === "d3d9-post-present-continuation" ||
  user32AsyncKeyStateGatewayProfile;
const d3d9PresentProfile =
  bgiProfile === "d3d9-first-present" || d3d9PostPresentContinuationProfile;
const d3d9PostEndSceneRasterStatusProfile =
  d3d9PresentProfile ||
  d3d9PostRasterStatusWaitableTimerProfile ||
  d3d9PostScenePresentGatewayProfile ||
  bgiProfile === "d3d9-post-end-scene-raster-status";
const d3d9EndSceneProfile =
  d3d9PostEndSceneRasterStatusProfile || bgiProfile === "d3d9-end-scene";
const d3d9DrawPrimitiveProfile =
  d3d9EndSceneProfile || bgiProfile === "d3d9-draw-primitive";
const d3d9SetStreamSourceProfile =
  d3d9DrawPrimitiveProfile || bgiProfile === "d3d9-set-stream-source";
const d3d9SetFvfProfile =
  d3d9SetStreamSourceProfile || bgiProfile === "d3d9-set-fvf";
const d3d9SetTextureProfile =
  d3d9SetFvfProfile || bgiProfile === "d3d9-set-texture";
const d3d9InitialTextureStageStatesProfile =
  d3d9SetTextureProfile || bgiProfile === "d3d9-initial-texture-stage-states";
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
const runtimeProtocol = "vine.browser-runtime-worker.v96";
if (bgiProfileOverride === null) {
  assert.equal(
    checkpoint.browserCheckpoint.browserRuntimeProtocol,
    runtimeProtocol,
    "current checkpoint does not identify the built Runtime Worker protocol",
  );
}
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
      scenario === "success" && user32AsyncKeyStateInitialScanProfile
        ? 600_000
        : scenario === "success" && sleepZeroProfile
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
        "d3d9-set-texture",
        "d3d9-set-fvf",
        "d3d9-set-stream-source",
        "d3d9-draw-primitive",
        "d3d9-end-scene",
        "d3d9-post-end-scene-raster-status",
        "d3d9-post-raster-status-waitable-timer",
        "d3d9-post-scene-present-gateway",
        "d3d9-first-present",
        "d3d9-post-present-continuation",
        "user32-async-key-state-input",
        "user32-async-key-state-input-continuation",
        "user32-async-key-state-initial-scan",
        "user32-async-key-state-initial-scan-continuation",
        "user32-peek-message-a",
        "user32-is-iconic",
        "runtime-thread-stack-growth",
        "runtime-trapless-execution-budget",
        "kernel32-heap-free-large-buffer",
        "kernel32-heap-alloc-24-byte-buffer",
        "winmm-time-get-time-post-heap-reuse",
        "kernel32-enter-critical-section-post-clock",
        "kernel32-enter-critical-section-post-wait",
        "kernel32-enter-critical-section-post-timer-wait-batch",
        "kernel32-enter-critical-section-post-timer-rearm-wait-batch",
        "kernel32-set-waitable-timer-post-critical-section-batch",
        "kernel32-set-waitable-timer-post-timer-wait-batch",
        "kernel32-set-waitable-timer-post-timer-rearm-wait-batch",
        "kernel32-wait-for-single-object-ex-post-timer-arm-batch",
        "kernel32-wait-for-single-object-ex-post-timer-rearm-batch",
        "kernel32-wait-for-single-object-ex-post-critical-section",
        "primary-main-loop-recurring-timer-cycle-batch",
        "primary-main-loop-recurring-timer-multi-cycle-batch",
        "primary-main-loop-recurring-timer-window-batch",
        "primary-main-loop-presentation-progress-gateway",
        "primary-main-loop-input-progress-gateway",
        "primary-main-loop-action-input-progress-gateway",
        "primary-main-loop-window-timer-callback-delivery-gateway",
        "primary-main-loop-window-timer-callback-recurring-batch",
        "primary-main-loop-window-timer-callback-action-input-progress-gateway",
        "primary-main-loop-pointer-message-action-progress-gateway",
        "user32-set-focus-action-continuation",
        "winmm-time-get-time-action-continuation",
        "user32-def-window-proc-a-action-continuation",
        "user32-peek-message-a-pointer-release-continuation",
        "user32-translate-message-pointer-release-continuation",
        "user32-dispatch-message-a-pointer-release-continuation",
        "user32-def-window-proc-a-pointer-release-continuation",
        "user32-peek-message-a-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-repeat-batch",
        "user32-get-window-rect-post-pointer-release-continuation",
        "user32-get-cursor-pos-post-pointer-release-continuation",
        "user32-get-system-metrics-post-pointer-release-continuation",
        "user32-get-system-metrics-caption-post-pointer-release-continuation",
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation",
        "user32-is-iconic-post-pointer-release-continuation",
        "winmm-time-get-time-post-is-iconic-continuation",
        "user32-get-async-key-state-post-is-iconic-clock-continuation",
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
        "d3d9-set-texture",
        "d3d9-set-fvf",
        "d3d9-set-stream-source",
        "d3d9-draw-primitive",
        "d3d9-end-scene",
        "d3d9-post-end-scene-raster-status",
        "d3d9-post-raster-status-waitable-timer",
        "d3d9-post-scene-present-gateway",
        "d3d9-first-present",
        "d3d9-post-present-continuation",
        "user32-async-key-state-input",
        "user32-async-key-state-input-continuation",
        "user32-async-key-state-initial-scan",
        "user32-async-key-state-initial-scan-continuation",
        "user32-peek-message-a",
        "user32-is-iconic",
        "runtime-thread-stack-growth",
        "runtime-trapless-execution-budget",
        "kernel32-heap-free-large-buffer",
        "kernel32-heap-alloc-24-byte-buffer",
        "winmm-time-get-time-post-heap-reuse",
        "kernel32-enter-critical-section-post-clock",
        "kernel32-enter-critical-section-post-wait",
        "kernel32-enter-critical-section-post-timer-wait-batch",
        "kernel32-enter-critical-section-post-timer-rearm-wait-batch",
        "kernel32-set-waitable-timer-post-critical-section-batch",
        "kernel32-set-waitable-timer-post-timer-wait-batch",
        "kernel32-set-waitable-timer-post-timer-rearm-wait-batch",
        "kernel32-wait-for-single-object-ex-post-timer-arm-batch",
        "kernel32-wait-for-single-object-ex-post-timer-rearm-batch",
        "kernel32-wait-for-single-object-ex-post-critical-section",
        "primary-main-loop-recurring-timer-cycle-batch",
        "primary-main-loop-recurring-timer-multi-cycle-batch",
        "primary-main-loop-recurring-timer-window-batch",
        "primary-main-loop-presentation-progress-gateway",
        "primary-main-loop-input-progress-gateway",
        "primary-main-loop-action-input-progress-gateway",
        "primary-main-loop-window-timer-callback-delivery-gateway",
        "primary-main-loop-window-timer-callback-recurring-batch",
        "primary-main-loop-window-timer-callback-action-input-progress-gateway",
        "primary-main-loop-pointer-message-action-progress-gateway",
        "user32-set-focus-action-continuation",
        "winmm-time-get-time-action-continuation",
        "user32-def-window-proc-a-action-continuation",
        "user32-peek-message-a-pointer-release-continuation",
        "user32-translate-message-pointer-release-continuation",
        "user32-dispatch-message-a-pointer-release-continuation",
        "user32-def-window-proc-a-pointer-release-continuation",
        "user32-peek-message-a-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-repeat-batch",
        "user32-get-window-rect-post-pointer-release-continuation",
        "user32-get-cursor-pos-post-pointer-release-continuation",
        "user32-get-system-metrics-post-pointer-release-continuation",
        "user32-get-system-metrics-caption-post-pointer-release-continuation",
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation",
        "user32-is-iconic-post-pointer-release-continuation",
        "winmm-time-get-time-post-is-iconic-continuation",
        "user32-get-async-key-state-post-is-iconic-clock-continuation",
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
        "d3d9-set-texture",
        "d3d9-set-fvf",
        "d3d9-set-stream-source",
        "d3d9-draw-primitive",
        "d3d9-end-scene",
        "d3d9-post-end-scene-raster-status",
        "d3d9-post-raster-status-waitable-timer",
        "d3d9-post-scene-present-gateway",
        "d3d9-first-present",
        "d3d9-post-present-continuation",
        "user32-async-key-state-input",
        "user32-async-key-state-input-continuation",
        "user32-async-key-state-initial-scan",
        "user32-async-key-state-initial-scan-continuation",
        "user32-peek-message-a",
        "user32-is-iconic",
        "runtime-thread-stack-growth",
        "runtime-trapless-execution-budget",
        "kernel32-heap-free-large-buffer",
        "kernel32-heap-alloc-24-byte-buffer",
        "winmm-time-get-time-post-heap-reuse",
        "kernel32-enter-critical-section-post-clock",
        "kernel32-enter-critical-section-post-wait",
        "kernel32-enter-critical-section-post-timer-wait-batch",
        "kernel32-enter-critical-section-post-timer-rearm-wait-batch",
        "kernel32-set-waitable-timer-post-critical-section-batch",
        "kernel32-set-waitable-timer-post-timer-wait-batch",
        "kernel32-set-waitable-timer-post-timer-rearm-wait-batch",
        "kernel32-wait-for-single-object-ex-post-timer-arm-batch",
        "kernel32-wait-for-single-object-ex-post-timer-rearm-batch",
        "kernel32-wait-for-single-object-ex-post-critical-section",
        "primary-main-loop-recurring-timer-cycle-batch",
        "primary-main-loop-recurring-timer-multi-cycle-batch",
        "primary-main-loop-recurring-timer-window-batch",
        "primary-main-loop-presentation-progress-gateway",
        "primary-main-loop-input-progress-gateway",
        "primary-main-loop-action-input-progress-gateway",
        "primary-main-loop-window-timer-callback-delivery-gateway",
        "primary-main-loop-window-timer-callback-recurring-batch",
        "primary-main-loop-window-timer-callback-action-input-progress-gateway",
        "primary-main-loop-pointer-message-action-progress-gateway",
        "user32-set-focus-action-continuation",
        "winmm-time-get-time-action-continuation",
        "user32-def-window-proc-a-action-continuation",
        "user32-peek-message-a-pointer-release-continuation",
        "user32-translate-message-pointer-release-continuation",
        "user32-dispatch-message-a-pointer-release-continuation",
        "user32-def-window-proc-a-pointer-release-continuation",
        "user32-peek-message-a-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-continuation",
        "winmm-time-get-time-post-pointer-release-repeat-batch",
        "user32-get-window-rect-post-pointer-release-continuation",
        "user32-get-cursor-pos-post-pointer-release-continuation",
        "user32-get-system-metrics-post-pointer-release-continuation",
        "user32-get-system-metrics-caption-post-pointer-release-continuation",
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation",
        "user32-is-iconic-post-pointer-release-continuation",
        "winmm-time-get-time-post-is-iconic-continuation",
        "user32-get-async-key-state-post-is-iconic-clock-continuation",
      ].includes(bgiProfile) && scenario === "success"
        ? user32AsyncKeyStateInitialScanProfile
          ? user32AsyncKeyStateInitialScanContinuationProfile
              ? runtimeThreadStackGrowthProfile
              ? windowTimerCallbackDeliveryGatewayProfile
                ? 2_314
                : windowTimerCallbackRecurringBatchProfile
                  ? 2_608
                : getAsyncKeyStatePostIsIconicClockContinuationProfile
                  ? 2_514
                : pointerMessageEvidenceProfile
                  ? 2_512
                : postClockCriticalSectionProfile ||
                postWaitCriticalSectionProfile ||
                postTimerWaitCriticalSectionProfile ||
                postCriticalSectionSetWaitableTimerProfile ||
                postTimerArmWaitProfile ||
                postCriticalSectionWaitProfile ||
                recurringTimerCycleProfile
                ? 2_457
                : inputProgressEvidenceProfile
                  ? 2_527
                  : presentationProgressGatewayProfile
                    ? 2_513
                  : recurringTimerWindowProfile
                    ? 2_471
                    : recurringTimerMultiCycleProfile
                      ? 2_461
                      : 2_456
              : user32IsIconicProfile
                ? 2_317
                : 2_314
            : 2_312
          : user32AsyncKeyStateInputContinuationProfile
            ? 2_020
            : d3d9PostPresentContinuationProfile
              ? 2_014
              : d3d9RecreatedTexturesProfile
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
                              bgiProfile ===
                                "kernel32-leave-critical-section" ||
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
    assert.equal(evidence.statusMessagesRetained, evidence.statuses.length);
    assert(evidence.statusMessagesObserved >= evidence.statusMessagesRetained);
    assert(evidence.statusMessagesRetained <= 4_096);
    if (evidence.statusMessagesObserved > 4_096) {
      assert.equal(evidence.statusMessagesRetained, 4_096);
    }
    assert(evidence.statuses.some(({ phase }) => phase === "running"));
    if (scenario === "success") {
      const capabilityCheckpoint =
        bgiProfile === "kernel32-wait-for-single-object-ex" ||
        bgiProfile === "kernel32-enter-critical-section" ||
        bgiProfile === "kernel32-enter-critical-section-continuation" ||
        bgiProfile === "kernel32-leave-critical-section-continuation" ||
        bgiProfile === "primary-main-loop-progression" ||
        bgiProfile === "kernel32-leave-critical-section" ||
        recurringTimerCycleProfile ||
        recurringTimerMultiCycleProfile ||
        recurringTimerWindowProfile ||
        presentationProgressEvidenceProfile ||
        windowTimerCallbackDeliveryProfile ||
        runtimeTraplessExecutionBudgetProfile ||
        largeHeapFreeProfile ||
        heapAlloc24Profile ||
        postHeapTimeGetTimeProfile ||
        postClockCriticalSectionProfile ||
        postWaitCriticalSectionProfile ||
        postTimerWaitCriticalSectionProfile ||
        postCriticalSectionSetWaitableTimerProfile ||
        postTimerArmWaitProfile ||
        postCriticalSectionWaitProfile ||
        user32AsyncKeyStateInputProfile ||
        user32AsyncKeyStateInitialScanCheckpointProfile ||
        (d3d9EndSceneProfile && !d3d9PostPresentContinuationProfile);
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
        setWindowPosRepeatActivationProfile ||
        user32AsyncKeyStateInitialScanContinuationProfile
      ) {
        const pacingCount = windowTimerCallbackRecurringBatchProfile
          ? 11_902
          : getAsyncKeyStatePostIsIconicClockContinuationProfile
            ? 11_473
          : pointerMessageEvidenceProfile
            ? 11_465
          : windowTimerCallbackActionInputProgressGatewayProfile
            ? 11_532
          : inputProgressEvidenceProfile
            ? 11_531
            : presentationProgressGatewayProfile
              ? 11_467
              : recurringTimerWindowProfile
                ? 11_272
                : recurringTimerMultiCycleProfile
                  ? 11_224
                  : recurringTimerCycleProfile
                    ? 11_209
                    : user32AsyncKeyStateInitialScanProfile
                  ? user32AsyncKeyStateInitialScanContinuationProfile
                    ? runtimeThreadStackGrowthProfile
                      ? postTimerRearmWaitProfile ||
                        postTimerRearmWaitCriticalSectionProfile ||
                        postTimerRearmWaitSetWaitableTimerProfile
                        ? 11_208
                        : postTimerArmWaitProfile ||
                            postTimerWaitCriticalSectionProfile ||
                            postTimerWaitSetWaitableTimerProfile
                          ? 11_207
                          : postCriticalSectionWaitProfile ||
                              postWaitCriticalSectionProfile ||
                              postCriticalSectionSetWaitableTimerProfile
                            ? 11_206
                            : 11_205
                      : user32IsIconicProfile
                        ? 10_571
                        : 10_569
                    : 10_567
                  : user32AsyncKeyStateInputContinuationProfile
                    ? 9_232
                    : d3d9PostPresentContinuationProfile
                      ? 9_205
                      : d3d9RecreatedTexturesProfile
                        ? d3d9PostRasterStatusWaitableTimerProfile
                          ? 9_187
                          : 9_186
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
                                  : bgiProfile ===
                                        "primary-main-loop-progression" ||
                                      bgiProfile ===
                                        "kernel32-leave-critical-section" ||
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
        if (user32AsyncKeyStateInitialScanProfile) {
          assert.equal(
            archiveReads.length,
            runtimeThreadStackGrowthProfile
              ? presentationProgressEvidenceProfile
                ? 90
                : recurringTimerWindowProfile
                  ? 84
                  : 82
              : user32IsIconicProfile
                ? 60
                : 59,
            "Chrome Worker initial scan changed the reached archive-read count",
          );
          assert(
            archiveReads.every(
              (read) =>
                read.succeeded && read.requestedBytes === read.transferredBytes,
            ),
            "Chrome Worker initial scan returned a partial archive read",
          );
        } else {
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
              ...(d3d9PostPresentContinuationProfile
                ? [
                    {
                      objectId: 2,
                      offset: 9_729,
                      requestedBytes: 3_001,
                      transferredBytes: 3_001,
                      succeeded: true,
                    },
                  ]
                : []),
              ...(user32AsyncKeyStateInputContinuationProfile
                ? [
                    {
                      objectId: 3,
                      offset: 0,
                      requestedBytes: 16,
                      transferredBytes: 16,
                      succeeded: true,
                    },
                    {
                      objectId: 3,
                      offset: 16,
                      requestedBytes: 19_968,
                      transferredBytes: 19_968,
                      succeeded: true,
                    },
                    {
                      objectId: 3,
                      offset: 4_114_329,
                      requestedBytes: 4_651,
                      transferredBytes: 4_651,
                      succeeded: true,
                    },
                  ]
                : []),
            ],
            "Chrome Worker primary boot-archive reads differ from the frozen slice",
          );
        }
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
          ...(d3d9PostPresentContinuationProfile
            ? [
                {
                  action: "return",
                  sequence: 127_352,
                  frameId: 30,
                  instructionCount: 28_761_544,
                  result: 0,
                },
              ]
            : []),
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
            implementationId: "user32.begin-paint.full-client-logical-dc.v1",
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
        assert.equal(update.beginPaint.paint.deviceContext.handle, 0x4701_0009);
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
        assert.equal(
          update.callbackActiveAtStop,
          !d3d9PostPresentContinuationProfile,
        );
        assert.equal(update.browserResourceCreated, false);
      } else {
        assert.equal(evidence.report.updateWindow, null);
      }
      if (d3d9ClearProfile) {
        const clear = evidence.report.d3d9Clear;
        assert(clear !== null, "Chrome Worker omitted D3D9 Clear evidence");
        assert.deepEqual(
          clear.call.arguments,
          [0x0101_0004, 0, 0, 1, 0xff00_0000, 0x3f80_0000, 0],
        );
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
        assert.equal(clear.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          clear.clearProfileId,
          "primary-full-render-target-black-v1",
        );
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
        assert.deepEqual(
          setRect.call.arguments,
          [0x006f_f29c, 0, 0, 1_280, 720],
        );
        assert.deepEqual(setRect.call.result, { action: "set", value: 1 });
        assert.equal(setRect.call.implementationId, "user32.set-rect.v1");
        assert.equal(setRect.call.returnAddress, 0x1005_b00b);
        assert.equal(setRect.call.lastError, 0);
        assert.deepEqual(setRect.call.guestWrites, [
          {
            address: 0x006f_f29c,
            bytes: [0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 208, 2, 0, 0],
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
        assert.equal(
          dirty.call.graphicsTexture?.content?.fills[0]?.sequence,
          3,
        );
        assert.equal(dirty.call.graphicsTexture?.generation, 41);
        assert.equal(dirty.compatibilityPolicyId, d3d9CompatibilityPolicyId);
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
        assert.deepEqual(
          update.call.arguments,
          [0x0101_0004, 0x0102_0004, 0x0103_0004],
        );
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
        assert.deepEqual(
          pair.destination.content?.fills,
          pair.source.content?.fills,
        );
        assert.deepEqual(pair.destination.content?.lastUpdate, {
          sequence: 1,
          sourceTextureId: 7,
          sourceContentGeneration: 1,
          dirtyRegion: { kind: "whole-texture" },
        });
        assert.equal(pair.destination.generation, 45);
        assert.equal(update.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          update.updateTextureProfileId,
          "primary-recreated-system-memory-to-static-default-texture-update-v2",
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
        assert.equal(update.partialDirtyRegionsImplemented, true);
        assert.deepEqual(update.partialDirtyRect, [0, 0, 1_280, 720]);
        assert.equal(
          update.partialTransferPrecondition,
          "destination-immediately-preceding-source-generation",
        );
        assert.equal(update.dynamicDestinationImplemented, false);
        assert.equal(update.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9UpdateTexture, null);
      }
      if (d3d9BeginSceneProfile) {
        const begin = evidence.report.d3d9BeginScene;
        assert(
          begin !== null,
          "Chrome Worker omitted D3D9 BeginScene evidence",
        );
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
        assert.deepEqual(begin.call.graphicsDevice?.activeScene, {
          sequence: 1,
        });
        assert.equal(begin.call.graphicsDevice?.generation, 45);
        assert.equal(
          begin.call.graphicsDevice?.backBuffer.contentGeneration,
          1,
        );
        assert.equal(
          begin.call.graphicsDevice?.backBuffer.uniformClear?.sequence,
          1,
        );
        assert.equal(begin.call.graphicsDevice?.renderer, undefined);
        assert.equal(begin.compatibilityPolicyId, d3d9CompatibilityPolicyId);
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
        assert.equal(begin.endSceneImplemented, d3d9EndSceneProfile);
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
        assert.equal(sampler.compatibilityPolicyId, d3d9CompatibilityPolicyId);
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
          0x1005_b1f5, 0x1005_b20b, 0x1005_b221, 0x1005_b237, 0x1005_b24d,
          0x1005_b263,
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
              ...(index >= 3 ? { alphaOperation: "select-argument-one" } : {}),
              ...(index >= 4 ? { alphaArgumentOne: "texture" } : {}),
              ...(index >= 5 ? { alphaArgumentTwo: "diffuse" } : {}),
            },
          });
          assert.equal(call.graphicsDevice?.generation, 48 + index);
          assert.equal(call.graphicsDevice?.renderer, undefined);
        }
        assert.equal(
          textureStage.compatibilityPolicyId,
          d3d9CompatibilityPolicyId,
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
      if (d3d9SetTextureProfile) {
        const binding = evidence.report.d3d9SetTexture;
        assert(
          binding !== null,
          "Chrome Worker omitted D3D9 SetTexture evidence",
        );
        const call = binding.call;
        assert.equal(call.sequence, 127_371);
        assert.equal(call.symbol, "IDirect3DDevice9.SetTexture");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 65);
        assert.deepEqual(call.arguments, [0x0101_0004, 0, 0x0103_0004]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b27c);
        assert.equal(call.stackPointerBefore, 0x006f_f258);
        assert.equal(call.stackPointerAfter, 0x006f_f268);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        assert.deepEqual(call.graphicsDevice?.textureBindings, {
          stageZero: 8,
        });
        assert.equal(call.graphicsDevice?.generation, 54);
        assert.equal(call.graphicsDevice?.renderer, undefined);
        assert.equal(binding.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          binding.textureBindingProfileId,
          "primary-first-frame-static-default-texture-stage-zero-v1",
        );
        assert.equal(
          binding.stateAuthority,
          "rust-runtime-graphics-logical-device-texture-bindings",
        );
        assert.equal(binding.bindingOwnership, "non-owning");
        assert.equal(
          binding.mutation,
          "validate-plan-preflight-targeted-idempotent-commit",
        );
        assert.equal(binding.sameValueGenerationChange, 0);
        assert.equal(binding.nullUnbindSupported, true);
        assert.equal(binding.unsupportedVariantResult, "0x8876086a");
        assert.equal(binding.failureAtomicity, true);
        assert.equal(binding.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9SetTexture, null);
      }
      if (d3d9SetFvfProfile) {
        const format = evidence.report.d3d9SetFvf;
        assert(format !== null, "Chrome Worker omitted D3D9 SetFVF evidence");
        const call = format.call;
        assert.equal(call.sequence, 127_372);
        assert.equal(call.symbol, "IDirect3DDevice9.SetFVF");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 89);
        assert.deepEqual(call.arguments, [0x0101_0004, 0x144]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b291);
        assert.equal(call.stackPointerBefore, 0x006f_f25c);
        assert.equal(call.stackPointerAfter, 0x006f_f268);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        assert.equal(
          call.graphicsDevice?.flexibleVertexFormat,
          "xyz-rhw-diffuse-texture1",
        );
        assert.deepEqual(call.graphicsDevice?.textureBindings, {
          stageZero: 8,
        });
        assert.equal(call.graphicsDevice?.generation, 55);
        assert.equal(call.graphicsDevice?.renderer, undefined);
        assert.equal(format.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          format.flexibleVertexFormatProfileId,
          "primary-first-frame-transformed-diffuse-textured-fvf-v1",
        );
        assert.equal(
          format.stateAuthority,
          "rust-runtime-graphics-logical-device-flexible-vertex-format",
        );
        assert.equal(
          format.mutation,
          "validate-plan-preflight-targeted-idempotent-commit",
        );
        assert.equal(format.sameValueGenerationChange, 0);
        assert.equal(format.unsupportedVariantResult, "0x8876086a");
        assert.equal(format.failureAtomicity, true);
        assert.equal(format.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9SetFvf, null);
      }
      if (d3d9SetStreamSourceProfile) {
        const binding = evidence.report.d3d9SetStreamSource;
        assert(
          binding !== null,
          "Chrome Worker omitted D3D9 SetStreamSource evidence",
        );
        const call = binding.call;
        assert.equal(call.sequence, 127_373);
        assert.equal(call.symbol, "IDirect3DDevice9.SetStreamSource");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 100);
        assert.deepEqual(call.arguments, [0x0101_0004, 0, 0x0105_0004, 0, 28]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b3ec);
        assert.equal(call.stackPointerBefore, 0x006f_f250);
        assert.equal(call.stackPointerAfter, 0x006f_f268);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        assert.deepEqual(call.graphicsDevice?.streamSources, {
          streamZero: { vertexBufferId: 3, offsetInBytes: 0, stride: 28 },
        });
        assert.equal(
          call.graphicsDevice?.flexibleVertexFormat,
          "xyz-rhw-diffuse-texture1",
        );
        assert.equal(call.graphicsDevice?.generation, 56);
        assert.equal(call.graphicsDevice?.renderer, undefined);
        assert.equal(binding.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          binding.streamSourceProfileId,
          "primary-first-frame-transformed-diffuse-textured-stream-zero-v1",
        );
        assert.equal(
          binding.stateAuthority,
          "rust-runtime-graphics-logical-device-stream-sources",
        );
        assert.equal(binding.bindingOwnership, "non-owning");
        assert.equal(
          binding.mutation,
          "validate-plan-preflight-targeted-idempotent-commit",
        );
        assert.equal(binding.sameValueGenerationChange, 0);
        assert.equal(binding.supportedStream, 0);
        assert.equal(binding.supportedOffsetInBytes, 0);
        assert.equal(binding.supportedStride, 28);
        assert.equal(binding.nullUnbindImplemented, false);
        assert.equal(binding.unsupportedVariantResult, "0x8876086a");
        assert.equal(binding.failureAtomicity, true);
        assert.equal(binding.browserRendererResourceCreated, false);
      } else {
        assert.equal(evidence.report.d3d9SetStreamSource, null);
      }
      if (d3d9DrawPrimitiveProfile) {
        const draw = evidence.report.d3d9DrawPrimitive;
        assert(
          draw !== null,
          "Chrome Worker omitted D3D9 DrawPrimitive evidence",
        );
        const call = draw.call;
        assert.equal(call.sequence, 127_374);
        assert.equal(call.symbol, "IDirect3DDevice9.DrawPrimitive");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 81);
        assert.deepEqual(call.arguments, [0x0101_0004, 5, 0, 2]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b402);
        assert.equal(call.stackPointerBefore, 0x006f_f254);
        assert.equal(call.stackPointerAfter, 0x006f_f268);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        const device = call.graphicsDevice;
        assert(device, "Chrome Worker omitted the Rust graphics snapshot");
        assert.deepEqual(device.activeScene, { sequence: 1 });
        assert.deepEqual(device.textureBindings, { stageZero: 8 });
        assert.equal(device.flexibleVertexFormat, "xyz-rhw-diffuse-texture1");
        assert.deepEqual(device.streamSources, {
          streamZero: { vertexBufferId: 3, offsetInBytes: 0, stride: 28 },
        });
        assert.equal(device.generation, 57);
        assert.equal(device.backBuffer.contentGeneration, 2);
        assert.equal(device.backBuffer.uniformClear?.sequence, 1);
        assert.equal(device.backBuffer.drawSubmissions?.length, 1);
        const submission = device.backBuffer.drawSubmissions?.[0];
        assert(
          submission,
          "Chrome Worker omitted the immutable draw submission",
        );
        assert.equal(submission.sequence, 1);
        assert.equal(submission.sceneSequence, 1);
        assert.equal(submission.primitiveTopology, "triangle-strip");
        assert.equal(submission.startVertex, 0);
        assert.equal(submission.primitiveCount, 2);
        assert.equal(submission.vertexCount, 4);
        assert.equal(
          submission.flexibleVertexFormat,
          "xyz-rhw-diffuse-texture1",
        );
        assert.deepEqual(submission.streamSource, {
          vertexBufferId: 3,
          offsetInBytes: 0,
          stride: 28,
        });
        assert.equal(submission.vertexBufferContentGeneration, 1);
        assert.equal(submission.vertexData.length, 112);
        assert.equal(
          hash(Buffer.from(submission.vertexData)),
          "405bea74541d2aece445d0dac693da55fb404e777c91ff81d87d4c6272581362",
        );
        assert.equal(submission.texture.textureId, 8);
        assert.equal(submission.texture.generation, 45);
        assert.equal(submission.texture.content.contentGeneration, 1);
        assert.equal(device.renderer, undefined);
        assert.equal(draw.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          draw.drawPrimitiveProfileId,
          "primary-first-frame-triangle-strip-quad-draw-v1",
        );
        assert.equal(
          draw.stateAuthority,
          "rust-runtime-graphics-logical-device-back-buffer-draw-submissions",
        );
        assert.equal(
          draw.vertexByteAuthority,
          "rust-guest-memory-synchronous-immutable-snapshot",
        );
        assert.equal(
          draw.mutation,
          "validate-plan-read-preflight-targeted-commit",
        );
        assert.equal(draw.supportedPrimitiveTopology, "triangle-strip");
        assert.equal(draw.supportedStartVertex, 0);
        assert.equal(draw.supportedPrimitiveCount, 2);
        assert.equal(draw.capturedVertexBytes, 112);
        assert.equal(draw.pendingSubmissionLimit, 4_096);
        assert.equal(draw.pendingVertexBytesLimit, 1_048_576);
        assert.equal(draw.clearDiscardsPendingSubmissions, true);
        assert.equal(draw.failureAtomicity, true);
        assert.equal(draw.browserRendererResourceCreated, false);
        assert.equal(draw.visiblePixelsProduced, false);
      } else {
        assert.equal(evidence.report.d3d9DrawPrimitive, null);
      }
      if (d3d9EndSceneProfile) {
        const endScene = evidence.report.d3d9EndScene;
        assert(
          endScene !== null,
          "Chrome Worker omitted D3D9 EndScene evidence",
        );
        const call = endScene.call;
        assert.equal(call.sequence, 127_375);
        assert.equal(call.symbol, "IDirect3DDevice9.EndScene");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 42);
        assert.deepEqual(call.arguments, [0x0101_0004]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b43b);
        assert.equal(call.stackPointerBefore, 0x006f_f260);
        assert.equal(call.stackPointerAfter, 0x006f_f268);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        const device = call.graphicsDevice;
        assert(
          device,
          "Chrome Worker omitted the ended Rust graphics snapshot",
        );
        assert.equal(device.activeScene, undefined);
        assert.equal(device.generation, 58);
        assert.equal(device.backBuffer.contentGeneration, 2);
        assert.equal(device.backBuffer.drawSubmissions?.length, 1);
        assert.equal(device.backBuffer.drawSubmissions?.[0]?.sequence, 1);
        assert.equal(device.backBuffer.drawSubmissions?.[0]?.sceneSequence, 1);
        assert.equal(device.renderer, undefined);
        assert.equal(endScene.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          endScene.endSceneProfileId,
          "primary-first-frame-scene-end-v1",
        );
        assert.equal(
          endScene.sceneAuthority,
          "rust-runtime-graphics-logical-device-active-scene",
        );
        assert.equal(endScene.lifecycle, "active-to-inactive");
        assert.equal(
          endScene.mutation,
          "validate-plan-preflight-targeted-commit",
        );
        assert.equal(endScene.preservesDrawSubmissions, true);
        assert.equal(endScene.repeatedEndReturnsInvalidCall, true);
        assert.equal(endScene.failureAtomicity, true);
        assert.equal(endScene.browserRendererResourceCreated, false);
        assert.equal(endScene.visiblePixelsProduced, false);
      } else {
        assert.equal(evidence.report.d3d9EndScene, null);
      }
      if (d3d9PostEndSceneRasterStatusProfile) {
        const rasterStatus = evidence.report.d3d9PostEndSceneRasterStatus;
        assert(
          rasterStatus !== null,
          "Chrome Worker omitted post-EndScene D3D9 GetRasterStatus evidence",
        );
        const call = rasterStatus.call;
        assert.equal(call.sequence, 127_376);
        assert.equal(call.symbol, "IDirect3DDevice9.GetRasterStatus");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 19);
        assert.deepEqual(call.arguments, [0x0101_0004, 0, 0x006f_f280]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_aedd);
        assert.equal(call.stackPointerBefore, 0x006f_f268);
        assert.equal(call.stackPointerAfter, 0x006f_f278);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, [
          {
            address: 0x006f_f280,
            bytes: [0, 0, 0, 0, 0x37, 0x04, 0, 0],
          },
        ]);
        assert.equal(call.graphicsDevice, undefined);
        assert.equal(rasterStatus.cumulativeCompletions, 784);
        assert.equal(
          rasterStatus.compatibilityPolicyId,
          d3d9CompatibilityPolicyId,
        );
        assert.equal(
          rasterStatus.rasterStatusProfileId,
          "primary-stable-last-visible-scanline-v2",
        );
        assert.equal(
          rasterStatus.displayProfileId,
          "single-monitor-1920x1080-work-1920x1032-v2",
        );
        assert.equal(
          rasterStatus.stateAuthority,
          "rust-d3d9-compatibility-policy",
        );
        assert.equal(rasterStatus.outputBytes, 8);
        assert.equal(rasterStatus.inVBlank, false);
        assert.equal(rasterStatus.scanLine, 1_079);
        assert.equal(rasterStatus.runtimeStateMutation, false);
        assert.equal(rasterStatus.referenceCountMutation, false);
        assert.equal(rasterStatus.lastErrorPolicy, "preserve");
        assert.equal(rasterStatus.guestWriteAtomic, true);
        assert.equal(rasterStatus.hostScanoutImported, false);
        assert.equal(rasterStatus.browserScanoutImported, false);
        assert.equal(rasterStatus.browserRendererResourceCreated, false);
        assert.equal(rasterStatus.visiblePixelsProduced, false);
      } else {
        assert.equal(evidence.report.d3d9PostEndSceneRasterStatus, null);
      }
      if (d3d9PresentProfile) {
        const present = evidence.report.d3d9Present;
        assert(present !== null, "Chrome Worker omitted D3D9 Present evidence");
        const call = present.call;
        assert.equal(call.sequence, 127_377);
        assert.equal(call.symbol, "IDirect3DDevice9.Present");
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 17);
        assert.deepEqual(call.arguments, [0x0101_0004, 0, 0, 0, 0]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 0x1005_b7b1);
        assert.equal(call.stackPointerBefore, 0x006f_f280);
        assert.equal(call.stackPointerAfter, 0x006f_f298);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        const device = call.graphicsDevice;
        assert(
          device,
          "Chrome Worker omitted the presented Rust graphics snapshot",
        );
        assert.equal(device.activeScene, undefined);
        assert.equal(device.generation, 59);
        assert.equal(device.presentedFrame?.sequence, 1);
        assert.equal(device.presentedFrame?.backBuffer.contentGeneration, 2);
        assert.equal(
          device.presentedFrame?.backBuffer.drawSubmissions?.length,
          1,
        );
        assert.equal(device.backBuffer.contentGeneration, 3);
        assert.equal(device.backBuffer.uniformClear, undefined);
        assert.equal(device.backBuffer.drawSubmissions, undefined);
        assert.equal(device.renderer, undefined);
        const receipt = call.browserPresentation;
        assert(receipt, "Chrome Worker omitted the WebGL2 Present receipt");
        assert.equal(receipt.backend, "webgl2-offscreen-canvas");
        assert.deepEqual(receipt.resource, {
          id: "browser-resource:1",
          kind: "d3d9-webgl2-device",
          binding: {
            objectId: "21",
            interfaceId: "42",
            interfaceName: "IDirect3DDevice9",
          },
        });
        assert.equal(receipt.browserResourceCount, 1);
        assert.equal(receipt.deviceId, 3);
        assert.equal(receipt.deviceGeneration, 59);
        assert.equal(receipt.frameSequence, 1);
        assert.equal(receipt.presentedContentGeneration, 2);
        assert.equal(receipt.discardedBackBufferContentGeneration, 3);
        assert.equal(receipt.width, 1_280);
        assert.equal(receipt.height, 720);
        assert.equal(receipt.drawSubmissionCount, 1);
        assert.equal(receipt.uploadedTextureBytes, 8_388_608);
        assert.equal(receipt.uploadedVertexBytes, 112);
        assert.deepEqual(receipt.centerPixelRgba, [0, 0, 0, 255]);
        assert.equal(receipt.webGlError, 0);
        assert.equal(receipt.capacityPreflightBeforeGuestExecution, true);
        assert.equal(receipt.bindingPublishedAfterSnapshotValidation, true);
        assert.equal(receipt.renderedBeforeGuestResume, true);
        assert.equal(present.compatibilityPolicyId, d3d9CompatibilityPolicyId);
        assert.equal(
          present.presentProfileId,
          "primary-first-frame-full-back-buffer-present-v1",
        );
        assert.equal(
          present.frameAuthority,
          "rust-runtime-graphics-logical-device-presented-frame",
        );
        assert.equal(
          present.browserResourceAuthority,
          "typescript-browser-resource-table",
        );
        assert.equal(
          present.mutation,
          "validate-plan-preflight-targeted-commit",
        );
        assert.equal(present.presentMode, "discard");
        assert.equal(present.sourceRectImplemented, false);
        assert.equal(present.destinationRectImplemented, false);
        assert.equal(present.destinationWindowOverrideImplemented, false);
        assert.equal(present.dirtyRegionImplemented, false);
        assert.equal(present.browserRendererResourceCreated, true);
        assert.equal(
          present.browserCapacityPreflightBeforeGuestExecution,
          true,
        );
        assert.equal(
          present.browserBindingPublishedAfterSnapshotValidation,
          true,
        );
        assert.equal(present.visiblePixelsProduced, true);
        assert.equal(present.asynchronousDeviceLossRecoveryImplemented, false);
        assert.equal(present.failureAtomicity, true);
      } else {
        assert.equal(evidence.report.d3d9Present, null);
      }
      if (d3d9PostPresentContinuationProfile) {
        const continuation = evidence.report.postPresentContinuation;
        assert(
          continuation !== null,
          "Chrome Worker omitted the post-Present continuation evidence",
        );
        assert.deepEqual(continuation.timeGetTimeCall, {
          sequence: 127_378,
          module: "winmm.dll",
          symbol: "timeGetTime",
          arguments: [],
          result: { action: "set", value: 3_601_007 },
          implementationId: "winmm.time-get-time.v1",
          returnAddress: 0x1009_1ebf,
          stackPointerBefore: 0x006f_f290,
          stackPointerAfter: 0x006f_f294,
          lastError: 0,
          guestWrites: [],
          ...(postHeapTimeGetTimeEvidenceProfile
            ? {
                tickCount: {
                  returnedTickCount: 3_601_007,
                  nextTickCount: 3_601_008,
                  incrementPerRead: 1,
                },
              }
            : {}),
        });
        assert.deepEqual(continuation.getForegroundWindowCall, {
          sequence: 127_379,
          module: "user32.dll",
          symbol: "GetForegroundWindow",
          arguments: [],
          result: { action: "set", value: 0x4801_0001 },
          implementationId: "user32.get-foreground-window.v1",
          returnAddress: 0x1009_390e,
          stackPointerBefore: 0x006f_f68c,
          stackPointerAfter: 0x006f_f690,
          lastError: 0,
          guestWrites: [],
        });
        assert.equal(continuation.updateWindowCallbackReturned, true);
        assert.equal(continuation.foregroundWindowQueryCompleted, true);
        assert.equal(
          continuation.stateAuthority,
          "rust-runtime-user32-thread-and-process-window-relations",
        );
        assert.equal(continuation.browserResourceCreated, false);
      } else {
        assert.equal(evidence.report.postPresentContinuation, null);
      }
      if (pointerMessageEvidenceProfile) {
        assert.deepEqual(evidence.report.keyboardInput, {
          receivedMessages: 0,
          queuedBeforePrepare: 0,
          receipts: [],
        });
        assert.equal(evidence.report.pointerInput.receivedMessages, 2);
        assert.equal(evidence.report.pointerInput.queuedBeforePrepare, 2);
        assert.equal(evidence.report.pointerInput.receipts.length, 2);
      } else if (inputProgressEvidenceProfile) {
        const inputVirtualKey = actionInputProgressGatewayProfile ? 0x01 : 0xa2;
        const downAffectedKeys = actionInputProgressGatewayProfile
          ? [
              {
                virtualKey: 0x01,
                down: true,
                pressedSinceLastQuery: true,
              },
            ]
          : [
              {
                virtualKey: 0x11,
                down: true,
                pressedSinceLastQuery: true,
              },
              {
                virtualKey: 0xa2,
                down: true,
                pressedSinceLastQuery: true,
              },
            ];
        const upAffectedKeys = actionInputProgressGatewayProfile
          ? [
              {
                virtualKey: 0x01,
                down: false,
                pressedSinceLastQuery: false,
              },
            ]
          : [
              {
                virtualKey: 0x11,
                down: false,
                pressedSinceLastQuery: false,
              },
              {
                virtualKey: 0xa2,
                down: false,
                pressedSinceLastQuery: true,
              },
            ];
        assert.deepEqual(evidence.report.keyboardInput, {
          receivedMessages: 2,
          queuedBeforePrepare: 2,
          receipts: [
            {
              sequence: 1,
              kind: "key-down",
              virtualKey: inputVirtualKey,
              repeat: false,
              sourceFocused: true,
              generation: 1,
              affectedKeys: downAffectedKeys,
            },
            {
              sequence: 2,
              kind: "key-up",
              virtualKey: inputVirtualKey,
              repeat: false,
              sourceFocused: true,
              generation: 3,
              affectedKeys: upAffectedKeys,
            },
          ],
        });
        assert.equal(evidence.report.getAsyncKeyState, null);
      } else if (user32AsyncKeyStateInputProfile) {
        assert.deepEqual(evidence.report.keyboardInput, {
          receivedMessages: 1,
          queuedBeforePrepare: 1,
          receipts: [
            {
              sequence: 1,
              kind: "key-down",
              virtualKey: 0xa2,
              repeat: false,
              sourceFocused: true,
              generation: 1,
              affectedKeys: [
                {
                  virtualKey: 0x11,
                  down: true,
                  pressedSinceLastQuery: true,
                },
                {
                  virtualKey: 0xa2,
                  down: true,
                  pressedSinceLastQuery: true,
                },
              ],
            },
          ],
        });
        const asyncKey = evidence.report.getAsyncKeyState;
        assert(
          asyncKey !== null,
          "Chrome Worker omitted GetAsyncKeyState input evidence",
        );
        assert.deepEqual(asyncKey.call, {
          sequence: 127_691,
          module: "user32.dll",
          symbol: "GetAsyncKeyState",
          arguments: [0x11],
          result: { action: "set", value: 0xffff_8001 },
          implementationId: "user32.get-async-key-state.v1",
          returnAddress: 0x1006_8405,
          stackPointerBefore: 0x006f_f53c,
          stackPointerAfter: 0x006f_f544,
          lastError: 0,
          guestWrites: [],
        });
        assert.equal(
          asyncKey.compatibilityPolicyId,
          "user32-single-monitor-window-frame-change-show-update-paint-window-state-v30",
        );
        assert.equal(
          asyncKey.keyboardProfileId,
          "foreground-focused-virtual-key-state-v1",
        );
        assert.deepEqual(
          asyncKey.inputEvent,
          evidence.report.keyboardInput.receipts[0],
        );
        assert.equal(asyncKey.stateAuthority, "rust-runtime-input-keyboard");
        assert.equal(
          asyncKey.browserSourceAuthority,
          "typescript-browser-input-adapter",
        );
        assert.equal(
          asyncKey.returnEncoding,
          "signed-short-high-bit-down-low-bit-pressed-since-query",
        );
        assert.equal(asyncKey.modifierAggregation, "side-specific-to-generic");
        assert.equal(
          asyncKey.pressedBitConsumption,
          "queried-virtual-key-only",
        );
        assert.equal(asyncKey.sourceFocusRequired, true);
        assert.equal(asyncKey.focusedForegroundWindowRequired, true);
        assert.equal(asyncKey.lastErrorPolicy, "preserve");
        assert.equal(asyncKey.guestMemoryMutation, false);
        assert.equal(asyncKey.failureAtomicity, true);
      } else {
        assert.deepEqual(evidence.report.keyboardInput, {
          receivedMessages: 0,
          queuedBeforePrepare: 0,
          receipts: [],
        });
        assert.equal(evidence.report.getAsyncKeyState, null);
      }
      if (user32AsyncKeyStateInitialScanProfile) {
        const getWindowRect = evidence.report.getWindowRect;
        const getCursorPos = evidence.report.getCursorPos;
        const scan = evidence.report.getAsyncKeyStateInitialScan;
        assert(
          getWindowRect !== null,
          "Chrome Worker omitted GetWindowRect evidence",
        );
        assert(
          getCursorPos !== null,
          "Chrome Worker omitted GetCursorPos evidence",
        );
        assert(
          scan !== null,
          "Chrome Worker omitted initial async-key scan evidence",
        );
        assert.deepEqual(
          getWindowRect.call.arguments.slice(0, 1),
          [0x4801_0001],
        );
        assert.deepEqual(getWindowRect.call.result, {
          action: "set",
          value: 1,
        });
        assert.equal(
          getWindowRect.call.implementationId,
          "user32.get-window-rect.v1",
        );
        assert.deepEqual(
          getWindowRect.call.guestWrites[0]?.bytes,
          [0x3d, 0x01, 0, 0, 0x9a, 0, 0, 0, 0x43, 0x06, 0, 0, 0x87, 0x03, 0, 0],
        );
        assert.equal(
          getWindowRect.windowAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(getWindowRect.browserGeometryImported, false);
        assert.equal(getWindowRect.failureAtomicity, true);
        assert.deepEqual(getCursorPos.call.result, { action: "set", value: 1 });
        assert.equal(
          getCursorPos.call.implementationId,
          "user32.get-cursor-pos.v1",
        );
        assert.deepEqual(
          getCursorPos.call.guestWrites[0]?.bytes,
          [0xc0, 0x03, 0, 0, 0x1c, 0x02, 0, 0],
        );
        assert.deepEqual(getCursorPos.cursorPosition, {
          profileId: "primary-display-center-cursor-v1",
          screenX: 960,
          screenY: 540,
          generation: 0,
        });
        assert.equal(
          getCursorPos.stateAuthority,
          "rust-runtime-input-cursor-position",
        );
        assert.equal(getCursorPos.browserPointerImported, false);
        assert.equal(getCursorPos.failureAtomicity, true);
        assert.equal(scan.leadingCallCount, 51);
        assert.deepEqual(
          scan.scanVirtualKeys,
          user32AsyncKeyStateInitialScanVirtualKeys,
        );
        assert.deepEqual(scan.firstCall.arguments, [14]);
        assert.deepEqual(scan.lastCall.arguments, [6]);
        assert.deepEqual(scan.firstCall.result, { action: "set", value: 0 });
        assert.deepEqual(scan.lastCall.result, { action: "set", value: 0 });
        assert.equal(
          scan.compatibilityPolicyId,
          defWindowProcAPointerReleaseContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-release-v35"
            : defWindowProcAActionContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-v34"
            : setFocusEvidenceProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-set-focus-v33"
            : pointerMessageEvidenceProfile
              ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-v32"
            : windowTimerCallbackDeliveryProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-delivery-v31"
            : "user32-single-monitor-window-frame-change-show-update-paint-window-state-v30",
        );
        assert.deepEqual(scan.cursorPosition, getCursorPos.cursorPosition);
        assert.equal(
          scan.cursorStateAuthority,
          "rust-runtime-input-cursor-position",
        );
        assert.equal(scan.inputEventCount, 0);
        assert.equal(scan.stateAuthority, "rust-runtime-input-keyboard");
        assert.equal(
          scan.browserSourceAuthority,
          "typescript-browser-input-adapter",
        );
        assert.equal(scan.guestMemoryMutation, false);
        assert.equal(scan.failureAtomicity, true);
      } else {
        assert.equal(evidence.report.getWindowRect, null);
        assert.equal(evidence.report.getCursorPos, null);
        assert.equal(evidence.report.getAsyncKeyStateInitialScan, null);
      }
      if (user32AsyncKeyStateInitialScanContinuationProfile) {
        const continuation = evidence.report.initialScanContinuation;
        assert(
          continuation !== null,
          "Chrome Worker omitted initial-scan continuation evidence",
        );
        assert.equal(continuation.prerequisiteScanCompletionSequence, 148_195);
        assert.equal(
          continuation.prerequisiteScanCompletionInstructionCount,
          147_769_226,
        );
        assert.deepEqual(
          continuation.calls.map((call) => ({
            sequence: call.sequence,
            implementationId: call.implementationId,
            arguments: call.arguments,
            result: call.result,
            returnAddress: call.returnAddress,
            stackPointerBefore: call.stackPointerBefore,
            stackPointerAfter: call.stackPointerAfter,
            lastError: call.lastError,
            guestWrites: call.guestWrites,
          })),
          [
            {
              sequence: 148_196,
              implementationId: "winmm.time-get-time.v1",
              arguments: [],
              result: { action: "set", value: 3_601_112 },
              returnAddress: 0x1009_1ebf,
              stackPointerBefore: 0x006f_f8f8,
              stackPointerAfter: 0x006f_f8fc,
              lastError: 0,
              guestWrites: [],
            },
            {
              sequence: 148_197,
              implementationId: "user32.set-rect.v1",
              arguments: [0x006f_f8cc, 0, 0, 1_280, 720],
              result: { action: "set", value: 1 },
              returnAddress: 0x1004_1533,
              stackPointerBefore: 0x006f_f8a8,
              stackPointerAfter: 0x006f_f8c0,
              lastError: 0,
              guestWrites: [
                {
                  address: 0x006f_f8cc,
                  bytes: [0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 208, 2, 0, 0],
                },
              ],
            },
          ],
        );
        assert.deepEqual(continuation.renderCompletionCounts, {
          clear: 2,
          setRect: 3,
          addDirtyRect: 2,
          updateTexture: 2,
          beginScene: 2,
          samplerState: 4,
          textureStageState: 12,
          setTexture: 2,
          setFvf: 2,
          setStreamSource: 2,
          drawPrimitive: 2,
          endScene: 2,
          present: 2,
          textureLockRect: 1,
          textureUnlockRect: 1,
          testCooperativeLevel: pointerMessageEvidenceProfile
            ? 8
            : inputProgressEvidenceProfile
            ? 16
            : windowTimerCallbackRecurringBatchProfile
              ? 62
            : presentationProgressGatewayProfile
              ? 8
            : runtimeThreadStackGrowthProfile
              ? 4
              : user32IsIconicProfile
                ? 3
                : 1,
        });
        assert.equal(
          continuation.nextSubsystemGateway,
          user32PeekMessageAProfile ? null : "user32-peek-message-a",
        );
        assert.equal(
          evidence.report.d3d9TextureLockRect?.lock.sequence,
          148_198,
        );
        assert.equal(
          evidence.report.d3d9TextureLockRect?.unlock.sequence,
          148_203,
        );
        assert.equal(
          evidence.report.d3d9TextureLockRect?.unlockWithoutLockSucceedsNoop,
          true,
        );
        assert.equal(
          evidence.report.d3d9PartialUpdateTexture?.call.sequence,
          148_272,
        );
        assert.deepEqual(
          evidence.report.d3d9PartialUpdateTexture?.dirtyRegion,
          { left: 0, top: 0, right: 1_280, bottom: 720 },
        );
        const cooperativeLevel = evidence.report.d3d9TestCooperativeLevel;
        assert(
          cooperativeLevel !== null,
          "Chrome Worker omitted TestCooperativeLevel evidence",
        );
        assert.equal(cooperativeLevel.call.sequence, 148_293);
        assert.equal(cooperativeLevel.instructionCount, 148_744_443);
        assert.deepEqual(cooperativeLevel.call.arguments, [0x0101_0004]);
        assert.deepEqual(cooperativeLevel.call.result, {
          kind: "h-result",
          value: 0,
        });
        assert.equal(cooperativeLevel.call.returnAddress, 0x1005_adcd);
        assert.equal(cooperativeLevel.call.stackPointerBefore, 0x006f_f8f4);
        assert.equal(cooperativeLevel.call.stackPointerAfter, 0x006f_f8fc);
        assert.equal(cooperativeLevel.call.referenceCount, 5);
        assert.equal(cooperativeLevel.call.objectRetired, false);
        assert.deepEqual(cooperativeLevel.call.guestWrites, []);
        assert.equal(
          cooperativeLevel.compatibilityPolicyId,
          "d3d9-primary-virtual-adapter-v32",
        );
        assert.equal(
          cooperativeLevel.profileId,
          "primary-live-operational-device-query-v1",
        );
        assert.equal(cooperativeLevel.queryResult, "operational-s-ok");
        assert.equal(cooperativeLevel.mutation, "none");
        assert.equal(cooperativeLevel.lastErrorPolicy, "preserve");
        assert.equal(cooperativeLevel.guestMemoryMutation, false);
        assert.equal(cooperativeLevel.referenceCountMutation, false);
        assert.equal(cooperativeLevel.failureAtomicity, true);
        assert.equal(cooperativeLevel.deviceLostStateImplemented, false);
        assert.equal(cooperativeLevel.deviceNotResetStateImplemented, false);
        assert.equal(cooperativeLevel.resetImplemented, false);
        assert.equal(cooperativeLevel.browserContextLossImported, false);
      } else {
        assert.equal(evidence.report.initialScanContinuation, null);
        assert.equal(evidence.report.d3d9TestCooperativeLevel, null);
      }
      if (user32PeekMessageAProfile) {
        const peek = evidence.report.peekMessageA;
        assert(peek !== null, "Chrome Worker omitted PeekMessageA evidence");
        assert.equal(
          peek.calls.length,
          windowTimerCallbackRecurringBatchProfile
            ? 77
            : peekMessageAPostPointerReleaseContinuationProfile
              ? 12
            : peekMessageAPointerReleaseContinuationProfile
              ? 11
            : pointerMessageEvidenceProfile
              ? 10
            : windowTimerCallbackDeliveryGatewayProfile
            ? 1
            : windowTimerCallbackActionInputProgressGatewayProfile
            ? 20
            : inputProgressEvidenceProfile
            ? 16
            : presentationProgressGatewayProfile
              ? 8
            : runtimeThreadStackGrowthProfile
              ? 4
              : user32IsIconicProfile
                ? 3
                : 1,
        );
        assert.deepEqual(
          peek.calls.map((call) => call.sequence),
          windowTimerCallbackRecurringBatchProfile
            ? [
                148_296, 148_300, 148_357, 148_478, 148_603, 161_543,
                161_547, 161_678, 161_809, 161_940, 162_070, 162_074,
                162_204, 162_334, 162_464, 162_522, 162_598, 162_656,
                162_786, 162_916, 163_046, 163_050, 163_180, 163_310,
                163_440, 163_570, 163_574, 163_632, 163_762, 163_892,
                164_022, 164_026, 164_156, 164_286, 164_416, 164_546,
                164_550, 164_680, 164_738, 164_868, 164_998, 165_128,
                165_132, 165_262, 165_392, 165_522, 165_652, 165_656,
                165_714, 165_844, 165_974, 166_104, 166_108, 166_238,
                166_368, 166_498, 166_628, 166_632, 166_762, 166_820,
                166_950, 167_080, 167_084, 167_214, 167_344, 167_474,
                167_604, 167_608, 167_738, 167_796, 167_926, 168_056,
                168_060, 168_190, 168_320, 168_450, 168_580,
              ]
            : peekMessageAPointerReleaseContinuationProfile
              ? [
                  148_296, 148_300, 148_357, 148_478, 148_603, 161_543,
                  161_547, 161_678, 161_809, 161_940, 161_949,
                  ...(peekMessageAPostPointerReleaseContinuationProfile
                    ? [161_953]
                    : []),
                ]
            : pointerMessageEvidenceProfile
              ? [
                  148_296, 148_300, 148_357, 148_478, 148_603, 161_543,
                  161_547, 161_678, 161_809, 161_940,
                ]
            : windowTimerCallbackActionInputProgressGatewayProfile
            ? [
                148_296, 148_300, 148_357, 148_478, 148_603, 161_543,
                161_547, 161_678, 161_809, 161_940, 162_070, 162_074,
                162_204, 162_334, 162_464, 162_522, 162_598, 162_656,
                162_786, 162_916,
              ]
            : inputProgressEvidenceProfile
            ? [
                148_296, 148_353, 148_474, 148_599, 161_539, 161_670, 161_801,
                161_932, 162_062, 162_192, 162_322, 162_452, 162_510, 162_640,
                162_770, 162_900,
              ]
            : presentationProgressGatewayProfile
              ? [
                148_296, 148_353, 148_474, 148_599, 161_539, 161_670, 161_801,
                161_932,
              ]
            : runtimeThreadStackGrowthProfile
              ? [148_296, 148_353, 148_474, 148_599]
              : user32IsIconicProfile
                ? [148_296, 148_353, 148_474]
                : [148_296],
        );
        assert.deepEqual(
          peek.instructionCounts,
          windowTimerCallbackRecurringBatchProfile
            ? [
                148_744_543, 148_745_210, 148_749_776, 148_771_454,
                153_196_988, 321_443_712, 321_444_379, 321_450_280,
                321_456_163, 321_462_072, 321_467_946, 321_468_613,
                321_474_488, 321_480_362, 321_486_236, 321_490_787,
                321_492_777, 321_497_408, 321_503_282, 321_509_156,
                321_515_030, 321_515_697, 321_521_572, 321_527_446,
                321_533_320, 321_539_194, 321_539_861, 321_544_413,
                321_550_287, 321_556_240, 321_562_114, 321_562_781,
                321_568_656, 321_574_530, 321_580_404, 321_586_278,
                321_586_945, 321_592_820, 321_597_371, 321_603_245,
                321_609_198, 321_615_072, 321_615_739, 321_621_614,
                321_627_488, 321_633_362, 321_639_236, 321_639_903,
                321_644_455, 321_650_329, 321_656_203, 321_662_077,
                321_662_744, 321_668_698, 321_674_572, 321_680_446,
                321_686_320, 321_686_987, 321_692_862, 321_697_413,
                321_703_287, 321_709_161, 321_709_828, 321_715_703,
                321_721_656, 321_727_530, 321_733_404, 321_734_071,
                321_739_946, 321_744_497, 321_750_371, 321_756_245,
                321_756_912, 321_762_787, 321_768_661, 321_774_535,
                321_780_488,
              ]
            : peekMessageAPointerReleaseContinuationProfile
              ? [
                  148_744_543, 148_745_210, 148_749_776, 148_771_454,
                  153_196_988, 321_443_712, 321_444_379, 321_450_280,
                  321_456_163, 321_462_072, 321_462_423,
                  ...(peekMessageAPostPointerReleaseContinuationProfile
                    ? [321_462_521]
                    : []),
                ]
            : pointerMessageEvidenceProfile
              ? [
                  148_744_543, 148_745_210, 148_749_776, 148_771_454,
                  153_196_988, 321_443_712, 321_444_379, 321_450_280,
                  321_456_163, 321_462_072,
                ]
            : windowTimerCallbackActionInputProgressGatewayProfile
            ? [
                148_744_543, 148_745_210, 148_749_776, 148_771_454,
                153_196_988, 321_443_712, 321_444_379, 321_450_280,
                321_456_163, 321_462_072, 321_467_946, 321_468_613,
                321_474_488, 321_480_362, 321_486_236, 321_490_787,
                321_492_777, 321_497_408, 321_503_282, 321_509_156,
              ]
            : actionInputProgressGatewayProfile
            ? [
                148_744_543, 148_749_108, 148_770_786, 153_196_320,
                321_443_044, 321_448_944, 321_454_827, 321_460_736,
                321_466_610, 321_472_484, 321_478_358, 321_484_232,
                321_488_783, 321_494_657, 321_500_610, 321_506_484,
              ]
            : inputProgressGatewayProfile
              ? [
                148_744_543, 148_749_108, 148_770_786, 153_196_320,
                321_443_044, 321_448_944, 321_454_827, 321_460_734,
                321_466_608, 321_472_482, 321_478_356, 321_484_230,
                321_488_781, 321_494_655, 321_500_608, 321_506_482,
              ]
            : presentationProgressGatewayProfile
              ? [
                148_744_543, 148_749_108, 148_770_786, 153_196_320, 321_443_044,
                321_448_944, 321_454_827, 321_460_736,
              ]
            : runtimeThreadStackGrowthProfile
              ? [148_744_543, 148_749_108, 148_770_786, 153_196_320]
              : user32IsIconicProfile
                ? [148_744_543, 148_749_108, 148_770_786]
                : [148_744_543],
        );
        const firstPeek = peek.calls[0];
        assert.deepEqual(
          {
            sequence: firstPeek.sequence,
            instructionCount: peek.instructionCounts[0],
            arguments: firstPeek.arguments,
            result: firstPeek.result,
            implementationId: firstPeek.implementationId,
            returnAddress: firstPeek.returnAddress,
            stackPointerBefore: firstPeek.stackPointerBefore,
            stackPointerAfter: firstPeek.stackPointerAfter,
            lastError: firstPeek.lastError,
            guestWrites: firstPeek.guestWrites,
          },
          {
            sequence: 148_296,
            instructionCount: 148_744_543,
            arguments: [0x006f_f8e8, 0, 0, 0, 1],
            result: {
              action: "set",
              value: windowTimerCallbackDeliveryProfile ? 1 : 0,
            },
            implementationId: "user32.peek-message-a.current-thread.v1",
            returnAddress: 0x1009_378b,
            stackPointerBefore: 0x006f_f8c8,
            stackPointerAfter: 0x006f_f8e0,
            lastError: 0,
            guestWrites: windowTimerCallbackDeliveryProfile
              ? [
                  {
                    address: 0x006f_f8e8,
                    bytes: [
                      0x01, 0x00, 0x01, 0x48, 0x13, 0x01, 0x00, 0x00, 0x01,
                      0x00, 0x00, 0x00, 0x00, 0x96, 0x09, 0x10, 0xda, 0xf2,
                      0x36, 0x00, 0xc0, 0x03, 0x00, 0x00, 0x1c, 0x02, 0x00,
                      0x00,
                    ],
                  },
                ]
              : [],
          },
        );
        for (const completion of peek.calls) {
          assert.deepEqual(completion.arguments, [0x006f_f8e8, 0, 0, 0, 1]);
          const messageDelivered =
            completion.result.action === "set" &&
            completion.result.value === 1;
          assert.deepEqual(completion.result, {
            action: "set",
            value: messageDelivered ? 1 : 0,
          });
          assert.equal(
            completion.implementationId,
            "user32.peek-message-a.current-thread.v1",
          );
          assert.equal(
            completion.stackPointerAfter,
            completion.stackPointerBefore + 24,
          );
          assert.equal(completion.lastError, 0);
          assert.equal(
            completion.guestWrites.length,
            messageDelivered ? 1 : 0,
          );
          const messageWrite = completion.guestWrites[0];
          const messageBytes =
            messageWrite !== undefined && "bytes" in messageWrite
              ? messageWrite.bytes
              : null;
          if (messageDelivered) {
            assert.equal(messageWrite?.address, completion.arguments[0]);
            assert.equal(messageBytes?.length, 28);
          }
          const completedMessage =
            messageBytes?.length === 28
              ? (messageBytes[4] |
                  (messageBytes[5] << 8) |
                  (messageBytes[6] << 16) |
                  (messageBytes[7] << 24)) >>>
                0
              : null;
          assert(
            completedMessage === null ||
              completedMessage === 0x0113 ||
              completedMessage === 0x0201 ||
              completedMessage === 0x0202,
            `PeekMessageA returned an unexpected message ${completedMessage}`,
          );
          assert.equal(messageDelivered, completedMessage !== null);
          assert.equal(
            completion.windowTimer !== undefined,
            completedMessage === 0x0113,
            "only WM_TIMER PeekMessageA completion may carry windowTimer metadata",
          );
        }
        assert.equal(
          peek.compatibilityPolicyId,
          defWindowProcAPointerReleaseContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-release-v35"
            : defWindowProcAActionContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-v34"
            : setFocusEvidenceProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-set-focus-v33"
            : pointerMessageActionProgressGatewayProfile
              ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-v32"
            : windowTimerCallbackDeliveryProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-delivery-v31"
            : "user32-single-monitor-window-frame-change-show-update-paint-window-state-v30",
        );
        assert.equal(
          peek.profileId,
          windowTimerCallbackDeliveryProfile
            ? "current-thread-unfiltered-peek-message-a-v1"
            : "current-thread-unfiltered-peek-message-a-v1",
        );
        assert.equal(peek.queueAuthority, "rust-runtime-thread-message-queue");
        assert.equal(
          peek.emptyQueueResult,
          !windowTimerCallbackDeliveryProfile,
        );
        assert.equal(
          peek.outputMutation,
          windowTimerCallbackDeliveryProfile
            ? "atomic-materialized-msg32-write"
            : "none-on-empty",
        );
        assert.equal(
          peek.removal,
          windowTimerCallbackDeliveryProfile
            ? "pm-remove-after-materialization"
            : "none-on-empty",
        );
        assert.equal(peek.lastErrorPolicy, "preserve");
        assert.equal(peek.failureAtomicity, true);
        assert.equal(peek.filteredRetrievalImplemented, false);
        assert.equal(peek.crossThreadRetrievalImplemented, false);
      } else {
        assert.equal(evidence.report.peekMessageA, null);
      }
      if (windowTimerCallbackDeliveryGatewayProfile) {
        const iconic = evidence.report.isIconic;
        assert(iconic !== null, "Chrome Worker omitted empty IsIconic scope evidence");
        assert.deepEqual(iconic.calls, []);
        assert.deepEqual(iconic.instructionCounts, []);
        assert.equal(
          iconic.compatibilityPolicyId,
          "user32-single-monitor-window-frame-change-show-update-paint-window-timer-delivery-v31",
        );
      } else if (user32IsIconicProfile) {
        const iconic = evidence.report.isIconic;
        assert(iconic !== null, "Chrome Worker omitted IsIconic evidence");
        assert.equal(
          iconic.calls.length,
          windowTimerCallbackRecurringBatchProfile
            ? 61
            : isIconicPostPointerReleaseContinuationProfile
              ? 8
            : pointerMessageEvidenceProfile
              ? 7
            : inputProgressEvidenceProfile
            ? 16
            : presentationProgressGatewayProfile
              ? 8
              : 3,
        );
        assert.deepEqual(
          iconic.calls.map((call) => call.sequence),
          windowTimerCallbackRecurringBatchProfile
            ? [
                148_309, 148_430, 148_487, 161_495, 161_630, 161_688,
                161_819, 161_950, 162_084, 162_214, 162_344, 162_474,
                162_608, 162_738, 162_796, 162_926, 163_060, 163_190,
                163_320, 163_450, 163_584, 163_714, 163_772, 163_902,
                164_036, 164_166, 164_296, 164_426, 164_560, 164_690,
                164_820, 164_878, 165_008, 165_142, 165_272, 165_402,
                165_532, 165_666, 165_796, 165_854, 165_984, 166_118,
                166_248, 166_378, 166_508, 166_642, 166_772, 166_902,
                166_960, 167_094, 167_224, 167_354, 167_484, 167_618,
                167_748, 167_878, 167_936, 168_070, 168_200, 168_330,
                168_460,
              ]
            : isIconicPostPointerReleaseContinuationProfile
              ? [
                  148_309, 148_430, 148_487, 161_495, 161_630, 161_688,
                  161_819, 161_963,
                ]
            : pointerMessageEvidenceProfile
              ? [
                  148_309, 148_430, 148_487, 161_495, 161_630, 161_688,
                  161_819,
                ]
            : windowTimerCallbackActionInputProgressGatewayProfile
            ? [
                148_309, 148_430, 148_487, 161_495, 161_630, 161_688,
                161_819, 161_950, 162_084, 162_214, 162_344, 162_474,
                162_608, 162_738, 162_796, 162_926,
              ]
            : inputProgressEvidenceProfile
            ? [
                148_305, 148_426, 148_483, 161_491, 161_622, 161_680, 161_811,
                161_942, 162_072, 162_202, 162_332, 162_462, 162_592, 162_650,
                162_780, 162_910,
              ]
            : presentationProgressGatewayProfile
              ? [
                148_305, 148_426, 148_483, 161_491, 161_622, 161_680, 161_811,
                161_942,
              ]
            : [148_305, 148_426, 148_483],
        );
        assert.deepEqual(
          iconic.instructionCounts,
          windowTimerCallbackRecurringBatchProfile
            ? [
                148_745_985, 148_767_663, 148_772_228, 321_439_921,
                321_446_489, 321_451_040, 321_456_923, 321_462_832,
                321_469_374, 321_475_248, 321_481_122, 321_486_996,
                321_493_617, 321_499_491, 321_504_042, 321_509_916,
                321_516_458, 321_522_332, 321_528_206, 321_534_080,
                321_540_622, 321_546_496, 321_551_126, 321_557_000,
                321_563_542, 321_569_416, 321_575_290, 321_581_164,
                321_587_706, 321_593_580, 321_599_454, 321_604_084,
                321_609_958, 321_616_500, 321_622_374, 321_628_248,
                321_634_122, 321_640_664, 321_646_538, 321_651_089,
                321_656_963, 321_663_584, 321_669_458, 321_675_332,
                321_681_206, 321_687_748, 321_693_622, 321_699_496,
                321_704_047, 321_710_589, 321_716_542, 321_722_416,
                321_728_290, 321_734_832, 321_740_706, 321_746_580,
                321_751_131, 321_757_673, 321_763_547, 321_769_421,
                321_775_374,
              ]
            : isIconicPostPointerReleaseContinuationProfile
              ? [
                  148_745_985, 148_767_663, 148_772_228, 321_439_921,
                  321_446_489, 321_451_040, 321_456_923, 321_463_282,
                ]
            : pointerMessageEvidenceProfile
              ? [
                  148_745_985, 148_767_663, 148_772_228, 321_439_921,
                  321_446_489, 321_451_040, 321_456_923,
                ]
            : windowTimerCallbackActionInputProgressGatewayProfile
            ? [
                148_745_985, 148_767_663, 148_772_228, 321_439_921,
                321_446_489, 321_451_040, 321_456_923, 321_462_832,
                321_469_374, 321_475_248, 321_481_122, 321_486_996,
                321_493_617, 321_499_491, 321_504_042, 321_509_916,
              ]
            : actionInputProgressGatewayProfile
            ? [
                148_745_317, 148_766_995, 148_771_560, 321_439_253,
                321_445_153, 321_449_704, 321_455_587, 321_461_496,
                321_467_370, 321_473_244, 321_479_118, 321_484_992,
                321_490_866, 321_495_496, 321_501_370, 321_507_244,
              ]
            : inputProgressGatewayProfile
              ? [
                148_745_317, 148_766_995, 148_771_560, 321_439_253,
                321_445_153, 321_449_704, 321_455_587, 321_461_494,
                321_467_368, 321_473_242, 321_479_116, 321_484_990,
                321_490_864, 321_495_494, 321_501_368, 321_507_242,
              ]
            : presentationProgressGatewayProfile
              ? [
                148_745_317, 148_766_995, 148_771_560, 321_439_253, 321_445_153,
                321_449_704, 321_455_587, 321_461_496,
              ]
            : [148_745_317, 148_766_995, 148_771_560],
        );
        const firstIconic = iconic.calls[0];
        assert.deepEqual(
          {
            sequence: firstIconic.sequence,
            instructionCount: iconic.instructionCounts[0],
            arguments: firstIconic.arguments,
            result: firstIconic.result,
            implementationId: firstIconic.implementationId,
            returnAddress: firstIconic.returnAddress,
            stackPointerBefore: firstIconic.stackPointerBefore,
            stackPointerAfter: firstIconic.stackPointerAfter,
            lastError: firstIconic.lastError,
            guestWrites: firstIconic.guestWrites,
          },
          {
            sequence:
              windowTimerCallbackRecurringBatchProfile ||
              windowTimerCallbackActionInputProgressGatewayProfile ||
              pointerMessageEvidenceProfile
              ? 148_309
              : 148_305,
            instructionCount:
              windowTimerCallbackRecurringBatchProfile ||
              windowTimerCallbackActionInputProgressGatewayProfile ||
              pointerMessageEvidenceProfile
              ? 148_745_985
              : 148_745_317,
            arguments: [0x4801_0001],
            result: { action: "set", value: 0 },
            implementationId: "user32.is-iconic.logical-window-style.v1",
            returnAddress: 0x1008_89c4,
            stackPointerBefore: 0x006f_f8d8,
            stackPointerAfter: 0x006f_f8e0,
            lastError: 0,
            guestWrites: [],
          },
        );
        for (const completion of iconic.calls) {
          assert.deepEqual(completion.arguments, [0x4801_0001]);
          assert.deepEqual(completion.result, { action: "set", value: 0 });
          assert.equal(
            completion.implementationId,
            "user32.is-iconic.logical-window-style.v1",
          );
          assert.equal(
            completion.stackPointerAfter,
            completion.stackPointerBefore + 8,
          );
          assert.equal(completion.lastError, 0);
          assert.deepEqual(completion.guestWrites, []);
        }
        assert.equal(
          iconic.compatibilityPolicyId,
          defWindowProcAPointerReleaseContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-release-v35"
            : defWindowProcAActionContinuationProfile
            ? "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-v34"
            : setFocusEvidenceProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-set-focus-v33"
            : pointerMessageActionProgressGatewayProfile
              ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-v32"
            : windowTimerCallbackDeliveryProfile
            ? "user32-single-monitor-window-frame-change-show-update-paint-window-timer-delivery-v31"
            : "user32-single-monitor-window-frame-change-show-update-paint-window-state-v30",
        );
        assert.equal(
          iconic.profileId,
          "logical-window-effective-style-is-iconic-v1",
        );
        assert.equal(
          iconic.windowStateAuthority,
          "rust-runtime-user32-logical-window",
        );
        assert.equal(iconic.minimizedStyleMask, 0x2000_0000);
        assert.equal(iconic.normalWindowResult, false);
        assert.equal(iconic.validLastErrorPolicy, "preserve");
        assert.equal(iconic.invalidWindowLastError, 1400);
        assert.equal(iconic.guestMemoryMutation, false);
        assert.equal(iconic.runtimeStateMutation, false);
        assert.equal(iconic.failureAtomicity, true);
        assert.equal(iconic.minimizeTransitionImplemented, false);
      } else {
        assert.equal(evidence.report.isIconic, null);
      }
      if (d3d9RecreatedTexturesProfile) {
        const recreated = evidence.report.d3d9RecreatedTextures;
        assert(
          recreated !== null,
          "Chrome Worker omitted D3D9 recreated texture evidence",
        );
        assert.equal(
          recreated.compatibilityPolicyId,
          d3d9CompatibilityPolicyId,
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
    threadId: growth.threadId,
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
    0x09, 0x00, 0x01, 0x47, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0xd0, 0x02, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
      sequence: snapshot.call.sequence,
      module: snapshot.call.module.toLowerCase(),
      symbol: snapshot.call.symbol,
      occurrence: snapshot.call.occurrence,
      trapInstructionCount: snapshot.call.trapInstructionCount,
      arguments: snapshot.call.arguments,
      pendingCallback: true,
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

function normalizePostPresentContinuation(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const normalizeCall = (call) => ({
    ...normalizeSemanticCall(call),
    implementationId: call.implementationId,
  });
  return {
    timeGetTimeCall: normalizeCall(snapshot.timeGetTimeCall),
    getForegroundWindowCall: normalizeCall(snapshot.getForegroundWindowCall),
    updateWindowCallbackReturned: snapshot.updateWindowCallbackReturned,
    foregroundWindowQueryCompleted: snapshot.foregroundWindowQueryCompleted,
    stateAuthority: snapshot.stateAuthority,
    browserResourceCreated: snapshot.browserResourceCreated,
  };
}

function normalizeGetAsyncKeyState(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      implementationId: snapshot.call.implementationId,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    keyboardProfileId: snapshot.keyboardProfileId,
    inputEvent: snapshot.inputEvent,
    stateAuthority: snapshot.stateAuthority,
    browserSourceAuthority: snapshot.browserSourceAuthority,
    returnEncoding: snapshot.returnEncoding,
    modifierAggregation: snapshot.modifierAggregation,
    pressedBitConsumption: snapshot.pressedBitConsumption,
    sourceFocusRequired: snapshot.sourceFocusRequired,
    focusedForegroundWindowRequired: snapshot.focusedForegroundWindowRequired,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    guestMemoryMutation: snapshot.guestMemoryMutation,
    failureAtomicity: snapshot.failureAtomicity,
  };
}

function normalizeGetWindowRect(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      implementationId: snapshot.call.implementationId,
    },
    windowAuthority: snapshot.windowAuthority,
    coordinateSpace: snapshot.coordinateSpace,
    browserGeometryImported: snapshot.browserGeometryImported,
    failureAtomicity: snapshot.failureAtomicity,
  };
}

function normalizeGetCursorPos(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      implementationId: snapshot.call.implementationId,
    },
    cursorPosition: snapshot.cursorPosition,
    stateAuthority: snapshot.stateAuthority,
    browserPointerImported: snapshot.browserPointerImported,
    failureAtomicity: snapshot.failureAtomicity,
  };
}

function normalizeGetAsyncKeyStateInitialScan(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const normalizeCall = (call) => ({
    ...normalizeSemanticCall(call),
    implementationId: call.implementationId,
  });
  return {
    leadingCallCount: snapshot.leadingCallCount,
    scanVirtualKeys: snapshot.scanVirtualKeys,
    firstCall: normalizeCall(snapshot.firstCall),
    lastCall: normalizeCall(snapshot.lastCall),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    cursorPosition: snapshot.cursorPosition,
    cursorStateAuthority: snapshot.cursorStateAuthority,
    keyboardProfileId: snapshot.keyboardProfileId,
    inputEventCount: snapshot.inputEventCount,
    stateAuthority: snapshot.stateAuthority,
    browserSourceAuthority: snapshot.browserSourceAuthority,
    returnEncoding: snapshot.returnEncoding,
    modifierAggregation: snapshot.modifierAggregation,
    pressedBitConsumption: snapshot.pressedBitConsumption,
    sourceFocusRequired: snapshot.sourceFocusRequired,
    focusedForegroundWindowRequired: snapshot.focusedForegroundWindowRequired,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    guestMemoryMutation: snapshot.guestMemoryMutation,
    failureAtomicity: snapshot.failureAtomicity,
  };
}

function normalizeInitialScanContinuation(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    prerequisiteScanCompletionSequence:
      snapshot.prerequisiteScanCompletionSequence,
    prerequisiteScanCompletionInstructionCount:
      snapshot.prerequisiteScanCompletionInstructionCount,
    calls: snapshot.calls.map((call) => ({
      ...normalizeSemanticCall(call),
      implementationId: call.implementationId,
    })),
    instructionCounts:
      snapshot.instructionCounts ??
      snapshot.calls.map((call) => call.trapInstructionCount),
    renderCompletionCounts: snapshot.renderCompletionCounts,
    nextSubsystemGateway: snapshot.nextSubsystemGateway,
  };
}

function normalizePeekMessageA(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    calls: snapshot.calls.map((call) => ({
      ...normalizeSemanticCall(call),
      implementationId: call.implementationId,
    })),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    profileId: snapshot.profileId,
    queueAuthority: snapshot.queueAuthority,
    emptyQueueResult: snapshot.emptyQueueResult,
    outputMutation: snapshot.outputMutation,
    removal: snapshot.removal,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    failureAtomicity: snapshot.failureAtomicity,
    filteredRetrievalImplemented: snapshot.filteredRetrievalImplemented,
    crossThreadRetrievalImplemented: snapshot.crossThreadRetrievalImplemented,
  };
}

function normalizeIsIconic(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    calls: snapshot.calls.map((call) => ({
      ...normalizeSemanticCall(call),
      implementationId: call.implementationId,
    })),
    instructionCounts:
      snapshot.instructionCounts ??
      snapshot.calls.map((call) => call.trapInstructionCount),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    profileId: snapshot.profileId,
    windowStateAuthority: snapshot.windowStateAuthority,
    minimizedStyleMask: snapshot.minimizedStyleMask,
    normalWindowResult: snapshot.normalWindowResult,
    validLastErrorPolicy: snapshot.validLastErrorPolicy,
    invalidWindowLastError: snapshot.invalidWindowLastError,
    guestMemoryMutation: snapshot.guestMemoryMutation,
    runtimeStateMutation: snapshot.runtimeStateMutation,
    failureAtomicity: snapshot.failureAtomicity,
    minimizeTransitionImplemented: snapshot.minimizeTransitionImplemented,
  };
}

function normalizeBrowserSourceRead(read) {
  return {
    requestId: String(read.request.requestId),
    sourceId: read.request.sourceId,
    objectId: read.request.objectId,
    offset: String(read.request.offset),
    requestedBytes: read.request.requestedBytes,
    transferredBytes: read.transferredBytes,
    contentSha256: read.contentSha256,
  };
}

function normalizeNodeSourceRead(read) {
  return {
    requestId: String(read.requestId),
    sourceId: read.sourceId,
    objectId: read.objectId,
    offset: String(read.offset),
    requestedBytes: read.requestedBytes,
    transferredBytes: read.transferredBytes,
    contentSha256: read.payloadSha256,
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
    partialDirtyRect: snapshot.partialDirtyRect,
    partialTransferPrecondition: snapshot.partialTransferPrecondition,
    dynamicDestinationImplemented: snapshot.dynamicDestinationImplemented,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9TextureLockRect(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  const normalizeTextureCall = (call) => ({
    ...normalizeSemanticCall(call),
    graphicsTexture: call.graphicsTexture,
  });
  return {
    lock: normalizeTextureCall(snapshot.lock),
    unlock: normalizeTextureCall(snapshot.unlock),
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    lockRectProfileId: snapshot.lockRectProfileId,
    textureAuthority: snapshot.textureAuthority,
    byteAuthority: snapshot.byteAuthority,
    backingAllocation: snapshot.backingAllocation,
    outputWrite: snapshot.outputWrite,
    lockLifetime: snapshot.lockLifetime,
    lockFlags: snapshot.lockFlags,
    mutation: snapshot.mutation,
    failureAtomicity: snapshot.failureAtomicity,
    nestedLockReturnsInvalidCall: snapshot.nestedLockReturnsInvalidCall,
    unlockWithoutLockSucceedsNoop: snapshot.unlockWithoutLockSucceedsNoop,
    dirtyRegionUpdatedByUnlock: snapshot.dirtyRegionUpdatedByUnlock,
    browserRendererByteCopy: snapshot.browserRendererByteCopy,
    additionalMipLevelsImplemented: snapshot.additionalMipLevelsImplemented,
    additionalFormatsImplemented: snapshot.additionalFormatsImplemented,
    additionalLockFlagsImplemented: snapshot.additionalLockFlagsImplemented,
  };
}

function normalizeD3d9PartialUpdateTexture(snapshot) {
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
    dirtyRegion: snapshot.dirtyRegion,
    byteAuthority: snapshot.byteAuthority,
    transferPublication: snapshot.transferPublication,
    partialTransferPrecondition: snapshot.partialTransferPrecondition,
    sourceDirtyRegionAfter: snapshot.sourceDirtyRegionAfter,
    mutation: snapshot.mutation,
    failureAtomicity: snapshot.failureAtomicity,
    arbitraryPartialHistoryImplemented:
      snapshot.arbitraryPartialHistoryImplemented,
    dynamicDestinationImplemented: snapshot.dynamicDestinationImplemented,
  };
}

function normalizeD3d9TestCooperativeLevel(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: normalizeSemanticCall(snapshot.call),
    instructionCount:
      snapshot.instructionCount ?? snapshot.call.trapInstructionCount,
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    profileId: snapshot.profileId,
    stateAuthority: snapshot.stateAuthority,
    queryResult: snapshot.queryResult,
    mutation: snapshot.mutation,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    guestMemoryMutation: snapshot.guestMemoryMutation,
    referenceCountMutation: snapshot.referenceCountMutation,
    failureAtomicity: snapshot.failureAtomicity,
    deviceLostStateImplemented: snapshot.deviceLostStateImplemented,
    deviceNotResetStateImplemented: snapshot.deviceNotResetStateImplemented,
    resetImplemented: snapshot.resetImplemented,
    browserContextLossImported: snapshot.browserContextLossImported,
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
    calls: snapshot.calls.slice(0, 2).map((call) => ({
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
    calls: snapshot.calls.slice(0, 6).map((call) => ({
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

function normalizeD3d9SetTexture(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    textureBindingProfileId: snapshot.textureBindingProfileId,
    stateAuthority: snapshot.stateAuthority,
    bindingOwnership: snapshot.bindingOwnership,
    mutation: snapshot.mutation,
    sameValueGenerationChange: snapshot.sameValueGenerationChange,
    nullUnbindSupported: snapshot.nullUnbindSupported,
    unsupportedVariantResult: snapshot.unsupportedVariantResult,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9SetFvf(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    flexibleVertexFormatProfileId: snapshot.flexibleVertexFormatProfileId,
    stateAuthority: snapshot.stateAuthority,
    mutation: snapshot.mutation,
    sameValueGenerationChange: snapshot.sameValueGenerationChange,
    unsupportedVariantResult: snapshot.unsupportedVariantResult,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9SetStreamSource(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    streamSourceProfileId: snapshot.streamSourceProfileId,
    stateAuthority: snapshot.stateAuthority,
    bindingOwnership: snapshot.bindingOwnership,
    mutation: snapshot.mutation,
    sameValueGenerationChange: snapshot.sameValueGenerationChange,
    supportedStream: snapshot.supportedStream,
    supportedOffsetInBytes: snapshot.supportedOffsetInBytes,
    supportedStride: snapshot.supportedStride,
    nullUnbindImplemented: snapshot.nullUnbindImplemented,
    unsupportedVariantResult: snapshot.unsupportedVariantResult,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
  };
}

function normalizeD3d9DrawPrimitive(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    drawPrimitiveProfileId: snapshot.drawPrimitiveProfileId,
    stateAuthority: snapshot.stateAuthority,
    vertexByteAuthority: snapshot.vertexByteAuthority,
    mutation: snapshot.mutation,
    supportedPrimitiveTopology: snapshot.supportedPrimitiveTopology,
    supportedStartVertex: snapshot.supportedStartVertex,
    supportedPrimitiveCount: snapshot.supportedPrimitiveCount,
    capturedVertexBytes: snapshot.capturedVertexBytes,
    pendingSubmissionLimit: snapshot.pendingSubmissionLimit,
    pendingVertexBytesLimit: snapshot.pendingVertexBytesLimit,
    clearDiscardsPendingSubmissions: snapshot.clearDiscardsPendingSubmissions,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
    visiblePixelsProduced: snapshot.visiblePixelsProduced,
  };
}

function normalizeD3d9EndScene(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    endSceneProfileId: snapshot.endSceneProfileId,
    sceneAuthority: snapshot.sceneAuthority,
    lifecycle: snapshot.lifecycle,
    mutation: snapshot.mutation,
    preservesDrawSubmissions: snapshot.preservesDrawSubmissions,
    repeatedEndReturnsInvalidCall: snapshot.repeatedEndReturnsInvalidCall,
    failureAtomicity: snapshot.failureAtomicity,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
    visiblePixelsProduced: snapshot.visiblePixelsProduced,
  };
}

function normalizeD3d9Present(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: {
      ...normalizeSemanticCall(snapshot.call),
      graphicsDevice: snapshot.call.graphicsDevice,
    },
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    presentProfileId: snapshot.presentProfileId,
    frameAuthority: snapshot.frameAuthority,
    mutation: snapshot.mutation,
    presentMode: snapshot.presentMode,
    sourceRectImplemented: snapshot.sourceRectImplemented,
    destinationRectImplemented: snapshot.destinationRectImplemented,
    destinationWindowOverrideImplemented:
      snapshot.destinationWindowOverrideImplemented,
    dirtyRegionImplemented: snapshot.dirtyRegionImplemented,
    asynchronousDeviceLossRecoveryImplemented:
      snapshot.asynchronousDeviceLossRecoveryImplemented,
    failureAtomicity: snapshot.failureAtomicity,
  };
}

function normalizeD3d9PostEndSceneRasterStatus(snapshot) {
  if (snapshot === null || snapshot === undefined) {
    return null;
  }
  return {
    call: normalizeSemanticCall(snapshot.call),
    cumulativeCompletions: snapshot.cumulativeCompletions,
    compatibilityPolicyId: snapshot.compatibilityPolicyId,
    rasterStatusProfileId: snapshot.rasterStatusProfileId,
    displayProfileId: snapshot.displayProfileId,
    stateAuthority: snapshot.stateAuthority,
    outputBytes: snapshot.outputBytes,
    inVBlank: snapshot.inVBlank,
    scanLine: snapshot.scanLine,
    runtimeStateMutation: snapshot.runtimeStateMutation,
    referenceCountMutation: snapshot.referenceCountMutation,
    lastErrorPolicy: snapshot.lastErrorPolicy,
    guestWriteAtomic: snapshot.guestWriteAtomic,
    hostScanoutImported: snapshot.hostScanoutImported,
    browserScanoutImported: snapshot.browserScanoutImported,
    browserRendererResourceCreated: snapshot.browserRendererResourceCreated,
    visiblePixelsProduced: snapshot.visiblePixelsProduced,
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
    normalizePostPresentContinuation(browser.report.postPresentContinuation),
    normalizePostPresentContinuation(oracle.postPresentContinuation),
    "Chrome Worker post-Present continuation differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGetAsyncKeyState(browser.report.getAsyncKeyState),
    normalizeGetAsyncKeyState(oracle.getAsyncKeyState),
    "Chrome Worker GetAsyncKeyState input evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGetWindowRect(browser.report.getWindowRect),
    normalizeGetWindowRect(oracle.getWindowRect),
    "Chrome Worker GetWindowRect evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGetCursorPos(browser.report.getCursorPos),
    normalizeGetCursorPos(oracle.getCursorPos),
    "Chrome Worker GetCursorPos evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeGetAsyncKeyStateInitialScan(
      browser.report.getAsyncKeyStateInitialScan,
    ),
    normalizeGetAsyncKeyStateInitialScan(oracle.getAsyncKeyStateInitialScan),
    "Chrome Worker initial GetAsyncKeyState scan differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeInitialScanContinuation(browser.report.initialScanContinuation),
    normalizeInitialScanContinuation(oracle.initialScanContinuation),
    "Chrome Worker initial-scan continuation differs from the Node oracle",
  );
  assert.deepEqual(
    normalizePeekMessageA(browser.report.peekMessageA),
    normalizePeekMessageA(oracle.peekMessageA),
    "Chrome Worker PeekMessageA evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeIsIconic(browser.report.isIconic),
    normalizeIsIconic(oracle.isIconic),
    "Chrome Worker IsIconic evidence differs from the Node oracle",
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
    normalizeD3d9TextureLockRect(browser.report.d3d9TextureLockRect),
    normalizeD3d9TextureLockRect(oracle.d3d9TextureLockRect),
    "Chrome Worker D3D9 LockRect/UnlockRect evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9PartialUpdateTexture(browser.report.d3d9PartialUpdateTexture),
    normalizeD3d9PartialUpdateTexture(oracle.d3d9PartialUpdateTexture),
    "Chrome Worker partial D3D9 UpdateTexture evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9TestCooperativeLevel(browser.report.d3d9TestCooperativeLevel),
    normalizeD3d9TestCooperativeLevel(oracle.d3d9TestCooperativeLevel),
    "Chrome Worker D3D9 TestCooperativeLevel evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9BeginScene(browser.report.d3d9BeginScene),
    normalizeD3d9BeginScene(oracle.d3d9BeginScene),
    "Chrome Worker D3D9 BeginScene evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9InitialSamplerStates(browser.report.d3d9InitialSamplerStates),
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
    normalizeD3d9SetTexture(browser.report.d3d9SetTexture),
    normalizeD3d9SetTexture(oracle.d3d9SetTexture),
    "Chrome Worker D3D9 SetTexture evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9SetFvf(browser.report.d3d9SetFvf),
    normalizeD3d9SetFvf(oracle.d3d9SetFvf),
    "Chrome Worker D3D9 SetFVF evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9SetStreamSource(browser.report.d3d9SetStreamSource),
    normalizeD3d9SetStreamSource(oracle.d3d9SetStreamSource),
    "Chrome Worker D3D9 SetStreamSource evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9DrawPrimitive(browser.report.d3d9DrawPrimitive),
    normalizeD3d9DrawPrimitive(oracle.d3d9DrawPrimitive),
    "Chrome Worker D3D9 DrawPrimitive evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9EndScene(browser.report.d3d9EndScene),
    normalizeD3d9EndScene(oracle.d3d9EndScene),
    "Chrome Worker D3D9 EndScene evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9Present(browser.report.d3d9Present),
    normalizeD3d9Present(oracle.d3d9Present),
    "Chrome Worker D3D9 Present logical evidence differs from the Node oracle",
  );
  assert.deepEqual(
    normalizeD3d9PostEndSceneRasterStatus(
      browser.report.d3d9PostEndSceneRasterStatus,
    ),
    normalizeD3d9PostEndSceneRasterStatus(oracle.d3d9PostEndSceneRasterStatus),
    "Chrome Worker post-EndScene D3D9 GetRasterStatus evidence differs from the Node oracle",
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
  assert.deepEqual(
    browser.report.sourceReads.map(normalizeBrowserSourceRead),
    oracle.synchronization.sourceReads.map(normalizeNodeSourceRead),
    "Chrome Worker source-read completions differ from the Node oracle",
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
      oracle.waitableTimerWait.pacingRequests?.length ??
        oracle.waitableTimerWait.pacingRequestCount,
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
    if (oracleCheckpoint.capability === "kernel32-heap-free-large-buffer") {
      const normalizeCompletion = ({ call, freeDelta, memoryAfter }) => ({
        call: {
          ...normalizeSemanticCall(call),
          implementationId: call.implementationId,
          heapFree: call.heapFree,
        },
        freeDelta: {
          generationAdvance: freeDelta.generation - freeDelta.fromGeneration,
          mappingOperations: freeDelta.mappingOperations,
          dirtyPages: freeDelta.dirtyPages,
          cacheEffects: freeDelta.cacheEffects,
          payloadBytes: freeDelta.payloadBytes,
          payloadSha256: freeDelta.payloadSha256,
        },
        memoryGenerationMatchesDelta:
          memoryAfter.generation === freeDelta.generation,
      });
      for (const completion of browserCheckpoint.completions) {
        assert.equal(
          completion.memoryAfter.acknowledgedGeneration,
          completion.freeDelta.fromGeneration,
          "Chrome Worker must preserve the pre-ack Rust memory snapshot",
        );
      }
      for (const completion of oracleCheckpoint.completions) {
        assert.equal(
          completion.memoryAfter.acknowledgedGeneration,
          completion.freeDelta.generation,
          "Node oracle must preserve its post-ack Rust memory snapshot",
        );
      }
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          freedByte: browserCheckpoint.freedByte,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeCompletion),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          freedByte: oracleCheckpoint.freedByte,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeCompletion),
        },
        "Chrome Worker large HeapFree lifecycle differs from the Node oracle",
      );
    }
    if (oracleCheckpoint.capability === "kernel32-heap-alloc-24-byte-buffer") {
      const normalizeCompletion = ({ call, allocationDelta, memoryAfter }) => ({
        call: {
          ...normalizeSemanticCall(call),
          implementationId: call.implementationId,
          heapAllocation: call.heapAllocation,
        },
        allocationDelta: {
          generationAdvance:
            allocationDelta.generation - allocationDelta.fromGeneration,
          mappingOperations: allocationDelta.mappingOperations,
          dirtyPages: allocationDelta.dirtyPages,
          cacheEffects: allocationDelta.cacheEffects,
          payloadBytes: allocationDelta.payloadBytes,
          payloadSha256: allocationDelta.payloadSha256,
        },
        memoryGenerationMatchesDelta:
          memoryAfter.generation === allocationDelta.generation,
      });
      for (const completion of browserCheckpoint.completions) {
        assert.equal(
          completion.memoryAfter.acknowledgedGeneration,
          completion.allocationDelta.fromGeneration,
          "Chrome Worker must preserve the pre-ack HeapAlloc memory snapshot",
        );
      }
      for (const completion of oracleCheckpoint.completions) {
        assert.equal(
          completion.memoryAfter.acknowledgedGeneration,
          completion.allocationDelta.generation,
          "Node oracle must preserve its post-ack HeapAlloc memory snapshot",
        );
      }
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeCompletion),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeCompletion),
        },
        "Chrome Worker HeapAlloc lifecycle differs from the Node oracle",
      );
    }
    if (oracleCheckpoint.capability === "winmm-time-get-time-post-heap-reuse") {
      const normalizeClockCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        tickCount: call.tickCount,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeClockCall),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeClockCall),
        },
        "Chrome Worker post-heap timeGetTime lifecycle differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "kernel32-enter-critical-section-post-clock"
    ) {
      const normalizeCriticalSectionCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          criticalSectionAddress: browserCheckpoint.criticalSectionAddress,
          threadId: browserCheckpoint.threadId,
          balanced: browserCheckpoint.balanced,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(
            normalizeCriticalSectionCall,
          ),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          criticalSectionAddress: oracleCheckpoint.criticalSectionAddress,
          threadId: oracleCheckpoint.threadId,
          balanced: oracleCheckpoint.balanced,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(
            normalizeCriticalSectionCall,
          ),
        },
        "Chrome Worker post-clock critical-section roundtrip differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
        "kernel32-enter-critical-section-post-wait" ||
      oracleCheckpoint.capability ===
        "kernel32-enter-critical-section-post-timer-wait-batch" ||
      oracleCheckpoint.capability ===
        "kernel32-enter-critical-section-post-timer-rearm-wait-batch"
    ) {
      const normalizeCriticalSectionCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          roundtripCount: browserCheckpoint.roundtripCount,
          criticalSectionAddresses: browserCheckpoint.criticalSectionAddresses,
          threadId: browserCheckpoint.threadId,
          balanced: browserCheckpoint.balanced,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(
            normalizeCriticalSectionCall,
          ),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          roundtripCount: oracleCheckpoint.roundtripCount,
          criticalSectionAddresses: oracleCheckpoint.criticalSectionAddresses,
          threadId: oracleCheckpoint.threadId,
          balanced: oracleCheckpoint.balanced,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(
            normalizeCriticalSectionCall,
          ),
        },
        "Chrome Worker balanced critical-section batch differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "primary-main-loop-recurring-timer-cycle-batch"
    ) {
      const normalizeCycleCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          waitCompletionCount: browserCheckpoint.waitCompletionCount,
          criticalSectionCompletionCount:
            browserCheckpoint.criticalSectionCompletionCount,
          timerArmCompletionCount: browserCheckpoint.timerArmCompletionCount,
          roundtripCount: browserCheckpoint.roundtripCount,
          criticalSectionAddresses: browserCheckpoint.criticalSectionAddresses,
          threadId: browserCheckpoint.threadId,
          balanced: browserCheckpoint.balanced,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeCycleCall),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          waitCompletionCount: oracleCheckpoint.waitCompletionCount,
          criticalSectionCompletionCount:
            oracleCheckpoint.criticalSectionCompletionCount,
          timerArmCompletionCount: oracleCheckpoint.timerArmCompletionCount,
          roundtripCount: oracleCheckpoint.roundtripCount,
          criticalSectionAddresses: oracleCheckpoint.criticalSectionAddresses,
          threadId: oracleCheckpoint.threadId,
          balanced: oracleCheckpoint.balanced,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeCycleCall),
        },
        "Chrome Worker recurring timer cycle differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "primary-main-loop-window-timer-callback-delivery-gateway"
    ) {
      const normalizeDispatch = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        windowTimer: call.windowTimer,
        callback: call.callback,
        callbackArguments: call.callbackArguments,
      });
      assert.equal(
        browserCheckpoint.capability,
        oracleCheckpoint.capability,
        "Chrome Worker reached a different window timer capability",
      );
      assert.deepEqual(
        {
          implementationId: browserCheckpoint.implementationId,
          requiredDeliveries: browserCheckpoint.requiredDeliveries,
          deliveryCount: browserCheckpoint.deliveryCount,
          dispatch: normalizeDispatch(browserCheckpoint.dispatch),
          instructionCount: browserCheckpoint.instructionCount,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          memoryContentSha256: browserCheckpoint.memoryContentSha256,
        },
        {
          implementationId: oracleCheckpoint.implementationId,
          requiredDeliveries: oracleCheckpoint.requiredDeliveries,
          deliveryCount: oracleCheckpoint.deliveryCount,
          dispatch: normalizeDispatch(oracleCheckpoint.dispatch),
          instructionCount: oracleCheckpoint.instructionCount,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          memoryContentSha256: oracleCheckpoint.memoryContentSha256,
        },
        "Chrome Worker window timer delivery differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "primary-main-loop-window-timer-callback-recurring-batch"
    ) {
      const normalizeDispatch = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        windowTimer: call.windowTimer,
        callback: call.callback,
        callbackArguments: call.callbackArguments,
      });
      assert.equal(
        browserCheckpoint.capability,
        oracleCheckpoint.capability,
        "Chrome Worker reached a different recurring window timer capability",
      );
      assert.deepEqual(
        {
          implementationId: browserCheckpoint.implementationId,
          requiredDeliveries: browserCheckpoint.requiredDeliveries,
          deliveryCount: browserCheckpoint.deliveryCount,
          requiredCompletions: browserCheckpoint.requiredCompletions,
          completionCount: browserCheckpoint.completionCount,
          firstCompletionSequence:
            browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          dispatches: browserCheckpoint.dispatches.map(normalizeDispatch),
          instructionCount: browserCheckpoint.instructionCount,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          memoryContentSha256: browserCheckpoint.memoryContentSha256,
        },
        {
          implementationId: oracleCheckpoint.implementationId,
          requiredDeliveries: oracleCheckpoint.requiredDeliveries,
          deliveryCount: oracleCheckpoint.deliveryCount,
          requiredCompletions: oracleCheckpoint.requiredCompletions,
          completionCount: oracleCheckpoint.completionCount,
          firstCompletionSequence:
            oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          dispatches: oracleCheckpoint.dispatches.map(normalizeDispatch),
          instructionCount: oracleCheckpoint.instructionCount,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          memoryContentSha256: oracleCheckpoint.memoryContentSha256,
        },
        "Chrome Worker recurring window timer batch differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
        "primary-main-loop-pointer-message-action-progress-gateway" ||
      oracleCheckpoint.capability === "user32-set-focus-action-continuation" ||
      oracleCheckpoint.capability ===
        "winmm-time-get-time-action-continuation" ||
      oracleCheckpoint.capability ===
        "user32-def-window-proc-a-action-continuation" ||
      oracleCheckpoint.capability ===
        "user32-peek-message-a-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-translate-message-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-dispatch-message-a-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-def-window-proc-a-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-peek-message-a-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "winmm-time-get-time-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "winmm-time-get-time-post-pointer-release-repeat-batch" ||
      oracleCheckpoint.capability ===
        "user32-get-window-rect-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-get-cursor-pos-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-get-system-metrics-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-get-system-metrics-caption-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "user32-is-iconic-post-pointer-release-continuation" ||
      oracleCheckpoint.capability ===
        "winmm-time-get-time-post-is-iconic-continuation" ||
      oracleCheckpoint.capability ===
        "user32-get-async-key-state-post-is-iconic-clock-continuation"
    ) {
      const normalizeThreadCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        occurrence: call.occurrence,
        trapInstructionCount: call.trapInstructionCount,
      });
      const normalizeInputEvent = ({ memory: _memory, ...event }) => event;
      const normalizePointerCheckpoint = (candidate) => ({
        implementationId: candidate.implementationId,
        requiredPointerMessages: candidate.requiredPointerMessages,
        pointerMessageDeliveryCount: candidate.pointerMessageDeliveryCount,
        inputBaseline: candidate.inputBaseline,
        inputEvents: candidate.inputEvents.map(normalizeInputEvent),
        inputQuery: normalizeThreadCall(candidate.inputQuery),
        peekMessage: normalizeThreadCall(candidate.peekMessage),
        translateMessage: normalizeThreadCall(candidate.translateMessage),
        dispatchEntry: {
          ...candidate.dispatchEntry,
          module: candidate.dispatchEntry.module.toLowerCase(),
        },
        callbackEntry: candidate.callbackEntry,
        releaseMessageQueued: candidate.releaseMessageQueued,
        activeCallbackDepthAtStop: candidate.activeCallbackDepthAtStop,
        ...(candidate.setFocus === undefined
          ? {}
          : {
              requiredCompletions: candidate.requiredCompletions,
              completionCount: candidate.completionCount,
              setFocus: {
                ...normalizeThreadCall(candidate.setFocus),
                runtimeThreadId: candidate.setFocus.runtimeThreadId,
              },
            }),
        ...(candidate.timeGetTime === undefined
          ? {}
          : {
              lastCompletionSequence: candidate.lastCompletionSequence,
              lastCompletionInstructionCount:
                candidate.lastCompletionInstructionCount,
              setFocusActiveCallbackDepthAtStop:
                candidate.setFocusActiveCallbackDepthAtStop,
              ...(candidate.timeGetTimeActiveCallbackDepthAtCompletion ===
              undefined
                ? {}
                : {
                    timeGetTimeActiveCallbackDepthAtCompletion:
                      candidate.timeGetTimeActiveCallbackDepthAtCompletion,
                  }),
              timeGetTime: {
                ...normalizeThreadCall(candidate.timeGetTime),
                runtimeThreadId: candidate.timeGetTime.runtimeThreadId,
                tickCount: candidate.timeGetTime.tickCount,
              },
            }),
        ...(candidate.defWindowProcA === undefined
          ? {}
          : {
              defWindowProcAActiveCallbackDepthAtCompletion:
                candidate.defWindowProcAActiveCallbackDepthAtCompletion,
              defWindowProcA: {
                ...normalizeThreadCall(candidate.defWindowProcA),
                runtimeThreadId: candidate.defWindowProcA.runtimeThreadId,
              },
            }),
        ...(candidate.peekMessageRelease === undefined
          ? {}
          : {
              releaseMessage: candidate.releaseMessage,
              queueDepthBefore: candidate.queueDepthBefore,
              queueDepthAfter: candidate.queueDepthAfter,
              peekMessageReleaseActiveCallbackDepthAtCompletion:
                candidate.peekMessageReleaseActiveCallbackDepthAtCompletion,
              peekMessageRelease: {
                ...normalizeThreadCall(candidate.peekMessageRelease),
                runtimeThreadId: candidate.peekMessageRelease.runtimeThreadId,
              },
            }),
        ...(candidate.translateMessageRelease === undefined
          ? {}
          : {
              translateMessageReleaseActiveCallbackDepthAtCompletion:
                candidate.translateMessageReleaseActiveCallbackDepthAtCompletion,
              queueDepthAtTranslateCompletion:
                candidate.queueDepthAtTranslateCompletion,
              translateMessageReleaseBytesPreserved:
                candidate.translateMessageReleaseBytesPreserved,
              translateMessageRelease: {
                ...normalizeThreadCall(candidate.translateMessageRelease),
                runtimeThreadId:
                  candidate.translateMessageRelease.runtimeThreadId,
                windowTimer: candidate.translateMessageRelease.windowTimer,
              },
            }),
        ...(candidate.dispatchMessageRelease === undefined
          ? {}
          : {
              requiredEntries: candidate.requiredEntries,
              entryCount: candidate.entryCount,
              lastEntrySequence: candidate.lastEntrySequence,
              lastEntryInstructionCount:
                candidate.lastEntryInstructionCount,
              dispatchMessageReleaseActiveCallbackDepthAtEntry:
                candidate.dispatchMessageReleaseActiveCallbackDepthAtEntry,
              queueDepthAtDispatchEntry:
                candidate.queueDepthAtDispatchEntry,
              dispatchMessageReleaseBytesPreserved:
                candidate.dispatchMessageReleaseBytesPreserved,
              dispatchMessageRelease: {
                ...candidate.dispatchMessageRelease,
                module:
                  candidate.dispatchMessageRelease.module.toLowerCase(),
                runtimeThreadId:
                  candidate.dispatchMessageRelease.runtimeThreadId,
              },
              releaseCallbackEntry: candidate.releaseCallbackEntry,
            }),
        ...(candidate.defWindowProcARelease === undefined
          ? {}
          : {
              defWindowProcAReleaseActiveCallbackDepthAtCompletion:
                candidate.defWindowProcAReleaseActiveCallbackDepthAtCompletion,
              defWindowProcARelease: {
                ...normalizeThreadCall(candidate.defWindowProcARelease),
                runtimeThreadId:
                  candidate.defWindowProcARelease.runtimeThreadId,
              },
              releaseCallbackReturn: candidate.releaseCallbackReturn,
              dispatchMessageReleaseResult:
                candidate.dispatchMessageReleaseResult,
            }),
        ...(candidate.peekMessagePostRelease === undefined
          ? {}
          : {
              peekMessagePostReleaseActiveCallbackDepthAtCompletion:
                candidate.peekMessagePostReleaseActiveCallbackDepthAtCompletion,
              postReleaseQueueDepthBefore:
                candidate.postReleaseQueueDepthBefore,
              postReleaseQueueDepthAfter:
                candidate.postReleaseQueueDepthAfter,
              postReleaseMessageBytesPreserved:
                candidate.postReleaseMessageBytesPreserved,
              peekMessagePostRelease: {
                ...normalizeThreadCall(candidate.peekMessagePostRelease),
                runtimeThreadId:
                  candidate.peekMessagePostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.timeGetTimePostRelease === undefined
          ? {}
          : {
              timeGetTimePostReleaseActiveCallbackDepthAtCompletion:
                candidate.timeGetTimePostReleaseActiveCallbackDepthAtCompletion,
              timeGetTimePostRelease: {
                ...normalizeThreadCall(candidate.timeGetTimePostRelease),
                runtimeThreadId:
                  candidate.timeGetTimePostRelease.runtimeThreadId,
                tickCount: candidate.timeGetTimePostRelease.tickCount,
              },
            }),
        ...(candidate.timeGetTimePostReleaseCompletions === undefined
          ? {}
          : {
              maximumRepeatCompletions:
                candidate.maximumRepeatCompletions,
              timeGetTimePostReleaseCompletions:
                candidate.timeGetTimePostReleaseCompletions.map(
                  (completion) => ({
                    ...normalizeThreadCall(completion),
                    runtimeThreadId: completion.runtimeThreadId,
                    tickCount: completion.tickCount,
                  }),
                ),
            }),
        ...(candidate.getWindowRectPostRelease === undefined
          ? {}
          : {
              getWindowRectPostReleaseActiveCallbackDepthAtCompletion:
                candidate
                  .getWindowRectPostReleaseActiveCallbackDepthAtCompletion,
              getWindowRectPostRelease: {
                ...normalizeThreadCall(candidate.getWindowRectPostRelease),
                runtimeThreadId:
                  candidate.getWindowRectPostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.getCursorPosPostRelease === undefined
          ? {}
          : {
              getCursorPosPostReleaseActiveCallbackDepthAtCompletion:
                candidate.getCursorPosPostReleaseActiveCallbackDepthAtCompletion,
              getCursorPosPostRelease: {
                ...normalizeThreadCall(candidate.getCursorPosPostRelease),
                runtimeThreadId:
                  candidate.getCursorPosPostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.getSystemMetricsPostRelease === undefined
          ? {}
          : {
              getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion:
                candidate
                  .getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion,
              getSystemMetricsPostRelease: {
                ...normalizeThreadCall(candidate.getSystemMetricsPostRelease),
                runtimeThreadId:
                  candidate.getSystemMetricsPostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.getSystemMetricsCaptionPostRelease === undefined
          ? {}
          : {
              getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion:
                candidate
                  .getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion,
              getSystemMetricsCaptionPostRelease: {
                ...normalizeThreadCall(
                  candidate.getSystemMetricsCaptionPostRelease,
                ),
                runtimeThreadId:
                  candidate.getSystemMetricsCaptionPostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.getSystemMetricsFixedFrameHeightPostRelease === undefined
          ? {}
          : {
              getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion:
                candidate
                  .getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion,
              getSystemMetricsFixedFrameHeightPostRelease: {
                ...normalizeThreadCall(
                  candidate.getSystemMetricsFixedFrameHeightPostRelease,
                ),
                runtimeThreadId:
                  candidate.getSystemMetricsFixedFrameHeightPostRelease
                    .runtimeThreadId,
              },
            }),
        ...(candidate.isIconicPostRelease === undefined
          ? {}
          : {
              isIconicPostReleaseActiveCallbackDepthAtCompletion:
                candidate.isIconicPostReleaseActiveCallbackDepthAtCompletion,
              isIconicPostRelease: {
                ...normalizeThreadCall(candidate.isIconicPostRelease),
                runtimeThreadId:
                  candidate.isIconicPostRelease.runtimeThreadId,
              },
            }),
        ...(candidate.timeGetTimePostIsIconic === undefined
          ? {}
          : {
              timeGetTimePostIsIconicActiveCallbackDepthAtCompletion:
                candidate.timeGetTimePostIsIconicActiveCallbackDepthAtCompletion,
              timeGetTimePostIsIconic: {
                ...normalizeThreadCall(candidate.timeGetTimePostIsIconic),
                runtimeThreadId:
                  candidate.timeGetTimePostIsIconic.runtimeThreadId,
              },
            }),
        ...(candidate.getAsyncKeyStatePostIsIconicClockFirst === undefined
          ? {}
          : {
              firstCompletionSequence: candidate.firstCompletionSequence,
              firstCompletionInstructionCount:
                candidate.firstCompletionInstructionCount,
              scanVirtualKeys: candidate.scanVirtualKeys,
              result: candidate.result,
              lastError: candidate.lastError,
              guestWriteBytes: candidate.guestWriteBytes,
              getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion:
                candidate
                  .getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion,
              getAsyncKeyStatePostIsIconicClockFirst: {
                ...normalizeThreadCall(
                  candidate.getAsyncKeyStatePostIsIconicClockFirst,
                ),
                runtimeThreadId:
                  candidate.getAsyncKeyStatePostIsIconicClockFirst
                    .runtimeThreadId,
              },
              getAsyncKeyStatePostIsIconicClockLast: {
                ...normalizeThreadCall(
                  candidate.getAsyncKeyStatePostIsIconicClockLast,
                ),
                runtimeThreadId:
                  candidate.getAsyncKeyStatePostIsIconicClockLast
                    .runtimeThreadId,
              },
            }),
        instructionCount: candidate.instructionCount,
        nextEndpoint: {
          ...candidate.nextEndpoint,
          module: candidate.nextEndpoint.module?.toLowerCase(),
        },
        nextArguments: candidate.nextArguments,
        nextReturnAddress: candidate.nextReturnAddress,
        nextStackPointer: candidate.nextStackPointer,
        memoryContentSha256: candidate.memoryContentSha256,
      });
      assert.equal(
        browserCheckpoint.capability,
        oracleCheckpoint.capability,
        "Chrome Worker reached a different pointer-message capability",
      );
      if (
        oracleCheckpoint.capability ===
        "user32-get-async-key-state-post-is-iconic-clock-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 162_079);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "1d7b2f9fd9e3677a19611e3648e62a40a5f571131c1176636130b06d1bddf284",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-async-key-state-post-is-iconic-clock-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 38);
        assert.equal(oracleCheckpoint.completionCount, 38);
        assert.equal(oracleCheckpoint.firstCompletionSequence, 161_965);
        assert.equal(
          oracleCheckpoint.firstCompletionInstructionCount,
          321_463_448,
        );
        assert.equal(oracleCheckpoint.lastCompletionSequence, 162_079);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_468_229,
        );
        assert.deepEqual(oracleCheckpoint.scanVirtualKeys, [
          14, 15, 13, 16, 27, 38, 33, 40, 34, 37, 39, 49, 97, 50, 98, 51,
          99, 52, 100, 53, 101, 54, 102, 55, 103, 56, 104, 57, 105, 48, 96,
          9, 17, 1, 2, 4, 5, 6,
        ]);
        assert.deepEqual(oracleCheckpoint.result, {
          action: "set",
          value: 0,
        });
        assert.equal(oracleCheckpoint.lastError, 0);
        assert.equal(oracleCheckpoint.guestWriteBytes, 0);
        assert.equal(
          oracleCheckpoint.getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion,
          0,
        );
        const firstCall =
          oracleCheckpoint.getAsyncKeyStatePostIsIconicClockFirst;
        const lastCall = oracleCheckpoint.getAsyncKeyStatePostIsIconicClockLast;
        assert.deepEqual(
          {
            sequence: firstCall.sequence,
            occurrence: firstCall.occurrence,
            trapInstructionCount: firstCall.trapInstructionCount,
            arguments: firstCall.arguments,
            result: firstCall.result,
            stackPointerBefore: firstCall.stackPointerBefore,
            stackPointerAfter: firstCall.stackPointerAfter,
            guestWrites: firstCall.guestWrites,
          },
          {
            sequence: 161_965,
            occurrence: 386,
            trapInstructionCount: 321_463_448,
            arguments: [14],
            result: { action: "set", value: 0 },
            stackPointerBefore: 0x006f_f80c,
            stackPointerAfter: 0x006f_f814,
            guestWrites: [],
          },
        );
        assert.deepEqual(
          {
            sequence: lastCall.sequence,
            occurrence: lastCall.occurrence,
            trapInstructionCount: lastCall.trapInstructionCount,
            arguments: lastCall.arguments,
            result: lastCall.result,
            stackPointerBefore: lastCall.stackPointerBefore,
            stackPointerAfter: lastCall.stackPointerAfter,
            guestWrites: lastCall.guestWrites,
          },
          {
            sequence: 162_079,
            occurrence: 423,
            trapInstructionCount: 321_468_229,
            arguments: [6],
            result: { action: "set", value: 0 },
            stackPointerBefore: 0x006f_f8c8,
            stackPointerAfter: 0x006f_f8d0,
            guestWrites: [],
          },
        );
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_468_292);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 0,
          address: 0x0101_1060,
          module: "<com>",
          symbol: { kind: "name", name: "interface-1-slot-3" },
          disposition: "declared-com-method",
          interfaceIndex: 1,
          vtableSlot: 3,
          interface: "IDirect3DDevice9",
          method: "TestCooperativeLevel",
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x0101_0004]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1005_adcd);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8f4);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "d4e6ec50e875a134762e7abeea2d49bbad4c2be754b407741de025ca7d78b2be",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_233);
        assert.equal(oracle.synchronization.dirtyPages, 244_664);
        assert.equal(oracle.synchronization.changedBytes, 43_072_357);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_995);
        assert.equal(oracle.synchronization.finalGeneration, 296_510);
        assert.equal(oracle.threadExecution.contextSwitches, 2_514);
        assert.equal(browser.report.instructions, 321_468_292);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 315_149);
        assert.equal(browser.report.writebackBatches, 183_978);
        assert.equal(browser.report.writebackPages, 282_705);
        assert.equal(browser.report.mappingDeltas, 127_995);
        assert.equal(browser.report.mappingDirtyPages, 150_400);
        assert.equal(browser.report.slices, 183_985);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 589_072);
        assert.equal(browser.report.contextSwitches, 2_514);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_708);
        assert.equal(browser.statusMessagesObserved, 24_363);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "winmm-time-get-time-post-is-iconic-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_964);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "893826b9bf64219e935d14b5455cff43a62df40ea8292aceabbea73dd724606e",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.winmm-time-get-time-post-is-iconic-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_964);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_291,
        );
        assert.equal(
          oracleCheckpoint.timeGetTimePostIsIconicActiveCallbackDepthAtCompletion,
          0,
        );
        const timeGetTime = oracleCheckpoint.timeGetTimePostIsIconic;
        assert.equal(timeGetTime.runtimeThreadId, 8_192);
        assert.equal(timeGetTime.sequence, 161_964);
        assert.equal(timeGetTime.occurrence, 223);
        assert.equal(timeGetTime.trapInstructionCount, 321_463_291);
        assert.equal(timeGetTime.module.toLowerCase(), "winmm.dll");
        assert.equal(timeGetTime.symbol, "timeGetTime");
        assert.equal(timeGetTime.implementationId, "winmm.time-get-time.v1");
        assert.deepEqual(timeGetTime.arguments, []);
        assert.deepEqual(timeGetTime.result, {
          action: "set",
          value: 3_601_228,
        });
        assert.equal(timeGetTime.returnAddress, 0x1009_1ebf);
        assert.equal(timeGetTime.stackPointerBefore, 0x006f_f8d8);
        assert.equal(timeGetTime.stackPointerAfter, 0x006f_f8dc);
        assert.equal(timeGetTime.lastError, 0);
        assert.deepEqual(timeGetTime.guestWrites, []);
        assert.deepEqual(timeGetTime.tickCount, {
          returnedTickCount: 3_601_228,
          nextTickCount: 3_601_229,
          incrementPerRead: 1,
        });
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_448);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 231,
          address: 0x7f00_0e60,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetAsyncKeyState" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1474,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [14, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1006_8405);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f80c);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "57976ebffcc035e81358e80479bd1848454c08d85c6f024676df353ad444aaae",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_118);
        assert.equal(oracle.synchronization.dirtyPages, 244_485);
        assert.equal(oracle.synchronization.changedBytes, 43_070_925);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_339);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_863);
        assert.equal(browser.report.writebackPages, 282_526);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_448);
        assert.equal(browser.report.slices, 183_870);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_701);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_978);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_413);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-is-iconic-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_963);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "2c88330f91f2f237a6b3e673e93279b62b29ad98124c4bc1eeac3a0c9d942c5a",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-is-iconic-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_963);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_282,
        );
        assert.equal(
          oracleCheckpoint.isIconicPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const isIconic = oracleCheckpoint.isIconicPostRelease;
        assert.equal(isIconic.runtimeThreadId, 8_192);
        assert.equal(isIconic.sequence, 161_963);
        assert.equal(isIconic.occurrence, 8);
        assert.equal(isIconic.trapInstructionCount, 321_463_282);
        assert.equal(isIconic.module.toLowerCase(), "user32.dll");
        assert.equal(isIconic.symbol, "IsIconic");
        assert.equal(
          isIconic.implementationId,
          "user32.is-iconic.logical-window-style.v1",
        );
        assert.deepEqual(isIconic.arguments, [0x4801_0001]);
        assert.deepEqual(isIconic.result, { action: "set", value: 0 });
        assert.equal(isIconic.returnAddress, 0x1008_89c4);
        assert.equal(isIconic.stackPointerBefore, 0x006f_f8d8);
        assert.equal(isIconic.stackPointerAfter, 0x006f_f8e0);
        assert.equal(isIconic.lastError, 0);
        assert.deepEqual(isIconic.guestWrites, []);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_291);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 8,
          address: 0x7f00_0070,
          module: "WINMM.dll",
          symbol: { kind: "name", name: "timeGetTime" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_14e4,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x1008_89d5, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_1ebf);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8d8);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "10137380172e018f7846166e399db43c34782052640d3c87126e1ca32555e4da",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_117);
        assert.equal(oracle.synchronization.dirtyPages, 244_483);
        assert.equal(oracle.synchronization.changedBytes, 43_070_766);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_338);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_862);
        assert.equal(browser.report.writebackPages, 282_524);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_291);
        assert.equal(browser.report.slices, 183_869);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_698);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_977);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_411);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_962);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "2ecc4577b2932d1b67de7def5c1d65cbd83ef0b300d63ceed326af363dda3c72",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_962);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_139,
        );
        assert.equal(
          oracleCheckpoint
            .getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const getSystemMetricsFixedFrameHeight =
          oracleCheckpoint.getSystemMetricsFixedFrameHeightPostRelease;
        assert.equal(
          getSystemMetricsFixedFrameHeight.runtimeThreadId,
          8_192,
        );
        assert.equal(getSystemMetricsFixedFrameHeight.sequence, 161_962);
        assert.equal(getSystemMetricsFixedFrameHeight.occurrence, 68);
        assert.equal(
          getSystemMetricsFixedFrameHeight.trapInstructionCount,
          321_463_139,
        );
        assert.equal(
          getSystemMetricsFixedFrameHeight.module.toLowerCase(),
          "user32.dll",
        );
        assert.equal(
          getSystemMetricsFixedFrameHeight.symbol,
          "GetSystemMetrics",
        );
        assert.equal(
          getSystemMetricsFixedFrameHeight.implementationId,
          "user32.get-system-metrics.v1",
        );
        assert.deepEqual(getSystemMetricsFixedFrameHeight.arguments, [8]);
        assert.deepEqual(getSystemMetricsFixedFrameHeight.result, {
          action: "set",
          value: 3,
        });
        assert.equal(
          getSystemMetricsFixedFrameHeight.returnAddress,
          0x1008_8590,
        );
        assert.equal(
          getSystemMetricsFixedFrameHeight.stackPointerBefore,
          0x006f_f8a0,
        );
        assert.equal(
          getSystemMetricsFixedFrameHeight.stackPointerAfter,
          0x006f_f8a8,
        );
        assert.equal(getSystemMetricsFixedFrameHeight.lastError, 0);
        assert.deepEqual(getSystemMetricsFixedFrameHeight.guestWrites, []);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_282);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 183,
          address: 0x7f00_0b60,
          module: "USER32.dll",
          symbol: { kind: "name", name: "IsIconic" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_13b4,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x4801_0001, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_89c4);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8d8);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "ac46e0896cc007c49ac9339a8b4747b894ea0dd99551c66faac13ac094e80820",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_116);
        assert.equal(oracle.synchronization.dirtyPages, 244_482);
        assert.equal(oracle.synchronization.changedBytes, 43_070_759);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_337);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_861);
        assert.equal(browser.report.writebackPages, 282_523);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_282);
        assert.equal(browser.report.slices, 183_868);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_695);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_976);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_409);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-get-system-metrics-caption-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_961);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "4c3df59720b48601c51755ae06eb7d7643660eeec435126672ccbdac01a86295",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-system-metrics-caption-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_961);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_133,
        );
        assert.equal(
          oracleCheckpoint
            .getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const getSystemMetricsCaption =
          oracleCheckpoint.getSystemMetricsCaptionPostRelease;
        assert.equal(getSystemMetricsCaption.runtimeThreadId, 8_192);
        assert.equal(getSystemMetricsCaption.sequence, 161_961);
        assert.equal(getSystemMetricsCaption.occurrence, 67);
        assert.equal(
          getSystemMetricsCaption.trapInstructionCount,
          321_463_133,
        );
        assert.equal(
          getSystemMetricsCaption.module.toLowerCase(),
          "user32.dll",
        );
        assert.equal(getSystemMetricsCaption.symbol, "GetSystemMetrics");
        assert.equal(
          getSystemMetricsCaption.implementationId,
          "user32.get-system-metrics.v1",
        );
        assert.deepEqual(getSystemMetricsCaption.arguments, [4]);
        assert.deepEqual(getSystemMetricsCaption.result, {
          action: "set",
          value: 23,
        });
        assert.equal(getSystemMetricsCaption.returnAddress, 0x1008_8582);
        assert.equal(
          getSystemMetricsCaption.stackPointerBefore,
          0x006f_f8a0,
        );
        assert.equal(
          getSystemMetricsCaption.stackPointerAfter,
          0x006f_f8a8,
        );
        assert.equal(getSystemMetricsCaption.lastError, 0);
        assert.deepEqual(getSystemMetricsCaption.guestWrites, []);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_139);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 203,
          address: 0x7f00_0ca0,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetSystemMetrics" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1404,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [8, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_8590);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8a0);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "fa931794640b51f2ba74f421f727b3cf1e80dad88c6db7418781d1c3ccd9f2a8",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_115);
        assert.equal(oracle.synchronization.dirtyPages, 244_481);
        assert.equal(oracle.synchronization.changedBytes, 43_070_698);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_336);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_860);
        assert.equal(browser.report.writebackPages, 282_522);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_139);
        assert.equal(browser.report.slices, 183_867);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_692);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_975);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_407);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-get-system-metrics-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_960);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "5ffb12979f68d21fadfb02caf67d2f5a47d7c51c77495a84be0aa7b1d77f0fed",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-system-metrics-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_960);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_128,
        );
        assert.equal(
          oracleCheckpoint
            .getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const getSystemMetrics =
          oracleCheckpoint.getSystemMetricsPostRelease;
        assert.equal(getSystemMetrics.runtimeThreadId, 8_192);
        assert.equal(getSystemMetrics.sequence, 161_960);
        assert.equal(getSystemMetrics.occurrence, 66);
        assert.equal(getSystemMetrics.trapInstructionCount, 321_463_128);
        assert.equal(getSystemMetrics.module.toLowerCase(), "user32.dll");
        assert.equal(getSystemMetrics.symbol, "GetSystemMetrics");
        assert.equal(
          getSystemMetrics.implementationId,
          "user32.get-system-metrics.v1",
        );
        assert.deepEqual(getSystemMetrics.arguments, [7]);
        assert.deepEqual(getSystemMetrics.result, {
          action: "set",
          value: 3,
        });
        assert.equal(getSystemMetrics.returnAddress, 0x1008_8576);
        assert.equal(getSystemMetrics.stackPointerBefore, 0x006f_f8a0);
        assert.equal(getSystemMetrics.stackPointerAfter, 0x006f_f8a8);
        assert.equal(getSystemMetrics.lastError, 0);
        assert.deepEqual(getSystemMetrics.guestWrites, []);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_133);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 203,
          address: 0x7f00_0ca0,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetSystemMetrics" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1404,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [4, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_8582);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8a0);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "d2a2edcf948a3d045a0d8394baa6a6cb7043862de2d047f849b77f8f7b06a3cf",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_114);
        assert.equal(oracle.synchronization.dirtyPages, 244_480);
        assert.equal(oracle.synchronization.changedBytes, 43_070_696);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_335);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_859);
        assert.equal(browser.report.writebackPages, 282_521);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_133);
        assert.equal(browser.report.slices, 183_866);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_689);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_974);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_405);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-get-cursor-pos-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_959);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "5f716c0c3da232b672e4b67e6037240a9549e9e1469ed51df579559368220c4d",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-cursor-pos-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_959);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_119,
        );
        assert.equal(
          oracleCheckpoint.getCursorPosPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const getCursorPos = oracleCheckpoint.getCursorPosPostRelease;
        assert.equal(getCursorPos.runtimeThreadId, 8_192);
        assert.equal(getCursorPos.sequence, 161_959);
        assert.equal(getCursorPos.occurrence, 17);
        assert.equal(getCursorPos.trapInstructionCount, 321_463_119);
        assert.equal(getCursorPos.module.toLowerCase(), "user32.dll");
        assert.equal(getCursorPos.symbol, "GetCursorPos");
        assert.equal(
          getCursorPos.implementationId,
          "user32.get-cursor-pos.v1",
        );
        assert.deepEqual(getCursorPos.arguments, [0x006f_f8b4]);
        assert.deepEqual(getCursorPos.result, { action: "set", value: 1 });
        assert.equal(getCursorPos.returnAddress, 0x1008_8563);
        assert.equal(getCursorPos.stackPointerBefore, 0x006f_f8a0);
        assert.equal(getCursorPos.stackPointerAfter, 0x006f_f8a8);
        assert.equal(getCursorPos.lastError, 0);
        assert.equal(getCursorPos.guestWrites.length, 1);
        assert.equal(getCursorPos.guestWrites[0].address, 0x006f_f8b4);
        assert.deepEqual(getCursorPos.guestWrites[0].bytes, [
          0xc0, 0x03, 0, 0, 0x1c, 0x02, 0, 0,
        ]);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_128);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 203,
          address: 0x7f00_0ca0,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetSystemMetrics" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1404,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [7, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_8576);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8a0);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "499462e38e04df0dfb6c404a7a46ab1123786b3191c3e4725d58139d452d9dc3",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_113);
        assert.equal(oracle.synchronization.dirtyPages, 244_479);
        assert.equal(oracle.synchronization.changedBytes, 43_070_692);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_939);
        assert.equal(oracle.synchronization.finalGeneration, 296_334);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_858);
        assert.equal(browser.report.writebackPages, 282_520);
        assert.equal(browser.report.mappingDeltas, 127_939);
        assert.equal(browser.report.mappingDirtyPages, 150_344);
        assert.equal(browser.report.instructions, 321_463_128);
        assert.equal(browser.report.slices, 183_865);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_686);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_973);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_403);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-get-window-rect-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_958);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "b19207accb1716be7ad4f0b2953e6041a7652002268e204710eef866d05291a7",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-get-window-rect-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_958);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_115,
        );
        assert.equal(oracleCheckpoint.maximumRepeatCompletions, 16);
        assert.equal(
          oracleCheckpoint.timeGetTimePostReleaseCompletions.length,
          4,
        );
        assert.equal(
          oracleCheckpoint
            .getWindowRectPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const getWindowRect = oracleCheckpoint.getWindowRectPostRelease;
        assert.equal(getWindowRect.runtimeThreadId, 8_192);
        assert.equal(getWindowRect.sequence, 161_958);
        assert.equal(getWindowRect.occurrence, 17);
        assert.equal(getWindowRect.trapInstructionCount, 321_463_115);
        assert.equal(getWindowRect.module.toLowerCase(), "user32.dll");
        assert.equal(getWindowRect.symbol, "GetWindowRect");
        assert.equal(
          getWindowRect.implementationId,
          "user32.get-window-rect.v1",
        );
        assert.deepEqual(getWindowRect.arguments, [
          0x4801_0001, 0x006f_f8bc,
        ]);
        assert.deepEqual(getWindowRect.result, { action: "set", value: 1 });
        assert.equal(getWindowRect.returnAddress, 0x1008_8558);
        assert.equal(getWindowRect.stackPointerBefore, 0x006f_f89c);
        assert.equal(getWindowRect.stackPointerAfter, 0x006f_f8a8);
        assert.equal(getWindowRect.lastError, 0);
        assert.equal(getWindowRect.guestWrites.length, 1);
        assert.equal(getWindowRect.guestWrites[0].address, 0x006f_f8bc);
        assert.deepEqual(getWindowRect.guestWrites[0].bytes, [
          0x3d, 0x01, 0, 0, 0x9a, 0, 0, 0, 0x43, 0x06, 0, 0, 0x87,
          0x03, 0, 0,
        ]);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_463_119);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 190,
          address: 0x7f00_0bd0,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetCursorPos" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_13d0,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x006f_f8b4, 0]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_8563);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8a0);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "75cab525352438b4b85f2def0a5e834d3ce48650ba61d290c65194cc0124ea5b",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_112);
        assert.equal(oracle.synchronization.dirtyPages, 244_478);
        assert.equal(oracle.synchronization.changedBytes, 43_070_688);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_938);
        assert.equal(oracle.synchronization.finalGeneration, 296_332);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_857);
        assert.equal(browser.report.writebackPages, 282_519);
        assert.equal(browser.report.mappingDeltas, 127_938);
        assert.equal(browser.report.mappingDirtyPages, 150_343);
        assert.equal(browser.report.instructions, 321_463_119);
        assert.equal(browser.report.slices, 183_864);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_683);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_971);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_400);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "winmm-time-get-time-post-pointer-release-repeat-batch"
      ) {
        assert.equal(oracle.supportedCallCount, 161_957);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "103c51e071e81d2548d97009caf4ec21f0aae06cd596854ce1b2972391646948",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.winmm-time-get-time-post-pointer-release-repeat-batch.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 4);
        assert.equal(oracleCheckpoint.completionCount, 4);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_957);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_463_010,
        );
        assert.equal(oracleCheckpoint.maximumRepeatCompletions, 16);
        assert.equal(
          oracleCheckpoint.timeGetTimePostReleaseCompletions.length,
          4,
        );
        assert.equal(oracleCheckpoint.instructionCount, 321_463_115);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 228,
          address: 0x7f00_0e30,
          module: "USER32.dll",
          symbol: { kind: "name", name: "GetWindowRect" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1468,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [
          0x4801_0001, 0x006f_f8bc,
        ]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1008_8558);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f89c);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "a9d1a45f4b8d5ba37bc1d0477f15f642eaf5ec9352b97b11b5d1c40667c8b1b0",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_111);
        assert.equal(oracle.synchronization.dirtyPages, 244_477);
        assert.equal(oracle.synchronization.changedBytes, 43_070_683);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_937);
        assert.equal(oracle.synchronization.finalGeneration, 296_330);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_856);
        assert.equal(browser.report.writebackPages, 282_518);
        assert.equal(browser.report.mappingDeltas, 127_937);
        assert.equal(browser.report.mappingDirtyPages, 150_342);
        assert.equal(browser.report.instructions, 321_463_115);
        assert.equal(browser.report.slices, 183_863);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_680);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_969);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_397);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "winmm-time-get-time-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_954);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "f3af63c202507aa5c593ac89707cc4fa65f49044fece05307bdfe65870ae91de",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.winmm-time-get-time-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_954);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_555,
        );
        assert.equal(
          oracleCheckpoint.timeGetTimePostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        const postReleaseTimeGetTime =
          oracleCheckpoint.timeGetTimePostRelease;
        assert.equal(postReleaseTimeGetTime.runtimeThreadId, 8_192);
        assert.equal(postReleaseTimeGetTime.sequence, 161_954);
        assert.equal(postReleaseTimeGetTime.occurrence, 219);
        assert.equal(
          postReleaseTimeGetTime.trapInstructionCount,
          321_462_555,
        );
        assert.equal(postReleaseTimeGetTime.module.toLowerCase(), "winmm.dll");
        assert.equal(postReleaseTimeGetTime.symbol, "timeGetTime");
        assert.equal(
          postReleaseTimeGetTime.implementationId,
          "winmm.time-get-time.v1",
        );
        assert.deepEqual(postReleaseTimeGetTime.arguments, []);
        assert.deepEqual(postReleaseTimeGetTime.result, {
          action: "set",
          value: 3_601_224,
        });
        assert.equal(postReleaseTimeGetTime.returnAddress, 0x1009_1ebf);
        assert.equal(postReleaseTimeGetTime.stackPointerBefore, 0x006f_f904);
        assert.equal(postReleaseTimeGetTime.stackPointerAfter, 0x006f_f908);
        assert.equal(postReleaseTimeGetTime.lastError, 0);
        assert.deepEqual(postReleaseTimeGetTime.guestWrites, []);
        assert.deepEqual(postReleaseTimeGetTime.tickCount, {
          returnedTickCount: 3_601_224,
          nextTickCount: 3_601_225,
          incrementPerRead: 1,
        });
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_462_721);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 8,
          address: 0x7f00_0070,
          module: "WINMM.dll",
          symbol: { kind: "name", name: "timeGetTime" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_14e4,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, []);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_1ebf);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f794);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "98474db8134f2b2be561cec20f73a615ba37b21580c38bc57178a2be05bdb099",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_108);
        assert.equal(oracle.synchronization.dirtyPages, 244_469);
        assert.equal(oracle.synchronization.changedBytes, 43_070_560);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_937);
        assert.equal(oracle.synchronization.finalGeneration, 296_327);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_853);
        assert.equal(browser.report.writebackPages, 282_510);
        assert.equal(browser.report.mappingDeltas, 127_937);
        assert.equal(browser.report.mappingDirtyPages, 150_342);
        assert.equal(browser.report.instructions, 321_462_721);
        assert.equal(browser.report.slices, 183_860);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_671);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_966);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_391);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-peek-message-a-post-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_953);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "15a113d7914ec12a9f54df3a4916c35f2829dc10b3bfca067b0a8cf6b5f6757a",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-peek-message-a-post-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_953);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_521,
        );
        assert.equal(
          oracleCheckpoint
            .peekMessagePostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(oracleCheckpoint.postReleaseQueueDepthBefore, 0);
        assert.equal(oracleCheckpoint.postReleaseQueueDepthAfter, 0);
        assert.equal(
          oracleCheckpoint.postReleaseMessageBytesPreserved,
          true,
        );
        const postReleasePeek = oracleCheckpoint.peekMessagePostRelease;
        assert.equal(postReleasePeek.runtimeThreadId, 8_192);
        assert.equal(postReleasePeek.sequence, 161_953);
        assert.equal(postReleasePeek.occurrence, 12);
        assert.equal(postReleasePeek.trapInstructionCount, 321_462_521);
        assert.equal(postReleasePeek.module.toLowerCase(), "user32.dll");
        assert.equal(postReleasePeek.symbol, "PeekMessageA");
        assert.equal(
          postReleasePeek.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.deepEqual(postReleasePeek.arguments, [
          0x006f_f8e8,
          0,
          0,
          0,
          1,
        ]);
        assert.deepEqual(postReleasePeek.result, {
          action: "set",
          value: 0,
        });
        assert.equal(postReleasePeek.returnAddress, 0x1009_37bb);
        assert.equal(postReleasePeek.stackPointerBefore, 0x006f_f8c4);
        assert.equal(postReleasePeek.stackPointerAfter, 0x006f_f8dc);
        assert.equal(postReleasePeek.lastError, 0);
        assert.deepEqual(postReleasePeek.guestWrites, []);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_462_555);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 8,
          address: 0x7f00_0070,
          module: "WINMM.dll",
          symbol: { kind: "name", name: "timeGetTime" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_14e4,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, []);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_1ebf);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f904);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "ad248923d1c232778ed6860164ebe1ad9fe4ffcea00154e758abbbca4f538e66",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_107);
        assert.equal(oracle.synchronization.dirtyPages, 244_465);
        assert.equal(oracle.synchronization.changedBytes, 43_070_511);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_937);
        assert.equal(oracle.synchronization.finalGeneration, 296_326);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_852);
        assert.equal(browser.report.writebackPages, 282_506);
        assert.equal(browser.report.mappingDeltas, 127_937);
        assert.equal(browser.report.mappingDirtyPages, 150_342);
        assert.equal(browser.report.instructions, 321_462_555);
        assert.equal(browser.report.slices, 183_859);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_668);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_965);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_389);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-def-window-proc-a-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_952);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "d884a63fe998798edcd606766c990d11e121dda2947685ce5439f85a00d2380c",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-def-window-proc-a-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_952);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_499,
        );
        assert.equal(
          oracleCheckpoint.defWindowProcAReleaseActiveCallbackDepthAtCompletion,
          1,
        );
        const releaseDefWindowProc = oracleCheckpoint.defWindowProcARelease;
        assert.equal(releaseDefWindowProc.runtimeThreadId, 8_192);
        assert.equal(releaseDefWindowProc.sequence, 161_952);
        assert.equal(releaseDefWindowProc.occurrence, 27);
        assert.equal(
          releaseDefWindowProc.trapInstructionCount,
          321_462_499,
        );
        assert.equal(releaseDefWindowProc.module.toLowerCase(), "user32.dll");
        assert.equal(releaseDefWindowProc.symbol, "DefWindowProcA");
        assert.equal(
          releaseDefWindowProc.implementationId,
          "user32.def-window-proc-a.v1",
        );
        assert.deepEqual(releaseDefWindowProc.arguments, [
          0x4801_0001,
          0x0202,
          0,
          0x0168_0280,
        ]);
        assert.deepEqual(releaseDefWindowProc.result, {
          action: "set",
          value: 0,
        });
        assert.equal(releaseDefWindowProc.lastError, 0);
        assert.deepEqual(releaseDefWindowProc.guestWrites, []);
        assert.deepEqual(oracleCheckpoint.releaseCallbackReturn, {
          action: "return",
          sequence: 161_951,
          frameId: 34,
          instructionCount: 321_462_512,
          result: 0,
        });
        assert.equal(oracleCheckpoint.dispatchMessageReleaseResult, 0);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.instructionCount, 321_462_521);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 164,
          address: 0x7f00_0a30,
          module: "USER32.dll",
          symbol: { kind: "name", name: "PeekMessageA" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1368,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [
          0x006f_f8e8,
          0,
          0,
          0,
          1,
        ]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_37bb);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8c4);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "3f84e3a33f2f3d23856d235cbede4d95c420dc61951d2c2646af6e96213be529",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_106);
        assert.equal(oracle.synchronization.dirtyPages, 244_464);
        assert.equal(oracle.synchronization.changedBytes, 43_070_501);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_937);
        assert.equal(oracle.synchronization.finalGeneration, 296_325);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_851);
        assert.equal(browser.report.writebackPages, 282_505);
        assert.equal(browser.report.mappingDeltas, 127_937);
        assert.equal(browser.report.mappingDirtyPages, 150_342);
        assert.equal(browser.report.instructions, 321_462_521);
        assert.equal(browser.report.slices, 183_858);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_665);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_964);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_387);
        assert.equal(browser.statusMessagesObserved, 24_347);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-dispatch-message-a-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_950);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "477a479e2ce3f9c8ee67e509c5ae9e65c9c870c569898c8163e92e7b5680e473",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-dispatch-message-a-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_950);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_431,
        );
        assert.equal(oracleCheckpoint.requiredEntries, 1);
        assert.equal(oracleCheckpoint.entryCount, 1);
        assert.equal(oracleCheckpoint.lastEntrySequence, 161_951);
        assert.equal(
          oracleCheckpoint.lastEntryInstructionCount,
          321_462_435,
        );
        assert.equal(oracleCheckpoint.requiredPointerMessages, 1);
        assert.equal(oracleCheckpoint.pointerMessageDeliveryCount, 1);
        assert.equal(oracleCheckpoint.releaseMessageQueued, false);
        assert.equal(
          oracleCheckpoint.peekMessageReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(
          oracleCheckpoint
            .translateMessageReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(
          oracleCheckpoint.dispatchMessageReleaseActiveCallbackDepthAtEntry,
          1,
        );
        assert.equal(oracleCheckpoint.queueDepthBefore, 1);
        assert.equal(oracleCheckpoint.queueDepthAfter, 0);
        assert.equal(oracleCheckpoint.queueDepthAtTranslateCompletion, 0);
        assert.equal(oracleCheckpoint.queueDepthAtDispatchEntry, 0);
        assert.equal(
          oracleCheckpoint.translateMessageReleaseBytesPreserved,
          true,
        );
        assert.equal(
          oracleCheckpoint.dispatchMessageReleaseBytesPreserved,
          true,
        );
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 1);
        const releaseDispatch = oracleCheckpoint.dispatchMessageRelease;
        assert.equal(releaseDispatch.runtimeThreadId, 8_192);
        assert.equal(releaseDispatch.sequence, 161_951);
        assert.equal(releaseDispatch.occurrence, 4);
        assert.equal(releaseDispatch.trapInstructionCount, 321_462_435);
        assert.equal(releaseDispatch.module.toLowerCase(), "user32.dll");
        assert.equal(releaseDispatch.symbol, "DispatchMessageA");
        assert.equal(
          releaseDispatch.implementationId,
          "user32.dispatch-message-a.window-timer-pointer-callback.v2",
        );
        assert.deepEqual(releaseDispatch.arguments, [0x006f_f8e8]);
        assert.equal(releaseDispatch.pendingCallback, true);
        assert.deepEqual(oracleCheckpoint.releaseCallbackEntry, {
          action: "enter",
          sequence: 161_951,
          runtimeThreadId: 8_192,
          frameId: 34,
          instructionCount: 321_462_435,
          depth: 1,
          target: 0x1009_2550,
          allocationStart: 0x006f_f8c0,
          stackPointer: 0x006f_f8c0,
          scratchAddress: 0,
          scratchBytes: 0,
          frameBytes: 20,
          returnSentinel: 0x7f01_0000,
          arguments: [
            0x4801_0001,
            0x0202,
            0,
            0x0168_0280,
          ],
        });
        assert.equal(oracleCheckpoint.instructionCount, 321_462_499);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 233,
          address: 0x7f00_0e80,
          module: "USER32.dll",
          symbol: { kind: "name", name: "DefWindowProcA" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_147c,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [
          0x4801_0001,
          0x0202,
          0,
          0x0168_0280,
        ]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_33bf);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f50c);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "3af9084b2b0447ce24bb8edd1cf7ee159d333f6a6f858cd5b070720d43c9deac",
        );
        assert.equal(oracle.synchronization.writebackBatches, 165_104);
        assert.equal(oracle.synchronization.dirtyPages, 244_462);
        assert.equal(oracle.synchronization.changedBytes, 43_070_479);
        assert.equal(oracle.synchronization.hostDeltaBatches, 127_937);
        assert.equal(oracle.synchronization.finalGeneration, 296_323);
        assert.equal(oracle.threadExecution.contextSwitches, 2_512);
        assert.equal(browser.report.writebackBatches, 183_849);
        assert.equal(browser.report.writebackPages, 282_503);
        assert.equal(browser.report.mappingDeltas, 127_937);
        assert.equal(browser.report.mappingDirtyPages, 150_342);
        assert.equal(browser.report.instructions, 321_462_499);
        assert.equal(browser.report.slices, 183_856);
        assert.equal(browser.report.budgetYields, 18_752);
        assert.equal(browser.report.executionTransitionCount, 588_660);
        assert.equal(browser.report.initialGeneration, 27);
        assert.equal(browser.report.finalGeneration, 314_962);
        assert.equal(browser.report.contextSwitches, 2_512);
        assert.equal(browser.report.callbackEntries, 34);
        assert.equal(browser.report.maximumCallbackDepth, 3);
        assert.equal(browser.report.wasmBridgeCalls, 489_383);
        assert.equal(browser.statusMessagesObserved, 24_346);
        assert.equal(browser.statusMessagesRetained, 4_096);
      }
      if (
        oracleCheckpoint.capability ===
        "user32-translate-message-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_950);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "477a479e2ce3f9c8ee67e509c5ae9e65c9c870c569898c8163e92e7b5680e473",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-translate-message-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_950);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_431,
        );
        assert.equal(oracleCheckpoint.requiredPointerMessages, 1);
        assert.equal(oracleCheckpoint.pointerMessageDeliveryCount, 1);
        assert.equal(oracleCheckpoint.releaseMessageQueued, false);
        assert.equal(
          oracleCheckpoint.peekMessageReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(
          oracleCheckpoint
            .translateMessageReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(
          oracleCheckpoint.translateMessageReleaseBytesPreserved,
          true,
        );
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.queueDepthBefore, 1);
        assert.equal(oracleCheckpoint.queueDepthAfter, 0);
        assert.equal(oracleCheckpoint.queueDepthAtTranslateCompletion, 0);
        assert.deepEqual(oracleCheckpoint.releaseMessage, {
          id: 4,
          source: {
            kind: "pointer-input",
            event_sequence: 2,
          },
          targetThreadId: 8_192,
          hwnd: 0x4801_0001,
          message: 0x0202,
          wParam: 0,
          lParam: 0x0168_0280,
          time: 3_601_223,
          pointX: 960,
          pointY: 540,
        });
        const releasePeek = oracleCheckpoint.peekMessageRelease;
        assert.equal(releasePeek.runtimeThreadId, 8_192);
        assert.equal(releasePeek.sequence, 161_949);
        assert.equal(releasePeek.occurrence, 11);
        assert.equal(releasePeek.trapInstructionCount, 321_462_423);
        assert.equal(releasePeek.module.toLowerCase(), "user32.dll");
        assert.equal(releasePeek.symbol, "PeekMessageA");
        assert.equal(
          releasePeek.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.deepEqual(releasePeek.arguments, [
          0x006f_f8e8,
          0,
          0,
          0,
          1,
        ]);
        assert.deepEqual(releasePeek.result, {
          action: "set",
          value: 1,
        });
        assert.equal(releasePeek.lastError, 0);
        assert.equal(releasePeek.guestWrites.length, 1);
        const releaseTranslate = oracleCheckpoint.translateMessageRelease;
        assert.equal(releaseTranslate.runtimeThreadId, 8_192);
        assert.equal(releaseTranslate.sequence, 161_950);
        assert.equal(releaseTranslate.occurrence, 4);
        assert.equal(
          releaseTranslate.trapInstructionCount,
          321_462_431,
        );
        assert.equal(
          releaseTranslate.module.toLowerCase(),
          "user32.dll",
        );
        assert.equal(releaseTranslate.symbol, "TranslateMessage");
        assert.equal(
          releaseTranslate.implementationId,
          "user32.translate-message.window-timer-pointer-noop.v2",
        );
        assert.deepEqual(releaseTranslate.arguments, [0x006f_f8e8]);
        assert.deepEqual(releaseTranslate.result, {
          action: "set",
          value: 0,
        });
        assert.equal(releaseTranslate.returnAddress, 0x1009_37a2);
        assert.equal(releaseTranslate.stackPointerBefore, 0x006f_f8d4);
        assert.equal(releaseTranslate.stackPointerAfter, 0x006f_f8dc);
        assert.equal(releaseTranslate.lastError, 0);
        assert.deepEqual(releaseTranslate.guestWrites, []);
        assert.equal(releaseTranslate.windowTimer, undefined);
        assert.equal(releaseTranslate.queuedThreadMessages, undefined);
        assert.equal(oracleCheckpoint.instructionCount, 321_462_435);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 182,
          address: 0x7f00_0b50,
          module: "USER32.dll",
          symbol: { kind: "name", name: "DispatchMessageA" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_13b0,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x006f_f8e8]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_37ac);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8d4);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "2f9626f3b3d8b8a0643ef7d1f454d44c37de4067cabe87d706d5a991e48d68d1",
        );
      }
      if (
        oracleCheckpoint.capability ===
        "user32-peek-message-a-pointer-release-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_949);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "7754804351d4e484a945df77b22300fad26444360bacaf8550e3ce0ee0c3d0de",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-peek-message-a-pointer-release-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_949);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_423,
        );
        assert.equal(oracleCheckpoint.requiredPointerMessages, 1);
        assert.equal(oracleCheckpoint.pointerMessageDeliveryCount, 1);
        assert.equal(oracleCheckpoint.releaseMessageQueued, false);
        assert.equal(
          oracleCheckpoint.peekMessageReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.queueDepthBefore, 1);
        assert.equal(oracleCheckpoint.queueDepthAfter, 0);
        assert.deepEqual(oracleCheckpoint.releaseMessage, {
          id: 4,
          source: {
            kind: "pointer-input",
            event_sequence: 2,
          },
          targetThreadId: 8_192,
          hwnd: 0x4801_0001,
          message: 0x0202,
          wParam: 0,
          lParam: 0x0168_0280,
          time: 3_601_223,
          pointX: 960,
          pointY: 540,
        });
        const releasePeek = oracleCheckpoint.peekMessageRelease;
        assert.equal(releasePeek.runtimeThreadId, 8_192);
        assert.equal(releasePeek.sequence, 161_949);
        assert.equal(releasePeek.occurrence, 11);
        assert.equal(releasePeek.trapInstructionCount, 321_462_423);
        assert.equal(releasePeek.module.toLowerCase(), "user32.dll");
        assert.equal(releasePeek.symbol, "PeekMessageA");
        assert.equal(
          releasePeek.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.deepEqual(releasePeek.arguments, [
          0x006f_f8e8,
          0,
          0,
          0,
          1,
        ]);
        assert.deepEqual(releasePeek.result, {
          action: "set",
          value: 1,
        });
        assert.equal(releasePeek.returnAddress, 0x1009_37bb);
        assert.equal(releasePeek.stackPointerBefore, 0x006f_f8c4);
        assert.equal(releasePeek.stackPointerAfter, 0x006f_f8dc);
        assert.equal(releasePeek.lastError, 0);
        assert.deepEqual(
          releasePeek.guestWrites.map(({ address, bytes }) => ({
            address,
            bytes,
          })),
          [
            {
              address: 0x006f_f8e8,
              bytes: [
                0x01, 0x00, 0x01, 0x48, 0x02, 0x02, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x80, 0x02, 0x68, 0x01, 0x47, 0xf3,
                0x36, 0x00, 0xc0, 0x03, 0x00, 0x00, 0x1c, 0x02, 0x00,
                0x00,
              ],
            },
          ],
        );
        assert.deepEqual(releasePeek.guestWrites[0]?.result, {
          bytesWritten: 28,
          dirtyPages: 1,
          invalidation: {
            generation: 296_319,
            dataTlb: false,
            instructionFetch: false,
            generatedCode: false,
          },
        });
        assert.equal(oracleCheckpoint.instructionCount, 321_462_431);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 162,
          address: 0x7f00_0a10,
          module: "USER32.dll",
          symbol: { kind: "name", name: "TranslateMessage" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1360,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [0x006f_f8e8]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_37a2);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8d4);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "3083e677a1fb9192ce07e416829d58b875965b2e60265363682d5ae91db0e542",
        );
      }
      if (
        oracleCheckpoint.capability ===
        "user32-def-window-proc-a-action-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_948);
        assert.match(
          oracle.supportedCallSemanticsSha256,
          /^[0-9a-f]{64}$/u,
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.user32-def-window-proc-a-action-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_948);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_401,
        );
        assert.equal(oracleCheckpoint.requiredPointerMessages, 1);
        assert.equal(oracleCheckpoint.pointerMessageDeliveryCount, 1);
        assert.equal(oracleCheckpoint.releaseMessageQueued, true);
        assert.equal(
          oracleCheckpoint.setFocusActiveCallbackDepthAtStop,
          1,
        );
        assert.equal(
          oracleCheckpoint.timeGetTimeActiveCallbackDepthAtCompletion,
          1,
        );
        assert.equal(
          oracleCheckpoint.defWindowProcAActiveCallbackDepthAtCompletion,
          1,
        );
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 0);
        assert.equal(oracleCheckpoint.setFocus.sequence, 161_946);
        assert.equal(oracleCheckpoint.timeGetTime.sequence, 161_947);
        assert.equal(
          oracleCheckpoint.defWindowProcA.runtimeThreadId,
          8_192,
        );
        assert.equal(oracleCheckpoint.defWindowProcA.sequence, 161_948);
        assert.equal(oracleCheckpoint.defWindowProcA.occurrence, 26);
        assert.equal(
          oracleCheckpoint.defWindowProcA.trapInstructionCount,
          321_462_401,
        );
        assert.equal(
          oracleCheckpoint.defWindowProcA.module.toLowerCase(),
          "user32.dll",
        );
        assert.equal(
          oracleCheckpoint.defWindowProcA.symbol,
          "DefWindowProcA",
        );
        assert.equal(
          oracleCheckpoint.defWindowProcA.implementationId,
          "user32.def-window-proc-a.v1",
        );
        assert.deepEqual(oracleCheckpoint.defWindowProcA.arguments, [
          0x4801_0001,
          0x0201,
          1,
          0x0168_0280,
        ]);
        assert.deepEqual(oracleCheckpoint.defWindowProcA.result, {
          action: "set",
          value: 0,
        });
        assert.equal(
          oracleCheckpoint.defWindowProcA.returnAddress,
          0x1009_33bf,
        );
        assert.equal(
          oracleCheckpoint.defWindowProcA.stackPointerBefore,
          0x006f_f50c,
        );
        assert.equal(
          oracleCheckpoint.defWindowProcA.stackPointerAfter,
          0x006f_f520,
        );
        assert.equal(oracleCheckpoint.defWindowProcA.lastError, 0);
        assert.deepEqual(oracleCheckpoint.defWindowProcA.guestWrites, []);
        assert.equal(oracleCheckpoint.instructionCount, 321_462_423);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 164,
          address: 0x7f00_0a30,
          module: "USER32.dll",
          symbol: { kind: "name", name: "PeekMessageA" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_1368,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [
          0x006f_f8e8,
          0,
          0,
          0,
          1,
        ]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_37bb);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f8c4);
        assert.match(
          oracleCheckpoint.memoryContentSha256,
          /^[0-9a-f]{64}$/u,
        );
      }
      if (
        oracleCheckpoint.capability ===
        "winmm-time-get-time-action-continuation"
      ) {
        assert.equal(oracle.supportedCallCount, 161_946);
        assert.equal(
          oracle.supportedCallSemanticsSha256,
          "76d10d5a5fc53ddf14496b6a6b223233cb690df235574cae38f6e4c2c7afdd46",
        );
        assert.equal(
          oracleCheckpoint.implementationId,
          "runtime.winmm-time-get-time-action-continuation.v1",
        );
        assert.equal(oracleCheckpoint.requiredCompletions, 1);
        assert.equal(oracleCheckpoint.completionCount, 1);
        assert.equal(oracleCheckpoint.lastCompletionSequence, 161_947);
        assert.equal(
          oracleCheckpoint.lastCompletionInstructionCount,
          321_462_368,
        );
        assert.equal(oracleCheckpoint.requiredPointerMessages, 1);
        assert.equal(oracleCheckpoint.pointerMessageDeliveryCount, 1);
        assert.equal(oracleCheckpoint.releaseMessageQueued, true);
        assert.equal(oracleCheckpoint.activeCallbackDepthAtStop, 1);
        assert.equal(
          oracleCheckpoint.setFocusActiveCallbackDepthAtStop,
          1,
        );
        assert.equal(oracleCheckpoint.setFocus.sequence, 161_946);
        assert.equal(
          oracleCheckpoint.setFocus.implementationId,
          "user32.set-focus.same-current-window.v1",
        );
        assert.equal(oracleCheckpoint.timeGetTime.runtimeThreadId, 8_192);
        assert.equal(oracleCheckpoint.timeGetTime.sequence, 161_947);
        assert.equal(oracleCheckpoint.timeGetTime.occurrence, 218);
        assert.equal(
          oracleCheckpoint.timeGetTime.trapInstructionCount,
          321_462_368,
        );
        assert.equal(
          oracleCheckpoint.timeGetTime.module.toLowerCase(),
          "winmm.dll",
        );
        assert.equal(oracleCheckpoint.timeGetTime.symbol, "timeGetTime");
        assert.equal(
          oracleCheckpoint.timeGetTime.implementationId,
          "winmm.time-get-time.v1",
        );
        assert.deepEqual(oracleCheckpoint.timeGetTime.arguments, []);
        assert.deepEqual(oracleCheckpoint.timeGetTime.result, {
          action: "set",
          value: 3_601_223,
        });
        assert.equal(
          oracleCheckpoint.timeGetTime.returnAddress,
          0x1009_1ebf,
        );
        assert.equal(
          oracleCheckpoint.timeGetTime.stackPointerBefore,
          0x006f_f508,
        );
        assert.equal(
          oracleCheckpoint.timeGetTime.stackPointerAfter,
          0x006f_f50c,
        );
        assert.equal(oracleCheckpoint.timeGetTime.lastError, 0);
        assert.deepEqual(oracleCheckpoint.timeGetTime.guestWrites, []);
        assert.deepEqual(oracleCheckpoint.timeGetTime.tickCount, {
          returnedTickCount: 3_601_223,
          nextTickCount: 3_601_224,
          incrementPerRead: 1,
        });
        assert.equal(oracleCheckpoint.instructionCount, 321_462_401);
        assert.deepEqual(oracleCheckpoint.nextEndpoint, {
          id: 233,
          address: 0x7f00_0e80,
          module: "USER32.dll",
          symbol: { kind: "name", name: "DefWindowProcA" },
          disposition: "declared-runtime-export",
          iatAddress: 0x100d_147c,
        });
        assert.deepEqual(oracleCheckpoint.nextArguments, [
          0x4801_0001,
          0x0201,
          1,
          23_593_600,
        ]);
        assert.equal(oracleCheckpoint.nextReturnAddress, 0x1009_33bf);
        assert.equal(oracleCheckpoint.nextStackPointer, 0x006f_f50c);
        assert.equal(
          oracleCheckpoint.memoryContentSha256,
          "fcfa916b3f18936ba6df5107491dd2cbf0f08f738bd8de67154682f6da31280f",
        );
      }
      assert.deepEqual(
        normalizePointerCheckpoint(browserCheckpoint),
        normalizePointerCheckpoint(oracleCheckpoint),
        "Chrome Worker pointer-message action continuation differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
        "primary-main-loop-recurring-timer-multi-cycle-batch" ||
      oracleCheckpoint.capability ===
        "primary-main-loop-recurring-timer-window-batch" ||
      oracleCheckpoint.capability ===
        "primary-main-loop-presentation-progress-gateway" ||
      oracleCheckpoint.capability ===
        "primary-main-loop-input-progress-gateway" ||
      oracleCheckpoint.capability ===
        "primary-main-loop-action-input-progress-gateway" ||
      oracleCheckpoint.capability ===
        "primary-main-loop-window-timer-callback-action-input-progress-gateway"
    ) {
      const presentationEvidence =
        oracleCheckpoint.capability ===
          "primary-main-loop-presentation-progress-gateway" ||
        oracleCheckpoint.capability ===
          "primary-main-loop-input-progress-gateway" ||
        oracleCheckpoint.capability ===
          "primary-main-loop-action-input-progress-gateway" ||
        oracleCheckpoint.capability ===
          "primary-main-loop-window-timer-callback-action-input-progress-gateway";
      const normalizeCycleBoundaryCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        runtimeThreadId: call.runtimeThreadId,
        trapInstructionCount: call.trapInstructionCount,
        ...(presentationEvidence
          ? { occurrence: call.occurrence }
          : {}),
      });
      const normalizeWindowTimerDispatch = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        windowTimer: call.windowTimer,
        callback: call.callback,
        callbackArguments: call.callbackArguments,
      });
      const normalizeMultiCycleCheckpoint = (checkpoint) => ({
        implementationId: checkpoint.implementationId,
        requiredCycles: checkpoint.requiredCycles,
        minimumRequiredCycles: checkpoint.minimumRequiredCycles,
        maximumCycles: checkpoint.maximumCycles,
        cycleCount: checkpoint.cycleCount,
        requiredCompletions: checkpoint.requiredCompletions,
        minimumRequiredCompletions: checkpoint.minimumRequiredCompletions,
        completionCount: checkpoint.completionCount,
        firstCompletionSequence: checkpoint.firstCompletionSequence,
        firstCompletionInstructionCount:
          checkpoint.firstCompletionInstructionCount,
        lastCompletionSequence: checkpoint.lastCompletionSequence,
        lastCompletionInstructionCount:
          checkpoint.lastCompletionInstructionCount,
        boundaryCycleIndexes: checkpoint.boundaryCycleIndexes,
        boundaryCompletions: checkpoint.boundaryCompletions.map(
          normalizeCycleBoundaryCall,
        ),
        cycles: checkpoint.cycles,
        interCycleCompletionCount: checkpoint.interCycleCompletionCount,
        interCycleIntervals: checkpoint.interCycleIntervals,
        waitCompletionCount: checkpoint.waitCompletionCount,
        criticalSectionCompletionCount:
          checkpoint.criticalSectionCompletionCount,
        criticalSectionAcquisitionCount:
          checkpoint.criticalSectionAcquisitionCount,
        criticalSectionReleaseCount: checkpoint.criticalSectionReleaseCount,
        carriedCycleCount: checkpoint.carriedCycleCount,
        timerArmCompletionCount: checkpoint.timerArmCompletionCount,
        roundtripCount: checkpoint.roundtripCount,
        criticalSectionAddresses: checkpoint.criticalSectionAddresses,
        mainLoopThreadId: checkpoint.mainLoopThreadId,
        participatingThreadIds: checkpoint.participatingThreadIds,
        threadTransitionCount: checkpoint.threadTransitionCount,
        balanced: checkpoint.balanced,
        ...(checkpoint.capability ===
          "primary-main-loop-presentation-progress-gateway" ||
        checkpoint.capability === "primary-main-loop-input-progress-gateway" ||
        checkpoint.capability ===
          "primary-main-loop-action-input-progress-gateway" ||
        checkpoint.capability ===
          "primary-main-loop-window-timer-callback-action-input-progress-gateway"
          ? {
              baseline: checkpoint.baseline,
              outcome: checkpoint.outcome,
              hardBoundReached: checkpoint.hardBoundReached,
              renderCompletionCount: checkpoint.renderCompletionCount,
              renderCompletions: checkpoint.renderCompletions.map((call) => ({
                ...normalizeSemanticCall(call),
                runtimeThreadId: call.runtimeThreadId,
                trapInstructionCount: call.trapInstructionCount,
                occurrence: call.occurrence,
              })),
              presentCompletion:
                checkpoint.presentCompletion === null
                  ? null
                  : {
                      ...normalizeSemanticCall(checkpoint.presentCompletion),
                      runtimeThreadId:
                        checkpoint.presentCompletion.runtimeThreadId,
                      trapInstructionCount:
                        checkpoint.presentCompletion.trapInstructionCount,
                      occurrence: checkpoint.presentCompletion.occurrence,
                    },
              timerLanes: checkpoint.timerLanes,
            }
          : {}),
        ...(checkpoint.capability ===
          "primary-main-loop-input-progress-gateway" ||
        checkpoint.capability ===
          "primary-main-loop-action-input-progress-gateway" ||
        checkpoint.capability ===
          "primary-main-loop-window-timer-callback-action-input-progress-gateway"
          ? {
              inputBaseline: checkpoint.inputBaseline,
              inputConsumed: checkpoint.inputConsumed,
              inputEvents: checkpoint.inputEvents.map(
                ({ memory: _memory, ...event }) => event,
              ),
              inputQuery:
                checkpoint.inputQuery === null
                  ? null
                  : {
                      ...normalizeSemanticCall(checkpoint.inputQuery),
                      runtimeThreadId: checkpoint.inputQuery.runtimeThreadId,
                      trapInstructionCount:
                        checkpoint.inputQuery.trapInstructionCount,
                    },
              ...(checkpoint.capability ===
              "primary-main-loop-window-timer-callback-action-input-progress-gateway"
                ? {
                    windowTimerDelivery: {
                      ...checkpoint.windowTimerDelivery,
                      dispatches:
                        checkpoint.windowTimerDelivery.dispatches.map(
                          normalizeWindowTimerDispatch,
                        ),
                    },
                  }
                : {}),
            }
          : {}),
        instructionCount: checkpoint.instructionCount,
        nextArguments: checkpoint.nextArguments,
        nextReturnAddress: checkpoint.nextReturnAddress,
        nextStackPointer: checkpoint.nextStackPointer,
        memoryContentSha256: checkpoint.memoryContentSha256,
      });
      assert.deepEqual(
        normalizeMultiCycleCheckpoint(browserCheckpoint),
        normalizeMultiCycleCheckpoint(oracleCheckpoint),
        "Chrome Worker recurring timer bounded window differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
        "kernel32-set-waitable-timer-post-critical-section-batch" ||
      oracleCheckpoint.capability ===
        "kernel32-set-waitable-timer-post-timer-wait-batch" ||
      oracleCheckpoint.capability ===
        "kernel32-set-waitable-timer-post-timer-rearm-wait-batch"
    ) {
      const normalizeTimerArmCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        waitableTimer: call.waitableTimer,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          threadId: browserCheckpoint.threadId,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeTimerArmCall),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          threadId: oracleCheckpoint.threadId,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeTimerArmCall),
        },
        "Chrome Worker post-critical-section timer arm differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
        "kernel32-wait-for-single-object-ex-post-critical-section" ||
      oracleCheckpoint.capability ===
        "kernel32-wait-for-single-object-ex-post-timer-arm-batch" ||
      oracleCheckpoint.capability ===
        "kernel32-wait-for-single-object-ex-post-timer-rearm-batch"
    ) {
      const normalizeWaitCall = (call) => ({
        ...normalizeSemanticCall(call),
        implementationId: call.implementationId,
        waitableTimer: call.waitableTimer,
      });
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          threadId: browserCheckpoint.threadId,
          nextArguments: browserCheckpoint.nextArguments,
          nextReturnAddress: browserCheckpoint.nextReturnAddress,
          nextStackPointer: browserCheckpoint.nextStackPointer,
          completions: browserCheckpoint.completions.map(normalizeWaitCall),
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          threadId: oracleCheckpoint.threadId,
          nextArguments: oracleCheckpoint.nextArguments,
          nextReturnAddress: oracleCheckpoint.nextReturnAddress,
          nextStackPointer: oracleCheckpoint.nextStackPointer,
          completions: oracleCheckpoint.completions.map(normalizeWaitCall),
        },
        "Chrome Worker post-arm wait lifecycle differs from the Node oracle",
      );
    }
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
    if (oracleCheckpoint.capability === "user32-async-key-state-input") {
      assert.deepEqual(
        {
          virtualKey: browserCheckpoint.virtualKey,
          result: browserCheckpoint.result,
          lastError: browserCheckpoint.lastError,
          inputEventSequence: browserCheckpoint.inputEventSequence,
          inputVirtualKey: browserCheckpoint.inputVirtualKey,
        },
        {
          virtualKey: oracleCheckpoint.virtualKey,
          result: oracleCheckpoint.result,
          lastError: oracleCheckpoint.lastError,
          inputEventSequence: oracleCheckpoint.inputEventSequence,
          inputVirtualKey: oracleCheckpoint.inputVirtualKey,
        },
        "Chrome Worker async-key checkpoint differs from the Node oracle",
      );
    }
    if (
      oracleCheckpoint.capability ===
      "user32-async-key-state-input-continuation"
    ) {
      assert.deepEqual(
        {
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          firstResult: browserCheckpoint.firstResult,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          virtualKey: browserCheckpoint.virtualKey,
          result: browserCheckpoint.result,
          lastError: browserCheckpoint.lastError,
          inputEventSequence: browserCheckpoint.inputEventSequence,
          inputVirtualKey: browserCheckpoint.inputVirtualKey,
        },
        {
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          firstResult: oracleCheckpoint.firstResult,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          virtualKey: oracleCheckpoint.virtualKey,
          result: oracleCheckpoint.result,
          lastError: oracleCheckpoint.lastError,
          inputEventSequence: oracleCheckpoint.inputEventSequence,
          inputVirtualKey: oracleCheckpoint.inputVirtualKey,
        },
        "Chrome Worker held-key continuation differs from the Node oracle",
      );
    }
    if (oracleCheckpoint.capability === "user32-async-key-state-initial-scan") {
      assert.deepEqual(
        {
          leadingCallCount: browserCheckpoint.leadingCallCount,
          firstCompletionSequence: browserCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            browserCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: browserCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            browserCheckpoint.lastCompletionInstructionCount,
          scanVirtualKeys: browserCheckpoint.scanVirtualKeys,
          result: browserCheckpoint.result,
          lastError: browserCheckpoint.lastError,
          guestWriteBytes: browserCheckpoint.guestWriteBytes,
        },
        {
          leadingCallCount: oracleCheckpoint.leadingCallCount,
          firstCompletionSequence: oracleCheckpoint.firstCompletionSequence,
          firstCompletionInstructionCount:
            oracleCheckpoint.firstCompletionInstructionCount,
          lastCompletionSequence: oracleCheckpoint.lastCompletionSequence,
          lastCompletionInstructionCount:
            oracleCheckpoint.lastCompletionInstructionCount,
          scanVirtualKeys: oracleCheckpoint.scanVirtualKeys,
          result: oracleCheckpoint.result,
          lastError: oracleCheckpoint.lastError,
          guestWriteBytes: oracleCheckpoint.guestWriteBytes,
        },
        "Chrome Worker initial async-key scan checkpoint differs from the Node oracle",
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
