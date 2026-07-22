import assert from "node:assert/strict";
import fs from "node:fs";
import { mkdir, readFile, rename, rm } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  CANONICAL_CHROME_FOR_TESTING_MANIFEST,
  chromeForTestingCachePaths,
  loadChromeForTestingArtifact,
  validateChromeForTestingManifest,
  verifyArtifactFile,
} from "./chrome-for-testing.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(root, CANONICAL_CHROME_FOR_TESTING_MANIFEST);
const manifest = validateChromeForTestingManifest(JSON.parse(await readFile(manifestPath)));
const paths = chromeForTestingCachePaths(root, manifest);

async function downloadArchive() {
  await mkdir(paths.cacheRoot, { recursive: true });
  try {
    await verifyArtifactFile(
      paths.archivePath,
      manifest.source.archive,
      "cached Chrome for Testing archive",
    );
    return;
  } catch (error) {
    if (!/ is missing at /u.test(error.message)) {
      throw new Error(
        `${error.message}; remove only the reported corrupt toolchain cache before retrying`,
      );
    }
  }

  const partialPath = `${paths.archivePath}.partial-${process.pid}`;
  await rm(partialPath, { force: true });
  try {
    const response = await fetch(manifest.source.url, { redirect: "error" });
    assert(response.ok, `Chrome for Testing download failed with HTTP ${response.status}`);
    assert(response.body !== null, "Chrome for Testing response body is missing");
    const contentLength = Number(response.headers.get("content-length"));
    assert.equal(
      contentLength,
      manifest.source.archive.bytes,
      "Chrome for Testing response length differs from the pinned manifest",
    );
    await pipeline(Readable.fromWeb(response.body), fs.createWriteStream(partialPath, { flags: "wx" }));
    await verifyArtifactFile(partialPath, manifest.source.archive, "downloaded Chrome for Testing archive");
    await rename(partialPath, paths.archivePath);
  } finally {
    await rm(partialPath, { force: true });
  }
}

function extractArchive() {
  const sevenZip = spawnSync(
    process.platform === "win32" ? "7z.exe" : "7z",
    ["x", "-y", `-o${paths.cacheRoot}`, paths.archivePath],
    { cwd: root, encoding: "utf8", stdio: "pipe", windowsHide: true },
  );
  if (sevenZip.error === undefined && sevenZip.status === 0) {
    return;
  }
  if (process.platform !== "win32") {
    throw sevenZip.error ?? new Error(`7z extraction failed\n${sevenZip.stdout}${sevenZip.stderr}`);
  }
  const powershell = spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "Expand-Archive -LiteralPath $env:VINE_CHROME_ARCHIVE -DestinationPath $env:VINE_CHROME_DESTINATION -Force",
    ],
    {
      cwd: root,
      env: {
        ...process.env,
        VINE_CHROME_ARCHIVE: paths.archivePath,
        VINE_CHROME_DESTINATION: paths.cacheRoot,
      },
      encoding: "utf8",
      stdio: "pipe",
      windowsHide: true,
    },
  );
  if (powershell.error !== undefined || powershell.status !== 0) {
    throw (
      powershell.error ??
      new Error(`Chrome for Testing extraction failed\n${powershell.stdout}${powershell.stderr}`)
    );
  }
}

assert.equal(process.platform, "win32", "canonical private performance Chrome is win64-only");
assert.equal(process.arch, "x64", "canonical private performance Chrome requires Windows x64");
await downloadArchive();
try {
  await verifyArtifactFile(
    paths.executablePath,
    manifest.executable,
    "cached Chrome for Testing executable",
  );
} catch {
  extractArchive();
}
const artifact = await loadChromeForTestingArtifact(root, { requireArchive: true });
console.log(
  `Chrome for Testing ready: ${path.relative(root, artifact.executablePath)}\n` +
    `${artifact.manifest.version}; executable sha256 ${artifact.executableIdentity.sha256}; ` +
    `archive sha256 ${artifact.archiveIdentity.sha256}`,
);
