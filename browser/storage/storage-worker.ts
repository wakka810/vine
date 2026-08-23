import { sha256 } from "@noble/hashes/sha2.js";
import sqlite3InitModule, {
  type Database,
  type SAHPoolUtil,
} from "@sqlite.org/sqlite-wasm";

import {
  type BrowserNativeStorageConfiguration,
  type BrowserProfileSeed,
  type BrowserSourceFile,
  type BrowserSourceMount,
  type BrowserStorageInitializationResult,
  type BrowserStorageRequest,
  type BrowserStorageResponse,
  type BrowserWritableProfileConfiguration,
  type WritableProfileSnapshot,
  browserNativeStorageSchema,
  browserNativeStorageSchemaVersion,
  browserSourceMountSchema,
  browserSourceMountSchemaVersion,
  browserStorageProtocolVersion,
  browserWritableProfileSchema,
  browserWritableProfileSchemaVersion,
} from "./protocol.js";

const workerScope = self as DedicatedWorkerGlobalScope;
const globalProfileLockName = "vine.browser-storage.opfs-sahpool.v1";
const sqlitePoolDirectory = "/vine/v1/sqlite-sahpool";
const sqlitePoolName = "vine-opfs-sahpool-v1";
const sqliteApplicationId = 0x5649_4e45;
const sqliteSchemaVersion = 1;
const maximumTransportBytes = 1024 * 1024;
const maximumProfileQuotaBytes = 16 * 1024 * 1024;
const maximumGeneration = Number.MAX_SAFE_INTEGER;
const identityPattern = /^[a-zA-Z0-9:._-]{1,128}$/u;
const simpleIdentityPattern = /^[a-zA-Z0-9._-]{1,128}$/u;
const sha256Pattern = /^[0-9a-f]{64}$/u;
const writableDeleteMarkerSha256 = "0".repeat(64);
const requestKeyPattern = /^[\x21-\x7e]{1,256}$/u;

type SqliteStatic = Awaited<ReturnType<typeof sqlite3InitModule>>;
type SuccessfulResult = Extract<BrowserStorageResponse, { type: "success" }>["result"];

interface SourceState {
  mount: BrowserSourceMount;
  files: Map<number, BrowserSourceFile>;
}

interface ProfileState {
  configuration: BrowserWritableProfileConfiguration;
  sqlite: SqliteStatic;
  pool: SAHPoolUtil;
  database: Database;
  openTestTransaction: boolean;
}

interface StorageState {
  sessionKey: string;
  sessionEpoch: number;
  allowTestFaults: boolean;
  source: SourceState;
  profile: ProfileState | null;
  releaseLock: (() => void) | null;
  closed: boolean;
}

class StorageFault extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "StorageFault";
    this.code = code;
  }
}

let state: StorageState | null = null;
let initializationStarted = false;
let requestQueue = Promise.resolve();

workerScope.addEventListener("message", (event: MessageEvent<BrowserStorageRequest>) => {
  const request = event.data;
  requestQueue = requestQueue.then(async () => {
    try {
      validateEnvelope(request);
      const result = await dispatch(request);
      if (result !== null) {
        postSuccess(request.requestId, result);
      }
    } catch (error) {
      postFailure(requestIdFrom(error, request), asStorageFault(error));
    }
  });
});

async function dispatch(request: BrowserStorageRequest): Promise<SuccessfulResult | null> {
  if (request.type === "initialize") {
    return initialize(request.configuration, request.sessionKey);
  }
  const active = requireSession(request);
  switch (request.type) {
    case "source-read":
      return {
        kind: "source-read",
        bytes: await readSourceRange(
          active.source,
          request.sourceId,
          request.objectId,
          request.offset,
          request.requestedBytes,
        ),
      };
    case "profile-snapshot":
      return { kind: "profile-snapshot", snapshot: profileSnapshot(requireProfile(active)) };
    case "profile-read":
      return {
        kind: "profile-read",
        bytes: profileRead(
          active,
          request.objectId,
          request.generation,
          request.offset,
          request.requestedBytes,
        ),
      };
    case "profile-commit": {
      const result = profileCommit(active, request);
      if (request.testDropResponse === true) {
        requireTestFaults(active);
        workerScope.postMessage({
          protocolVersion: browserStorageProtocolVersion,
          type: "test-commit-published",
          requestId: request.requestId,
          requestKey: request.requestKey,
          committedGeneration: result.committedGeneration,
        } satisfies BrowserStorageResponse);
        return null;
      }
      return result;
    }
    case "profile-directory-create":
      return profileDirectoryCreate(active, request);
    case "test-begin-uncommitted":
      requireTestFaults(active);
      beginUncommittedMutation(active, request);
      return { kind: "test-uncommitted-started" };
    case "close":
      await closeStorage(active);
      return { kind: "closed" };
  }
}

async function initialize(
  configuration: BrowserNativeStorageConfiguration,
  sessionKey: string,
): Promise<BrowserStorageInitializationResult> {
  if (initializationStarted) {
    throw new StorageFault(
      "browser-storage-already-initialized",
      "storage worker accepts one initialization",
    );
  }
  initializationStarted = true;
  validateConfiguration(configuration);
  validateSessionKey(sessionKey);
  const source = await prepareSource(configuration.sourceMount);
  if (configuration.writableProfile === undefined) {
    state = {
      sessionKey,
      sessionEpoch: 1,
      allowTestFaults: configuration.allowTestFaults === true,
      source,
      profile: null,
      releaseLock: null,
      closed: false,
    };
    return initializationResult(state, null);
  }
  return acquireProfileLock(
    configuration,
    sessionKey,
    source,
    configuration.writableProfile,
  );
}

function acquireProfileLock(
  configuration: BrowserNativeStorageConfiguration,
  sessionKey: string,
  source: SourceState,
  writableProfile: BrowserWritableProfileConfiguration,
): Promise<BrowserStorageInitializationResult> {
  let settleAcquisition:
    | {
        resolve: (result: BrowserStorageInitializationResult) => void;
        reject: (error: unknown) => void;
      }
    | undefined;
  const acquired = new Promise<BrowserStorageInitializationResult>((resolve, reject) => {
    settleAcquisition = { resolve, reject };
  });
  void navigator.locks
    .request(
      globalProfileLockName,
      { mode: "exclusive", ifAvailable: true },
      async (lock) => {
        if (lock === null) {
          settleAcquisition?.reject(
            new StorageFault(
              "browser-storage-profile-locked",
              "another Vine storage worker owns the OPFS SQLite pool",
            ),
          );
          return;
        }
        let releaseLock: (() => void) | null = null;
        try {
          const profile = await openProfile(source, writableProfile, sessionKey);
          const active: StorageState = {
            sessionKey,
            sessionEpoch: profileSessionEpoch(profile.database),
            allowTestFaults: configuration.allowTestFaults === true,
            source,
            profile,
            releaseLock: null,
            closed: false,
          };
          const held = new Promise<void>((resolve) => {
            releaseLock = resolve;
            active.releaseLock = resolve;
          });
          state = active;
          settleAcquisition?.resolve(initializationResult(active, profile.sqlite.version.libVersion));
          await held;
        } catch (error) {
          settleAcquisition?.reject(error);
        } finally {
          if (state !== null && state.releaseLock === releaseLock) {
            await cleanupProfile(state.profile);
            state.closed = true;
            state = null;
          }
        }
      },
    )
    .catch((error: unknown) => settleAcquisition?.reject(error));
  return acquired;
}

async function prepareSource(mount: BrowserSourceMount): Promise<SourceState> {
  validateSourceMount(mount);
  const permissionHandle = mount.directoryHandle as FileSystemDirectoryHandle & {
    queryPermission?: (descriptor: { mode: "read" }) => Promise<PermissionState>;
  };
  const permission =
    permissionHandle.queryPermission === undefined
      ? "granted"
      : await permissionHandle.queryPermission({ mode: "read" });
  if (permission !== "granted") {
    throw new StorageFault(
      "browser-source-permission-required",
      "source directory read permission must be granted from a user gesture",
    );
  }
  const files = new Map<number, BrowserSourceFile>();
  for (const descriptor of mount.files) {
    const handle = await resolveSourceFile(mount.directoryHandle, descriptor.pathSegments);
    const file = await handle.getFile();
    assertSourceIdentity(descriptor, file);
    files.set(descriptor.objectId, structuredClone(descriptor));
  }
  return { mount, files };
}

async function openProfile(
  source: SourceState,
  configuration: BrowserWritableProfileConfiguration,
  sessionKey: string,
): Promise<ProfileState> {
  validateProfileConfiguration(configuration, source.mount.titleFingerprint);
  const initializeSqlite = sqlite3InitModule as unknown as (options: {
    locateFile: (filename: string) => string;
  }) => Promise<SqliteStatic>;
  const sqlite = await initializeSqlite({
    locateFile: (filename) => new URL(filename, workerScope.location.href).href,
  });
  const pool = await sqlite.installOpfsSAHPoolVfs({
    clearOnInit: false,
    initialCapacity: 8,
    directory: sqlitePoolDirectory,
    name: sqlitePoolName,
  });
  const filename =
    `/profiles/${configuration.titleFingerprint}--${configuration.profileId}.sqlite3`;
  const database = new pool.OpfsSAHPoolDb(filename);
  let succeeded = false;
  try {
    database.exec("PRAGMA journal_mode=DELETE");
    database.exec("PRAGMA synchronous=FULL");
    database.exec("PRAGMA foreign_keys=ON");
    database.exec("PRAGMA temp_store=MEMORY");
    const userVersion = integerValue(database.selectValue("PRAGMA user_version"));
    if (userVersion === 0) {
      const seeds = await loadProfileSeeds(source, configuration);
      createProfileSchema(database, configuration, seeds, sessionKey);
    } else {
      if (userVersion !== sqliteSchemaVersion) {
        throw new StorageFault(
          "browser-profile-schema-unsupported",
          `profile schema ${userVersion} is unsupported`,
        );
      }
      const applicationId = integerValue(database.selectValue("PRAGMA application_id"));
      if (applicationId !== sqliteApplicationId) {
        throw new StorageFault(
          "browser-profile-database-identity-mismatch",
          "OPFS database is not a Vine writable profile",
        );
      }
      validateExistingProfile(database, configuration);
      advanceProfileLease(database, sessionKey);
    }
    const journalMode = database.selectValue("PRAGMA journal_mode");
    const synchronous = integerValue(database.selectValue("PRAGMA synchronous"));
    if (journalMode !== "delete" || synchronous !== 2) {
      throw new StorageFault(
        "browser-profile-durability-mode-mismatch",
        "SQLite rollback journal or FULL synchronous mode was not retained",
      );
    }
    succeeded = true;
    return {
      configuration,
      sqlite,
      pool,
      database,
      openTestTransaction: false,
    };
  } finally {
    if (!succeeded) {
      database.close();
      pool.pauseVfs();
    }
  }
}

async function loadProfileSeeds(
  source: SourceState,
  configuration: BrowserWritableProfileConfiguration,
): Promise<Map<number, Uint8Array>> {
  const seeds = new Map<number, Uint8Array>();
  let totalBytes = 0;
  for (const descriptor of configuration.files) {
    if (descriptor.seed === undefined) {
      continue;
    }
    const bytes = await readVerifiedSeed(source.mount.directoryHandle, descriptor.seed);
    seeds.set(descriptor.objectId, bytes);
    totalBytes += bytes.byteLength;
  }
  if (totalBytes > configuration.quotaBytes) {
    throw new StorageFault(
      "browser-profile-seed-quota-exceeded",
      "profile seed content exceeds its quota",
    );
  }
  return seeds;
}

function createProfileSchema(
  database: Database,
  configuration: BrowserWritableProfileConfiguration,
  seeds: Map<number, Uint8Array>,
  sessionKey: string,
): void {
  database.transaction("IMMEDIATE", () => {
    database.exec(profileSchemaSql);
    execute(database, "PRAGMA application_id = 1447644741");
    execute(database, `PRAGMA user_version = ${sqliteSchemaVersion}`);
    execute(
      database,
      `INSERT INTO profile_metadata(
        singleton, schema_version, title_fingerprint, profile_id, root_path_a,
        quota_bytes, lease_epoch
      ) VALUES(1, ?, ?, ?, ?, ?, 1)`,
      [
        sqliteSchemaVersion,
        configuration.titleFingerprint,
        configuration.profileId,
        Uint8Array.from(configuration.rootPathA),
        configuration.quotaBytes,
      ],
    );
    for (const descriptor of configuration.files) {
      const seed = seeds.get(descriptor.objectId);
      const exists = seed === undefined ? 0 : 1;
      const digest = seed === undefined ? null : hex(sha256(seed));
      execute(
        database,
        `INSERT INTO overlay_files(
          object_id, relative_path_a, is_present, generation, byte_length, sha256, content
        ) VALUES(?, ?, ?, ?, ?, ?, ?)`,
        [
          descriptor.objectId,
          Uint8Array.from(descriptor.relativePathA),
          exists,
          exists,
          seed?.byteLength ?? 0,
          digest,
          seed ?? null,
        ],
      );
    }
    for (const descriptor of configuration.directories) {
      execute(
        database,
        `INSERT INTO overlay_directories(
          directory_id, relative_path_a, is_present, generation
        ) VALUES(?, ?, 0, 0)`,
        [descriptor.directoryId, Uint8Array.from(descriptor.relativePathA)],
      );
    }
    execute(
      database,
      `INSERT INTO profile_lease(singleton, epoch, session_key, active)
       VALUES(1, 1, ?, 1)`,
      [sessionKey],
    );
  });
}

function validateExistingProfile(
  database: Database,
  configuration: BrowserWritableProfileConfiguration,
): void {
  const metadata = database.selectObject(
    `SELECT schema_version, title_fingerprint, profile_id, root_path_a, quota_bytes
     FROM profile_metadata WHERE singleton = 1`,
  );
  if (
    metadata === undefined ||
    integerValue(metadata.schema_version) !== sqliteSchemaVersion ||
    metadata.title_fingerprint !== configuration.titleFingerprint ||
    metadata.profile_id !== configuration.profileId ||
    !bytesEqual(blobValue(metadata.root_path_a), configuration.rootPathA) ||
    integerValue(metadata.quota_bytes) !== configuration.quotaBytes
  ) {
    throw new StorageFault(
      "browser-profile-configuration-mismatch",
      "stored profile identity differs from the requested profile",
    );
  }
  const files = database.selectObjects(
    "SELECT object_id, relative_path_a FROM overlay_files ORDER BY object_id",
  );
  if (
    files.length !== configuration.files.length ||
    files.some((row, index) => {
      const expected = configuration.files[index];
      return (
        expected === undefined ||
        integerValue(row.object_id) !== expected.objectId ||
        !bytesEqual(blobValue(row.relative_path_a), expected.relativePathA)
      );
    })
  ) {
    throw new StorageFault(
      "browser-profile-file-allowlist-mismatch",
      "stored profile file allowlist differs from the requested profile",
    );
  }
  const directories = database.selectObjects(
    "SELECT directory_id, relative_path_a FROM overlay_directories ORDER BY directory_id",
  );
  if (
    directories.length !== configuration.directories.length ||
    directories.some((row, index) => {
      const expected = configuration.directories[index];
      return (
        expected === undefined ||
        integerValue(row.directory_id) !== expected.directoryId ||
        !bytesEqual(blobValue(row.relative_path_a), expected.relativePathA)
      );
    })
  ) {
    throw new StorageFault(
      "browser-profile-directory-allowlist-mismatch",
      "stored profile directory allowlist differs from the requested profile",
    );
  }
}

function advanceProfileLease(database: Database, sessionKey: string): void {
  database.transaction("IMMEDIATE", () => {
    const previous = integerValue(
      database.selectValue(
        "SELECT lease_epoch FROM profile_metadata WHERE singleton = 1",
      ),
    );
    if (previous <= 0 || previous >= maximumGeneration) {
      throw new StorageFault(
        "browser-profile-lease-exhausted",
        "profile lease epoch cannot advance",
      );
    }
    const epoch = previous + 1;
    execute(
      database,
      "UPDATE profile_metadata SET lease_epoch = ? WHERE singleton = 1",
      [epoch],
    );
    execute(
      database,
      `INSERT INTO profile_lease(singleton, epoch, session_key, active)
       VALUES(1, ?, ?, 1)
       ON CONFLICT(singleton) DO UPDATE SET
         epoch = excluded.epoch,
         session_key = excluded.session_key,
         active = 1`,
      [epoch, sessionKey],
    );
  });
}

function profileSessionEpoch(database: Database): number {
  return integerValue(
    database.selectValue(
      `SELECT lease_epoch FROM profile_metadata WHERE singleton = 1`,
    ),
  );
}

async function readSourceRange(
  source: SourceState,
  sourceId: number,
  objectId: number,
  offset: number,
  requestedBytes: number,
): Promise<Uint8Array> {
  if (sourceId !== source.mount.sourceId) {
    throw new StorageFault(
      "browser-source-id-mismatch",
      "source read uses an unknown mount identity",
    );
  }
  const descriptor = source.files.get(objectId);
  if (descriptor === undefined) {
    throw new StorageFault(
      "browser-source-object-unknown",
      "source read uses an unknown object identity",
    );
  }
  validateRange(offset, requestedBytes, descriptor.byteLength, "source read");
  const handle = await resolveSourceFile(
    source.mount.directoryHandle,
    descriptor.pathSegments,
  );
  const before = await handle.getFile();
  assertSourceIdentity(descriptor, before);
  const transferred = Math.min(requestedBytes, descriptor.byteLength - offset);
  const bytes = new Uint8Array(
    await before.slice(offset, offset + transferred).arrayBuffer(),
  );
  const after = await handle.getFile();
  assertSourceIdentity(descriptor, after);
  if (bytes.byteLength !== transferred) {
    throw new StorageFault(
      "browser-source-short-read",
      "source object ended before the requested range",
    );
  }
  return bytes;
}

function profileSnapshot(profile: ProfileState): WritableProfileSnapshot {
  const database = profile.database;
  assertActiveLease(database, requireActiveState());
  const files = database.selectObjects(
    `SELECT object_id, relative_path_a, is_present, generation, byte_length, sha256
     FROM overlay_files ORDER BY object_id`,
  );
  const directories = database.selectObjects(
    `SELECT directory_id, relative_path_a, is_present, generation
     FROM overlay_directories ORDER BY directory_id`,
  );
  return {
    schema: "vine.writable-profile-snapshot",
    schemaVersion: 3,
    titleFingerprint: profile.configuration.titleFingerprint,
    profileId: profile.configuration.profileId,
    rootPathA: [...profile.configuration.rootPathA],
    quotaBytes: profile.configuration.quotaBytes,
    files: files.map((row) => ({
      objectId: integerValue(row.object_id),
      relativePathA: [...blobValue(row.relative_path_a)],
      exists: integerValue(row.is_present) === 1,
      generation: integerValue(row.generation),
      byteLength: integerValue(row.byte_length),
      sha256: nullableText(row.sha256),
    })),
    directories: directories.map((row) => ({
      directoryId: integerValue(row.directory_id),
      relativePathA: [...blobValue(row.relative_path_a)],
      exists: integerValue(row.is_present) === 1,
      generation: integerValue(row.generation),
    })),
  };
}

function profileRead(
  active: StorageState,
  objectId: number,
  generation: number,
  offset: number,
  requestedBytes: number,
): Uint8Array {
  const profile = requireProfile(active);
  assertActiveLease(profile.database, active);
  const row = profile.database.selectObject(
    `SELECT is_present, generation, byte_length, content
     FROM overlay_files WHERE object_id = ?`,
    [objectId],
  );
  if (row === undefined || integerValue(row.is_present) !== 1) {
    throw new StorageFault(
      "browser-profile-file-missing",
      "writable profile object is not committed",
    );
  }
  const committedGeneration = integerValue(row.generation);
  if (generation !== committedGeneration) {
    throw new StorageFault(
      "writable-profile-read-generation-stale",
      "writable profile read generation is stale",
    );
  }
  const byteLength = integerValue(row.byte_length);
  validateRange(offset, requestedBytes, byteLength, "profile read");
  const content = blobValue(row.content);
  if (content.byteLength !== byteLength) {
    throw new StorageFault(
      "browser-profile-content-invalid",
      "writable profile content length differs from metadata",
    );
  }
  return content.slice(offset, Math.min(offset + requestedBytes, byteLength));
}

function profileCommit(
  active: StorageState,
  request: Extract<BrowserStorageRequest, { type: "profile-commit" }>,
): Extract<SuccessfulResult, { kind: "profile-commit" }> {
  const profile = requireProfile(active);
  validateCommitInput(request);
  const digest = hex(sha256(request.bytes));
  const deleteFile = request.deleteFile === true;
  if (
    deleteFile
      ? request.bytes.byteLength !== 0 ||
        request.contentSha256 !== writableDeleteMarkerSha256
      : digest !== request.contentSha256
  ) {
    throw new StorageFault(
      "writable-profile-commit-content-mismatch",
      "writable profile commit hash differs from its bytes",
    );
  }
  return profile.database.transaction("IMMEDIATE", () => {
    assertActiveLease(profile.database, active);
    const duplicate = profile.database.selectObject(
      `SELECT object_id, base_generation, committed_generation, byte_length, content_sha256
       FROM commit_ledger WHERE request_key = ?`,
      [request.requestKey],
    );
    if (duplicate !== undefined) {
      if (
        integerValue(duplicate.object_id) !== request.objectId ||
        integerValue(duplicate.base_generation) !== request.baseGeneration ||
        integerValue(duplicate.byte_length) !== request.bytes.byteLength ||
        duplicate.content_sha256 !== request.contentSha256
      ) {
        throw new StorageFault(
          "writable-profile-request-key-conflict",
          "writable profile request key was reused for different content",
        );
      }
      return {
        kind: "profile-commit",
        objectId: request.objectId,
        baseGeneration: request.baseGeneration,
        committedGeneration: integerValue(duplicate.committed_generation),
        duplicate: true,
      };
    }
    const file = profile.database.selectObject(
      `SELECT is_present, generation, byte_length FROM overlay_files WHERE object_id = ?`,
      [request.objectId],
    );
    if (file === undefined) {
      throw new StorageFault(
        "writable-profile-object-unknown",
        "writable profile object is not allowlisted",
      );
    }
    const currentGeneration = integerValue(file.generation);
    if (
      (deleteFile && integerValue(file.is_present) !== 1) ||
      currentGeneration !== request.baseGeneration
    ) {
      throw new StorageFault(
        "writable-profile-generation-stale",
        "writable profile base generation is stale",
      );
    }
    const totalBytes = integerValue(
      profile.database.selectValue("SELECT COALESCE(SUM(byte_length), 0) FROM overlay_files"),
    );
    const nextTotal =
      totalBytes - integerValue(file.byte_length) + request.bytes.byteLength;
    if (nextTotal > profile.configuration.quotaBytes) {
      throw new StorageFault(
        "writable-profile-quota-exceeded",
        "writable profile commit exceeds its quota",
      );
    }
    if (currentGeneration >= maximumGeneration) {
      throw new StorageFault(
        "writable-profile-generation-exhausted",
        "writable profile generation cannot advance",
      );
    }
    const committedGeneration = currentGeneration + 1;
    if (deleteFile) {
      execute(
        profile.database,
        `UPDATE overlay_files
         SET is_present = 0, generation = 0, byte_length = 0, sha256 = NULL, content = NULL
         WHERE object_id = ?`,
        [request.objectId],
      );
    } else {
      execute(
        profile.database,
        `UPDATE overlay_files
         SET is_present = 1, generation = ?, byte_length = ?, sha256 = ?, content = ?
         WHERE object_id = ?`,
        [
          committedGeneration,
          request.bytes.byteLength,
          request.contentSha256,
          request.bytes,
          request.objectId,
        ],
      );
    }
    execute(
      profile.database,
      `INSERT INTO commit_ledger(
        request_key, object_id, base_generation, committed_generation,
        byte_length, content_sha256
      ) VALUES(?, ?, ?, ?, ?, ?)`,
      [
        request.requestKey,
        request.objectId,
        request.baseGeneration,
        committedGeneration,
        request.bytes.byteLength,
        request.contentSha256,
      ],
    );
    return {
      kind: "profile-commit",
      objectId: request.objectId,
      baseGeneration: request.baseGeneration,
      committedGeneration,
      duplicate: false,
    };
  });
}

function profileDirectoryCreate(
  active: StorageState,
  request: Extract<BrowserStorageRequest, { type: "profile-directory-create" }>,
): Extract<SuccessfulResult, { kind: "profile-directory-create" }> {
  const profile = requireProfile(active);
  validateRequestKey(request.requestKey);
  validateGeneration(request.baseGeneration, "directory base generation");
  return profile.database.transaction("IMMEDIATE", () => {
    assertActiveLease(profile.database, active);
    const duplicate = profile.database.selectObject(
      `SELECT directory_id, base_generation, committed_generation
       FROM directory_commit_ledger WHERE request_key = ?`,
      [request.requestKey],
    );
    if (duplicate !== undefined) {
      if (
        integerValue(duplicate.directory_id) !== request.directoryId ||
        integerValue(duplicate.base_generation) !== request.baseGeneration
      ) {
        throw new StorageFault(
          "writable-profile-directory-request-key-conflict",
          "directory request key was reused for a different operation",
        );
      }
      return {
        kind: "profile-directory-create",
        directoryId: request.directoryId,
        baseGeneration: request.baseGeneration,
        committedGeneration: integerValue(duplicate.committed_generation),
        duplicate: true,
      };
    }
    const directory = profile.database.selectObject(
      `SELECT is_present, generation FROM overlay_directories WHERE directory_id = ?`,
      [request.directoryId],
    );
    if (directory === undefined) {
      throw new StorageFault(
        "writable-profile-directory-unknown",
        "writable profile directory is not allowlisted",
      );
    }
    const generation = integerValue(directory.generation);
    if (integerValue(directory.is_present) !== 0 || generation !== request.baseGeneration) {
      throw new StorageFault(
        "writable-profile-directory-generation-stale",
        "writable profile directory base state is stale",
      );
    }
    if (generation >= maximumGeneration) {
      throw new StorageFault(
        "writable-profile-directory-generation-exhausted",
        "writable profile directory generation cannot advance",
      );
    }
    const committedGeneration = generation + 1;
    execute(
      profile.database,
      `UPDATE overlay_directories SET is_present = 1, generation = ?
       WHERE directory_id = ?`,
      [committedGeneration, request.directoryId],
    );
    execute(
      profile.database,
      `INSERT INTO directory_commit_ledger(
        request_key, directory_id, base_generation, committed_generation
      ) VALUES(?, ?, ?, ?)`,
      [
        request.requestKey,
        request.directoryId,
        request.baseGeneration,
        committedGeneration,
      ],
    );
    return {
      kind: "profile-directory-create",
      directoryId: request.directoryId,
      baseGeneration: request.baseGeneration,
      committedGeneration,
      duplicate: false,
    };
  });
}

function beginUncommittedMutation(
  active: StorageState,
  request: Extract<BrowserStorageRequest, { type: "test-begin-uncommitted" }>,
): void {
  const profile = requireProfile(active);
  if (profile.openTestTransaction) {
    throw new StorageFault(
      "browser-storage-test-transaction-active",
      "test transaction is already active",
    );
  }
  validateGeneration(request.baseGeneration, "test base generation");
  if (!sha256Pattern.test(request.contentSha256)) {
    throw new StorageFault(
      "browser-storage-test-hash-invalid",
      "test content hash is invalid",
    );
  }
  if (hex(sha256(request.bytes)) !== request.contentSha256) {
    throw new StorageFault(
      "browser-storage-test-content-mismatch",
      "test content differs from its hash",
    );
  }
  profile.database.exec("BEGIN IMMEDIATE");
  try {
    assertActiveLease(profile.database, active);
    const file = profile.database.selectObject(
      "SELECT generation, byte_length FROM overlay_files WHERE object_id = ?",
      [request.objectId],
    );
    if (
      file === undefined ||
      integerValue(file.generation) !== request.baseGeneration
    ) {
      throw new StorageFault(
        "writable-profile-generation-stale",
        "test mutation base generation is stale",
      );
    }
    const totalBytes = integerValue(
      profile.database.selectValue("SELECT COALESCE(SUM(byte_length), 0) FROM overlay_files"),
    );
    if (
      totalBytes - integerValue(file.byte_length) + request.bytes.byteLength >
      profile.configuration.quotaBytes
    ) {
      throw new StorageFault(
        "writable-profile-quota-exceeded",
        "test mutation exceeds the profile quota",
      );
    }
    execute(
      profile.database,
      `UPDATE overlay_files
       SET is_present = 1, generation = ?, byte_length = ?, sha256 = ?, content = ?
       WHERE object_id = ?`,
      [
        request.baseGeneration + 1,
        request.bytes.byteLength,
        request.contentSha256,
        request.bytes,
        request.objectId,
      ],
    );
    profile.openTestTransaction = true;
  } catch (error) {
    profile.database.exec("ROLLBACK");
    throw error;
  }
}

async function closeStorage(active: StorageState): Promise<void> {
  if (active.closed) {
    return;
  }
  const profile = active.profile;
  if (profile !== null) {
    if (profile.openTestTransaction) {
      profile.database.exec("ROLLBACK");
      profile.openTestTransaction = false;
    }
    profile.database.transaction("IMMEDIATE", () => {
      assertActiveLease(profile.database, active);
      execute(
        profile.database,
        `UPDATE profile_lease SET active = 0
         WHERE singleton = 1 AND epoch = ? AND session_key = ?`,
        [active.sessionEpoch, active.sessionKey],
      );
    });
    await cleanupProfile(profile);
    active.profile = null;
  }
  active.closed = true;
  state = null;
  active.releaseLock?.();
  active.releaseLock = null;
}

async function cleanupProfile(profile: ProfileState | null): Promise<void> {
  if (profile === null) {
    return;
  }
  if (profile.database.isOpen()) {
    if (profile.openTestTransaction) {
      try {
        profile.database.exec("ROLLBACK");
      } catch {}
      profile.openTestTransaction = false;
    }
    profile.database.close();
  }
  if (!profile.pool.isPaused()) {
    profile.pool.pauseVfs();
  }
}

function assertActiveLease(database: Database, active: StorageState): void {
  const lease = database.selectObject(
    `SELECT epoch, session_key, active FROM profile_lease WHERE singleton = 1`,
  );
  if (
    lease === undefined ||
    integerValue(lease.epoch) !== active.sessionEpoch ||
    lease.session_key !== active.sessionKey ||
    integerValue(lease.active) !== 1
  ) {
    throw new StorageFault(
      "writable-profile-lease-revoked",
      "writable profile lease is no longer active",
    );
  }
}

function requireSession(
  request: Exclude<BrowserStorageRequest, { type: "initialize" }>,
): StorageState {
  const active = requireActiveState();
  if (
    active.closed ||
    request.sessionKey !== active.sessionKey ||
    request.sessionEpoch !== active.sessionEpoch
  ) {
    throw new StorageFault(
      "writable-profile-lease-revoked",
      "storage request uses a stale session lease",
    );
  }
  return active;
}

function requireActiveState(): StorageState {
  if (state === null || state.closed) {
    throw new StorageFault(
      "browser-storage-not-initialized",
      "storage worker has no active session",
    );
  }
  return state;
}

function requireProfile(active: StorageState): ProfileState {
  if (active.profile === null) {
    throw new StorageFault(
      "browser-profile-not-configured",
      "storage session has no writable profile",
    );
  }
  return active.profile;
}

function requireTestFaults(active: StorageState): void {
  if (!active.allowTestFaults) {
    throw new StorageFault(
      "browser-storage-test-fault-disabled",
      "storage failure injection is disabled",
    );
  }
}

function initializationResult(
  active: StorageState,
  sqliteVersion: string | null,
): BrowserStorageInitializationResult {
  return {
    kind: "initialized",
    titleFingerprint: active.source.mount.titleFingerprint,
    snapshotId: active.source.mount.snapshotId,
    sourceId: active.source.mount.sourceId,
    profileId: active.profile?.configuration.profileId ?? null,
    sessionEpoch: active.sessionEpoch,
    sqliteVersion,
    storageBackend: "file-system-access+opfs-sahpool-sqlite",
  };
}

function validateEnvelope(request: BrowserStorageRequest): void {
  if (
    request === null ||
    typeof request !== "object" ||
    request.protocolVersion !== browserStorageProtocolVersion ||
    !/^(0|[1-9][0-9]*)$/u.test(request.requestId)
  ) {
    throw new StorageFault(
      "browser-storage-request-invalid",
      "storage request envelope is invalid",
    );
  }
}

function validateConfiguration(configuration: BrowserNativeStorageConfiguration): void {
  if (
    configuration === null ||
    typeof configuration !== "object" ||
    configuration.schema !== browserNativeStorageSchema ||
    configuration.schemaVersion !== browserNativeStorageSchemaVersion ||
    (configuration.allowTestFaults !== undefined &&
      typeof configuration.allowTestFaults !== "boolean")
  ) {
    throw new StorageFault(
      "browser-storage-configuration-invalid",
      "browser-native storage configuration is invalid",
    );
  }
}

function validateSourceMount(mount: BrowserSourceMount): void {
  if (
    mount.schema !== browserSourceMountSchema ||
    mount.schemaVersion !== browserSourceMountSchemaVersion ||
    !simpleIdentityPattern.test(mount.mountId) ||
    !identityPattern.test(mount.titleFingerprint) ||
    !simpleIdentityPattern.test(mount.snapshotId) ||
    !Number.isInteger(mount.sourceId) ||
    mount.sourceId <= 0 ||
    !validByteArray(mount.rootPathA, true) ||
    mount.directoryHandle?.kind !== "directory" ||
    !Array.isArray(mount.files) ||
    mount.files.length === 0 ||
    mount.files.length > 4096
  ) {
    throw new StorageFault(
      "browser-source-mount-invalid",
      "browser source mount metadata is invalid",
    );
  }
  const objectIds = new Set<number>();
  for (const file of mount.files) {
    if (
      !Number.isInteger(file.objectId) ||
      file.objectId <= 0 ||
      objectIds.has(file.objectId) ||
      !validByteArray(file.relativePathA, false) ||
      !validPathSegments(file.pathSegments) ||
      !Number.isSafeInteger(file.byteLength) ||
      file.byteLength < 0 ||
      !sha256Pattern.test(file.sha256) ||
      !Number.isSafeInteger(file.lastModified) ||
      file.lastModified < 0
    ) {
      throw new StorageFault(
        "browser-source-file-invalid",
        "browser source file descriptor is invalid or duplicated",
      );
    }
    objectIds.add(file.objectId);
  }
}

function validateProfileConfiguration(
  configuration: BrowserWritableProfileConfiguration,
  sourceTitleFingerprint: string,
): void {
  if (
    configuration.schema !== browserWritableProfileSchema ||
    configuration.schemaVersion !== browserWritableProfileSchemaVersion ||
    !identityPattern.test(configuration.titleFingerprint) ||
    configuration.titleFingerprint !== sourceTitleFingerprint ||
    !simpleIdentityPattern.test(configuration.profileId) ||
    !validByteArray(configuration.rootPathA, true) ||
    !Number.isSafeInteger(configuration.quotaBytes) ||
    configuration.quotaBytes <= 0 ||
    configuration.quotaBytes > maximumProfileQuotaBytes ||
    !Array.isArray(configuration.files) ||
    configuration.files.length === 0 ||
    configuration.files.length > 128 ||
    !Array.isArray(configuration.directories) ||
    configuration.directories.length > 64
  ) {
    throw new StorageFault(
      "browser-profile-configuration-invalid",
      "browser writable profile configuration is invalid",
    );
  }
  const objectIds = new Set<number>();
  const directoryIds = new Set<number>();
  const paths = new Set<string>();
  for (const file of configuration.files) {
    const pathIdentity = foldedPathIdentity(file.relativePathA);
    if (
      !Number.isInteger(file.objectId) ||
      file.objectId <= 0 ||
      objectIds.has(file.objectId) ||
      !validByteArray(file.relativePathA, false) ||
      paths.has(pathIdentity) ||
      (file.seed !== undefined && !validSeed(file.seed))
    ) {
      throw new StorageFault(
        "browser-profile-file-invalid",
        "browser writable profile file is invalid or duplicated",
      );
    }
    objectIds.add(file.objectId);
    paths.add(pathIdentity);
  }
  for (const directory of configuration.directories) {
    const pathIdentity = foldedPathIdentity(directory.relativePathA);
    if (
      !Number.isInteger(directory.directoryId) ||
      directory.directoryId <= 0 ||
      directoryIds.has(directory.directoryId) ||
      !validByteArray(directory.relativePathA, false) ||
      paths.has(pathIdentity)
    ) {
      throw new StorageFault(
        "browser-profile-directory-invalid",
        "browser writable profile directory is invalid or duplicated",
      );
    }
    directoryIds.add(directory.directoryId);
    paths.add(pathIdentity);
  }
}

function validSeed(seed: BrowserProfileSeed): boolean {
  return (
    validPathSegments(seed.pathSegments) &&
    Number.isSafeInteger(seed.byteLength) &&
    seed.byteLength >= 0 &&
    sha256Pattern.test(seed.sha256)
  );
}

function validateCommitInput(
  request: Extract<BrowserStorageRequest, { type: "profile-commit" }>,
): void {
  validateRequestKey(request.requestKey);
  validateGeneration(request.baseGeneration, "commit base generation");
  if (
    !Number.isInteger(request.objectId) ||
    request.objectId <= 0 ||
    !(request.bytes instanceof Uint8Array) ||
    !sha256Pattern.test(request.contentSha256) ||
    (request.deleteFile !== undefined && typeof request.deleteFile !== "boolean")
  ) {
    throw new StorageFault(
      "writable-profile-commit-invalid",
      "writable profile commit metadata is invalid",
    );
  }
}

function validateRequestKey(requestKey: string): void {
  if (!requestKeyPattern.test(requestKey)) {
    throw new StorageFault(
      "writable-profile-request-key-invalid",
      "writable profile request key is invalid",
    );
  }
}

function validateSessionKey(sessionKey: string): void {
  if (!simpleIdentityPattern.test(sessionKey)) {
    throw new StorageFault(
      "browser-storage-session-key-invalid",
      "browser storage session key is invalid",
    );
  }
}

function validateGeneration(generation: number, label: string): void {
  if (!Number.isSafeInteger(generation) || generation < 0) {
    throw new StorageFault(
      "writable-profile-generation-invalid",
      `${label} is invalid`,
    );
  }
}

function validateRange(
  offset: number,
  requestedBytes: number,
  byteLength: number,
  label: string,
): void {
  if (
    !Number.isSafeInteger(offset) ||
    offset < 0 ||
    offset > byteLength ||
    !Number.isInteger(requestedBytes) ||
    requestedBytes <= 0 ||
    requestedBytes > maximumTransportBytes
  ) {
    throw new StorageFault(
      "browser-storage-range-invalid",
      `${label} range is invalid`,
    );
  }
}

async function resolveSourceFile(
  root: FileSystemDirectoryHandle,
  pathSegments: string[],
): Promise<FileSystemFileHandle> {
  if (!validPathSegments(pathSegments)) {
    throw new StorageFault(
      "browser-source-path-invalid",
      "source host path binding is invalid",
    );
  }
  let directory = root;
  for (const segment of pathSegments.slice(0, -1)) {
    directory = await directory.getDirectoryHandle(segment, { create: false });
  }
  const filename = pathSegments.at(-1);
  if (filename === undefined) {
    throw new StorageFault("browser-source-path-invalid", "source host path is empty");
  }
  return directory.getFileHandle(filename, { create: false });
}

async function readVerifiedSeed(
  root: FileSystemDirectoryHandle,
  seed: BrowserProfileSeed,
): Promise<Uint8Array> {
  const handle = await resolveSourceFile(root, seed.pathSegments);
  const before = await handle.getFile();
  if (before.size !== seed.byteLength) {
    throw new StorageFault(
      "browser-profile-seed-length-mismatch",
      "profile seed byte length changed",
    );
  }
  const bytes = new Uint8Array(await before.arrayBuffer());
  const after = await handle.getFile();
  if (
    before.size !== after.size ||
    before.lastModified !== after.lastModified ||
    hex(sha256(bytes)) !== seed.sha256
  ) {
    throw new StorageFault(
      "browser-profile-seed-identity-mismatch",
      "profile seed identity changed",
    );
  }
  return bytes;
}

function assertSourceIdentity(descriptor: BrowserSourceFile, file: File): void {
  if (
    file.size !== descriptor.byteLength ||
    file.lastModified !== descriptor.lastModified
  ) {
    throw new StorageFault(
      "browser-source-object-changed",
      `source object ${descriptor.objectId} changed after mount verification`,
    );
  }
}

function execute(database: Database, sql: string, bindings?: readonly unknown[]): void {
  if (bindings === undefined) {
    database.exec(sql);
    return;
  }
  const statement = database.prepare(sql);
  try {
    statement.bind(bindings as never).stepFinalize();
  } finally {
    statement.finalize();
  }
}

function integerValue(value: unknown): number {
  const numeric = typeof value === "bigint" ? Number(value) : value;
  if (!Number.isSafeInteger(numeric)) {
    throw new StorageFault(
      "browser-profile-database-content-invalid",
      "SQLite integer is outside the JavaScript safety bound",
    );
  }
  return numeric as number;
}

function blobValue(value: unknown): Uint8Array {
  if (!(value instanceof Uint8Array)) {
    throw new StorageFault(
      "browser-profile-database-content-invalid",
      "SQLite value is not a BLOB",
    );
  }
  return value;
}

function nullableText(value: unknown): string | null {
  if (value !== null && typeof value !== "string") {
    throw new StorageFault(
      "browser-profile-database-content-invalid",
      "SQLite value is not nullable text",
    );
  }
  return value;
}

function bytesEqual(left: Uint8Array, right: number[]): boolean {
  return left.byteLength === right.length && left.every((byte, index) => byte === right[index]);
}

function validByteArray(value: unknown, trailingNul: boolean): value is number[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.length <= 32_768 &&
    value.every(
      (byte) => Number.isInteger(byte) && (byte as number) >= 0 && (byte as number) <= 0xff,
    ) &&
    (trailingNul
      ? value.at(-1) === 0 && !value.slice(0, -1).includes(0)
      : !value.includes(0))
  );
}

function validPathSegments(segments: string[]): boolean {
  return (
    Array.isArray(segments) &&
    segments.length > 0 &&
    segments.length <= 256 &&
    segments.every(
      (segment) =>
        typeof segment === "string" &&
        segment.length > 0 &&
        segment.length <= 255 &&
        segment !== "." &&
        segment !== ".." &&
        !segment.includes("/") &&
        !segment.includes("\\") &&
        !segment.includes("\0"),
    )
  );
}

function foldedPathIdentity(bytes: number[]): string {
  if (!validByteArray(bytes, false)) {
    return "invalid";
  }
  const folded: number[] = [];
  for (let index = 0; index < bytes.length; index += 1) {
    const byte = bytes[index] ?? 0;
    const trail = bytes[index + 1];
    if (isWindows932LeadByte(byte) && isWindows932TrailByte(trail)) {
      folded.push(byte, trail ?? 0);
      index += 1;
    } else {
      folded.push(byte >= 0x41 && byte <= 0x5a ? byte + 0x20 : byte);
    }
  }
  return folded.join(",");
}

function isWindows932LeadByte(byte: number): boolean {
  return (byte >= 0x81 && byte <= 0x9f) || (byte >= 0xe0 && byte <= 0xfc);
}

function isWindows932TrailByte(byte: number | undefined): boolean {
  return (
    byte !== undefined &&
    ((byte >= 0x40 && byte <= 0x7e) || (byte >= 0x80 && byte <= 0xfc)) &&
    byte !== 0x7f
  );
}

function postSuccess(requestId: string, result: SuccessfulResult): void {
  const response = {
    protocolVersion: browserStorageProtocolVersion,
    requestId,
    type: "success",
    result,
  } satisfies BrowserStorageResponse;
  const transfer: Transferable[] = [];
  if (result.kind === "source-read" || result.kind === "profile-read") {
    transfer.push(result.bytes.buffer);
  }
  workerScope.postMessage(response, transfer);
}

function postFailure(requestId: string, fault: StorageFault): void {
  workerScope.postMessage({
    protocolVersion: browserStorageProtocolVersion,
    requestId,
    type: "error",
    code: fault.code,
    message: fault.message,
  } satisfies BrowserStorageResponse);
}

function requestIdFrom(error: unknown, request: unknown): string {
  if (
    request !== null &&
    typeof request === "object" &&
    "requestId" in request &&
    typeof request.requestId === "string"
  ) {
    return request.requestId;
  }
  return error instanceof StorageFault ? "0" : "0";
}

function asStorageFault(error: unknown): StorageFault {
  if (error instanceof StorageFault) {
    return error;
  }
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("SQLITE_BUSY") || message.includes("SQLITE_LOCKED")) {
    return new StorageFault("SQLITE_BUSY", message);
  }
  if (message.includes("SQLITE_FULL")) {
    return new StorageFault("SQLITE_FULL", message);
  }
  return new StorageFault("browser-storage-failure", message);
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

const profileSchemaSql = `
CREATE TABLE profile_metadata (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  schema_version INTEGER NOT NULL,
  title_fingerprint TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  root_path_a BLOB NOT NULL,
  quota_bytes INTEGER NOT NULL CHECK(quota_bytes > 0),
  lease_epoch INTEGER NOT NULL CHECK(lease_epoch > 0)
) STRICT;
CREATE TABLE overlay_files (
  object_id INTEGER PRIMARY KEY CHECK(object_id > 0),
  relative_path_a BLOB NOT NULL UNIQUE,
  is_present INTEGER NOT NULL CHECK(is_present IN (0, 1)),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
  sha256 TEXT,
  content BLOB,
  CHECK(
    (is_present = 0 AND generation = 0 AND byte_length = 0 AND sha256 IS NULL AND content IS NULL)
    OR
    (is_present = 1 AND generation > 0 AND length(sha256) = 64 AND length(content) = byte_length)
  )
) STRICT;
CREATE TABLE overlay_directories (
  directory_id INTEGER PRIMARY KEY CHECK(directory_id > 0),
  relative_path_a BLOB NOT NULL UNIQUE,
  is_present INTEGER NOT NULL CHECK(is_present IN (0, 1)),
  generation INTEGER NOT NULL CHECK(generation >= 0),
  CHECK((is_present = 0 AND generation = 0) OR (is_present = 1 AND generation > 0))
) STRICT;
CREATE TABLE overlay_tombstones (
  relative_path_a BLOB PRIMARY KEY,
  generation INTEGER NOT NULL CHECK(generation > 0)
) STRICT;
CREATE TABLE registry_keys (
  key_id INTEGER PRIMARY KEY CHECK(key_id > 0),
  parent_key_id INTEGER REFERENCES registry_keys(key_id),
  name_a BLOB NOT NULL,
  generation INTEGER NOT NULL CHECK(generation > 0),
  UNIQUE(parent_key_id, name_a)
) STRICT;
CREATE TABLE registry_values (
  key_id INTEGER NOT NULL REFERENCES registry_keys(key_id) ON DELETE CASCADE,
  name_a BLOB NOT NULL,
  value_type INTEGER NOT NULL,
  value_bytes BLOB NOT NULL,
  generation INTEGER NOT NULL CHECK(generation > 0),
  PRIMARY KEY(key_id, name_a)
) STRICT;
CREATE TABLE overlay_metadata (
  key TEXT PRIMARY KEY,
  value BLOB NOT NULL
) STRICT;
CREATE TABLE commit_ledger (
  request_key TEXT PRIMARY KEY,
  object_id INTEGER NOT NULL REFERENCES overlay_files(object_id),
  base_generation INTEGER NOT NULL CHECK(base_generation >= 0),
  committed_generation INTEGER NOT NULL CHECK(committed_generation > base_generation),
  byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
  content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64)
) STRICT;
CREATE TABLE directory_commit_ledger (
  request_key TEXT PRIMARY KEY,
  directory_id INTEGER NOT NULL REFERENCES overlay_directories(directory_id),
  base_generation INTEGER NOT NULL CHECK(base_generation >= 0),
  committed_generation INTEGER NOT NULL CHECK(committed_generation > base_generation)
) STRICT;
CREATE TABLE profile_lease (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  epoch INTEGER NOT NULL CHECK(epoch > 0),
  session_key TEXT NOT NULL,
  active INTEGER NOT NULL CHECK(active IN (0, 1))
) STRICT;
`;
