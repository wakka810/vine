export const browserStorageProtocolVersion = 1 as const;
export const browserSourceMountSchema = "vine.browser-source-mount" as const;
export const browserSourceMountSchemaVersion = 1 as const;
export const browserWritableProfileSchema =
  "vine.browser-writable-profile" as const;
export const browserWritableProfileSchemaVersion = 1 as const;
export const browserNativeStorageSchema = "vine.browser-native-storage" as const;
export const browserNativeStorageSchemaVersion = 1 as const;

export interface BrowserSourceFile {
  objectId: number;
  relativePathA: number[];
  pathSegments: string[];
  byteLength: number;
  sha256: string;
  lastModified: number;
}

export interface BrowserSourceMount {
  schema: typeof browserSourceMountSchema;
  schemaVersion: typeof browserSourceMountSchemaVersion;
  mountId: string;
  titleFingerprint: string;
  snapshotId: string;
  sourceId: number;
  rootPathA: number[];
  directoryHandle: FileSystemDirectoryHandle;
  verifiedAt: string;
  files: BrowserSourceFile[];
}

export interface BrowserProfileSeed {
  pathSegments: string[];
  byteLength: number;
  sha256: string;
}

export interface BrowserWritableProfileFile {
  objectId: number;
  relativePathA: number[];
  seed?: BrowserProfileSeed;
}

export interface BrowserWritableProfileDirectory {
  directoryId: number;
  relativePathA: number[];
}

export interface BrowserWritableProfileConfiguration {
  schema: typeof browserWritableProfileSchema;
  schemaVersion: typeof browserWritableProfileSchemaVersion;
  titleFingerprint: string;
  profileId: string;
  rootPathA: number[];
  quotaBytes: number;
  files: BrowserWritableProfileFile[];
  directories: BrowserWritableProfileDirectory[];
}

export interface BrowserNativeStorageConfiguration {
  schema: typeof browserNativeStorageSchema;
  schemaVersion: typeof browserNativeStorageSchemaVersion;
  sourceMount: BrowserSourceMount;
  writableProfile?: BrowserWritableProfileConfiguration;
  allowTestFaults?: boolean;
}

export interface WritableProfileSnapshot {
  schema: "vine.writable-profile-snapshot";
  schemaVersion: 3;
  titleFingerprint: string;
  profileId: string;
  rootPathA: number[];
  quotaBytes: number;
  files: Array<{
    objectId: number;
    relativePathA: number[];
    exists: boolean;
    generation: number;
    byteLength: number;
    sha256: string | null;
  }>;
  directories: Array<{
    directoryId: number;
    relativePathA: number[];
    exists: boolean;
    generation: number;
  }>;
}

interface RequestEnvelope {
  protocolVersion: typeof browserStorageProtocolVersion;
  requestId: string;
}

interface SessionRequestEnvelope extends RequestEnvelope {
  sessionKey: string;
  sessionEpoch: number;
}

export type BrowserStorageRequest =
  | (RequestEnvelope & {
      type: "initialize";
      sessionKey: string;
      configuration: BrowserNativeStorageConfiguration;
    })
  | (SessionRequestEnvelope & {
      type: "source-read";
      sourceId: number;
      objectId: number;
      offset: number;
      requestedBytes: number;
    })
  | (SessionRequestEnvelope & { type: "profile-snapshot" })
  | (SessionRequestEnvelope & {
      type: "profile-read";
      objectId: number;
      generation: number;
      offset: number;
      requestedBytes: number;
    })
  | (SessionRequestEnvelope & {
      type: "profile-commit";
      requestKey: string;
      objectId: number;
      baseGeneration: number;
      contentSha256: string;
      bytes: Uint8Array;
      testDropResponse?: boolean;
    })
  | (SessionRequestEnvelope & {
      type: "profile-directory-create";
      requestKey: string;
      directoryId: number;
      baseGeneration: number;
    })
  | (SessionRequestEnvelope & {
      type: "test-begin-uncommitted";
      objectId: number;
      baseGeneration: number;
      contentSha256: string;
      bytes: Uint8Array;
    })
  | (SessionRequestEnvelope & { type: "close" });

export interface BrowserStorageInitializationResult {
  kind: "initialized";
  titleFingerprint: string;
  snapshotId: string;
  sourceId: number;
  profileId: string | null;
  sessionEpoch: number;
  sqliteVersion: string | null;
  storageBackend: "file-system-access+opfs-sahpool-sqlite";
}

export type BrowserStorageResponse =
  | (RequestEnvelope & {
      type: "success";
      result:
        | BrowserStorageInitializationResult
        | { kind: "source-read"; bytes: Uint8Array }
        | { kind: "profile-snapshot"; snapshot: WritableProfileSnapshot }
        | { kind: "profile-read"; bytes: Uint8Array }
        | {
            kind: "profile-commit";
            objectId: number;
            baseGeneration: number;
            committedGeneration: number;
            duplicate: boolean;
          }
        | {
            kind: "profile-directory-create";
            directoryId: number;
            baseGeneration: number;
            committedGeneration: number;
            duplicate: boolean;
          }
        | { kind: "test-uncommitted-started" }
        | { kind: "closed" };
    })
  | (RequestEnvelope & {
      type: "error";
      code: string;
      message: string;
    })
  | {
      protocolVersion: typeof browserStorageProtocolVersion;
      type: "test-commit-published";
      requestId: string;
      requestKey: string;
      committedGeneration: number;
    };

export function sourceSnapshotManifest(mount: BrowserSourceMount): object {
  return {
    schema: "vine.source-snapshot-manifest",
    schemaVersion: 1,
    titleFingerprint: mount.titleFingerprint,
    snapshotId: mount.snapshotId,
    mount: { sourceId: mount.sourceId, rootPathA: mount.rootPathA },
    files: mount.files.map((file) => ({
      objectId: file.objectId,
      relativePathA: file.relativePathA,
      byteLength: file.byteLength,
      sha256: file.sha256,
    })),
  };
}
