// Browser-only backing store. Rust decides Windows file semantics; this module
// only confines relative paths to a single game's OPFS directory.
export async function openWritableTree(
  fingerprint: string,
): Promise<WritableTree> {
  if (!/^[0-9a-f]{64}$/u.test(fingerprint)) {
    throw new Error("Invalid game fingerprint");
  }
  const root = await navigator.storage.getDirectory();
  const game = await root.getDirectoryHandle(fingerprint, { create: true });
  return new WritableTree(
    await game.getDirectoryHandle("default", { create: true }),
  );
}

export class WritableTree {
  constructor(private readonly root: FileSystemDirectoryHandle) {}

  // Browser metadata only; Rust decides CreateFile/GetFileAttributes results.
  async stat(relativePath: string): Promise<{ kind: "file"; byteLength: number } | { kind: "directory" } | null> {
    const parts = pathParts(relativePath);
    const parent = await this.parent(parts);
    if (parent === null) return null;
    const name = await existingName(parent, parts.at(-1)!) ?? parts.at(-1)!;
    try {
      const handle = await parent.getFileHandle(name, { create: false });
      const file = await handle.getFile();
      if (!Number.isSafeInteger(file.size) || file.size < 0) throw new Error("OPFS file length is not representable");
      return { kind: "file", byteLength: file.size };
    } catch (error) {
      if (!isMissing(error) && !isWrongKind(error)) throw error;
    }
    try {
      await parent.getDirectoryHandle(name, { create: false });
      return { kind: "directory" };
    } catch (error) {
      if (isMissing(error) || isWrongKind(error)) return null;
      throw error;
    }
  }

  async read(
    relativePath: string,
    offset: number,
    length: number,
  ): Promise<Uint8Array | null> {
    const parts = pathParts(relativePath);
    if (
      !Number.isSafeInteger(offset) || offset < 0 ||
      !Number.isSafeInteger(length) || length < 0 || length > 2 * 1024 * 1024 ||
      !Number.isSafeInteger(offset + length)
    ) {
      throw new Error("Invalid OPFS read range");
    }
    const parent = await this.parent(parts);
    if (parent === null) return null;
    try {
      const name = await existingName(parent, parts.at(-1)!);
      if (name === null) return null;
      const file = await parent.getFileHandle(name, { create: false });
      const snapshot = await file.getFile();
      return new Uint8Array(
        await snapshot.slice(offset, offset + length).arrayBuffer(),
      );
    } catch (error) {
      if (isMissing(error)) return null;
      throw error;
    }
  }

  // Create/truncate only when Rust has authorized a successful file open.
  // A missing parent or a failed stream must not leave an empty save behind.
  async createEmpty(relativePath: string, sourceParents = 0): Promise<void> {
    await this.writeBytes(relativePath, new Uint8Array(), undefined, sourceParents);
  }

  // A WriteFile on an already-open handle must never recreate a file that
  // disappeared between open and write. The browser does not own that handle.
  async writeExisting(relativePath: string, bytes: Uint8Array, offset: number): Promise<void> {
    await this.writeBytes(relativePath, bytes, offset);
  }

  // Only createEmpty can create/truncate; a cursor write always preserves
  // surrounding bytes and never recreates a missing file.
  private async writeBytes(relativePath: string, bytes: Uint8Array, offset?: number, sourceParents = 0): Promise<void> {
    const parts = pathParts(relativePath);
    if (!Number.isInteger(sourceParents) || sourceParents < 0 || sourceParents >= parts.length ||
        (offset !== undefined && sourceParents !== 0)) {
      throw new Error("Invalid source-verified parent count");
    }
    if (bytes.byteLength > 2 * 1024 * 1024 ||
        (offset !== undefined && (!Number.isSafeInteger(offset) || offset < 0 ||
          !Number.isSafeInteger(offset + bytes.byteLength)))) {
      throw new Error("Invalid OPFS write range");
    }
    const parent = await this.parent(parts, sourceParents);
    if (parent === null) throw new DOMException("Parent directory does not exist", "NotFoundError");
    const stableBytes = new Uint8Array(bytes);
    const name = await existingName(parent, parts.at(-1)!) ?? parts.at(-1)!;
    let file: FileSystemFileHandle;
    let created = false;
    try {
      file = await parent.getFileHandle(name, { create: false });
    } catch (error) {
      if (!isMissing(error) || offset !== undefined) throw error;
      file = await parent.getFileHandle(name, { create: true });
      created = true;
    }
    let stream: FileSystemWritableFileStream | null = null;
    try {
      stream = await file.createWritable({ keepExistingData: offset !== undefined });
      if (offset !== undefined) await stream.seek(offset);
      await stream.write(stableBytes);
      await stream.close();
    } catch (error) {
      if (stream !== null) {
        try { await stream.abort(); } catch { /* Already closed or aborted. */ }
      }
      if (created) {
        try { await parent.removeEntry(name); } catch { /* Preserve the original write failure. */ }
      }
      throw error;
    }
  }

  async createDirectory(relativePath: string, sourceParents = 0): Promise<void> {
    const parts = pathParts(relativePath);
    if (!Number.isInteger(sourceParents) || sourceParents < 0 || sourceParents >= parts.length) {
      throw new Error("Invalid source-verified parent count");
    }
    const parent = await this.parent(parts, sourceParents);
    if (parent === null) throw new DOMException("Parent directory does not exist", "NotFoundError");
    const name = await existingName(parent, parts.at(-1)!) ?? parts.at(-1)!;
    await parent.getDirectoryHandle(name, { create: true });
  }

  // DeleteFile must not remove an empty directory even if the path is present.
  async removeFile(relativePath: string): Promise<void> {
    const parts = pathParts(relativePath);
    const parent = await this.parent(parts);
    if (parent === null) throw new DOMException("Parent directory does not exist", "NotFoundError");
    const name = await existingName(parent, parts.at(-1)!);
    if (name === null) throw new DOMException("File does not exist", "NotFoundError");
    await parent.getFileHandle(name, { create: false });
    await parent.removeEntry(name);
  }

  async remove(relativePath: string): Promise<void> {
    const parts = pathParts(relativePath);
    const parent = await this.parent(parts);
    if (parent === null) throw new DOMException("Parent directory does not exist", "NotFoundError");
    const name = await existingName(parent, parts.at(-1)!);
    if (name === null) throw new DOMException("File does not exist", "NotFoundError");
    await parent.removeEntry(name);
  }

  private async parent(parts: string[], sourceParents = 0): Promise<FileSystemDirectoryHandle | null> {
    let directory = this.root;
    for (const [index, segment] of parts.slice(0, -1).entries()) {
      try {
        const name = await existingName(directory, segment);
        if (name === null && index >= sourceParents) return null;
        directory = await directory.getDirectoryHandle(name ?? segment, { create: index < sourceParents });
      } catch (error) {
        if (isMissing(error)) return null;
        throw error;
      }
    }
    return directory;
  }
}

// Windows-932 path identity folds ASCII, not arbitrary Unicode. Resolve the
// actual OPFS spelling before operating, and fail closed on pre-existing
// collisions instead of selecting or overwriting an arbitrary save file.
async function existingName(directory: FileSystemDirectoryHandle, requested: string): Promise<string | null> {
  const identity = asciiFold(requested);
  let found: string | null = null;
  for await (const name of directory.keys()) {
    if (asciiFold(name) !== identity) continue;
    if (found !== null) throw new Error("Ambiguous case-insensitive OPFS path");
    found = name;
  }
  return found;
}

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/gu, letter => letter.toLowerCase());
}

function pathParts(path: string): string[] {
  if (
    path.length === 0 || path.length > 4096 ||
    /^[\\/]/u.test(path) || /^[a-z]:/iu.test(path)
  ) {
    throw new Error("Writable path must be relative to the game");
  }
  const parts = path.split(/[\\/]/u);
  if (parts.length > 128 || parts.some((part) =>
    !part || part === "." || part === ".." || part.length > 255 ||
    /[\u0000-\u001f\u007f:]/u.test(part)
  )) {
    throw new Error("Writable path escapes or exceeds the game directory");
  }
  return parts;
}

function isMissing(error: unknown): boolean {
  return error instanceof DOMException && error.name === "NotFoundError";
}

function isWrongKind(error: unknown): boolean {
  return error instanceof DOMException && error.name === "TypeMismatchError";
}
