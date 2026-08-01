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
  const wait = requireRecord(waitableTimerWait, "timer pacing evidence");
  const hasFull = Object.hasOwn(wait, "pacingRequests");
  const compactFields = [
    "pacingRequestCount",
    "firstPacingRequest",
    "lastPacingRequest",
  ];
  const hasCompact = compactFields.some((field) => Object.hasOwn(wait, field));
  const scheduledFields = ["count", "first", "last"];
  const hasScheduled = scheduledFields.some((field) => Object.hasOwn(wait, field));
  if ([hasFull, hasCompact, hasScheduled].filter(Boolean).length !== 1) {
    throw new TypeError(
      "timer pacing evidence must have exactly one full, compact, or scheduled shape",
    );
  }
  if (hasFull) {
    if (!Array.isArray(wait.pacingRequests)) {
      throw new TypeError("timer pacing evidence.pacingRequests must be an array");
    }
    return {
      totalCount: wait.pacingRequests.length,
      sampledRequests: wait.pacingRequests,
    };
  }

  const totalCount = hasScheduled ? wait.count : wait.pacingRequestCount;
  if (!Number.isSafeInteger(totalCount) || totalCount < 0) {
    throw new TypeError(
      "timer pacing evidence count must be a non-negative integer",
    );
  }
  const firstRequest = hasScheduled ? wait.first : wait.firstPacingRequest;
  const lastRequest = hasScheduled ? wait.last : wait.lastPacingRequest;
  if (totalCount === 0) {
    if (firstRequest !== null || lastRequest !== null) {
      throw new TypeError(
        "empty pacing evidence must have null first and last requests",
      );
    }
    return { totalCount, sampledRequests: [] };
  }

  const first = requireRecord(
    firstRequest,
    "timer pacing evidence first request",
  );
  const last = requireRecord(
    lastRequest,
    "timer pacing evidence last request",
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

export function projectCapabilityCheckpointCompletion(completion) {
  const projected = structuredClone(
    requireRecord(completion, "capability checkpoint completion"),
  );
  const hasSequence = Object.hasOwn(projected, "lastCompletionSequence");
  const hasInstructionCount = Object.hasOwn(
    projected,
    "lastCompletionInstructionCount",
  );
  if (hasSequence !== hasInstructionCount) {
    throw new TypeError(
      "capability checkpoint completion must have both direct last-completion fields",
    );
  }
  if (hasSequence || projected.setFocus === undefined) {
    return projected;
  }

  const setFocus = requireRecord(
    projected.setFocus,
    "capability checkpoint completion setFocus",
  );
  const window = requireRecord(
    setFocus.window,
    "capability checkpoint completion setFocus window",
  );
  if (
    !Number.isSafeInteger(setFocus.sequence) ||
    !Number.isSafeInteger(setFocus.trapInstructionCount)
  ) {
    throw new TypeError(
      "call-backed capability checkpoint completion requires integer sequence fields",
    );
  }
  if (
    !Number.isSafeInteger(setFocus.runtimeThreadId) ||
    setFocus.runtimeThreadId !== window.ownerThreadId
  ) {
    throw new TypeError(
      "call-backed capability checkpoint completion requires the SetFocus owner thread",
    );
  }
  projected.lastCompletionSequence = setFocus.sequence;
  projected.lastCompletionInstructionCount = setFocus.trapInstructionCount;
  return projected;
}
