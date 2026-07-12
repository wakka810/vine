import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "v86-browser-benchmark");
const reportPath = path.join(outputDirectory, "report.json");

const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

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
    const startedAt = performance.now();
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error(`${path.basename(command)} timed out\n${stdout}${stderr}`));
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
      const elapsedMilliseconds = performance.now() - startedAt;
      if (code === 0) {
        resolve({ stdout, stderr, elapsedMilliseconds });
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
    if (error.code !== "ESRCH") throw error;
  }
}

async function startServer() {
  const child = spawn(
    process.execPath,
    ["tools/run-browser-runtime-check.mjs", "--serve", "--port", "4190"],
    {
      cwd: root,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      terminateProcess(child);
      reject(new Error(`benchmark server did not start\n${stderr}`));
    }, 10_000);
    let stdout = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      const match = /listening at (http:\/\/127\.0\.0\.1:\d+\/)/u.exec(stdout);
      if (match !== null) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    });
    child.once("error", reject);
    child.once("close", (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`benchmark server exited (${signal ?? code})\n${stdout}${stderr}`));
    });
  });
  return { child, url };
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
    const timeout = setTimeout(() => {
      if (!settled) {
        settled = true;
        terminateProcess(child);
        reject(new Error(`Chrome did not expose DevTools\n${stdout}${stderr}`));
      }
    }, timeoutMs);
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
        resolve({ browserWebSocketUrl: match[1], child, exit });
      }
    });
    child.once("error", reject);
    child.once("close", (code, signal) => resolveExit({ code, signal }));
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
    socket.addEventListener("error", () => reject(new Error("DevTools connection failed")), {
      once: true,
    });
  });
  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!Number.isInteger(message.id)) return;
    const request = pending.get(message.id);
    if (request === undefined) return;
    pending.delete(message.id);
    clearTimeout(request.timeout);
    if (message.error === undefined) request.resolve(message.result);
    else request.reject(new Error(`DevTools ${request.method}: ${message.error.message}`));
  });
  return {
    close() {
      if (socket.readyState === WebSocket.OPEN) socket.close();
    },
    send(method, params = {}, timeoutMs = 10_000) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`DevTools ${method} timed out`));
        }, timeoutMs);
        pending.set(id, { method, resolve, reject, timeout });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
  };
}

async function createTarget(browserWebSocketUrl, url) {
  const endpoint = new URL(browserWebSocketUrl);
  const response = await fetch(`http://${endpoint.host}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  assert(response.ok, `target creation failed with ${response.status}`);
  return response.json();
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

async function waitFor(client, predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let snapshot = null;
  while (Date.now() < deadline) {
    try {
      snapshot = await evaluate(client, `window.__vineBenchmark?.snapshot?.() ?? null`);
      if (snapshot !== null && predicate(snapshot)) return snapshot;
    } catch {
      // Navigation can replace the execution context while polling.
    }
    await wait(100);
  }
  throw new Error(`${label} timed out: ${JSON.stringify(snapshot)}`);
}

async function clickRun(client) {
  const point = await evaluate(
    client,
    `(() => { const r = document.querySelector("#run-benchmark").getBoundingClientRect(); ` +
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

function findChrome() {
  const candidates = [
    process.env.VINE_CHROME,
    process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Google", "Chrome", "Application", "chrome.exe"),
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Google", "Chrome", "Application", "chrome.exe"),
    process.env["ProgramFiles(x86)"] &&
      path.join(process.env["ProgramFiles(x86)"], "Google", "Chrome", "Application", "chrome.exe"),
  ].filter(Boolean);
  const chrome = candidates.find((candidate) => fs.existsSync(candidate));
  assert(chrome, "Chrome stable was not found; set VINE_CHROME");
  return chrome;
}

async function chromeVersion(chrome) {
  if (process.platform !== "win32") return (await run(chrome, ["--version"])).stdout.trim();
  return (
    await run(
      "powershell.exe",
      ["-NoProfile", "-Command", "(Get-Item -LiteralPath $env:VINE_CHROME_PATH).VersionInfo.ProductVersion"],
      { env: { ...process.env, VINE_CHROME_PATH: chrome } },
    )
  ).stdout.trim();
}

async function optionalBgiBenchmark() {
  const bgi = process.env.VINE_BGI_EXE ?? "V:\\softs\\サクラノ詩\\BGI.exe";
  if (!fs.existsSync(bgi)) return { status: "skipped", reason: "primary BGI file unavailable" };
  const result = await run(
    process.execPath,
    ["tools/run-runtime-v86-bgi-first-trap.mjs", "--checkpoint", "bgi-seventeenth-win32-call"],
    { env: { ...process.env, VINE_BGI_EXE: bgi }, timeoutMs: 300_000 },
  );
  return {
    status: "measured",
    elapsedMilliseconds: result.elapsedMilliseconds,
    instructions: 238,
    dispatchedCalls: 17,
    reportSha256: "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
    reflectionSha256: "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
  };
}

async function stopProcess(process_) {
  if (process_ === null) return;
  terminateProcess(process_.child ?? process_);
  if (process_.exit !== undefined) {
    await Promise.race([process_.exit, wait(5_000)]);
  }
}

await fs.promises.mkdir(outputDirectory, { recursive: true });
const server = await startServer();
const temporaryRoot = path.resolve(os.tmpdir());
let profile = null;
let chrome = null;
let browserClient = null;
let pageClient = null;
try {
  const chromePath = findChrome();
  const version = await chromeVersion(chromePath);
  profile = await mkdtemp(path.join(temporaryRoot, "vine-v86-benchmark-"));
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
    "--window-size=1100,760",
    "about:blank",
  ]);
  browserClient = await connectDevTools(chrome.browserWebSocketUrl);
  const benchmarkTarget = await createTarget(chrome.browserWebSocketUrl, `${server.url}benchmark.html`);
  pageClient = await connectDevTools(benchmarkTarget.webSocketDebuggerUrl);
  await pageClient.send("Runtime.enable");
  await pageClient.send("Page.enable");
  await waitFor(pageClient, () => true, 20_000, "benchmark page readiness");
  await clickRun(pageClient);
  const foreground = await waitFor(
    pageClient,
    (snapshot) => snapshot.status === "foreground-complete" || snapshot.status === "failed",
    180_000,
    "foreground benchmark",
  );
  assert.notEqual(foreground.status, "failed", foreground.error ?? "foreground benchmark failed");

  const coverTarget = await createTarget(chrome.browserWebSocketUrl, "about:blank");
  await browserClient.send("Target.activateTarget", { targetId: coverTarget.id });
  await wait(500);
  const hiddenVisibility = await evaluate(pageClient, "document.visibilityState");
  const browserReport = await evaluate(pageClient, "window.__vineBenchmark.runBackgroundProbe()", {
    awaitPromise: true,
    timeoutMs: 30_000,
  });
  await browserClient.send("Target.activateTarget", { targetId: benchmarkTarget.id });
  assert.equal(browserReport.background.visibilityBefore, hiddenVisibility);
  const screenshot = await pageClient.send("Page.captureScreenshot", {
    format: "png",
    fromSurface: true,
  });
  await writeFile(path.join(outputDirectory, "benchmark.png"), Buffer.from(screenshot.data, "base64"));

  const [commit, status, buildManifest, config, bgi] = await Promise.all([
    run("git", ["rev-parse", "HEAD"]),
    run("git", ["status", "--short"]),
    readFile(path.join(root, ".work", "browser-runtime", "build-manifest.json"), "utf8"),
    readFile(path.join(root, "fixtures", "benchmarks", "v86-browser-v1.json"), "utf8"),
    optionalBgiBenchmark(),
  ]);
  const report = {
    ...browserReport,
    source: {
      commit: commit.stdout.trim(),
      dirty: status.stdout.trim().length > 0,
      buildManifest: JSON.parse(buildManifest),
      benchmarkConfiguration: JSON.parse(config),
    },
    host: {
      platform: `${os.platform()} ${os.release()} ${os.arch()}`,
      cpu: os.cpus()[0]?.model ?? "unknown",
      logicalCpus: os.cpus().length,
      totalMemoryBytes: os.totalmem(),
      node: process.version,
      chromeVersion: version,
    },
    privateBgiCheckpoint: bgi,
  };
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    `v86 browser benchmark ${report.decision}: ` +
      `integer=${Math.round(report.foreground.suite.integer.instructionsPerSecond).toLocaleString()} ips, ` +
      `x87=${Math.round(report.foreground.suite.x87.instructionsPerSecond).toLocaleString()} ips, ` +
      `SSE=${Math.round(report.foreground.suite.sse.instructionsPerSecond).toLocaleString()} ips, ` +
      `traps=${Math.round(report.foreground.suite.traps.roundTripsPerSecond).toLocaleString()}/s, ` +
      `production-api=${Math.round(report.foreground.suite.productionApi.roundTripsPerSecond).toLocaleString()}/s, ` +
      `steady-memory=${
        report.foreground.memorySteady.bytes === null
          ? "unavailable"
          : `${Number(report.foreground.memorySteady.bytes).toLocaleString()} bytes`
      }, ` +
      `background=${report.background.foregroundRatio.toFixed(3)}, ` +
      `failures=${report.failures.length}, Chrome=${version}`,
  );
  if (report.decision !== "go") process.exitCode = 1;
} finally {
  pageClient?.close();
  browserClient?.close();
  await stopProcess(chrome);
  await stopProcess(server.child);
  if (profile !== null) {
    const safePrefix = path.join(temporaryRoot, "vine-v86-benchmark-");
    assert(profile.startsWith(safePrefix), "refusing to remove unexpected Chrome profile");
    await rm(profile, { force: true, maxRetries: 5, recursive: true, retryDelay: 100 });
  }
}
