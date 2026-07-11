import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  browserPresentEventLogPrefix,
  validateFixedW2BrowserPresentEvent,
} from "../spikes/browser/w2-first-pixel/present-event.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const browserSpike = path.join(root, "spikes", "browser", "w2-first-pixel");
const serveOnly = process.argv.includes("--serve");

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : process.argv[index + 1] ?? null;
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
      reject(
        new Error(
          [
            `${path.basename(command)} timed out after ${timeoutMs} ms`,
            stdout,
            stderr,
          ]
            .filter(Boolean)
            .join("\n")
            .trim(),
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
      if (code !== 0) {
        reject(
          new Error(
            `${path.basename(command)} failed (${signal ?? code})\n${stdout}${stderr}`.trim(),
          ),
        );
        return;
      }
      resolve({ stdout, stderr });
    });
  });
}

const wait = (milliseconds) =>
  new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });

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

function launchChrome(command, arguments_, { env = process.env, timeoutMs = 15_000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, arguments_, {
      cwd: root,
      detached: process.platform !== "win32",
      env,
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
    const logs = () => `${stdout}${stderr}`.trim();
    const fail = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      terminateProcess(child);
      reject(error);
    };
    const timeout = setTimeout(() => {
      fail(
        new Error(
          [`${path.basename(command)} did not expose DevTools within ${timeoutMs} ms`, logs()]
            .filter(Boolean)
            .join("\n"),
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
      const match = /DevTools listening on (ws:\/\/\S+)/u.exec(stderr);
      if (match !== null && !settled) {
        settled = true;
        clearTimeout(timeout);
        resolve({
          browserWebSocketUrl: match[1],
          child,
          exit,
          logs,
        });
      }
    });
    child.once("error", fail);
    child.once("close", (code, signal) => {
      resolveExit({ code, signal });
      fail(
        new Error(
          [
            `${path.basename(command)} exited before exposing DevTools (${signal ?? code})`,
            logs(),
          ]
            .filter(Boolean)
            .join("\n"),
        ),
      );
    });
  });
}

async function connectDevTools(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error("DevTools WebSocket connection timed out"));
    }, 10_000);
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
        reject(new Error("DevTools WebSocket connection failed"));
      },
      { once: true },
    );
  });

  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    assert.equal(typeof event.data, "string", "DevTools message must be text");
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
    if (message.error !== undefined) {
      request.reject(new Error(`DevTools ${request.method} failed: ${message.error.message}`));
    } else {
      request.resolve(message.result);
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
    send(method, params = {}) {
      const id = nextId;
      nextId += 1;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`DevTools ${method} timed out`));
        }, 10_000);
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

async function readBrowserCheckpoint(client) {
  const expression = `(() => {
    const body = document.body;
    return body === null ? null : {
      readyState: document.readyState,
      status: body.dataset.checkStatus ?? null,
      error: body.dataset.error ?? null,
      presentCount: body.dataset.presentCount ?? null,
      schemaVersion: body.dataset.schemaVersion ?? null,
      presentSequence: body.dataset.presentSequence ?? null,
      canvasSize: body.dataset.canvasSize ?? null,
      pixelRgba: body.dataset.pixelRgba ?? null,
    };
  })()`;
  const deadline = Date.now() + 15_000;
  let lastState = null;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const evaluation = await client.send("Runtime.evaluate", {
        expression,
        returnByValue: true,
      });
      if (evaluation.exceptionDetails === undefined) {
        lastState = evaluation.result.value ?? null;
        if (
          lastState !== null &&
          (lastState.status === "passed" || lastState.status === "failed")
        ) {
          return lastState;
        }
      }
    } catch (error) {
      lastError = error;
    }
    await wait(100);
  }
  throw new Error(
    `browser checkpoint timed out: ${JSON.stringify(lastState)}`,
    lastError === null ? undefined : { cause: lastError },
  );
}

async function stopChrome(chromeProcess) {
  if (chromeProcess === null) {
    return;
  }
  terminateProcess(chromeProcess.child);
  const exited = await Promise.race([
    chromeProcess.exit.then(() => true),
    wait(5_000).then(() => false),
  ]);
  if (!exited) {
    terminateProcess(chromeProcess.child, "SIGKILL");
    await chromeProcess.exit;
  }
}

async function producePresentEvent() {
  const loader = path.join(root, "tools", "run-runtime-window-loader-slice.mjs");
  const { stdout } = await run(process.execPath, [loader], { timeoutMs: 60_000 });
  const eventLines = stdout
    .split(/\r?\n/u)
    .filter((line) => line.startsWith(browserPresentEventLogPrefix));
  assert.equal(eventLines.length, 1, "W2 harness must emit exactly one browser present event");
  const event = JSON.parse(eventLines[0].slice(browserPresentEventLogPrefix.length));
  return validateFixedW2BrowserPresentEvent(event);
}

function findChrome() {
  const candidates = [
    process.env.VINE_CHROME,
    process.env.ProgramFiles &&
      path.join(process.env.ProgramFiles, "Google", "Chrome", "Application", "chrome.exe"),
    process.env.LOCALAPPDATA &&
      path.join(process.env.LOCALAPPDATA, "Google", "Chrome", "Application", "chrome.exe"),
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

async function chromeVersion(chrome) {
  if (process.platform !== "win32") {
    const { stdout } = await run(chrome, ["--version"]);
    return stdout.trim();
  }
  const environment = { ...process.env, VINE_CHROME_PATH: chrome };
  const { stdout } = await run(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "(Get-Item -LiteralPath $env:VINE_CHROME_PATH).VersionInfo.ProductVersion",
    ],
    { env: environment },
  );
  return stdout.trim();
}

async function checkpointAssets(event) {
  return {
    event: `${JSON.stringify(event)}\n`,
    html: await readFile(path.join(browserSpike, "index.html")),
    sink: await readFile(path.join(browserSpike, "canvas-sink.mjs")),
    schema: await readFile(path.join(browserSpike, "present-event.mjs")),
  };
}

function createServer(assets) {
  return http.createServer((request, response) => {
    const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
    const routes = new Map([
      ["/", ["text/html; charset=utf-8", assets.html]],
      ["/index.html", ["text/html; charset=utf-8", assets.html]],
      ["/canvas-sink.mjs", ["text/javascript; charset=utf-8", assets.sink]],
      ["/present-event.mjs", ["text/javascript; charset=utf-8", assets.schema]],
      ["/present-event.json", ["application/json; charset=utf-8", assets.event]],
    ]);
    const route = routes.get(pathname);
    if (!route) {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("not found\n");
      return;
    }
    response.writeHead(200, {
      "Cache-Control": "no-store",
      "Content-Type": route[0],
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Resource-Policy": "same-origin",
    });
    response.end(route[1]);
  });
}

function listen(server, port) {
  return new Promise((resolve, reject) => {
    const onError = (error) => {
      server.off("listening", onListening);
      reject(error);
    };
    const onListening = () => {
      server.off("error", onError);
      resolve(server.address().port);
    };
    server.once("error", onError);
    server.once("listening", onListening);
    server.listen(port, "127.0.0.1");
  });
}

async function startServer(assets, preferredPort) {
  const attempts =
    preferredPort === 0
      ? [0]
      : Array.from({ length: 20 }, (_, index) => preferredPort + index);
  let lastError = null;
  for (const port of attempts) {
    const server = createServer(assets);
    try {
      const selectedPort = await listen(server, port);
      return { port: selectedPort, server };
    } catch (error) {
      lastError = error;
      if (error?.code !== "EADDRINUSE") {
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

async function waitForShutdown() {
  await new Promise((resolve) => {
    process.once("SIGINT", resolve);
    process.once("SIGTERM", resolve);
  });
}

const event = await producePresentEvent();
const assets = await checkpointAssets(event);
const requestedPort = serveOnly ? Number(optionValue("--port") ?? 4173) : 0;
assert(
  Number.isInteger(requestedPort) && requestedPort >= 0 && requestedPort <= 65_535,
  "--port must be an integer between 0 and 65535",
);
const { port, server } = await startServer(assets, requestedPort);
const url = `http://127.0.0.1:${port}/`;

if (serveOnly) {
  console.log(`W2 browser checkpoint server listening at ${url}`);
  await waitForShutdown();
  await closeServer(server);
} else {
  const temporaryRoot = path.resolve(os.tmpdir());
  let profile = null;
  let chromeProcess = null;
  let devTools = null;
  try {
    const chrome = findChrome();
    const version = await chromeVersion(chrome);
    profile = await mkdtemp(path.join(temporaryRoot, "vine-w2-browser-"));
    chromeProcess = await launchChrome(
      chrome,
      [
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
        "--no-default-browser-check",
        "--no-first-run",
        "--remote-debugging-port=0",
        `--user-data-dir=${profile}`,
        "--window-size=800,600",
        "about:blank",
      ],
    );
    const target = await createDebugTarget(chromeProcess.browserWebSocketUrl, url);
    devTools = await connectDevTools(target.webSocketDebuggerUrl);
    const checkpoint = await readBrowserCheckpoint(devTools);
    assert.equal(checkpoint.status, "passed", checkpoint.error ?? "browser check failed");
    assert.equal(checkpoint.presentCount, "1");
    assert.equal(checkpoint.schemaVersion, "1");
    assert.equal(checkpoint.presentSequence, "1");
    assert.equal(checkpoint.canvasSize, "2x2");
    assert.equal(checkpoint.pixelRgba, "255,0,0,255");
    console.log(
      `runtime window browser first pixel passed: schema=${event.schema}.v${event.schemaVersion}, ` +
        `presents=1, canvas=2x2, pixel(1,1)=rgba(255,0,0,255), Chrome=${version}`,
    );
  } finally {
    try {
      if (devTools !== null) {
        devTools.close();
      }
      await stopChrome(chromeProcess);
    } finally {
      try {
        await closeServer(server);
      } finally {
        if (profile !== null) {
          const safePrefix = path.join(temporaryRoot, "vine-w2-browser-");
          assert(
            profile.startsWith(safePrefix),
            "refusing to remove unexpected Chrome profile path",
          );
          await rm(profile, { force: true, maxRetries: 5, recursive: true, retryDelay: 100 });
        }
      }
    }
  }
}
