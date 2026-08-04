import { observePageLifecycle } from "./page-lifecycle.js";
import {
  assertProtocolVersion,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionMode,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";
import {
  asciiSourcePathBindings,
  registerSourceMount,
} from "../storage/mount-catalog.js";
import {
  type BrowserNativeStorageConfiguration,
  browserNativeStorageSchema,
  browserNativeStorageSchemaVersion,
} from "../storage/protocol.js";

type SourceScenario = SessionMode | "wrong-origin" | "browser-native";

interface SourceBrowserEvidence {
  scenario: SourceScenario;
  report: SessionReport;
  heartbeatSamples: number;
  maximumHeartbeatGapMilliseconds: number;
  statuses: Array<{ phase: string; detail: string }>;
}

declare global {
  interface Window {
    __vineSourceCheck: { result: SourceBrowserEvidence | null; error: string | null };
  }
}

const worker = new Worker(new URL("./runtime-worker.js", import.meta.url), { type: "module" });
const status = requiredElement<HTMLOutputElement>("#status");
const scenario = parseScenario(new URL(location.href).searchParams.get("scenario"));
const mode: SessionMode =
  scenario === "wrong-origin" || scenario === "browser-native"
    ? "success"
    : scenario;
const sessionId = 1;
const pageLifecycle = observePageLifecycle((lifecycle) => {
  worker.postMessage({
    type: "page-lifecycle",
    protocolVersion,
    sessionId,
    lifecycle,
  } satisfies MainToWorkerMessage);
});
const statuses: Array<{ phase: string; detail: string }> = [];
let heartbeatSamples = 0;
let maximumHeartbeatGapMilliseconds = 0;
let lastHeartbeat = performance.now();
let cancellationSent = false;
const heartbeat = window.setInterval(() => {
  const now = performance.now();
  maximumHeartbeatGapMilliseconds = Math.max(
    maximumHeartbeatGapMilliseconds,
    now - lastHeartbeat,
  );
  lastHeartbeat = now;
  heartbeatSamples += 1;
}, 5);

window.__vineSourceCheck = { result: null, error: null };

worker.addEventListener("message", (event: MessageEvent<WorkerToMainMessage>) => {
  const message = event.data;
  assertProtocolVersion(message.protocolVersion);
  if (message.sessionId !== sessionId) {
    return;
  }
  if (message.type === "session-status") {
    statuses.push({ phase: message.phase, detail: message.detail });
    status.textContent = message.detail;
    if (
      scenario === "cancel-pending" &&
      message.phase === "waiting-host" &&
      !cancellationSent
    ) {
      cancellationSent = true;
      worker.postMessage({
        type: "cancel-session",
        protocolVersion,
        sessionId,
      } satisfies MainToWorkerMessage);
    }
    return;
  }
  if (message.type === "host-request") {
    fail(new Error(`source fixture emitted unexpected external host request ${message.service}`));
    return;
  }
  finish(message.report);
});

worker.addEventListener("error", (event) => fail(new Error(event.message)));

void start();

async function start(): Promise<void> {
  try {
    const browserNativeStorage =
      scenario === "browser-native" ? await prepareBrowserNativeStorage() : undefined;
    worker.postMessage({
      type: "start-session",
      protocolVersion,
      sessionId,
      configuration: {
        mode,
        evidenceLevel: "full",
        workload: "controlled-source-read",
        executableUrl: "./source-read.exe",
        policyUrl: "./kernel32-source-file-read-v1.json",
        sourceManifestUrl: "/api/source/v1/manifest",
        sourceApiUrl:
          scenario === "wrong-origin"
            ? "https://example.invalid/api/source/v1/"
            : "/api/source/v1/",
        ...(browserNativeStorage === undefined ? {} : { browserNativeStorage }),
        instructionBudget: 8,
        watchdogInstructionLimit: 100_000,
        watchdogWallMilliseconds: scenario === "watchdog" ? 100 : 10_000,
        pageLifecycle: pageLifecycle.snapshot(),
      },
    } satisfies MainToWorkerMessage);
  } catch (error) {
    fail(error instanceof Error ? error : new Error(String(error)));
  }
}

async function prepareBrowserNativeStorage(): Promise<BrowserNativeStorageConfiguration> {
  const manifestResponse = await fetch("/api/source/v1/manifest", { cache: "no-store" });
  invariant(manifestResponse.status === 200, "source manifest bootstrap failed");
  const manifest = (await manifestResponse.json()) as {
    schema: "vine.source-snapshot-manifest";
    schemaVersion: 1;
    titleFingerprint: string;
    snapshotId: string;
    mount: { sourceId: number; rootPathA: number[] };
    files: Array<{
      objectId: number;
      relativePathA: number[];
      byteLength: number;
      sha256: string;
    }>;
  };
  invariant(manifest.files.length === 1, "source fixture manifest changed");
  const descriptor = manifest.files[0];
  invariant(descriptor !== undefined, "source fixture descriptor is missing");
  const objectUrl =
    `/api/source/v1/titles/${encodeURIComponent(manifest.titleFingerprint)}` +
    `/snapshots/${encodeURIComponent(manifest.snapshotId)}` +
    `/sources/${manifest.mount.sourceId}/objects/${descriptor.objectId}` +
    `?offset=0&length=${descriptor.byteLength}`;
  const objectResponse = await fetch(objectUrl, { cache: "no-store" });
  invariant(objectResponse.status === 206, "source fixture bootstrap read failed");
  const bytes = new Uint8Array(await objectResponse.arrayBuffer());
  invariant(bytes.byteLength === descriptor.byteLength, "source fixture bootstrap was short");
  const opfsRoot = await navigator.storage.getDirectory();
  const directory = await opfsRoot.getDirectoryHandle("runtime-source-check", {
    create: true,
  });
  const file = await directory.getFileHandle("source-read-v1.dat", { create: true });
  const writable = await file.createWritable();
  await writable.write(bytes);
  await writable.close();
  const pathBindings = asciiSourcePathBindings(manifest);
  pathBindings[0] = {
    objectId: descriptor.objectId,
    pathSegments: ["source-read-v1.dat"],
  };
  const sourceMount = await registerSourceMount({
    mountId: "runtime-source-check-v1",
    directoryHandle: directory,
    manifest,
    pathBindings,
  });
  return {
    schema: browserNativeStorageSchema,
    schemaVersion: browserNativeStorageSchemaVersion,
    sourceMount,
  };
}

function finish(report: SessionReport): void {
  window.clearInterval(heartbeat);
  const expectedOutcome =
    scenario === "success" || scenario === "browser-native"
      ? "completed"
      : scenario === "wrong-origin"
        ? "fault"
      : scenario === "watchdog"
        ? "watchdog"
        : "cancelled";
  if (report.outcome !== expectedOutcome) {
    fail(new Error(report.error ?? `unexpected source fixture outcome ${report.outcome}`));
    return;
  }
  if (
    scenario === "wrong-origin" &&
    report.error !== "source API URL must use the Runtime Worker origin"
  ) {
    fail(new Error(report.error ?? "cross-origin source API was not rejected"));
    return;
  }
  const evidence: SourceBrowserEvidence = {
    scenario,
    report,
    heartbeatSamples,
    maximumHeartbeatGapMilliseconds,
    statuses,
  };
  window.__vineSourceCheck.result = evidence;
  document.body.dataset.checkStatus = "passed";
  document.body.dataset.report = JSON.stringify(evidence);
  status.textContent = expectedOutcome;
  worker.terminate();
}

function fail(error: Error): void {
  window.clearInterval(heartbeat);
  window.__vineSourceCheck.error = error.message;
  document.body.dataset.checkStatus = "failed";
  document.body.dataset.error = error.message;
  status.textContent = error.message;
  worker.terminate();
}

function parseScenario(value: string | null): SourceScenario {
  return value === "cancel-pending" ||
    value === "watchdog" ||
    value === "wrong-origin" ||
    value === "browser-native"
    ? value
    : "success";
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  invariant(element !== null, `missing ${selector}`);
  return element;
}
