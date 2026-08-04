import { sha256 } from "@noble/hashes/sha2.js";

import { BrowserStorageClient, BrowserStorageError } from "./client.js";
import {
  asciiSourcePathBindings,
  getSourceMount,
  registerSourceMount,
} from "./mount-catalog.js";
import {
  type BrowserNativeStorageConfiguration,
  type BrowserSourceMount,
  browserNativeStorageSchema,
  browserNativeStorageSchemaVersion,
  browserWritableProfileSchema,
  browserWritableProfileSchemaVersion,
} from "./protocol.js";

interface StorageCheckReport {
  schema: "vine.browser-native-storage-check";
  schemaVersion: 1;
  phase: "prepare" | "reload";
  runId: string;
  capabilities: {
    secureContext: boolean;
    directoryPicker: boolean;
    opfs: boolean;
    persistedBefore: boolean;
    persistRequestGranted: boolean;
    persistedAfter: boolean;
    quota: number | null;
    usage: number | null;
  };
  source: {
    mountId: string;
    indexedDbHandleRoundTrip: boolean;
    workerHandleClone: boolean;
    byteLength: number;
    sha256: string;
    rangeHex: string;
  };
  profile: {
    sqliteVersion: string;
    initialGeneration: number;
    committedGeneration: number;
    committedSha256: string;
    directoryGeneration: number;
  };
  failureAtomicity: {
    concurrentWorkerRejected: boolean;
    uncommittedWorkerKillRolledBack: boolean;
    responseLossRecoveredIdempotently: boolean;
  };
}

declare global {
  interface Window {
    __vineBrowserNativeStorageCheck: {
      result: StorageCheckReport | null;
      error: string | null;
    };
  }
}

const query = new URL(location.href).searchParams;
const runId = query.get("runId") ?? "";
const phase = query.get("phase") === "reload" ? "reload" : "prepare";
const runIdPattern = /^[a-z0-9-]{8,80}$/u;
const sourceFilename = "source-fixture.dat";
const mountId = `storage-check-${runId}`;
const titleFingerprint = hex(sha256(new TextEncoder().encode(`vine-title:${runId}`)));
const sourceBytes = new TextEncoder().encode(
  `Vine browser-native immutable source fixture\nrun=${runId}\n`,
);
const sourceSha256 = hex(sha256(sourceBytes));
const firstCommitBytes = new TextEncoder().encode(
  `Vine committed generation one\nrun=${runId}\n`,
);
const secondCommitBytes = new TextEncoder().encode(
  `Vine committed generation two after response loss\nrun=${runId}\n`,
);
const uncommittedBytes = new TextEncoder().encode(
  `Vine uncommitted mutation must roll back\nrun=${runId}\n`,
);
const firstCommitSha256 = hex(sha256(firstCommitBytes));
const secondCommitSha256 = hex(sha256(secondCommitBytes));
const uncommittedSha256 = hex(sha256(uncommittedBytes));

window.__vineBrowserNativeStorageCheck = { result: null, error: null };
void runCheck().then(pass, fail);

async function runCheck(): Promise<StorageCheckReport> {
  invariant(runIdPattern.test(runId), "storage check run ID is invalid");
  invariant(isSecureContext, "browser-native storage check requires a secure context");
  invariant(
    typeof window.showDirectoryPicker === "function",
    "File System Access directory picker is unavailable",
  );
  invariant(
    typeof navigator.storage.getDirectory === "function",
    "Origin Private File System is unavailable",
  );
  const persistedBefore = await navigator.storage.persisted();
  const persistRequestGranted = await navigator.storage.persist();
  const persistedAfter = await navigator.storage.persisted();
  const estimate = await navigator.storage.estimate();
  const capabilities: StorageCheckReport["capabilities"] = {
    secureContext: isSecureContext,
    directoryPicker: typeof window.showDirectoryPicker === "function",
    opfs: typeof navigator.storage.getDirectory === "function",
    persistedBefore,
    persistRequestGranted,
    persistedAfter,
    quota: estimate.quota ?? null,
    usage: estimate.usage ?? null,
  };
  return phase === "prepare"
    ? preparePhase(capabilities)
    : reloadPhase(capabilities);
}

async function preparePhase(
  capabilities: StorageCheckReport["capabilities"],
): Promise<StorageCheckReport> {
  const sourceDirectory = await sourceDirectoryHandle(true);
  const sourceFile = await sourceDirectory.getFileHandle(sourceFilename, { create: true });
  const writable = await sourceFile.createWritable();
  await writable.write(sourceBytes);
  await writable.close();
  const manifest = sourceManifest();
  await registerSourceMount({
    mountId,
    directoryHandle: sourceDirectory,
    manifest,
    pathBindings: asciiSourcePathBindings(manifest),
  });
  const mount = await requireStoredMount();
  const indexedDbHandleRoundTrip = await mount.directoryHandle.isSameEntry(sourceDirectory);
  invariant(indexedDbHandleRoundTrip, "IndexedDB changed the source directory handle");

  let client = await initializeClient(mount);
  const workerHandleClone = await verifySourceRange(client);
  const initial = await client.profileSnapshot();
  invariant(
    initial.files[0]?.exists === false && initial.files[0].generation === 0,
    "new OPFS profile did not start absent",
  );
  const first = await client.profileCommit({
    requestKey: `${runId}:commit-one`,
    objectId: 1,
    baseGeneration: 0,
    contentSha256: firstCommitSha256,
    bytes: firstCommitBytes,
  });
  invariant(
    first.committedGeneration === 1 && !first.duplicate,
    "first OPFS profile commit did not publish generation one",
  );

  const contender = new BrowserStorageClient();
  let concurrentWorkerRejected = false;
  try {
    await contender.initialize(storageConfiguration(mount), sessionKey("contender"));
  } catch (error) {
    concurrentWorkerRejected =
      error instanceof BrowserStorageError &&
      error.code === "browser-storage-profile-locked";
  } finally {
    contender.dispose();
  }
  invariant(concurrentWorkerRejected, "concurrent OPFS SQLite worker was not rejected");

  await client.beginUncommittedMutationForTest({
    objectId: 1,
    baseGeneration: 1,
    contentSha256: uncommittedSha256,
    bytes: uncommittedBytes,
  });
  client.terminateForTest();
  await waitForProfileLockRelease();

  client = await initializeClient(mount);
  const afterRollback = await client.profileSnapshot();
  const rollbackFile = afterRollback.files[0];
  invariant(
    rollbackFile?.generation === 1 && rollbackFile.sha256 === firstCommitSha256,
    "worker kill exposed an uncommitted profile generation",
  );
  const rolledBackBytes = await client.profileRead(
    1,
    1,
    0,
    firstCommitBytes.byteLength,
  );
  invariant(
    bytesEqual(rolledBackBytes, firstCommitBytes),
    "worker kill changed the last committed profile bytes",
  );

  const publishedGeneration = await client.commitWithoutResponseForTest({
    requestKey: `${runId}:commit-two`,
    objectId: 1,
    baseGeneration: 1,
    contentSha256: secondCommitSha256,
    bytes: secondCommitBytes,
  });
  invariant(publishedGeneration === 2, "response-loss commit did not publish generation two");
  client.terminateForTest();
  await waitForProfileLockRelease();

  client = await initializeClient(mount);
  const afterResponseLoss = await client.profileSnapshot();
  const committedFile = afterResponseLoss.files[0];
  invariant(
    committedFile?.generation === 2 && committedFile.sha256 === secondCommitSha256,
    "published profile generation was lost with its response",
  );
  const duplicate = await client.profileCommit({
    requestKey: `${runId}:commit-two`,
    objectId: 1,
    baseGeneration: 1,
    contentSha256: secondCommitSha256,
    bytes: secondCommitBytes,
  });
  invariant(
    duplicate.duplicate && duplicate.committedGeneration === 2,
    "response-loss retry was not idempotent",
  );
  const directory = await client.createProfileDirectory({
    requestKey: `${runId}:directory-one`,
    directoryId: 1,
    baseGeneration: 0,
  });
  invariant(
    directory.committedGeneration === 1 && !directory.duplicate,
    "profile directory generation did not commit",
  );
  const initialization = lastInitialization;
  invariant(initialization !== null, "storage initialization evidence is missing");
  await client.close();
  await waitForProfileLockRelease();

  return {
    schema: "vine.browser-native-storage-check",
    schemaVersion: 1,
    phase: "prepare",
    runId,
    capabilities,
    source: {
      mountId,
      indexedDbHandleRoundTrip,
      workerHandleClone,
      byteLength: sourceBytes.byteLength,
      sha256: sourceSha256,
      rangeHex: expectedSourceRangeHex(),
    },
    profile: {
      sqliteVersion: initialization.sqliteVersion ?? "",
      initialGeneration: 0,
      committedGeneration: 2,
      committedSha256: secondCommitSha256,
      directoryGeneration: 1,
    },
    failureAtomicity: {
      concurrentWorkerRejected,
      uncommittedWorkerKillRolledBack: true,
      responseLossRecoveredIdempotently: true,
    },
  };
}

async function reloadPhase(
  capabilities: StorageCheckReport["capabilities"],
): Promise<StorageCheckReport> {
  const mount = await requireStoredMount();
  const expectedDirectory = await sourceDirectoryHandle(false);
  const indexedDbHandleRoundTrip = await mount.directoryHandle.isSameEntry(expectedDirectory);
  invariant(
    indexedDbHandleRoundTrip,
    "source directory handle did not survive browser restart",
  );
  const client = await initializeClient(mount);
  const workerHandleClone = await verifySourceRange(client);
  const snapshot = await client.profileSnapshot();
  const file = snapshot.files[0];
  const directory = snapshot.directories[0];
  invariant(
    file?.exists === true &&
      file.generation === 2 &&
      file.byteLength === secondCommitBytes.byteLength &&
      file.sha256 === secondCommitSha256,
    "profile generation did not survive browser restart",
  );
  invariant(
    directory?.exists === true && directory.generation === 1,
    "profile directory generation did not survive browser restart",
  );
  const bytes = await client.profileRead(1, 2, 0, secondCommitBytes.byteLength);
  invariant(bytesEqual(bytes, secondCommitBytes), "profile bytes changed across browser restart");
  const initialization = lastInitialization;
  invariant(initialization !== null, "storage initialization evidence is missing");
  await client.close();
  await waitForProfileLockRelease();

  return {
    schema: "vine.browser-native-storage-check",
    schemaVersion: 1,
    phase: "reload",
    runId,
    capabilities,
    source: {
      mountId,
      indexedDbHandleRoundTrip,
      workerHandleClone,
      byteLength: sourceBytes.byteLength,
      sha256: sourceSha256,
      rangeHex: expectedSourceRangeHex(),
    },
    profile: {
      sqliteVersion: initialization.sqliteVersion ?? "",
      initialGeneration: 0,
      committedGeneration: file.generation,
      committedSha256: file.sha256 ?? "",
      directoryGeneration: directory.generation,
    },
    failureAtomicity: {
      concurrentWorkerRejected: true,
      uncommittedWorkerKillRolledBack: true,
      responseLossRecoveredIdempotently: true,
    },
  };
}

let lastInitialization: Awaited<ReturnType<BrowserStorageClient["initialize"]>> | null =
  null;

async function initializeClient(mount: BrowserSourceMount): Promise<BrowserStorageClient> {
  const client = new BrowserStorageClient();
  try {
    lastInitialization = await client.initialize(
      storageConfiguration(mount),
      sessionKey("primary"),
    );
    invariant(
      lastInitialization.storageBackend ===
        "file-system-access+opfs-sahpool-sqlite" &&
        lastInitialization.profileId === `check-${runId}` &&
        lastInitialization.sessionEpoch > 0 &&
        (lastInitialization.sqliteVersion?.length ?? 0) > 0,
      "browser storage initialization identity is invalid",
    );
    return client;
  } catch (error) {
    client.dispose();
    throw error;
  }
}

async function verifySourceRange(client: BrowserStorageClient): Promise<boolean> {
  const offset = 5;
  const requestedBytes = 19;
  const actual = await client.sourceRead(1, 1, offset, requestedBytes);
  const expected = sourceBytes.slice(offset, offset + requestedBytes);
  invariant(bytesEqual(actual, expected), "storage worker source range differs from the mount");
  invariant(hex(actual) === expectedSourceRangeHex(), "source range evidence changed");
  return true;
}

function storageConfiguration(
  sourceMount: BrowserSourceMount,
): BrowserNativeStorageConfiguration {
  return {
    schema: browserNativeStorageSchema,
    schemaVersion: browserNativeStorageSchemaVersion,
    sourceMount,
    writableProfile: {
      schema: browserWritableProfileSchema,
      schemaVersion: browserWritableProfileSchemaVersion,
      titleFingerprint,
      profileId: `check-${runId}`,
      rootPathA: asciiBytes("C:\\Vine\0"),
      quotaBytes: 1024 * 1024,
      files: [{ objectId: 1, relativePathA: asciiBytes("save.dat") }],
      directories: [{ directoryId: 1, relativePathA: asciiBytes("UserData") }],
    },
    allowTestFaults: true,
  };
}

function sourceManifest(): {
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
} {
  return {
    schema: "vine.source-snapshot-manifest",
    schemaVersion: 1,
    titleFingerprint,
    snapshotId: `check-${runId}`,
    mount: { sourceId: 1, rootPathA: asciiBytes("C:\\Vine\0") },
    files: [
      {
        objectId: 1,
        relativePathA: asciiBytes(sourceFilename),
        byteLength: sourceBytes.byteLength,
        sha256: sourceSha256,
      },
    ],
  };
}

async function sourceDirectoryHandle(create: boolean): Promise<FileSystemDirectoryHandle> {
  const root = await navigator.storage.getDirectory();
  const checks = await root.getDirectoryHandle("vine-storage-check-sources", { create });
  return checks.getDirectoryHandle(runId, { create });
}

async function requireStoredMount(): Promise<BrowserSourceMount> {
  const mount = await getSourceMount(mountId);
  invariant(mount !== null, "IndexedDB source mount is missing");
  invariant(
    mount.titleFingerprint === titleFingerprint &&
      mount.snapshotId === `check-${runId}` &&
      mount.files[0]?.sha256 === sourceSha256,
    "IndexedDB source mount identity changed",
  );
  return mount;
}

async function waitForProfileLockRelease(): Promise<void> {
  const deadline = performance.now() + 10_000;
  while (performance.now() < deadline) {
    const locks = await navigator.locks.query();
    if (!locks.held?.some((lock) => lock.name === "vine.browser-storage.opfs-sahpool.v1")) {
      return;
    }
    await delay(20);
  }
  throw new Error("browser storage Web Lock was not released");
}

function sessionKey(role: string): string {
  return `${role}-${crypto.randomUUID()}`;
}

function expectedSourceRangeHex(): string {
  return hex(sourceBytes.slice(5, 24));
}

function asciiBytes(value: string): number[] {
  return [...value].map((character) => character.charCodeAt(0));
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  return left.byteLength === right.byteLength && left.every((byte, index) => byte === right[index]);
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function pass(report: StorageCheckReport): void {
  window.__vineBrowserNativeStorageCheck.result = report;
  document.body.dataset.checkStatus = "passed";
  document.body.dataset.report = JSON.stringify(report);
  requiredOutput().textContent = `${report.phase} passed`;
}

function fail(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  window.__vineBrowserNativeStorageCheck.error = message;
  document.body.dataset.checkStatus = "failed";
  document.body.dataset.error = message;
  requiredOutput().textContent = message;
}

function requiredOutput(): HTMLOutputElement {
  const output = document.querySelector<HTMLOutputElement>("#status");
  invariant(output !== null, "storage check output is missing");
  return output;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
