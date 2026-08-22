import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";

export const writableProfileSchema = "vine.writable-profile-snapshot";
export const writableProfileSchemaVersion = 3;
export const writableProfileReadService = "storage.profile-read.v1";
export const writableProfileCommitService = "storage.profile-commit.v1";
export const writableProfileDeleteService = "storage.profile-delete.v1";
export const writableProfileDirectoryCreateService = "storage.profile-directory-create.v1";
export const writableProfileMaximumBytes = 16 * 1024 * 1024;
export const writableProfileMaximumIoBytes = 1024 * 1024;

const databaseSchemaVersion = 3;
const maximumGeneration = Number.MAX_SAFE_INTEGER;
const errorWriteFault = 29;
const errorLockViolation = 33;
const errorDiskFull = 112;
const errorOperationAborted = 995;
export const writableProfileDeleteMarkerSha256 = "0".repeat(64);

export function openWritableProfile({
  databasePath,
  titleFingerprint,
  profileId,
  rootPathA,
  quotaBytes,
  files,
  directories = [],
}) {
  assertText(titleFingerprint, "title fingerprint", 128);
  assertText(profileId, "profile ID", 128);
  if (
    !/^[a-zA-Z0-9:._-]+$/u.test(titleFingerprint) ||
    !/^[a-zA-Z0-9._-]+$/u.test(profileId)
  ) {
    throw profileError(
      "writable-profile-identity-invalid",
      "title fingerprint or profile ID contains an unsupported character",
    );
  }
  const root = boundedBytes(rootPathA, "profile root", 32 * 1024);
  if (root.length < 4 || root.at(-1) !== 0 || root.subarray(0, -1).includes(0)) {
    throw profileError(
      "writable-profile-root-invalid",
      "profile root must be one NUL-terminated Windows path",
    );
  }
  assertInteger(quotaBytes, "profile quota", 1, writableProfileMaximumBytes);
  if (!Array.isArray(files) || files.length === 0 || files.length > 64) {
    throw profileError("writable-profile-files-invalid", "profile requires 1..64 writable paths");
  }
  const normalizedFiles = files.map((file) => normalizeConfiguredFile(file));
  const objectIds = new Set(normalizedFiles.map((file) => file.objectId));
  const paths = new Set(normalizedFiles.map((file) => windowsPathIdentity(file.relativePathA)));
  if (objectIds.size !== normalizedFiles.length || paths.size !== normalizedFiles.length) {
    throw profileError(
      "writable-profile-files-duplicate",
      "profile writable object or path identity is duplicated",
    );
  }
  if (!Array.isArray(directories) || directories.length > 64) {
    throw profileError(
      "writable-profile-directories-invalid",
      "profile permits at most 64 writable directory paths",
    );
  }
  const normalizedDirectories = directories.map((directory) =>
    normalizeConfiguredDirectory(directory)
  );
  const directoryIds = new Set(
    normalizedDirectories.map((directory) => directory.directoryId),
  );
  const directoryPaths = new Set(
    normalizedDirectories.map((directory) => windowsPathIdentity(directory.relativePathA)),
  );
  if (
    directoryIds.size !== normalizedDirectories.length ||
    directoryPaths.size !== normalizedDirectories.length ||
    normalizedDirectories.some((directory) =>
      paths.has(windowsPathIdentity(directory.relativePathA))
    )
  ) {
    throw profileError(
      "writable-profile-directories-duplicate",
      "profile writable directory identity is duplicated or conflicts with a file path",
    );
  }

  const configuration = {
    titleFingerprint,
    profileId,
    rootPathA: root,
    quotaBytes,
    files: normalizedFiles,
    directories: normalizedDirectories,
  };
  fs.mkdirSync(path.dirname(databasePath), { recursive: true });
  const database = new DatabaseSync(databasePath);
  try {
    database.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;");
    database.exec(`
      CREATE TABLE IF NOT EXISTS profile_metadata (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        schema_version INTEGER NOT NULL,
        title_fingerprint TEXT NOT NULL,
        profile_id TEXT NOT NULL,
        root_path_a BLOB NOT NULL,
        quota_bytes INTEGER NOT NULL
      ) STRICT;
      CREATE TABLE IF NOT EXISTS writable_paths (
        object_id INTEGER PRIMARY KEY,
        relative_path_a BLOB NOT NULL UNIQUE
      ) STRICT;
      CREATE TABLE IF NOT EXISTS files (
        object_id INTEGER PRIMARY KEY REFERENCES writable_paths(object_id),
        generation INTEGER NOT NULL CHECK (generation > 0),
        byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
        sha256 TEXT NOT NULL,
        content BLOB NOT NULL
      ) STRICT;
      CREATE TABLE IF NOT EXISTS commits (
        request_key TEXT PRIMARY KEY,
        object_id INTEGER NOT NULL REFERENCES writable_paths(object_id),
        base_generation INTEGER NOT NULL,
        committed_generation INTEGER NOT NULL,
        byte_length INTEGER NOT NULL,
        sha256 TEXT NOT NULL
      ) STRICT;
      CREATE TABLE IF NOT EXISTS writable_directory_paths (
        directory_id INTEGER PRIMARY KEY,
        relative_path_a BLOB NOT NULL UNIQUE
      ) STRICT;
      CREATE TABLE IF NOT EXISTS directories (
        directory_id INTEGER PRIMARY KEY REFERENCES writable_directory_paths(directory_id),
        generation INTEGER NOT NULL CHECK (generation > 0)
      ) STRICT;
      CREATE TABLE IF NOT EXISTS directory_commits (
        request_key TEXT PRIMARY KEY,
        directory_id INTEGER NOT NULL REFERENCES writable_directory_paths(directory_id),
        base_generation INTEGER NOT NULL,
        committed_generation INTEGER NOT NULL
      ) STRICT;
      CREATE TABLE IF NOT EXISTS profile_lease (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        epoch INTEGER NOT NULL CHECK (epoch >= 0),
        session_key TEXT,
        active INTEGER NOT NULL CHECK (active IN (0, 1)),
        CHECK (
          (active = 0 AND session_key IS NULL) OR
          (active = 1 AND session_key IS NOT NULL)
        )
      ) STRICT;
      INSERT OR IGNORE INTO profile_lease(singleton, epoch, session_key, active)
      VALUES (1, 0, NULL, 0);
    `);
    initializeProfile(database, configuration);
  } catch (error) {
    database.close();
    throw error;
  }
  return {
    database,
    databasePath: path.resolve(databasePath),
    ...configuration,
    close() {
      database.close();
    },
  };
}

export function snapshotWritableProfile(profile) {
  const stored = new Map(
    profile.database
      .prepare("SELECT object_id, generation, byte_length, sha256, content FROM files")
      .all()
      .map((file) => [file.object_id, file]),
  );
  const storedDirectories = new Map(
    profile.database
      .prepare("SELECT directory_id, generation FROM directories")
      .all()
      .map((directory) => [directory.directory_id, directory]),
  );
  return {
    schema: writableProfileSchema,
    schemaVersion: writableProfileSchemaVersion,
    titleFingerprint: profile.titleFingerprint,
    profileId: profile.profileId,
    rootPathA: [...profile.rootPathA],
    quotaBytes: profile.quotaBytes,
    files: profile.files.map((configured) => {
      const file = stored.get(configured.objectId);
      if (file === undefined) {
        return {
          objectId: configured.objectId,
          relativePathA: [...configured.relativePathA],
          exists: false,
          generation: 0,
          byteLength: 0,
          sha256: null,
        };
      }
      const content = Buffer.from(file.content);
      assertStoredFile(file, content);
      return {
        objectId: configured.objectId,
        relativePathA: [...configured.relativePathA],
        exists: true,
        generation: file.generation,
        byteLength: file.byte_length,
        sha256: file.sha256,
      };
    }),
    directories: profile.directories.map((configured) => {
      const directory = storedDirectories.get(configured.directoryId);
      return {
        directoryId: configured.directoryId,
        relativePathA: [...configured.relativePathA],
        exists: directory !== undefined,
        generation: directory?.generation ?? 0,
      };
    }),
  };
}

export function acquireWritableProfileLease(profile, sessionKey) {
  assertSessionKey(sessionKey);
  const database = profile.database;
  database.exec("BEGIN IMMEDIATE");
  let committed = false;
  try {
    const current = database
      .prepare("SELECT epoch FROM profile_lease WHERE singleton = 1")
      .get();
    if (
      current === undefined ||
      !Number.isSafeInteger(current.epoch) ||
      current.epoch < 0 ||
      current.epoch >= maximumGeneration
    ) {
      throw profileError(
        "writable-profile-lease-state-invalid",
        "writable profile lease epoch is missing, invalid, or exhausted",
      );
    }
    const epoch = current.epoch + 1;
    database
      .prepare(
        "UPDATE profile_lease SET epoch = ?, session_key = ?, active = 1 WHERE singleton = 1",
      )
      .run(epoch, sessionKey);
    database.exec("COMMIT");
    committed = true;
    return { sessionKey, epoch };
  } finally {
    if (!committed) {
      database.exec("ROLLBACK");
    }
  }
}

export function revokeWritableProfileLease(profile, lease) {
  const normalized = normalizeLease(lease);
  const database = profile.database;
  database.exec("BEGIN IMMEDIATE");
  let committed = false;
  try {
    const active = activeWritableProfileLease(database);
    const revoked =
      active.active === 1 &&
      active.epoch === normalized.epoch &&
      active.session_key === normalized.sessionKey;
    if (revoked) {
      database
        .prepare(
          "UPDATE profile_lease SET session_key = NULL, active = 0 WHERE singleton = 1",
        )
        .run();
    }
    database.exec("COMMIT");
    committed = true;
    return { ...normalized, revoked };
  } finally {
    if (!committed) {
      database.exec("ROLLBACK");
    }
  }
}

export function readWritableProfile(profile, request) {
  if (request.sourceId !== 0) {
    throw profileError(
      "writable-profile-read-source-invalid",
      "writable profile reads require the reserved source ID zero",
    );
  }
  const configured = profile.files.find((file) => file.objectId === request.objectId);
  if (configured === undefined) {
    throw profileError(
      "writable-profile-read-object-unknown",
      "profile read object is not in the writable allowlist",
    );
  }
  const stored = profile.database
    .prepare("SELECT generation, byte_length, sha256, content FROM files WHERE object_id = ?")
    .get(request.objectId);
  if (stored === undefined) {
    throw profileError(
      "writable-profile-read-file-missing",
      "profile read object has no committed generation",
    );
  }
  const bytes = Buffer.from(stored.content);
  assertStoredFile(stored, bytes);
  if (request.offset > BigInt(bytes.length)) {
    throw profileError(
      "writable-profile-read-range-invalid",
      "profile read offset exceeds the committed object",
    );
  }
  const offset = Number(request.offset);
  const transferredBytes = Math.min(request.requestedBytes, bytes.length - offset);
  return {
    sourceId: 0,
    objectId: request.objectId,
    offset: request.offset,
    requestedBytes: request.requestedBytes,
    transferredBytes,
    bytes: Buffer.from(bytes.subarray(offset, offset + transferredBytes)),
    generation: stored.generation,
  };
}

export function commitWritableProfile(
  profile,
  request,
  content,
  { failpoint = null, deleteFile = false } = {},
) {
  const normalized = normalizeCommitRequest(profile, request, content, deleteFile);
  if (failpoint === "before-transaction") {
    throw profileError("writable-profile-failpoint", "injected failure before transaction");
  }
  const database = profile.database;
  database.exec("BEGIN IMMEDIATE");
  let committed = false;
  try {
    assertActiveWritableProfileLease(database, normalized);
    const duplicate = database
      .prepare(
        "SELECT object_id, base_generation, committed_generation, byte_length, sha256 FROM commits WHERE request_key = ?",
      )
      .get(normalized.requestKey);
    if (duplicate !== undefined) {
      assertDuplicateCommit(duplicate, normalized);
      database.exec("COMMIT");
      committed = true;
      return {
        objectId: normalized.objectId,
        baseGeneration: normalized.baseGeneration,
        committedGeneration: duplicate.committed_generation,
        win32Error: 0,
        duplicate: true,
      };
    }

    const current = database
      .prepare("SELECT generation FROM files WHERE object_id = ?")
      .get(normalized.objectId);
    const currentGeneration = current?.generation ?? 0;
    if ((deleteFile && current === undefined) || currentGeneration !== normalized.baseGeneration) {
      throw profileError(
        "writable-profile-generation-stale",
        `commit base generation ${normalized.baseGeneration} does not match ${currentGeneration}`,
      );
    }
    const otherBytes = database
      .prepare("SELECT COALESCE(SUM(byte_length), 0) AS total FROM files WHERE object_id <> ?")
      .get(normalized.objectId).total;
    if (otherBytes + normalized.byteLength > profile.quotaBytes) {
      throw profileError(
        "writable-profile-quota-exceeded",
        "commit would exceed the writable profile quota",
      );
    }
    const committedGeneration = normalized.baseGeneration + 1;
    if (committedGeneration > maximumGeneration) {
      throw profileError(
        "writable-profile-generation-exhausted",
        "writable profile generation exceeds lossless host accounting",
      );
    }
    if (deleteFile) {
      database.prepare("DELETE FROM files WHERE object_id = ?").run(normalized.objectId);
    } else {
      database
        .prepare(`
          INSERT INTO files(object_id, generation, byte_length, sha256, content)
          VALUES (?, ?, ?, ?, ?)
          ON CONFLICT(object_id) DO UPDATE SET
            generation = excluded.generation,
            byte_length = excluded.byte_length,
            sha256 = excluded.sha256,
            content = excluded.content
        `)
        .run(
          normalized.objectId,
          committedGeneration,
          normalized.byteLength,
          normalized.sha256,
          normalized.content,
        );
    }
    database
      .prepare(`
        INSERT INTO commits(
          request_key, object_id, base_generation, committed_generation, byte_length, sha256
        ) VALUES (?, ?, ?, ?, ?, ?)
      `)
      .run(
        normalized.requestKey,
        normalized.objectId,
        normalized.baseGeneration,
        committedGeneration,
        normalized.byteLength,
        normalized.sha256,
      );
    if (failpoint === "after-write-before-commit") {
      throw profileError("writable-profile-failpoint", "injected failure before SQLite commit");
    }
    database.exec("COMMIT");
    committed = true;
    if (failpoint === "after-commit-response-lost") {
      throw profileError("writable-profile-response-lost", "injected response loss after commit");
    }
    return {
      objectId: normalized.objectId,
      baseGeneration: normalized.baseGeneration,
      committedGeneration,
      win32Error: 0,
      duplicate: false,
    };
  } finally {
    if (!committed) {
      database.exec("ROLLBACK");
    }
  }
}

export function deleteWritableProfile(profile, request, options = {}) {
  return commitWritableProfile(
    profile,
    {
      ...request,
      byteLength: 0,
      sha256: writableProfileDeleteMarkerSha256,
    },
    Buffer.alloc(0),
    { ...options, deleteFile: true },
  );
}

export function commitWritableDirectory(profile, request, { failpoint = null } = {}) {
  const normalized = normalizeDirectoryCommitRequest(profile, request);
  if (failpoint === "before-transaction") {
    throw profileError(
      "writable-profile-directory-failpoint",
      "injected failure before directory transaction",
    );
  }
  const database = profile.database;
  database.exec("BEGIN IMMEDIATE");
  let committed = false;
  try {
    assertActiveWritableProfileLease(database, normalized);
    const duplicate = database
      .prepare(
        "SELECT directory_id, base_generation, committed_generation FROM directory_commits WHERE request_key = ?",
      )
      .get(normalized.requestKey);
    if (duplicate !== undefined) {
      assertDuplicateDirectoryCommit(duplicate, normalized);
      database.exec("COMMIT");
      committed = true;
      return {
        directoryId: normalized.directoryId,
        baseGeneration: normalized.baseGeneration,
        committedGeneration: duplicate.committed_generation,
        win32Error: 0,
        duplicate: true,
      };
    }
    const current = database
      .prepare("SELECT generation FROM directories WHERE directory_id = ?")
      .get(normalized.directoryId);
    const currentGeneration = current?.generation ?? 0;
    if (currentGeneration !== normalized.baseGeneration) {
      throw profileError(
        "writable-profile-directory-generation-stale",
        `directory create base generation ${normalized.baseGeneration} does not match ${currentGeneration}`,
      );
    }
    const committedGeneration = normalized.baseGeneration + 1;
    if (committedGeneration > maximumGeneration) {
      throw profileError(
        "writable-profile-directory-generation-exhausted",
        "writable directory generation exceeds lossless host accounting",
      );
    }
    database
      .prepare(`
        INSERT INTO directories(directory_id, generation)
        VALUES (?, ?)
        ON CONFLICT(directory_id) DO UPDATE SET generation = excluded.generation
      `)
      .run(normalized.directoryId, committedGeneration);
    database
      .prepare(`
        INSERT INTO directory_commits(
          request_key, directory_id, base_generation, committed_generation
        ) VALUES (?, ?, ?, ?)
      `)
      .run(
        normalized.requestKey,
        normalized.directoryId,
        normalized.baseGeneration,
        committedGeneration,
      );
    if (failpoint === "after-write-before-commit") {
      throw profileError(
        "writable-profile-directory-failpoint",
        "injected failure before SQLite directory commit",
      );
    }
    database.exec("COMMIT");
    committed = true;
    if (failpoint === "after-commit-response-lost") {
      throw profileError(
        "writable-profile-directory-response-lost",
        "injected response loss after directory commit",
      );
    }
    return {
      directoryId: normalized.directoryId,
      baseGeneration: normalized.baseGeneration,
      committedGeneration,
      win32Error: 0,
      duplicate: false,
    };
  } finally {
    if (!committed) {
      database.exec("ROLLBACK");
    }
  }
}

export async function handleWritableProfileHttp(request, response, profile) {
  const url = new URL(request.url ?? "/", "http://127.0.0.1");
  if (!url.pathname.startsWith("/api/profile/v3/")) {
    return false;
  }
  try {
    if (url.pathname === "/api/profile/v3/sessions") {
      if (request.method !== "POST") {
        sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
        return true;
      }
      const sessionKey = requiredHeader(request, "x-vine-session-key", 256);
      const lease = acquireWritableProfileLease(profile, sessionKey);
      response.writeHead(201, profileHeaders(profile, {
        "Content-Length": "0",
        "X-Vine-Session-Epoch": String(lease.epoch),
        "X-Vine-Session-Key": lease.sessionKey,
      }));
      response.end();
      return true;
    }
    const sessionMatch = url.pathname.match(
      /^\/api\/profile\/v3\/sessions\/([1-9][0-9]*)$/u,
    );
    if (sessionMatch !== null) {
      if (request.method !== "DELETE") {
        sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
        return true;
      }
      const lease = {
        sessionKey: requiredHeader(request, "x-vine-session-key", 256),
        epoch: parseSafeInteger(sessionMatch[1], "profile session epoch", 1),
      };
      const result = revokeWritableProfileLease(profile, lease);
      response.writeHead(204, profileHeaders(profile, {
        "X-Vine-Session-Epoch": String(result.epoch),
        "X-Vine-Session-Key": result.sessionKey,
        "X-Vine-Session-Revoked": String(result.revoked),
      }));
      response.end();
      return true;
    }

    if (url.pathname === "/api/profile/v3/snapshot") {
      if (request.method !== "GET") {
        sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
        return true;
      }
      const lease = requestLease(request);
      assertActiveWritableProfileLease(profile.database, lease);
      const bytes = Buffer.from(`${JSON.stringify(snapshotWritableProfile(profile))}\n`);
      response.writeHead(200, profileHeaders(profile, {
        "Content-Type": "application/json; charset=utf-8",
        "Content-Length": String(bytes.length),
        "X-Vine-Session-Epoch": String(lease.sessionEpoch),
      }));
      response.end(bytes);
      return true;
    }

    const match = url.pathname.match(
      /^\/api\/profile\/v3\/titles\/([^/]+)\/profiles\/([^/]+)\/directories\/([1-9][0-9]*)\/commits$/u,
    );
    if (match !== null) {
      const [, encodedTitle, encodedProfile, directoryText] = match;
      const titleFingerprint = decodeURIComponent(encodedTitle);
      const profileId = decodeURIComponent(encodedProfile);
      const directoryId = Number(directoryText);
      const configured = profile.directories.find(
        (directory) => directory.directoryId === directoryId,
      );
      if (
        titleFingerprint !== profile.titleFingerprint ||
        profileId !== profile.profileId ||
        !Number.isSafeInteger(directoryId) ||
        configured === undefined
      ) {
        sendProfileError(
          response,
          404,
          "writable-profile-directory-unknown",
          "profile directory not found",
        );
        return true;
      }
      if (request.method !== "POST") {
        sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
        return true;
      }
      const lease = requestLease(request);
      const requestKey = requiredHeader(request, "x-vine-request-key", 256);
      const baseGeneration = parseSafeInteger(
        requiredHeader(request, "x-vine-base-generation", 32),
        "directory create base generation",
        0,
        maximumGeneration - 1,
      );
      let storageErrorCode = null;
      let result;
      try {
        result = commitWritableDirectory(profile, {
          requestKey,
          titleFingerprint,
          profileId,
          directoryId,
          relativePathA: configured.relativePathA,
          baseGeneration,
          ...lease,
        });
      } catch (error) {
        const win32Error = writableCommitWin32Error(error);
        if (win32Error === null) {
          throw error;
        }
        storageErrorCode =
          typeof error?.code === "string"
            ? error.code
            : "writable-profile-directory-storage-failure";
        result = {
          directoryId,
          baseGeneration,
          committedGeneration: baseGeneration,
          win32Error,
          duplicate: false,
        };
      }
      const control = encodeDirectoryCommitResponse(result);
      response.writeHead(200, profileHeaders(profile, {
        "Content-Type": "application/vnd.vine.profile-directory-create.v1",
        "Content-Length": String(control.length),
        "X-Vine-Directory-Id": String(directoryId),
        "X-Vine-Base-Generation": String(baseGeneration),
        "X-Vine-Committed-Generation": String(result.committedGeneration),
        "X-Vine-Duplicate": String(result.duplicate),
        "X-Vine-Session-Epoch": String(lease.sessionEpoch),
        ...(storageErrorCode === null
          ? {}
          : { "X-Vine-Storage-Error-Code": storageErrorCode }),
      }));
      response.end(control);
      return true;
    }

    const objectMatch = url.pathname.match(
      /^\/api\/profile\/v3\/titles\/([^/]+)\/profiles\/([^/]+)\/objects\/([1-9][0-9]*)(\/commits)?$/u,
    );
    if (objectMatch === null) {
      sendProfileError(response, 404, "writable-profile-route-unknown", "profile route not found");
      return true;
    }
    const [, encodedTitle, encodedProfile, objectText, commitSuffix] = objectMatch;
    const titleFingerprint = decodeURIComponent(encodedTitle);
    const profileId = decodeURIComponent(encodedProfile);
    const objectId = Number(objectText);
    if (
      titleFingerprint !== profile.titleFingerprint ||
      profileId !== profile.profileId ||
      !Number.isSafeInteger(objectId) ||
      !profile.files.some((file) => file.objectId === objectId)
    ) {
      sendProfileError(response, 404, "writable-profile-object-unknown", "profile object not found");
      return true;
    }
    const lease = requestLease(request);

    if (commitSuffix === undefined) {
      if (request.method !== "GET") {
        sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
        return true;
      }
      const generation = parseSafeInteger(url.searchParams.get("generation"), "read generation", 1);
      const offset = parseBoundedBigInt(url.searchParams.get("offset"), "read offset");
      const requestedBytes = parseSafeInteger(
        url.searchParams.get("length"),
        "read length",
        1,
        writableProfileMaximumIoBytes,
      );
      assertActiveWritableProfileLease(profile.database, lease);
      const result = readWritableProfile(profile, {
        sourceId: 0,
        objectId,
        offset,
        requestedBytes,
      });
      if (result.generation !== generation) {
        throw profileError(
          "writable-profile-read-generation-stale",
          "profile read generation differs from committed storage",
        );
      }
      response.writeHead(206, profileHeaders(profile, {
        "Content-Type": "application/octet-stream",
        "Content-Length": String(result.bytes.length),
        "X-Vine-Object-Id": String(objectId),
        "X-Vine-Generation": String(result.generation),
        "X-Vine-Offset": offset.toString(),
        "X-Vine-Requested-Bytes": String(requestedBytes),
        "X-Vine-Transferred-Bytes": String(result.transferredBytes),
        "X-Vine-Session-Epoch": String(lease.sessionEpoch),
      }));
      response.end(result.bytes);
      return true;
    }

    if (request.method !== "POST") {
      sendProfileError(response, 405, "writable-profile-method-invalid", "method not allowed");
      return true;
    }
    const requestKey = requiredHeader(request, "x-vine-request-key", 256);
    const baseGeneration = parseSafeInteger(
      requiredHeader(request, "x-vine-base-generation", 32),
      "commit base generation",
      0,
      maximumGeneration - 1,
    );
    const byteLength = parseSafeInteger(
      requiredHeader(request, "x-vine-byte-length", 32),
      "commit byte length",
      0,
      profile.quotaBytes,
    );
    const contentSha256 = requiredHeader(request, "x-vine-content-sha256", 64);
    if (!/^[0-9a-f]{64}$/u.test(contentSha256)) {
      throw profileError(
        "writable-profile-commit-hash-invalid",
        "commit content hash is not lowercase SHA-256",
      );
    }
    const deleteHeader = request.headers["x-vine-delete"];
    if (deleteHeader !== undefined && deleteHeader !== "1") {
      throw profileError(
        "writable-profile-delete-header-invalid",
        "delete marker header is invalid",
      );
    }
    const deleteFile = deleteHeader === "1";
    const content = await readRequestBytes(request, profile.quotaBytes);
    if (
      deleteFile &&
      (byteLength !== 0 || contentSha256 !== writableProfileDeleteMarkerSha256)
    ) {
      throw profileError(
        "writable-profile-delete-metadata-invalid",
        "delete mutation requires an empty body and the deletion marker hash",
      );
    }
    if (content.length !== byteLength) {
      throw profileError(
        "writable-profile-commit-length-mismatch",
        "commit body length differs from declared content length",
      );
    }
    const configured = profile.files.find((file) => file.objectId === objectId);
    let storageErrorCode = null;
    let result;
    try {
      const mutation = {
        requestKey,
        titleFingerprint,
        profileId,
        objectId,
        relativePathA: configured.relativePathA,
        baseGeneration,
        byteLength,
        sha256: contentSha256,
        ...lease,
      };
      result = deleteFile
        ? deleteWritableProfile(profile, mutation)
        : commitWritableProfile(profile, mutation, content);
    } catch (error) {
      const win32Error = writableCommitWin32Error(error);
      if (win32Error === null) {
        throw error;
      }
      storageErrorCode =
        typeof error?.code === "string" ? error.code : "writable-profile-storage-failure";
      result = {
        objectId,
        baseGeneration,
        committedGeneration: baseGeneration,
        win32Error,
        duplicate: false,
      };
    }
    const control = encodeCommitResponse(result);
    response.writeHead(200, profileHeaders(profile, {
      "Content-Type": "application/vnd.vine.profile-commit.v1",
      "Content-Length": String(control.length),
      "X-Vine-Object-Id": String(objectId),
      "X-Vine-Base-Generation": String(baseGeneration),
      "X-Vine-Committed-Generation": String(result.committedGeneration),
      "X-Vine-Content-Sha256": contentSha256,
      "X-Vine-Delete": String(deleteFile),
      "X-Vine-Duplicate": String(result.duplicate),
      "X-Vine-Session-Epoch": String(lease.sessionEpoch),
      ...(storageErrorCode === null
        ? {}
        : { "X-Vine-Storage-Error-Code": storageErrorCode }),
    }));
    response.end(control);
    return true;
  } catch (error) {
    const code = typeof error?.code === "string" ? error.code : "writable-profile-request-invalid";
    const status = code.includes("quota") || code.includes("bytes") ? 413
      : code.includes("stale") || code.includes("conflict") ? 409
        : code.includes("missing") || code.includes("unknown") ? 404
          : 400;
    sendProfileError(response, status, code, error instanceof Error ? error.message : String(error));
    return true;
  }
}

function initializeProfile(database, configuration) {
  database.exec("BEGIN IMMEDIATE");
  let committed = false;
  try {
    const metadata = database.prepare("SELECT * FROM profile_metadata WHERE singleton = 1").get();
    if (metadata === undefined) {
      database
        .prepare(`
          INSERT INTO profile_metadata(
            singleton, schema_version, title_fingerprint, profile_id, root_path_a, quota_bytes
          ) VALUES (1, ?, ?, ?, ?, ?)
        `)
        .run(
          databaseSchemaVersion,
          configuration.titleFingerprint,
          configuration.profileId,
          configuration.rootPathA,
          configuration.quotaBytes,
        );
      const insertPath = database.prepare(
        "INSERT INTO writable_paths(object_id, relative_path_a) VALUES (?, ?)",
      );
      const seed = database.prepare(
        "INSERT INTO files(object_id, generation, byte_length, sha256, content) VALUES (?, 1, ?, ?, ?)",
      );
      let seededBytes = 0;
      for (const file of configuration.files) {
        insertPath.run(file.objectId, file.relativePathA);
        if (file.seedBytes !== null) {
          seededBytes += file.seedBytes.length;
          if (seededBytes > configuration.quotaBytes) {
            throw profileError(
              "writable-profile-seed-quota-exceeded",
              "initial profile seed exceeds its quota",
            );
          }
          seed.run(file.objectId, file.seedBytes.length, sha256(file.seedBytes), file.seedBytes);
        }
      }
      const insertDirectory = database.prepare(
        "INSERT INTO writable_directory_paths(directory_id, relative_path_a) VALUES (?, ?)",
      );
      for (const directory of configuration.directories) {
        insertDirectory.run(directory.directoryId, directory.relativePathA);
      }
    } else {
      const previousSchemaVersion = metadata.schema_version;
      if (previousSchemaVersion === 1 || previousSchemaVersion === 2) {
        database
          .prepare("UPDATE profile_metadata SET schema_version = ? WHERE singleton = 1")
          .run(databaseSchemaVersion);
        metadata.schema_version = databaseSchemaVersion;
      }
      assertMetadata(metadata, configuration);
      const storedPaths = database
        .prepare("SELECT object_id, relative_path_a FROM writable_paths ORDER BY object_id")
        .all();
      if (
        storedPaths.length !== configuration.files.length ||
        storedPaths.some((stored, index) => {
          const expected = [...configuration.files].sort((a, b) => a.objectId - b.objectId)[index];
          return (
            stored.object_id !== expected.objectId ||
            !Buffer.from(stored.relative_path_a).equals(expected.relativePathA)
          );
        })
      ) {
        throw profileError(
          "writable-profile-path-identity-mismatch",
          "existing database writable paths differ from the requested profile",
        );
      }
      if (previousSchemaVersion === 1 || previousSchemaVersion === 2) {
        const insertDirectory = database.prepare(
          "INSERT OR IGNORE INTO writable_directory_paths(directory_id, relative_path_a) VALUES (?, ?)",
        );
        for (const directory of configuration.directories) {
          insertDirectory.run(directory.directoryId, directory.relativePathA);
        }
      }
      const storedDirectories = database
        .prepare(
          "SELECT directory_id, relative_path_a FROM writable_directory_paths ORDER BY directory_id",
        )
        .all();
      if (
        storedDirectories.length !== configuration.directories.length ||
        storedDirectories.some((stored, index) => {
          const expected = [...configuration.directories].sort(
            (a, b) => a.directoryId - b.directoryId,
          )[index];
          return (
            stored.directory_id !== expected.directoryId ||
            !Buffer.from(stored.relative_path_a).equals(expected.relativePathA)
          );
        })
      ) {
        throw profileError(
          "writable-profile-directory-identity-mismatch",
          "existing database writable directories differ from the requested profile",
        );
      }
    }
    database.exec("COMMIT");
    committed = true;
  } finally {
    if (!committed) {
      database.exec("ROLLBACK");
    }
  }
}

function normalizeConfiguredFile(file) {
  assertInteger(file.objectId, "writable object ID", 1, 0xffff_ffff);
  const relativePathA = boundedBytes(file.relativePathA, "writable relative path", 32 * 1024);
  if (relativePathA.length === 0 || relativePathA.includes(0)) {
    throw profileError(
      "writable-profile-relative-path-invalid",
      "writable relative path must be nonempty and NUL-free",
    );
  }
  const seedBytes = file.seedBytes === undefined || file.seedBytes === null
    ? null
    : boundedBytes(file.seedBytes, "writable seed", writableProfileMaximumBytes);
  return { objectId: file.objectId, relativePathA, seedBytes };
}

function normalizeConfiguredDirectory(directory) {
  assertInteger(directory.directoryId, "writable directory ID", 1, 0xffff_ffff);
  const relativePathA = boundedBytes(
    directory.relativePathA,
    "writable directory relative path",
    32 * 1024,
  );
  if (relativePathA.length === 0 || relativePathA.includes(0)) {
    throw profileError(
      "writable-profile-directory-relative-path-invalid",
      "writable directory relative path must be nonempty and NUL-free",
    );
  }
  return { directoryId: directory.directoryId, relativePathA };
}

function windowsPathIdentity(bytes) {
  return Buffer.from(
    [...bytes].map((byte) => (byte >= 0x41 && byte <= 0x5a ? byte + 0x20 : byte)),
  ).toString("base64");
}

function normalizeCommitRequest(profile, request, content, deleteFile = false) {
  assertText(request.requestKey, "commit request key", 256);
  if (
    request.titleFingerprint !== profile.titleFingerprint ||
    request.profileId !== profile.profileId
  ) {
    throw profileError(
      "writable-profile-commit-profile-mismatch",
      "commit title or profile identity does not match the database",
    );
  }
  assertInteger(request.objectId, "commit object ID", 1, 0xffff_ffff);
  assertInteger(request.baseGeneration, "commit base generation", 0, maximumGeneration - 1);
  assertInteger(request.byteLength, "commit byte length", 0, profile.quotaBytes);
  const lease = normalizeLease({
    sessionKey: request.sessionKey,
    epoch: request.sessionEpoch,
  });
  const configured = profile.files.find((file) => file.objectId === request.objectId);
  if (configured === undefined) {
    throw profileError(
      "writable-profile-commit-object-unknown",
      "commit object is not in the writable allowlist",
    );
  }
  if (
    request.relativePathA !== undefined &&
    !boundedBytes(request.relativePathA, "commit relative path", 32 * 1024).equals(
      configured.relativePathA,
    )
  ) {
    throw profileError(
      "writable-profile-commit-path-mismatch",
      "commit path does not match the writable object identity",
    );
  }
  const bytes = boundedBytes(content, "commit content", profile.quotaBytes);
  if (
    bytes.length !== request.byteLength ||
    !isSha256(request.sha256) ||
    (deleteFile
      ? bytes.length !== 0 || request.sha256 !== writableProfileDeleteMarkerSha256
      : sha256(bytes) !== request.sha256)
  ) {
    throw profileError(
      "writable-profile-commit-content-mismatch",
      "commit bytes do not match the declared length and SHA-256",
    );
  }
  return {
    ...request,
    sessionKey: lease.sessionKey,
    sessionEpoch: lease.epoch,
    content: bytes,
  };
}

function normalizeDirectoryCommitRequest(profile, request) {
  assertText(request.requestKey, "directory create request key", 256);
  if (
    request.titleFingerprint !== profile.titleFingerprint ||
    request.profileId !== profile.profileId
  ) {
    throw profileError(
      "writable-profile-directory-profile-mismatch",
      "directory create title or profile identity does not match the database",
    );
  }
  assertInteger(request.directoryId, "directory ID", 1, 0xffff_ffff);
  assertInteger(
    request.baseGeneration,
    "directory create base generation",
    0,
    maximumGeneration - 1,
  );
  const lease = normalizeLease({
    sessionKey: request.sessionKey,
    epoch: request.sessionEpoch,
  });
  const configured = profile.directories.find(
    (directory) => directory.directoryId === request.directoryId,
  );
  if (configured === undefined) {
    throw profileError(
      "writable-profile-directory-unknown",
      "directory create identity is not in the writable allowlist",
    );
  }
  if (
    request.relativePathA !== undefined &&
    !boundedBytes(
      request.relativePathA,
      "directory create relative path",
      32 * 1024,
    ).equals(configured.relativePathA)
  ) {
    throw profileError(
      "writable-profile-directory-path-mismatch",
      "directory create path does not match the writable directory identity",
    );
  }
  return {
    ...request,
    sessionKey: lease.sessionKey,
    sessionEpoch: lease.epoch,
  };
}

function requestLease(request) {
  return {
    sessionKey: requiredHeader(request, "x-vine-session-key", 256),
    sessionEpoch: parseSafeInteger(
      requiredHeader(request, "x-vine-session-epoch", 32),
      "profile session epoch",
      1,
    ),
  };
}

function normalizeLease(lease) {
  assertSessionKey(lease?.sessionKey);
  assertInteger(lease?.epoch, "profile session epoch", 1, maximumGeneration);
  return { sessionKey: lease.sessionKey, epoch: lease.epoch };
}

function assertSessionKey(sessionKey) {
  assertText(sessionKey, "profile session key", 256);
  if (!/^[a-zA-Z0-9:._-]+$/u.test(sessionKey)) {
    throw profileError(
      "writable-profile-session-key-invalid",
      "profile session key contains an unsupported character",
    );
  }
}

function activeWritableProfileLease(database) {
  const active = database
    .prepare("SELECT epoch, session_key, active FROM profile_lease WHERE singleton = 1")
    .get();
  if (
    active === undefined ||
    !Number.isSafeInteger(active.epoch) ||
    active.epoch < 0 ||
    (active.active !== 0 && active.active !== 1) ||
    (active.active === 0 ? active.session_key !== null : typeof active.session_key !== "string")
  ) {
    throw profileError(
      "writable-profile-lease-state-invalid",
      "writable profile lease state is invalid",
    );
  }
  return active;
}

function assertActiveWritableProfileLease(database, request) {
  const lease = normalizeLease({
    sessionKey: request.sessionKey,
    epoch: request.sessionEpoch,
  });
  const active = activeWritableProfileLease(database);
  if (
    active.active !== 1 ||
    active.epoch !== lease.epoch ||
    active.session_key !== lease.sessionKey
  ) {
    throw profileError(
      "writable-profile-lease-revoked",
      "writable profile request does not own the active server lease",
    );
  }
}

function writableCommitWin32Error(error) {
  const code = typeof error?.code === "string" ? error.code : "";
  const message = error instanceof Error ? error.message : String(error);
  if (code.includes("quota") || code.includes("disk-full")) {
    return errorDiskFull;
  }
  if (code.includes("generation-stale")) {
    return errorWriteFault;
  }
  if (code === "writable-profile-lease-revoked") {
    return errorOperationAborted;
  }
  if (
    code.includes("SQLITE_BUSY") ||
    code.includes("SQLITE_LOCKED") ||
    /database is (?:busy|locked)/iu.test(message)
  ) {
    return errorLockViolation;
  }
  if (code.startsWith("ERR_SQLITE") || code.startsWith("SQLITE_")) {
    return errorWriteFault;
  }
  return null;
}

function assertMetadata(metadata, configuration) {
  if (
    metadata.schema_version !== databaseSchemaVersion ||
    metadata.title_fingerprint !== configuration.titleFingerprint ||
    metadata.profile_id !== configuration.profileId ||
    !Buffer.from(metadata.root_path_a).equals(configuration.rootPathA) ||
    metadata.quota_bytes !== configuration.quotaBytes
  ) {
    throw profileError(
      "writable-profile-database-identity-mismatch",
      "existing writable database metadata differs from the requested profile",
    );
  }
}

function assertStoredFile(file, content) {
  if (
    file.byte_length !== content.length ||
    !isSha256(file.sha256) ||
    sha256(content) !== file.sha256 ||
    !Number.isSafeInteger(file.generation) ||
    file.generation <= 0
  ) {
    throw profileError(
      "writable-profile-database-content-invalid",
      "stored writable bytes fail their generation, length, or hash invariant",
    );
  }
}

function assertDuplicateCommit(stored, request) {
  if (
    stored.object_id !== request.objectId ||
    stored.base_generation !== request.baseGeneration ||
    stored.byte_length !== request.byteLength ||
    stored.sha256 !== request.sha256
  ) {
    throw profileError(
      "writable-profile-request-key-conflict",
      "durable request key was reused for different commit content",
    );
  }
}

function assertDuplicateDirectoryCommit(stored, request) {
  if (
    stored.directory_id !== request.directoryId ||
    stored.base_generation !== request.baseGeneration
  ) {
    throw profileError(
      "writable-profile-directory-request-key-conflict",
      "durable directory request key was reused for another identity or generation",
    );
  }
}

function boundedBytes(value, label, maximum) {
  const bytes = Buffer.from(value);
  if (bytes.length > maximum) {
    throw profileError("writable-profile-bytes-invalid", `${label} exceeds ${maximum} bytes`);
  }
  return bytes;
}

function assertText(value, label, maximum) {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum) {
    throw profileError("writable-profile-text-invalid", `${label} is empty or too long`);
  }
}

function assertInteger(value, label, minimum, maximum) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw profileError(
      "writable-profile-integer-invalid",
      `${label} is outside ${minimum}..${maximum}`,
    );
  }
}

function isSha256(value) {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function profileHeaders(profile, headers = {}) {
  return {
    "Cache-Control": "no-store",
    "Cross-Origin-Resource-Policy": "same-origin",
    "X-Content-Type-Options": "nosniff",
    "X-Vine-Title-Fingerprint": profile.titleFingerprint,
    "X-Vine-Profile-Id": profile.profileId,
    ...headers,
  };
}

function requiredHeader(request, name, maximumLength) {
  const value = request.headers[name];
  if (typeof value !== "string" || value.length === 0 || value.length > maximumLength) {
    throw profileError(
      "writable-profile-header-invalid",
      `required ${name} header is missing or too long`,
    );
  }
  return value;
}

function parseSafeInteger(value, label, minimum, maximum = Number.MAX_SAFE_INTEGER) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/u.test(value)) {
    throw profileError("writable-profile-integer-invalid", `${label} is not canonical decimal`);
  }
  const parsed = Number(value);
  assertInteger(parsed, label, minimum, maximum);
  return parsed;
}

function parseBoundedBigInt(value, label) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/u.test(value)) {
    throw profileError("writable-profile-integer-invalid", `${label} is not canonical decimal`);
  }
  const parsed = BigInt(value);
  if (parsed > 0xffff_ffff_ffff_ffffn) {
    throw profileError("writable-profile-integer-invalid", `${label} exceeds unsigned 64-bit`);
  }
  return parsed;
}

async function readRequestBytes(request, maximumBytes) {
  const declared = request.headers["content-length"];
  if (declared !== undefined) {
    parseSafeInteger(String(declared), "request content length", 0, maximumBytes);
  }
  const chunks = [];
  let total = 0;
  for await (const chunk of request) {
    const bytes = Buffer.from(chunk);
    total += bytes.length;
    if (total > maximumBytes) {
      throw profileError(
        "writable-profile-request-bytes-exceeded",
        "writable profile request body exceeds the profile quota",
      );
    }
    chunks.push(bytes);
  }
  return Buffer.concat(chunks, total);
}

function encodeCommitResponse(result) {
  const control = Buffer.alloc(32);
  const words = [
    1,
    result.objectId,
    result.baseGeneration >>> 0,
    Math.floor(result.baseGeneration / 0x1_0000_0000),
    result.committedGeneration >>> 0,
    Math.floor(result.committedGeneration / 0x1_0000_0000),
    result.win32Error,
    0,
  ];
  words.forEach((word, index) => control.writeUInt32LE(word, index * 4));
  return control;
}

function encodeDirectoryCommitResponse(result) {
  const control = Buffer.alloc(32);
  const words = [
    1,
    result.directoryId,
    result.baseGeneration >>> 0,
    Math.floor(result.baseGeneration / 0x1_0000_0000),
    result.committedGeneration >>> 0,
    Math.floor(result.committedGeneration / 0x1_0000_0000),
    result.win32Error,
    0,
  ];
  words.forEach((word, index) => control.writeUInt32LE(word, index * 4));
  return control;
}

function sendProfileError(response, status, code, message) {
  const body = Buffer.from(`${JSON.stringify({ code, message })}\n`);
  response.writeHead(status, {
    "Cache-Control": "no-store",
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": String(body.length),
    "Cross-Origin-Resource-Policy": "same-origin",
    "X-Content-Type-Options": "nosniff",
    "X-Vine-Error-Code": code,
  });
  response.end(body);
}

function profileError(code, message) {
  return Object.assign(new Error(message), { code });
}
