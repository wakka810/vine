import { sha256 } from "@noble/hashes/sha2.js";

// Walk only handles obtained from the selected root. No user-supplied path is
// passed to a filesystem API, and browsing never requests write permission.
export interface SourceEntry {
  name: string;
  kind: "directory" | "executable" | "file";
}

export async function browseSourceDirectory(
  root: FileSystemDirectoryHandle,
  segments: readonly string[],
): Promise<SourceEntry[]> {
  let directory = root;
  for (const segment of segments) {
    if (!validSegment(segment)) {
      throw new Error("Invalid directory name");
    }
    directory = await directory.getDirectoryHandle(segment, { create: false });
  }
  const entries: SourceEntry[] = [];
  for await (const [name, handle] of directory.entries()) {
    if (!validSegment(name)) {
      continue;
    }
    if (handle.kind === "directory") {
      entries.push({ name, kind: "directory" });
    } else if (handle.kind === "file") {
      entries.push({ name, kind: /\.exe$/iu.test(name) ? "executable" : "file" });
    }
  }
  return entries.sort((a, b) =>
    (a.kind === "directory" ? 0 : 1) - (b.kind === "directory" ? 0 : 1) ||
    a.name.localeCompare(b.name),
  );
}

export async function openSourceExecutable(
  root: FileSystemDirectoryHandle,
  segments: readonly string[],
  name: string,
): Promise<File> {
  if (!/\.exe$/iu.test(name)) throw new Error("Not an executable filename");
  return openSourceFile(root, [...segments, name]);
}

export async function openSourceDll(
  root: FileSystemDirectoryHandle, segments: readonly string[], name: string,
): Promise<File> {
  if (!/\.dll$/iu.test(name)) throw new Error("Not a DLL filename");
  return openSourceFile(root, [...segments, name]);
}

export interface VerifiedSourceFile {
  kind: "file";
  pathSegments: string[];
  byteLength: number;
  lastModified: number;
  sha256: string;
}

// Directory enumeration returns names, not Windows matches. Rust selects which
// candidates to verify; no source file is opened or hashed during this listing.
export async function listSourceDirectory(
  root: FileSystemDirectoryHandle, relativeDirectoryA: readonly number[],
): Promise<{ pathSegments: string[]; entries: Array<{ name: string; kind: "file" | "directory" }> } | null> {
  if (relativeDirectoryA.length > 4096 || relativeDirectoryA.some((byte) =>
    !Number.isInteger(byte) || byte < 0 || byte > 255)) {
    throw new Error("Invalid source search directory");
  }
  const parts = relativeDirectoryA.length === 0 ? [] :
    new TextDecoder("shift_jis", { fatal: true })
      .decode(Uint8Array.from(relativeDirectoryA)).split("\\");
  if (parts.length > 256 || parts.some((part) => !validSegment(part))) {
    throw new Error("Source search escapes the selected directory");
  }
  let directory = root;
  const pathSegments: string[] = [];
  for (const part of parts) {
    const matches: string[] = [];
    for await (const [name, handle] of directory.entries()) {
      if (handle.kind === "directory" && validSegment(name) && asciiFold(name) === asciiFold(part)) {
        matches.push(name);
      }
    }
    if (matches.length === 0) return null;
    if (matches.length !== 1) throw new Error("Ambiguous source directory under Windows case folding");
    pathSegments.push(matches[0]!);
    directory = await directory.getDirectoryHandle(matches[0]!, { create: false });
  }
  const entries: Array<{ name: string; kind: "file" | "directory" }> = [];
  for await (const [name, handle] of directory.entries()) {
    if (!validSegment(name) || !["file", "directory"].includes(handle.kind)) continue;
    if (entries.length === 4096) throw new Error("Source directory exceeds bounded enumeration");
    entries.push({ name, kind: handle.kind as "file" | "directory" });
  }
  return { pathSegments, entries };
}

// The relative Windows-932 path is resolved by Rust. Here we only locate that
// name beneath the selected handle and hash the immutable browser snapshot.
export async function inspectSourcePath(
  root: FileSystemDirectoryHandle,
  relativePathA: readonly number[],
): Promise<VerifiedSourceFile | {kind: "directory"} | null> {
  if (relativePathA.length === 0 || relativePathA.length > 4096 ||
      relativePathA.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
    throw new Error("Invalid source lookup path");
  }
  const parts = new TextDecoder("shift_jis", { fatal: true })
    .decode(Uint8Array.from(relativePathA)).split("\\");
  if (parts.length > 256 || parts.some((part) => !validSegment(part))) {
    throw new Error("Source lookup escapes the selected directory");
  }
  const resolved: string[] = [];
  let directory = root;
  for (let index = 0; index < parts.length; index += 1) {
    const candidate = parts[index]!;
    const matches: Array<[string, FileSystemHandle]> = [];
    for await (const [name, handle] of directory.entries()) {
      if (validSegment(name) && asciiFold(name) === asciiFold(candidate)) matches.push([name, handle]);
    }
    if (matches.length === 0) return null;
    if (matches.length !== 1) throw new Error("Ambiguous source path under Windows case folding");
    const [name, handle] = matches[0]!;
    resolved.push(name);
    if (index < parts.length - 1) {
      if (handle.kind !== "directory") return null;
      directory = await directory.getDirectoryHandle(name, { create: false });
      continue;
    }
    if (handle.kind === "directory") return { kind: "directory" };
    if (handle.kind !== "file") return null;
  }
  const file = await openSourceFile(root, resolved);
  if (!Number.isSafeInteger(file.size) || file.size > 0xffff_ffff) {
    throw new Error("Source file exceeds PE32 file size support");
  }
  const hasher = sha256.create();
  for (let offset = 0; offset < file.size; offset += 2 * 1024 * 1024) {
    hasher.update(new Uint8Array(await file.slice(offset, offset + 2 * 1024 * 1024).arrayBuffer()));
  }
  const after = await openSourceFile(root, resolved);
  if (after.size !== file.size || after.lastModified !== file.lastModified) {
    throw new Error("Source file changed while hashing");
  }
  return {
    kind: "file",
    pathSegments: resolved,
    byteLength: file.size,
    lastModified: file.lastModified,
    sha256: Array.from(hasher.digest(), (byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/gu, (letter) => letter.toLowerCase());
}

export async function readSourceRange(
  root: FileSystemDirectoryHandle,
  pathSegments: readonly string[],
  byteLength: number,
  lastModified: number,
  offset: number,
  length: number,
): Promise<Uint8Array> {
  if (
    !Number.isSafeInteger(byteLength) || byteLength < 0 ||
    !Number.isSafeInteger(lastModified) || lastModified < 0 ||
    !Number.isSafeInteger(offset) || offset < 0 || offset > 0xffff_ffff ||
    !Number.isSafeInteger(length) || length < 0 || length > 2 * 1024 * 1024
  ) throw new Error("Invalid source read range");
  const before = await openSourceFile(root, pathSegments);
  if (before.size !== byteLength || before.lastModified !== lastModified) {
    throw new Error("Source file changed since it was verified");
  }
  const transferred = Math.min(length, Math.max(0, byteLength - offset));
  const bytes = new Uint8Array(await before.slice(offset, offset + transferred).arrayBuffer());
  const after = await openSourceFile(root, pathSegments);
  if (after.size !== byteLength || after.lastModified !== lastModified ||
      bytes.byteLength !== transferred) {
    throw new Error("Source file changed during the read");
  }
  return bytes;
}

async function openSourceFile(
  root: FileSystemDirectoryHandle,
  pathSegments: readonly string[],
): Promise<File> {
  if (pathSegments.length === 0 || pathSegments.length > 256 ||
      pathSegments.some((segment) => segment.length > 255 || !validSegment(segment))) {
    throw new Error("Invalid source path");
  }
  let directory = root;
  for (const segment of pathSegments.slice(0, -1)) {
    directory = await directory.getDirectoryHandle(segment, { create: false });
  }
  return (await directory.getFileHandle(pathSegments.at(-1)!, { create: false })).getFile();
}

function validSegment(segment: string): boolean {
  return segment.length > 0 && segment !== "." && segment !== ".." &&
    !/[\\/\u0000]/u.test(segment);
}
