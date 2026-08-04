import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "runtime-worker-storage-contract");
const output = path.join(outputDirectory, "profile-storage.mjs");
fs.mkdirSync(outputDirectory, { recursive: true });
await build({
  entryPoints: [path.join(root, "browser", "runtime-worker", "profile-storage.ts")],
  bundle: true,
  format: "esm",
  outfile: output,
  platform: "node",
  target: "node22",
});
const {
  recoverableCommitHttpError,
  recoverableCommitStorageError,
  writableCommitFailureControl,
  writableCommitSuccessControl,
  writableDirectoryCreateFailureControl,
} = await import(`${pathToFileURL(output).href}?v=${Date.now()}`);

for (const [status, errorCode, win32Error] of [
  [409, "writable-profile-generation-stale", 29],
  [409, "writable-profile-directory-generation-stale", 29],
  [413, "writable-profile-quota-exceeded", 112],
  [423, null, 33],
  [429, null, 33],
  [500, "ERR_SQLITE_ERROR", 29],
  [503, "SQLITE_IOERR", 29],
  [400, "writable-profile-lease-revoked", 995],
]) {
  assert.equal(
    recoverableCommitHttpError(status, errorCode),
    win32Error,
  );
}
for (const status of [200, 400, 401, 404, 409, 413, 422, 500, 503, 599, 600]) {
  assert.equal(recoverableCommitHttpError(status, null), null);
}
for (const errorCode of [
  "writable-profile-request-key-conflict",
  "writable-profile-commit-content-mismatch",
  "writable-profile-object-unknown",
  "writable-profile-header-invalid",
]) {
  assert.equal(recoverableCommitHttpError(409, errorCode), null);
}

for (const [errorCode, win32Error] of [
  ["writable-profile-generation-stale", 29],
  ["writable-profile-directory-generation-stale", 29],
  ["writable-profile-quota-exceeded", 112],
  ["SQLITE_FULL", 112],
  ["SQLITE_BUSY", 33],
  ["writable-profile-lease-revoked", 995],
  ["browser-storage-failure", 29],
]) {
  assert.equal(recoverableCommitStorageError(errorCode), win32Error);
}
for (const errorCode of [
  "writable-profile-request-key-conflict",
  "writable-profile-commit-content-mismatch",
  "writable-profile-object-unknown",
]) {
  assert.equal(recoverableCommitStorageError(errorCode), null);
}

const control = writableCommitFailureControl(7, 0x0123_4567_89ab_cdefn, 112);
assert.equal(control.byteLength, 32);
const view = new DataView(control.buffer, control.byteOffset, control.byteLength);
assert.deepEqual(
  Array.from({ length: 8 }, (_, index) => view.getUint32(index * 4, true)),
  [1, 7, 0x89ab_cdef, 0x0123_4567, 0x89ab_cdef, 0x0123_4567, 112, 0],
);
assert.deepEqual(
  writableDirectoryCreateFailureControl(9, 5n, 29),
  writableCommitFailureControl(9, 5n, 29),
);
const success = writableCommitSuccessControl(
  11,
  0x0123_4567_89ab_cdefn,
  0x0123_4567_89ab_cdf0n,
);
const successView = new DataView(
  success.buffer,
  success.byteOffset,
  success.byteLength,
);
assert.deepEqual(
  Array.from({ length: 8 }, (_, index) =>
    successView.getUint32(index * 4, true),
  ),
  [1, 11, 0x89ab_cdef, 0x0123_4567, 0x89ab_cdf0, 0x0123_4567, 0, 0],
);

const worker = fs.readFileSync(
  path.join(root, "browser", "runtime-worker", "runtime-worker.ts"),
  "utf8",
);
assert.match(
  worker,
  /response === null\)[\s\S]{0,160}unknown publication outcome/u,
);
assert.match(worker, /!Object\.hasOwn\(file, "bytes"\)/u);
assert.match(worker, /new BrowserStorageClient\(\)/u);
assert.match(worker, /sourceSnapshotManifest\(browserNativeStorage\.sourceMount\)/u);
assert.match(worker, /browserStorage\.sourceRead\(/u);
assert.match(worker, /browserStorage\.profileSnapshot\(/u);
assert.match(worker, /browserStorage\.profileCommit\(/u);
assert.match(worker, /browserStorage\.createProfileDirectory\(/u);
assert.match(worker, /await browserStorage\.close\(\)/u);
const cancellation = methodSource(worker, "#cancelAndFinish(", "#abortSourceReads(");
assertOrdered(cancellation, [
  "await this.#settleProfileLeaseAcquisition()",
  "await this.#drainProfileTransports()",
  "await this.#drainWritableMutations()",
  "await this.#revokeWritableProfileLease()",
  "this.#bridge.cancel<CancellationMetadata>()",
  "this.#finishNow({",
]);
const completion = methodSource(
  worker,
  "#finish(",
  "async #settleProfileLeaseAcquisition(",
);
assertOrdered(completion, [
  "await this.#drainProfileTransports()",
  "await this.#drainWritableMutations()",
  "await this.#revokeWritableProfileLease()",
  "this.#finishNow(final)",
]);

console.log("Runtime Worker storage contract passed");

function methodSource(source, method, nextMethod) {
  const start = source.indexOf(`\n  ${method}`);
  const end = source.indexOf(`\n  ${nextMethod}`, start + 1);
  assert(start >= 0 && end > start, `could not isolate ${method}`);
  return source.slice(start, end);
}

function assertOrdered(source, fragments) {
  let previous = -1;
  for (const fragment of fragments) {
    const index = source.indexOf(fragment);
    assert(index > previous, `${fragment} is missing or out of order`);
    previous = index;
  }
}
