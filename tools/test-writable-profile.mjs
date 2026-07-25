import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

import {
  acquireWritableProfileLease,
  commitWritableProfile,
  handleWritableProfileHttp,
  openWritableProfile,
  readWritableProfile,
  snapshotWritableProfile,
} from "./writable-profile.mjs";

const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "vine-writable-profile-"));
let testCompleted = false;
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
  const directLease = acquireWritableProfileLease(profile, "session-a");
  assert.deepEqual(snapshotWritableProfile(profile).files[0], {
    objectId: 1,
    relativePathA: [...Buffer.from("BGI.gdb", "ascii")],
    exists: false,
    generation: 0,
    byteLength: 0,
    sha256: null,
  });

  const firstBytes = Buffer.from("SDC FORMAT 1.00", "ascii");
  const firstRequest = request("session-a:1", 0, firstBytes, directLease);
  assert.deepEqual(commitWritableProfile(profile, firstRequest, firstBytes), {
    objectId: 1,
    baseGeneration: 0,
    committedGeneration: 1,
    win32Error: 0,
    duplicate: false,
  });
  assert.equal(commitWritableProfile(profile, firstRequest, firstBytes).duplicate, true);
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        request("session-a:1", 0, Buffer.from("other"), directLease),
        Buffer.from("other"),
      ),
    { code: "writable-profile-request-key-conflict" },
  );
  assert.throws(
    () => commitWritableProfile(profile, request("session-a:2", 0, firstBytes, directLease), firstBytes),
    { code: "writable-profile-generation-stale" },
  );
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        {
          ...request("session-a:3", 1, firstBytes, directLease),
          titleFingerprint: "0".repeat(64),
        },
        firstBytes,
      ),
    { code: "writable-profile-commit-profile-mismatch" },
  );
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        { ...request("session-a:4", 1, firstBytes, directLease), sha256: "0".repeat(64) },
        firstBytes,
      ),
    { code: "writable-profile-commit-content-mismatch" },
  );

  const rollbackBytes = Buffer.from("rollback", "ascii");
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        request("session-a:5", 1, rollbackBytes, directLease),
        rollbackBytes,
        {
        failpoint: "after-write-before-commit",
        },
      ),
    { code: "writable-profile-failpoint" },
  );
  assert.equal(snapshotWritableProfile(profile).files[0].generation, 1);

  const finalBytes = Buffer.from("new durable state", "ascii");
  const lostRequest = request("session-a:6", 1, finalBytes, directLease);
  assert.throws(
    () =>
      commitWritableProfile(profile, lostRequest, finalBytes, {
        failpoint: "after-commit-response-lost",
      }),
    { code: "writable-profile-response-lost" },
  );
  profile.close();

  const legacyDatabase = new DatabaseSync(configuration.databasePath);
  try {
    legacyDatabase.exec(`
      UPDATE profile_metadata SET schema_version = 1 WHERE singleton = 1;
      DROP TABLE profile_lease;
    `);
  } finally {
    legacyDatabase.close();
  }
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
  assert.equal(
    profile.database
      .prepare("SELECT schema_version FROM profile_metadata WHERE singleton = 1")
      .get().schema_version,
    2,
  );
  const migratedLeaseState = profile.database
    .prepare("SELECT epoch, session_key, active FROM profile_lease WHERE singleton = 1")
    .get();
  assert.equal(migratedLeaseState.epoch, 0);
  assert.equal(migratedLeaseState.session_key, null);
  assert.equal(migratedLeaseState.active, 0);
  const restartedLease = acquireWritableProfileLease(profile, "session-a-restarted");
  const restarted = snapshotWritableProfile(profile).files[0];
  assert.equal(restarted.generation, 2);
  assert.deepEqual(
    readWritableProfile(profile, {
      sourceId: 0,
      objectId: 1,
      offset: 0n,
      requestedBytes: finalBytes.length,
    }).bytes,
    finalBytes,
  );
  assert.equal(
    commitWritableProfile(
      profile,
      {
        ...lostRequest,
        sessionKey: restartedLease.sessionKey,
        sessionEpoch: restartedLease.epoch,
      },
      finalBytes,
    ).duplicate,
    true,
  );
  assert.throws(
    () =>
      commitWritableProfile(
        profile,
        request("session-a:7", 2, Buffer.alloc(33), restartedLease),
        Buffer.alloc(33),
      ),
    { code: "writable-profile-integer-invalid" },
  );

  const server = http.createServer(async (incoming, response) => {
    if (!(await handleWritableProfileHttp(incoming, response, profile))) {
      response.writeHead(404).end();
    }
  });
  const port = await listen(server);
  const baseUrl = `http://127.0.0.1:${port}/api/profile/v2`;
  try {
    assert.equal(
      (await fetch(`http://127.0.0.1:${port}/api/profile/v1/snapshot`)).status,
      404,
    );
    assert.equal((await fetch(`${baseUrl}/snapshot`)).status, 400);
    const leaseResponse = await fetch(`${baseUrl}/sessions`, {
      method: "POST",
      headers: { "X-Vine-Session-Key": "http-session" },
    });
    assert.equal(leaseResponse.status, 201);
    const httpEpoch = leaseResponse.headers.get("x-vine-session-epoch");
    assert.match(httpEpoch, /^[1-9][0-9]*$/u);
    const leaseHeaders = {
      "X-Vine-Session-Key": "http-session",
      "X-Vine-Session-Epoch": httpEpoch,
    };
    const snapshotResponse = await fetch(`${baseUrl}/snapshot`, { headers: leaseHeaders });
    assert.equal(snapshotResponse.status, 200);
    assert.deepEqual(await snapshotResponse.json(), snapshotWritableProfile(profile));
    const rangeResponse = await fetch(
      `${baseUrl}/titles/${configuration.titleFingerprint}/profiles/default/objects/1` +
        `?generation=2&offset=4&length=6`,
      { headers: leaseHeaders },
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
      ...leaseHeaders,
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
    const staleBytes = Buffer.from("stale generation", "ascii");
    const staleResponse = await fetch(commitUrl, {
      method: "POST",
      headers: {
        ...commitHeaders,
        "X-Vine-Request-Key": "http-session:stale",
        "X-Vine-Base-Generation": "2",
        "X-Vine-Byte-Length": String(staleBytes.length),
        "X-Vine-Content-Sha256": crypto.createHash("sha256").update(staleBytes).digest("hex"),
      },
      body: staleBytes,
    });
    assert.equal(staleResponse.status, 200);
    assert.equal(decodeCommitResponse(await staleResponse.arrayBuffer()).win32Error, 29);
    assert.equal(snapshotWritableProfile(profile).files[0].generation, 3);

    const revokeResponse = await fetch(`${baseUrl}/sessions/${httpEpoch}`, {
      method: "DELETE",
      headers: { "X-Vine-Session-Key": "http-session" },
    });
    assert.equal(revokeResponse.status, 204);
    assert.equal(revokeResponse.headers.get("x-vine-session-revoked"), "true");
    const lateBytes = Buffer.from("late old session", "ascii");
    const lateResponse = await fetch(commitUrl, {
      method: "POST",
      headers: {
        ...commitHeaders,
        "X-Vine-Request-Key": "http-session:late",
        "X-Vine-Base-Generation": "3",
        "X-Vine-Byte-Length": String(lateBytes.length),
        "X-Vine-Content-Sha256": crypto.createHash("sha256").update(lateBytes).digest("hex"),
      },
      body: lateBytes,
    });
    assert.equal(lateResponse.status, 200);
    assert.equal(decodeCommitResponse(await lateResponse.arrayBuffer()).win32Error, 995);
    assert.equal(snapshotWritableProfile(profile).files[0].generation, 3);

    const nextLeaseResponse = await fetch(`${baseUrl}/sessions`, {
      method: "POST",
      headers: { "X-Vine-Session-Key": "next-session" },
    });
    assert.equal(nextLeaseResponse.status, 201);
    const nextEpoch = nextLeaseResponse.headers.get("x-vine-session-epoch");
    assert.notEqual(nextEpoch, httpEpoch);
    const staleAfterNext = await fetch(commitUrl, {
      method: "POST",
      headers: {
        ...commitHeaders,
        "X-Vine-Request-Key": "http-session:after-next",
        "X-Vine-Base-Generation": "3",
        "X-Vine-Byte-Length": String(lateBytes.length),
        "X-Vine-Content-Sha256": crypto.createHash("sha256").update(lateBytes).digest("hex"),
      },
      body: lateBytes,
    });
    assert.equal(staleAfterNext.status, 200);
    assert.equal(decodeCommitResponse(await staleAfterNext.arrayBuffer()).win32Error, 995);
    assert.equal(snapshotWritableProfile(profile).files[0].generation, 3);
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
  assert.deepEqual(
    readWritableProfile(profile, {
      sourceId: 0,
      objectId: 1,
      offset: 0n,
      requestedBytes: httpRestarted.byteLength,
    }).bytes,
    Buffer.from("http commit", "ascii"),
  );
  profile.close();

  const failureProfile = openWritableProfile({
    ...configuration,
    databasePath: path.join(temporaryDirectory, "failure-profile.sqlite"),
    files: [
      { objectId: 1, relativePathA: Buffer.from("BGI.gdb", "ascii") },
      {
        objectId: 2,
        relativePathA: Buffer.from("settings.dat", "ascii"),
        seedBytes: Buffer.alloc(16, 0x5a),
      },
    ],
  });
  const failureServer = http.createServer(async (incoming, response) => {
    if (!(await handleWritableProfileHttp(incoming, response, failureProfile))) {
      response.writeHead(404).end();
    }
  });
  const failurePort = await listen(failureServer);
  const failureBase = `http://127.0.0.1:${failurePort}/api/profile/v2`;
  try {
    const leaseResponse = await fetch(`${failureBase}/sessions`, {
      method: "POST",
      headers: { "X-Vine-Session-Key": "failure-session" },
    });
    const epoch = leaseResponse.headers.get("x-vine-session-epoch");
    assert.equal(leaseResponse.status, 201);
    const commitUrl =
      `${failureBase}/titles/${configuration.titleFingerprint}/profiles/default/objects/1/commits`;
    const quotaBytes = Buffer.alloc(17, 0x71);
    const quotaResponse = await fetch(commitUrl, {
      method: "POST",
      headers: commitHeadersFor("failure-session:quota", 0, quotaBytes, epoch),
      body: quotaBytes,
    });
    assert.equal(quotaResponse.status, 200);
    assert.equal(decodeCommitResponse(await quotaResponse.arrayBuffer()).win32Error, 112);
    assert.equal(snapshotWritableProfile(failureProfile).files[0].generation, 0);

    const lock = new DatabaseSync(failureProfile.databasePath);
    try {
      lock.exec("BEGIN IMMEDIATE");
      const lockedBytes = Buffer.from("retryable", "ascii");
      const lockedResponse = await fetch(commitUrl, {
        method: "POST",
        headers: commitHeadersFor("failure-session:locked", 0, lockedBytes, epoch),
        body: lockedBytes,
      });
      assert.equal(lockedResponse.status, 200);
      assert.equal(decodeCommitResponse(await lockedResponse.arrayBuffer()).win32Error, 33);
      assert.equal(snapshotWritableProfile(failureProfile).files[0].generation, 0);
      lock.exec("ROLLBACK");
    } finally {
      lock.close();
    }
  } finally {
    await close(failureServer);
    failureProfile.close();
  }

  const largeBytes = Buffer.alloc(4 * 1024 * 1024, 0xff);
  const largeProfile = openWritableProfile({
    ...configuration,
    databasePath: path.join(temporaryDirectory, "large-profile.sqlite"),
    quotaBytes: 16 * 1024 * 1024,
    files: [
      {
        objectId: 1,
        relativePathA: Buffer.from("BGI.gdb", "ascii"),
        seedBytes: largeBytes,
      },
    ],
  });
  try {
    const largeSnapshot = snapshotWritableProfile(largeProfile);
    const serialized = Buffer.from(JSON.stringify(largeSnapshot));
    assert(serialized.length < 4096);
    assert.equal(Object.hasOwn(largeSnapshot.files[0], "bytes"), false);
    const lastChunk = readWritableProfile(largeProfile, {
      sourceId: 0,
      objectId: 1,
      offset: BigInt(largeBytes.length - 1024 * 1024),
      requestedBytes: 1024 * 1024,
    });
    assert.deepEqual(lastChunk.bytes, largeBytes.subarray(largeBytes.length - 1024 * 1024));
  } finally {
    largeProfile.close();
  }
  testCompleted = true;
  console.log("writable profile SQLite tests passed");
} finally {
  try {
    fs.rmSync(temporaryDirectory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  } catch (cleanupError) {
    if (testCompleted) {
      throw cleanupError;
    }
    process.stderr.write(
      `writable profile test cleanup deferred for ${temporaryDirectory}: ${String(cleanupError)}\n`,
    );
  }
}

function request(requestKey, baseGeneration, bytes, lease) {
  return {
    requestKey,
    titleFingerprint: configuration.titleFingerprint,
    profileId: configuration.profileId,
    objectId: 1,
    relativePathA: Buffer.from("BGI.gdb", "ascii"),
    baseGeneration,
    byteLength: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    sessionKey: lease.sessionKey,
    sessionEpoch: lease.epoch,
  };
}

function commitHeadersFor(requestKey, baseGeneration, bytes, sessionEpoch) {
  return {
    "Content-Type": "application/octet-stream",
    "X-Vine-Request-Key": requestKey,
    "X-Vine-Base-Generation": String(baseGeneration),
    "X-Vine-Byte-Length": String(bytes.length),
    "X-Vine-Content-Sha256": crypto.createHash("sha256").update(bytes).digest("hex"),
    "X-Vine-Session-Key": "failure-session",
    "X-Vine-Session-Epoch": sessionEpoch,
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
