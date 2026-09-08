import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadChromeForTestingArtifact } from "./chrome-for-testing.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, ".work", "browser-native-storage");
const requiredAssets = [
  "storage-check.html",
  "storage-check.js",
  "browser-storage-worker.js",
  "sqlite3.wasm",
];
for (const asset of requiredAssets) {
  assert(fs.existsSync(path.join(assetsRoot, asset)), `storage asset ${asset} is missing`);
}

const runId = `run-${crypto.randomBytes(8).toString("hex")}`;
const server = createServer();
const port = await listen(server);
const origin = `http://127.0.0.1:${port}`;
const temporaryRoot = path.resolve(os.tmpdir());
const profile = await mkdtemp(path.join(temporaryRoot, "vine-browser-storage-"));
const chromeArtifact = await loadChromeForTestingArtifact(root);
let chrome = null;

try {
  chrome = await startChrome(chromeArtifact.executablePath, profile);
  const prepare = await runPhase(
    chrome.browserWebSocketUrl,
    `${origin}/storage-check.html?phase=prepare&runId=${encodeURIComponent(runId)}`,
  );
  assertReport(prepare, "prepare");
  await stopChrome(chrome);
  chrome = null;

  chrome = await startChrome(chromeArtifact.executablePath, profile);
  const reload = await runPhase(
    chrome.browserWebSocketUrl,
    `${origin}/storage-check.html?phase=reload&runId=${encodeURIComponent(runId)}`,
  );
  assertReport(reload, "reload");
  assert.deepEqual(reload.source, prepare.source, "source mount changed across Chrome restart");
  assert.deepEqual(
    reload.profile,
    prepare.profile,
    "writable profile changed across Chrome restart",
  );

  const report = {
    schema: "vine.browser-native-storage-browser-report",
    schemaVersion: 1,
    runId,
    chrome: {
      artifactId: chromeArtifact.manifest.id,
      version: chromeArtifact.manifest.version,
      executable: chromeArtifact.executableIdentity,
    },
    prepare,
    reload,
  };
  const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
  await writeFile(path.join(assetsRoot, "browser-report.json"), reportBytes);
  console.log(
    `browser-native storage passed: Chrome=${chromeArtifact.manifest.version}, ` +
      `generation=${reload.profile.committedGeneration}, ` +
      `restartSource=${reload.source.indexedDbHandleRoundTrip}, ` +
      `reportSha256=${crypto.createHash("sha256").update(reportBytes).digest("hex")}`,
  );
} finally {
  try {
    await stopChrome(chrome);
  } finally {
    try {
      await closeServer(server);
    } finally {
      const safePrefix = path.join(temporaryRoot, "vine-browser-storage-");
      assert(profile.startsWith(safePrefix), "refusing to remove unexpected Chrome profile");
      await rm(profile, {
        force: true,
        maxRetries: 5,
        recursive: true,
        retryDelay: 100,
      });
    }
  }
}

function assertReport(report, phase) {
  assert.equal(report.schema, "vine.browser-native-storage-check");
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.phase, phase);
  assert.equal(report.runId, runId);
  assert.equal(report.capabilities.secureContext, true);
  assert.equal(report.capabilities.directoryPicker, true);
  assert.equal(report.capabilities.opfs, true);
  assert.equal(report.source.indexedDbHandleRoundTrip, true);
  assert.equal(report.source.workerHandleClone, true);
  assert.match(report.source.sha256, /^[0-9a-f]{64}$/u);
  assert(report.source.byteLength > 0);
  assert.equal(report.profile.initialGeneration, 0);
  assert.equal(report.profile.committedGeneration, 2);
  assert.match(report.profile.committedSha256, /^[0-9a-f]{64}$/u);
  assert.equal(report.profile.directoryGeneration, 1);
  assert.match(report.profile.sqliteVersion, /^3\.\d+\.\d+/u);
  assert.equal(report.failureAtomicity.concurrentWorkerRejected, true);
  assert.equal(report.failureAtomicity.uncommittedWorkerKillRolledBack, true);
  assert.equal(report.failureAtomicity.responseLossRecoveredIdempotently, true);
}

async function runPhase(browserWebSocketUrl, url) {
  const target = await createDebugTarget(browserWebSocketUrl, url);
  const client = await connectDevTools(target.webSocketDebuggerUrl);
  try {
    await waitForCheckPage(client);
    return await evaluate(
      client,
      `new Promise((resolve, reject) => {
        const deadline = performance.now() + 120000;
        const poll = () => {
          const check = window.__vineBrowserNativeStorageCheck;
          if (check?.result !== null && check?.result !== undefined) {
            resolve(check.result);
          } else if (check?.error) {
            reject(new Error(check.error));
          } else if (performance.now() >= deadline) {
            reject(new Error("browser-native storage page timed out"));
          } else {
            setTimeout(poll, 25);
          }
        };
        poll();
      })`,
      { awaitPromise: true, timeoutMs: 130_000 },
    );
  } finally {
    client.close();
  }
}

async function waitForCheckPage(client) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      if (
        await evaluate(
          client,
          "Boolean(window.__vineBrowserNativeStorageCheck && document.querySelector('#status'))",
        )
      ) {
        return;
      }
    } catch {
      // Navigation may still be replacing the target's default context.
    }
    await wait(50);
  }
  const page = await evaluate(client, `({
    url: location.href, readyState: document.readyState,
    title: document.title, status: document.querySelector('#status')?.textContent,
    text: document.body?.innerText.slice(0, 500)
  })`);
  throw new Error(`browser-native storage page did not become ready: ${JSON.stringify(page)}`);
}

function createServer() {
  return http.createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
      const name = pathname === "/" ? "storage-check.html" : pathname.slice(1);
      const file = path.resolve(assetsRoot, name);
      assert(
        file.startsWith(`${path.resolve(assetsRoot)}${path.sep}`),
        "asset request escaped the browser-native storage output",
      );
      const bytes = await readFile(file);
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": mimeType(file),
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Resource-Policy": "same-origin",
        "X-Content-Type-Options": "nosniff",
      });
      response.end(bytes);
    } catch {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("not found\n");
    }
  });
}

function mimeType(file) {
  switch (path.extname(file)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    case ".json":
      return "application/json; charset=utf-8";
    default:
      return "application/octet-stream";
  }
}

function listen(httpServer) {
  return new Promise((resolve, reject) => {
    httpServer.once("error", reject);
    httpServer.listen(0, "127.0.0.1", () => resolve(httpServer.address().port));
  });
}

function closeServer(httpServer) {
  return new Promise((resolve, reject) => {
    httpServer.close((error) => (error ? reject(error) : resolve()));
  });
}

function startChrome(executablePath, userDataDirectory) {
  return launchChrome(executablePath, [
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
    "--metrics-recording-only",
    "--mute-audio",
    "--no-default-browser-check",
    "--no-first-run",
    "--remote-debugging-port=0",
    `--user-data-dir=${userDataDirectory}`,
    "about:blank",
  ]);
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
      fail(new Error(`Chrome exited before DevTools (${signal ?? code})\n${stdout}${stderr}`));
    });
  });
}

async function stopChrome(runningChrome) {
  if (runningChrome === null) {
    return;
  }
  terminateProcess(runningChrome.child);
  if (
    !(await Promise.race([
      runningChrome.exit.then(() => true),
      wait(5_000).then(() => false),
    ]))
  ) {
    terminateProcess(runningChrome.child, "SIGKILL");
    await runningChrome.exit;
  }
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
  const response = await fetch(
    `http://${browserEndpoint.host}/json/new?${encodeURIComponent(url)}`,
    { method: "PUT" },
  );
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
    const description = result.exceptionDetails.exception?.description;
    throw new Error(
      typeof description === "string"
        ? description
        : `browser evaluation failed: ${JSON.stringify(result.exceptionDetails)}`,
    );
  }
  return result.result.value;
}

function wait(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
