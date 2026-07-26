import { isDeepStrictEqual } from "node:util";

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireRecord(value, label) {
  if (!isRecord(value)) {
    throw new TypeError(`${label} must be an object`);
  }
  return value;
}

export function projectWaitableTimerPacingRequests(waitableTimerWait) {
  const wait = requireRecord(waitableTimerWait, "waitableTimerWait");
  const hasFull = Object.hasOwn(wait, "pacingRequests");
  const compactFields = [
    "pacingRequestCount",
    "firstPacingRequest",
    "lastPacingRequest",
  ];
  const hasCompact = compactFields.some((field) => Object.hasOwn(wait, field));
  if (hasFull === hasCompact) {
    throw new TypeError(
      "waitableTimerWait must have exactly one full or compact pacing shape",
    );
  }
  if (hasFull) {
    if (!Array.isArray(wait.pacingRequests)) {
      throw new TypeError("waitableTimerWait.pacingRequests must be an array");
    }
    return {
      totalCount: wait.pacingRequests.length,
      sampledRequests: wait.pacingRequests,
    };
  }

  const totalCount = wait.pacingRequestCount;
  if (!Number.isSafeInteger(totalCount) || totalCount < 0) {
    throw new TypeError(
      "waitableTimerWait.pacingRequestCount must be a non-negative integer",
    );
  }
  if (totalCount === 0) {
    if (wait.firstPacingRequest !== null || wait.lastPacingRequest !== null) {
      throw new TypeError(
        "empty compact pacing evidence must have null first and last requests",
      );
    }
    return { totalCount, sampledRequests: [] };
  }

  const first = requireRecord(
    wait.firstPacingRequest,
    "waitableTimerWait.firstPacingRequest",
  );
  const last = requireRecord(
    wait.lastPacingRequest,
    "waitableTimerWait.lastPacingRequest",
  );
  if (totalCount === 1) {
    if (!isDeepStrictEqual(first, last)) {
      throw new TypeError(
        "single compact pacing request must have matching first and last samples",
      );
    }
    return { totalCount, sampledRequests: [first] };
  }
  return { totalCount, sampledRequests: [first, last] };
}
