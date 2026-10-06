// The directory handle is a browser capability, not a physical path. Remember
// it without requesting write access so the same selection keeps its own save tree.
const databaseName = "vine-source-directories";
const storeName = "directories";

interface Entry {
  id: string;
  handle: FileSystemDirectoryHandle;
}

type ReadableDirectory = FileSystemDirectoryHandle & {
  queryPermission?: (options: { mode: "read" }) => Promise<PermissionState>;
  requestPermission?: (options: { mode: "read" }) => Promise<PermissionState>;
};

export async function chooseSourceDirectory(): Promise<FileSystemDirectoryHandle> {
  const browser = window as Window & {
    showDirectoryPicker?: (options: { id: string; mode: "read" }) => Promise<FileSystemDirectoryHandle>;
  };
  if (!browser.showDirectoryPicker) throw new Error("Chrome directory picker is unavailable");
  return browser.showDirectoryPicker({ id: "vine-source-directory", mode: "read" });
}

export async function ensureSourceReadPermission(handle: FileSystemDirectoryHandle): Promise<void> {
  const directory = handle as ReadableDirectory;
  if (!directory.queryPermission || await directory.queryPermission({ mode: "read" }) === "granted") {
    return;
  }
  if (!directory.requestPermission ||
      await directory.requestPermission({ mode: "read" }) !== "granted") {
    throw new Error("Read permission was not granted");
  }
}

// Comparing two directory capabilities is asynchronous. Serialize selections
// in this Worker so rapid clicks cannot assign two save trees to one directory.
let catalogOperation: Promise<void> = Promise.resolve();

export function rememberSourceDirectory(handle: FileSystemDirectoryHandle): Promise<string> {
  const result = catalogOperation.then(() => identifyDirectory(handle));
  catalogOperation = result.then(() => {}, () => {});
  return result;
}

// A selected executable's path is part of its save identity: two copies with
// identical PE bytes under one selected directory must not share writable data.
export async function gameStorageFingerprint(
  handle: FileSystemDirectoryHandle, segments: string[], name: string, executableSha256: string,
): Promise<string> {
  if (!Array.isArray(segments) || typeof name !== "string" ||
      !/^[0-9a-f]{64}$/u.test(executableSha256)) {
    throw new Error("Invalid executable storage identity");
  }
  const directoryId = await rememberSourceDirectory(handle);
  const identity = JSON.stringify([directoryId, segments, name, executableSha256]);
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(identity));
  return Array.from(new Uint8Array(digest),
    (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function identifyDirectory(handle: FileSystemDirectoryHandle): Promise<string> {
  if (handle.kind !== "directory") throw new Error("Source is not a directory");
  const database = await openDatabase();
  try {
    const entries = await new Promise<Entry[]>((resolve, reject) => {
      const request = database.transaction(storeName, "readonly")
        .objectStore(storeName).getAll();
      request.onsuccess = () => resolve(request.result as Entry[]);
      request.onerror = () => reject(request.error);
    });
    for (const entry of entries) {
      if (!/^[0-9a-f-]{36}$/u.test(entry.id) || entry.handle?.kind !== "directory") {
        throw new Error("Source directory catalog is corrupt");
      }
      if (await entry.handle.isSameEntry(handle)) return entry.id;
    }
    const id = crypto.randomUUID();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction(storeName, "readwrite");
      transaction.objectStore(storeName).add({ id, handle } satisfies Entry);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
    return id;
  } finally {
    database.close();
  }
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(databaseName);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(storeName, { keyPath: "id" });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Source directory catalog is blocked"));
  });
}
