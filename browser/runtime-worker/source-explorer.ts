import { isDevSource, listDevSource, openDevSourceFile, type SourceDirectory, type SourceFile } from "../storage/dev-source-directory.js";

// Walk only handles obtained from the selected root. No user-supplied path is
// passed to a filesystem API, and browsing never requests write permission.
export interface SourceEntry {
  name: string;
  kind: "directory" | "executable" | "file";
}

export async function browseSourceDirectory(
  root: SourceDirectory,
  segments: readonly string[],
): Promise<SourceEntry[]> {
  const entries: SourceEntry[] = [];
  for (const { name, kind } of await sourceEntries(root, segments)) {
    if (kind === "directory") {
      entries.push({ name, kind: "directory" });
    } else if (kind === "file") {
      entries.push({ name, kind: /\.exe$/iu.test(name) ? "executable" : "file" });
    }
  }
  return entries.sort((a, b) =>
    (a.kind === "directory" ? 0 : 1) - (b.kind === "directory" ? 0 : 1) ||
    a.name.localeCompare(b.name),
  );
}

export async function openSourceExecutable(
  root: SourceDirectory,
  segments: readonly string[],
  name: string,
): Promise<SourceFile> {
  if (!/\.exe$/iu.test(name)) throw new Error("Not an executable filename");
  return openSourceFile(root, [...segments, name]);
}

export async function openSourceDll(
  root: SourceDirectory, segments: readonly string[], name: string,
): Promise<SourceFile> {
  if (!/\.dll$/iu.test(name)) throw new Error("Not a DLL filename");
  return openSourceFile(root, [...segments, name]);
}

export interface VerifiedSourceFile {
  kind: "file";
  pathSegments: string[];
  byteLength: number;
  lastModified: number;
  sourceVersion?: string;
}

// Directory enumeration returns names, not Windows matches. Rust selects which
// candidates to verify; no source file is opened or hashed during this listing.
export async function listSourceDirectory(
  root: SourceDirectory, relativeDirectoryA: readonly number[],
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
  const pathSegments: string[] = [];
  for (const part of parts) {
    const matches: string[] = [];
    for (const { name, kind } of await sourceEntries(root, pathSegments)) {
      if (kind === "directory" && validSegment(name) && asciiFold(name) === asciiFold(part)) {
        matches.push(name);
      }
    }
    if (matches.length === 0) return null;
    if (matches.length !== 1) throw new Error("Ambiguous source directory under Windows case folding");
    pathSegments.push(matches[0]!);
  }
  const entries: Array<{ name: string; kind: "file" | "directory" }> = [];
  for (const { name, kind } of await sourceEntries(root, pathSegments)) {
    if (!validSegment(name) || !["file", "directory"].includes(kind)) continue;
    if (entries.length === 4096) throw new Error("Source directory exceeds bounded enumeration");
    entries.push({ name, kind });
  }
  return { pathSegments, entries };
}

// The relative Windows-932 path is resolved by Rust. Here we only locate that
// name beneath the selected handle and verify its bounded metadata. Asset reads
// validate the same snapshot observations before and after each requested range.
export async function inspectSourcePath(
  root: SourceDirectory,
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
  for (let index = 0; index < parts.length; index += 1) {
    const candidate = parts[index]!;
    const matches = (await sourceEntries(root, resolved)).filter(({name}) =>
      validSegment(name) && asciiFold(name) === asciiFold(candidate));
    if (matches.length === 0) return null;
    if (matches.length !== 1) throw new Error("Ambiguous source path under Windows case folding");
    const handle = matches[0]!;
    resolved.push(handle.name);
    if (index < parts.length - 1) {
      if (handle.kind !== "directory") return null;
      continue;
    }
    if (handle.kind === "directory") return { kind: "directory" };
    if (handle.kind !== "file") return null;
  }
  const file = await openSourceFile(root, resolved);
  if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > 0xffff_ffff ||
      !Number.isSafeInteger(file.lastModified) || file.lastModified < 0) {
    throw new Error("Invalid source file metadata");
  }
  const after = await openSourceFile(root, resolved);
  if (after.size !== file.size || after.lastModified !== file.lastModified ||
      after.sourceVersion !== file.sourceVersion) {
    throw new Error("Source file changed during inspection");
  }
  return {
    kind: "file",
    pathSegments: resolved,
    byteLength: file.size,
    lastModified: file.lastModified,
    ...(file.sourceVersion === undefined ? {} : { sourceVersion: file.sourceVersion }),
  };
}

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/gu, (letter) => letter.toLowerCase());
}

export async function readSourceRange(
  root: SourceDirectory,
  pathSegments: readonly string[],
  byteLength: number,
  lastModified: number,
  offset: number,
  length: number,
  sourceVersion?: string,
): Promise<Uint8Array> {
  if (
    !Number.isSafeInteger(byteLength) || byteLength < 0 ||
    !Number.isSafeInteger(lastModified) || lastModified < 0 ||
    !Number.isSafeInteger(offset) || offset < 0 || offset > 0xffff_ffff ||
    !Number.isSafeInteger(length) || length < 0 || length > 2 * 1024 * 1024
  ) throw new Error("Invalid source read range");
  const before = await openSourceFile(root, pathSegments);
  if (before.size !== byteLength || before.lastModified !== lastModified ||
      before.sourceVersion !== sourceVersion) {
    throw new Error("Source file changed since it was verified");
  }
  const transferred = Math.min(length, Math.max(0, byteLength - offset));
  const bytes = new Uint8Array(await before.slice(offset, offset + transferred).arrayBuffer());
  const after = await openSourceFile(root, pathSegments);
  if (after.size !== byteLength || after.lastModified !== lastModified ||
      after.sourceVersion !== sourceVersion ||
      bytes.byteLength !== transferred) {
    throw new Error("Source file changed during the read");
  }
  return bytes;
}

async function openSourceFile(
  root: SourceDirectory,
  pathSegments: readonly string[],
): Promise<SourceFile> {
  if (pathSegments.length === 0 || pathSegments.length > 256 ||
      pathSegments.some((segment) => segment.length > 255 || !validSegment(segment))) {
    throw new Error("Invalid source path");
  }
  if (isDevSource(root)) return openDevSourceFile(root, pathSegments);
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

async function sourceEntries(root: SourceDirectory, segments: readonly string[]):
Promise<Array<{ name: string; kind: "file" | "directory" }>> {
  if (segments.length > 256 || segments.some(segment => !validSegment(segment) || segment.length > 255)) {
    throw new Error("Invalid directory name");
  }
  if (isDevSource(root)) return listDevSource(root, segments);
  let directory = root;
  for (const segment of segments) directory = await directory.getDirectoryHandle(segment, { create: false });
  const entries: Array<{ name: string; kind: "file" | "directory" }> = [];
  for await (const [name, handle] of directory.entries()) {
    if (!validSegment(name) || (handle.kind !== "file" && handle.kind !== "directory")) continue;
    if (entries.length >= 4096) throw new Error("Source directory exceeds bounded enumeration");
    entries.push({ name, kind: handle.kind });
  }
  return entries;
}
