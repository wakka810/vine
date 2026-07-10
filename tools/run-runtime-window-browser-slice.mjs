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
      reject(new Error(`${path.basename(command)} timed out after ${timeoutMs} ms`));
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
  try {
    const chrome = findChrome();
    const version = await chromeVersion(chrome);
    profile = await mkdtemp(path.join(temporaryRoot, "vine-w2-browser-"));
    const { stdout } = await run(
      chrome,
      [
        "--headless=new",
        "--disable-background-networking",
        "--disable-breakpad",
        "--disable-component-update",
        "--disable-default-apps",
        "--disable-extensions",
        "--disable-sync",
        "--metrics-recording-only",
        "--no-default-browser-check",
        "--no-first-run",
        `--user-data-dir=${profile}`,
        "--virtual-time-budget=5000",
        "--window-size=800,600",
        "--dump-dom",
        url,
      ],
      { timeoutMs: 45_000 },
    );
    assert.match(stdout, /data-check-status="passed"/u);
    assert.match(stdout, /data-present-count="1"/u);
    assert.match(stdout, /data-schema-version="1"/u);
    assert.match(stdout, /data-present-sequence="1"/u);
    assert.match(stdout, /data-canvas-size="2x2"/u);
    assert.match(stdout, /data-pixel-rgba="255,0,0,255"/u);
    console.log(
      `runtime window browser first pixel passed: schema=${event.schema}.v${event.schemaVersion}, ` +
        `presents=1, canvas=2x2, pixel(1,1)=rgba(255,0,0,255), Chrome=${version}`,
    );
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
