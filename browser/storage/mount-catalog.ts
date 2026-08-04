import { sha256 } from "@noble/hashes/sha2.js";

import {
  type BrowserSourceFile,
  type BrowserSourceMount,
  browserSourceMountSchema,
  browserSourceMountSchemaVersion,
} from "./protocol.js";

const catalogDatabaseName = "vine-browser-capabilities-v1";
const catalogDatabaseVersion = 1;
const sourceMountStore = "source-mounts";
const sha256Pattern = /^[0-9a-f]{64}$/u;
const mountIdPattern = /^[a-zA-Z0-9._-]{1,128}$/u;
const fingerprintPattern = /^[a-zA-Z0-9:._-]{1,128}$/u;

interface SourceManifestInput {
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
}

export interface SourcePathBinding {
  objectId: number;
  pathSegments: string[];
}

export interface RegisterSourceMountOptions {
  mountId: string;
  directoryHandle: FileSystemDirectoryHandle;
  manifest: SourceManifestInput;
  pathBindings: SourcePathBinding[];
  onProgress?: (completed: number, total: number) => void;
}

type PermissionDirectoryHandle = FileSystemDirectoryHandle & {
  queryPermission?: (descriptor: { mode: "read" }) => Promise<PermissionState>;
  requestPermission?: (descriptor: { mode: "read" }) => Promise<PermissionState>;
};

declare global {
  interface Window {
    showDirectoryPicker?: (options?: {
      id?: string;
      mode?: "read" | "readwrite";
    }) => Promise<FileSystemDirectoryHandle>;
  }
}

export async function chooseSourceDirectory(
  pickerId = "vine-source-directory",
): Promise<FileSystemDirectoryHandle> {
  invariant(
    typeof window.showDirectoryPicker === "function",
    "File System Access directory picker is unavailable",
  );
  return window.showDirectoryPicker({ id: pickerId, mode: "read" });
}

export async function ensureSourceMountReadPermission(
  handle: FileSystemDirectoryHandle,
  request: boolean,
): Promise<PermissionState> {
  const permissionHandle = handle as PermissionDirectoryHandle;
  if (permissionHandle.queryPermission === undefined) {
    return "granted";
  }
  const current = await permissionHandle.queryPermission({ mode: "read" });
  if (current === "granted" || !request) {
    return current;
  }
  invariant(
    permissionHandle.requestPermission !== undefined,
    "source directory cannot request read permission",
  );
  return permissionHandle.requestPermission({ mode: "read" });
}

export async function registerSourceMount(
  options: RegisterSourceMountOptions,
): Promise<BrowserSourceMount> {
  validateRegistration(options);
  const permission = await ensureSourceMountReadPermission(
    options.directoryHandle,
    false,
  );
  invariant(permission === "granted", "source directory read permission is not granted");

  const bindings = new Map(
    options.pathBindings.map((binding) => [binding.objectId, binding.pathSegments]),
  );
  const files: BrowserSourceFile[] = [];
  for (const [index, descriptor] of options.manifest.files.entries()) {
    const pathSegments = bindings.get(descriptor.objectId);
    invariant(pathSegments !== undefined, "source path binding is missing");
    const handle = await resolveSourceFile(options.directoryHandle, pathSegments);
    const before = await handle.getFile();
    invariant(
      before.size === descriptor.byteLength,
      `source object ${descriptor.objectId} byte length changed`,
    );
    const observedSha256 = await hashFile(before);
    const after = await handle.getFile();
    invariant(
      before.size === after.size && before.lastModified === after.lastModified,
      `source object ${descriptor.objectId} changed while it was verified`,
    );
    invariant(
      observedSha256 === descriptor.sha256,
      `source object ${descriptor.objectId} SHA-256 mismatch`,
    );
    files.push({
      objectId: descriptor.objectId,
      relativePathA: [...descriptor.relativePathA],
      pathSegments: [...pathSegments],
      byteLength: descriptor.byteLength,
      sha256: descriptor.sha256,
      lastModified: before.lastModified,
    });
    options.onProgress?.(index + 1, options.manifest.files.length);
  }

  const mount: BrowserSourceMount = {
    schema: browserSourceMountSchema,
    schemaVersion: browserSourceMountSchemaVersion,
    mountId: options.mountId,
    titleFingerprint: options.manifest.titleFingerprint,
    snapshotId: options.manifest.snapshotId,
    sourceId: options.manifest.mount.sourceId,
    rootPathA: [...options.manifest.mount.rootPathA],
    directoryHandle: options.directoryHandle,
    verifiedAt: new Date().toISOString(),
    files,
  };
  const database = await openCatalog();
  try {
    const transaction = database.transaction(sourceMountStore, "readwrite");
    transaction.objectStore(sourceMountStore).put(mount);
    await transactionComplete(transaction);
  } finally {
    database.close();
  }
  return mount;
}

export async function getSourceMount(
  mountId: string,
): Promise<BrowserSourceMount | null> {
  invariant(mountIdPattern.test(mountId), "source mount ID is invalid");
  const database = await openCatalog();
  try {
    const transaction = database.transaction(sourceMountStore, "readonly");
    const value = await requestResult(
      transaction.objectStore(sourceMountStore).get(mountId),
    );
    await transactionComplete(transaction);
    if (value === undefined) {
      return null;
    }
    validateStoredMount(value);
    return value;
  } finally {
    database.close();
  }
}

export async function deleteSourceMount(mountId: string): Promise<void> {
  invariant(mountIdPattern.test(mountId), "source mount ID is invalid");
  const database = await openCatalog();
  try {
    const transaction = database.transaction(sourceMountStore, "readwrite");
    transaction.objectStore(sourceMountStore).delete(mountId);
    await transactionComplete(transaction);
  } finally {
    database.close();
  }
}

export function asciiSourcePathBindings(
  manifest: SourceManifestInput,
): SourcePathBinding[] {
  return manifest.files.map((file) => {
    invariant(
      file.relativePathA.every((byte) => byte > 0 && byte < 0x80),
      `source object ${file.objectId} requires an explicit Unicode path binding`,
    );
    const path = String.fromCharCode(...file.relativePathA).replaceAll("\\", "/");
    const pathSegments = path.split("/");
    validatePathSegments(pathSegments);
    return { objectId: file.objectId, pathSegments };
  });
}

async function resolveSourceFile(
  root: FileSystemDirectoryHandle,
  pathSegments: string[],
): Promise<FileSystemFileHandle> {
  validatePathSegments(pathSegments);
  let directory = root;
  for (const segment of pathSegments.slice(0, -1)) {
    directory = await directory.getDirectoryHandle(segment, { create: false });
  }
  const filename = pathSegments.at(-1);
  invariant(filename !== undefined, "source file path is empty");
  return directory.getFileHandle(filename, { create: false });
}

async function hashFile(file: File): Promise<string> {
  const hasher = sha256.create();
  const reader = file.stream().getReader();
  let bytesRead = 0;
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) {
        break;
      }
      hasher.update(chunk.value);
      bytesRead += chunk.value.byteLength;
    }
  } finally {
    reader.releaseLock();
  }
  invariant(bytesRead === file.size, "source file ended while it was verified");
  return hex(hasher.digest());
}

function validateRegistration(options: RegisterSourceMountOptions): void {
  invariant(mountIdPattern.test(options.mountId), "source mount ID is invalid");
  const manifest = options.manifest;
  invariant(
    manifest.schema === "vine.source-snapshot-manifest" && manifest.schemaVersion === 1,
    "source manifest schema is invalid",
  );
  invariant(
    fingerprintPattern.test(manifest.titleFingerprint) &&
      mountIdPattern.test(manifest.snapshotId) &&
      Number.isInteger(manifest.mount.sourceId) &&
      manifest.mount.sourceId > 0 &&
      validByteArray(manifest.mount.rootPathA, true),
    "source manifest identity is invalid",
  );
  invariant(
    manifest.files.length > 0 && manifest.files.length <= 4096,
    "source manifest file count is invalid",
  );
  invariant(
    options.pathBindings.length === manifest.files.length,
    "source path binding count differs from the manifest",
  );
  const objectIds = new Set<number>();
  const bindingIds = new Set<number>();
  for (const binding of options.pathBindings) {
    invariant(
      Number.isInteger(binding.objectId) &&
        binding.objectId > 0 &&
        !bindingIds.has(binding.objectId),
      "source path binding identity is invalid or duplicated",
    );
    validatePathSegments(binding.pathSegments);
    bindingIds.add(binding.objectId);
  }
  for (const file of manifest.files) {
    invariant(
      Number.isInteger(file.objectId) &&
        file.objectId > 0 &&
        !objectIds.has(file.objectId) &&
        bindingIds.has(file.objectId) &&
        validByteArray(file.relativePathA, false) &&
        Number.isSafeInteger(file.byteLength) &&
        file.byteLength >= 0 &&
        sha256Pattern.test(file.sha256),
      "source file descriptor is invalid or duplicated",
    );
    objectIds.add(file.objectId);
  }
}

function validateStoredMount(value: unknown): asserts value is BrowserSourceMount {
  invariant(value !== null && typeof value === "object", "stored source mount is invalid");
  const mount = value as Partial<BrowserSourceMount>;
  invariant(
    mount.schema === browserSourceMountSchema &&
      mount.schemaVersion === browserSourceMountSchemaVersion &&
      typeof mount.mountId === "string" &&
      mountIdPattern.test(mount.mountId) &&
      typeof mount.titleFingerprint === "string" &&
      fingerprintPattern.test(mount.titleFingerprint) &&
      typeof mount.snapshotId === "string" &&
      mountIdPattern.test(mount.snapshotId) &&
      Number.isInteger(mount.sourceId) &&
      (mount.sourceId ?? 0) > 0 &&
      validByteArray(mount.rootPathA, true) &&
      mount.directoryHandle?.kind === "directory" &&
      typeof mount.verifiedAt === "string" &&
      !Number.isNaN(Date.parse(mount.verifiedAt)) &&
      Array.isArray(mount.files) &&
      mount.files.length > 0,
    "stored source mount metadata is invalid",
  );
}

function validatePathSegments(segments: string[]): void {
  invariant(
    segments.length > 0 &&
      segments.length <= 256 &&
      segments.every(
        (segment) =>
          segment.length > 0 &&
          segment.length <= 255 &&
          segment !== "." &&
          segment !== ".." &&
          !segment.includes("/") &&
          !segment.includes("\\") &&
          !segment.includes("\0"),
      ),
    "source path binding is invalid",
  );
}

function validByteArray(value: unknown, trailingNul: boolean): value is number[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.length <= 32_768 &&
    value.every(
      (byte) => Number.isInteger(byte) && (byte as number) >= 0 && (byte as number) <= 0xff,
    ) &&
    (trailingNul
      ? value.at(-1) === 0 && !value.slice(0, -1).includes(0)
      : !value.includes(0))
  );
}

function openCatalog(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(catalogDatabaseName, catalogDatabaseVersion);
    request.addEventListener("upgradeneeded", () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(sourceMountStore)) {
        database.createObjectStore(sourceMountStore, { keyPath: "mountId" });
      }
    });
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
    request.addEventListener("blocked", () => {
      reject(new Error("source mount catalog upgrade is blocked"));
    });
  });
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error), { once: true });
  });
}

function transactionComplete(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", () => resolve(), { once: true });
    transaction.addEventListener(
      "abort",
      () => reject(transaction.error ?? new Error("IndexedDB transaction aborted")),
      { once: true },
    );
    transaction.addEventListener("error", () => reject(transaction.error), { once: true });
  });
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
