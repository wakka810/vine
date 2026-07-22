import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

export const CANONICAL_CHROME_FOR_TESTING_MANIFEST = path.join(
  "fixtures",
  "toolchains",
  "chrome-for-testing-win64-151.0.7922.34-v1.json",
);

const sha256Pattern = /^[0-9a-f]{64}$/u;

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function validateRelativeArtifactPath(value, label) {
  invariant(typeof value === "string" && value.length > 0, `${label} must be a string`);
  invariant(!path.isAbsolute(value), `${label} must be relative`);
  const normalized = value.replaceAll("\\", "/");
  invariant(
    normalized !== ".." && !normalized.startsWith("../") && !normalized.includes("/../"),
    `${label} escapes its artifact root`,
  );
  invariant(!normalized.startsWith("/"), `${label} must not be rooted`);
  return normalized;
}

function validateFileIdentity(identity, label) {
  invariant(identity !== null && typeof identity === "object", `${label} is missing`);
  invariant(
    Number.isSafeInteger(identity.bytes) && identity.bytes > 0,
    `${label}.bytes must be a positive safe integer`,
  );
  invariant(sha256Pattern.test(identity.sha256), `${label}.sha256 must be lowercase SHA-256`);
}

export function validateChromeForTestingManifest(manifest) {
  invariant(manifest !== null && typeof manifest === "object", "Chrome artifact manifest is missing");
  invariant(manifest.schema === "vine.chrome-for-testing-artifact", "Chrome artifact schema is invalid");
  invariant(manifest.schemaVersion === 1, "Chrome artifact schema version is unsupported");
  invariant(
    typeof manifest.id === "string" && manifest.id.length > 0,
    "Chrome artifact ID is invalid",
  );
  invariant(/^\d+\.\d+\.\d+\.\d+$/u.test(manifest.version), "Chrome version is invalid");
  invariant(manifest.platform === "win64", "only the canonical win64 Chrome artifact is supported");
  invariant(manifest.source !== null && typeof manifest.source === "object", "Chrome source is missing");
  const sourceUrl = new URL(manifest.source.url);
  invariant(sourceUrl.protocol === "https:", "Chrome artifact URL must use HTTPS");
  invariant(
    sourceUrl.hostname === "storage.googleapis.com",
    "Chrome artifact URL must use the official Chrome for Testing storage host",
  );
  const expectedSuffix = `/chrome-for-testing-public/${manifest.version}/${manifest.platform}/chrome-win64.zip`;
  invariant(sourceUrl.pathname === expectedSuffix, "Chrome artifact URL does not match its identity");
  validateFileIdentity(manifest.source.archive, "Chrome archive identity");
  validateRelativeArtifactPath(manifest.source.archive.relativePath, "Chrome archive path");
  validateFileIdentity(manifest.executable, "Chrome executable identity");
  validateRelativeArtifactPath(manifest.executable.relativePath, "Chrome executable path");
  invariant(
    manifest.executable.productVersion === manifest.version,
    "Chrome executable product version differs from the artifact version",
  );
  return manifest;
}

export async function sha256File(filePath) {
  const hasher = crypto.createHash("sha256");
  await new Promise((resolve, reject) => {
    const input = fs.createReadStream(filePath);
    input.on("data", (chunk) => hasher.update(chunk));
    input.once("error", reject);
    input.once("end", resolve);
  });
  return hasher.digest("hex");
}

export async function verifyArtifactFile(filePath, expected, label) {
  let details;
  try {
    details = await stat(filePath);
  } catch (error) {
    if (error.code === "ENOENT") {
      throw new Error(`${label} is missing at ${filePath}`);
    }
    throw error;
  }
  invariant(details.isFile(), `${label} is not a file at ${filePath}`);
  invariant(
    details.size === expected.bytes,
    `${label} byte length mismatch: expected ${expected.bytes}, observed ${details.size}`,
  );
  const sha256 = await sha256File(filePath);
  invariant(
    sha256 === expected.sha256,
    `${label} SHA-256 mismatch: expected ${expected.sha256}, observed ${sha256}`,
  );
  return { bytes: details.size, sha256 };
}

export async function loadChromeForTestingArtifact(root, { requireArchive = false } = {}) {
  const manifestPath = path.join(root, CANONICAL_CHROME_FOR_TESTING_MANIFEST);
  const manifestBytes = await readFile(manifestPath);
  const manifest = validateChromeForTestingManifest(JSON.parse(manifestBytes));
  const cacheRoot = path.join(
    root,
    ".work",
    "toolchain",
    "chrome-for-testing",
    manifest.version,
    manifest.platform,
  );
  const archivePath = path.resolve(cacheRoot, manifest.source.archive.relativePath);
  const executablePath = path.resolve(cacheRoot, manifest.executable.relativePath);
  for (const [candidate, label] of [
    [archivePath, "Chrome archive"],
    [executablePath, "Chrome executable"],
  ]) {
    assert.equal(
      path.relative(cacheRoot, candidate).startsWith(".."),
      false,
      `${label} resolved outside the cache root`,
    );
  }
  const executableIdentity = await verifyArtifactFile(
    executablePath,
    manifest.executable,
    "canonical Chrome for Testing executable",
  );
  const archiveIdentity = requireArchive
    ? await verifyArtifactFile(
        archivePath,
        manifest.source.archive,
        "canonical Chrome for Testing archive",
      )
    : {
        bytes: manifest.source.archive.bytes,
        sha256: manifest.source.archive.sha256,
      };
  return {
    manifest,
    manifestPath,
    manifestIdentity: {
      bytes: manifestBytes.byteLength,
      sha256: crypto.createHash("sha256").update(manifestBytes).digest("hex"),
    },
    cacheRoot,
    archivePath,
    archiveIdentity,
    executablePath,
    executableIdentity,
  };
}

export function chromeForTestingCachePaths(root, manifest) {
  validateChromeForTestingManifest(manifest);
  const cacheRoot = path.join(
    root,
    ".work",
    "toolchain",
    "chrome-for-testing",
    manifest.version,
    manifest.platform,
  );
  return {
    cacheRoot,
    archivePath: path.resolve(cacheRoot, manifest.source.archive.relativePath),
    executablePath: path.resolve(cacheRoot, manifest.executable.relativePath),
  };
}
