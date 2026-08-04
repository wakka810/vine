export function writableCommitFailureControl(
  objectId: number,
  baseGeneration: bigint,
  win32Error: number,
): Uint8Array {
  const control = new Uint8Array(32);
  const view = new DataView(control.buffer);
  [
    1,
    objectId,
    Number(baseGeneration & 0xffff_ffffn),
    Number((baseGeneration >> 32n) & 0xffff_ffffn),
    Number(baseGeneration & 0xffff_ffffn),
    Number((baseGeneration >> 32n) & 0xffff_ffffn),
    win32Error,
    0,
  ].forEach((word, index) => view.setUint32(index * 4, word, true));
  return control;
}

export function writableDirectoryCreateFailureControl(
  directoryId: number,
  baseGeneration: bigint,
  win32Error: number,
): Uint8Array {
  return writableCommitFailureControl(directoryId, baseGeneration, win32Error);
}

export function writableCommitSuccessControl(
  objectId: number,
  baseGeneration: bigint,
  committedGeneration: bigint,
): Uint8Array {
  const control = writableCommitFailureControl(objectId, baseGeneration, 0);
  const view = new DataView(control.buffer, control.byteOffset, control.byteLength);
  view.setUint32(16, Number(committedGeneration & 0xffff_ffffn), true);
  view.setUint32(20, Number((committedGeneration >> 32n) & 0xffff_ffffn), true);
  return control;
}

export function recoverableCommitStorageError(errorCode: string): number | null {
  if (
    errorCode.includes("invalid") ||
    errorCode.includes("mismatch") ||
    errorCode.includes("unknown") ||
    errorCode.includes("request-key-conflict")
  ) {
    return null;
  }
  if (errorCode === "writable-profile-lease-revoked") {
    return 995;
  }
  if (errorCode.includes("SQLITE_BUSY") || errorCode.includes("SQLITE_LOCKED")) {
    return 33;
  }
  if (
    errorCode.includes("quota") ||
    errorCode.includes("SQLITE_FULL") ||
    errorCode.includes("disk-full") ||
    errorCode.includes("bytes-exceeded")
  ) {
    return 112;
  }
  if (
    errorCode.includes("generation-stale") ||
    errorCode.startsWith("ERR_SQLITE") ||
    errorCode.startsWith("SQLITE_") ||
    errorCode === "writable-profile-storage-failure" ||
    errorCode === "writable-profile-directory-storage-failure" ||
    errorCode === "browser-storage-failure"
  ) {
    return 29;
  }
  return null;
}

export function recoverableCommitHttpError(
  status: number,
  errorCode: string | null,
): number | null {
  if (errorCode !== null) {
    if (
      errorCode.includes("invalid") ||
      errorCode.includes("mismatch") ||
      errorCode.includes("unknown") ||
      errorCode.includes("request-key-conflict")
    ) {
      return null;
    }
    const mapped = recoverableCommitStorageError(errorCode);
    if (mapped !== null) {
      return mapped;
    }
  }
  if (status === 423 || status === 429) {
    return 33;
  }
  return null;
}
