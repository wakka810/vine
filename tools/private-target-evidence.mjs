import crypto from "node:crypto";

import {
  formatManifestRuntimeEndpointIdentity,
  formatRuntimeEndpointSymbol,
} from "./runtime-endpoint-symbol.mjs";

export const PRIVATE_TARGET_EVIDENCE_SCHEMA = "vine.private-target-evidence";
export const PRIVATE_TARGET_EVIDENCE_VERSION = 6;
export const PRIVATE_TARGET_EVIDENCE_DESCENDANT_PATHS = Object.freeze([
  "docs/private-target-evidence.json",
]);

const GIT_OBJECT_ID = /^[0-9a-f]{40}$/u;
const SHA256 = /^[0-9a-f]{64}$/u;
const MAXIMUM_SEMANTIC_REPORT_BYTES = 4 * 1024 * 1024;

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireRecord(value, label, errors) {
  if (!isRecord(value)) {
    errors.push(`${label} must be an object`);
    return {};
  }
  return value;
}

function requireString(value, label, errors) {
  if (typeof value !== "string" || value.length === 0) {
    errors.push(`${label} must be a non-empty string`);
  }
}

function requireInteger(value, label, errors, minimum = 0) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    errors.push(`${label} must be an integer >= ${minimum}`);
  }
}

function requireBoolean(value, label, errors) {
  if (typeof value !== "boolean") {
    errors.push(`${label} must be a boolean`);
  }
}

function requireSha256(value, label, errors) {
  if (typeof value !== "string" || !SHA256.test(value)) {
    errors.push(`${label} must be a lowercase SHA-256 digest`);
  }
}

function requireGitObjectId(value, label, errors) {
  if (typeof value !== "string" || !GIT_OBJECT_ID.test(value)) {
    errors.push(`${label} must be a lowercase Git object ID`);
  }
}

function requireStringArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return [];
  }
  if (new Set(value).size !== value.length) {
    errors.push(`${label} must not contain duplicates`);
  }
  return value;
}

function checkEqual(actual, expected, label, errors) {
  if (actual !== expected) {
    errors.push(`${label} must equal ${JSON.stringify(expected)}`);
  }
}

function capabilityCompletionFields(expected, includeDispatchBoundary = false) {
  if (expected.requiredEntries !== undefined) {
    return [
      "capability",
      "implementationId",
      "requiredEntries",
      "entryCount",
      "lastEntrySequence",
      "lastEntryInstructionCount",
      ...(expected.requiredCompletions === undefined
        ? []
        : [
            "requiredCompletions",
            "completionCount",
            "lastCompletionSequence",
            "lastCompletionInstructionCount",
          ]),
    ];
  }
  return expected.requiredPointerMessages === undefined
    ? [
        "capability",
        "implementationId",
        "requiredCompletions",
        "completionCount",
        "lastCompletionSequence",
        "lastCompletionInstructionCount",
      ]
    : [
        "capability",
        "implementationId",
        "requiredPointerMessages",
        "pointerMessageDeliveryCount",
        "releaseMessageQueued",
        "activeCallbackDepthAtStop",
        ...(includeDispatchBoundary
          ? ["dispatchSequence", "dispatchInstructionCount"]
          : []),
      ];
}

function validatePointerReleaseCompletion(
  completion,
  expected,
  label,
  errors,
  { requireWriteReceipt = false } = {},
) {
  if (
    expected.requiredEntries === undefined ||
    expected.requiredCompletions === undefined
  ) {
    return;
  }
  const releaseCall = requireRecord(
    completion.defWindowProcARelease,
    `${label}.defWindowProcARelease`,
    errors,
  );
  const callbackEntry = requireRecord(
    completion.releaseCallbackEntry,
    `${label}.releaseCallbackEntry`,
    errors,
  );
  const callbackReturn = requireRecord(
    completion.releaseCallbackReturn,
    `${label}.releaseCallbackReturn`,
    errors,
  );
  const postInputScanWaitableTimerCycle =
    expected.capability === "d3d9-post-input-scan-waitable-timer-cycle";
  const postInputScanTestCooperativeLevel =
    postInputScanWaitableTimerCycle ||
    expected.capability ===
    "d3d9-test-cooperative-level-post-input-scan-continuation";
  const postIsIconicGetAsyncKeyStateScan =
    postInputScanTestCooperativeLevel ||
    expected.capability ===
      "user32-get-async-key-state-post-is-iconic-clock-continuation";
  const postIsIconicTimeGetTime =
    postIsIconicGetAsyncKeyStateScan ||
    expected.capability ===
    "winmm-time-get-time-post-is-iconic-continuation";
  const postReleaseIsIconic =
    postIsIconicTimeGetTime ||
    expected.capability ===
    "user32-is-iconic-post-pointer-release-continuation";
  const postReleaseGetSystemMetricsFixedFrameHeight =
    postReleaseIsIconic ||
    expected.capability ===
    "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation";
  const postReleaseGetSystemMetricsCaption =
    postReleaseGetSystemMetricsFixedFrameHeight ||
    expected.capability ===
    "user32-get-system-metrics-caption-post-pointer-release-continuation";
  const postReleaseGetSystemMetrics =
    postReleaseGetSystemMetricsCaption ||
    expected.capability ===
      "user32-get-system-metrics-post-pointer-release-continuation";
  const postReleaseGetCursorPos =
    postReleaseGetSystemMetrics ||
    expected.capability ===
    "user32-get-cursor-pos-post-pointer-release-continuation";
  const postReleaseGetWindowRect =
    postReleaseGetCursorPos ||
    expected.capability ===
    "user32-get-window-rect-post-pointer-release-continuation";
  const postReleaseTimeGetTimeRepeatBatch =
    postReleaseGetWindowRect ||
    expected.capability ===
      "winmm-time-get-time-post-pointer-release-repeat-batch";
  const postReleaseTimeGetTime =
    postReleaseTimeGetTimeRepeatBatch ||
    expected.capability ===
      "winmm-time-get-time-post-pointer-release-continuation";
  const postReleasePeek =
    postReleaseTimeGetTime ||
    expected.capability ===
      "user32-peek-message-a-post-pointer-release-continuation";
  const finalCallName = postInputScanWaitableTimerCycle
    ? "timerWait"
    : postInputScanTestCooperativeLevel
    ? "d3d9TestCooperativeLevelPostInputScan"
    : postIsIconicGetAsyncKeyStateScan
    ? "getAsyncKeyStatePostIsIconicClockLast"
    : postIsIconicTimeGetTime
    ? "timeGetTimePostIsIconic"
    : postReleaseIsIconic
    ? "isIconicPostRelease"
    : postReleaseGetSystemMetricsFixedFrameHeight
    ? "getSystemMetricsFixedFrameHeightPostRelease"
    : postReleaseGetSystemMetricsCaption
    ? "getSystemMetricsCaptionPostRelease"
    : postReleaseGetSystemMetrics
    ? "getSystemMetricsPostRelease"
    : postReleaseGetCursorPos
    ? "getCursorPosPostRelease"
    : postReleaseGetWindowRect
      ? "getWindowRectPostRelease"
    : postReleaseTimeGetTime
      ? "timeGetTimePostRelease"
      : postReleasePeek
        ? "peekMessagePostRelease"
        : "defWindowProcARelease";
  const repeatCompletions = postReleaseTimeGetTimeRepeatBatch
    ? Array.isArray(completion.timeGetTimePostReleaseCompletions)
      ? completion.timeGetTimePostReleaseCompletions
      : (errors.push(
          `${label}.timeGetTimePostReleaseCompletions must be an array`,
        ),
        [])
    : [];
  const finalTimeGetTime = postReleaseTimeGetTimeRepeatBatch
    ? requireRecord(
        repeatCompletions.at(-1),
        `${label}.timeGetTimePostReleaseCompletions[last]`,
        errors,
      )
    : postReleaseTimeGetTime
      ? requireRecord(
        completion.timeGetTimePostRelease,
        `${label}.timeGetTimePostRelease`,
        errors,
      )
    : {};
  const finalCall = postInputScanWaitableTimerCycle
    ? requireRecord(completion.timerWait, `${label}.timerWait`, errors)
    : postInputScanTestCooperativeLevel
    ? requireRecord(
        completion.d3d9TestCooperativeLevelPostInputScan,
        `${label}.d3d9TestCooperativeLevelPostInputScan`,
        errors,
      )
    : postIsIconicGetAsyncKeyStateScan
    ? requireRecord(
        completion.getAsyncKeyStatePostIsIconicClockLast,
        `${label}.getAsyncKeyStatePostIsIconicClockLast`,
        errors,
      )
    : postIsIconicTimeGetTime
    ? requireRecord(
        completion.timeGetTimePostIsIconic,
        `${label}.timeGetTimePostIsIconic`,
        errors,
      )
    : postReleaseIsIconic
    ? requireRecord(
        completion.isIconicPostRelease,
        `${label}.isIconicPostRelease`,
        errors,
      )
    : postReleaseGetSystemMetricsFixedFrameHeight
    ? requireRecord(
        completion.getSystemMetricsFixedFrameHeightPostRelease,
        `${label}.getSystemMetricsFixedFrameHeightPostRelease`,
        errors,
      )
    : postReleaseGetSystemMetricsCaption
    ? requireRecord(
        completion.getSystemMetricsCaptionPostRelease,
        `${label}.getSystemMetricsCaptionPostRelease`,
        errors,
      )
    : postReleaseGetSystemMetrics
    ? requireRecord(
        completion.getSystemMetricsPostRelease,
        `${label}.getSystemMetricsPostRelease`,
        errors,
      )
    : postReleaseGetCursorPos
    ? requireRecord(
        completion.getCursorPosPostRelease,
        `${label}.getCursorPosPostRelease`,
        errors,
      )
    : postReleaseGetWindowRect
      ? requireRecord(
        completion.getWindowRectPostRelease,
        `${label}.getWindowRectPostRelease`,
        errors,
      )
    : postReleaseTimeGetTime
      ? finalTimeGetTime
      : postReleasePeek
        ? requireRecord(
            completion.peekMessagePostRelease,
            `${label}.peekMessagePostRelease`,
            errors,
          )
        : releaseCall;
  checkEqual(
    finalCall.sequence,
    expected.lastCompletionSequence,
    `${label}.${finalCallName}.sequence`,
    errors,
  );
  checkEqual(
    finalCall.trapInstructionCount,
    expected.lastCompletionInstructionCount,
    `${label}.${finalCallName}.trapInstructionCount`,
    errors,
  );
  if (postReleasePeek) {
    const peekCall = requireRecord(
      completion.peekMessagePostRelease,
      `${label}.peekMessagePostRelease`,
      errors,
    );
    checkEqual(
      peekCall.implementationId,
      "user32.peek-message-a.current-thread.v1",
      `${label}.peekMessagePostRelease.implementationId`,
      errors,
    );
    checkEqual(
      peekCall.result?.value,
      0,
      `${label}.peekMessagePostRelease.result.value`,
      errors,
    );
    checkEqual(
      completion.postReleaseQueueDepthBefore,
      0,
      `${label}.postReleaseQueueDepthBefore`,
      errors,
    );
    checkEqual(
      completion.postReleaseQueueDepthAfter,
      0,
      `${label}.postReleaseQueueDepthAfter`,
      errors,
    );
    checkEqual(
      completion.postReleaseMessageBytesPreserved,
      true,
      `${label}.postReleaseMessageBytesPreserved`,
      errors,
    );
  }
  if (postReleaseTimeGetTime) {
    const expectedReturnedTickCount = postReleaseTimeGetTimeRepeatBatch
      ? 3_601_227
      : 3_601_224;
    checkEqual(
      finalTimeGetTime.implementationId,
      "winmm.time-get-time.v1",
      `${label}.timeGetTimePostRelease.implementationId`,
      errors,
    );
    checkEqual(
      finalTimeGetTime.result?.value,
      expectedReturnedTickCount,
      `${label}.timeGetTimePostRelease.result.value`,
      errors,
    );
    checkEqual(
      finalTimeGetTime.tickCount?.returnedTickCount,
      expectedReturnedTickCount,
      `${label}.timeGetTimePostRelease.tickCount.returnedTickCount`,
      errors,
    );
    checkEqual(
      finalTimeGetTime.tickCount?.nextTickCount,
      expectedReturnedTickCount + 1,
      `${label}.timeGetTimePostRelease.tickCount.nextTickCount`,
      errors,
    );
    checkEqual(
      finalTimeGetTime.tickCount?.incrementPerRead,
      1,
      `${label}.timeGetTimePostRelease.tickCount.incrementPerRead`,
      errors,
    );
    if (postReleaseTimeGetTimeRepeatBatch) {
      const expectedSamples = [
        {
          sequence: 161_954,
          occurrence: 219,
          instructionCount: 321_462_555,
          stackPointerBefore: 7_338_244,
          returnedTickCount: 3_601_224,
        },
        {
          sequence: 161_955,
          occurrence: 220,
          instructionCount: 321_462_721,
          stackPointerBefore: 7_337_876,
          returnedTickCount: 3_601_225,
        },
        {
          sequence: 161_956,
          occurrence: 221,
          instructionCount: 321_462_906,
          stackPointerBefore: 7_338_216,
          returnedTickCount: 3_601_226,
        },
        {
          sequence: 161_957,
          occurrence: 222,
          instructionCount: 321_463_010,
          stackPointerBefore: 7_338_236,
          returnedTickCount: 3_601_227,
        },
      ];
      checkEqual(
        completion.maximumRepeatCompletions,
        16,
        `${label}.maximumRepeatCompletions`,
        errors,
      );
      checkEqual(
        repeatCompletions.length,
        4,
        `${label}.timeGetTimePostReleaseCompletions.length`,
        errors,
      );
      for (const [index, expectedSample] of expectedSamples.entries()) {
        const sample = requireRecord(
          repeatCompletions[index],
          `${label}.timeGetTimePostReleaseCompletions[${index}]`,
          errors,
        );
        const sampleLabel = `${label}.timeGetTimePostReleaseCompletions[${index}]`;
        checkEqual(sample.sequence, expectedSample.sequence, `${sampleLabel}.sequence`, errors);
        checkEqual(sample.occurrence, expectedSample.occurrence, `${sampleLabel}.occurrence`, errors);
        checkEqual(
          sample.trapInstructionCount,
          expectedSample.instructionCount,
          `${sampleLabel}.trapInstructionCount`,
          errors,
        );
        checkEqual(sample.runtimeThreadId, 8_192, `${sampleLabel}.runtimeThreadId`, errors);
        checkEqual(sample.module?.toLowerCase(), "winmm.dll", `${sampleLabel}.module`, errors);
        checkEqual(sample.symbol, "timeGetTime", `${sampleLabel}.symbol`, errors);
        checkEqual(
          sample.implementationId,
          "winmm.time-get-time.v1",
          `${sampleLabel}.implementationId`,
          errors,
        );
        checkEqual(
          Array.isArray(sample.arguments) ? sample.arguments.length : -1,
          0,
          `${sampleLabel}.arguments.length`,
          errors,
        );
        checkEqual(sample.result?.action, "set", `${sampleLabel}.result.action`, errors);
        checkEqual(
          sample.result?.value,
          expectedSample.returnedTickCount,
          `${sampleLabel}.result.value`,
          errors,
        );
        checkEqual(sample.returnAddress, 269_033_151, `${sampleLabel}.returnAddress`, errors);
        checkEqual(
          sample.stackPointerBefore,
          expectedSample.stackPointerBefore,
          `${sampleLabel}.stackPointerBefore`,
          errors,
        );
        checkEqual(
          sample.stackPointerAfter,
          expectedSample.stackPointerBefore + 4,
          `${sampleLabel}.stackPointerAfter`,
          errors,
        );
        checkEqual(sample.lastError, 0, `${sampleLabel}.lastError`, errors);
        checkEqual(
          Array.isArray(sample.guestWrites) ? sample.guestWrites.length : -1,
          0,
          `${sampleLabel}.guestWrites.length`,
          errors,
        );
        checkEqual(
          sample.tickCount?.returnedTickCount,
          expectedSample.returnedTickCount,
          `${sampleLabel}.tickCount.returnedTickCount`,
          errors,
        );
        checkEqual(
          sample.tickCount?.nextTickCount,
          expectedSample.returnedTickCount + 1,
          `${sampleLabel}.tickCount.nextTickCount`,
          errors,
        );
        checkEqual(
          sample.tickCount?.incrementPerRead,
          1,
          `${sampleLabel}.tickCount.incrementPerRead`,
          errors,
        );
      }
      const firstCall = requireRecord(
        completion.timeGetTimePostRelease,
        `${label}.timeGetTimePostRelease`,
        errors,
      );
      checkEqual(
        firstCall.sequence,
        161_954,
        `${label}.timeGetTimePostRelease.sequence`,
        errors,
      );
      checkEqual(
        firstCall.tickCount?.returnedTickCount,
        3_601_224,
        `${label}.timeGetTimePostRelease.tickCount.returnedTickCount`,
        errors,
      );
      checkEqual(
        firstCall.tickCount?.nextTickCount,
        3_601_225,
        `${label}.timeGetTimePostRelease.tickCount.nextTickCount`,
        errors,
      );
    }
  }
  if (postReleaseGetWindowRect) {
    const getWindowRect = requireRecord(
      completion.getWindowRectPostRelease,
      `${label}.getWindowRectPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(getWindowRect.guestWrites)
      ? getWindowRect.guestWrites
      : (errors.push(
          `${label}.getWindowRectPostRelease.guestWrites must be an array`,
        ),
        []);
    const rectWrite = requireRecord(
      guestWrites[0],
      `${label}.getWindowRectPostRelease.guestWrites[0]`,
      errors,
    );
    checkEqual(
      getWindowRect.sequence,
      161_958,
      `${label}.getWindowRectPostRelease.sequence`,
      errors,
    );
    checkEqual(
      getWindowRect.trapInstructionCount,
      321_463_115,
      `${label}.getWindowRectPostRelease.trapInstructionCount`,
      errors,
    );
    for (const [field, actual, expectedValue] of [
      ["runtimeThreadId", getWindowRect.runtimeThreadId, 8_192],
      ["module", getWindowRect.module?.toLowerCase(), "user32.dll"],
      ["symbol", getWindowRect.symbol, "GetWindowRect"],
      ["occurrence", getWindowRect.occurrence, 17],
      [
        "implementationId",
        getWindowRect.implementationId,
        "user32.get-window-rect.v1",
      ],
      ["arguments", getWindowRect.arguments?.join(","), "1208025089,7338172"],
      ["result.action", getWindowRect.result?.action, "set"],
      ["result.value", getWindowRect.result?.value, 1],
      ["returnAddress", getWindowRect.returnAddress, 268_993_880],
      ["stackPointerBefore", getWindowRect.stackPointerBefore, 7_338_140],
      ["stackPointerAfter", getWindowRect.stackPointerAfter, 7_338_152],
      ["lastError", getWindowRect.lastError, 0],
      ["guestWrites.length", guestWrites.length, 1],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getWindowRectPostRelease.${field}`,
        errors,
      );
    }
    checkEqual(
      rectWrite.address,
      7_338_172,
      `${label}.getWindowRectPostRelease.guestWrites[0].address`,
      errors,
    );
    checkEqual(
      rectWrite.bytes?.join(","),
      "61,1,0,0,154,0,0,0,67,6,0,0,135,3,0,0",
      `${label}.getWindowRectPostRelease.guestWrites[0].bytes`,
      errors,
    );
    if (requireWriteReceipt || rectWrite.result !== undefined) {
      const writeResult = requireRecord(
        rectWrite.result,
        `${label}.getWindowRectPostRelease.guestWrites[0].result`,
        errors,
      );
      for (const [field, actual, expectedValue] of [
        ["bytesWritten", writeResult.bytesWritten, 16],
        ["dirtyPages", writeResult.dirtyPages, 1],
        ["invalidation.dataTlb", writeResult.invalidation?.dataTlb, false],
        [
          "invalidation.instructionFetch",
          writeResult.invalidation?.instructionFetch,
          false,
        ],
        [
          "invalidation.generatedCode",
          writeResult.invalidation?.generatedCode,
          false,
        ],
      ]) {
        checkEqual(
          actual,
          expectedValue,
          `${label}.getWindowRectPostRelease.guestWrites[0].result.${field}`,
          errors,
        );
      }
    }
  }
  if (postReleaseGetCursorPos) {
    const getCursorPos = requireRecord(
      completion.getCursorPosPostRelease,
      `${label}.getCursorPosPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(getCursorPos.guestWrites)
      ? getCursorPos.guestWrites
      : (errors.push(
          `${label}.getCursorPosPostRelease.guestWrites must be an array`,
        ),
        []);
    const pointWrite = requireRecord(
      guestWrites[0],
      `${label}.getCursorPosPostRelease.guestWrites[0]`,
      errors,
    );
    for (const [field, actual, expectedValue] of [
      ["runtimeThreadId", getCursorPos.runtimeThreadId, 8_192],
      ["module", getCursorPos.module?.toLowerCase(), "user32.dll"],
      ["symbol", getCursorPos.symbol, "GetCursorPos"],
      ["occurrence", getCursorPos.occurrence, 17],
      [
        "implementationId",
        getCursorPos.implementationId,
        "user32.get-cursor-pos.v1",
      ],
      ["arguments", getCursorPos.arguments?.join(","), "7338164"],
      ["result.action", getCursorPos.result?.action, "set"],
      ["result.value", getCursorPos.result?.value, 1],
      ["returnAddress", getCursorPos.returnAddress, 268_993_891],
      ["stackPointerBefore", getCursorPos.stackPointerBefore, 7_338_144],
      ["stackPointerAfter", getCursorPos.stackPointerAfter, 7_338_152],
      ["lastError", getCursorPos.lastError, 0],
      ["guestWrites.length", guestWrites.length, 1],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getCursorPosPostRelease.${field}`,
        errors,
      );
    }
    checkEqual(
      completion.getCursorPosPostReleaseActiveCallbackDepthAtCompletion,
      0,
      `${label}.getCursorPosPostReleaseActiveCallbackDepthAtCompletion`,
      errors,
    );
    checkEqual(
      pointWrite.address,
      7_338_164,
      `${label}.getCursorPosPostRelease.guestWrites[0].address`,
      errors,
    );
    checkEqual(
      pointWrite.bytes?.join(","),
      "192,3,0,0,28,2,0,0",
      `${label}.getCursorPosPostRelease.guestWrites[0].bytes`,
      errors,
    );
    if (requireWriteReceipt || pointWrite.result !== undefined) {
      const writeResult = requireRecord(
        pointWrite.result,
        `${label}.getCursorPosPostRelease.guestWrites[0].result`,
        errors,
      );
      for (const [field, actual, expectedValue] of [
        ["bytesWritten", writeResult.bytesWritten, 8],
        ["dirtyPages", writeResult.dirtyPages, 1],
        ["invalidation.dataTlb", writeResult.invalidation?.dataTlb, false],
        [
          "invalidation.instructionFetch",
          writeResult.invalidation?.instructionFetch,
          false,
        ],
        [
          "invalidation.generatedCode",
          writeResult.invalidation?.generatedCode,
          false,
        ],
      ]) {
        checkEqual(
          actual,
          expectedValue,
          `${label}.getCursorPosPostRelease.guestWrites[0].result.${field}`,
          errors,
        );
      }
    }
  }
  if (postReleaseGetSystemMetrics) {
    const getSystemMetrics = requireRecord(
      completion.getSystemMetricsPostRelease,
      `${label}.getSystemMetricsPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(getSystemMetrics.guestWrites)
      ? getSystemMetrics.guestWrites
      : (errors.push(
          `${label}.getSystemMetricsPostRelease.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      ["sequence", getSystemMetrics.sequence, 161_960],
      ["runtimeThreadId", getSystemMetrics.runtimeThreadId, 8_192],
      ["module", getSystemMetrics.module?.toLowerCase(), "user32.dll"],
      ["symbol", getSystemMetrics.symbol, "GetSystemMetrics"],
      ["occurrence", getSystemMetrics.occurrence, 66],
      ["trapInstructionCount", getSystemMetrics.trapInstructionCount, 321_463_128],
      [
        "implementationId",
        getSystemMetrics.implementationId,
        "user32.get-system-metrics.v1",
      ],
      ["arguments", getSystemMetrics.arguments?.join(","), "7"],
      ["result.action", getSystemMetrics.result?.action, "set"],
      ["result.value", getSystemMetrics.result?.value, 3],
      ["returnAddress", getSystemMetrics.returnAddress, 268_993_910],
      ["stackPointerBefore", getSystemMetrics.stackPointerBefore, 7_338_144],
      ["stackPointerAfter", getSystemMetrics.stackPointerAfter, 7_338_152],
      ["lastError", getSystemMetrics.lastError, 0],
      ["guestWrites.length", guestWrites.length, 0],
      [
        "activeCallbackDepthAtCompletion",
        completion.getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion,
        0,
      ],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getSystemMetricsPostRelease.${field}`,
        errors,
      );
    }
  }
  if (postReleaseGetSystemMetricsCaption) {
    const getSystemMetricsCaption = requireRecord(
      completion.getSystemMetricsCaptionPostRelease,
      `${label}.getSystemMetricsCaptionPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(getSystemMetricsCaption.guestWrites)
      ? getSystemMetricsCaption.guestWrites
      : (errors.push(
          `${label}.getSystemMetricsCaptionPostRelease.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      ["sequence", getSystemMetricsCaption.sequence, 161_961],
      ["runtimeThreadId", getSystemMetricsCaption.runtimeThreadId, 8_192],
      ["module", getSystemMetricsCaption.module?.toLowerCase(), "user32.dll"],
      ["symbol", getSystemMetricsCaption.symbol, "GetSystemMetrics"],
      ["occurrence", getSystemMetricsCaption.occurrence, 67],
      [
        "trapInstructionCount",
        getSystemMetricsCaption.trapInstructionCount,
        321_463_133,
      ],
      [
        "implementationId",
        getSystemMetricsCaption.implementationId,
        "user32.get-system-metrics.v1",
      ],
      ["arguments", getSystemMetricsCaption.arguments?.join(","), "4"],
      ["result.action", getSystemMetricsCaption.result?.action, "set"],
      ["result.value", getSystemMetricsCaption.result?.value, 23],
      ["returnAddress", getSystemMetricsCaption.returnAddress, 268_993_922],
      ["stackPointerBefore", getSystemMetricsCaption.stackPointerBefore, 7_338_144],
      ["stackPointerAfter", getSystemMetricsCaption.stackPointerAfter, 7_338_152],
      ["lastError", getSystemMetricsCaption.lastError, 0],
      ["guestWrites.length", guestWrites.length, 0],
      [
        "activeCallbackDepthAtCompletion",
        completion
          .getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion,
        0,
      ],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getSystemMetricsCaptionPostRelease.${field}`,
        errors,
      );
    }
  }
  if (postReleaseGetSystemMetricsFixedFrameHeight) {
    const getSystemMetricsFixedFrameHeight = requireRecord(
      completion.getSystemMetricsFixedFrameHeightPostRelease,
      `${label}.getSystemMetricsFixedFrameHeightPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(
      getSystemMetricsFixedFrameHeight.guestWrites,
    )
      ? getSystemMetricsFixedFrameHeight.guestWrites
      : (errors.push(
          `${label}.getSystemMetricsFixedFrameHeightPostRelease.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      ["sequence", getSystemMetricsFixedFrameHeight.sequence, 161_962],
      ["runtimeThreadId", getSystemMetricsFixedFrameHeight.runtimeThreadId, 8_192],
      ["module", getSystemMetricsFixedFrameHeight.module?.toLowerCase(), "user32.dll"],
      ["symbol", getSystemMetricsFixedFrameHeight.symbol, "GetSystemMetrics"],
      ["occurrence", getSystemMetricsFixedFrameHeight.occurrence, 68],
      [
        "trapInstructionCount",
        getSystemMetricsFixedFrameHeight.trapInstructionCount,
        321_463_139,
      ],
      [
        "implementationId",
        getSystemMetricsFixedFrameHeight.implementationId,
        "user32.get-system-metrics.v1",
      ],
      ["arguments", getSystemMetricsFixedFrameHeight.arguments?.join(","), "8"],
      ["result.action", getSystemMetricsFixedFrameHeight.result?.action, "set"],
      ["result.value", getSystemMetricsFixedFrameHeight.result?.value, 3],
      ["returnAddress", getSystemMetricsFixedFrameHeight.returnAddress, 268_993_936],
      ["stackPointerBefore", getSystemMetricsFixedFrameHeight.stackPointerBefore, 7_338_144],
      ["stackPointerAfter", getSystemMetricsFixedFrameHeight.stackPointerAfter, 7_338_152],
      ["lastError", getSystemMetricsFixedFrameHeight.lastError, 0],
      ["guestWrites.length", guestWrites.length, 0],
      [
        "activeCallbackDepthAtCompletion",
        completion
          .getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion,
        0,
      ],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getSystemMetricsFixedFrameHeightPostRelease.${field}`,
        errors,
      );
    }
  }
  if (postReleaseIsIconic) {
    const isIconic = requireRecord(
      completion.isIconicPostRelease,
      `${label}.isIconicPostRelease`,
      errors,
    );
    const guestWrites = Array.isArray(isIconic.guestWrites)
      ? isIconic.guestWrites
      : (errors.push(`${label}.isIconicPostRelease.guestWrites must be an array`),
        []);
    for (const [field, actual, expectedValue] of [
      ["sequence", isIconic.sequence, 161_963],
      ["runtimeThreadId", isIconic.runtimeThreadId, 8_192],
      ["module", isIconic.module?.toLowerCase(), "user32.dll"],
      ["symbol", isIconic.symbol, "IsIconic"],
      ["occurrence", isIconic.occurrence, 8],
      ["trapInstructionCount", isIconic.trapInstructionCount, 321_463_282],
      [
        "implementationId",
        isIconic.implementationId,
        "user32.is-iconic.logical-window-style.v1",
      ],
      ["arguments", isIconic.arguments?.join(","), "1208025089"],
      ["result.action", isIconic.result?.action, "set"],
      ["result.value", isIconic.result?.value, 0],
      ["returnAddress", isIconic.returnAddress, 268_995_012],
      ["stackPointerBefore", isIconic.stackPointerBefore, 7_338_200],
      ["stackPointerAfter", isIconic.stackPointerAfter, 7_338_208],
      ["lastError", isIconic.lastError, 0],
      ["guestWrites.length", guestWrites.length, 0],
      [
        "activeCallbackDepthAtCompletion",
        completion.isIconicPostReleaseActiveCallbackDepthAtCompletion,
        0,
      ],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.isIconicPostRelease.${field}`,
        errors,
      );
    }
  }
  if (postIsIconicTimeGetTime) {
    const timeGetTime = requireRecord(
      completion.timeGetTimePostIsIconic,
      `${label}.timeGetTimePostIsIconic`,
      errors,
    );
    const guestWrites = Array.isArray(timeGetTime.guestWrites)
      ? timeGetTime.guestWrites
      : (errors.push(
          `${label}.timeGetTimePostIsIconic.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      ["sequence", timeGetTime.sequence, 161_964],
      ["runtimeThreadId", timeGetTime.runtimeThreadId, 8_192],
      ["module", timeGetTime.module?.toLowerCase(), "winmm.dll"],
      ["symbol", timeGetTime.symbol, "timeGetTime"],
      ["occurrence", timeGetTime.occurrence, 223],
      ["trapInstructionCount", timeGetTime.trapInstructionCount, 321_463_291],
      ["implementationId", timeGetTime.implementationId, "winmm.time-get-time.v1"],
      ["arguments", timeGetTime.arguments?.join(","), ""],
      ["result.action", timeGetTime.result?.action, "set"],
      ["result.value", timeGetTime.result?.value, 3_601_228],
      ["returnAddress", timeGetTime.returnAddress, 269_033_151],
      ["stackPointerBefore", timeGetTime.stackPointerBefore, 7_338_200],
      ["stackPointerAfter", timeGetTime.stackPointerAfter, 7_338_204],
      ["lastError", timeGetTime.lastError, 0],
      ["guestWrites.length", guestWrites.length, 0],
      [
        "tickCount.returnedTickCount",
        timeGetTime.tickCount?.returnedTickCount,
        3_601_228,
      ],
      ["tickCount.nextTickCount", timeGetTime.tickCount?.nextTickCount, 3_601_229],
      ["tickCount.incrementPerRead", timeGetTime.tickCount?.incrementPerRead, 1],
      [
        "activeCallbackDepthAtCompletion",
        completion.timeGetTimePostIsIconicActiveCallbackDepthAtCompletion,
        0,
      ],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.timeGetTimePostIsIconic.${field}`,
        errors,
      );
    }
  }
  if (postIsIconicGetAsyncKeyStateScan) {
    const first = requireRecord(
      completion.getAsyncKeyStatePostIsIconicClockFirst,
      `${label}.getAsyncKeyStatePostIsIconicClockFirst`,
      errors,
    );
    const last = requireRecord(
      completion.getAsyncKeyStatePostIsIconicClockLast,
      `${label}.getAsyncKeyStatePostIsIconicClockLast`,
      errors,
    );
    const firstGuestWrites = Array.isArray(first.guestWrites)
      ? first.guestWrites
      : (errors.push(
          `${label}.getAsyncKeyStatePostIsIconicClockFirst.guestWrites must be an array`,
        ),
        []);
    const lastGuestWrites = Array.isArray(last.guestWrites)
      ? last.guestWrites
      : (errors.push(
          `${label}.getAsyncKeyStatePostIsIconicClockLast.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      [
        "requiredCompletions",
        completion.requiredCompletions,
        postInputScanTestCooperativeLevel ? 1 : 38,
      ],
      [
        "completionCount",
        completion.completionCount,
        postInputScanTestCooperativeLevel ? 1 : 38,
      ],
      [
        "firstCompletionSequence",
        completion.firstCompletionSequence,
        postInputScanWaitableTimerCycle
          ? 162_081
          : postInputScanTestCooperativeLevel
            ? 162_080
            : 161_965,
      ],
      [
        "firstCompletionInstructionCount",
        completion.firstCompletionInstructionCount,
        postInputScanWaitableTimerCycle
          ? 321_468_343
          : postInputScanTestCooperativeLevel
            ? 321_468_292
            : 321_463_448,
      ],
      [
        "lastCompletionSequence",
        completion.lastCompletionSequence,
        postInputScanWaitableTimerCycle
          ? 162_082
          : postInputScanTestCooperativeLevel
            ? 162_080
            : 162_079,
      ],
      [
        "lastCompletionInstructionCount",
        completion.lastCompletionInstructionCount,
        postInputScanWaitableTimerCycle
          ? 321_468_349
          : postInputScanTestCooperativeLevel
            ? 321_468_292
            : 321_468_229,
      ],
      [
        "scanVirtualKeys",
        completion.scanVirtualKeys?.join(","),
        "14,15,13,16,27,38,33,40,34,37,39,49,97,50,98,51,99,52,100,53,101,54,102,55,103,56,104,57,105,48,96,9,17,1,2,4,5,6",
      ],
      ["result.action", completion.result?.action, "set"],
      ["result.value", completion.result?.value, 0],
      ["lastError", completion.lastError, 0],
      ["guestWriteBytes", completion.guestWriteBytes, 0],
      [
        "activeCallbackDepthAtCompletion",
        completion.getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion,
        0,
      ],
      ["setFocusActiveCallbackDepthAtStop", completion.setFocusActiveCallbackDepthAtStop, 1],
      ["first.sequence", first.sequence, 161_965],
      ["first.runtimeThreadId", first.runtimeThreadId, 8_192],
      ["first.module", first.module?.toLowerCase(), "user32.dll"],
      ["first.symbol", first.symbol, "GetAsyncKeyState"],
      ["first.occurrence", first.occurrence, 386],
      ["first.trapInstructionCount", first.trapInstructionCount, 321_463_448],
      ["first.implementationId", first.implementationId, "user32.get-async-key-state.v1"],
      ["first.arguments", first.arguments?.join(","), "14"],
      ["first.result.action", first.result?.action, "set"],
      ["first.result.value", first.result?.value, 0],
      ["first.returnAddress", first.returnAddress, 268_862_469],
      ["first.stackPointerBefore", first.stackPointerBefore, 7_337_996],
      ["first.stackPointerAfter", first.stackPointerAfter, 7_338_004],
      ["first.lastError", first.lastError, 0],
      ["first.guestWrites.length", firstGuestWrites.length, 0],
      ["last.sequence", last.sequence, 162_079],
      ["last.runtimeThreadId", last.runtimeThreadId, 8_192],
      ["last.module", last.module?.toLowerCase(), "user32.dll"],
      ["last.symbol", last.symbol, "GetAsyncKeyState"],
      ["last.occurrence", last.occurrence, 423],
      ["last.trapInstructionCount", last.trapInstructionCount, 321_468_229],
      ["last.implementationId", last.implementationId, "user32.get-async-key-state.v1"],
      ["last.arguments", last.arguments?.join(","), "6"],
      ["last.result.action", last.result?.action, "set"],
      ["last.result.value", last.result?.value, 0],
      ["last.returnAddress", last.returnAddress, 268_862_469],
      ["last.stackPointerBefore", last.stackPointerBefore, 7_338_184],
      ["last.stackPointerAfter", last.stackPointerAfter, 7_338_192],
      ["last.lastError", last.lastError, 0],
      ["last.guestWrites.length", lastGuestWrites.length, 0],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.getAsyncKeyStatePostIsIconicClock.${field}`,
        errors,
      );
    }
  }
  if (postInputScanTestCooperativeLevel) {
    const call = requireRecord(
      completion.d3d9TestCooperativeLevelPostInputScan,
      `${label}.d3d9TestCooperativeLevelPostInputScan`,
      errors,
    );
    const guestWrites = Array.isArray(call.guestWrites)
      ? call.guestWrites
      : (errors.push(
          `${label}.d3d9TestCooperativeLevelPostInputScan.guestWrites must be an array`,
        ),
        []);
    for (const [field, actual, expectedValue] of [
      [
        "prerequisiteScanCompletionSequence",
        completion.prerequisiteScanCompletionSequence,
        162_079,
      ],
      [
        "prerequisiteScanCompletionInstructionCount",
        completion.prerequisiteScanCompletionInstructionCount,
        321_468_229,
      ],
      [
        "activeCallbackDepthAtCompletion",
        completion.d3d9TestCooperativeLevelPostInputScanActiveCallbackDepthAtCompletion,
        0,
      ],
      ["sequence", call.sequence, 162_080],
      ["runtimeThreadId", call.runtimeThreadId, 8_192],
      ["module", call.module?.toLowerCase(), "<com>"],
      ["symbol", call.symbol, "IDirect3DDevice9.TestCooperativeLevel"],
      ["occurrence", call.occurrence, 11],
      ["trapInstructionCount", call.trapInstructionCount, 321_468_292],
      ["interface", call.interface, "IDirect3DDevice9"],
      ["vtableSlot", call.vtableSlot, 3],
      ["arguments", call.arguments?.join(","), "16842756"],
      ["result.kind", call.result?.kind, "h-result"],
      ["result.value", call.result?.value, 0],
      ["returnAddress", call.returnAddress, 268_807_629],
      ["stackPointerBefore", call.stackPointerBefore, 7_338_228],
      ["stackPointerAfter", call.stackPointerAfter, 7_338_236],
      ["referenceCount", call.referenceCount, 5],
      ["objectRetired", call.objectRetired, false],
      ["guestWrites.length", guestWrites.length, 0],
      ["cumulativeCompletions", completion.cumulativeCompletions, 9],
    ]) {
      checkEqual(
        actual,
        expectedValue,
        `${label}.d3d9TestCooperativeLevelPostInputScan.${field}`,
        errors,
      );
    }
  }
  if (postInputScanWaitableTimerCycle) {
    const arm = requireRecord(completion.timerArm, `${label}.timerArm`, errors);
    const wait = requireRecord(completion.timerWait, `${label}.timerWait`, errors);
    const armTimer = requireRecord(
      arm.waitableTimer,
      `${label}.timerArm.waitableTimer`,
      errors,
    );
    const waitTimer = requireRecord(
      wait.waitableTimer,
      `${label}.timerWait.waitableTimer`,
      errors,
    );
    const armState = requireRecord(
      armTimer.timer,
      `${label}.timerArm.waitableTimer.timer`,
      errors,
    );
    const waitState = requireRecord(
      waitTimer.timer,
      `${label}.timerWait.waitableTimer.timer`,
      errors,
    );
    const armObject = requireRecord(
      armTimer.objectRecord,
      `${label}.timerArm.waitableTimer.objectRecord`,
      errors,
    );
    const waitObject = requireRecord(
      waitTimer.objectRecord,
      `${label}.timerWait.waitableTimer.objectRecord`,
      errors,
    );
    for (const [field, actual, expectedValue] of [
      [
        "prerequisiteTestCooperativeLevelSequence",
        completion.prerequisiteTestCooperativeLevelSequence,
        162_080,
      ],
      [
        "prerequisiteTestCooperativeLevelInstructionCount",
        completion.prerequisiteTestCooperativeLevelInstructionCount,
        321_468_292,
      ],
      ["firstCompletionSequence", completion.firstCompletionSequence, 162_081],
      [
        "firstCompletionInstructionCount",
        completion.firstCompletionInstructionCount,
        321_468_343,
      ],
      [
        "timerArmActiveCallbackDepthAtCompletion",
        completion.timerArmActiveCallbackDepthAtCompletion,
        0,
      ],
      [
        "timerWaitActiveCallbackDepthAtCompletion",
        completion.timerWaitActiveCallbackDepthAtCompletion,
        0,
      ],
      ["timerArm.sequence", arm.sequence, 162_081],
      ["timerArm.occurrence", arm.occurrence, 11_476],
      ["timerArm.trapInstructionCount", arm.trapInstructionCount, 321_468_343],
      ["timerArm.runtimeThreadId", arm.runtimeThreadId, 8_192],
      ["timerArm.module", arm.module?.toLowerCase(), "kernel32.dll"],
      ["timerArm.symbol", arm.symbol, "SetWaitableTimer"],
      [
        "timerArm.implementationId",
        arm.implementationId,
        "kernel32.set-waitable-timer.relative-one-shot.v1",
      ],
      ["timerArm.arguments", arm.arguments?.join(","), "1459683329,7338224,0,0,0,0"],
      ["timerArm.result.action", arm.result?.action, "set"],
      ["timerArm.result.value", arm.result?.value, 1],
      ["timerArm.returnAddress", arm.returnAddress, 268_764_679],
      ["timerArm.stackPointerBefore", arm.stackPointerBefore, 7_338_196],
      ["timerArm.stackPointerAfter", arm.stackPointerAfter, 7_338_224],
      ["timerArm.lastError", arm.lastError, 0],
      ["timerArm.guestWrites.length", arm.guestWrites?.length, 0],
      ["timerArm.waitableTimer.handle", armTimer.handle, 1_459_683_329],
      ["timerArm.waitableTimer.timer.id", armState.id, 1],
      ["timerArm.waitableTimer.timer.manualReset", armState.manualReset, true],
      [
        "timerArm.waitableTimer.timer.arm.relativeDueTime100ns",
        armState.arm?.relativeDueTime100ns,
        20_000,
      ],
      [
        "timerArm.waitableTimer.timer.arm.armedAtTime100ns",
        armState.arm?.armedAtTime100ns,
        57_470_000,
      ],
      [
        "timerArm.waitableTimer.timer.arm.deadlineTime100ns",
        armState.arm?.deadlineTime100ns,
        57_490_000,
      ],
      ["timerArm.waitableTimer.timer.arm.periodMilliseconds", armState.arm?.periodMilliseconds, 0],
      ["timerArm.waitableTimer.timer.arm.generation", armState.arm?.generation, 9],
      ["timerArm.waitableTimer.objectRecord.id", armObject.id, 20],
      ["timerArm.waitableTimer.objectRecord.waitState.state", armObject.waitState?.state, "nonsignaled"],
      ["timerWait.sequence", wait.sequence, 162_082],
      ["timerWait.occurrence", wait.occurrence, 11_475],
      ["timerWait.trapInstructionCount", wait.trapInstructionCount, 321_468_349],
      ["timerWait.runtimeThreadId", wait.runtimeThreadId, 8_192],
      ["timerWait.module", wait.module?.toLowerCase(), "kernel32.dll"],
      ["timerWait.symbol", wait.symbol, "WaitForSingleObjectEx"],
      [
        "timerWait.implementationId",
        wait.implementationId,
        "kernel32.wait-for-single-object-ex.waitable-timer.v1",
      ],
      ["timerWait.arguments", wait.arguments?.join(","), "1459683329,10,1"],
      ["timerWait.result.action", wait.result?.action, "set"],
      ["timerWait.result.value", wait.result?.value, 0],
      ["timerWait.returnAddress", wait.returnAddress, 268_764_693],
      ["timerWait.stackPointerBefore", wait.stackPointerBefore, 7_338_208],
      ["timerWait.stackPointerAfter", wait.stackPointerAfter, 7_338_224],
      ["timerWait.lastError", wait.lastError, 0],
      ["timerWait.guestWrites.length", wait.guestWrites?.length, 0],
      ["timerWait.waitableTimer.handle", waitTimer.handle, 1_459_683_329],
      ["timerWait.waitableTimer.timer.id", waitState.id, 1],
      ["timerWait.waitableTimer.timer.manualReset", waitState.manualReset, true],
      [
        "timerWait.waitableTimer.timer.lastExpiration.armGeneration",
        waitState.lastExpiration?.armGeneration,
        9,
      ],
      [
        "timerWait.waitableTimer.timer.lastExpiration.deadlineTime100ns",
        waitState.lastExpiration?.deadlineTime100ns,
        57_490_000,
      ],
      ["timerWait.waitableTimer.objectRecord.id", waitObject.id, 20],
      ["timerWait.waitableTimer.objectRecord.waitState.state", waitObject.waitState?.state, "signaled"],
      ["timerWait.waitableTimer.objectRecord.waitState.exit_code", waitObject.waitState?.exit_code, 0],
      [
        "timerWait.waitableTimer.timerClock.currentTime100ns",
        waitTimer.timerClock?.currentTime100ns,
        57_490_000,
      ],
      ["timerWait.waitableTimer.timerClock.generation", waitTimer.timerClock?.generation, 11_474],
      ["timerWait.waitableTimer.nextWaitRegistrationId", waitTimer.nextWaitRegistrationId, 11_475],
      ["cumulativeTimerArmCompletions", completion.cumulativeTimerArmCompletions, 11_476],
      ["cumulativeTimerWaitCompletions", completion.cumulativeTimerWaitCompletions, 11_475],
    ]) {
      checkEqual(actual, expectedValue, `${label}.${field}`, errors);
    }
  }
  checkEqual(
    releaseCall.implementationId,
    "user32.def-window-proc-a.v1",
    `${label}.defWindowProcARelease.implementationId`,
    errors,
  );
  checkEqual(
    releaseCall.result?.value,
    0,
    `${label}.defWindowProcARelease.result.value`,
    errors,
  );
  checkEqual(
    callbackReturn.frameId,
    callbackEntry.frameId,
    `${label}.releaseCallbackReturn.frameId`,
    errors,
  );
  checkEqual(
    callbackReturn.result,
    0,
    `${label}.releaseCallbackReturn.result`,
    errors,
  );
  checkEqual(
    completion.dispatchMessageReleaseResult,
    0,
    `${label}.dispatchMessageReleaseResult`,
    errors,
  );
  checkEqual(
    completion.activeCallbackDepthAtStop,
    0,
    `${label}.activeCallbackDepthAtStop`,
    errors,
  );
}

function validateExecutionStateMachine(value, label, errors) {
  const stateMachine = requireRecord(value, label, errors);
  for (const scenario of ["success", "cancellation", "watchdog"]) {
    const outcome = requireRecord(
      stateMachine[scenario],
      `${label}.${scenario}`,
      errors,
    );
    checkEqual(
      outcome.finalState,
      "stopped",
      `${label}.${scenario}.finalState`,
      errors,
    );
    requireInteger(
      outcome.transitionCount,
      `${label}.${scenario}.transitionCount`,
      errors,
      1,
    );
    checkEqual(
      outcome.lastTransitionTo,
      "stopped",
      `${label}.${scenario}.lastTransitionTo`,
      errors,
    );
    checkEqual(
      outcome.pendingHostRequestRecords,
      0,
      `${label}.${scenario}.pendingHostRequestRecords`,
      errors,
    );
    if (scenario !== "watchdog") {
      checkEqual(
        outcome.watchdogContext,
        null,
        `${label}.${scenario}.watchdogContext`,
        errors,
      );
      continue;
    }
    const context = requireRecord(
      outcome.watchdogContext,
      `${label}.watchdog.watchdogContext`,
      errors,
    );
    requireInteger(
      context.threadId,
      `${label}.watchdog.watchdogContext.threadId`,
      errors,
      1,
    );
    if (
      context.requestId !== null &&
      (typeof context.requestId !== "string" || !/^(0|[1-9][0-9]*)$/u.test(context.requestId))
    ) {
      errors.push(`${label}.watchdog.watchdogContext.requestId must be null or a decimal ID`);
    }
    const transition = requireRecord(
      context.lastTransition,
      `${label}.watchdog.watchdogContext.lastTransition`,
      errors,
    );
    for (const field of ["from", "to", "cause"]) {
      requireString(
        transition[field],
        `${label}.watchdog.watchdogContext.lastTransition.${field}`,
        errors,
      );
    }
  }
}

function digestPayload(evidence) {
  const payload = structuredClone(evidence);
  delete payload.payloadSha256;
  return crypto
    .createHash("sha256")
    .update(`${JSON.stringify(payload, null, 2)}\n`)
    .digest("hex");
}

export function validatePrivateTargetEvidenceDescendantPaths(paths) {
  if (!Array.isArray(paths)) {
    return ["private target evidence descendant paths must be an array"];
  }
  const allowed = new Set(PRIVATE_TARGET_EVIDENCE_DESCENDANT_PATHS);
  const errors = [];
  for (const value of paths) {
    if (typeof value !== "string" || value.length === 0) {
      errors.push("private target evidence descendant path must be a non-empty string");
      continue;
    }
    if (!allowed.has(value)) {
      errors.push(
        `private target evidence is stale after descendant change ${JSON.stringify(value)}`,
      );
    }
  }
  return errors;
}

export function finalizePrivateTargetEvidence(payload) {
  const evidence = structuredClone(payload);
  evidence.payloadSha256 = digestPayload(evidence);
  return evidence;
}

export function validatePrivateTargetEvidence(evidence, checkpoint) {
  const errors = [];
  const root = requireRecord(evidence, "evidence", errors);
  checkEqual(root.schema, PRIVATE_TARGET_EVIDENCE_SCHEMA, "evidence.schema", errors);
  checkEqual(
    root.schemaVersion,
    PRIVATE_TARGET_EVIDENCE_VERSION,
    "evidence.schemaVersion",
    errors,
  );
  checkEqual(root.status, "passed", "evidence.status", errors);
  requireString(root.recordedAt, "evidence.recordedAt", errors);
  if (typeof root.recordedAt === "string" && !Number.isFinite(Date.parse(root.recordedAt))) {
    errors.push("evidence.recordedAt must be an ISO timestamp");
  }

  const repository = requireRecord(root.repository, "evidence.repository", errors);
  requireGitObjectId(
    repository.implementationCommit,
    "evidence.repository.implementationCommit",
    errors,
  );
  requireGitObjectId(
    repository.implementationTree,
    "evidence.repository.implementationTree",
    errors,
  );
  requireGitObjectId(repository.headCommit, "evidence.repository.headCommit", errors);
  checkEqual(
    repository.headCommit,
    repository.implementationCommit,
    "evidence.repository.headCommit",
    errors,
  );
  requireBoolean(repository.dirty, "evidence.repository.dirty", errors);
  const dirtyPaths = requireStringArray(
    repository.dirtyPaths,
    "evidence.repository.dirtyPaths",
    errors,
  );
  const allowedDirtyPaths = requireStringArray(
    repository.allowedDirtyPaths,
    "evidence.repository.allowedDirtyPaths",
    errors,
  );
  if (allowedDirtyPaths.some((entry) => !dirtyPaths.includes(entry))) {
    errors.push("evidence.repository.allowedDirtyPaths must be a subset of dirtyPaths");
  }
  checkEqual(
    repository.dirty,
    dirtyPaths.length > 0,
    "evidence.repository.dirty",
    errors,
  );
  checkEqual(
    repository.executionInputsClean,
    true,
    "evidence.repository.executionInputsClean",
    errors,
  );
  const disallowedDirtyPaths = requireStringArray(
    repository.disallowedDirtyPaths,
    "evidence.repository.disallowedDirtyPaths",
    errors,
  );
  if (disallowedDirtyPaths.length !== 0) {
    errors.push("evidence.repository.disallowedDirtyPaths must be empty");
  }

  const command = requireRecord(root.command, "evidence.command", errors);
  checkEqual(
    command.canonical,
    "npm run check:private-target",
    "evidence.command.canonical",
    errors,
  );
  requireString(command.invocation, "evidence.command.invocation", errors);
  checkEqual(
    command.semanticGate,
    "required",
    "evidence.command.semanticGate",
    errors,
  );
  checkEqual(
    command.performanceGate,
    "separate-observation",
    "evidence.command.performanceGate",
    errors,
  );

  const source = requireRecord(root.source, "evidence.source", errors);
  requireString(source.fileName, "evidence.source.fileName", errors);
  requireInteger(source.bytes, "evidence.source.bytes", errors, 1);
  requireSha256(source.sha256, "evidence.source.sha256", errors);

  const expectedBrowser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  checkEqual(source.bytes, expectedBrowser.sourceBytes, "evidence.source.bytes", errors);
  checkEqual(source.sha256, expectedBrowser.sourceSha256, "evidence.source.sha256", errors);

  const profile = requireRecord(root.profile, "evidence.profile", errors);
  checkEqual(
    profile.currentCheckpointSchema,
    checkpoint.schema,
    "evidence.profile.currentCheckpointSchema",
    errors,
  );
  checkEqual(
    profile.currentCheckpointSchemaVersion,
    checkpoint.schemaVersion,
    "evidence.profile.currentCheckpointSchemaVersion",
    errors,
  );
  checkEqual(
    profile.runtimeCheckpoint,
    checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
    "evidence.profile.runtimeCheckpoint",
    errors,
  );
  checkEqual(
    profile.runtimeProfile,
    checkpoint.activeRuntimeCheckpoint.profile,
    "evidence.profile.runtimeProfile",
    errors,
  );
  const policy = checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
  checkEqual(profile.policySchema, policy.schema, "evidence.profile.policySchema", errors);
  checkEqual(
    profile.policySchemaVersion,
    policy.schemaVersion,
    "evidence.profile.policySchemaVersion",
    errors,
  );
  checkEqual(profile.policyId, policy.id, "evidence.profile.policyId", errors);
  checkEqual(profile.policySource, policy.source, "evidence.profile.policySource", errors);
  requireString(profile.nodeReportSchema, "evidence.profile.nodeReportSchema", errors);
  requireInteger(
    profile.nodeReportSchemaVersion,
    "evidence.profile.nodeReportSchemaVersion",
    errors,
    1,
  );
  checkEqual(
    profile.browserRuntimeProtocol,
    checkpoint.browserCheckpoint.browserRuntimeProtocol,
    "evidence.profile.browserRuntimeProtocol",
    errors,
  );
  checkEqual(
    profile.executionEngineProfile,
    checkpoint.browserCheckpoint.executionEngineProfile,
    "evidence.profile.executionEngineProfile",
    errors,
  );

  const environment = requireRecord(root.environment, "evidence.environment", errors);
  for (const field of ["platform", "osRelease", "architecture", "node", "rustc", "cargo", "chrome"] ) {
    requireString(environment[field], `evidence.environment.${field}`, errors);
  }
  const llvm = requireRecord(environment.llvm, "evidence.environment.llvm", errors);
  for (const field of ["clang", "wasmClang", "lldLink", "llvmReadobj"]) {
    requireString(llvm[field], `evidence.environment.llvm.${field}`, errors);
  }
  const executionEngine = requireRecord(
    environment.executionEngine,
    "evidence.environment.executionEngine",
    errors,
  );
  requireString(
    executionEngine.engine,
    "evidence.environment.executionEngine.engine",
    errors,
  );
  requireString(
    executionEngine.sourceRevision,
    "evidence.environment.executionEngine.sourceRevision",
    errors,
  );
  const cpuContextAbi = requireRecord(
    executionEngine.cpuContextAbi,
    "evidence.environment.executionEngine.cpuContextAbi",
    errors,
  );
  requireInteger(
    cpuContextAbi.version,
    "evidence.environment.executionEngine.cpuContextAbi.version",
    errors,
    1,
  );
  requireInteger(
    cpuContextAbi.bytes,
    "evidence.environment.executionEngine.cpuContextAbi.bytes",
    errors,
    1,
  );
  const adapterArtifact = requireRecord(
    executionEngine.adapterArtifact,
    "evidence.environment.executionEngine.adapterArtifact",
    errors,
  );
  requireInteger(
    adapterArtifact.bytes,
    "evidence.environment.executionEngine.adapterArtifact.bytes",
    errors,
    1,
  );
  requireSha256(
    adapterArtifact.sha256,
    "evidence.environment.executionEngine.adapterArtifact.sha256",
    errors,
  );

  const semantic = requireRecord(root.semantic, "evidence.semantic", errors);
  checkEqual(semantic.outcome, "passed", "evidence.semantic.outcome", errors);
  const node = requireRecord(semantic.node, "evidence.semantic.node", errors);
  const browser = requireRecord(semantic.browser, "evidence.semantic.browser", errors);
  const expectedOutcome =
    checkpoint.nextStructuredStop.kind === "capability-checkpoint"
      ? "checkpoint-complete"
      : "structured-stop";
  checkEqual(node.outcome, expectedOutcome, "evidence.semantic.node.outcome", errors);
  checkEqual(
    browser.outcome,
    expectedOutcome,
    "evidence.semantic.browser.outcome",
    errors,
  );
  for (const [label, result] of [
    ["node", node],
    ["browser", browser],
  ]) {
    checkEqual(
      result.supportedCalls,
      checkpoint.activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount,
      `evidence.semantic.${label}.supportedCalls`,
      errors,
    );
    checkEqual(
      result.instructions,
      checkpoint.nextStructuredStop.instructionCount,
      `evidence.semantic.${label}.instructions`,
      errors,
    );
    checkEqual(
      result.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
      `evidence.semantic.${label}.memoryContentSha256`,
      errors,
    );
    requireSha256(
      result.callSemanticsSha256,
      `evidence.semantic.${label}.callSemanticsSha256`,
      errors,
    );
  }
  if (checkpoint.nextStructuredStop.kind === "capability-checkpoint") {
    const expectedCompletion = checkpoint.nextStructuredStop.completion;
    const expectedEndpoint = checkpoint.nextStructuredStop.nextEndpoint;
    for (const [label, result] of [
      ["node", node],
      ["browser", browser],
    ]) {
      const completion = requireRecord(
        result.checkpointCompletion,
        `evidence.semantic.${label}.checkpointCompletion`,
        errors,
      );
      for (const field of capabilityCompletionFields(expectedCompletion)) {
        checkEqual(
          completion[field],
          expectedCompletion[field],
          `evidence.semantic.${label}.checkpointCompletion.${field}`,
          errors,
        );
      }
      validatePointerReleaseCompletion(
        completion,
        expectedCompletion,
        `evidence.semantic.${label}.checkpointCompletion`,
        errors,
        { requireWriteReceipt: label === "node" },
      );
      if (expectedCompletion.requiredPointerMessages !== undefined) {
        const dispatchEntry = requireRecord(
          completion.dispatchEntry,
          `evidence.semantic.${label}.checkpointCompletion.dispatchEntry`,
          errors,
        );
        checkEqual(
          dispatchEntry.sequence,
          expectedCompletion.dispatchSequence,
          `evidence.semantic.${label}.checkpointCompletion.dispatchEntry.sequence`,
          errors,
        );
        checkEqual(
          dispatchEntry.trapInstructionCount,
          expectedCompletion.dispatchInstructionCount,
          `evidence.semantic.${label}.checkpointCompletion.dispatchEntry.trapInstructionCount`,
          errors,
        );
      }
      checkEqual(
        completion.instructionCount,
        checkpoint.nextStructuredStop.instructionCount,
        `evidence.semantic.${label}.checkpointCompletion.instructionCount`,
        errors,
      );
      checkEqual(
        completion.memoryContentSha256,
        expectedBrowser.memoryContentSha256,
        `evidence.semantic.${label}.checkpointCompletion.memoryContentSha256`,
        errors,
      );
      const endpoint = requireRecord(
        completion.nextEndpoint,
        `evidence.semantic.${label}.checkpointCompletion.nextEndpoint`,
        errors,
      );
      checkEqual(
        endpoint.id,
        expectedEndpoint.endpointId,
        `evidence.semantic.${label}.checkpointCompletion.nextEndpoint.id`,
        errors,
      );
      checkEqual(
        endpoint.address,
        Number.parseInt(expectedEndpoint.endpointAddress, 16),
        `evidence.semantic.${label}.checkpointCompletion.nextEndpoint.address`,
        errors,
      );
      if (typeof endpoint.module === "string" && endpoint.module.length > 0) {
        try {
          checkEqual(
            `${endpoint.module}!${formatRuntimeEndpointSymbol(endpoint)}`.toLowerCase(),
            formatManifestRuntimeEndpointIdentity(expectedEndpoint).toLowerCase(),
            `evidence.semantic.${label}.checkpointCompletion.nextEndpoint.identity`,
            errors,
          );
        } catch (error) {
          errors.push(
            `evidence.semantic.${label}.checkpointCompletion.nextEndpoint.identity is invalid: ${error.message}`,
          );
        }
      } else {
        errors.push(
          `evidence.semantic.${label}.checkpointCompletion.nextEndpoint.module must be a non-empty string`,
        );
      }
      if (expectedCompletion.roundtrips !== undefined) {
        checkEqual(
          JSON.stringify(completion.roundtrips),
          JSON.stringify(expectedCompletion.roundtrips),
          `evidence.semantic.${label}.checkpointCompletion.roundtrips`,
          errors,
        );
      }
    }
  }
  checkEqual(
    node.callSemanticsSha256,
    browser.callSemanticsSha256,
    "evidence.semantic.callSemanticsSha256",
    errors,
  );
  checkEqual(
    node.callSemanticsSha256,
    expectedBrowser.callSemanticsSha256,
    "evidence.semantic.callSemanticsSha256 checkpoint digest",
    errors,
  );
  checkEqual(
    node.implementedApiContracts,
    checkpoint.activeRuntimeCheckpoint.genericDispatcherContractCount,
    "evidence.semantic.node.implementedApiContracts",
    errors,
  );
  checkEqual(
    node.finalGeneration,
    expectedBrowser.nodeFinalGeneration,
    "evidence.semantic.node.finalGeneration",
    errors,
  );
  checkEqual(
    browser.finalGeneration,
    expectedBrowser.finalGeneration,
    "evidence.semantic.browser.finalGeneration",
    errors,
  );
  validateExecutionStateMachine(
    browser.executionStateMachine,
    "evidence.semantic.browser.executionStateMachine",
    errors,
  );
  const browserEngine = requireRecord(
    browser.executionEngine,
    "evidence.semantic.browser.executionEngine",
    errors,
  );
  checkEqual(
    browserEngine.schema,
    "vine.execution-engine-profile",
    "evidence.semantic.browser.executionEngine.schema",
    errors,
  );
  checkEqual(
    browserEngine.schemaVersion,
    1,
    "evidence.semantic.browser.executionEngine.schemaVersion",
    errors,
  );
  checkEqual(
    browserEngine.engine,
    executionEngine.engine,
    "evidence.semantic.browser.executionEngine.engine",
    errors,
  );
  checkEqual(
    browserEngine.sourceRevision,
    executionEngine.sourceRevision,
    "evidence.semantic.browser.executionEngine.sourceRevision",
    errors,
  );
  checkEqual(
    browserEngine.cpuContextAbi?.version,
    cpuContextAbi.version,
    "evidence.semantic.browser.executionEngine.cpuContextAbi.version",
    errors,
  );
  checkEqual(
    browserEngine.cpuContextAbi?.bytes,
    cpuContextAbi.bytes,
    "evidence.semantic.browser.executionEngine.cpuContextAbi.bytes",
    errors,
  );
  checkEqual(
    browserEngine.artifact?.bytes,
    adapterArtifact.bytes,
    "evidence.semantic.browser.executionEngine.artifact.bytes",
    errors,
  );
  checkEqual(
    browserEngine.artifact?.sha256,
    adapterArtifact.sha256,
    "evidence.semantic.browser.executionEngine.artifact.sha256",
    errors,
  );
  checkEqual(
    browser.executionEngineLinearMemoryBytes,
    expectedBrowser.executionEngineLinearMemoryBytes,
    "evidence.semantic.browser.executionEngineLinearMemoryBytes",
    errors,
  );
  checkEqual(
    semantic.memoryEquivalent,
    true,
    "evidence.semantic.memoryEquivalent",
    errors,
  );

  const next = requireRecord(
    semantic.nextStructuredStop,
    "evidence.semantic.nextStructuredStop",
    errors,
  );
  for (const field of [
    "kind",
    "nextCheckpoint",
    "instructionCount",
    "capabilityCluster",
    "reason",
  ]) {
    checkEqual(
      next[field],
      checkpoint.nextStructuredStop[field],
      `evidence.semantic.nextStructuredStop.${field}`,
      errors,
    );
  }
  if (checkpoint.nextStructuredStop.kind === "capability-checkpoint") {
    const completion = requireRecord(
      next.completion,
      "evidence.semantic.nextStructuredStop.completion",
      errors,
    );
    const expectedCompletion = checkpoint.nextStructuredStop.completion;
    for (const field of capabilityCompletionFields(expectedCompletion, true)) {
      checkEqual(
        completion[field],
        expectedCompletion[field],
        `evidence.semantic.nextStructuredStop.completion.${field}`,
        errors,
      );
    }
    const endpoint = requireRecord(
      next.nextEndpoint,
      "evidence.semantic.nextStructuredStop.nextEndpoint",
      errors,
    );
    for (const field of [
      "endpointKind",
      "endpointId",
      "api",
      "endpointAddress",
    ]) {
      checkEqual(
        endpoint[field],
        checkpoint.nextStructuredStop.nextEndpoint[field],
        `evidence.semantic.nextStructuredStop.nextEndpoint.${field}`,
        errors,
      );
    }
  } else if (checkpoint.nextStructuredStop.kind === "api-endpoint") {
    for (const field of [
      "api",
      "endpointKind",
      "endpointId",
      "endpointAddress",
      "iatAddress",
      "returnAddress",
    ]) {
      checkEqual(
        next[field],
        checkpoint.nextStructuredStop[field],
        `evidence.semantic.nextStructuredStop.${field}`,
        errors,
      );
    }
  } else if (checkpoint.nextStructuredStop.kind === "com-method") {
    for (const field of [
      "endpointKind",
      "interface",
      "vtableSlot",
      "method",
      "thisPointer",
      "endpointAddress",
      "returnAddress",
    ]) {
      checkEqual(
        next[field],
        checkpoint.nextStructuredStop[field],
        `evidence.semantic.nextStructuredStop.${field}`,
        errors,
      );
    }
  } else {
    checkEqual(next.endpoint, null, "evidence.semantic.nextStructuredStop.endpoint", errors);
    const nextFault = requireRecord(
      next.fault,
      "evidence.semantic.nextStructuredStop.fault",
      errors,
    );
    const expectedFault = checkpoint.nextStructuredStop.fault;
    for (const field of [
      "class",
      "stage",
      "code",
      "instructionPointer",
      "previousInstructionPointer",
      "faultAddress",
      "pageFaultErrorCode",
    ]) {
      checkEqual(
        nextFault[field],
        expectedFault[field],
        `evidence.semantic.nextStructuredStop.fault.${field}`,
        errors,
      );
    }
  }

  const cleanup = requireRecord(semantic.cleanup, "evidence.semantic.cleanup", errors);
  for (const field of [
    "pendingHostCalls",
    "pendingCallbacks",
    "cancelPendingHostCalls",
    "cancelPendingCallbacks",
    "watchdogPendingHostCalls",
    "watchdogPendingCallbacks",
  ]) {
    checkEqual(cleanup[field], 0, `evidence.semantic.cleanup.${field}`, errors);
  }
  checkEqual(
    cleanup.cancelInstructions,
    expectedBrowser.cancelInstructions,
    "evidence.semantic.cleanup.cancelInstructions",
    errors,
  );
  checkEqual(
    cleanup.watchdogInstructions,
    expectedBrowser.watchdogInstructions,
    "evidence.semantic.cleanup.watchdogInstructions",
    errors,
  );

  const artifacts = requireRecord(root.artifacts, "evidence.artifacts", errors);
  for (const name of [
    "nodeReport",
    "browserReport",
    "toolchainReport",
    "v86AdapterReport",
    "browserBuildManifest",
    "runtimeCoreWasm",
    "v86AdapterWasm",
    "cargoLock",
    "npmLock",
  ]) {
    const artifact = requireRecord(artifacts[name], `evidence.artifacts.${name}`, errors);
    requireString(artifact.path, `evidence.artifacts.${name}.path`, errors);
    requireInteger(artifact.bytes, `evidence.artifacts.${name}.bytes`, errors, 1);
    requireSha256(artifact.sha256, `evidence.artifacts.${name}.sha256`, errors);
  }
  for (const name of ["nodeReport", "browserReport"]) {
    const artifact = requireRecord(artifacts[name], `evidence.artifacts.${name}`, errors);
    if (
      Number.isSafeInteger(artifact.bytes) &&
      artifact.bytes > MAXIMUM_SEMANTIC_REPORT_BYTES
    ) {
      errors.push(
        `evidence.artifacts.${name}.bytes must be <= ${MAXIMUM_SEMANTIC_REPORT_BYTES}`,
      );
    }
  }
  checkEqual(
    artifacts.runtimeCoreWasm.bytes,
    expectedBrowser.runtimeCoreWasmBytes,
    "evidence.artifacts.runtimeCoreWasm.bytes",
    errors,
  );
  checkEqual(
    artifacts.runtimeCoreWasm.sha256,
    expectedBrowser.runtimeCoreWasmSha256,
    "evidence.artifacts.runtimeCoreWasm.sha256",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.bytes,
    adapterArtifact.bytes,
    "evidence.artifacts.v86AdapterWasm.bytes",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.sha256,
    adapterArtifact.sha256,
    "evidence.artifacts.v86AdapterWasm.sha256",
    errors,
  );
  checkEqual(
    artifacts.v86AdapterWasm.sha256,
    checkpoint.browserCheckpoint.bgiBenchmarkEvidence.v86AdapterWasmSha256,
    "evidence.artifacts.v86AdapterWasm checkpoint digest",
    errors,
  );

  const performance = requireRecord(
    root.performanceObservation,
    "evidence.performanceObservation",
    errors,
  );
  checkEqual(
    performance.classification,
    "observation-not-semantic-gate",
    "evidence.performanceObservation.classification",
    errors,
  );
  checkEqual(
    performance.status,
    checkpoint.browserCheckpoint.bgiBenchmark,
    "evidence.performanceObservation.status",
    errors,
  );
  checkEqual(
    performance.configuration,
    checkpoint.browserCheckpoint.bgiBenchmarkEvidence.configuration,
    "evidence.performanceObservation.configuration",
    errors,
  );
  const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
  checkEqual(
    performance.benchmarkClassification,
    benchmark.performanceClassification,
    "evidence.performanceObservation.benchmarkClassification",
    errors,
  );
  checkEqual(
    performance.decisionStatus,
    benchmark.decisionStatus,
    "evidence.performanceObservation.decisionStatus",
    errors,
  );
  checkEqual(
    performance.decisionReason,
    benchmark.decisionReason,
    "evidence.performanceObservation.decisionReason",
    errors,
  );
  checkEqual(
    performance.thresholdEvaluationCount,
    benchmark.thresholdEvaluationCount,
    "evidence.performanceObservation.thresholdEvaluationCount",
    errors,
  );
  requireInteger(
    performance.durationMilliseconds,
    "evidence.performanceObservation.durationMilliseconds",
    errors,
    1,
  );
  requireInteger(
    performance.completedSessions,
    "evidence.performanceObservation.completedSessions",
    errors,
    1,
  );
  requireInteger(
    performance.failedSessions,
    "evidence.performanceObservation.failedSessions",
    errors,
  );
  for (const field of [
    "durationMilliseconds",
    "completedSessions",
    "failedSessions",
    "reportSha256",
  ]) {
    checkEqual(
      performance[field],
      benchmark[field],
      `evidence.performanceObservation.${field}`,
      errors,
    );
  }
  const thresholdFailureIds = requireStringArray(
    performance.thresholdFailureIds,
    "evidence.performanceObservation.thresholdFailureIds",
    errors,
  );
  checkEqual(
    thresholdFailureIds.length,
    benchmark.thresholdFailures,
    "evidence.performanceObservation.thresholdFailureIds.length",
    errors,
  );
  checkEqual(
    JSON.stringify(thresholdFailureIds),
    JSON.stringify(benchmark.thresholdFailureIds),
    "evidence.performanceObservation.thresholdFailureIds",
    errors,
  );
  requireSha256(
    performance.reportSha256,
    "evidence.performanceObservation.reportSha256",
    errors,
  );

  requireSha256(root.payloadSha256, "evidence.payloadSha256", errors);
  if (typeof root.payloadSha256 === "string" && SHA256.test(root.payloadSha256)) {
    checkEqual(
      root.payloadSha256,
      digestPayload(root),
      "evidence.payloadSha256",
      errors,
    );
  }
  return errors;
}

export function assertValidPrivateTargetEvidence(evidence, checkpoint) {
  const errors = validatePrivateTargetEvidence(evidence, checkpoint);
  if (errors.length > 0) {
    throw new Error(`private target evidence is invalid:\n- ${errors.join("\n- ")}`);
  }
  return evidence;
}
