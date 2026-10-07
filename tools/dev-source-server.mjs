import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import { constants } from "node:fs";
import { lstat, open, opendir, realpath } from "node:fs/promises";
import path from "node:path";

const prefix = "/__vine_dev_source/";
const maxRange = 2 * 1024 * 1024;
const maxEntries = 4096;
const maxCachedFiles = 256;
const maxConcurrentRequests = 16;

class SourceError extends Error {
  constructor(status, message) { super(message); this.status = status; }
}
const fail = (status, message) => { throw new SourceError(status, message); };
const hash = (value) => createHash("sha256").update(value).digest("hex");
const descriptorPath = (handle) => `/proc/self/fd/${handle.fd}`;
const validName = (name) => typeof name === "string" && name.length > 0 &&
  name.length <= 255 && Buffer.byteLength(name) <= 255 && name !== "." && name !== ".." &&
  !/[\\/\u0000-\u001f\u007f]/u.test(name) && name.isWellFormed();

function snapshot(stat) {
  // Nanosecond ctime and inode identity catch replacement and writes that restore mtime.
  return hash([stat.dev, stat.ino, stat.mode, stat.nlink, stat.size,
    stat.mtimeNs, stat.ctimeNs].join(":"));
}

function fileMetadata(stat) {
  const size = Number(stat.size);
  const lastModified = Number(stat.mtimeNs / 1_000_000n);
  if (!Number.isSafeInteger(size) || size < 0 ||
      !Number.isSafeInteger(lastModified) || lastModified < 0) {
    fail(413, "Source metadata exceeds supported integer bounds");
  }
  return { size, lastModified, version: snapshot(stat) };
}

function pathSegments(parameters) {
  let segments;
  try { segments = JSON.parse(parameters.get("path")); } catch { fail(400, "Invalid source path"); }
  if (!Array.isArray(segments) || segments.length > 256 || !segments.every(validName) ||
      Buffer.byteLength(segments.join("/")) > 4096) fail(400, "Invalid source path");
  return segments;
}

function integerParameter(parameters, name, maximum) {
  const text = parameters.get(name);
  if (typeof text !== "string" || !/^(?:0|[1-9][0-9]*)$/u.test(text)) fail(400, "Invalid source range");
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value > maximum) fail(400, "Invalid source range");
  return value;
}

function requireSameOrigin(request) {
  const host = request.headers.host;
  // Explicit literal loopback names also admit the cloud browser's 127.0.0.2 route.
  // The port must be this listener's, not another service on the same machine.
  const match = typeof host === "string" && /^(127\.0\.0\.[12]|localhost|\[::1\])(?::([1-9][0-9]{0,4}))?$/u.exec(host);
  if (!match || Number(match[2] ?? 80) !== request.socket.localPort) fail(403, "Untrusted source host");
  const origin = request.headers.origin;
  const forwardedOrigin = match[1] === "127.0.0.2" && origin === `https://${host}`;
  if (origin !== undefined && origin !== `http://${host}` && !forwardedOrigin) {
    fail(403, "Source requests must be same-origin");
  }
  const site = request.headers["sec-fetch-site"];
  if (site !== undefined && site !== "same-origin" && site !== "none") {
    fail(403, "Source requests must be same-origin");
  }
}

function sendJson(response, status, value) {
  const bytes = Buffer.from(JSON.stringify(value));
  response.writeHead(status, { "Content-Type": "application/json; charset=utf-8", "Content-Length": bytes.length });
  response.end(bytes);
}

/**
 * A read-only development capability, never enabled by the browser build itself.
 * Linux descriptor-relative traversal is intentional: Node has no portable openat.
 * Every user-named component is opened O_NOFOLLOW beneath an already-open directory.
 * No selected-source path is ever opened for writing or copied into browser assets.
 */
export async function createDevSourceHandler(rootPath) {
  if (process.platform !== "linux" || !constants.O_NOFOLLOW || !constants.O_DIRECTORY) {
    throw new Error("The read-only development source currently requires Linux /proc/self/fd and O_NOFOLLOW");
  }
  if (typeof rootPath !== "string" || !path.isAbsolute(rootPath)) {
    throw new Error("--dev-game-root requires an absolute directory path");
  }
  const canonicalRoot = await realpath(rootPath);
  if (canonicalRoot !== path.resolve(rootPath)) {
    throw new Error("--dev-game-root must be a canonical directory path without symlinks");
  }
  const rootStat = await lstat(canonicalRoot, { bigint: true });
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) throw new Error("Development source root must be a directory");
  const rootHandle = await open(canonicalRoot, constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW);
  try {
    const openedRoot = await rootHandle.stat({ bigint: true });
    if (openedRoot.dev !== rootStat.dev || openedRoot.ino !== rootStat.ino ||
        await realpath(descriptorPath(rootHandle)) !== canonicalRoot) {
      throw new Error("Development source root changed while opening");
    }
  } catch (error) {
    await rootHandle.close();
    throw error;
  }
  const id = hash(canonicalRoot);
  const token = randomBytes(32).toString("hex");
  const tokenBytes = Buffer.from(token);
  const cache = new Map();
  let active = 0;
  let closed = false;

  const verifyDescriptor = async (handle, expected) => {
    if (await realpath(descriptorPath(handle)) !== expected) fail(409, "Source path changed");
    if (await realpath(descriptorPath(rootHandle)) !== canonicalRoot) fail(409, "Source root changed");
  };

  const withTarget = async (segments, directory, operation) => {
    const handles = [];
    let current = rootHandle;
    let expected = canonicalRoot;
    try {
      await verifyDescriptor(current, expected);
      for (let index = 0; index < segments.length; index += 1) {
        const name = segments[index];
        const directoryComponent = directory || index < segments.length - 1;
        const anchored = `${descriptorPath(current)}/${name}`;
        const observed = await lstat(anchored, { bigint: true });
        if (observed.isSymbolicLink()) fail(403, "Source symlinks are not allowed");
        if (!(directoryComponent ? observed.isDirectory() : observed.isFile())) {
          fail(403, "Source entry is not a regular file or directory");
        }
        // O_NONBLOCK ensures a file replaced with a FIFO cannot hang this server.
        const child = await open(anchored, constants.O_RDONLY | constants.O_NOFOLLOW |
          constants.O_NONBLOCK | (directoryComponent ? constants.O_DIRECTORY : 0));
        handles.push(child);
        const actual = await child.stat({ bigint: true });
        if (actual.dev !== observed.dev || actual.ino !== observed.ino ||
            !(directoryComponent ? actual.isDirectory() : actual.isFile())) fail(409, "Source entry changed");
        expected = path.join(expected, name);
        await verifyDescriptor(child, expected);
        current = child;
      }
      if (!directory && segments.length === 0) fail(400, "A source file path is required");
      const before = await current.stat({ bigint: true });
      const result = await operation(current, before);
      const after = await current.stat({ bigint: true });
      await verifyDescriptor(current, expected);
      if (snapshot(before) !== snapshot(after)) fail(409, "Source changed during the read");
      return result;
    } finally {
      await Promise.all(handles.map((handle) => handle.close()));
    }
  };

  const metadata = async (segments) => withTarget(segments, false, async (handle, stat) => {
    const result = fileMetadata(stat);
    const key = JSON.stringify(segments);
    let cached = cache.get(key);
    if (cached?.version !== result.version) {
      const digest = createHash("sha256");
      let read = 0;
      if (result.size > 0) {
        const stream = handle.createReadStream({ start: 0, end: result.size - 1,
          autoClose: false, highWaterMark: 1024 * 1024 });
        for await (const bytes of stream) { digest.update(bytes); read += bytes.length; }
      }
      if (read !== result.size || snapshot(await handle.stat({ bigint: true })) !== result.version) {
        fail(409, "Source changed while hashing");
      }
      cached = { ...result, sha256: digest.digest("hex") };
      cache.delete(key);
      cache.set(key, cached);
      if (cache.size > maxCachedFiles) cache.delete(cache.keys().next().value);
    } else {
      // Keep recently inspected files while placing a fixed bound on cache memory.
      cache.delete(key);
      cache.set(key, cached);
    }
    return cached;
  });

  const handler = async (request, response) => {
    if (typeof request.url !== "string" || !request.url.startsWith(prefix)) return false;
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("X-Content-Type-Options", "nosniff");
    let entered = false;
    try {
      requireSameOrigin(request);
      if (request.method !== "GET") {
        response.setHeader("Allow", "GET");
        fail(405, "Source access is read-only");
      }
      if (closed) fail(503, "Development source is closed");
      if (request.url.length > 32768) fail(414, "Source request is too long");
      const url = new URL(request.url, "http://127.0.0.1");
      const operation = url.pathname.slice(prefix.length);
      const fields = operation === "config" ? [] : operation === "range" ?
        ["path", "offset", "length", "version"] : ["path"];
      if (!["config", "list", "file", "range"].includes(operation)) fail(404, "Unknown source operation");
      if (fields.length !== [...url.searchParams.keys()].length ||
          fields.some((field) => url.searchParams.getAll(field).length !== 1)) fail(400, "Invalid source parameters");
      if (operation === "config") {
        sendJson(response, 200, { kind: "directory", transport: "dev-server", name: path.basename(canonicalRoot), id, token });
        return true;
      }
      const supplied = request.headers["x-vine-source-token"];
      if (typeof supplied !== "string" || !/^[0-9a-f]{64}$/u.test(supplied) ||
          !timingSafeEqual(Buffer.from(supplied), tokenBytes)) fail(403, "Invalid source capability");
      if (active >= maxConcurrentRequests) fail(503, "Too many concurrent source requests");
      active += 1;
      entered = true;
      const segments = pathSegments(url.searchParams);
      if (operation === "list") {
        const entries = await withTarget(segments, true, async (handle) => {
          const directory = await opendir(descriptorPath(handle));
          const entries = [];
          let visited = 0;
          for await (const entry of directory) {
            if (++visited > maxEntries) fail(413, "Source directory exceeds bounded enumeration");
            if (!validName(entry.name)) continue;
            const stat = await lstat(`${descriptorPath(handle)}/${entry.name}`);
            if (stat.isSymbolicLink()) continue;
            if (stat.isDirectory()) entries.push({ name: entry.name, kind: "directory" });
            else if (stat.isFile()) entries.push({ name: entry.name, kind: "file" });
          }
          return entries.sort((a, b) => a.name.localeCompare(b.name));
        });
        sendJson(response, 200, { entries });
      } else if (operation === "file") {
        sendJson(response, 200, await metadata(segments));
      } else {
        const offset = integerParameter(url.searchParams, "offset", Number.MAX_SAFE_INTEGER);
        const length = integerParameter(url.searchParams, "length", maxRange);
        const version = url.searchParams.get("version");
        if (!/^[0-9a-f]{64}$/u.test(version)) fail(400, "Invalid source version");
        const bytes = await withTarget(segments, false, async (handle, stat) => {
          const current = fileMetadata(stat);
          if (current.version !== version) fail(409, "Source changed since it was verified");
          const count = Math.min(length, Math.max(0, current.size - offset));
          const bytes = Buffer.alloc(count);
          let read = 0;
          while (read < count) {
            const chunk = await handle.read(bytes, read, count - read, offset + read);
            if (chunk.bytesRead === 0) fail(409, "Source changed during the read");
            read += chunk.bytesRead;
          }
          return bytes;
        });
        response.writeHead(200, { "Content-Type": "application/octet-stream", "Content-Length": bytes.length });
        response.end(bytes);
      }
    } catch (error) {
      const status = error instanceof SourceError ? error.status :
        ["ENOENT", "ENOTDIR"].includes(error?.code) ? 404 :
          ["EACCES", "EPERM", "ELOOP"].includes(error?.code) ? 403 : 500;
      sendJson(response, status, { error: error instanceof SourceError ? error.message :
        status === 404 ? "Source entry not found" : status === 403 ? "Source access denied" : "Source read failed" });
    } finally {
      if (entered) active -= 1;
    }
    return true;
  };
  handler.close = async () => {
    closed = true;
    cache.clear();
    await rootHandle.close();
  };
  return handler;
}
