// A development server grants one read-only source capability. This descriptor is
// structured-cloneable so workers use the same source without copying its assets.
export interface DevSourceDirectory {
  kind: "directory";
  transport: "dev-server";
  name: string;
  id: string;
  token: string;
}
export type SourceDirectory = FileSystemDirectoryHandle | DevSourceDirectory;
export interface SourceFile {
  readonly size: number;
  readonly lastModified: number;
  readonly sha256?: string;
  readonly sourceVersion?: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start: number, end: number): { arrayBuffer(): Promise<ArrayBuffer> };
}
const endpoint = "/__vine_dev_source/";
const chunkSize = 2 * 1024 * 1024;

export function isDevSource(root: SourceDirectory): root is DevSourceDirectory {
  return "transport" in root && root.transport === "dev-server";
}

function validateRoot(value: unknown): DevSourceDirectory {
  const root = value as Partial<DevSourceDirectory> | null;
  if (!root || root.kind !== "directory" || root.transport !== "dev-server" ||
      typeof root.name !== "string" || !root.name ||
      typeof root.id !== "string" || !/^[0-9a-f]{64}$/u.test(root.id) ||
      typeof root.token !== "string" || !/^[0-9a-f]{64}$/u.test(root.token)) {
    throw new Error("Invalid development source capability");
  }
  return root as DevSourceDirectory;
}

export async function discoverDevSource(): Promise<DevSourceDirectory | null> {
  const response = await fetch(`${endpoint}config`, { cache: "no-store", credentials: "same-origin" });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Development source is unavailable (${response.status})`);
  return validateRoot(await response.json());
}

async function request(root: DevSourceDirectory, operation: string,
  segments: readonly string[], parameters: Record<string, string> = {}): Promise<Response> {
  validateRoot(root);
  if (!Array.isArray(segments) || segments.length > 256 || segments.some(segment =>
    typeof segment !== "string" || !segment || segment.length > 255 ||
    segment === "." || segment === ".." || /[\\/\u0000]/u.test(segment))) {
    throw new Error("Invalid development source path");
  }
  const query = new URLSearchParams({ path: JSON.stringify(segments), ...parameters });
  const response = await fetch(`${endpoint}${operation}?${query}`, {
    headers: { "x-vine-source-token": root.token }, cache: "no-store", credentials: "same-origin",
  });
  if (response.status === 404) throw new DOMException("Development source entry was not found", "NotFoundError");
  if (!response.ok) throw new Error(`Development source ${operation} failed (${response.status})`);
  return response;
}

export async function listDevSource(root: DevSourceDirectory, segments: readonly string[]):
Promise<Array<{ name: string; kind: "file" | "directory" }>> {
  const result = await (await request(root, "list", segments)).json() as {
    entries?: Array<{ name: string; kind: "file" | "directory" }>;
  };
  if (!Array.isArray(result.entries) || result.entries.length > 4096 || result.entries.some(entry =>
    !entry || typeof entry.name !== "string" || !entry.name || entry.name.length > 255 ||
    entry.name === "." || entry.name === ".." || /[\\/\u0000]/u.test(entry.name) ||
    (entry.kind !== "file" && entry.kind !== "directory"))) {
    throw new Error("Invalid development source directory listing");
  }
  return result.entries;
}

export async function openDevSourceFile(root: DevSourceDirectory,
  segments: readonly string[]): Promise<SourceFile> {
  if (segments.length === 0) throw new Error("Missing development source file");
  const metadata = await (await request(root, "file", segments)).json() as {
    size: number; lastModified: number; sha256: string; version: string;
  };
  if (!Number.isSafeInteger(metadata.size) || metadata.size < 0 || metadata.size > 0xffff_ffff ||
      !Number.isSafeInteger(metadata.lastModified) || metadata.lastModified < 0 ||
      typeof metadata.sha256 !== "string" || !/^[0-9a-f]{64}$/u.test(metadata.sha256) ||
      typeof metadata.version !== "string" || !metadata.version || metadata.version.length > 512) {
    throw new Error("Invalid development source file metadata");
  }
  const range = async (start: number, end: number): Promise<ArrayBuffer> => {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start ||
        end - start > chunkSize) throw new Error("Invalid development source range");
    const offset = Math.min(start, metadata.size);
    const length = Math.min(end, metadata.size) - offset;
    const response = await request(root, "range", segments, {
      offset: String(offset), length: String(length), version: metadata.version,
    });
    const bytes = await response.arrayBuffer();
    if (bytes.byteLength !== length) throw new Error("Incomplete development source read");
    return bytes;
  };
  return {
    size: metadata.size, lastModified: metadata.lastModified, sha256: metadata.sha256, sourceVersion: metadata.version,
    slice: (start, end) => ({ arrayBuffer: () => range(start, end) }),
    async arrayBuffer() {
      // Only PE/DLL callers request whole files; asset access always stays ranged.
      if (metadata.size > 16 * 1024 * 1024) throw new Error("Whole source read exceeds the bridge input limit");
      const bytes = new Uint8Array(metadata.size);
      for (let offset = 0; offset < metadata.size; offset += chunkSize) {
        bytes.set(new Uint8Array(await range(offset, Math.min(metadata.size, offset + chunkSize))), offset);
      }
      return bytes.buffer;
    },
  };
}
