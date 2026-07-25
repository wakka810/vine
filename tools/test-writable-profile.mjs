import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

import {
  commitWritableProfile,
  handleWritableProfileHttp,
  openWritableProfile,
  snapshotWritableProfile,
} from "./writable-profile.mjs";

const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "vine-writable-profile-"));
const databasePath = path.join(temporaryDirectory, "profile.sqlite");
const configuration = {
  databasePath,
  titleFingerprint: "f".repeat(64),
  profileId: "default",
  rootPathA: Buffer.from("C:\\Vine\0", "binary"),
  quotaBytes: 32,
  files: [{ objectId: 1, relativePathA: Buffer.from("BGI.gdb", "ascii") }],
};

try {
  let profile = openWritableProfile(configuration);
  assert.deepEqual(snapshotWritableProfile(profile).files[0], {
    objectId: 1,
    relativePathA: [...Buffer.from("BGI.gdb", "ascii")],
    exists: false,
    generation: 0,
    byteLength: 0,
    sha256: null,
    bytes: [],
  });

  const firstBytes = Buffer.from("SDC FORMAT 1.00", "ascii");
  const firstRequest = request("session-a:1", 0, firstBytes);
  assert.deepEqual(commitWritableProfile(profile, firstRequest, firstBytes), {
    objectId: 1,
    baseGeneration: 0,
    committedGeneration: 1,
    win32Error: 0,
    duplicate: false,
  });
  assert.equal(commitWritableProfile(profile, firstRequest, firstBytes).duplicate, true);
  assert.throws(
    () => commitWritableProfile(profile, request("session-a:1", 0, Buffer.from("other")), Buffer.from("other")),
    { code: "writable-profile-request-key-conflict" },
  );
  assert.throws(
    () => commitWritableProfile(profile, request("session-a:2", 0, firstBytes), firstBytes),
    { code: "writable-profile-generation-stale" },
  );
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        { ...request("session-a:3", 1, firstBytes), titleFingerprint: "0".repeat(64) },
        firstBytes,
      ),
    { code: "writable-profile-commit-profile-mismatch" },
  );
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        { ...request("session-a:4", 1, firstBytes), sha256: "0".repeat(64) },
        firstBytes,
      ),
    { code: "writable-profile-commit-content-mismatch" },
  );

  const rollbackBytes = Buffer.from("rollback", "ascii");
  assert.throws(
    () =>
      commitWritableProfile(profile, request("session-a:5", 1, rollbackBytes), rollbackBytes, {
        failpoint: "after-write-before-commit",
      }),
    { code: "writable-profile-failpoint" },
  );
  assert.equal(snapshotWritableProfile(profile).files[0].generation, 1);

  const finalBytes = Buffer.from("new durable state", "ascii");
  const lostRequest = request("session-a:6", 1, finalBytes);
  assert.throws(
    () =>
      commitWritableProfile(profile, lostRequest, finalBytes, {
        failpoint: "after-commit-response-lost",
      }),
    { code: "writable-profile-response-lost" },
  );
  profile.close();

  profile = openWritableProfile({
    ...configuration,
    files: [
      {
        objectId: 1,
        relativePathA: Buffer.from("BGI.gdb", "ascii"),
        seedBytes: Buffer.from("ignored after initialization", "ascii"),
      },
    ],
  });
  const restarted = snapshotWritableProfile(profile).files[0];
  assert.equal(restarted.generation, 2);
  assert.deepEqual(Buffer.from(restarted.bytes), finalBytes);
  assert.equal(commitWritableProfile(profile, lostRequest, finalBytes).duplicate, true);
  assert.throws(
    () => commitWritableProfile(profile, request("session-a:7", 2, Buffer.alloc(33)), Buffer.alloc(33)),
    { code: "writable-profile-integer-invalid" },
  );

  const server = http.createServer(async (incoming, response) => {
    if (!(await handleWritableProfileHttp(incoming, response, profile))) {
      response.writeHead(404).end();
    }
  });
  const port = await listen(server);
  const baseUrl = `http://127.0.0.1:${port}/api/profile/v1`;
  try {
    const snapshotResponse = await fetch(`${baseUrl}/snapshot`);
    assert.equal(snapshotResponse.status, 200);
    assert.deepEqual(await snapshotResponse.json(), snapshotWritableProfile(profile));
    const rangeResponse = await fetch(
      `${baseUrl}/titles/${configuration.titleFingerprint}/profiles/default/objects/1` +
        `?generation=2&offset=4&length=6`,
    );
    assert.equal(rangeResponse.status, 206);
    assert.deepEqual(Buffer.from(await rangeResponse.arrayBuffer()), finalBytes.subarray(4, 10));

    const httpBytes = Buffer.from("http commit", "ascii");
    const httpHash = crypto.createHash("sha256").update(httpBytes).digest("hex");
    const commitUrl =
      `${baseUrl}/titles/${configuration.titleFingerprint}/profiles/default/objects/1/commits`;
    const commitHeaders = {
      "Content-Type": "application/octet-stream",
      "X-Vine-Request-Key": "http-session:1",
      "X-Vine-Base-Generation": "2",
      "X-Vine-Byte-Length": String(httpBytes.length),
      "X-Vine-Content-Sha256": httpHash,
    };
    const commitResponse = await fetch(commitUrl, {
      method: "POST",
      headers: commitHeaders,
      body: httpBytes,
    });
    assert.equal(commitResponse.status, 200);
    assert.deepEqual(decodeCommitResponse(await commitResponse.arrayBuffer()), {
      version: 1,
      objectId: 1,
      baseGeneration: 2,
      committedGeneration: 3,
      win32Error: 0,
      reserved: 0,
    });
    assert.equal(commitResponse.headers.get("x-vine-duplicate"), "false");
    const duplicateResponse = await fetch(commitUrl, {
      method: "POST",
      headers: commitHeaders,
      body: httpBytes,
    });
    assert.equal(duplicateResponse.status, 200);
    assert.equal(duplicateResponse.headers.get("x-vine-duplicate"), "true");
  } finally {
    await close(server);
  }
  profile.close();

  assert.throws(
    () => openWritableProfile({ ...configuration, quotaBytes: 31 }),
    { code: "writable-profile-database-identity-mismatch" },
  );
  profile = openWritableProfile(configuration);
  const httpRestarted = snapshotWritableProfile(profile).files[0];
  assert.equal(httpRestarted.generation, 3);
  assert.deepEqual(Buffer.from(httpRestarted.bytes), Buffer.from("http commit", "ascii"));
  profile.close();
  console.log("writable profile SQLite tests passed");
} finally {
  fs.rmSync(temporaryDirectory, { recursive: true, force: true });
}

function request(requestKey, baseGeneration, bytes) {
  return {
    requestKey,
    titleFingerprint: configuration.titleFingerprint,
    profileId: configuration.profileId,
    objectId: 1,
    relativePathA: Buffer.from("BGI.gdb", "ascii"),
    baseGeneration,
    byteLength: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

function listen(server) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve(server.address().port));
  });
}

function close(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function decodeCommitResponse(buffer) {
  const bytes = Buffer.from(buffer);
  assert.equal(bytes.length, 32);
  const words = Array.from({ length: 8 }, (_, index) => bytes.readUInt32LE(index * 4));
  return {
    version: words[0],
    objectId: words[1],
    baseGeneration: words[2] + words[3] * 0x1_0000_0000,
    committedGeneration: words[4] + words[5] * 0x1_0000_0000,
    win32Error: words[6],
    reserved: words[7],
  };
}
