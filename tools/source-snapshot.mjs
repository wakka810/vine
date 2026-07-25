import crypto from "node:crypto";
import { readFile } from "node:fs/promises";

export const sourceSnapshotSchema = "vine.source-snapshot-manifest";
export const sourceSnapshotVersion = 1;
export const sourceReadService = "storage.source-read.v1";
export const profileReadService = "storage.profile-read.v1";
export const sourceReadTransportVersion = 1;
export const sourceReadMaximumBytes = 1024 * 1024;

export async function registerSourceSnapshot({ manifestPath, objectFiles }) {
  const manifestBytes = await readFile(manifestPath);
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  validateManifest(manifest);
  const configured = new Map(
    Object.entries(objectFiles).map(([objectId, file]) => [Number(objectId), file]),
  );
  if (configured.size !== manifest.files.length) {
    throw new Error("source snapshot object file count does not match the manifest");
  }
  const objects = new Map();
  for (const descriptor of manifest.files) {
    const file = configured.get(descriptor.objectId);
    if (typeof file !== "string" || file.length === 0) {
      throw new Error(`source object ${descriptor.objectId} has no registered host file`);
    }
    const bytes = await readFile(file);
    const sha256 = hash(bytes);
    if (bytes.byteLength !== descriptor.byteLength || sha256 !== descriptor.sha256) {
      throw new Error(
        `source object ${descriptor.objectId} identity mismatch: ` +
          `${bytes.byteLength} bytes/${sha256}`,
      );
    }
    objects.set(descriptor.objectId, {
      descriptor: structuredClone(descriptor),
      bytes: Buffer.from(bytes),
    });
  }
  return Object.freeze({
    manifestPath,
    manifestBytes: Buffer.from(manifestBytes),
    manifest: structuredClone(manifest),
    objects,
  });
}

export function decodeSourceReadRequest(requestId, service, arguments_) {
  if (service !== sourceReadService && service !== profileReadService) {
    throw new Error(`unsupported source service ${service}`);
  }
  if (!/^(0|[1-9][0-9]*)$/u.test(String(requestId))) {
    throw new Error("source read request ID is not a canonical unsigned integer");
  }
  if (
    !Array.isArray(arguments_) ||
    arguments_.length !== 6 ||
    arguments_.some(
      (word) => !Number.isInteger(word) || word < 0 || word > 0xffff_ffff,
    )
  ) {
    throw new Error("source read request payload must contain six u32 words");
  }
  const [version, sourceId, objectId, offsetLow, offsetHigh, requestedBytes] = arguments_;
  if (version !== sourceReadTransportVersion) {
    throw new Error(`unsupported source read transport version ${version}`);
  }
  if (requestedBytes === 0 || requestedBytes > sourceReadMaximumBytes) {
    throw new Error("source read request length exceeds the bounded transport");
  }
  return {
    requestId: String(requestId),
    sourceId,
    objectId,
    offset: BigInt(offsetLow) | (BigInt(offsetHigh) << 32n),
    requestedBytes,
  };
}

export function readSourceSnapshot(registration, request) {
  const { manifest, objects } = registration;
  if (request.sourceId !== manifest.mount.sourceId) {
    throw sourceIdentityError("source-read-source-unknown", "source mount identity is not registered");
  }
  const object = objects.get(request.objectId);
  if (object === undefined) {
    throw sourceIdentityError("source-read-object-unknown", "source object identity is not registered");
  }
  if (request.offset > BigInt(object.bytes.byteLength)) {
    throw sourceIdentityError("source-read-range-invalid", "source read offset exceeds the object");
  }
  if (
    !Number.isInteger(request.requestedBytes) ||
    request.requestedBytes <= 0 ||
    request.requestedBytes > sourceReadMaximumBytes
  ) {
    throw sourceIdentityError("source-read-length-invalid", "source read length exceeds the bound");
  }
  const offset = Number(request.offset);
  const transferredBytes = Math.min(
    request.requestedBytes,
    object.bytes.byteLength - offset,
  );
  return {
    snapshotId: manifest.snapshotId,
    sourceId: request.sourceId,
    objectId: request.objectId,
    offset: request.offset,
    requestedBytes: request.requestedBytes,
    transferredBytes,
    bytes: Buffer.from(object.bytes.subarray(offset, offset + transferredBytes)),
  };
}

export function encodeSourceReadResponse(response, win32Error = 0) {
  const control = Buffer.alloc(32);
  const words = [
    sourceReadTransportVersion,
    response.sourceId,
    response.objectId,
    Number(response.offset & 0xffff_ffffn),
    Number((response.offset >> 32n) & 0xffff_ffffn),
    response.requestedBytes,
    win32Error === 0 ? response.transferredBytes : 0,
    win32Error,
  ];
  words.forEach((word, index) => control.writeUInt32LE(word >>> 0, index * 4));
  return {
    control,
    payload: win32Error === 0 ? Buffer.from(response.bytes) : Buffer.alloc(0),
  };
}

export function handleSourceSnapshotHttp(request, response, registration) {
  const url = new URL(request.url ?? "/", "http://127.0.0.1");
  const prefix = "/api/source/v1";
  if (!url.pathname.startsWith(`${prefix}/`) && url.pathname !== `${prefix}/manifest`) {
    return false;
  }
  const commonHeaders = {
    "Cache-Control": "no-store",
    "Cross-Origin-Resource-Policy": "same-origin",
  };
  if (request.method !== "GET") {
    response.writeHead(405, { ...commonHeaders, Allow: "GET" });
    response.end();
    return true;
  }
  if (url.pathname === `${prefix}/manifest`) {
    if (url.search !== "") {
      response.writeHead(400, commonHeaders);
      response.end();
      return true;
    }
    response.writeHead(200, {
      ...commonHeaders,
      "Content-Type": "application/json; charset=utf-8",
      "Content-Length": registration.manifestBytes.byteLength,
    });
    response.end(registration.manifestBytes);
    return true;
  }
  const match = /^\/api\/source\/v1\/titles\/([^/]+)\/snapshots\/([^/]+)\/sources\/([0-9]+)\/objects\/([0-9]+)$/u.exec(
    url.pathname,
  );
  if (match === null) {
    response.writeHead(404, commonHeaders);
    response.end();
    return true;
  }
  let titleFingerprint;
  let snapshotId;
  try {
    titleFingerprint = decodeURIComponent(match[1]);
    snapshotId = decodeURIComponent(match[2]);
  } catch {
    response.writeHead(400, commonHeaders);
    response.end();
    return true;
  }
  const queryKeys = [...url.searchParams.keys()];
  const offsetText = url.searchParams.get("offset");
  const lengthText = url.searchParams.get("length");
  if (
    titleFingerprint !== registration.manifest.titleFingerprint ||
    snapshotId !== registration.manifest.snapshotId ||
    !canonicalDecimal(match[3]) ||
    !canonicalDecimal(match[4]) ||
    queryKeys.length !== 2 ||
    new Set(queryKeys).size !== 2 ||
    !queryKeys.includes("offset") ||
    !queryKeys.includes("length") ||
    !canonicalDecimal(offsetText) ||
    !canonicalDecimal(lengthText)
  ) {
    response.writeHead(404, commonHeaders);
    response.end();
    return true;
  }
  const sourceId = Number(match[3]);
  const objectId = Number(match[4]);
  const requestedBytes = Number(lengthText);
  let offset;
  try {
    offset = BigInt(offsetText);
  } catch {
    response.writeHead(416, commonHeaders);
    response.end();
    return true;
  }
  try {
    const result = readSourceSnapshot(registration, {
      sourceId,
      objectId,
      offset,
      requestedBytes,
    });
    response.writeHead(206, {
      ...commonHeaders,
      "Content-Type": "application/octet-stream",
      "Content-Length": result.bytes.byteLength,
      "X-Vine-Title-Fingerprint": registration.manifest.titleFingerprint,
      "X-Vine-Snapshot-Id": result.snapshotId,
      "X-Vine-Source-Id": String(result.sourceId),
      "X-Vine-Object-Id": String(result.objectId),
      "X-Vine-Offset": result.offset.toString(),
      "X-Vine-Requested-Bytes": String(result.requestedBytes),
      "X-Vine-Transferred-Bytes": String(result.transferredBytes),
    });
    response.end(result.bytes);
  } catch (error) {
    const status = error?.code === "source-read-range-invalid" ? 416 : 404;
    response.writeHead(status, commonHeaders);
    response.end();
  }
  return true;
}

function validateManifest(manifest) {
  if (
    manifest?.schema !== sourceSnapshotSchema ||
    manifest.schemaVersion !== sourceSnapshotVersion ||
    typeof manifest.titleFingerprint !== "string" ||
    manifest.titleFingerprint.length === 0 ||
    manifest.titleFingerprint.length > 128 ||
    typeof manifest.snapshotId !== "string" ||
    !/^[a-zA-Z0-9._-]{1,128}$/u.test(manifest.snapshotId) ||
    !Number.isInteger(manifest.mount?.sourceId) ||
    manifest.mount.sourceId <= 0 ||
    !validByteArray(manifest.mount.rootPathA, true) ||
    !Array.isArray(manifest.files) ||
    manifest.files.length === 0 ||
    manifest.files.length > 4096
  ) {
    throw new Error("source snapshot manifest identity or mount is invalid");
  }
  const objectIds = new Set();
  const paths = new Set();
  for (const file of manifest.files) {
    const validPath = validByteArray(file.relativePathA, false);
    const pathIdentity = validPath
      ? Buffer.from(windows932AsciiFold(file.relativePathA)).toString("hex")
      : null;
    if (
      !Number.isInteger(file.objectId) ||
      file.objectId <= 0 ||
      objectIds.has(file.objectId) ||
      !validPath ||
      paths.has(pathIdentity) ||
      !Number.isSafeInteger(file.byteLength) ||
      file.byteLength < 0 ||
      file.byteLength > 0xffff_ffff ||
      typeof file.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/u.test(file.sha256)
    ) {
      throw new Error("source snapshot file descriptor is invalid or duplicated");
    }
    objectIds.add(file.objectId);
    paths.add(pathIdentity);
  }
}

function validByteArray(bytes, trailingNul) {
  return (
    Array.isArray(bytes) &&
    bytes.length > 0 &&
    bytes.length <= 32_768 &&
    bytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff) &&
    (trailingNul
      ? bytes.at(-1) === 0 && !bytes.slice(0, -1).includes(0)
      : !bytes.includes(0))
  );
}

function canonicalDecimal(value) {
  return typeof value === "string" && /^(0|[1-9][0-9]*)$/u.test(value);
}

function windows932AsciiFold(bytes) {
  const folded = [];
  for (let index = 0; index < bytes.length; index += 1) {
    const byte = bytes[index];
    const trail = bytes[index + 1];
    if (isWindows932LeadByte(byte) && isWindows932TrailByte(trail)) {
      folded.push(byte, trail);
      index += 1;
    } else {
      folded.push(byte >= 0x41 && byte <= 0x5a ? byte + 0x20 : byte);
    }
  }
  return folded;
}

function isWindows932LeadByte(byte) {
  return Number.isInteger(byte) && ((byte >= 0x81 && byte <= 0x9f) || (byte >= 0xe0 && byte <= 0xfc));
}

function isWindows932TrailByte(byte) {
  return Number.isInteger(byte) &&
    ((byte >= 0x40 && byte <= 0x7e) || (byte >= 0x80 && byte <= 0xfc)) &&
    byte !== 0x7f;
}

function hash(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function sourceIdentityError(code, message) {
  return Object.assign(new Error(message), { code });
}
