import assert from "node:assert/strict";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, ".work", "browser-runtime");
const serveOnly = process.argv.includes("--serve");
const stressMilliseconds = Number(optionValue("--stress-ms") ?? 0);

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
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!Number.isInteger(message.id)) {
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
      const id = nextId;
      nextId += 1;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`DevTools ${method} timed out`));
        }, timeoutMs);
        pending.set(id, { method, reject, resolve, timeout });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
  };
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
      const name = pathname === "/" ? "index.html" : pathname.slice(1);
      const file = path.resolve(assetsRoot, name);
      assert(file.startsWith(`${path.resolve(assetsRoot)}${path.sep}`), "invalid asset path");
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
assert(Number.isFinite(stressMilliseconds) && stressMilliseconds >= 0, "--stress-ms is invalid");
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
      schemaVersion: 2,
      runtimeProtocol: "vine.browser-runtime-worker.v2",
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
