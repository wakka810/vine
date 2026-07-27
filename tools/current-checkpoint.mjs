import fs from "node:fs";

export const README_STATUS_START = "<!-- BEGIN GENERATED CURRENT STATUS -->";
export const README_STATUS_END = "<!-- END GENERATED CURRENT STATUS -->";

const SHA256 = /^[0-9a-f]{64}$/;
const HEX32 = /^0x[0-9a-f]{1,8}$/;

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

function requireFiniteNumber(value, label, errors) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    errors.push(`${label} must be a finite number`);
  }
}

function requireSha256(value, label, errors) {
  if (typeof value !== "string" || !SHA256.test(value)) {
    errors.push(`${label} must be a lowercase SHA-256 digest`);
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

function requireIntegerArray(value, label, errors) {
  if (
    !Array.isArray(value) ||
    value.some((item) => !Number.isSafeInteger(item))
  ) {
    errors.push(`${label} must be an array of integers`);
    return [];
  }
  if (new Set(value).size !== value.length) {
    errors.push(`${label} must not contain duplicates`);
  }
  return value;
}

function requireOrderedIntegerArray(value, label, errors) {
  if (
    !Array.isArray(value) ||
    value.some((item) => !Number.isSafeInteger(item))
  ) {
    errors.push(`${label} must be an array of integers`);
    return [];
  }
  return value;
}

function requireOrderedStringArray(value, label, errors) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return [];
  }
  return value;
}

function validateExecutionStateMachine(value, label, errors) {
  const stateMachine = requireRecord(value, label, errors);
  for (const scenario of ["success", "cancellation", "watchdog"]) {
    const outcome = requireRecord(
      stateMachine[scenario],
      `${label}.${scenario}`,
      errors,
    );
    if (outcome.finalState !== "stopped") {
      errors.push(`${label}.${scenario}.finalState must be stopped`);
    }
    requireInteger(
      outcome.transitionCount,
      `${label}.${scenario}.transitionCount`,
      errors,
      1,
    );
    if (outcome.lastTransitionTo !== "stopped") {
      errors.push(`${label}.${scenario}.lastTransitionTo must be stopped`);
    }
    requireInteger(
      outcome.pendingHostRequestRecords,
      `${label}.${scenario}.pendingHostRequestRecords`,
      errors,
    );
    if (outcome.pendingHostRequestRecords !== 0) {
      errors.push(`${label}.${scenario} must not retain host request records`);
    }
    if (scenario !== "watchdog") {
      if (outcome.watchdogContext !== null) {
        errors.push(`${label}.${scenario}.watchdogContext must be null`);
      }
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
      (typeof context.requestId !== "string" ||
        !/^(0|[1-9][0-9]*)$/u.test(context.requestId))
    ) {
      errors.push(
        `${label}.watchdog.watchdogContext.requestId must be null or a decimal ID`,
      );
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

export function validateCurrentCheckpoint(manifest) {
  const errors = [];
  const root = requireRecord(manifest, "manifest", errors);
  const primaryMainLoopProgressionActive =
    root.activeRuntimeCheckpoint?.profile === "primary-main-loop-progression";
  const leaveCriticalSectionActive =
    root.activeRuntimeCheckpoint?.profile === "kernel32-leave-critical-section";
  const enterCriticalSectionContinuationActive =
    root.activeRuntimeCheckpoint?.profile ===
    "kernel32-enter-critical-section-continuation";
  const leaveCriticalSectionContinuationActive =
    root.activeRuntimeCheckpoint?.profile ===
    "kernel32-leave-critical-section-continuation";
  const dragAcceptFilesActive =
    root.activeRuntimeCheckpoint?.profile === "shell32-drag-accept-files";
  const d3d9InitialSamplerStatesActive =
    root.activeRuntimeCheckpoint?.profile === "d3d9-initial-sampler-states";
  const d3d9BeginSceneActive =
    d3d9InitialSamplerStatesActive ||
    root.activeRuntimeCheckpoint?.profile === "d3d9-begin-scene";
  const d3d9UpdateTextureActive =
    d3d9BeginSceneActive ||
    root.activeRuntimeCheckpoint?.profile === "d3d9-update-texture";
  const d3d9AddDirtyRectActive =
    d3d9UpdateTextureActive ||
    root.activeRuntimeCheckpoint?.profile === "d3d9-add-dirty-rect";
  const setRectActive =
    d3d9AddDirtyRectActive ||
    root.activeRuntimeCheckpoint?.profile === "user32-set-rect";
  const d3d9ClearActive =
    setRectActive || root.activeRuntimeCheckpoint?.profile === "d3d9-clear";
  const updateWindowActive =
    d3d9ClearActive ||
    root.activeRuntimeCheckpoint?.profile === "user32-update-window";
  const showWindowNormalActive =
    updateWindowActive ||
    root.activeRuntimeCheckpoint?.profile === "user32-show-window-normal";
  const setWindowPosFrameChangeMoveActive =
    showWindowNormalActive ||
    root.activeRuntimeCheckpoint?.profile ===
    "user32-set-window-pos-frame-change-move";
  const d3d9RecreatedTexturesActive =
    setWindowPosFrameChangeMoveActive ||
    root.activeRuntimeCheckpoint?.profile === "d3d9-recreated-textures";
  const setWindowPosRepeatActivationActive =
    d3d9RecreatedTexturesActive ||
    root.activeRuntimeCheckpoint?.profile ===
      "user32-set-window-pos-repeat-activation";
  const createDirectoryAActive =
    setWindowPosRepeatActivationActive ||
    root.activeRuntimeCheckpoint?.profile === "kernel32-create-directory-a";
  const getFileAttributesAActive =
    root.activeRuntimeCheckpoint?.profile ===
      "kernel32-get-file-attributes-a" || createDirectoryAActive;
  const archiveSourceReadActive =
    root.activeRuntimeCheckpoint?.profile === "kernel32-archive-source-read" ||
    root.activeRuntimeCheckpoint?.profile === "kernel32-sleep-zero" ||
    root.activeRuntimeCheckpoint?.profile === "kernel32-global-memory-status" ||
    getFileAttributesAActive;
  const globalMemoryStatusActive =
    root.activeRuntimeCheckpoint?.profile === "kernel32-global-memory-status" ||
    getFileAttributesAActive;
  const sleepZeroActive =
    root.activeRuntimeCheckpoint?.profile === "kernel32-sleep-zero" ||
    globalMemoryStatusActive;
  const mainLoopContinuationActive =
    primaryMainLoopProgressionActive ||
    leaveCriticalSectionActive ||
    enterCriticalSectionContinuationActive ||
    leaveCriticalSectionContinuationActive ||
    dragAcceptFilesActive ||
    archiveSourceReadActive;
  const setWindowPosCallCount = setWindowPosFrameChangeMoveActive
    ? 6
    : setWindowPosRepeatActivationActive
      ? 5
      : 4;
  const setWindowPosSequences = setWindowPosFrameChangeMoveActive
    ? "1402,1408,17831,89387,108718,127346"
    : setWindowPosRepeatActivationActive
      ? "1402,1408,17831,89387,108718"
      : "1402,1408,17831,89387";
  const setWindowPosInstructionCounts = setWindowPosFrameChangeMoveActive
    ? "11336519,11336704,11666552,15952303,28273977,28760801"
    : setWindowPosRepeatActivationActive
      ? "11336519,11336704,11666552,15952303,28273977"
      : "11476612,11476797,11806645,15951782";
  const setWindowPosReturnAddresses = setWindowPosFrameChangeMoveActive
    ? "0x10067f88,0x10067f9f,0x1005cb4b,0x1005cb4b,0x1005cb4b,0x100938c2"
    : setWindowPosRepeatActivationActive
      ? "0x10067f88,0x10067f9f,0x1005cb4b,0x1005cb4b,0x1005cb4b"
      : "0x10067f88,0x10067f9f,0x1005cb4b,0x1005cb4b";
  const setWindowPosResults = setWindowPosFrameChangeMoveActive
    ? "1,1,1,1,1,1"
    : setWindowPosRepeatActivationActive
      ? "1,1,1,1,1"
      : "1,1,1,1";
  const setWindowPosLastErrors = setWindowPosFrameChangeMoveActive
    ? "0,0,0,0,0,0"
    : setWindowPosRepeatActivationActive
      ? "0,0,0,0,0"
      : "0,0,0,0";
  const setWindowPosImplementationId = setWindowPosRepeatActivationActive
    ? "user32.set-window-pos.v5"
    : "user32.set-window-pos.v3";
  const sleepZeroCallCount = d3d9RecreatedTexturesActive ? 480 : 240;
  const sleepZeroSequences = d3d9RecreatedTexturesActive
    ? "89430,126996"
    : "89430,107670";
  const sleepZeroOccurrences = d3d9RecreatedTexturesActive ? "1,480" : "1,240";
  const sleepZeroInstructionCounts = d3d9RecreatedTexturesActive
    ? "15954287,28612007"
    : createDirectoryAActive
      ? "15954287,16290526"
      : "15953766,16290005";
  const timerCycleCount = d3d9RecreatedTexturesActive
    ? 9_186
    : createDirectoryAActive
      ? 6_974
      : getFileAttributesAActive
        ? 6_937
        : sleepZeroActive
          ? 6_910
          : archiveSourceReadActive
            ? 4_716
            : dragAcceptFilesActive
              ? 4_706
              : mainLoopContinuationActive
                ? 7
                : 3;
  const timerArmCount = d3d9RecreatedTexturesActive ? 9_187 : timerCycleCount;
  const timerSetSequences = d3d9RecreatedTexturesActive
    ? "18681,127344"
    : createDirectoryAActive
      ? "18681,108706"
      : getFileAttributesAActive
        ? "18681,108148"
        : sleepZeroActive
          ? "18681,107739"
          : archiveSourceReadActive
            ? "18681,89364"
            : dragAcceptFilesActive
              ? "18681,89217"
              : mainLoopContinuationActive
                ? "18681,18689,18697,18705,18713,18721,18729"
                : "18681,18689,18697";
  const timerSetInstructionCounts = d3d9RecreatedTexturesActive
    ? "11974074,28759195"
    : createDirectoryAActive
      ? "11974074,28269117"
      : getFileAttributesAActive
        ? "11973553,16432845"
        : sleepZeroActive
          ? "11973553,16291292"
          : archiveSourceReadActive
            ? "11973553,15942875"
            : dragAcceptFilesActive
              ? "11973553,15600688"
              : mainLoopContinuationActive
                ? "11973553,11973700,11973847,11973994,11974141,11974288,11974435"
                : "11973553,11973700,11973847";
  const timerWaitSequences = d3d9RecreatedTexturesActive
    ? "18682,127337"
    : createDirectoryAActive
      ? "18682,108707"
      : getFileAttributesAActive
        ? "18682,108149"
        : sleepZeroActive
          ? "18682,107740"
          : archiveSourceReadActive
            ? "18682,89365"
            : dragAcceptFilesActive
              ? "18682,89218"
              : mainLoopContinuationActive
                ? "18682,18690,18698,18706,18714,18722,18730"
                : "18682,18690,18698";
  const timerWaitInstructionCounts = d3d9RecreatedTexturesActive
    ? "11974080,28759054"
    : createDirectoryAActive
      ? "11974080,28269123"
      : getFileAttributesAActive
        ? "11973559,16432851"
        : sleepZeroActive
          ? "11973559,16291298"
          : archiveSourceReadActive
            ? "11973559,15942881"
            : dragAcceptFilesActive
              ? "11973559,15600694"
              : mainLoopContinuationActive
                ? "11973559,11973706,11973853,11974000,11974147,11974294,11974441"
                : "11973559,11973706,11973853";
  const timerGenerations = d3d9RecreatedTexturesActive
    ? "1,9186"
    : createDirectoryAActive
      ? "1,6974"
      : getFileAttributesAActive
        ? "1,6937"
        : sleepZeroActive
          ? "1,6910"
          : archiveSourceReadActive
            ? "1,4716"
            : dragAcceptFilesActive
              ? "1,4706"
              : mainLoopContinuationActive
                ? "1,2,3,4,5,6,7"
                : "1,2,3";
  const timerDeadlines = d3d9RecreatedTexturesActive
    ? "5000,45930000"
    : createDirectoryAActive
      ? "5000,34870000"
      : getFileAttributesAActive
        ? "5000,34685000"
        : sleepZeroActive
          ? "5000,34550000"
          : archiveSourceReadActive
            ? "5000,23580000"
            : dragAcceptFilesActive
              ? "5000,23530000"
              : mainLoopContinuationActive
                ? "5000,10000,15000,20000,25000,30000,35000"
                : "5000,10000,15000";
  const timerPacingRequestIds = d3d9RecreatedTexturesActive
    ? "3,9197"
    : createDirectoryAActive
      ? "3,6985"
      : getFileAttributesAActive
        ? "3,6942"
        : sleepZeroActive
          ? "3,6915"
          : archiveSourceReadActive
            ? "3,4721"
            : dragAcceptFilesActive
              ? "3,4708"
              : mainLoopContinuationActive
                ? "3,4,5,6,7,8,9"
                : "3,4,5";
  const timerPacingRegistrationIds = d3d9RecreatedTexturesActive
    ? "1,9186"
    : createDirectoryAActive
      ? "1,6974"
      : getFileAttributesAActive
        ? "1,6937"
        : sleepZeroActive
          ? "1,6910"
          : archiveSourceReadActive
            ? "1,4716"
            : dragAcceptFilesActive
              ? "1,4706"
              : mainLoopContinuationActive
                ? "1,2,3,4,5,6,7"
                : "1,2,3";
  const timerPacingClockGenerations = d3d9RecreatedTexturesActive
    ? "0,9185"
    : createDirectoryAActive
      ? "0,6973"
      : getFileAttributesAActive
        ? "0,6936"
        : sleepZeroActive
          ? "0,6909"
          : archiveSourceReadActive
            ? "0,4715"
            : dragAcceptFilesActive
              ? "0,4705"
              : mainLoopContinuationActive
                ? "0,1,2,3,4,5,6"
                : "0,1,2";
  const timerNextRegistrationIds = d3d9RecreatedTexturesActive
    ? "2,9187"
    : createDirectoryAActive
      ? "2,6975"
      : getFileAttributesAActive
        ? "2,6938"
        : sleepZeroActive
          ? "2,6911"
          : archiveSourceReadActive
            ? "2,4717"
            : dragAcceptFilesActive
              ? "2,4707"
              : mainLoopContinuationActive
                ? "2,3,4,5,6,7,8"
                : "2,3,4";
  const profileExpectations = d3d9InitialSamplerStatesActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_769,
        browserMappingDirtyPages: 109_858,
        browserSuccessExecutionTransitions: 416_539,
        latestSchema: "vine.v86-bgi-d3d9-initial-sampler-states",
        latestWritebackBatches: 127_462,
        latestWritebackDirtyPages: 175_984,
        latestWritebackChangedBytes: 6_619_837,
        latestHostDeltaBatches: 106_769,
        latestFinalMemoryGeneration: 235_514,
        semanticCallDigestSha256:
          "80e3477ae2d017c3fbd72a545c8655d317b2dc0dad11d877a4ee8372a7190c42",
        memoryContentSha256:
          "68d7490f9f03716b648544083c8ba006959e2792659f172c1ffe7c2cc8ed6ca8",
      }
    : d3d9BeginSceneActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_769,
        browserMappingDirtyPages: 109_858,
        browserSuccessExecutionTransitions: 416_529,
        latestSchema: "vine.v86-bgi-d3d9-begin-scene",
        latestWritebackBatches: 127_457,
        latestWritebackDirtyPages: 175_979,
        latestWritebackChangedBytes: 6_619_810,
        latestHostDeltaBatches: 106_769,
        latestFinalMemoryGeneration: 235_509,
        semanticCallDigestSha256:
          "cc5c45b8a381b78aef9f92199249d28f2a08d783ce302747b29096b5cd579336",
        memoryContentSha256:
          "a8dae8df1346601cdaf44e32e3f082c29b1e88aae363c561fffe70e32036b2cf",
      }
    : d3d9UpdateTextureActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_769,
        browserMappingDirtyPages: 109_858,
        browserSuccessExecutionTransitions: 416_527,
        latestSchema: "vine.v86-bgi-d3d9-update-texture",
        latestWritebackBatches: 127_456,
        latestWritebackDirtyPages: 175_978,
        latestWritebackChangedBytes: 6_619_792,
        latestHostDeltaBatches: 106_769,
        latestFinalMemoryGeneration: 235_508,
        semanticCallDigestSha256:
          "b5f44675f551e3986efd94d97b0a50dcb26431f22697fe16178824bafcdc828d",
        memoryContentSha256:
          "6a33233c5973dc1d97badc37755780ed99688c0fa835c0d2d041acc9e231e57a",
      }
    : d3d9AddDirtyRectActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_769,
        browserMappingDirtyPages: 109_858,
        browserSuccessExecutionTransitions: 416_525,
        latestSchema: "vine.v86-bgi-d3d9-add-dirty-rect",
        latestWritebackBatches: 127_455,
        latestWritebackDirtyPages: 175_977,
        latestWritebackChangedBytes: 6_619_787,
        latestHostDeltaBatches: 106_769,
        latestFinalMemoryGeneration: 235_507,
        semanticCallDigestSha256:
          "af311f2769ed51c752349385619cb00400ecb468a866133f5cba71a7c80c373a",
        memoryContentSha256:
          "2e8deed2908c310e490b75ba714ebc8eb615a87b66efab2ab49287784d0a408a",
      }
    : setRectActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_769,
        browserMappingDirtyPages: 109_858,
        browserSuccessExecutionTransitions: 416_523,
        latestSchema: "vine.v86-bgi-user32-set-rect",
        latestWritebackBatches: 127_454,
        latestWritebackDirtyPages: 175_976,
        latestWritebackChangedBytes: 6_619_715,
        latestHostDeltaBatches: 106_769,
        latestFinalMemoryGeneration: 235_506,
        semanticCallDigestSha256:
          "01f5a4a0ca80adefdfde1a8d7a7ab3a229c76fc4d35271ebacc46cc61340761f",
        memoryContentSha256:
          "870976fcdad8dd1de12cb1d2279279c3764ee320e30e14e9640d5ce346296afb",
      }
    : d3d9ClearActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_768,
        browserMappingDirtyPages: 109_857,
        browserSuccessExecutionTransitions: 416_521,
        latestSchema: "vine.v86-bgi-d3d9-clear",
        latestWritebackBatches: 127_453,
        latestWritebackDirtyPages: 175_975,
        latestWritebackChangedBytes: 6_619_704,
        latestHostDeltaBatches: 106_768,
        latestFinalMemoryGeneration: 235_504,
        semanticCallDigestSha256:
          "b8d2e16f49c417e6deebf9fa52a40195204ea2502a65b5299d9867b5229eb9cc",
        memoryContentSha256:
          "b567254a8ef1a0ea18a6a983b10293b57b25ad5b9864bda28c28598d5200e5ae",
      }
    : updateWindowActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_768,
        browserMappingDirtyPages: 109_857,
        browserSuccessExecutionTransitions: 416_519,
        latestSchema: "vine.v86-bgi-user32-update-window",
        latestWritebackBatches: 127_452,
        latestWritebackDirtyPages: 175_974,
        latestWritebackChangedBytes: 6_619_682,
        latestHostDeltaBatches: 106_768,
        latestFinalMemoryGeneration: 235_503,
        semanticCallDigestSha256:
          "33ef06b514d5d26eefb8d2ecdea0e569197752884b798af5787e3e0ef8d0b4b5",
        memoryContentSha256:
          "ceaaf33c0382b0f1859997d2f114a1d85eeed1ce5090bb9e7f6fabcc49de03ff",
      }
    : setWindowPosFrameChangeMoveActive
    ? {
        benchmarkObservation: "current-archive-source-read-observation",
        browserMappingDeltas: 106_766,
        browserMappingDirtyPages: 109_855,
        browserSuccessExecutionTransitions: 416_510,
        latestSchema: "vine.v86-bgi-user32-set-window-pos-frame-change-move",
        latestWritebackBatches: 127_448,
        latestWritebackDirtyPages: 175_970,
        latestWritebackChangedBytes: 6_619_588,
        latestHostDeltaBatches: 106_766,
        latestFinalMemoryGeneration: 235_497,
        semanticCallDigestSha256:
          "6288683381bcc5df62a20abad70a9b73bbedb153dd0f5f8aa9cfe2e3abde300c",
        memoryContentSha256:
          "b7a48b1a742ff2e7bf477880574bdad5f9334288f4adc3c9d6a8616ed49ad628",
      }
    : d3d9RecreatedTexturesActive
      ? {
          benchmarkObservation: "current-archive-source-read-observation",
          browserMappingDeltas: 106_761,
          browserMappingDirtyPages: 109_847,
          browserSuccessExecutionTransitions: 416_491,
          latestSchema: "vine.v86-bgi-d3d9-recreated-textures",
          latestWritebackBatches: 127_439,
          latestWritebackDirtyPages: 175_958,
          latestWritebackChangedBytes: 6_619_474,
          latestHostDeltaBatches: 106_761,
          latestFinalMemoryGeneration: 235_480,
          semanticCallDigestSha256:
            "481f448cc414e9edf6e9a74c577349d811735cda65e4a2ed8481e42cee672c3b",
          memoryContentSha256:
            "b4e097035105aee8547a5d8c02fffae3bafaa8a943b484f1f0de8cb3ad1a9973",
        }
      : setWindowPosRepeatActivationActive
        ? {
            benchmarkObservation: "current-archive-source-read-observation",
            browserMappingDeltas: 92_825,
            browserMappingDirtyPages: 95_902,
            browserSuccessExecutionTransitions: 353_544,
            latestSchema:
              "vine.v86-bgi-user32-set-window-pos-repeat-activation",
            latestWritebackBatches: 108_830,
            latestWritebackDirtyPages: 150_285,
            latestWritebackChangedBytes: 6_369_334,
            latestHostDeltaBatches: 92_825,
            latestFinalMemoryGeneration: 202_769,
            semanticCallDigestSha256:
              "7cb784a2fb34fa1c00e0be37385fcc83b1959c98df5efa063fd2c386503938c2",
            memoryContentSha256:
              "cfc177c8645e76604946c2392b272e7b8ef945b1371d21736938deed45e056ac",
          }
        : createDirectoryAActive
          ? {
              benchmarkObservation: "current-archive-source-read-observation",
              browserMappingDeltas: 92_815,
              browserMappingDirtyPages: 95_893,
              browserSuccessExecutionTransitions: 353_486,
              latestSchema: "vine.v86-bgi-kernel32-create-directory-a",
              latestWritebackBatches: 108_810,
              latestWritebackDirtyPages: 150_254,
              latestWritebackChangedBytes: 6_368_965,
              latestHostDeltaBatches: 92_815,
              latestFinalMemoryGeneration: 202_575,
              semanticCallDigestSha256:
                "e925e87a010017aa85316f8564a8bcbdac81fe59f87b84e5c288a865db0adb65",
              memoryContentSha256:
                "0b1e924899c1bbcf774d70a88b0bbfb796d1be420b14d7377384a63b20addd75",
            }
          : getFileAttributesAActive
            ? {
                benchmarkObservation: "current-archive-source-read-observation",
                browserMappingDeltas: 92_492,
                browserMappingDirtyPages: 94_681,
                browserSuccessExecutionTransitions: 349_581,
                latestSchema: "vine.v86-bgi-kernel32-get-file-attributes-a",
                latestWritebackBatches: 108_267,
                latestWritebackDirtyPages: 148_855,
                latestWritebackChangedBytes: 4_925_375,
                latestHostDeltaBatches: 92_492,
                latestFinalMemoryGeneration: 201_672,
                semanticCallDigestSha256:
                  "b1846ec7eb00de76f15194a14e4877f2fb1871e0ac49a00f3b81b8dc6f0583a2",
                memoryContentSha256:
                  "b0294a8cc38e30d97d0851e4abb0bd0387dbccb6d79fed8ce7b8872b3f04a345",
              }
            : globalMemoryStatusActive
              ? {
                  benchmarkObservation:
                    "current-archive-source-read-observation",
                  browserMappingDeltas: 92_327,
                  browserMappingDirtyPages: 94_516,
                  browserSuccessExecutionTransitions: 348_240,
                  latestSchema: "vine.v86-bgi-kernel32-global-memory-status",
                  latestWritebackBatches: 107_849,
                  latestWritebackDirtyPages: 148_256,
                  latestWritebackChangedBytes: 4_917_540,
                  latestHostDeltaBatches: 92_327,
                  latestFinalMemoryGeneration: 201_089,
                  semanticCallDigestSha256:
                    "ef1e28a26937875cb742983528bf5a9cd278aab6bd6fee49ac8de1415418a884",
                  memoryContentSha256:
                    "0fb78a7fa320825c0ae840a28ad90def5246ce164110c521cc958eb9b2483246",
                }
              : sleepZeroActive
                ? {
                    benchmarkObservation:
                      "current-archive-source-read-observation",
                    browserMappingDeltas: 92_304,
                    browserMappingDirtyPages: 94_493,
                    browserSuccessExecutionTransitions: 348_147,
                    latestSchema: "vine.v86-bgi-kernel32-sleep-zero",
                    latestWritebackBatches: 107_818,
                    latestWritebackDirtyPages: 148_160,
                    latestWritebackChangedBytes: 4_916_226,
                    latestHostDeltaBatches: 92_304,
                    latestFinalMemoryGeneration: 201_035,
                    semanticCallDigestSha256:
                      "bc221fdb22e8b69ad03f00bedc4181e753a2507c882a182004de8634058a1dd1",
                    memoryContentSha256:
                      "924c7f2fe3c2c13300ab5113d289101abc7df31c1c0f6848ddf9fcb8e07983f4",
                  }
                : archiveSourceReadActive
                  ? {
                      benchmarkObservation:
                        "current-archive-source-read-observation",
                      browserMappingDeltas: 78_613,
                      browserMappingDirtyPages: 80_530,
                      browserSuccessExecutionTransitions: 286_089,
                      latestSchema:
                        "vine.v86-bgi-kernel32-archive-source-read.v1",
                      latestWritebackBatches: 89_491,
                      latestWritebackDirtyPages: 122_969,
                      latestWritebackChangedBytes: 3_623_229,
                      latestHostDeltaBatches: 78_613,
                      latestFinalMemoryGeneration: 169_016,
                      semanticCallDigestSha256:
                        "2308a70a77a72340fed6cc500aa05b3135d116812328e69172c5561c767264f4",
                      memoryContentSha256:
                        "e62c0b7b8dd44505ea2ccd201273d014242e3bbb1a8dc446d4039715318fb8fd",
                    }
                  : dragAcceptFilesActive
                    ? {
                        benchmarkObservation:
                          "current-shell32-drag-accept-files-observation",
                        browserMappingDeltas: 78_501,
                        browserMappingDirtyPages: 80_168,
                        browserSuccessExecutionTransitions: 285_545,
                        latestSchema:
                          "vine.v86-bgi-shell32-drag-accept-files.v1",
                        latestWritebackBatches: 89_339,
                        latestWritebackDirtyPages: 122_724,
                        latestWritebackChangedBytes: 3_608_013,
                        latestHostDeltaBatches: 78_501,
                        latestFinalMemoryGeneration: 168_414,
                        semanticCallDigestSha256:
                          "61a51e57987cc3644926f1e61615ad630c48477334dd5b9c1e9a32cf985e6977",
                        memoryContentSha256:
                          "2f041d4df7be96665dfa3bcdce1c6c21aea92e1e486ef025ab834b7e692da31e",
                      }
                    : leaveCriticalSectionContinuationActive
                      ? {
                          benchmarkObservation:
                            "current-critical-section-continuation-release-observation",
                          browserMappingDeltas: 17_384,
                          browserMappingDirtyPages: 18_891,
                          browserSuccessExecutionTransitions: 58_464,
                          latestSchema:
                            "vine.v86-bgi-kernel32-leave-critical-section-continuation.v1",
                          latestWritebackBatches: 18_793,
                          latestWritebackDirtyPages: 37_790,
                          latestWritebackChangedBytes: 2_933_180,
                          latestHostDeltaBatches: 17_384,
                          latestFinalMemoryGeneration: 36_750,
                          semanticCallDigestSha256:
                            "cedf137a3e4d410bd8a409c6405fd02f53f2f1efb6f4c6511e2ecad67bb45631",
                          memoryContentSha256:
                            "9473c7af4f1c00a6bf8f2f5db611cb7a5253d04387297b9e0244d81f032315bc",
                        }
                      : enterCriticalSectionContinuationActive
                        ? {
                            benchmarkObservation:
                              "current-critical-section-acquisition-observation",
                            browserMappingDeltas: 17_383,
                            browserMappingDirtyPages: 18_890,
                            browserSuccessExecutionTransitions: 58_461,
                            latestSchema:
                              "vine.v86-bgi-kernel32-enter-critical-section-continuation.v1",
                            latestWritebackBatches: 18_792,
                            latestWritebackDirtyPages: 37_788,
                            latestWritebackChangedBytes: 2_933_171,
                            latestHostDeltaBatches: 17_383,
                            latestFinalMemoryGeneration: 36_748,
                            semanticCallDigestSha256:
                              "da20a011071131b44421f357e9dd1fa044ff1ee2ec33e509ed28a38137bec04b",
                            memoryContentSha256:
                              "12de5674ab03760d39da41c1eb5aa2cea0c242a98387518aaeb52fc22e4e60df",
                          }
                        : leaveCriticalSectionActive
                          ? {
                              benchmarkObservation:
                                "current-critical-section-release-observation",
                              browserMappingDeltas: 17_382,
                              browserMappingDirtyPages: 18_889,
                              browserSuccessExecutionTransitions: 58_458,
                              latestSchema:
                                "vine.v86-bgi-kernel32-leave-critical-section.v1",
                              latestWritebackBatches: 18_791,
                              latestWritebackDirtyPages: 37_785,
                              latestWritebackChangedBytes: 2_933_159,
                              latestHostDeltaBatches: 17_382,
                              latestFinalMemoryGeneration: 36_746,
                              semanticCallDigestSha256:
                                "77dee747fc8fc285f6c4e32f68e12ba2334118b7b6984ecc68236be8acdac7f4",
                              memoryContentSha256:
                                "b326ab911f02986bbbac818825b0b67171299bc946495eb52f5a0be6fb7e8636",
                            }
                          : primaryMainLoopProgressionActive
                            ? {
                                benchmarkObservation:
                                  "current-main-loop-progression-observation",
                                browserMappingDeltas: 17_381,
                                browserMappingDirtyPages: 18_888,
                                browserSuccessExecutionTransitions: 58_455,
                                latestSchema:
                                  "vine.v86-bgi-primary-main-loop-progression.v1",
                                latestWritebackBatches: 18_790,
                                latestWritebackDirtyPages: 37_783,
                                latestWritebackChangedBytes: 2_933_150,
                                latestHostDeltaBatches: 17_381,
                                latestFinalMemoryGeneration: 36_744,
                                semanticCallDigestSha256:
                                  "0b620924832019b38cc8af0ac9ffa3b1f6300647262bc8a4544ce64408dbf4c8",
                                memoryContentSha256:
                                  "ca7efd6774fe36acfa3ee61621e12b2b6fccbaf4327cc026f323f3aa0859fc2b",
                              }
                            : {
                                benchmarkObservation:
                                  "current-critical-section-idle-cycle-observation",
                                browserMappingDeltas: 17_359,
                                browserMappingDirtyPages: 18_866,
                                browserSuccessExecutionTransitions: 58_352,
                                latestSchema:
                                  "vine.v86-bgi-kernel32-enter-critical-section.v1",
                                latestWritebackBatches: 18_760,
                                latestWritebackDirtyPages: 37_740,
                                latestWritebackChangedBytes: 2_932_769,
                                latestHostDeltaBatches: 17_359,
                                latestFinalMemoryGeneration: 36_692,
                                semanticCallDigestSha256:
                                  "c1d21b090a8bd6469784aed5c4d54f7304b339b4efd4eeecb19e45e6448546da",
                                memoryContentSha256:
                                  "ef30be973462cb545e57a0153362a9f7e40bac0ede6c01cf56424958bcedb2c5",
                              };
  const timerEvidenceEntries =
    dragAcceptFilesActive || archiveSourceReadActive ? 2 : timerCycleCount;
  const timerRepeatedOnes = Array(timerEvidenceEntries).fill(1).join(",");
  const timerRepeatedTwos = Array(timerEvidenceEntries).fill(2).join(",");
  const timerRepeatedZeroes = Array(timerEvidenceEntries).fill(0).join(",");
  const expectedRootKeys = [
    "schema",
    "schemaVersion",
    "scenarioMilestones",
    "historicalBaseline",
    "memoryAuthorityEvidence",
    "browserCheckpoint",
    "engineDecisionEvidence",
    "activeRuntimeCheckpoint",
    "activeBlocker",
    "nextStructuredStop",
  ];
  const rootKeys = Object.keys(root);
  if (
    rootKeys.length !== expectedRootKeys.length ||
    expectedRootKeys.some((key) => !rootKeys.includes(key))
  ) {
    errors.push(
      `manifest root keys must be exactly ${expectedRootKeys.join(", ")}`,
    );
  }
  if (root.schema !== "vine.current-checkpoint") {
    errors.push("manifest.schema must be vine.current-checkpoint");
  }
  if (root.schemaVersion !== 4) {
    errors.push("manifest.schemaVersion must be 4");
  }

  if (root.activeBlocker !== null) {
    const blocker = requireRecord(root.activeBlocker, "activeBlocker", errors);
    for (const field of [
      "schema",
      "id",
      "kind",
      "checkpoint",
      "summary",
      "exactReproduction",
      "diagnosticReproduction",
      "resumeCondition",
    ]) {
      requireString(blocker[field], `activeBlocker.${field}`, errors);
    }
    requireInteger(
      blocker.schemaVersion,
      "activeBlocker.schemaVersion",
      errors,
      1,
    );
    const firstFailure = requireRecord(
      blocker.firstFailure,
      "activeBlocker.firstFailure",
      errors,
    );
    for (const field of ["stage", "code", "relativePath"]) {
      requireString(
        firstFailure[field],
        `activeBlocker.firstFailure.${field}`,
        errors,
      );
    }
    for (const field of ["objectId", "expectedBytes", "observedBytes"]) {
      requireInteger(
        firstFailure[field],
        `activeBlocker.firstFailure.${field}`,
        errors,
        1,
      );
    }
    for (const field of ["expectedSha256", "observedSha256"]) {
      requireSha256(
        firstFailure[field],
        `activeBlocker.firstFailure.${field}`,
        errors,
      );
    }
    const evidence = requireRecord(
      blocker.evidence,
      "activeBlocker.evidence",
      errors,
    );
    for (const field of [
      "sourceLastWriteTime",
      "sourceExecutableSha256",
      "diagnosticSourceSnapshotId",
      "diagnosticTerminalApi",
      "nativeTraceProfile",
      "nativePrimaryTraceSha256",
      "nativeConfirmationTraceSha256",
      "nativeRepetitionTraceSha256",
    ]) {
      requireString(evidence[field], `activeBlocker.evidence.${field}`, errors);
    }
    for (const field of [
      "sourceExecutableBytes",
      "diagnosticImeCallsCompleted",
      "diagnosticFirstImeSequence",
      "diagnosticLastImeSequence",
      "diagnosticFirstImeInstructionCount",
      "nativeConfirmationDiagnostics",
      "nativeRepetitionDiagnostics",
    ]) {
      requireInteger(
        evidence[field],
        `activeBlocker.evidence.${field}`,
        errors,
      );
    }
    const diagnosticCallLimits = requireIntegerArray(
      evidence.diagnosticCallLimits,
      "activeBlocker.evidence.diagnosticCallLimits",
      errors,
    );
    const diagnosticTerminalOccurrences = requireOrderedIntegerArray(
      evidence.diagnosticTerminalOccurrences,
      "activeBlocker.evidence.diagnosticTerminalOccurrences",
      errors,
    );
    const diagnosticTerminalInstructionCounts = requireOrderedIntegerArray(
      evidence.diagnosticTerminalInstructionCounts,
      "activeBlocker.evidence.diagnosticTerminalInstructionCounts",
      errors,
    );
    const attemptedOptions = requireStringArray(
      blocker.attemptedOptions,
      "activeBlocker.attemptedOptions",
      errors,
    );
    const protectedInvariants = requireStringArray(
      blocker.protectedInvariants,
      "activeBlocker.protectedInvariants",
      errors,
    );
    if (
      blocker.schema !== "vine.structured-blocker" ||
      blocker.schemaVersion !== 1 ||
      blocker.id !==
        "imm32-default-ime-context-canonical-source-and-liveness-20260725" ||
      blocker.kind !== "corpus-identity-and-feasibility" ||
      blocker.checkpoint !== "imm32-default-ime-context" ||
      firstFailure.stage !== "source-registration" ||
      firstFailure.code !== "source-object-identity-mismatch" ||
      firstFailure.objectId !== 1 ||
      firstFailure.relativePath !== "BGI.gdb" ||
      firstFailure.expectedBytes !== 125_461 ||
      firstFailure.observedBytes !== 125_461 ||
      firstFailure.expectedSha256 !==
        "e14899243887737b5fd1bddcf35d9fb3e74dc5f7ff0eea7198e1821f9a4d0f92" ||
      firstFailure.observedSha256 !==
        "511684c20839ce847c2593a58a69dc966f819b10985438b2b44a4bc6bc2030ba" ||
      evidence.sourceExecutableBytes !== 1_193_472 ||
      evidence.sourceExecutableSha256 !==
        "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
      evidence.sourceLastWriteOverlapsNativeTrace !== true ||
      evidence.sourceModificationAttributionConfirmed !== false ||
      evidence.diagnosticSourceSnapshotId !==
        "bgi-gdb-511684c20839ce84-diagnostic-v1" ||
      evidence.diagnosticCanonical !== false ||
      evidence.diagnosticImeCallsCompleted !== 3 ||
      evidence.diagnosticFirstImeSequence !== 18_643 ||
      evidence.diagnosticLastImeSequence !== 18_645 ||
      evidence.diagnosticFirstImeInstructionCount !== 12_111_929 ||
      diagnosticCallLimits.join(",") !== "19000,32768" ||
      evidence.diagnosticTerminalApi !== "kernel32.dll!EnterCriticalSection" ||
      diagnosticTerminalOccurrences.join(",") !== "161,7045" ||
      diagnosticTerminalInstructionCounts.join(",") !== "13891597,14380361" ||
      evidence.diagnosticContextDetached !== true ||
      evidence.diagnosticNextStructuredStopReached !== false ||
      evidence.nativeTraceProfile !==
        "x86-bgi-default-ime-window-context-detach-prefix-v1" ||
      evidence.nativePrimaryTraceComplete !== false ||
      evidence.nativeConfirmationDiagnostics !== 0 ||
      evidence.nativeRepetitionDiagnostics !== 0 ||
      evidence.nativeRestoreReached !== false ||
      evidence.nativeReopenStatusWindowReached !== false ||
      evidence.privateTargetAccepted !== false ||
      evidence.chromeAccepted !== false ||
      attemptedOptions.length < 3 ||
      protectedInvariants.length < 3
    ) {
      errors.push("active structured blocker is invalid or incomplete");
    }
    for (const field of [
      "nativePrimaryTraceSha256",
      "nativeConfirmationTraceSha256",
      "nativeRepetitionTraceSha256",
    ]) {
      requireSha256(evidence[field], `activeBlocker.evidence.${field}`, errors);
    }
  }

  const scenarioMilestones = requireRecord(
    root.scenarioMilestones,
    "scenarioMilestones",
    errors,
  );
  if (scenarioMilestones.schema !== "vine.scenario-milestones") {
    errors.push("scenarioMilestones.schema must be vine.scenario-milestones");
  }
  if (scenarioMilestones.schemaVersion !== 1) {
    errors.push("scenarioMilestones.schemaVersion must be 1");
  }
  requireString(
    scenarioMilestones.activeMilestone,
    "scenarioMilestones.activeMilestone",
    errors,
  );
  const expectedMilestoneIds = [
    "boot-and-process-initialization",
    "logical-window-and-message-flow",
    "immutable-source-asset-access",
    "first-present-readable-screen",
    "user-input-consumption",
    "audible-output",
    "save-browser-restart-load",
    "representative-flow",
    "full-play-long-run",
  ];
  const milestones = Array.isArray(scenarioMilestones.milestones)
    ? scenarioMilestones.milestones
    : [];
  if (!Array.isArray(scenarioMilestones.milestones)) {
    errors.push("scenarioMilestones.milestones must be an array");
  }
  const milestoneIds = [];
  for (let index = 0; index < milestones.length; index += 1) {
    const milestone = requireRecord(
      milestones[index],
      `scenarioMilestones.milestones[${index}]`,
      errors,
    );
    requireString(
      milestone.id,
      `scenarioMilestones.milestones[${index}].id`,
      errors,
    );
    requireString(
      milestone.label,
      `scenarioMilestones.milestones[${index}].label`,
      errors,
    );
    requireString(
      milestone.evidenceBoundary,
      `scenarioMilestones.milestones[${index}].evidenceBoundary`,
      errors,
    );
    if (
      ![
        "in-progress",
        "bounded-foundation",
        "fixture-only",
        "not-reached",
      ].includes(milestone.status)
    ) {
      errors.push(`scenarioMilestones.milestones[${index}].status is invalid`);
    }
    milestoneIds.push(milestone.id);
  }
  if (milestoneIds.join(",") !== expectedMilestoneIds.join(",")) {
    errors.push("scenarioMilestones must contain the canonical scenario order");
  }
  const activeScenario = milestones.find(
    ({ id }) => id === scenarioMilestones.activeMilestone,
  );
  if (activeScenario?.status !== "in-progress") {
    errors.push(
      "scenarioMilestones.activeMilestone must identify an in-progress milestone",
    );
  }

  const historical = requireRecord(
    root.historicalBaseline,
    "historicalBaseline",
    errors,
  );
  requireString(historical.checkpoint, "historicalBaseline.checkpoint", errors);
  requireInteger(
    historical.supportedCallCount,
    "historicalBaseline.supportedCallCount",
    errors,
  );
  requireInteger(
    historical.entryInstructionCount,
    "historicalBaseline.entryInstructionCount",
    errors,
  );
  requireString(
    historical.checkpointManifest,
    "historicalBaseline.checkpointManifest",
    errors,
  );

  const memoryAuthority = requireRecord(
    root.memoryAuthorityEvidence,
    "memoryAuthorityEvidence",
    errors,
  );
  const mirrorCapacity = requireRecord(
    memoryAuthority.executionMirrorCapacity,
    "memoryAuthorityEvidence.executionMirrorCapacity",
    errors,
  );
  for (const field of [
    "maximumPhysicalPages",
    "maximumGuestBytes",
    "initialPhysicalPoolBytes",
    "maximumPhysicalPoolBytes",
    "maximumPageTablePages",
    "excludedPhysicalRangeBytes",
    "contractExpansionPages",
    "contractExpandedPoolBytes",
  ]) {
    requireInteger(
      mirrorCapacity[field],
      `memoryAuthorityEvidence.executionMirrorCapacity.${field}`,
      errors,
      1,
    );
  }
  if (
    mirrorCapacity.schema !== "vine.v86-physical-pool.v1" ||
    mirrorCapacity.authority !== "rust-prepared-metadata" ||
    mirrorCapacity.maximumPhysicalPages * 4096 !==
      mirrorCapacity.maximumGuestBytes ||
    mirrorCapacity.initialPhysicalPoolBytes >=
      mirrorCapacity.contractExpandedPoolBytes ||
    mirrorCapacity.contractExpandedPoolBytes >
      mirrorCapacity.maximumPhysicalPoolBytes ||
    mirrorCapacity.maximumPageTablePages !== 1024 ||
    mirrorCapacity.growthPolicy !==
      "geometric-adapter-reinstantiation-before-mutation" ||
    mirrorCapacity.releasePolicy !==
      "zero-free-list-reuse-and-empty-page-table-reclamation" ||
    mirrorCapacity.exhaustionCode !== "v86-physical-pool-exhausted" ||
    mirrorCapacity.exhaustionPolicy !== "typed-preflight-before-mutation"
  ) {
    errors.push("execution mirror capacity contract is inconsistent");
  }

  const browser = requireRecord(
    root.browserCheckpoint,
    "browserCheckpoint",
    errors,
  );
  if (browser.status !== "passed") {
    errors.push("browserCheckpoint.status must be passed");
  }
  if (browser.bgiSession !== "semantic-equivalence-passed") {
    errors.push(
      "browserCheckpoint.bgiSession must identify passed semantic equivalence",
    );
  }
  if (
    !new Set([
      "threshold-passed",
      "threshold-blocked",
      "environment-blocked",
    ]).has(browser.bgiBenchmark)
  ) {
    errors.push(
      "browserCheckpoint.bgiBenchmark must identify a threshold gate outcome",
    );
  }
  if (browser.browserRuntimeProtocol !== "vine.browser-runtime-worker.v34") {
    errors.push(
      "browserCheckpoint.browserRuntimeProtocol must identify protocol v34",
    );
  }
  if (
    browser.executionEngineProfile !==
    "fixtures/execution-engines/v86-browser-v1.json"
  ) {
    errors.push(
      "browserCheckpoint.executionEngineProfile must identify the pinned adapter",
    );
  }
  requireString(browser.workload, "browserCheckpoint.workload", errors);
  const browserEvidence = requireRecord(
    browser.browserRuntimeEvidence,
    "browserCheckpoint.browserRuntimeEvidence",
    errors,
  );
  requireInteger(
    browserEvidence.guestInstructions,
    "browserCheckpoint.browserRuntimeEvidence.guestInstructions",
    errors,
  );
  if (
    browserEvidence.rustSchedulerTransitions !== browserEvidence.hostRequests
  ) {
    errors.push(
      "browser scheduler transitions must authorize every completed host request",
    );
  }
  if (
    JSON.stringify(browserEvidence.registeredThreadIds) !==
      JSON.stringify([8192, 8193]) ||
    JSON.stringify(browserEvidence.executedThreadIds) !==
      JSON.stringify([8192, 8193]) ||
    browserEvidence.contextSwitches < 2 ||
    !Array.isArray(browserEvidence.hostWaitExecutions) ||
    browserEvidence.hostWaitExecutions.length !== 1 ||
    browserEvidence.hostWaitExecutions[0]?.waitingThreadId !== 8192 ||
    browserEvidence.hostWaitExecutions[0]?.executedThreadId !== 8193 ||
    !Array.isArray(browserEvidence.threadExits) ||
    browserEvidence.threadExits.length !== 1 ||
    browserEvidence.threadExits[0]?.threadId !== 8193 ||
    browserEvidence.threadExits[0]?.exitCode !== 7 ||
    browserEvidence.threadExits[0]?.markerValue !== 0x5448_5244
  ) {
    errors.push("browser multi-thread execution evidence is incomplete");
  }
  requireInteger(
    browserEvidence.executionEngineLinearMemoryBytes,
    "browserCheckpoint.browserRuntimeEvidence.executionEngineLinearMemoryBytes",
    errors,
    1,
  );
  validateExecutionStateMachine(
    browserEvidence.executionStateMachine,
    "browserCheckpoint.browserRuntimeEvidence.executionStateMachine",
    errors,
  );
  const mappingOperations = requireRecord(
    browserEvidence.mappingOperations,
    "browserCheckpoint.browserRuntimeEvidence.mappingOperations",
    errors,
  );
  const mappingVariants = [
    "reserve",
    "commit",
    "protect",
    "decommit",
    "release",
  ];
  for (const variant of mappingVariants) {
    requireInteger(
      mappingOperations[variant],
      `browserCheckpoint.browserRuntimeEvidence.mappingOperations.${variant}`,
      errors,
    );
  }
  const mappingCacheEffects = requireRecord(
    browserEvidence.mappingCacheEffects,
    "browserCheckpoint.browserRuntimeEvidence.mappingCacheEffects",
    errors,
  );
  if (
    mappingVariants.some((variant) => mappingOperations[variant] < 1) ||
    browserEvidence.mappingDeltas < 1 ||
    browserEvidence.mappingDirtyPages < 1 ||
    browserEvidence.mappingAtomicRejections < 1 ||
    browserEvidence.mappingStaleAccessStops < 2 ||
    browserEvidence.mirrorRebuilds < 1 ||
    mappingCacheEffects.tlbFlushes < 1 ||
    mappingCacheEffects.instructionFetchInvalidations < 1 ||
    mappingCacheEffects.generatedCodeInvalidations < 1
  ) {
    errors.push("browser mapping-operation evidence is incomplete");
  }
  const browserRuntimeCoreWasm = requireRecord(
    browser.browserRuntimeCoreWasm,
    "browserCheckpoint.browserRuntimeCoreWasm",
    errors,
  );
  requireInteger(
    browserRuntimeCoreWasm.bytes,
    "browserCheckpoint.browserRuntimeCoreWasm.bytes",
    errors,
    1,
  );
  requireSha256(
    browserRuntimeCoreWasm.sha256,
    "browserCheckpoint.browserRuntimeCoreWasm.sha256",
    errors,
  );
  if (browserRuntimeCoreWasm.imports !== 0) {
    errors.push("browser runtime core Wasm must remain import-free");
  }
  const browserRuntimeFixture = requireRecord(
    browser.browserRuntimeFixture,
    "browserCheckpoint.browserRuntimeFixture",
    errors,
  );
  requireSha256(
    browserRuntimeFixture.sha256,
    "browserCheckpoint.browserRuntimeFixture.sha256",
    errors,
  );
  if (
    browserRuntimeFixture.bytes !== 3_584 ||
    browserRuntimeFixture.sha256 !==
      "6f3f5d91ee8fe586b70da650c204cc07b1bf2fdbf210b3fa04bc723d09942b56" ||
    browserRuntimeFixture.nativeExitCode !== 0
  ) {
    errors.push("browser runtime fixture identity is invalid");
  }
  if (
    browserEvidence.guestInstructions !== 1_966 ||
    browserEvidence.cpuSlices !== 255 ||
    browserEvidence.budgetYields !== 233 ||
    browserEvidence.hostRequests !== 2 ||
    browserEvidence.rustSchedulerTransitions !== 2 ||
    browserEvidence.callbackEntries !== 2 ||
    browserEvidence.maximumCallbackDepth !== 2 ||
    browserEvidence.writebackBatches !== 63 ||
    browserEvidence.writebackPages !== 65 ||
    browserEvidence.initialGeneration !== 15 ||
    browserEvidence.finalGeneration !== 123 ||
    browserEvidence.runtimeCoreLinearMemoryBytes !== 1_638_400 ||
    browserEvidence.executionEngineLinearMemoryBytes !== 9_961_472 ||
    browserEvidence.executionStateMachine?.success?.transitionCount !== 769 ||
    browserEvidence.executionStateMachine?.cancellation?.transitionCount !==
      12 ||
    browserEvidence.executionStateMachine?.watchdog?.transitionCount !== 12 ||
    browserEvidence.exitCode !== 0 ||
    browserEvidence.cancelInstructions !== 13 ||
    browserEvidence.cancelPendingHostRequests !== 1 ||
    browserEvidence.cancelPendingHostCallsAfterCleanup !== 0 ||
    browserEvidence.cancelPendingCallbacksAfterCleanup !== 0 ||
    browserEvidence.watchdogInstructions !== 13 ||
    browserEvidence.watchdogPendingHostCallsAfterCleanup !== 0 ||
    browserEvidence.watchdogPendingCallbacksAfterCleanup !== 0
  ) {
    errors.push("controlled browser runtime evidence is invalid");
  }
  const sourceFixture = requireRecord(
    browser.browserSourceReadFixture,
    "browserCheckpoint.browserSourceReadFixture",
    errors,
  );
  const sourceEvidence = requireRecord(
    browser.browserSourceReadEvidence,
    "browserCheckpoint.browserSourceReadEvidence",
    errors,
  );
  for (const [field, value] of [
    ["fixture sha256", sourceFixture.sha256],
    ["source object sha256", sourceFixture.sourceObjectSha256],
    ["report sha256", sourceEvidence.reportSha256],
  ]) {
    requireSha256(value, `browser source ${field}`, errors);
  }
  if (
    sourceFixture.bytes !== 2_560 ||
    sourceFixture.sha256 !==
      "292d298cb77a8223cb0f6fb66333457b3fab9b8e4f0c1643cf01936b769a1d63" ||
    sourceFixture.nativeExitCode !== 0 ||
    sourceFixture.sourceObjectBytes !== 94 ||
    sourceFixture.sourceObjectSha256 !==
      "95c642cb0120d8973d8dc66972cd09c1191f31dc85f30af4f75a80fb5836ed2c" ||
    sourceEvidence.guestInstructions !== 217 ||
    sourceEvidence.cpuSlices !== 30 ||
    sourceEvidence.budgetYields !== 23 ||
    sourceEvidence.hostRequests !== 2 ||
    sourceEvidence.rustSchedulerTransitions !== 2 ||
    sourceEvidence.sourceReadCompletions !== 2 ||
    sourceEvidence.sourceReadOffsets?.join(",") !== "0,50" ||
    sourceEvidence.sourceReadRequestedBytes?.join(",") !== "8,11" ||
    sourceEvidence.sourceReadTransferredBytes?.join(",") !== "8,11" ||
    sourceEvidence.sourceReadPayloadSha256?.join(",") !==
      "202034b246570abc8e0e549637dbeb3ce7990de1fedeef6bcbf795934a3bb2da,71c02fe83d1ee0855a1875c47a53372209e4892b3b8bd7bcdc5d1ea9610d0dc3" ||
    sourceEvidence.titleSnapshotObjectHttpIdentity !== true ||
    sourceEvidence.initialGeneration !== 12 ||
    sourceEvidence.finalGeneration !== 24 ||
    sourceEvidence.wasmBridgeCalls !== 26 ||
    sourceEvidence.runtimeCoreLinearMemoryBytes !== 1_572_864 ||
    sourceEvidence.executionEngineLinearMemoryBytes !== 9_764_864 ||
    sourceEvidence.successExitCode !== 0 ||
    sourceEvidence.successPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.successPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.cancelInstructions !== 32 ||
    sourceEvidence.cancelSourceReadCompletions !== 0 ||
    sourceEvidence.cancelPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.cancelPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.watchdogInstructions !== 32 ||
    sourceEvidence.watchdogRequestId !== "1" ||
    sourceEvidence.watchdogLastTransition !== "waiting-host" ||
    sourceEvidence.watchdogSourceReadCompletions !== 0 ||
    sourceEvidence.watchdogPendingHostCallsAfterCleanup !== 0 ||
    sourceEvidence.watchdogPendingCallbacksAfterCleanup !== 0 ||
    sourceEvidence.reportBytes !== 23_505 ||
    sourceEvidence.reportSha256 !==
      "a8a5aba21b10092fa616ac7c128c5985e464976200c1dc8d1bf4735aa6bda4ab"
  ) {
    errors.push("browser immutable source-read evidence is invalid");
  }
  const bgiBrowserEvidence = requireRecord(
    browser.bgiRuntimeEvidence,
    "browserCheckpoint.bgiRuntimeEvidence",
    errors,
  );
  for (const field of [
    "guestInstructions",
    "cpuSlices",
    "budgetYields",
    "supportedCalls",
    "writebackBatches",
    "writebackPages",
    "hostDeltaBatches",
    "mappingDeltas",
    "mappingDirtyPages",
    "mappingAtomicRejections",
    "mappingStaleAccessStops",
    "mirrorRebuilds",
    "initialGeneration",
    "finalGeneration",
    "nodeFinalGeneration",
    "wasmBridgeCalls",
    "runtimeCoreLinearMemoryBytes",
    "executionEngineLinearMemoryBytes",
    "heartbeatSamples",
    "cancelInstructions",
    "watchdogInstructions",
    "pendingHostCallsAfterCleanup",
    "pendingCallbacksAfterCleanup",
    "nodeOracleReportBytes",
    "browserReportBytes",
    "largeHeapPageMaterializations",
    "largeHeapAllocationBytes",
    "largeHeapAllocationId",
    "largeHeapAddressAllocationId",
    "largeHeapMemoryOwner",
    "largeHeapFirstFaultInstructionCount",
    "largeHeapFirstPageFaultErrorCode",
    "largeHeapFirstCommittedPageBytes",
    "largeHeapNodeMaterializationFromGeneration",
    "largeHeapNodeMaterializationGeneration",
    "largeHeapBrowserMaterializationFromGeneration",
    "largeHeapBrowserMaterializationGeneration",
    "sourceReadCompletions",
    "sourceReadPendingAfterCleanup",
    "callbackReplacements",
    "logicalWindowCompletions",
    "d3d9GetAdapterCountCompletions",
    "d3d9GetAdapterCountSequence",
    "d3d9GetAdapterCountResult",
    "d3d9GetAdapterCountGuestWriteBytes",
    "d3d9GetAdapterCountReferenceCount",
    "d3d9GetAdapterMonitorCompletions",
    "d3d9GetAdapterMonitorGuestWriteBytes",
    "d3d9GetAdapterMonitorReferenceCount",
    "d3d9GetAdapterDisplayModeCompletions",
    "d3d9GetAdapterDisplayModeSequence",
    "d3d9GetAdapterDisplayModeReferenceCount",
    "d3d9GetAdapterIdentifierCompletions",
    "d3d9GetAdapterIdentifierSequence",
    "d3d9GetAdapterIdentifierOutputBytes",
    "d3d9GetAdapterIdentifierReferenceCount",
    "d3d9GetDeviceCapsCompletions",
    "d3d9GetDeviceCapsSequence",
    "d3d9GetDeviceCapsOutputBytes",
    "d3d9GetDeviceCapsReferenceCount",
    "d3d9CheckDeviceTypeCompletions",
    "d3d9CheckDeviceTypeReferenceCount",
    "d3d9CheckDeviceTypeGuestWriteBytes",
    "d3d9CheckDeviceFormatCompletions",
    "d3d9CheckDeviceFormatSequence",
    "d3d9CheckDeviceFormatReferenceCount",
    "d3d9CheckDeviceFormatGuestWriteBytes",
    "d3d9CreateDeviceCompletions",
    "d3d9CreateDeviceSequence",
    "d3d9CreateDeviceLastError",
    "d3d9CreateDeviceGuestWriteBytes",
    "d3d9CreateDeviceObjectId",
    "d3d9CreateDeviceInterfaceId",
    "d3d9CreateDeviceRootReferenceCount",
    "d3d9CreateDeviceReferenceCount",
    "d3d9CreateDeviceLogicalGraphicsDeviceCount",
    "d3d9CreateTextureCompletions",
    "d3d9CreateTextureSequence",
    "d3d9CreateTextureLastError",
    "d3d9CreateTextureGuestWriteBytes",
    "d3d9CreateTextureObjectId",
    "d3d9CreateTextureInterfaceId",
    "d3d9CreateTextureDeviceReferenceCount",
    "d3d9CreateTextureReferenceCount",
    "d3d9CreateTextureLogicalGraphicsDeviceId",
    "d3d9CreateTextureLogicalGraphicsTextureId",
    "d3d9CreateTextureLogicalGraphicsDeviceCount",
    "d3d9CreateTextureLogicalGraphicsTextureCount",
    "d3d9GetSurfaceLevelCompletions",
    "d3d9GetSurfaceLevelSequence",
    "d3d9GetSurfaceLevelLastError",
    "d3d9GetSurfaceLevelGuestWriteBytes",
    "d3d9GetSurfaceLevelObjectId",
    "d3d9GetSurfaceLevelInterfaceId",
    "d3d9GetSurfaceLevelTextureReferenceCount",
    "d3d9GetSurfaceLevelReferenceCount",
    "d3d9GetSurfaceLevelLogicalGraphicsTextureId",
    "d3d9GetSurfaceLevelLogicalGraphicsSurfaceId",
    "d3d9GetSurfaceLevelLogicalGraphicsSurfaceCount",
    "d3d9SurfaceGetDcCompletions",
    "d3d9SurfaceGetDcSequence",
    "d3d9SurfaceGetDcLastError",
    "d3d9SurfaceGetDcGuestWriteBytes",
    "d3d9SurfaceGetDcDeviceContextId",
    "d3d9SurfaceGetDcLogicalGraphicsSurfaceId",
    "d3d9SurfaceGetDcSurfaceComObjectId",
    "d3d9SurfaceGetDcReferenceCount",
    "d3d9SurfaceGetDcLiveDeviceContexts",
    "d3d9SurfaceGetStockObjectSequence",
    "d3d9SurfaceFillRectSequence",
    "d3d9SurfaceFillRectLastError",
    "d3d9SurfaceFillRectGuestWriteBytes",
    "d3d9SurfaceFillRectLogicalGraphicsSurfaceId",
    "d3d9SurfaceFillRectColorRef",
    "d3d9SurfaceFillRectContentGeneration",
    "d3d9SurfaceFillRectSurfaceGeneration",
    "d3d9SurfaceReleaseDcCompletions",
    "d3d9SurfaceReleaseDcSequence",
    "d3d9SurfaceReleaseDcLastError",
    "d3d9SurfaceReleaseDcGuestWriteBytes",
    "d3d9SurfaceReleaseDcLiveDeviceContexts",
    "d3d9SurfaceReleaseSequence",
    "d3d9SurfaceReleaseResult",
    "d3d9SurfaceReleaseReferenceCount",
    "d3d9SurfaceReleaseLogicalGraphicsSurfaceCount",
    "d3d9SecondCreateTextureSequence",
    "d3d9SecondCreateTexturePool",
    "d3d9SecondCreateTextureLastError",
    "d3d9SecondCreateTextureGuestWriteBytes",
    "d3d9SecondCreateTextureDeviceReferenceCount",
    "d3d9SecondCreateTextureReferenceCount",
    "d3d9ThirdCreateTextureSequence",
    "d3d9ThirdCreateTexturePool",
    "d3d9ThirdCreateTextureUsage",
    "d3d9ThirdCreateTextureLastError",
    "d3d9ThirdCreateTextureGuestWriteBytes",
    "d3d9ThirdCreateTextureDeviceReferenceCount",
    "d3d9ThirdCreateTextureReferenceCount",
    "d3d9CreateVertexBufferCompletions",
    "d3d9CreateVertexBufferSequence",
    "d3d9CreateVertexBufferInstructionCount",
    "d3d9CreateVertexBufferLastError",
    "d3d9CreateVertexBufferGuestWriteBytes",
    "d3d9CreateVertexBufferDeviceReferenceCountAtCompletion",
    "d3d9CreateVertexBufferReferenceCount",
    "d3d9CreateVertexBufferLogicalGraphicsDeviceId",
    "d3d9CreateVertexBufferLogicalGraphicsVertexBufferId",
    "d3d9CreateVertexBufferLogicalGraphicsVertexBufferCount",
    "d3d9CreateVertexBufferBackingBytes",
    "d3d9VertexBufferLockCompletions",
    "d3d9VertexBufferLockSequence",
    "d3d9VertexBufferLockInstructionCount",
    "d3d9VertexBufferLockLastError",
    "d3d9VertexBufferLockGuestWriteBytes",
    "d3d9VertexBufferLockReferenceCount",
    "d3d9VertexBufferUnlockCompletions",
    "d3d9VertexBufferUnlockSequence",
    "d3d9VertexBufferUnlockInstructionCount",
    "d3d9VertexBufferUnlockLastError",
    "d3d9VertexBufferUnlockGuestWriteBytes",
    "d3d9VertexBufferUnlockReferenceCount",
    "d3d9VertexBufferUnlockContentGeneration",
    "d3d9SetRenderStateCompletions",
    "d3d9SetRenderStateDeviceReferenceCount",
    "d3d9SetRenderStateGenerationDelta",
    "d3d9GetRasterStatusCompletions",
    "d3d9GetRasterStatusSequence",
    "d3d9GetRasterStatusInstructionCount",
    "d3d9GetRasterStatusLastError",
    "d3d9GetRasterStatusGuestWriteBytes",
    "d3d9GetRasterStatusDeviceReferenceCount",
    "d3d9GetRasterStatusScanLine",
    "d3d9ClearCompletions",
    "d3d9ClearSequence",
    "d3d9ClearInstructionCount",
    "d3d9ClearGuestWriteBytes",
    "d3d9ClearDeviceReferenceCount",
    "d3d9ClearLogicalGraphicsDeviceId",
    "d3d9ClearBackBufferContentGeneration",
    "d3d9ClearBackBufferSequence",
    "d3d9ClearLogicalGraphicsDeviceGeneration",
    "getMonitorInfoACompletions",
    "getMonitorInfoASequence",
    "getMonitorInfoAInstructionCount",
    "getMonitorInfoALastError",
    "getMonitorInfoAGuestWriteBytes",
    "directInput8CreateCompletions",
    "directInput8CreateSequence",
    "directInput8CreateInstructionCount",
    "directInput8CreateLastError",
    "directInput8CreateGuestWriteBytes",
    "directInput8CreateObjectId",
    "directInput8CreateInterfaceId",
    "directInput8CreateReferenceCount",
    "directInput8CreateBackingBytes",
    "directInput8EnumDevicesCompletions",
    "directInput8EnumDevicesSequence",
    "directInput8EnumDevicesInstructionCount",
    "directInput8EnumDevicesLastError",
    "directInput8EnumDevicesGuestWriteBytes",
    "directInput8EnumDevicesDeviceClass",
    "directInput8EnumDevicesCallbackInvocations",
    "directInput8EnumDevicesDeviceCount",
    "directInput8EnumDevicesReferenceCount",
    "directSoundCreate8Completions",
    "directSoundCreate8Sequence",
    "directSoundCreate8InstructionCount",
    "directSoundCreate8LastError",
    "directSoundCreate8GuestWriteBytes",
    "directSoundCreate8ObjectId",
    "directSoundCreate8InterfaceId",
    "directSoundCreate8ReferenceCount",
    "directSoundCreate8BackingBytes",
    "directSoundSetCooperativeLevelCompletions",
    "directSoundSetCooperativeLevelSequence",
    "directSoundSetCooperativeLevelInstructionCount",
    "directSoundSetCooperativeLevelGuestWriteBytes",
    "directSoundSetCooperativeLevelReferenceCount",
    "directSoundPrimaryBufferCompletions",
    "directSoundPrimaryBufferSequence",
    "directSoundPrimaryBufferInstructionCount",
    "directSoundPrimaryBufferObjectId",
    "directSoundPrimaryBufferInterfaceId",
    "directSoundPrimaryBufferLogicalAudioBufferId",
    "directSoundPrimarySetFormatCompletions",
    "directSoundPrimarySetFormatSequence",
    "directSoundPrimarySetFormatInstructionCount",
    "directSoundPrimaryPlayCompletions",
    "directSoundPrimaryPlaySequence",
    "directSoundPrimaryPlayInstructionCount",
    "directSoundSecondaryBufferCompletions",
    "directSoundSecondaryBufferSequence",
    "directSoundSecondaryBufferInstructionCount",
    "directSoundSecondaryBufferObjectId",
    "directSoundSecondaryBufferInterfaceId",
    "directSoundSecondaryBufferLogicalAudioBufferId",
    "directSoundBuffer8QueryInterfaceCompletions",
    "directSoundBuffer8QueryInterfaceSequence",
    "directSoundBuffer8QueryInterfaceInstructionCount",
    "directSoundBuffer8QueryInterfaceReferenceCount",
    "directSoundBuffer8ReleaseCompletions",
    "directSoundBuffer8ReleaseSequence",
    "directSoundBuffer8ReleaseInstructionCount",
    "directSoundBuffer8ReleaseResult",
    "directSoundBuffer8ReleaseReferenceCount",
    "directSoundBufferLockCompletions",
    "directSoundBufferLockSequence",
    "directSoundBufferLockInstructionCount",
    "directSoundBufferLockGuestWriteBytes",
    "directSoundBufferLockFirstSegmentBytes",
    "directSoundBufferLockSecondSegmentBytes",
    "directSoundBufferLockReferenceCount",
    "directSoundBufferUnlockCompletions",
    "directSoundBufferUnlockSequence",
    "directSoundBufferUnlockInstructionCount",
    "directSoundBufferUnlockGuestWriteBytes",
    "directSoundBufferUnlockContentGeneration",
    "directSoundBufferUnlockZeroBytes",
    "directSoundBufferUnlockReferenceCount",
    "directSoundSecondaryPlayCompletions",
    "directSoundSecondaryPlaySequence",
    "directSoundSecondaryPlayInstructionCount",
    "directSoundSecondaryPlayReferenceCount",
    "loadLibraryACompletions",
    "loadLibraryASequence",
    "loadLibraryALastError",
    "loadLibraryAGuestWriteBytes",
    "loadLibraryAModuleId",
    "loadLibraryAAttachSequence",
    "loadLibraryAObjectId",
    "loadLibraryAThreadReferenceCount",
    "d3dxGetProcAddressCompletions",
    "d3dxGetProcAddressSequence",
    "d3dxGetProcAddressLastError",
    "getClientRectCompletions",
    "getClientRectSequence",
    "getClientRectLastError",
    "getClientRectGuestWriteBytes",
    "getDcCompletions",
    "getDcSequence",
    "getDcLastError",
    "getDcGuestWriteBytes",
    "getDcDeviceContextId",
    "getDcSurfaceId",
    "getDcLiveDeviceContexts",
    "getStockObjectCompletions",
    "getStockObjectSequence",
    "getStockObjectLastError",
    "getStockObjectGuestWriteBytes",
    "getStockObjectId",
    "getStockObjectColorRef",
    "getStockObjectLiveObjects",
    "fillRectCompletions",
    "fillRectSequence",
    "fillRectLastError",
    "fillRectGuestWriteBytes",
    "fillRectSurfaceId",
    "fillRectSurfaceWidth",
    "fillRectSurfaceHeight",
    "fillRectSurfaceGeneration",
    "fillRectColorRef",
    "releaseDcCompletions",
    "releaseDcSequence",
    "releaseDcLastError",
    "releaseDcGuestWriteBytes",
    "releaseDcLiveDeviceContexts",
    "releaseDcLiveSurfaces",
    "setWindowLongACompletions",
    "setWindowLongASequence",
    "setWindowLongALastError",
    "setWindowLongAGuestWriteBytes",
    "setWindowPosActivationCompletions",
    "setWindowPosActivationSequence",
    "setWindowPosActivationLastError",
    "setWindowPosActivationGuestWriteBytes",
    "timeGetTimeCompletions",
    "timeGetTimeIncrementPerRead",
    "threadPriorityCompletions",
    "threadPrioritySequence",
    "threadPriorityInstructionCount",
    "threadPriorityResult",
    "threadPriorityLastError",
    "threadPriorityGuestWriteBytes",
    "threadPriorityThreadId",
    "threadPriorityNodeCompletions",
    "threadPriorityChromeCompletions",
    "screenDcCompletions",
    "screenDcSequence",
    "screenDcInstructionCount",
    "screenDcLastError",
    "screenDcGuestWriteBytes",
    "screenDcDeviceContextId",
    "screenDcOwnerThreadId",
    "screenDcLiveDeviceContexts",
    "screenDcNodeCompletions",
    "screenDcChromeCompletions",
    "fontEnumerationCompletions",
    "fontEnumerationSequence",
    "fontEnumerationInstructionCount",
    "fontEnumerationLastError",
    "fontEnumerationGuestWriteBytes",
    "fontEnumerationCallbackFrameId",
    "fontEnumerationCallbackResult",
    "fontEnumerationCallbackReturnInstructionCount",
    "fontEnumerationCallbackScratchBytes",
    "fontEnumerationCharacterSet",
    "fontEnumerationFontType",
    "fontEnumerationEnumLogFontExABytes",
    "fontEnumerationNewTextMetricExABytes",
    "fontEnumerationNodeCompletions",
    "fontEnumerationChromeCompletions",
    "screenReleaseDcSequence",
    "screenReleaseDcInstructionCount",
    "screenReleaseDcResult",
    "screenReleaseDcLastError",
    "screenReleaseDcLiveDeviceContexts",
    "fontCreationCompletions",
    "fontCreationSequence",
    "fontCreationInstructionCount",
    "fontCreationLastError",
    "fontCreationGuestWriteBytes",
    "fontCreationObjectId",
    "fontCreationLiveObjects",
    "fontCreationNodeCompletions",
    "fontCreationChromeCompletions",
    "compatibleDcCompletions",
    "compatibleDcSequence",
    "compatibleDcInstructionCount",
    "compatibleDcLastError",
    "compatibleDcGuestWriteBytes",
    "compatibleDcDeviceContextId",
    "compatibleDcOwnerThreadId",
    "compatibleDcSelectedFontId",
    "compatibleDcSelectedBitmapId",
    "compatibleDcSelectionGeneration",
    "compatibleDcInitialFontStockIndex",
    "compatibleDcInitialBitmapStockIndex",
    "compatibleDcLiveDeviceContexts",
    "compatibleDcLiveObjects",
    "compatibleDcNodeCompletions",
    "compatibleDcChromeCompletions",
    "dibCreateCompletions",
    "dibCreateSequence",
    "dibCreateInstructionCount",
    "dibCreateLastError",
    "dibCreateGuestWriteBytes",
    "dibObjectId",
    "dibPaletteEntries",
    "dibBackingBytes",
    "dibBackingAddressAllocationId",
    "dibBackingMemoryOwner",
    "dibCreateDeltaFromGeneration",
    "dibCreateDeltaGeneration",
    "dibCreateDeltaMappingOperations",
    "dibCreateDeltaDirtyPages",
    "dibCreateDeltaPayloadBytes",
    "dibGetObjectCompletions",
    "dibGetObjectSequence",
    "dibGetObjectInstructionCount",
    "dibGetObjectResult",
    "dibGetObjectLastError",
    "dibGetObjectGuestWriteBytes",
    "dibSelectCompletions",
    "dibSelectGuestWriteBytes",
    "dibNodeCompletions",
    "dibChromeCompletions",
    "outlineTextMetricsCompletions",
    "outlineTextMetricsOutputBytes",
    "outlineTextMetricsDeviceContextId",
    "outlineTextMetricsSelectedFontId",
    "outlineTextMetricsSelectionGeneration",
    "outlineTextMetricsNodeCompletions",
    "outlineTextMetricsChromeCompletions",
    "textRasterCompletions",
    "textRasterSequence",
    "textRasterInstructionCount",
    "textRasterResult",
    "textRasterLastError",
    "textRasterGuestWriteBytes",
    "textRasterChangedBytes",
    "textRasterDeviceContextId",
    "textRasterSelectedFontId",
    "textRasterSelectedDibId",
    "textRasterSelectionGeneration",
    "textRasterContentGeneration",
    "textRasterNodeCompletions",
    "textRasterChromeCompletions",
    "gdiplusStartupCompletions",
    "gdiplusStartupSequence",
    "gdiplusStartupInstructionCount",
    "gdiplusStartupResult",
    "gdiplusStartupLastError",
    "gdiplusStartupGuestWriteBytes",
    "gdiplusStartupSessionId",
    "gdiplusStartupTokenId",
    "gdiplusStartupSessionGeneration",
    "gdiplusStartupLiveSessions",
    "gdiplusStartupNodeCompletions",
    "gdiplusStartupChromeCompletions",
    "virtualAllocCompletions",
    "virtualAllocSequence",
    "virtualAllocInstructionCount",
    "virtualAllocLastError",
    "virtualAllocGuestWriteBytes",
    "virtualAllocAddressAllocationId",
    "virtualAllocMemoryOwner",
    "virtualAllocReservedBytes",
    "virtualAllocNodeCompletions",
    "virtualAllocChromeCompletions",
    "dibDeleteObjectSequence",
    "dibDeleteObjectInstructionCount",
    "dibDeleteObjectResult",
    "fontDeleteObjectSequence",
    "fontDeleteObjectInstructionCount",
    "fontDeleteObjectResult",
    "deleteDcSequence",
    "deleteDcInstructionCount",
    "deleteDcResult",
    "setWaitableTimerCompletions",
    "setWaitableTimerSequence",
    "setWaitableTimerInstructionCount",
    "setWaitableTimerResult",
    "setWaitableTimerLastError",
    "setWaitableTimerGuestWriteBytes",
    "setWaitableTimerTimerId",
    "setWaitableTimerRelativeDueTime100ns",
    "setWaitableTimerPeriodMilliseconds",
    "setWaitableTimerArmGeneration",
    "setWaitableTimerObjectId",
    "setWaitableTimerOpenHandles",
    "waitForSingleObjectExCompletions",
    "waitForSingleObjectExGuestWriteBytes",
    "waitForSingleObjectExPacingPayloadVersion",
    "waitForSingleObjectExBrowserPacingRequests",
    "waitForSingleObjectExBrowserPacingCompletions",
    "waitForSingleObjectExBrowserPacingCancellations",
    "waitForSingleObjectExBrowserPacingStaleCallbacks",
    "waitForSingleObjectExBrowserPacingPending",
    "dragAcceptFilesCompletions",
    "dragAcceptFilesSequence",
    "dragAcceptFilesInstructionCount",
    "dragAcceptFilesLastError",
    "dragAcceptFilesGuestWriteBytes",
    "dragAcceptFilesWindowId",
    "dragAcceptFilesNodeCompletions",
    "dragAcceptFilesChromeCompletions",
    "successExecutionTransitionCount",
  ]) {
    requireInteger(
      bgiBrowserEvidence[field],
      `browserCheckpoint.bgiRuntimeEvidence.${field}`,
      errors,
    );
  }
  const bgiMappingOperations = requireRecord(
    bgiBrowserEvidence.mappingOperations,
    "browserCheckpoint.bgiRuntimeEvidence.mappingOperations",
    errors,
  );
  for (const variant of mappingVariants) {
    requireInteger(
      bgiMappingOperations[variant],
      `browserCheckpoint.bgiRuntimeEvidence.mappingOperations.${variant}`,
      errors,
    );
  }
  validateExecutionStateMachine(
    bgiBrowserEvidence.executionStateMachine,
    "browserCheckpoint.bgiRuntimeEvidence.executionStateMachine",
    errors,
  );
  for (const field of [
    "sourceSha256",
    "memoryContentSha256",
    "nodeOracleReportSha256",
    "browserReportSha256",
    "callSemanticsSha256",
    "largeHeapMaterializationPayloadSha256",
    "sourceReadPayloadSha256",
    "d3d9GetAdapterIdentifierOutputSha256",
    "d3d9GetDeviceCapsOutputSha256",
    "d3d9CreateVertexBufferInitialContentSha256",
    "d3d9VertexBufferUnlockContentSha256",
    "dibCreateDeltaPayloadSha256",
    "outlineTextMetricsOutputSha256",
  ]) {
    requireSha256(
      bgiBrowserEvidence[field],
      `browserCheckpoint.bgiRuntimeEvidence.${field}`,
      errors,
    );
  }
  const bgiD3d9CheckDeviceTypeSequences = requireIntegerArray(
    bgiBrowserEvidence.d3d9CheckDeviceTypeSequences,
    "browserCheckpoint.bgiRuntimeEvidence.d3d9CheckDeviceTypeSequences",
    errors,
  );
  const bgiD3d9AdapterMonitorSequences = requireIntegerArray(
    bgiBrowserEvidence.d3d9GetAdapterMonitorSequences,
    "browserCheckpoint.bgiRuntimeEvidence.d3d9GetAdapterMonitorSequences",
    errors,
  );
  if (
    bgiBrowserEvidence.nodeRole !== "semantic-oracle-only" ||
    bgiBrowserEvidence.runtimeStateAuthority !==
      "rust-wasm-in-runtime-worker" ||
    bgiBrowserEvidence.evidenceLevel !==
      (sleepZeroActive
        ? "summary"
        : dragAcceptFilesActive || archiveSourceReadActive
          ? "compact"
          : "summary") ||
    bgiBrowserEvidence.callSemanticsAlgorithm !== "sha256-canonical-json-v1" ||
    bgiBrowserEvidence.nodeOracleReportBytes > 2 * 1024 * 1024 ||
    bgiBrowserEvidence.browserReportBytes > 2 * 1024 * 1024 ||
    bgiMappingOperations.reserve !==
      (setWindowPosFrameChangeMoveActive ? 38 : sleepZeroActive ? 29 : 21) ||
    bgiMappingOperations.commit !==
      (setWindowPosFrameChangeMoveActive
        ? 142
        : sleepZeroActive
          ? 94
          : archiveSourceReadActive
            ? 78
            : dragAcceptFilesActive
              ? 72
              : 71) ||
    bgiMappingOperations.protect !==
      (setWindowPosFrameChangeMoveActive ? 62 : sleepZeroActive ? 45 : 28) ||
    bgiMappingOperations.decommit !== 0 ||
    bgiMappingOperations.release !==
      (setWindowPosFrameChangeMoveActive ? 21 : sleepZeroActive ? 12 : 4) ||
    bgiBrowserEvidence.mappingDeltas !==
      profileExpectations.browserMappingDeltas ||
    bgiBrowserEvidence.mappingDirtyPages !==
      profileExpectations.browserMappingDirtyPages ||
    bgiBrowserEvidence.mappingAtomicRejections !== 0 ||
    bgiBrowserEvidence.mappingStaleAccessStops !== 0 ||
    bgiBrowserEvidence.mirrorRebuilds !== 0 ||
    bgiBrowserEvidence.successExecutionTransitionCount !==
      profileExpectations.browserSuccessExecutionTransitions ||
    JSON.stringify(bgiBrowserEvidence.registeredThreadIds) !==
      JSON.stringify([8_192, 8_193]) ||
    JSON.stringify(bgiBrowserEvidence.executedThreadIds) !==
      JSON.stringify([8_192, 8_193]) ||
    bgiBrowserEvidence.contextSwitches !==
      (d3d9RecreatedTexturesActive
        ? 2_010
        : createDirectoryAActive
          ? 1_526
          : getFileAttributesAActive
            ? 1_518
            : sleepZeroActive
              ? 1_512
              : archiveSourceReadActive
                ? 1_032
                : dragAcceptFilesActive
                  ? 1_030
                  : mainLoopContinuationActive
                    ? 2
                    : 1) ||
    bgiBrowserEvidence.pendingHostCallsAfterCleanup !== 0 ||
    bgiBrowserEvidence.pendingCallbacksAfterCleanup !== 0 ||
    bgiBrowserEvidence.processLifecycleAfterCleanup !== "exited" ||
    bgiBrowserEvidence.dllLifecycleAfterCleanup !== "process-detached" ||
    bgiBrowserEvidence.largeHeapPageMaterializations !== 31 ||
    bgiBrowserEvidence.largeHeapAllocationAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapAllocationBytes !== 64 * 1024 * 1024 ||
    bgiBrowserEvidence.largeHeapAllocationId !== 12 ||
    bgiBrowserEvidence.largeHeapAddressAllocationId !== 37 ||
    bgiBrowserEvidence.largeHeapMemoryOwner !== 12 ||
    bgiBrowserEvidence.largeHeapFirstFaultInstructionCount !== 50_123 ||
    bgiBrowserEvidence.largeHeapFirstFaultAddress !== "0x40000000" ||
    bgiBrowserEvidence.largeHeapFirstPageFaultErrorCode !== 6 ||
    bgiBrowserEvidence.largeHeapFirstCommittedPageBytes !== 4_096 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationFromGeneration !== 1_473 ||
    bgiBrowserEvidence.largeHeapNodeMaterializationGeneration !== 1_475 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationFromGeneration !==
      1_487 ||
    bgiBrowserEvidence.largeHeapBrowserMaterializationGeneration !== 1_489 ||
    bgiBrowserEvidence.sourceReadCompletions !==
      (createDirectoryAActive ? 10 : archiveSourceReadActive ? 5 : 2) ||
    bgiBrowserEvidence.sourceReadPendingAfterCleanup !== 0 ||
    bgiBrowserEvidence.callbackEntries !==
      (updateWindowActive
        ? 30
        : setWindowPosFrameChangeMoveActive
        ? 29
        : setWindowPosRepeatActivationActive
          ? 25
          : 24) ||
    bgiBrowserEvidence.callbackReplacements !==
      (setWindowPosFrameChangeMoveActive ? 15 : 13) ||
    bgiBrowserEvidence.maximumCallbackDepth !== 3 ||
    bgiBrowserEvidence.logicalWindowCompletions !==
      (setWindowPosFrameChangeMoveActive
        ? 8
        : setWindowPosRepeatActivationActive
          ? 7
          : 6) ||
    bgiBrowserEvidence.logicalWindowHandle !== "0x48010001" ||
    bgiBrowserEvidence.logicalWindowFinalLifecycle !== "created" ||
    bgiBrowserEvidence.logicalWindowRequestedStyle !== "0x80ca0000" ||
    bgiBrowserEvidence.logicalWindowEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.logicalWindowVisible !== true ||
    bgiBrowserEvidence.logicalWindowTopmost !== false ||
    bgiBrowserEvidence.browserOwnsLogicalWindowState !== false ||
    bgiBrowserEvidence.setWindowPosResizeCompletions !== 1 ||
    bgiBrowserEvidence.setWindowPosResizeSequence !== 89_387 ||
    bgiBrowserEvidence.setWindowPosResizeCallbackFrameIds?.join(",") !==
      "21,22,23,24" ||
    bgiBrowserEvidence.setWindowPosResizeCallbackMessages?.join(",") !==
      "70,131,5,71" ||
    bgiBrowserEvidence.setWindowPosResizeCallbackDepthAfterCompletion !== 0 ||
    bgiBrowserEvidence.setWindowPosResizeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.systemMenuCompletions !== 2 ||
    bgiBrowserEvidence.systemMenuSequences?.join(",") !== "89390,89391" ||
    bgiBrowserEvidence.systemMenuInstructionCounts?.join(",") !==
      "15952000,15952003" ||
    bgiBrowserEvidence.getSystemMenuArguments?.join(",") !==
      "0x48010001,0x00000000" ||
    bgiBrowserEvidence.getSystemMenuResult !== "0x4e010001" ||
    bgiBrowserEvidence.getSystemMenuLastError !== 0 ||
    bgiBrowserEvidence.getSystemMenuImplementationId !==
      "user32.get-system-menu.logical-system-menu.v2" ||
    bgiBrowserEvidence.enableMenuItemArguments?.join(",") !==
      "0x4e010001,0x0000f060,0x00000000" ||
    bgiBrowserEvidence.enableMenuItemResult !== 0 ||
    bgiBrowserEvidence.enableMenuItemLastError !== 0 ||
    bgiBrowserEvidence.enableMenuItemImplementationId !==
      "user32.enable-menu-item.logical-system-menu.v2" ||
    bgiBrowserEvidence.systemMenuStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    bgiBrowserEvidence.systemMenuHandleNamespace !== "vine.logical-menu.v1" ||
    bgiBrowserEvidence.systemMenuOwnerWindowHandle !== "0x48010001" ||
    bgiBrowserEvidence.systemMenuCloseItemEnabled !== true ||
    bgiBrowserEvidence.systemMenuPreviousCloseItemState !== 0 ||
    bgiBrowserEvidence.systemMenuMutation !==
      "separate-menu-generation-plan-stale-check-targeted-commit" ||
    bgiBrowserEvidence.systemMenuWindowMutationGenerationIndependent !== true ||
    bgiBrowserEvidence.systemMenuFailureAtomicity !== true ||
    bgiBrowserEvidence.systemMenuBrowserOwnsWindowsState !== false ||
    bgiBrowserEvidence.systemMenuBrowserResourceCreated !== false ||
    bgiBrowserEvidence.systemMenuNodeChromeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.monitorFromWindowCompletions !== 2 ||
    bgiBrowserEvidence.monitorFromWindowSequence !== 17_842 ||
    bgiBrowserEvidence.monitorFromWindowArguments?.join(",") !==
      "0x48010001,0x00000001" ||
    bgiBrowserEvidence.monitorFromWindowResult !== "0x4f010001" ||
    bgiBrowserEvidence.monitorFromWindowLastError !== 0 ||
    bgiBrowserEvidence.monitorFromWindowGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.monitorFromWindowImplementationId !==
      "user32.monitor-from-window.v1" ||
    bgiBrowserEvidence.monitorFromWindowDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    bgiBrowserEvidence.monitorFromWindowMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    bgiBrowserEvidence.monitorFromWindowOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.monitorFromWindowStateAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    bgiBrowserEvidence.monitorFromWindowWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    bgiBrowserEvidence.monitorFromWindowInvalidWindowResult !== "0x00000000" ||
    bgiBrowserEvidence.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    bgiBrowserEvidence.monitorFromWindowUnsupportedFlagsFault !==
      "monitor-from-window-flags-unsupported" ||
    bgiBrowserEvidence.monitorFromWindowFailureAtomicity !== true ||
    bgiBrowserEvidence.monitorFromWindowHostDisplayTopologyImported !== false ||
    bgiBrowserEvidence.monitorFromWindowBrowserDisplayTopologyImported !==
      false ||
    bgiBrowserEvidence.d3d9GetAdapterCountCompletions !== 2 ||
    bgiBrowserEvidence.d3d9GetAdapterCountSequence !== 17_843 ||
    bgiBrowserEvidence.d3d9GetAdapterCountArguments?.join(",") !==
      "0x01000004" ||
    bgiBrowserEvidence.d3d9GetAdapterCountResult !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterCountGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9GetAdapterCountReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterCountObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterCountOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorCompletions !== 2 ||
    bgiD3d9AdapterMonitorSequences.join(",") !== "17844,17848" ||
    JSON.stringify(bgiBrowserEvidence.d3d9GetAdapterMonitorArguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000"],
        ["0x01000004", "0x00000000"],
      ]) ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorResults?.join(",") !==
      "0x4f010001,0x4f010001" ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorIdentityMatchesMonitorFromWindow !==
      true ||
    bgiBrowserEvidence.d3d9GetAdapterMonitorOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeCompletions !== 2 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeSequence !== 17_817 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeArguments?.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterDisplayModeOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierSequence !== 17_818 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOutputBytes !== 1_100 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOutputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetAdapterIdentifierOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.d3d9GetDeviceCapsCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsSequence !== 17_819 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOutputBytes !== 304 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOutputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    bgiBrowserEvidence.d3d9GetDeviceCapsReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetDeviceCapsObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetDeviceCapsOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeCompletions !== 2 ||
    bgiD3d9CheckDeviceTypeSequences.join(",") !== "17820,17821" ||
    JSON.stringify(bgiBrowserEvidence.d3d9CheckDeviceTypeArguments) !==
      JSON.stringify([
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000001",
        ],
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000000",
        ],
      ]) ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeResults?.join(",") !==
      "0x00000000,0x00000000" ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9CheckDeviceTypeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatSequence !== 17_822 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9CheckDeviceFormatOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateDeviceCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceSequence !== 17_849 ||
    bgiBrowserEvidence.d3d9CreateDeviceArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    bgiBrowserEvidence.d3d9CreateDeviceResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateDeviceLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateDeviceGuestWriteBytes !== 16 ||
    bgiBrowserEvidence.d3d9CreateDevicePresentationPrefixHex !==
      "200300005802000016000000" ||
    bgiBrowserEvidence.d3d9CreateDevicePointer !== "0x01010004" ||
    bgiBrowserEvidence.d3d9CreateDeviceObjectId !== 3 ||
    bgiBrowserEvidence.d3d9CreateDeviceInterfaceId !== 6 ||
    bgiBrowserEvidence.d3d9CreateDeviceRootReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9CreateDeviceReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateDeviceLogicalGraphicsDeviceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateDeviceBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateDeviceOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateTextureCompletions !== 3 ||
    bgiBrowserEvidence.d3d9CreateTextureSequence !== 17_850 ||
    bgiBrowserEvidence.d3d9CreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    bgiBrowserEvidence.d3d9CreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9CreateTexturePointer !== "0x01020004" ||
    bgiBrowserEvidence.d3d9CreateTextureObjectId !== 4 ||
    bgiBrowserEvidence.d3d9CreateTextureInterfaceId !== 8 ||
    bgiBrowserEvidence.d3d9CreateTextureDeviceReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9CreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsDeviceId !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsTextureId !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsDeviceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateTextureLogicalGraphicsTextureCount !== 3 ||
    bgiBrowserEvidence.d3d9CreateTextureBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateTextureOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelSequence !== 17_851 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelArguments?.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLastError !== 0 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelPointer !== "0x01030004" ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelObjectId !== 5 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelInterfaceId !== 10 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelTextureReferenceCount !== 2 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsTextureId !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceCount !== 1 ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9GetSurfaceLevelOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceGetDcCompletions !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSequence !== 17_852 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcArguments?.join(",") !==
      "0x01030004,0x006ffd3c" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcOutputAddress !== "0x006ffd3c" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcHandle !== "0x47010002" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcDeviceContextId !== 2 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcHandleNamespace !==
      "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcTargetKind !== "graphics-surface" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSurfaceComObjectId !== 5 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcSurfaceGeometry?.join(",") !==
      "1024,1024" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcColorFormat !== "b8g8r8x8-unorm" ||
    bgiBrowserEvidence.d3d9SurfaceGetDcReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcObjectRetired !== false ||
    bgiBrowserEvidence.d3d9SurfaceGetDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceGetDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceGetStockObjectSequence !== 17_853 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectSequence !== 17_854 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectArguments?.join(",") !==
      "0x47010002,0x006ffd4c,0x47020001" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectResult !== "0x00000001" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectLogicalGraphicsSurfaceId !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectRect?.join(",") !== "0,0,800,600" ||
    bgiBrowserEvidence.d3d9SurfaceFillRectColorRef !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectContentGeneration !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectSurfaceGeneration !== 4 ||
    bgiBrowserEvidence.d3d9SurfaceFillRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcCompletions !== 1 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcSequence !== 17_855 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcArguments?.join(",") !==
      "0x01030004,0x47010002" ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcLastError !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcLiveDeviceContexts !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseSequence !== 17_856 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseResult !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseReferenceCount !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseObjectRetired !== true ||
    bgiBrowserEvidence.d3d9SurfaceReleaseLogicalGraphicsSurfaceCount !== 0 ||
    bgiBrowserEvidence.d3d9SurfaceReleaseOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SecondCreateTextureSequence !== 17_857 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000000,0x101517d0,0x00000000" ||
    bgiBrowserEvidence.d3d9SecondCreateTexturePool !== 0 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9SecondCreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9SecondCreateTexturePointer !== "0x01030004" ||
    bgiBrowserEvidence.d3d9SecondCreateTextureDeviceReferenceCount !== 3 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9SecondCreateTextureOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureSequence !== 17_858 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000200,0x00000016,0x00000000,0x101517d4,0x00000000" ||
    bgiBrowserEvidence.d3d9ThirdCreateTexturePool !== 0 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureUsage !== 512 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureLastError !== 0 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9ThirdCreateTexturePointer !== "0x01040004" ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureDeviceReferenceCount !== 4 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9ThirdCreateTextureOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9CreateVertexBufferCompletions !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferSequence !== 17_859 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferInstructionCount !== 11_807_897 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferArguments?.join(",") !==
      "0x01010004,0x00000070,0x00000008,0x00000144,0x00000001,0x101517dc,0x00000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLastError !== 0 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferPointer !== "0x01050004" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferDeviceReferenceCountAtCompletion !==
      4 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferObjectRetired !== false ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsDeviceId !== 1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsVertexBufferId !==
      1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferLogicalGraphicsVertexBufferCount !==
      1 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBackingAddress !== "0x44000000" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBackingBytes !== 112 ||
    bgiBrowserEvidence.d3d9CreateVertexBufferInitialContentSha256 !==
      "b5fdab78d8947eacc864bfeecb4d2100780e5afe1cd8efafb124887913ac49fa" ||
    bgiBrowserEvidence.d3d9CreateVertexBufferBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9CreateVertexBufferOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9VertexBufferLockCompletions !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferLockSequence !== 17_860 ||
    bgiBrowserEvidence.d3d9VertexBufferLockInstructionCount !== 11_808_429 ||
    bgiBrowserEvidence.d3d9VertexBufferLockArguments?.join(",") !==
      "0x01050004,0x00000000,0x00000000,0x006ffd54,0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockLastError !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferLockGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.d3d9VertexBufferLockDataPointer !== "0x44000000" ||
    bgiBrowserEvidence.d3d9VertexBufferLockReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferLockOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockCompletions !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockSequence !== 17_861 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockInstructionCount !== 11_808_439 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockArguments?.join(",") !==
      "0x01050004" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockLastError !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockReferenceCount !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockContentGeneration !== 1 ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockContentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockBackingProtection !==
      "read-only" ||
    bgiBrowserEvidence.d3d9VertexBufferUnlockOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9SetRenderStateCompletions !== 4 ||
    bgiBrowserEvidence.d3d9SetRenderStateSequences?.join(",") !==
      "17862,17863,17864,17865" ||
    bgiBrowserEvidence.d3d9SetRenderStateInstructionCounts?.join(",") !==
      "11808458,11808466,11808474,11808482" ||
    JSON.stringify(bgiBrowserEvidence.d3d9SetRenderStateArguments) !==
      JSON.stringify([
        ["0x01010004", "0x00000016", "0x00000001"],
        ["0x01010004", "0x00000007", "0x00000000"],
        ["0x01010004", "0x00000089", "0x00000000"],
        ["0x01010004", "0x0000001b", "0x00000001"],
      ]) ||
    bgiBrowserEvidence.d3d9SetRenderStateResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    bgiBrowserEvidence.d3d9SetRenderStateReturnAddresses?.join(",") !==
      "0x1005ad13,0x1005ad26,0x1005ad3c,0x1005ad50" ||
    bgiBrowserEvidence.d3d9SetRenderStateGuestWriteBytes?.join(",") !==
      "0,0,0,0" ||
    bgiBrowserEvidence.d3d9SetRenderStateDeviceReferenceCount !== 5 ||
    bgiBrowserEvidence.d3d9SetRenderStateLogicalValues?.join(",") !==
      "cull-none,depth-test-disabled,lighting-disabled,alpha-blend-enabled" ||
    bgiBrowserEvidence.d3d9SetRenderStateGenerationDelta !== 4 ||
    bgiBrowserEvidence.d3d9SetRenderStateBrowserOwnsState !== false ||
    bgiBrowserEvidence.d3d9SetRenderStateBrowserRendererCreated !== false ||
    bgiBrowserEvidence.d3d9SetRenderStateOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3d9GetRasterStatusCompletions !== 1 ||
    bgiBrowserEvidence.d3d9GetRasterStatusSequence !== 17_866 ||
    bgiBrowserEvidence.d3d9GetRasterStatusInstructionCount !== 11_808_586 ||
    bgiBrowserEvidence.d3d9GetRasterStatusArguments?.join(",") !==
      "0x01010004,0x00000000,0x006ffcf0" ||
    bgiBrowserEvidence.d3d9GetRasterStatusResult !== "0x00000000" ||
    bgiBrowserEvidence.d3d9GetRasterStatusLastError !== 0 ||
    bgiBrowserEvidence.d3d9GetRasterStatusGuestWriteBytes !== 8 ||
    bgiBrowserEvidence.d3d9GetRasterStatusOutputAddress !== "0x006ffcf0" ||
    bgiBrowserEvidence.d3d9GetRasterStatusOutputHex !== "0000000001000000" ||
    bgiBrowserEvidence.d3d9GetRasterStatusDeviceReferenceCount !== 5 ||
    bgiBrowserEvidence.d3d9GetRasterStatusObjectRetired !== false ||
    bgiBrowserEvidence.d3d9GetRasterStatusPolicyProfileId !==
      "primary-stable-nonblank-scanline-v1" ||
    bgiBrowserEvidence.d3d9GetRasterStatusDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    bgiBrowserEvidence.d3d9GetRasterStatusInVBlank !== false ||
    bgiBrowserEvidence.d3d9GetRasterStatusScanLine !== 1 ||
    bgiBrowserEvidence.d3d9GetRasterStatusStateMutation !== false ||
    bgiBrowserEvidence.d3d9GetRasterStatusHostScanoutImported !== false ||
    bgiBrowserEvidence.d3d9GetRasterStatusBrowserScanoutImported !== false ||
    bgiBrowserEvidence.d3d9GetRasterStatusOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.queryPerformanceFrequencyCompletions !== 1 ||
    bgiBrowserEvidence.queryPerformanceFrequencySequence !== 17_867 ||
    bgiBrowserEvidence.queryPerformanceFrequencyInstructionCount !==
      11_808_626 ||
    bgiBrowserEvidence.queryPerformanceFrequencyArguments?.join(",") !==
      "0x006ffcf0" ||
    bgiBrowserEvidence.queryPerformanceFrequencyResult !== "0x00000001" ||
    bgiBrowserEvidence.queryPerformanceFrequencyLastError !== 0 ||
    bgiBrowserEvidence.queryPerformanceFrequencyGuestWriteBytes !== 8 ||
    bgiBrowserEvidence.queryPerformanceFrequencyOutputAddress !==
      "0x006ffcf0" ||
    bgiBrowserEvidence.queryPerformanceFrequencyOutputHex !==
      "8096980000000000" ||
    bgiBrowserEvidence.queryPerformanceFrequencyValue !== 10_000_000 ||
    bgiBrowserEvidence.queryPerformanceFrequencyImplementationId !==
      "kernel32.query-performance-frequency.v1" ||
    bgiBrowserEvidence.queryPerformanceFrequencyStateMutation !== false ||
    bgiBrowserEvidence.queryPerformanceFrequencyHostClockImported !== false ||
    bgiBrowserEvidence.queryPerformanceFrequencyBrowserClockImported !==
      false ||
    bgiBrowserEvidence.queryPerformanceFrequencyOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.queryPerformanceCounterCompletions !== 301 ||
    bgiBrowserEvidence.queryPerformanceCounterFirstSequence !== 5 ||
    bgiBrowserEvidence.queryPerformanceCounterSecondSequence !== 17_868 ||
    bgiBrowserEvidence.queryPerformanceCounterLastSequence !== 18_437 ||
    bgiBrowserEvidence.queryPerformanceCounterFirstValue !== "12345678901" ||
    bgiBrowserEvidence.queryPerformanceCounterSecondValue !== "12345688901" ||
    bgiBrowserEvidence.queryPerformanceCounterLastValue !== "12348678901" ||
    bgiBrowserEvidence.queryPerformanceCounterIncrementPerRead !== 10_000 ||
    bgiBrowserEvidence.queryPerformanceCounterImplementationId !==
      "kernel32.query-performance-counter.v1" ||
    bgiBrowserEvidence.queryPerformanceCounterStateAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    bgiBrowserEvidence.queryPerformanceCounterHostClockImported !== false ||
    bgiBrowserEvidence.queryPerformanceCounterBrowserClockImported !== false ||
    bgiBrowserEvidence.queryPerformanceCounterOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.getMonitorInfoACompletions !== 1 ||
    bgiBrowserEvidence.getMonitorInfoASequence !== 18_439 ||
    bgiBrowserEvidence.getMonitorInfoAInstructionCount !== 11_824_580 ||
    bgiBrowserEvidence.getMonitorInfoAArguments?.join(",") !==
      "0x4f010001,0x006ffd44" ||
    bgiBrowserEvidence.getMonitorInfoAResult !== "0x00000001" ||
    bgiBrowserEvidence.getMonitorInfoALastError !== 0 ||
    bgiBrowserEvidence.getMonitorInfoAGuestWriteBytes !== 40 ||
    bgiBrowserEvidence.getMonitorInfoAOutputAddress !== "0x006ffd44" ||
    bgiBrowserEvidence.getMonitorInfoAOutputHex !==
      "28000000000000000000000080070000380400000000000000000000800700000804000001000000" ||
    bgiBrowserEvidence.getMonitorInfoAImplementationId !==
      "user32.get-monitor-info-a.v1" ||
    bgiBrowserEvidence.getMonitorInfoADisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    bgiBrowserEvidence.getMonitorInfoAMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    bgiBrowserEvidence.getMonitorInfoAWorkRect?.join(",") !== "0,0,1920,1032" ||
    bgiBrowserEvidence.getMonitorInfoAOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.directInput8CreateCompletions !== 1 ||
    bgiBrowserEvidence.directInput8CreateSequence !== 18_442 ||
    bgiBrowserEvidence.directInput8CreateInstructionCount !== 11_825_212 ||
    bgiBrowserEvidence.directInput8CreateArguments?.join(",") !==
      "0x10000000,0x00000800,0x100d22b8,0x10151800,0x00000000" ||
    bgiBrowserEvidence.directInput8CreateResult !== "0x00000000" ||
    bgiBrowserEvidence.directInput8CreateLastError !== 0 ||
    bgiBrowserEvidence.directInput8CreateGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.directInput8CreateOutputAddress !== "0x10151800" ||
    bgiBrowserEvidence.directInput8CreatePointer !== "0x01060004" ||
    bgiBrowserEvidence.directInput8CreateImplementationId !==
      "dinput8.direct-input-8-create.v1" ||
    bgiBrowserEvidence.directInput8CreateClassIdentity !==
      "dinput8.idirectinput8a-root.v1" ||
    bgiBrowserEvidence.directInput8CreateObjectId !== 9 ||
    bgiBrowserEvidence.directInput8CreateInterfaceId !== 18 ||
    bgiBrowserEvidence.directInput8CreateReferenceCount !== 1 ||
    bgiBrowserEvidence.directInput8CreateBackingAddress !== "0x01060000" ||
    bgiBrowserEvidence.directInput8CreateBackingBytes !== 8_192 ||
    bgiBrowserEvidence.directInput8CreateBrowserOwnsComState !== false ||
    bgiBrowserEvidence.directInput8CreateOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.directInput8EnumDevicesCompletions !== 1 ||
    bgiBrowserEvidence.directInput8EnumDevicesSequence !== 18_443 ||
    bgiBrowserEvidence.directInput8EnumDevicesInstructionCount !== 11_825_229 ||
    bgiBrowserEvidence.directInput8EnumDevicesArguments?.join(",") !==
      "0x01060004,0x00000004,0x1005bef0,0x00000000,0x00000001" ||
    bgiBrowserEvidence.directInput8EnumDevicesResult !== "0x00000000" ||
    bgiBrowserEvidence.directInput8EnumDevicesLastError !== 0 ||
    bgiBrowserEvidence.directInput8EnumDevicesGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.directInput8EnumDevicesDeviceClass !== 4 ||
    bgiBrowserEvidence.directInput8EnumDevicesFlags !== "0x00000001" ||
    bgiBrowserEvidence.directInput8EnumDevicesCallbackInvocations !== 0 ||
    bgiBrowserEvidence.directInput8EnumDevicesDeviceCount !== 0 ||
    bgiBrowserEvidence.directInput8EnumDevicesReferenceCount !== 1 ||
    bgiBrowserEvidence.directInput8EnumDevicesObjectRetired !== false ||
    bgiBrowserEvidence.directInput8EnumDevicesPolicyProfileId !==
      "empty-attached-game-controller-enumeration-v1" ||
    bgiBrowserEvidence.directInput8EnumDevicesBrowserInputImported !== false ||
    bgiBrowserEvidence.directInput8EnumDevicesOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.directSoundCreate8Completions !== 1 ||
    bgiBrowserEvidence.directSoundCreate8Sequence !== 18_453 ||
    bgiBrowserEvidence.directSoundCreate8InstructionCount !== 11_921_739 ||
    bgiBrowserEvidence.directSoundCreate8Arguments?.join(",") !==
      "0x00000000,0x100feb10,0x00000000" ||
    bgiBrowserEvidence.directSoundCreate8Result !== "0x00000000" ||
    bgiBrowserEvidence.directSoundCreate8LastError !== 0 ||
    bgiBrowserEvidence.directSoundCreate8GuestWriteBytes !== 4 ||
    bgiBrowserEvidence.directSoundCreate8OutputAddress !== "0x100feb10" ||
    bgiBrowserEvidence.directSoundCreate8Pointer !== "0x01070004" ||
    bgiBrowserEvidence.directSoundCreate8ImplementationId !==
      "dsound.direct-sound-create-8.v1" ||
    bgiBrowserEvidence.directSoundCreate8ClassIdentity !==
      "dsound.idirectsound8-root.v1" ||
    bgiBrowserEvidence.directSoundCreate8ObjectId !== 10 ||
    bgiBrowserEvidence.directSoundCreate8InterfaceId !== 20 ||
    bgiBrowserEvidence.directSoundCreate8ReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundCreate8BackingAddress !== "0x01070000" ||
    bgiBrowserEvidence.directSoundCreate8BackingBytes !== 8_192 ||
    bgiBrowserEvidence.directSoundCreate8BrowserOwnsComOrAudioState !== false ||
    bgiBrowserEvidence.directSoundCreate8OrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelCompletions !== 1 ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelSequence !== 18_454 ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelInstructionCount !==
      11_921_749 ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelArguments?.join(",") !==
      "0x01070004,0x48010001,0x00000002" ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelObjectRetired !== false ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelPolicyProfileId !==
      "primary-live-window-priority-v1" ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelAudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelBrowserAudioImported !==
      false ||
    bgiBrowserEvidence.directSoundSetCooperativeLevelOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.directSoundPrimaryBufferCompletions !== 1 ||
    bgiBrowserEvidence.directSoundPrimaryBufferSequence !== 18_455 ||
    bgiBrowserEvidence.directSoundPrimaryBufferInstructionCount !==
      11_921_781 ||
    bgiBrowserEvidence.directSoundPrimaryBufferArguments?.join(",") !==
      "0x01070004,0x100feae8,0x100feb18,0x00000000" ||
    bgiBrowserEvidence.directSoundPrimaryBufferResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundPrimaryBufferPointer !== "0x01080004" ||
    bgiBrowserEvidence.directSoundPrimaryBufferObjectId !== 11 ||
    bgiBrowserEvidence.directSoundPrimaryBufferInterfaceId !== 22 ||
    bgiBrowserEvidence.directSoundPrimaryBufferLogicalAudioBufferId !== 1 ||
    bgiBrowserEvidence.directSoundPrimarySetFormatCompletions !== 1 ||
    bgiBrowserEvidence.directSoundPrimarySetFormatSequence !== 18_456 ||
    bgiBrowserEvidence.directSoundPrimarySetFormatInstructionCount !==
      11_921_800 ||
    bgiBrowserEvidence.directSoundPrimarySetFormatResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundPrimaryPlayCompletions !== 1 ||
    bgiBrowserEvidence.directSoundPrimaryPlaySequence !== 18_457 ||
    bgiBrowserEvidence.directSoundPrimaryPlayInstructionCount !== 11_921_809 ||
    bgiBrowserEvidence.directSoundPrimaryPlayResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundSecondaryBufferCompletions !== 1 ||
    bgiBrowserEvidence.directSoundSecondaryBufferSequence !== 18_458 ||
    bgiBrowserEvidence.directSoundSecondaryBufferInstructionCount !==
      11_921_862 ||
    bgiBrowserEvidence.directSoundSecondaryBufferArguments?.join(",") !==
      "0x01070004,0x006ffe34,0x006ffe24,0x00000000" ||
    bgiBrowserEvidence.directSoundSecondaryBufferResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundSecondaryBufferPointer !== "0x01090004" ||
    bgiBrowserEvidence.directSoundSecondaryBufferObjectId !== 12 ||
    bgiBrowserEvidence.directSoundSecondaryBufferInterfaceId !== 24 ||
    bgiBrowserEvidence.directSoundSecondaryBufferLogicalAudioBufferId !== 2 ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceCompletions !== 1 ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceSequence !== 18_459 ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceInstructionCount !==
      11_921_872 ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceResult !==
      "0x00000000" ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceOutputPointer !==
      "0x01090004" ||
    bgiBrowserEvidence.directSoundBuffer8QueryInterfaceReferenceCount !== 2 ||
    bgiBrowserEvidence.directSoundBuffer8ReleaseCompletions !== 1 ||
    bgiBrowserEvidence.directSoundBuffer8ReleaseSequence !== 18_460 ||
    bgiBrowserEvidence.directSoundBuffer8ReleaseInstructionCount !==
      11_921_878 ||
    bgiBrowserEvidence.directSoundBuffer8ReleaseResult !== 1 ||
    bgiBrowserEvidence.directSoundBuffer8ReleaseReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundInitialBuffersPolicyProfileId !==
      "primary-stereo-pcm-and-secondary-88200-v1" ||
    bgiBrowserEvidence.directSoundInitialBuffersAudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    bgiBrowserEvidence.directSoundInitialBuffersBrowserAudioImported !==
      false ||
    bgiBrowserEvidence.directSoundInitialBuffersOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.directSoundBufferLockCompletions !== 1 ||
    bgiBrowserEvidence.directSoundBufferLockSequence !== 18_461 ||
    bgiBrowserEvidence.directSoundBufferLockInstructionCount !== 11_921_896 ||
    bgiBrowserEvidence.directSoundBufferLockArguments?.join(",") !==
      "0x01090004,0x00000000,0x00015888,0x006ffe2c,0x006ffe30,0x006ffe20,0x006ffe28,0x00000000" ||
    bgiBrowserEvidence.directSoundBufferLockResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundBufferLockGuestWriteBytes !== 16 ||
    bgiBrowserEvidence.directSoundBufferLockFirstSegmentAddress !==
      "0x45000000" ||
    bgiBrowserEvidence.directSoundBufferLockFirstSegmentBytes !== 88_200 ||
    bgiBrowserEvidence.directSoundBufferLockSecondSegmentAddress !==
      "0x00000000" ||
    bgiBrowserEvidence.directSoundBufferLockSecondSegmentBytes !== 0 ||
    bgiBrowserEvidence.directSoundBufferLockReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundBufferUnlockCompletions !== 1 ||
    bgiBrowserEvidence.directSoundBufferUnlockSequence !== 18_462 ||
    bgiBrowserEvidence.directSoundBufferUnlockInstructionCount !== 11_929_559 ||
    bgiBrowserEvidence.directSoundBufferUnlockArguments?.join(",") !==
      "0x01090004,0x45000000,0x00015888,0x00000000,0x00000000" ||
    bgiBrowserEvidence.directSoundBufferUnlockResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundBufferUnlockGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.directSoundBufferUnlockContentGeneration !== 1 ||
    bgiBrowserEvidence.directSoundBufferUnlockZeroBytes !== 88_200 ||
    bgiBrowserEvidence.directSoundBufferUnlockBackingProtection !==
      "read-only" ||
    bgiBrowserEvidence.directSoundBufferUnlockReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundSecondaryPlayCompletions !== 1 ||
    bgiBrowserEvidence.directSoundSecondaryPlaySequence !== 18_463 ||
    bgiBrowserEvidence.directSoundSecondaryPlayInstructionCount !==
      11_929_568 ||
    bgiBrowserEvidence.directSoundSecondaryPlayArguments?.join(",") !==
      "0x01090004,0x00000000,0x00000000,0x00000001" ||
    bgiBrowserEvidence.directSoundSecondaryPlayResult !== "0x00000000" ||
    bgiBrowserEvidence.directSoundSecondaryPlaybackState !== "looping" ||
    bgiBrowserEvidence.directSoundSecondaryPlayReferenceCount !== 1 ||
    bgiBrowserEvidence.directSoundBufferTransferPolicyProfileId !==
      "primary-secondary-full-lock-looping-play-v1" ||
    bgiBrowserEvidence.directSoundBufferTransferAudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    bgiBrowserEvidence.directSoundBufferTransferMemoryAuthority !==
      "rust-guest-memory" ||
    bgiBrowserEvidence.directSoundBufferTransferBrowserAudioImported !==
      false ||
    bgiBrowserEvidence.directSoundBufferTransferOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.loadLibraryACompletions !== 1 ||
    bgiBrowserEvidence.loadLibraryASequence !== 17_823 ||
    bgiBrowserEvidence.loadLibraryAArguments?.join(",") !== "0x100dc9c8" ||
    bgiBrowserEvidence.loadLibraryAResult !== "0x4d010010" ||
    bgiBrowserEvidence.loadLibraryALastError !== 0 ||
    bgiBrowserEvidence.loadLibraryAGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.loadLibraryAModuleId !== 16 ||
    bgiBrowserEvidence.loadLibraryAModuleClass !== "runtime-system-module" ||
    bgiBrowserEvidence.loadLibraryAModuleState !== "process-attached" ||
    bgiBrowserEvidence.loadLibraryAAttachSequence !== 16 ||
    bgiBrowserEvidence.loadLibraryAObjectId !== 23 ||
    bgiBrowserEvidence.loadLibraryAThreadReferenceCount !== 1 ||
    bgiBrowserEvidence.loadLibraryAOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.d3dxGetProcAddressCompletions !== 1 ||
    bgiBrowserEvidence.d3dxGetProcAddressSequence !== 17_824 ||
    bgiBrowserEvidence.d3dxGetProcAddressArguments?.join(",") !==
      "0x4d010010,0x100dc9d8" ||
    bgiBrowserEvidence.d3dxGetProcAddressResult !== "0x7f020040" ||
    bgiBrowserEvidence.d3dxGetProcAddressLastError !== 0 ||
    bgiBrowserEvidence.d3dxGetProcAddressOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getClientRectCompletions !==
      (archiveSourceReadActive ? 2 : 1) ||
    bgiBrowserEvidence.getClientRectSequence !== 17_825 ||
    bgiBrowserEvidence.getClientRectArguments?.join(",") !==
      "0x48010001,0x006ffdc4" ||
    bgiBrowserEvidence.getClientRectResult !== "0x00000001" ||
    bgiBrowserEvidence.getClientRectLastError !== 0 ||
    bgiBrowserEvidence.getClientRectGuestWriteBytes !== 16 ||
    bgiBrowserEvidence.getClientRectOutputHex !==
      "00000000000000002003000058020000" ||
    bgiBrowserEvidence.getClientRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getDcCompletions !== (archiveSourceReadActive ? 2 : 1) ||
    bgiBrowserEvidence.getDcSequence !== 17_826 ||
    bgiBrowserEvidence.getDcArguments?.join(",") !== "0x48010001" ||
    bgiBrowserEvidence.getDcResult !== "0x47010001" ||
    bgiBrowserEvidence.getDcLastError !== 0 ||
    bgiBrowserEvidence.getDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.getDcDeviceContextId !== 1 ||
    bgiBrowserEvidence.getDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.getDcSurfaceId !== 1 ||
    bgiBrowserEvidence.getDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.getDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.getStockObjectCompletions !==
      (archiveSourceReadActive ? 3 : 2) ||
    bgiBrowserEvidence.getStockObjectSequence !== 17_827 ||
    bgiBrowserEvidence.getStockObjectArguments?.join(",") !== "0x00000004" ||
    bgiBrowserEvidence.getStockObjectResult !== "0x47020001" ||
    bgiBrowserEvidence.getStockObjectLastError !== 0 ||
    bgiBrowserEvidence.getStockObjectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.getStockObjectId !== 1 ||
    bgiBrowserEvidence.getStockObjectHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    bgiBrowserEvidence.getStockObjectColorRef !== 0 ||
    bgiBrowserEvidence.getStockObjectLiveObjects !== 1 ||
    bgiBrowserEvidence.getStockObjectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.fillRectCompletions !==
      (archiveSourceReadActive ? 3 : 2) ||
    bgiBrowserEvidence.fillRectSequence !== 17_828 ||
    bgiBrowserEvidence.fillRectArguments?.join(",") !==
      "0x47010001,0x006ffdc4,0x47020001" ||
    bgiBrowserEvidence.fillRectResult !== "0x00000001" ||
    bgiBrowserEvidence.fillRectLastError !== 0 ||
    bgiBrowserEvidence.fillRectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.fillRectSurfaceId !== 1 ||
    bgiBrowserEvidence.fillRectSurfaceWidth !== 800 ||
    bgiBrowserEvidence.fillRectSurfaceHeight !== 600 ||
    bgiBrowserEvidence.fillRectSurfaceGeneration !== 1 ||
    bgiBrowserEvidence.fillRectFillRect?.join(",") !== "0,0,800,600" ||
    bgiBrowserEvidence.fillRectColorRef !== 0 ||
    bgiBrowserEvidence.fillRectOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.releaseDcCompletions !==
      (archiveSourceReadActive ? 3 : 2) ||
    bgiBrowserEvidence.releaseDcSequence !== 17_829 ||
    bgiBrowserEvidence.releaseDcArguments?.join(",") !==
      "0x48010001,0x47010001" ||
    bgiBrowserEvidence.releaseDcResult !== "0x00000001" ||
    bgiBrowserEvidence.releaseDcLastError !== 0 ||
    bgiBrowserEvidence.releaseDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.releaseDcLiveDeviceContexts !== 0 ||
    bgiBrowserEvidence.releaseDcLiveSurfaces !== 1 ||
    bgiBrowserEvidence.releaseDcOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.setWindowLongACompletions !==
      (archiveSourceReadActive ? 2 : 1) ||
    bgiBrowserEvidence.setWindowLongASequence !== 17_830 ||
    bgiBrowserEvidence.setWindowLongAArguments?.join(",") !==
      "0x48010001,0xfffffff0,0x90ca0000" ||
    bgiBrowserEvidence.setWindowLongAResult !== "0x84ca0000" ||
    bgiBrowserEvidence.setWindowLongALastError !== 0 ||
    bgiBrowserEvidence.setWindowLongAGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.setWindowLongAPreviousEffectiveStyle !== "0x84ca0000" ||
    bgiBrowserEvidence.setWindowLongAEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.setWindowLongAVisible !== true ||
    bgiBrowserEvidence.setWindowLongATopmost !== false ||
    bgiBrowserEvidence.setWindowLongAOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.setWindowPosActivationCompletions !== 1 ||
    bgiBrowserEvidence.setWindowPosActivationSequence !== 17_831 ||
    bgiBrowserEvidence.setWindowPosActivationArguments?.join(",") !==
      "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000326,0x00000275,0x0000010a" ||
    bgiBrowserEvidence.setWindowPosActivationResult !== "0x00000001" ||
    bgiBrowserEvidence.setWindowPosActivationLastError !== 0 ||
    bgiBrowserEvidence.setWindowPosActivationGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.setWindowPosActivationEffectiveStyle !== "0x94ca0000" ||
    bgiBrowserEvidence.setWindowPosActivationGeometry?.join(",") !==
      "15360,8640,806,629,15363,8666,800,600" ||
    bgiBrowserEvidence.setWindowPosActivationVisible !== true ||
    bgiBrowserEvidence.setWindowPosActivationTopmost !== false ||
    bgiBrowserEvidence.setWindowPosActivationMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    bgiBrowserEvidence.setWindowPosActivationMessageResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    bgiBrowserEvidence.setWindowPosActivationActiveWindow !== "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationFocusWindow !== "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationForegroundWindow !==
      "0x48010001" ||
    bgiBrowserEvidence.setWindowPosActivationOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.timeGetTimeCompletions !== 0 ||
    bgiBrowserEvidence.timeGetTimeReached !== false ||
    bgiBrowserEvidence.timeGetTimeImplementationId !==
      "winmm.time-get-time.v1" ||
    bgiBrowserEvidence.timeGetTimePolicyProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    bgiBrowserEvidence.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    bgiBrowserEvidence.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    bgiBrowserEvidence.timeGetTimeIncrementPerRead !== 1 ||
    bgiBrowserEvidence.timeGetTimeTimerResolutionAffectsClock !== false ||
    bgiBrowserEvidence.timeGetTimeHostClockImported !== false ||
    bgiBrowserEvidence.timeGetTimeBrowserClockImported !== false ||
    bgiBrowserEvidence.timeGetTimeNodeChromeReachabilityMatched !== true ||
    bgiBrowserEvidence.browserOwnsGdiState !== false ||
    bgiBrowserEvidence.runtimeGraphicsCreatedByWindowDcSlice !== false ||
    bgiBrowserEvidence.browserRendererResourceCreatedByWindowDcSlice !==
      false ||
    bgiBrowserEvidence.runtimeGraphicsSurfaceFilledBySurfaceDcSlice !== true ||
    bgiBrowserEvidence.browserRendererResourceCreatedBySurfaceDcSlice !==
      false ||
    bgiBrowserEvidence.graphicsSurfacePixelStorageCreatedBySurfaceDcSlice !==
      false
  ) {
    errors.push("browser BGI authority or teardown evidence is invalid");
  }
  if (
    setWindowPosRepeatActivationActive &&
    (bgiBrowserEvidence.setWindowPosRepeatCompletions !== 1 ||
      bgiBrowserEvidence.setWindowPosRepeatCallCount !== 5 ||
      bgiBrowserEvidence.setWindowPosRepeatSequence !== 108_718 ||
      bgiBrowserEvidence.setWindowPosRepeatOccurrence !== 5 ||
      bgiBrowserEvidence.setWindowPosRepeatInstructionCount !== 28_273_977 ||
      bgiBrowserEvidence.setWindowPosRepeatArguments?.join(",") !==
        "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000506,0x000002ed,0x0000010a" ||
      bgiBrowserEvidence.setWindowPosRepeatResult !== 1 ||
      bgiBrowserEvidence.setWindowPosRepeatImplementationId !==
        "user32.set-window-pos.v5" ||
      bgiBrowserEvidence.setWindowPosRepeatReturnAddress !== "0x1005cb4b" ||
      bgiBrowserEvidence.setWindowPosRepeatStackPointers?.join(",") !==
        "0x006ff5e0,0x006ff600" ||
      bgiBrowserEvidence.setWindowPosRepeatLastError !== 0 ||
      bgiBrowserEvidence.setWindowPosRepeatGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.setWindowPosRepeatGeometry?.join(",") !==
        "15360,8640,1286,749,15363,8666,1280,720" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackFrameId !== 25 ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackTarget !== "0x10092550" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackAllocationStart !==
        "0x006ff5cc" ||
      bgiBrowserEvidence.setWindowPosRepeatScratchAddress !== "0x00501000" ||
      bgiBrowserEvidence.setWindowPosRepeatScratchBytes !== 28 ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackFrameBytes !== 20 ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackReturnSentinel !==
        "0x7f010000" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackMessages?.join(",") !==
        "70" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackResults?.join(",") !== "0" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackReturnInstructionCount !==
        28_274_079 ||
      bgiBrowserEvidence.setWindowPosRepeatWindowAuthority !==
        "rust-runtime-user32-logical-window" ||
      bgiBrowserEvidence.setWindowPosRepeatRelationAuthority !==
        "rust-runtime-user32-thread-and-process-window-relations" ||
      bgiBrowserEvidence.setWindowPosRepeatCallbackDisposition !==
        "validate-only" ||
      bgiBrowserEvidence.setWindowPosRepeatMutation !== false ||
      bgiBrowserEvidence.setWindowPosRepeatFailureAtomicity !== true ||
      bgiBrowserEvidence.setWindowPosRepeatBrowserOwnsLogicalWindowState !==
        false ||
      bgiBrowserEvidence.setWindowPosRepeatBrowserResourceCreated !== false ||
      bgiBrowserEvidence.setWindowPosRepeatNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI repeated SetWindowPos evidence is invalid");
  }
  if (
    d3d9RecreatedTexturesActive &&
    (bgiBrowserEvidence.d3d9RecreatedTexturesCompletions !== 9 ||
      bgiBrowserEvidence.d3d9RecreatedTexturesSequences?.join(",") !==
        "108735,108736,108737,108738,108739,108740,108741,108742,108743" ||
      bgiBrowserEvidence.d3d9RecreatedTextureCreateSequences?.join(",") !==
        "108735,108742,108743" ||
      bgiBrowserEvidence.d3d9RecreatedTextureCreateInstructionCounts?.join(
        ",",
      ) !== "28274782,28274850,28274866" ||
      bgiBrowserEvidence.d3d9RecreatedTexturePointers?.join(",") !==
        "0x01020004,0x01030004,0x01040004" ||
      bgiBrowserEvidence.d3d9RecreatedTextureReferenceCounts?.join(",") !==
        "1,2,3" ||
      bgiBrowserEvidence.d3d9RecreatedTextureSurfaceGeometry?.join(",") !==
        "2048,1024" ||
      bgiBrowserEvidence.d3d9RecreatedTextureFillRect?.join(",") !==
        "0,0,1280,720" ||
      bgiBrowserEvidence.d3d9RecreatedTextureCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9RecreatedTextureCreateProfileId !==
        "primary-initial-and-recreated-x8r8g8b8-textures-v2" ||
      bgiBrowserEvidence.d3d9RecreatedTextureGetSurfaceLevelProfileId !==
        "primary-system-memory-texture-level-zero-surfaces-v2" ||
      bgiBrowserEvidence.d3d9RecreatedTextureBrowserRendererResourceCreated !==
        false ||
      bgiBrowserEvidence.d3d9RecreatedTextureMutation !==
        "cross-owner-preflight-transactional-commit" ||
      bgiBrowserEvidence.d3d9RecreatedTextureNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI recreated-texture evidence is invalid");
  }
  if (
    setWindowPosFrameChangeMoveActive &&
    (bgiBrowserEvidence.setWindowPosFrameChangeMoveCompletions !== 1 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveCallCount !== 6 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveSequence !== 127_346 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveOccurrence !== 6 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveInstructionCount !==
        28_760_801 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveArguments?.join(",") !==
        "0x48010001,0x00000000,0x0000013d,0x0000009a,0x00000000,0x00000000,0x00000025" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveResult !== 1 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveImplementationId !==
        "user32.set-window-pos.v5" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveReturnAddress !==
        "0x100938c2" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveStackPointers?.join(",") !==
        "0x006ff670,0x006ff690" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveLastError !== 0 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveGeometry?.join(",") !==
        "317,154,1286,749,320,180,1280,720" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveCallbackFrameIds?.join(
        ",",
      ) !== "26,27,28" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveCallbackMessages?.join(
        ",",
      ) !== "70,131,71" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveCallbackResults?.join(
        ",",
      ) !== "0,0,0" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveCallbackInstructionCounts?.join(
        ",",
      ) !== "28760801,28760858,28760858,28760911,28760911,28761017" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNestedMoveFrameId !== 29 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNestedMoveSequence !==
        127_349 ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNestedMoveInstructionCounts?.join(
        ",",
      ) !== "28760951,28761004" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNestedMoveLParam !==
        "0x00b40140" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveWindowAuthority !==
        "rust-runtime-user32-logical-window" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveRelationAuthority !==
        "rust-runtime-user32-thread-and-process-window-relations" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveGeometryCommit !==
        "nccalcsize-completion-targeted-commit" ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveRelationMutation !==
        false ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveGdiResourceMutation !==
        false ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveBrowserResourceCreated !==
        false ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNonClientPaintAndEraseModeled !==
        false ||
      bgiBrowserEvidence.setWindowPosFrameChangeMoveNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI frame-change SetWindowPos evidence is invalid");
  }
  if (
    showWindowNormalActive &&
    (bgiBrowserEvidence.showWindowNormalCompletions !== 1 ||
      bgiBrowserEvidence.showWindowNormalSequence !== 127_351 ||
      bgiBrowserEvidence.showWindowNormalInstructionCount !== 28_761_027 ||
      bgiBrowserEvidence.showWindowNormalArguments?.join(",") !==
        "0x48010001,0x00000001" ||
      bgiBrowserEvidence.showWindowNormalResult !== 1 ||
      bgiBrowserEvidence.showWindowNormalImplementationId !==
        "user32.show-window.visible-normal-noop.v1" ||
      bgiBrowserEvidence.showWindowNormalReturnAddress !== "0x100938da" ||
      bgiBrowserEvidence.showWindowNormalStackPointers?.join(",") !==
        "0x006ff684,0x006ff690" ||
      bgiBrowserEvidence.showWindowNormalLastError !== 0 ||
      bgiBrowserEvidence.showWindowNormalGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.showWindowNormalWindowAuthority !==
        "rust-runtime-user32-logical-window" ||
      bgiBrowserEvidence.showWindowNormalRelationAuthority !==
        "rust-runtime-user32-thread-and-process-window-relations" ||
      bgiBrowserEvidence.showWindowNormalMutation !== false ||
      bgiBrowserEvidence.showWindowNormalCallbackCount !== 0 ||
      bgiBrowserEvidence.showWindowNormalBrowserResourceCreated !== false ||
      bgiBrowserEvidence.showWindowNormalNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI ShowWindow normal evidence is invalid");
  }
  if (
    updateWindowActive &&
    (bgiBrowserEvidence.updateWindowCalls !== 1 ||
      bgiBrowserEvidence.updateWindowSequence !== 127_352 ||
      bgiBrowserEvidence.updateWindowInstructionCount !== 28_761_031 ||
      bgiBrowserEvidence.updateWindowArguments?.join(",") !== "0x48010001" ||
      bgiBrowserEvidence.updateWindowPendingCallback !== true ||
      bgiBrowserEvidence.updateWindowCallbackFrameId !== 30 ||
      bgiBrowserEvidence.updateWindowCallbackMessage !== 15 ||
      bgiBrowserEvidence.updateWindowCallbackTarget !== "0x10092550" ||
      bgiBrowserEvidence.updateWindowCallbackStackPointer !== "0x006ff674" ||
      bgiBrowserEvidence.beginPaintCompletions !== 1 ||
      bgiBrowserEvidence.beginPaintSequence !== 127_353 ||
      bgiBrowserEvidence.beginPaintInstructionCount !== 28_761_068 ||
      bgiBrowserEvidence.beginPaintArguments?.join(",") !==
        "0x48010001,0x006ff318" ||
      bgiBrowserEvidence.beginPaintResult !== "0x47010009" ||
      bgiBrowserEvidence.beginPaintImplementationId !==
        "user32.begin-paint.full-client-logical-dc.v1" ||
      bgiBrowserEvidence.beginPaintReturnAddress !== "0x100928d9" ||
      bgiBrowserEvidence.beginPaintStackPointers?.join(",") !==
        "0x006ff2cc,0x006ff2d8" ||
      bgiBrowserEvidence.beginPaintLastError !== 0 ||
      bgiBrowserEvidence.beginPaintGuestWriteBytes !== 64 ||
      bgiBrowserEvidence.beginPaintPaintStructBytes !== 64 ||
      bgiBrowserEvidence.beginPaintErase !== 1 ||
      bgiBrowserEvidence.beginPaintUpdateRect?.join(",") !== "0,0,1280,720" ||
      bgiBrowserEvidence.beginPaintGeneration !== 1 ||
      bgiBrowserEvidence.beginPaintLivePaintSessions !== 1 ||
      bgiBrowserEvidence.beginPaintLiveDeviceContexts !== 1 ||
      bgiBrowserEvidence.endPaintCompletions !== 1 ||
      bgiBrowserEvidence.endPaintSequence !== 127_354 ||
      bgiBrowserEvidence.endPaintInstructionCount !== 28_761_073 ||
      bgiBrowserEvidence.endPaintArguments?.join(",") !==
        "0x48010001,0x006ff318" ||
      bgiBrowserEvidence.endPaintResult !== 1 ||
      bgiBrowserEvidence.endPaintImplementationId !==
        "user32.end-paint.matching-session.v1" ||
      bgiBrowserEvidence.endPaintReturnAddress !== "0x100928e5" ||
      bgiBrowserEvidence.endPaintStackPointers?.join(",") !==
        "0x006ff2cc,0x006ff2d8" ||
      bgiBrowserEvidence.endPaintLastError !== 0 ||
      bgiBrowserEvidence.endPaintGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.endPaintLivePaintSessions !== 0 ||
      bgiBrowserEvidence.endPaintLiveDeviceContexts !== 0 ||
      bgiBrowserEvidence.updateWindowWindowAuthority !==
        "rust-runtime-user32-logical-window" ||
      bgiBrowserEvidence.updateWindowUpdateRegionAuthority !==
        "rust-runtime-user32-window-update-region" ||
      bgiBrowserEvidence.updateWindowUpdateRegionSource !==
        "set-window-pos-frame-change-nccalcsize-completion" ||
      bgiBrowserEvidence.updateWindowUpdateRegionConsumption !==
        "begin-paint-targeted-commit" ||
      bgiBrowserEvidence.updateWindowPaintSessionAuthority !==
        "rust-runtime-user32-paint-session" ||
      bgiBrowserEvidence.updateWindowDeviceContextAuthority !==
        "rust-runtime-gdi-device-context" ||
      bgiBrowserEvidence.updateWindowPaintSessionRetired !== true ||
      bgiBrowserEvidence.updateWindowDeviceContextRetired !== true ||
      bgiBrowserEvidence.updateWindowFailureAtomicity !== true ||
      bgiBrowserEvidence.updateWindowCallbackActiveAtStop !== true ||
      bgiBrowserEvidence.updateWindowBrowserResourceCreated !== false ||
      bgiBrowserEvidence.updateWindowNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("browser BGI UpdateWindow paint lifecycle evidence is invalid");
  }
  if (
    d3d9ClearActive &&
    (bgiBrowserEvidence.d3d9ClearCompletions !== 1 ||
      bgiBrowserEvidence.d3d9ClearSequence !== 127_355 ||
      bgiBrowserEvidence.d3d9ClearInstructionCount !== 28_761_128 ||
      bgiBrowserEvidence.d3d9ClearArguments?.join(",") !==
        "0x01010004,0x00000000,0x00000000,0x00000001,0xff000000,0x3f800000,0x00000000" ||
      bgiBrowserEvidence.d3d9ClearResult !== "0x00000000" ||
      bgiBrowserEvidence.d3d9ClearReturnAddress !== "0x1005af9e" ||
      bgiBrowserEvidence.d3d9ClearStackPointers?.join(",") !==
        "0x006ff248,0x006ff268" ||
      bgiBrowserEvidence.d3d9ClearGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.d3d9ClearDeviceReferenceCount !== 5 ||
      bgiBrowserEvidence.d3d9ClearDeviceObjectRetired !== false ||
      bgiBrowserEvidence.d3d9ClearCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9ClearProfileId !==
        "primary-full-render-target-black-v1" ||
      bgiBrowserEvidence.d3d9ClearTargetAuthority !==
        "rust-runtime-graphics-logical-device-back-buffer" ||
      bgiBrowserEvidence.d3d9ClearTargetScope !==
        "full-presentation-back-buffer" ||
      bgiBrowserEvidence.d3d9ClearMutation !==
        "validate-plan-preflight-targeted-commit" ||
      bgiBrowserEvidence.d3d9ClearFailureAtomicity !== true ||
      bgiBrowserEvidence.d3d9ClearLogicalGraphicsDeviceId !== 3 ||
      bgiBrowserEvidence.d3d9ClearLogicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      bgiBrowserEvidence.d3d9ClearLogicalGraphicsComObjectId !== "21" ||
      bgiBrowserEvidence.d3d9ClearPresentationGeometry?.join(",") !==
        "1280,720" ||
      bgiBrowserEvidence.d3d9ClearBackBufferContentGeneration !== 1 ||
      bgiBrowserEvidence.d3d9ClearBackBufferSequence !== 1 ||
      bgiBrowserEvidence.d3d9ClearBackBufferRect?.join(",") !==
        "0,0,1280,720" ||
      bgiBrowserEvidence.d3d9ClearBackBufferColorArgb !== "0xff000000" ||
      bgiBrowserEvidence.d3d9ClearLogicalGraphicsDeviceGeneration !== 44 ||
      bgiBrowserEvidence.d3d9ClearDepthStencilCleared !== false ||
      bgiBrowserEvidence.d3d9ClearPartialRectanglesImplemented !== false ||
      bgiBrowserEvidence.d3d9ClearBrowserRendererResourceCreated !== false ||
      bgiBrowserEvidence.d3d9ClearNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("browser BGI D3D9 Clear evidence is invalid");
  }
  if (
    setRectActive &&
    (bgiBrowserEvidence.setRectCompletions !== 1 ||
      bgiBrowserEvidence.setRectSequence !== 127_356 ||
      bgiBrowserEvidence.setRectInstructionCount !== 28_761_157 ||
      bgiBrowserEvidence.setRectArguments?.join(",") !==
        "0x006ff29c,0x00000000,0x00000000,0x00000500,0x000002d0" ||
      bgiBrowserEvidence.setRectResult !== 1 ||
      bgiBrowserEvidence.setRectImplementationId !== "user32.set-rect.v1" ||
      bgiBrowserEvidence.setRectReturnAddress !== "0x1005b00b" ||
      bgiBrowserEvidence.setRectStackPointers?.join(",") !==
        "0x006ff250,0x006ff268" ||
      bgiBrowserEvidence.setRectLastError !== 0 ||
      bgiBrowserEvidence.setRectGuestWriteBytes !== 16 ||
      bgiBrowserEvidence.setRectOutputAddress !== "0x006ff29c" ||
      bgiBrowserEvidence.setRectOutputHex !==
        "000000000000000000050000d0020000" ||
      bgiBrowserEvidence.setRectOutputType !== "RECT" ||
      bgiBrowserEvidence.setRectCoordinateEncoding !== "signed-long32" ||
      bgiBrowserEvidence.setRectLastErrorPolicy !== "preserve" ||
      bgiBrowserEvidence.setRectMutation !==
        "guest-memory-preflight-atomic-write" ||
      bgiBrowserEvidence.setRectNullOutputReturnsFalse !== true ||
      bgiBrowserEvidence.setRectFailureAtomicity !== true ||
      bgiBrowserEvidence.setRectBrowserOwnsRectState !== false ||
      bgiBrowserEvidence.setRectNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("browser BGI SetRect evidence is invalid");
  }
  if (
    d3d9AddDirtyRectActive &&
    (bgiBrowserEvidence.d3d9AddDirtyRectCompletions !== 1 ||
      bgiBrowserEvidence.d3d9AddDirtyRectSequence !== 127_357 ||
      bgiBrowserEvidence.d3d9AddDirtyRectInstructionCount !== 28_761_165 ||
      bgiBrowserEvidence.d3d9AddDirtyRectArguments?.join(",") !==
        "0x01020004,0x006ff29c" ||
      bgiBrowserEvidence.d3d9AddDirtyRectResult !== "0x00000000" ||
      bgiBrowserEvidence.d3d9AddDirtyRectReturnAddress !== "0x1005b01d" ||
      bgiBrowserEvidence.d3d9AddDirtyRectStackPointers?.join(",") !==
        "0x006ff25c,0x006ff268" ||
      bgiBrowserEvidence.d3d9AddDirtyRectGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureReferenceCount !== 1 ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureObjectRetired !== false ||
      bgiBrowserEvidence.d3d9AddDirtyRectCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9AddDirtyRectProfileId !==
        "primary-recreated-system-memory-texture-dirty-rect-v1" ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureAuthority !==
        "rust-runtime-graphics-logical-texture" ||
      bgiBrowserEvidence.d3d9AddDirtyRectInitialDirtyRegion !==
        "whole-texture" ||
      bgiBrowserEvidence.d3d9AddDirtyRectMutation !==
        "validate-plan-preflight-idempotent-completion" ||
      bgiBrowserEvidence.d3d9AddDirtyRectFailureAtomicity !== true ||
      bgiBrowserEvidence.d3d9AddDirtyRectLogicalGraphicsTextureId !== 7 ||
      bgiBrowserEvidence.d3d9AddDirtyRectLogicalGraphicsTextureNamespace !==
        "vine.logical-graphics-texture.v1" ||
      bgiBrowserEvidence.d3d9AddDirtyRectLogicalGraphicsDeviceId !== 3 ||
      bgiBrowserEvidence.d3d9AddDirtyRectLogicalGraphicsComObjectId !== "22" ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureGeometry?.join(",") !==
        "2048,1024" ||
      bgiBrowserEvidence.d3d9AddDirtyRectTexturePool !== "system-memory" ||
      bgiBrowserEvidence.d3d9AddDirtyRectDirtyRegion !== "whole-texture" ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureGeneration !== 41 ||
      bgiBrowserEvidence.d3d9AddDirtyRectContentGeneration !== 1 ||
      bgiBrowserEvidence.d3d9AddDirtyRectLogicalFillSequence !== 3 ||
      bgiBrowserEvidence.d3d9AddDirtyRectTextureGenerationChanged !== false ||
      bgiBrowserEvidence.d3d9AddDirtyRectDirtyRegionConsumed !== false ||
      bgiBrowserEvidence.d3d9AddDirtyRectBrowserRendererResourceCreated !==
        false ||
      bgiBrowserEvidence.d3d9AddDirtyRectNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI D3D9 AddDirtyRect evidence is invalid");
  }
  if (
    d3d9UpdateTextureActive &&
    (bgiBrowserEvidence.d3d9UpdateTextureCompletions !== 1 ||
      bgiBrowserEvidence.d3d9UpdateTextureSequence !== 127_358 ||
      bgiBrowserEvidence.d3d9UpdateTextureInstructionCount !== 28_761_175 ||
      bgiBrowserEvidence.d3d9UpdateTextureArguments?.join(",") !==
        "0x01010004,0x01020004,0x01030004" ||
      bgiBrowserEvidence.d3d9UpdateTextureResult !== "0x00000000" ||
      bgiBrowserEvidence.d3d9UpdateTextureReturnAddress !== "0x1005b038" ||
      bgiBrowserEvidence.d3d9UpdateTextureStackPointers?.join(",") !==
        "0x006ff258,0x006ff268" ||
      bgiBrowserEvidence.d3d9UpdateTextureGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.d3d9UpdateTextureDeviceReferenceCount !== 5 ||
      bgiBrowserEvidence.d3d9UpdateTextureDeviceObjectRetired !== false ||
      bgiBrowserEvidence.d3d9UpdateTextureCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9UpdateTextureProfileId !==
        "primary-recreated-system-memory-to-static-default-texture-update-v1" ||
      bgiBrowserEvidence.d3d9UpdateTextureContentAuthority !==
        "rust-runtime-graphics-logical-texture" ||
      bgiBrowserEvidence.d3d9UpdateTextureSourceDirtyRegionBefore !==
        "whole-texture" ||
      bgiBrowserEvidence.d3d9UpdateTextureSourceDirtyRegionAfter !== "clean" ||
      bgiBrowserEvidence.d3d9UpdateTextureDirtyRegionConsumed !== true ||
      bgiBrowserEvidence.d3d9UpdateTextureTransferScope !==
        "whole-texture-level-zero-logical-content" ||
      bgiBrowserEvidence.d3d9UpdateTextureMutation !==
        "dual-texture-preflight-targeted-commit" ||
      bgiBrowserEvidence.d3d9UpdateTextureFailureAtomicity !== true ||
      bgiBrowserEvidence.d3d9UpdateTextureSourceTextureId !== 7 ||
      bgiBrowserEvidence.d3d9UpdateTextureSourceComObjectId !== "22" ||
      bgiBrowserEvidence.d3d9UpdateTextureSourcePool !== "system-memory" ||
      bgiBrowserEvidence.d3d9UpdateTextureSourceGeneration !== 42 ||
      bgiBrowserEvidence.d3d9UpdateTextureDestinationTextureId !== 8 ||
      bgiBrowserEvidence.d3d9UpdateTextureDestinationComObjectId !== "24" ||
      bgiBrowserEvidence.d3d9UpdateTextureDestinationPool !== "default" ||
      bgiBrowserEvidence.d3d9UpdateTextureDestinationGeneration !== 45 ||
      bgiBrowserEvidence.d3d9UpdateTextureContentGeneration !== 1 ||
      bgiBrowserEvidence.d3d9UpdateTextureLogicalFillSequence !== 3 ||
      bgiBrowserEvidence.d3d9UpdateTextureLogicalFillRect?.join(",") !==
        "0,0,1280,720" ||
      bgiBrowserEvidence.d3d9UpdateTextureLogicalFillColorRef !==
        "0x00000000" ||
      bgiBrowserEvidence.d3d9UpdateTextureUpdateSequence !== 1 ||
      bgiBrowserEvidence.d3d9UpdateTexturePartialDirtyRegionsImplemented !==
        false ||
      bgiBrowserEvidence.d3d9UpdateTextureDynamicDestinationImplemented !==
        false ||
      bgiBrowserEvidence.d3d9UpdateTextureBrowserRendererResourceCreated !==
        false ||
      bgiBrowserEvidence.d3d9UpdateTextureNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI D3D9 UpdateTexture evidence is invalid");
  }
  if (
    d3d9BeginSceneActive &&
    (bgiBrowserEvidence.d3d9BeginSceneCompletions !== 1 ||
      bgiBrowserEvidence.d3d9BeginSceneSequence !== 127_359 ||
      bgiBrowserEvidence.d3d9BeginSceneInstructionCount !== 28_761_181 ||
      bgiBrowserEvidence.d3d9BeginSceneArguments?.join(",") !== "0x01010004" ||
      bgiBrowserEvidence.d3d9BeginSceneResult !== "0x00000000" ||
      bgiBrowserEvidence.d3d9BeginSceneReturnAddress !== "0x1005b048" ||
      bgiBrowserEvidence.d3d9BeginSceneStackPointers?.join(",") !==
        "0x006ff260,0x006ff268" ||
      bgiBrowserEvidence.d3d9BeginSceneGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.d3d9BeginSceneDeviceReferenceCount !== 5 ||
      bgiBrowserEvidence.d3d9BeginSceneDeviceObjectRetired !== false ||
      bgiBrowserEvidence.d3d9BeginSceneCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9BeginSceneProfileId !==
        "primary-first-frame-scene-begin-v1" ||
      bgiBrowserEvidence.d3d9BeginSceneAuthority !==
        "rust-runtime-graphics-logical-device" ||
      bgiBrowserEvidence.d3d9BeginSceneLifecycle !== "inactive-to-active" ||
      bgiBrowserEvidence.d3d9BeginSceneMutation !==
        "validate-plan-preflight-targeted-commit" ||
      bgiBrowserEvidence.d3d9BeginSceneFailureAtomicity !== true ||
      bgiBrowserEvidence.d3d9BeginSceneLogicalGraphicsDeviceId !== 3 ||
      bgiBrowserEvidence.d3d9BeginSceneLogicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      bgiBrowserEvidence.d3d9BeginSceneLogicalGraphicsComObjectId !== "21" ||
      bgiBrowserEvidence.d3d9BeginScenePresentationGeometry?.join(",") !==
        "1280,720" ||
      bgiBrowserEvidence.d3d9BeginSceneBackBufferContentGeneration !== 1 ||
      bgiBrowserEvidence.d3d9BeginSceneBackBufferClearSequence !== 1 ||
      bgiBrowserEvidence.d3d9BeginSceneActiveSceneSequence !== 1 ||
      bgiBrowserEvidence.d3d9BeginSceneLogicalGraphicsDeviceGeneration !== 45 ||
      bgiBrowserEvidence.d3d9BeginSceneNestedBeginReturnsInvalidCall !== true ||
      bgiBrowserEvidence.d3d9BeginSceneEndSceneImplemented !== false ||
      bgiBrowserEvidence.d3d9BeginSceneBrowserRendererResourceCreated !== false ||
      bgiBrowserEvidence.d3d9BeginSceneNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("browser BGI D3D9 BeginScene evidence is invalid");
  }
  if (
    d3d9InitialSamplerStatesActive &&
    (bgiBrowserEvidence.d3d9SamplerStateCompletions !== 2 ||
      bgiBrowserEvidence.d3d9SamplerStateSequences?.join(",") !==
        "127360,127361" ||
      bgiBrowserEvidence.d3d9SamplerStateInstructionCounts?.join(",") !==
        "28761195,28761204" ||
      JSON.stringify(bgiBrowserEvidence.d3d9SamplerStateArguments) !==
        JSON.stringify([
          ["0x01010004", "0x00000000", "0x00000006", "0x00000002"],
          ["0x01010004", "0x00000000", "0x00000005", "0x00000002"],
        ]) ||
      bgiBrowserEvidence.d3d9SamplerStateResults?.join(",") !==
        "0x00000000,0x00000000" ||
      bgiBrowserEvidence.d3d9SamplerStateReturnAddresses?.join(",") !==
        "0x1005b18a,0x1005b1a0" ||
      JSON.stringify(bgiBrowserEvidence.d3d9SamplerStateStackPointers) !==
        JSON.stringify([
          ["0x006ff254", "0x006ff268"],
          ["0x006ff254", "0x006ff268"],
        ]) ||
      bgiBrowserEvidence.d3d9SamplerStateGuestWriteBytes?.join(",") !== "0,0" ||
      bgiBrowserEvidence.d3d9SamplerStateDeviceReferenceCounts?.join(",") !==
        "5,5" ||
      bgiBrowserEvidence.d3d9SamplerStateDeviceObjectsRetired?.join(",") !==
        "false,false" ||
      bgiBrowserEvidence.d3d9SamplerStateCompatibilityPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      bgiBrowserEvidence.d3d9SamplerStateProfileId !==
        "primary-first-frame-linear-sampler-states-v1" ||
      bgiBrowserEvidence.d3d9SamplerStateAuthority !==
        "rust-runtime-graphics-logical-device-sampler-states" ||
      bgiBrowserEvidence.d3d9SamplerStateMutation !==
        "validate-plan-preflight-targeted-idempotent-commit" ||
      bgiBrowserEvidence.d3d9SamplerStateLogicalGraphicsDeviceId !== 3 ||
      bgiBrowserEvidence.d3d9SamplerStateLogicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      bgiBrowserEvidence.d3d9SamplerStateLogicalGraphicsComObjectId !== "21" ||
      JSON.stringify(bgiBrowserEvidence.d3d9SamplerStateFiltersAfterCalls) !==
        JSON.stringify([
          { min: "linear", mag: null },
          { min: "linear", mag: "linear" },
        ]) ||
      bgiBrowserEvidence.d3d9SamplerStateLogicalGraphicsDeviceGenerations?.join(
        ",",
      ) !== "46,47" ||
      bgiBrowserEvidence.d3d9SamplerStateSameValueGenerationChange !== 0 ||
      bgiBrowserEvidence.d3d9SamplerStateUnsupportedVariantResult !==
        "0x8876086a" ||
      bgiBrowserEvidence.d3d9SamplerStateFailureAtomicity !== true ||
      bgiBrowserEvidence.d3d9SamplerStateBrowserRendererResourceCreated !==
        false ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedCompletions !== 3 ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedSequences?.join(",") !==
        "127362,127363,127364" ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedInstructionCounts?.join(
        ",",
      ) !== "28761212,28761220,28761228" ||
      JSON.stringify(
        bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedArguments,
      ) !==
        JSON.stringify([
          ["0x01010004", "0x000000ab", "0x00000001"],
          ["0x01010004", "0x00000014", "0x00000006"],
          ["0x01010004", "0x00000013", "0x00000005"],
        ]) ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedResults?.join(",") !==
        "0x8876086a,0x8876086a,0x8876086a" ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedReturnAddresses?.join(
        ",",
      ) !== "0x1005b1b7,0x1005b1cb,0x1005b1df" ||
      bgiBrowserEvidence.d3d9PostSamplerRenderStateRejectedCallerIgnoredResults !==
        true ||
      bgiBrowserEvidence.d3d9SamplerStateNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI D3D9 sampler-state evidence is invalid");
  }
  if (
    bgiBrowserEvidence.setWaitableTimerCompletions !== timerArmCount ||
    bgiBrowserEvidence.setWaitableTimerSequence !== 18_681 ||
    bgiBrowserEvidence.setWaitableTimerSequences?.join(",") !==
      timerSetSequences ||
    bgiBrowserEvidence.setWaitableTimerInstructionCounts?.join(",") !==
      timerSetInstructionCounts ||
    bgiBrowserEvidence.setWaitableTimerInstructionCount !==
      (createDirectoryAActive ? 11_974_074 : 11_973_553) ||
    bgiBrowserEvidence.setWaitableTimerArguments?.join(",") !==
      "0x57010002,0x500fff98,0x00000000,0x00000000,0x00000000,0x00000000" ||
    bgiBrowserEvidence.setWaitableTimerResult !== 1 ||
    bgiBrowserEvidence.setWaitableTimerReturnAddress !== "0x10050607" ||
    bgiBrowserEvidence.setWaitableTimerLastError !== 0 ||
    bgiBrowserEvidence.setWaitableTimerGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.setWaitableTimerImplementationId !==
      "kernel32.set-waitable-timer.relative-one-shot.v1" ||
    bgiBrowserEvidence.setWaitableTimerHandle !== "0x57010002" ||
    bgiBrowserEvidence.setWaitableTimerTimerId !== 2 ||
    bgiBrowserEvidence.setWaitableTimerManualReset !== true ||
    bgiBrowserEvidence.setWaitableTimerRelativeDueTime100ns !== 5_000 ||
    bgiBrowserEvidence.setWaitableTimerPeriodMilliseconds !== 0 ||
    bgiBrowserEvidence.setWaitableTimerArmGeneration !== 1 ||
    bgiBrowserEvidence.setWaitableTimerObjectId !== 24 ||
    bgiBrowserEvidence.setWaitableTimerObjectNamespace !==
      "vine.runtime-waitable-timer-object.v1" ||
    bgiBrowserEvidence.setWaitableTimerWaitState !== "nonsignaled" ||
    bgiBrowserEvidence.setWaitableTimerOpenHandles !== 1 ||
    bgiBrowserEvidence.setWaitableTimerStateAuthority !==
      "runtime-timers-and-typed-object-table" ||
    bgiBrowserEvidence.setWaitableTimerFailureAtomicity !== true ||
    bgiBrowserEvidence.setWaitableTimerClockAuthorityUsed !== true ||
    bgiBrowserEvidence.setWaitableTimerBrowserTimerCreated !== false ||
    bgiBrowserEvidence.setWaitableTimerExpirationImplemented !== true ||
    bgiBrowserEvidence.setWaitableTimerNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("browser BGI waitable-timer arm evidence is invalid");
  }
  if (
    bgiBrowserEvidence.waitForSingleObjectExCompletions !== timerCycleCount ||
    bgiBrowserEvidence.waitForSingleObjectExSequences?.join(",") !==
      timerWaitSequences ||
    bgiBrowserEvidence.waitForSingleObjectExInstructionCounts?.join(",") !==
      timerWaitInstructionCounts ||
    bgiBrowserEvidence.waitForSingleObjectExArguments?.join(",") !==
      "0x57010002,0x0000000a,0x00000001" ||
    bgiBrowserEvidence.waitForSingleObjectExResults?.join(",") !==
      timerRepeatedZeroes ||
    bgiBrowserEvidence.waitForSingleObjectExReturnAddress !== "0x10050615" ||
    bgiBrowserEvidence.waitForSingleObjectExLastErrors?.join(",") !==
      timerRepeatedZeroes ||
    bgiBrowserEvidence.waitForSingleObjectExGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.waitForSingleObjectExImplementationId !==
      "kernel32.wait-for-single-object-ex.waitable-timer.v1" ||
    bgiBrowserEvidence.waitForSingleObjectExTimerId !== 2 ||
    bgiBrowserEvidence.waitForSingleObjectExManualReset !== true ||
    bgiBrowserEvidence.waitForSingleObjectExArmGenerations?.join(",") !==
      timerGenerations ||
    bgiBrowserEvidence.waitForSingleObjectExDeadlineTimes100ns?.join(",") !==
      timerDeadlines ||
    bgiBrowserEvidence.waitForSingleObjectExClockTimes100ns?.join(",") !==
      timerDeadlines ||
    bgiBrowserEvidence.waitForSingleObjectExClockGenerations?.join(",") !==
      timerGenerations ||
    bgiBrowserEvidence.waitForSingleObjectExWaitState !== "signaled" ||
    bgiBrowserEvidence.waitForSingleObjectExNextRegistrationIds?.join(",") !==
      timerNextRegistrationIds ||
    bgiBrowserEvidence.waitForSingleObjectExStateAuthority !==
      "runtime-timers-scheduler-and-typed-object-table" ||
    bgiBrowserEvidence.waitForSingleObjectExClockAuthority !==
      "runtime-timers-logical-clock" ||
    bgiBrowserEvidence.waitForSingleObjectExTimeoutMode !==
      "finite-zero-through-ten-milliseconds" ||
    bgiBrowserEvidence.waitForSingleObjectExAlertableAccepted !== true ||
    bgiBrowserEvidence.waitForSingleObjectExApcDeliveryImplemented !== false ||
    bgiBrowserEvidence.waitForSingleObjectExPacingService !==
      "runtime.timer-delay.v1" ||
    bgiBrowserEvidence.waitForSingleObjectExPacingPayloadVersion !== 1 ||
    bgiBrowserEvidence.waitForSingleObjectExPacingRequestIds?.join(",") !==
      timerPacingRequestIds ||
    bgiBrowserEvidence.waitForSingleObjectExPacingRegistrationIds?.join(",") !==
      timerPacingRegistrationIds ||
    bgiBrowserEvidence.waitForSingleObjectExPacingDelayMilliseconds?.join(
      ",",
    ) !== timerRepeatedOnes ||
    bgiBrowserEvidence.waitForSingleObjectExPacingTimerIds?.join(",") !==
      timerRepeatedTwos ||
    bgiBrowserEvidence.waitForSingleObjectExPacingTargetTimes100ns?.join(
      ",",
    ) !== timerDeadlines ||
    bgiBrowserEvidence.waitForSingleObjectExPacingClockGenerations?.join(
      ",",
    ) !== timerPacingClockGenerations ||
    bgiBrowserEvidence.waitForSingleObjectExPacingArmGenerations?.join(",") !==
      timerGenerations ||
    bgiBrowserEvidence.waitForSingleObjectExPacingOutcome !==
      "object-signaled" ||
    bgiBrowserEvidence.waitForSingleObjectExNodePacingMode !==
      "deterministic-node-ack" ||
    bgiBrowserEvidence.waitForSingleObjectExLogicalCompletionAuthority !==
      "rust-runtime" ||
    bgiBrowserEvidence.waitForSingleObjectExPageLifecyclePolicy !==
      "freeze-resume-preserve-remaining-bounded-catch-up" ||
    bgiBrowserEvidence.waitForSingleObjectExRuntimeWidePacingBarrier !== true ||
    bgiBrowserEvidence.waitForSingleObjectExOwnerResumeAuthority !==
      "rust-host-completion" ||
    bgiBrowserEvidence.waitForSingleObjectExOtherGuestLaneExecutionDuringPacing !==
      false ||
    bgiBrowserEvidence.waitForSingleObjectExHostClockImported !== false ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserTimerCreated !== true ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserPacingRequests !==
      timerCycleCount ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserPacingCompletions !==
      timerCycleCount ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserPacingCancellations !== 0 ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserPacingStaleCallbacks !== 0 ||
    bgiBrowserEvidence.waitForSingleObjectExBrowserPacingPending !== 0 ||
    bgiBrowserEvidence.waitForSingleObjectExFailureAtomicity !== true ||
    bgiBrowserEvidence.waitForSingleObjectExNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("browser BGI waitable-timer wait evidence is invalid");
  }
  if (
    (dragAcceptFilesActive || archiveSourceReadActive) &&
    (bgiBrowserEvidence.dragAcceptFilesCompletions !== 1 ||
      bgiBrowserEvidence.dragAcceptFilesSequence !== 89_249 ||
      bgiBrowserEvidence.dragAcceptFilesInstructionCount !==
        (createDirectoryAActive ? 15_602_280 : 15_601_759) ||
      bgiBrowserEvidence.dragAcceptFilesArguments?.join(",") !==
        "0x48010001,0x00000000" ||
      bgiBrowserEvidence.dragAcceptFilesReturnAddress !== "0x1005f272" ||
      bgiBrowserEvidence.dragAcceptFilesResult !== "void-eax-preserved" ||
      bgiBrowserEvidence.dragAcceptFilesLastError !== 0 ||
      bgiBrowserEvidence.dragAcceptFilesGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.dragAcceptFilesImplementationId !==
        "shell32.drag-accept-files.logical-window-style-v1" ||
      bgiBrowserEvidence.dragAcceptFilesStateAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      bgiBrowserEvidence.dragAcceptFilesWindowId !== 1 ||
      bgiBrowserEvidence.dragAcceptFilesWindowHandleNamespace !==
        "vine.logical-window.v1" ||
      bgiBrowserEvidence.dragAcceptFilesWindowLifecycle !== "created" ||
      bgiBrowserEvidence.dragAcceptFilesEffectiveExtendedStyle !==
        "0x00040100" ||
      bgiBrowserEvidence.dragAcceptFilesAcceptsFiles !== false ||
      bgiBrowserEvidence.dragAcceptFilesMutation !==
        "owner-plan-stale-check-targeted-commit" ||
      bgiBrowserEvidence.dragAcceptFilesIdempotentUpdatePreservesGeneration !==
        true ||
      bgiBrowserEvidence.dragAcceptFilesFailureAtomicity !== true ||
      bgiBrowserEvidence.dragAcceptFilesBrowserOwnsLogicalWindowState !==
        false ||
      bgiBrowserEvidence.dragAcceptFilesBrowserResourceCreated !== false ||
      bgiBrowserEvidence.dragAcceptFilesWmDropFilesImplemented !== false ||
      bgiBrowserEvidence.dragAcceptFilesNodeCompletions !== 1 ||
      bgiBrowserEvidence.dragAcceptFilesChromeCompletions !== 1 ||
      bgiBrowserEvidence.dragAcceptFilesNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("browser BGI DragAcceptFiles evidence is invalid");
  }
  requireSha256(
    bgiBrowserEvidence.createThreadCreationDeltaPayloadSha256,
    "browserCheckpoint.bgiRuntimeEvidence.createThreadCreationDeltaPayloadSha256",
    errors,
  );
  if (
    bgiBrowserEvidence.createThreadCompletions !== 1 ||
    bgiBrowserEvidence.createThreadSequence !== 18_598 ||
    bgiBrowserEvidence.createThreadInstructionCount !== 11_945_954 ||
    bgiBrowserEvidence.createThreadArguments?.join(",") !==
      "0x00000000,0x00000000,0x100a1ee8,0x20139940,0x00000004,0x20139940" ||
    bgiBrowserEvidence.createThreadResult !== "0x54010002" ||
    bgiBrowserEvidence.createThreadReturnAddress !== "0x100a1fad" ||
    bgiBrowserEvidence.createThreadLastError !== 0 ||
    bgiBrowserEvidence.createThreadGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.createThreadOutputAddress !== "0x20139940" ||
    bgiBrowserEvidence.createThreadOutputHex !== "01200000" ||
    bgiBrowserEvidence.createThreadImplementationId !==
      "kernel32.create-thread.v1" ||
    bgiBrowserEvidence.createThreadHandleNamespace !==
      "vine.runtime-thread-object.v1" ||
    bgiBrowserEvidence.createThreadObjectId !== 25 ||
    bgiBrowserEvidence.createThreadId !== 8_193 ||
    bgiBrowserEvidence.createThreadStackAddressAllocationId !== 57 ||
    bgiBrowserEvidence.createThreadStackMemoryOwner !== 27 ||
    bgiBrowserEvidence.createThreadStackReserveBase !== "0x50000000" ||
    bgiBrowserEvidence.createThreadStackReserveBytes !== 1_048_576 ||
    bgiBrowserEvidence.createThreadStackCommitBase !== "0x500ff000" ||
    bgiBrowserEvidence.createThreadStackCommitBytes !== 4_096 ||
    bgiBrowserEvidence.createThreadStackGuardPage !== "0x500fe000" ||
    bgiBrowserEvidence.createThreadStackTop !== "0x50100000" ||
    bgiBrowserEvidence.createThreadInitialStackPointer !== "0x500ffff8" ||
    bgiBrowserEvidence.createThreadReturnSentinel !== "0x7f010010" ||
    bgiBrowserEvidence.createThreadEnvironmentAddressAllocationId !== 58 ||
    bgiBrowserEvidence.createThreadEnvironmentMemoryOwner !== 28 ||
    bgiBrowserEvidence.createThreadTebAddress !== "0x7f030000" ||
    bgiBrowserEvidence.createThreadPebAddress !== "0x7ffdf000" ||
    bgiBrowserEvidence.createThreadEntryPoint !== "0x100a1ee8" ||
    bgiBrowserEvidence.createThreadParameter !== "0x20139940" ||
    bgiBrowserEvidence.createThreadInitialSuspendCount !== 1 ||
    bgiBrowserEvidence.createThreadCreationDeltaFromGeneration !== 36_502 ||
    bgiBrowserEvidence.createThreadCreationDeltaGeneration !== 36_511 ||
    bgiBrowserEvidence.createThreadCreationDeltaMappingOperations !== 4 ||
    bgiBrowserEvidence.createThreadCreationDeltaDirtyPages !== 3 ||
    bgiBrowserEvidence.createThreadCreationDeltaControlBytes !== 264 ||
    bgiBrowserEvidence.createThreadCreationDeltaPayloadBytes !== 12_288 ||
    bgiBrowserEvidence.createThreadCreationDeltaPayloadSha256 !==
      "1e958a4c95b466754ec28a482c317c674175fb5ec5ce4a389406325684acb054" ||
    bgiBrowserEvidence.resumeThreadCompletions !== 1 ||
    bgiBrowserEvidence.resumeThreadSequence !== 18_599 ||
    bgiBrowserEvidence.resumeThreadInstructionCount !== 11_945_961 ||
    bgiBrowserEvidence.resumeThreadArguments?.join(",") !== "0x54010002" ||
    bgiBrowserEvidence.resumeThreadResult !== 1 ||
    bgiBrowserEvidence.resumeThreadReturnAddress !== "0x100a1fbd" ||
    bgiBrowserEvidence.resumeThreadLastError !== 0 ||
    bgiBrowserEvidence.resumeThreadGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.resumeThreadImplementationId !==
      "kernel32.resume-thread.v1" ||
    bgiBrowserEvidence.resumeThreadId !== 8_193 ||
    bgiBrowserEvidence.resumeThreadPreviousSuspendCount !== 1 ||
    bgiBrowserEvidence.resumeThreadFinalSchedulerState !== "runnable" ||
    bgiBrowserEvidence.guestThreadSchedulingPolicy !==
      "synchronous-resume-preserves-caller-until-runnable-contention-api-boundary-quantum-64-expires" ||
    bgiBrowserEvidence.guestThreadBrowserOwnsWindowsState !== false ||
    bgiBrowserEvidence.guestThreadNodeChromeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.guestThreadExecutedBeforeNextStop !== true
  ) {
    errors.push("browser BGI guest-thread bootstrap evidence is invalid");
  }
  if (
    bgiBrowserEvidence.threadPriorityCompletions !== 1 ||
    bgiBrowserEvidence.threadPrioritySequence !== 18_600 ||
    bgiBrowserEvidence.threadPriorityInstructionCount !== 11_945_977 ||
    bgiBrowserEvidence.threadPriorityArguments?.join(",") !==
      "0x54010002,0x00000001" ||
    bgiBrowserEvidence.threadPriorityResult !== 1 ||
    bgiBrowserEvidence.threadPriorityReturnAddress !== "0x10091b77" ||
    bgiBrowserEvidence.threadPriorityLastError !== 0 ||
    bgiBrowserEvidence.threadPriorityGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.threadPriorityImplementationId !==
      "kernel32.set-thread-priority.v1" ||
    bgiBrowserEvidence.threadPriorityHandle !== "0x54010002" ||
    bgiBrowserEvidence.threadPriorityThreadId !== 8_193 ||
    bgiBrowserEvidence.threadPriorityValue !== "above-normal" ||
    bgiBrowserEvidence.threadPriorityStateAuthority !==
      "runtime/thread.rs::RuntimeThreads" ||
    bgiBrowserEvidence.threadPriorityMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    bgiBrowserEvidence.threadPrioritySchedulerPolicy !==
      "logical-state-only-cooperative-selection-unchanged" ||
    bgiBrowserEvidence.threadPriorityHostSchedulerImported !== false ||
    bgiBrowserEvidence.threadPriorityBrowserSchedulerImported !== false ||
    bgiBrowserEvidence.threadPriorityFailureAtomicity !== true ||
    bgiBrowserEvidence.threadPriorityNodeCompletions !== 1 ||
    bgiBrowserEvidence.threadPriorityChromeCompletions !== 1 ||
    bgiBrowserEvidence.threadPriorityNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("browser BGI logical thread-priority evidence is invalid");
  }
  if (
    bgiBrowserEvidence.screenDcCompletions !== 1 ||
    bgiBrowserEvidence.screenDcSequence !== 18_608 ||
    bgiBrowserEvidence.screenDcInstructionCount !== 11_957_736 ||
    bgiBrowserEvidence.screenDcArguments?.join(",") !== "0x00000000" ||
    bgiBrowserEvidence.screenDcResult !== "0x47010003" ||
    bgiBrowserEvidence.screenDcReturnAddress !== "0x1002c155" ||
    bgiBrowserEvidence.screenDcLastError !== 0 ||
    bgiBrowserEvidence.screenDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.screenDcImplementationId !==
      "user32.get-dc.window-or-screen-acquisition.v2" ||
    bgiBrowserEvidence.screenDcDeviceContextId !== 3 ||
    bgiBrowserEvidence.screenDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.screenDcOwnerThreadId !== 8_192 ||
    bgiBrowserEvidence.screenDcTargetKind !== "screen" ||
    bgiBrowserEvidence.screenDcMonitorHandle !== "0x4f010001" ||
    bgiBrowserEvidence.screenDcRect?.join(",") !== "0,0,1920,1080" ||
    bgiBrowserEvidence.screenDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.screenDcSurfaceCreated !== false ||
    bgiBrowserEvidence.screenDcStateAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.screenDcDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    bgiBrowserEvidence.screenDcRelation !==
      "non-owning-primary-monitor-and-acquiring-thread" ||
    bgiBrowserEvidence.screenDcRendererResourceCreated !== false ||
    bgiBrowserEvidence.screenDcFailureAtomicity !== true ||
    bgiBrowserEvidence.screenDcNodeCompletions !== 1 ||
    bgiBrowserEvidence.screenDcChromeCompletions !== 1 ||
    bgiBrowserEvidence.screenDcNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push(
      "browser BGI primary-screen device-context evidence is invalid",
    );
  }
  if (
    bgiBrowserEvidence.fontEnumerationCompletions !== 1 ||
    bgiBrowserEvidence.fontEnumerationSequence !== 18_609 ||
    bgiBrowserEvidence.fontEnumerationInstructionCount !== 11_957_745 ||
    bgiBrowserEvidence.fontEnumerationArguments?.join(",") !==
      "0x47010003,0x006ffd00,0x1002d2e0,0x100dce7c,0x00000000" ||
    bgiBrowserEvidence.fontEnumerationResult !== 0 ||
    bgiBrowserEvidence.fontEnumerationReturnAddress !== "0x1002c16a" ||
    bgiBrowserEvidence.fontEnumerationLastError !== 0 ||
    bgiBrowserEvidence.fontEnumerationGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.fontEnumerationImplementationId !==
      "gdi32.enum-font-families-ex-a.cp932-screen.v1" ||
    bgiBrowserEvidence.fontEnumerationCallbackFrameId !== 20 ||
    bgiBrowserEvidence.fontEnumerationCallbackTarget !== "0x1002d2e0" ||
    bgiBrowserEvidence.fontEnumerationCallbackResult !== 0 ||
    bgiBrowserEvidence.fontEnumerationCallbackReturnInstructionCount !==
      11_957_981 ||
    bgiBrowserEvidence.fontEnumerationCallbackScratchBytes !== 284 ||
    bgiBrowserEvidence.fontEnumerationPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.fontEnumerationProfileId !==
      "primary-cp932-ms-gothic-regular-v1" ||
    bgiBrowserEvidence.fontEnumerationCharacterSet !== 128 ||
    bgiBrowserEvidence.fontEnumerationFaceWindows932Hex !==
      "826c827220835383568362834e" ||
    bgiBrowserEvidence.fontEnumerationFontType !== 4 ||
    bgiBrowserEvidence.fontEnumerationEnumLogFontExABytes !== 188 ||
    bgiBrowserEvidence.fontEnumerationNewTextMetricExABytes !== 96 ||
    bgiBrowserEvidence.fontEnumerationCatalogAuthority !==
      "rust-gdi32-compatibility-policy" ||
    bgiBrowserEvidence.fontEnumerationBrowserFontDiscoveryUsed !== false ||
    bgiBrowserEvidence.fontEnumerationBrowserFontResourceCreated !== false ||
    bgiBrowserEvidence.fontEnumerationNodeCompletions !== 1 ||
    bgiBrowserEvidence.fontEnumerationChromeCompletions !== 1 ||
    bgiBrowserEvidence.fontEnumerationNodeChromeOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.screenReleaseDcSequence !== 18_610 ||
    bgiBrowserEvidence.screenReleaseDcInstructionCount !== 11_957_987 ||
    bgiBrowserEvidence.screenReleaseDcArguments?.join(",") !==
      "0x00000000,0x47010003" ||
    bgiBrowserEvidence.screenReleaseDcResult !== 1 ||
    bgiBrowserEvidence.screenReleaseDcReturnAddress !== "0x1002c177" ||
    bgiBrowserEvidence.screenReleaseDcLastError !== 0 ||
    bgiBrowserEvidence.screenReleaseDcLiveDeviceContexts !== 0
  ) {
    errors.push("browser BGI bounded font-enumeration evidence is invalid");
  }
  if (
    bgiBrowserEvidence.fontCreationCompletions !== 1 ||
    bgiBrowserEvidence.fontCreationSequence !== 18_611 ||
    bgiBrowserEvidence.fontCreationInstructionCount !== 11_958_023 ||
    bgiBrowserEvidence.fontCreationArguments?.join(",") !==
      "0x00000040,0x00000020,0x00000000,0x00000000,0x00000064,0x00000000,0x00000000,0x00000000,0x00000080,0x00000004,0x00000000,0x00000002,0x00000000,0x100dce7c" ||
    bgiBrowserEvidence.fontCreationResult !== "0x47020002" ||
    bgiBrowserEvidence.fontCreationReturnAddress !== "0x1002c838" ||
    bgiBrowserEvidence.fontCreationLastError !== 0 ||
    bgiBrowserEvidence.fontCreationGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.fontCreationImplementationId !==
      "gdi32.create-font-a.cp932-logical-object.v1" ||
    bgiBrowserEvidence.fontCreationPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.fontCreationProfileId !==
      "primary-cp932-ms-gothic-64x32-thin-v1" ||
    bgiBrowserEvidence.fontCreationObjectId !== 2 ||
    bgiBrowserEvidence.fontCreationHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    bgiBrowserEvidence.fontCreationShared !== false ||
    bgiBrowserEvidence.fontCreationLogFontFields?.join(",") !==
      "64,32,0,0,100,0,0,0,128,4,0,2,0" ||
    bgiBrowserEvidence.fontCreationFaceWindows932Hex !==
      "826c827220835383568362834e" ||
    bgiBrowserEvidence.fontCreationLiveObjects !== 2 ||
    bgiBrowserEvidence.fontCreationStateAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.fontCreationMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    bgiBrowserEvidence.fontCreationFailureAtomicity !== true ||
    bgiBrowserEvidence.fontCreationBrowserFontResourceCreated !== false ||
    bgiBrowserEvidence.fontCreationNodeCompletions !== 1 ||
    bgiBrowserEvidence.fontCreationChromeCompletions !== 1 ||
    bgiBrowserEvidence.fontCreationNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("browser BGI logical font-creation evidence is invalid");
  }
  if (
    bgiBrowserEvidence.compatibleDcCompletions !== 1 ||
    bgiBrowserEvidence.compatibleDcSequence !== 18_613 ||
    bgiBrowserEvidence.compatibleDcInstructionCount !== 11_960_140 ||
    bgiBrowserEvidence.compatibleDcArguments?.join(",") !== "0x00000000" ||
    bgiBrowserEvidence.compatibleDcResult !== "0x47010004" ||
    bgiBrowserEvidence.compatibleDcReturnAddress !== "0x1002c8ad" ||
    bgiBrowserEvidence.compatibleDcLastError !== 0 ||
    bgiBrowserEvidence.compatibleDcGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.compatibleDcImplementationId !==
      "gdi32.create-compatible-dc.null-screen-memory.v1" ||
    bgiBrowserEvidence.compatibleDcPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.compatibleDcProfileId !==
      "primary-null-screen-memory-dc-v1" ||
    bgiBrowserEvidence.compatibleDcDeviceContextId !== 4 ||
    bgiBrowserEvidence.compatibleDcHandleNamespace !==
      "vine.logical-gdi-dc.v1" ||
    bgiBrowserEvidence.compatibleDcOwnerThreadId !== 8_192 ||
    bgiBrowserEvidence.compatibleDcTargetKind !== "memory" ||
    bgiBrowserEvidence.compatibleDcMonitorHandle !== "0x4f010001" ||
    bgiBrowserEvidence.compatibleDcRect?.join(",") !== "0,0,1920,1080" ||
    bgiBrowserEvidence.compatibleDcSelectedFontId !== 3 ||
    bgiBrowserEvidence.compatibleDcSelectedBitmapId !== 4 ||
    bgiBrowserEvidence.compatibleDcSelectionGeneration !== 0 ||
    bgiBrowserEvidence.compatibleDcInitialFontHandle !== "0x47020003" ||
    bgiBrowserEvidence.compatibleDcInitialFontStockIndex !== 13 ||
    bgiBrowserEvidence.compatibleDcInitialFontShared !== true ||
    bgiBrowserEvidence.compatibleDcInitialBitmapHandle !== "0x47020004" ||
    bgiBrowserEvidence.compatibleDcInitialBitmapStockIndex !== 21 ||
    bgiBrowserEvidence.compatibleDcInitialBitmapGeometry?.join(",") !==
      "1,1,1" ||
    bgiBrowserEvidence.compatibleDcInitialBitmapShared !== true ||
    bgiBrowserEvidence.compatibleDcLiveDeviceContexts !== 1 ||
    bgiBrowserEvidence.compatibleDcLiveObjects !== 4 ||
    bgiBrowserEvidence.compatibleDcStateAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.compatibleDcDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    bgiBrowserEvidence.compatibleDcRelation !==
      "non-owning-primary-monitor-and-creating-thread" ||
    bgiBrowserEvidence.compatibleDcMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    bgiBrowserEvidence.compatibleDcFailureAtomicity !== true ||
    bgiBrowserEvidence.compatibleDcBrowserResourceCreated !== false ||
    bgiBrowserEvidence.compatibleDcPixelStorageCreated !== false ||
    bgiBrowserEvidence.compatibleDcNodeCompletions !== 1 ||
    bgiBrowserEvidence.compatibleDcChromeCompletions !== 1 ||
    bgiBrowserEvidence.compatibleDcNodeChromeOrderedSemanticsMatched !== true ||
    bgiBrowserEvidence.deleteDcContractImplemented !== true ||
    bgiBrowserEvidence.deleteDcImplementationId !==
      "gdi32.delete-dc.logical-memory.v1" ||
    bgiBrowserEvidence.deleteDcReachedBeforeNextStop !== true ||
    bgiBrowserEvidence.deleteDcSequence !== 18_628 ||
    bgiBrowserEvidence.deleteDcInstructionCount !== 12_072_567 ||
    bgiBrowserEvidence.deleteDcResult !== 1 ||
    bgiBrowserEvidence.deleteDcReturnAddress !== "0x1002c472"
  ) {
    errors.push("browser BGI compatible memory-DC evidence is invalid");
  }
  if (
    bgiBrowserEvidence.dibCreateCompletions !== 1 ||
    bgiBrowserEvidence.dibCreateSequence !== 18_614 ||
    bgiBrowserEvidence.dibCreateInstructionCount !== 11_960_150 ||
    bgiBrowserEvidence.dibCreateArguments?.join(",") !==
      "0x47010004,0x2013a250,0x00000000,0x006ffef4,0x00000000,0x00000000" ||
    bgiBrowserEvidence.dibCreateResult !== "0x47020005" ||
    bgiBrowserEvidence.dibCreateReturnAddress !== "0x1002c8c5" ||
    bgiBrowserEvidence.dibCreateLastError !== 0 ||
    bgiBrowserEvidence.dibCreateGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.dibCreateOutputAddress !== "0x006ffef4" ||
    bgiBrowserEvidence.dibCreateOutputHex !== "00100044" ||
    bgiBrowserEvidence.dibCreateImplementationId !==
      "gdi32.create-dib-section.primary-8bpp.v1" ||
    bgiBrowserEvidence.dibCreatePolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.dibCreateProfileId !==
      "primary-128x-96-8bpp-reverse-grayscale-v1" ||
    bgiBrowserEvidence.dibObjectId !== 5 ||
    bgiBrowserEvidence.dibHandleNamespace !== "vine.logical-gdi-object.v1" ||
    bgiBrowserEvidence.dibShared !== false ||
    bgiBrowserEvidence.dibDescriptor?.join(",") !== "128,-96,1,8,0,128,12288" ||
    bgiBrowserEvidence.dibPaletteEntries !== 256 ||
    bgiBrowserEvidence.dibPaletteOrder !== "reverse-grayscale-rgbquad" ||
    bgiBrowserEvidence.dibBackingAddress !== "0x44001000" ||
    bgiBrowserEvidence.dibBackingBytes !== 12_288 ||
    bgiBrowserEvidence.dibBackingAddressAllocationId !== 59 ||
    bgiBrowserEvidence.dibBackingMemoryOwner !== 29 ||
    bgiBrowserEvidence.dibBackingMemoryRegionKind !== "gdi-resource" ||
    bgiBrowserEvidence.dibBackingAddressClassification !== "graphics" ||
    bgiBrowserEvidence.dibBackingAddressLifetime !==
      "object-explicit-release" ||
    bgiBrowserEvidence.dibCreateDeltaFromGeneration !== 36_537 ||
    bgiBrowserEvidence.dibCreateDeltaGeneration !== 36_540 ||
    bgiBrowserEvidence.dibCreateDeltaMappingOperations !== 2 ||
    bgiBrowserEvidence.dibCreateDeltaDirtyPages !== 4 ||
    bgiBrowserEvidence.dibCreateDeltaPayloadBytes !== 16_384 ||
    bgiBrowserEvidence.dibCreateDeltaPayloadSha256 !==
      "fcd36d346f9edd7098ec07a2d73a83319e75c08e9ac1c9ded494bf2470dcabe2" ||
    bgiBrowserEvidence.dibGetObjectCompletions !== 1 ||
    bgiBrowserEvidence.dibGetObjectSequence !== 18_616 ||
    bgiBrowserEvidence.dibGetObjectInstructionCount !== 11_960_180 ||
    bgiBrowserEvidence.dibGetObjectArguments?.join(",") !==
      "0x47020005,0x00000018,0x006ffd78" ||
    bgiBrowserEvidence.dibGetObjectResult !== 24 ||
    bgiBrowserEvidence.dibGetObjectReturnAddress !== "0x1002c8e7" ||
    bgiBrowserEvidence.dibGetObjectLastError !== 0 ||
    bgiBrowserEvidence.dibGetObjectGuestWriteBytes !== 24 ||
    bgiBrowserEvidence.dibGetObjectOutputHex !==
      "000000008000000060000000800000000100080000100044" ||
    bgiBrowserEvidence.dibGetObjectImplementationId !==
      "gdi32.get-object-a.font-or-dib.v1" ||
    bgiBrowserEvidence.dibSelectCompletions !== 4 ||
    bgiBrowserEvidence.dibSelectSequences?.join(",") !==
      "18617,18618,18624,18625" ||
    bgiBrowserEvidence.dibSelectInstructionCounts?.join(",") !==
      "11960192,11960199,12072548,12072554" ||
    bgiBrowserEvidence.dibSelectHandles?.join(",") !==
      "0x47020005,0x47020002,0x47020004,0x47020003" ||
    bgiBrowserEvidence.dibSelectPreviousHandles?.join(",") !==
      "0x47020004,0x47020003,0x47020005,0x47020002" ||
    bgiBrowserEvidence.dibSelectReturnAddresses?.join(",") !==
      "0x1002c90c,0x1002c922,0x1002c43d,0x1002c44d" ||
    bgiBrowserEvidence.dibSelectLastErrors?.join(",") !== "0,0,0,0" ||
    bgiBrowserEvidence.dibSelectGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.dibSelectImplementationId !==
      "gdi32.select-object.memory-font-bitmap.v1" ||
    bgiBrowserEvidence.dibSelectionGenerations?.join(",") !== "1,2,3,4" ||
    bgiBrowserEvidence.dibDeleteObjectContractImplemented !== true ||
    bgiBrowserEvidence.dibDeleteObjectImplementationId !==
      "gdi32.delete-object.font-or-dib.v1" ||
    bgiBrowserEvidence.dibDeleteObjectReachedBeforeNextStop !== true ||
    bgiBrowserEvidence.dibDeleteObjectSequence !== 18_626 ||
    bgiBrowserEvidence.dibDeleteObjectInstructionCount !== 12_072_559 ||
    bgiBrowserEvidence.dibDeleteObjectResult !== 1 ||
    bgiBrowserEvidence.dibDeleteObjectReturnAddress !== "0x1002c45c" ||
    bgiBrowserEvidence.fontDeleteObjectSequence !== 18_627 ||
    bgiBrowserEvidence.fontDeleteObjectInstructionCount !== 12_072_563 ||
    bgiBrowserEvidence.fontDeleteObjectResult !== 1 ||
    bgiBrowserEvidence.fontDeleteObjectReturnAddress !== "0x1002c465" ||
    bgiBrowserEvidence.dibObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.dibByteAuthority !== "rust-guest-memory" ||
    bgiBrowserEvidence.dibAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    bgiBrowserEvidence.dibMutation !==
      "address-gdi-memory-preflight-targeted-commit" ||
    bgiBrowserEvidence.dibFailureAtomicity !== true ||
    bgiBrowserEvidence.dibBrowserBitmapResourceCreated !== false ||
    bgiBrowserEvidence.dibNodeCompletions !== 7 ||
    bgiBrowserEvidence.dibChromeCompletions !== 7 ||
    bgiBrowserEvidence.dibNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("browser BGI DIB-section lifecycle evidence is invalid");
  }
  if (
    bgiBrowserEvidence.outlineTextMetricsCompletions !== 2 ||
    bgiBrowserEvidence.outlineTextMetricsSequences?.join(",") !==
      "18619,18621" ||
    bgiBrowserEvidence.outlineTextMetricsInstructionCounts?.join(",") !==
      "11960243,11960296" ||
    JSON.stringify(bgiBrowserEvidence.outlineTextMetricsArguments) !==
      JSON.stringify([
        ["0x47010004", "0x00000000", "0x00000000"],
        ["0x47010004", "0x0000010d", "0x2013a250"],
      ]) ||
    bgiBrowserEvidence.outlineTextMetricsResults?.join(",") !== "269,269" ||
    bgiBrowserEvidence.outlineTextMetricsReturnAddresses?.join(",") !==
      "0x1002c51f,0x1002c539" ||
    bgiBrowserEvidence.outlineTextMetricsLastErrors?.join(",") !== "0,0" ||
    bgiBrowserEvidence.outlineTextMetricsGuestWriteBytes?.join(",") !==
      "0,269" ||
    bgiBrowserEvidence.outlineTextMetricsOutputAddress !== "0x2013a250" ||
    bgiBrowserEvidence.outlineTextMetricsOutputBytes !== 269 ||
    bgiBrowserEvidence.outlineTextMetricsOutputSha256 !==
      "d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d" ||
    bgiBrowserEvidence.outlineTextMetricsImplementationId !==
      "gdi32.get-outline-text-metrics-a.cp932-selected-font.v1" ||
    bgiBrowserEvidence.outlineTextMetricsPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.outlineTextMetricsProfileId !==
      "primary-cp932-ms-gothic-outline-metrics-v1" ||
    bgiBrowserEvidence.outlineTextMetricsDeviceContext !== "0x47010004" ||
    bgiBrowserEvidence.outlineTextMetricsDeviceContextId !== 4 ||
    bgiBrowserEvidence.outlineTextMetricsSelectedFont !== "0x47020002" ||
    bgiBrowserEvidence.outlineTextMetricsSelectedFontId !== 2 ||
    bgiBrowserEvidence.outlineTextMetricsSelectionGeneration !== 2 ||
    bgiBrowserEvidence.outlineTextMetricsMetricsAuthority !==
      "gdi32-compatibility-policy" ||
    bgiBrowserEvidence.outlineTextMetricsSelectedFontAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.outlineTextMetricsByteAuthority !==
      "rust-guest-memory" ||
    bgiBrowserEvidence.outlineTextMetricsNativePaddingPolicy !==
      "zero-four-nondeterministic-padding-bytes" ||
    bgiBrowserEvidence.outlineTextMetricsHostFontMeasurementImported !==
      false ||
    bgiBrowserEvidence.outlineTextMetricsBrowserFontMeasurementImported !==
      false ||
    bgiBrowserEvidence.outlineTextMetricsNodeCompletions !== 2 ||
    bgiBrowserEvidence.outlineTextMetricsChromeCompletions !== 2 ||
    bgiBrowserEvidence.outlineTextMetricsNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("browser BGI outline-text-metrics evidence is invalid");
  }
  if (
    bgiBrowserEvidence.textRasterCompletions !== 1 ||
    bgiBrowserEvidence.textRasterSequence !== 18_623 ||
    bgiBrowserEvidence.textRasterInstructionCount !== 11_961_619 ||
    bgiBrowserEvidence.textRasterArguments?.join(",") !==
      "0x47010004,0x00000000,0x00000000,0x006ffd6c,0x00000001" ||
    bgiBrowserEvidence.textRasterResult !== 1 ||
    bgiBrowserEvidence.textRasterReturnAddress !== "0x1002cab1" ||
    bgiBrowserEvidence.textRasterLastError !== 0 ||
    bgiBrowserEvidence.textRasterGuestWriteAddress !== "0x44001a00" ||
    bgiBrowserEvidence.textRasterGuestWriteBytes !== 4_384 ||
    bgiBrowserEvidence.textRasterChangedBytes !== 476 ||
    bgiBrowserEvidence.textRasterImplementationId !==
      "gdi32.text-out-a.cp932-8bpp-raster.v1" ||
    bgiBrowserEvidence.textRasterPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    bgiBrowserEvidence.textRasterProfileId !==
      "primary-cp932-ms-gothic-a-8bpp-raster-v1" ||
    bgiBrowserEvidence.textRasterDeviceContextId !== 4 ||
    bgiBrowserEvidence.textRasterSelectedFontId !== 2 ||
    bgiBrowserEvidence.textRasterSelectedDibId !== 5 ||
    bgiBrowserEvidence.textRasterSelectionGeneration !== 2 ||
    bgiBrowserEvidence.textRasterContentGeneration !== 1 ||
    bgiBrowserEvidence.textRasterInitialPixelPlaneFnv1a32 !== "0x52965dc5" ||
    bgiBrowserEvidence.textRasterOutputPixelPlaneFnv1a32 !== "0xe6fc8461" ||
    bgiBrowserEvidence.textRasterObjectAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    bgiBrowserEvidence.textRasterByteAuthority !== "rust-guest-memory" ||
    bgiBrowserEvidence.textRasterMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    bgiBrowserEvidence.textRasterFailureAtomicity !== true ||
    bgiBrowserEvidence.textRasterHostFontRasterizationImported !== false ||
    bgiBrowserEvidence.textRasterBrowserFontRasterizationImported !== false ||
    bgiBrowserEvidence.textRasterBrowserBitmapResourceCreated !== false ||
    bgiBrowserEvidence.textRasterNodeCompletions !== 1 ||
    bgiBrowserEvidence.textRasterChromeCompletions !== 1 ||
    bgiBrowserEvidence.textRasterNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("browser BGI bounded TextOutA raster evidence is invalid");
  }
  if (
    bgiBrowserEvidence.gdiplusStartupCompletions !== 1 ||
    bgiBrowserEvidence.gdiplusStartupSequence !== 18_638 ||
    bgiBrowserEvidence.gdiplusStartupInstructionCount !== 12_073_229 ||
    bgiBrowserEvidence.gdiplusStartupArguments?.join(",") !==
      "0x10151f8c,0x006fff44,0x00000000" ||
    bgiBrowserEvidence.gdiplusStartupResult !== 0 ||
    bgiBrowserEvidence.gdiplusStartupReturnAddress !== "0x100652c8" ||
    bgiBrowserEvidence.gdiplusStartupLastError !== 0 ||
    bgiBrowserEvidence.gdiplusStartupGuestWriteAddress !== "0x10151f8c" ||
    bgiBrowserEvidence.gdiplusStartupGuestWriteBytes !== 4 ||
    bgiBrowserEvidence.gdiplusStartupGuestWriteHex !== "01005047" ||
    bgiBrowserEvidence.gdiplusStartupImplementationId !==
      "gdiplus.startup.primary-v1" ||
    bgiBrowserEvidence.gdiplusStartupPolicyId !==
      "gdiplus-primary-startup-v1" ||
    bgiBrowserEvidence.gdiplusStartupProfileId !==
      "primary-x86-callback-free-v1" ||
    bgiBrowserEvidence.gdiplusStartupSessionId !== 1 ||
    bgiBrowserEvidence.gdiplusStartupTokenNamespace !==
      "vine.logical-gdiplus-session-token.v1" ||
    bgiBrowserEvidence.gdiplusStartupToken !== "0x47500001" ||
    bgiBrowserEvidence.gdiplusStartupTokenId !== 1 ||
    bgiBrowserEvidence.gdiplusStartupSessionGeneration !== 1 ||
    bgiBrowserEvidence.gdiplusStartupLiveSessions !== 1 ||
    bgiBrowserEvidence.gdiplusStartupPolicyAuthority !==
      "rust-gdiplus-compatibility-policy" ||
    bgiBrowserEvidence.gdiplusStartupSessionAuthority !==
      "runtime-gdiplus-logical-session" ||
    bgiBrowserEvidence.gdiplusStartupMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    bgiBrowserEvidence.gdiplusStartupFailureAtomicity !== true ||
    bgiBrowserEvidence.gdiplusStartupNativeTokenImported !== false ||
    bgiBrowserEvidence.gdiplusStartupBrowserResourceCreated !== false ||
    bgiBrowserEvidence.gdiplusStartupImageCodecImported !== false ||
    bgiBrowserEvidence.gdiplusStartupNodeCompletions !== 1 ||
    bgiBrowserEvidence.gdiplusStartupChromeCompletions !== 1 ||
    bgiBrowserEvidence.gdiplusStartupNodeChromeOrderedSemanticsMatched !==
      true ||
    bgiBrowserEvidence.gdiplusShutdownContractImplemented !== true ||
    bgiBrowserEvidence.gdiplusShutdownReachedBeforeNextStop !== false ||
    bgiBrowserEvidence.gdiplusProcessExitRetiresSessions !== true
  ) {
    errors.push("browser BGI GDI+ startup session evidence is invalid");
  }
  if (
    bgiBrowserEvidence.virtualAllocCompletions !== 1 ||
    bgiBrowserEvidence.virtualAllocSequence !== 18_641 ||
    bgiBrowserEvidence.virtualAllocInstructionCount !== 12_073_370 ||
    bgiBrowserEvidence.virtualAllocArguments?.join(",") !==
      "0x00000000,0x06000000,0x00102000,0x00000004" ||
    bgiBrowserEvidence.virtualAllocResult !== "0x79000000" ||
    bgiBrowserEvidence.virtualAllocReturnAddress !== "0x1008fba8" ||
    bgiBrowserEvidence.virtualAllocLastError !== 0 ||
    bgiBrowserEvidence.virtualAllocGuestWriteBytes !== 0 ||
    bgiBrowserEvidence.virtualAllocImplementationId !==
      "kernel32.virtual-alloc.reserve-top-down.v1" ||
    bgiBrowserEvidence.virtualAllocPolicyId !==
      "kernel32-virtual-alloc-reserve-v1" ||
    bgiBrowserEvidence.virtualAllocAddressAllocationId !== 60 ||
    bgiBrowserEvidence.virtualAllocMemoryOwner !== 30 ||
    bgiBrowserEvidence.virtualAllocAddressBase !== "0x79000000" ||
    bgiBrowserEvidence.virtualAllocReservedBytes !== 96 * 1024 * 1024 ||
    bgiBrowserEvidence.virtualAllocRegionKind !== "anonymous" ||
    bgiBrowserEvidence.virtualAllocReserveOnly !== true ||
    bgiBrowserEvidence.virtualAllocTopDown !== true ||
    bgiBrowserEvidence.virtualAllocPolicyAuthority !==
      "rust-kernel32-compatibility-policy" ||
    bgiBrowserEvidence.virtualAllocAddressAuthority !==
      "runtime-process-address-space" ||
    bgiBrowserEvidence.virtualAllocByteAuthority !== "rust-guest-memory" ||
    bgiBrowserEvidence.virtualAllocMutation !==
      "cross-owner-preflight-transactional-commit" ||
    bgiBrowserEvidence.virtualAllocFailureAtomicity !== true ||
    bgiBrowserEvidence.virtualAllocHostAddressImported !== false ||
    bgiBrowserEvidence.virtualAllocBrowserAddressImported !== false ||
    bgiBrowserEvidence.virtualAllocNodeCompletions !== 1 ||
    bgiBrowserEvidence.virtualAllocChromeCompletions !== 1 ||
    bgiBrowserEvidence.virtualAllocNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("browser BGI VirtualAlloc reserve evidence is invalid");
  }
  const bgiBenchmarkEvidence = requireRecord(
    browser.bgiBenchmarkEvidence,
    "browserCheckpoint.bgiBenchmarkEvidence",
    errors,
  );
  requireString(
    bgiBenchmarkEvidence.chromeVersion,
    "browserCheckpoint.bgiBenchmarkEvidence.chromeVersion",
    errors,
  );
  for (const field of [
    "performanceClassification",
    "decisionStatus",
    "decisionReason",
    "runtimeProfile",
    "observationStatus",
    "chromeArtifactId",
    "environmentPolicy",
    "environmentMonitor",
    "environmentStatus",
  ]) {
    requireString(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  const benchmarkFailureIds = requireStringArray(
    bgiBenchmarkEvidence.thresholdFailureIds,
    "browserCheckpoint.bgiBenchmarkEvidence.thresholdFailureIds",
    errors,
  );
  for (const field of [
    "schemaVersion",
    "durationMilliseconds",
    "completedSessions",
    "failedSessions",
    "mainHeapSamples",
    "workerHeapSamples",
    "mainHeapMaximumBytes",
    "workerHeapMaximumBytes",
    "workerHeapCapacityMaximumBytes",
    "workerBackingStorageMaximumBytes",
    "wasmBridgeCallsPerSession",
    "runtimeCoreLinearMemoryBytes",
    "executionEngineLinearMemoryBytes",
    "thresholdFailures",
    "thresholdEvaluationCount",
    "environmentSamples",
    "environmentObservationSchemaVersion",
    "environmentMonitorBytes",
    "externalProcessAttributionLimit",
    "updaterEvents",
    "sustainedEnvironmentExcursions",
    "qosChangesObserved",
    "qosChangesAfterWarmup",
    "runtimeWorkerBytes",
    "benchmarkMainBytes",
    "runtimeCoreWasmBytes",
    "v86AdapterWasmBytes",
    "reportBytes",
  ]) {
    requireInteger(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  for (const field of [
    "sessionP95Milliseconds",
    "sliceP99Milliseconds",
    "heartbeatP99Milliseconds",
    "maximumHeartbeatGapMilliseconds",
    "mainHeapSlopeBytesPerMinute",
    "workerHeapSlopeBytesPerMinute",
    "workerBackingStorageSlopeBytesPerMinute",
  ]) {
    requireFiniteNumber(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  for (const field of [
    "configurationSha256",
    "chromeManifestSha256",
    "chromeArchiveSha256",
    "chromeExecutableSha256",
    "environmentPolicySha256",
    "environmentMonitorSha256",
    "runtimeCoreWasmSha256",
    "runtimeWorkerSha256",
    "benchmarkMainSha256",
    "v86AdapterWasmSha256",
    "reportSha256",
  ]) {
    requireSha256(
      bgiBenchmarkEvidence[field],
      `browserCheckpoint.bgiBenchmarkEvidence.${field}`,
      errors,
    );
  }
  const benchmarkEnvironmentReasonIds = requireStringArray(
    bgiBenchmarkEvidence.environmentReasonIds,
    "browserCheckpoint.bgiBenchmarkEvidence.environmentReasonIds",
    errors,
  );
  const benchmarkEnvironmentBlocked =
    browser.bgiBenchmark === "environment-blocked";
  if (
    bgiBenchmarkEvidence.schema !== "vine.browser-bgi-benchmark-report" ||
    bgiBenchmarkEvidence.schemaVersion !== 3 ||
    bgiBenchmarkEvidence.configuration !==
      (archiveSourceReadActive
        ? "fixtures/benchmarks/bgi-browser-archive-source-read-v2.json"
        : dragAcceptFilesActive
          ? "fixtures/benchmarks/bgi-browser-drag-accept-files-v1.json"
          : "fixtures/benchmarks/bgi-browser-waitable-timer-pacing-v1.json") ||
    bgiBenchmarkEvidence.runtimeProfile !==
      (sleepZeroActive
        ? "kernel32-archive-source-read"
        : root.activeRuntimeCheckpoint?.profile) ||
    bgiBenchmarkEvidence.observationStatus !==
      profileExpectations.benchmarkObservation ||
    bgiBenchmarkEvidence.performanceClassification !==
      "repeated-distribution-threshold" ||
    bgiBenchmarkEvidence.chromeArtifactId !==
      "chrome-for-testing-151.0.7922.34-win64" ||
    bgiBenchmarkEvidence.environmentPolicy !==
      "fixtures/benchmarks/browser-performance-environment-windows-v1.json" ||
    bgiBenchmarkEvidence.environmentMonitor !==
      "tools/windows/monitor-browser-performance-environment.ps1" ||
    bgiBenchmarkEvidence.environmentObservationSchemaVersion !== 2 ||
    bgiBenchmarkEvidence.environmentMonitorBytes !== 21_398 ||
    bgiBenchmarkEvidence.environmentMonitorSha256 !==
      "1d846304ef1590e564b3c82f44c14c3e53ce51db48457bd5eadf6c24cee7c94d" ||
    bgiBenchmarkEvidence.externalProcessAttributionLimit !== 8 ||
    (benchmarkEnvironmentBlocked
      ? bgiBenchmarkEvidence.environmentStatus !== "invalid-environment" ||
        benchmarkEnvironmentReasonIds.length === 0
      : bgiBenchmarkEvidence.environmentStatus !== "valid" ||
        benchmarkEnvironmentReasonIds.length !== 0) ||
    bgiBenchmarkEvidence.updaterEvents !== 0 ||
    (benchmarkEnvironmentBlocked
      ? bgiBenchmarkEvidence.sustainedEnvironmentExcursions < 1
      : bgiBenchmarkEvidence.sustainedEnvironmentExcursions !== 0) ||
    bgiBenchmarkEvidence.qosChangesAfterWarmup !== 0 ||
    bgiBenchmarkEvidence.decisionStatus !==
      (benchmarkFailureIds.length === 0 ? "passed" : "failed") ||
    bgiBenchmarkEvidence.decisionReason !==
      (benchmarkFailureIds.length === 0
        ? `all-${bgiBenchmarkEvidence.thresholdEvaluationCount}-versioned-thresholds-passed`
        : `${benchmarkFailureIds.length}-of-${bgiBenchmarkEvidence.thresholdEvaluationCount}-versioned-thresholds-failed`) ||
    bgiBenchmarkEvidence.thresholdEvaluationCount < 1 ||
    bgiBenchmarkEvidence.completedSessions < 1 ||
    bgiBenchmarkEvidence.failedSessions !== 0 ||
    bgiBenchmarkEvidence.mainHeapSamples < 2 ||
    bgiBenchmarkEvidence.workerHeapSamples < 2 ||
    bgiBenchmarkEvidence.thresholdFailures !== benchmarkFailureIds.length ||
    (browser.bgiBenchmark === "threshold-passed" &&
      benchmarkFailureIds.length !== 0) ||
    (browser.bgiBenchmark === "threshold-blocked" &&
      benchmarkFailureIds.length === 0) ||
    (benchmarkEnvironmentBlocked && benchmarkFailureIds.length !== 0) ||
    bgiBenchmarkEvidence.runtimeCoreWasmBytes !==
      (archiveSourceReadActive
        ? 4_560_864
        : dragAcceptFilesActive
          ? 4_574_225
          : 4_565_781) ||
    bgiBenchmarkEvidence.runtimeCoreWasmSha256 !==
      (archiveSourceReadActive
        ? "8a1c9775762a1e794f220582098b23411955e309efc875419a19af90feccbdad"
        : dragAcceptFilesActive
          ? "55c01dbcf3e4a88bf13051924fe61c94f28c9b4801308e77a2b00544cec1e9d1"
          : "db194f8abaf482facc8fdfa18b561dd568dbcb04469ec5818470ed2dc18456c5") ||
    bgiBenchmarkEvidence.executionEngineLinearMemoryBytes !==
      bgiBrowserEvidence.executionEngineLinearMemoryBytes
  ) {
    errors.push("browser BGI benchmark evidence is invalid or inconsistent");
  }
  if (
    browser.bgiBenchmark === "threshold-passed" &&
    browser.bgiBenchmarkBlocker !== undefined
  ) {
    errors.push(
      "passed browser benchmark must not retain a structured performance blocker",
    );
  }
  if (benchmarkEnvironmentBlocked) {
    const benchmarkBlocker = requireRecord(
      browser.bgiBenchmarkBlocker,
      "browserCheckpoint.bgiBenchmarkBlocker",
      errors,
    );
    for (const field of [
      "schema",
      "id",
      "kind",
      "reason",
      "status",
      "blockedWork",
      "exactReproduction",
      "requiredAuthority",
      "resumeCondition",
    ]) {
      requireString(
        benchmarkBlocker[field],
        `browserCheckpoint.bgiBenchmarkBlocker.${field}`,
        errors,
      );
    }
    requireInteger(
      benchmarkBlocker.schemaVersion,
      "browserCheckpoint.bgiBenchmarkBlocker.schemaVersion",
      errors,
    );
    const firstEnvironmentFailure = requireRecord(
      benchmarkBlocker.firstFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure",
      errors,
    );
    const confirmationEnvironmentFailure = requireRecord(
      benchmarkBlocker.confirmationFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure",
      errors,
    );
    for (const [label, failure] of [
      ["firstFailure", firstEnvironmentFailure],
      ["confirmationFailure", confirmationEnvironmentFailure],
    ]) {
      requireString(
        failure.reasonId,
        `browserCheckpoint.bgiBenchmarkBlocker.${label}.reasonId`,
        errors,
      );
      for (const field of [
        "observedConsecutiveSamples",
        "requiredConsecutiveSamples",
      ]) {
        requireInteger(
          failure[field],
          `browserCheckpoint.bgiBenchmarkBlocker.${label}.${field}`,
          errors,
        );
      }
      for (const field of ["thresholdPercent", "maximumObservedPercent"]) {
        requireFiniteNumber(
          failure[field],
          `browserCheckpoint.bgiBenchmarkBlocker.${label}.${field}`,
          errors,
        );
      }
    }
    const benchmarkBlockerEvidence = requireRecord(
      benchmarkBlocker.evidence,
      "browserCheckpoint.bgiBenchmarkBlocker.evidence",
      errors,
    );
    for (const field of [
      "firstBenchmarkReport",
      "confirmationBenchmarkReport",
      "firstEnvironmentStatus",
      "confirmationEnvironmentStatus",
      "host",
      "cpu",
      "chromeVersion",
      "nodeVersion",
    ]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportBytes",
      "firstEnvironmentSamples",
      "firstCompletedSessions",
      "firstFailedSessions",
      "firstThresholdEvaluations",
      "firstThresholdFailures",
      "firstRuntimeCoreLinearMemoryBytes",
      "confirmationBenchmarkReportBytes",
      "confirmationEnvironmentSamples",
      "confirmationCompletedSessions",
      "confirmationFailedSessions",
      "confirmationThresholdEvaluations",
      "confirmationThresholdFailures",
      "confirmationRuntimeCoreLinearMemoryBytes",
      "environmentUpdaterEvents",
      "environmentSustainedExcursions",
      "environmentQosChangesAfterWarmup",
      "logicalCpus",
    ]) {
      requireInteger(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstTotalCpuMeanPercent",
      "firstTotalCpuMaximumPercent",
      "firstExternalCpuMeanPercent",
      "firstExternalCpuMaximumPercent",
      "confirmationTotalCpuMeanPercent",
      "confirmationTotalCpuMaximumPercent",
      "confirmationExternalCpuMeanPercent",
      "confirmationExternalCpuMaximumPercent",
    ]) {
      requireFiniteNumber(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportSha256",
      "confirmationBenchmarkReportSha256",
    ]) {
      requireSha256(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    const attemptedOptions = requireStringArray(
      benchmarkBlocker.attemptedOptions,
      "browserCheckpoint.bgiBenchmarkBlocker.attemptedOptions",
      errors,
    );
    const protectedInvariants = requireStringArray(
      benchmarkBlocker.protectedInvariants,
      "browserCheckpoint.bgiBenchmarkBlocker.protectedInvariants",
      errors,
    );
    if (
      benchmarkBlocker.schema !== "vine.structured-blocker" ||
      benchmarkBlocker.schemaVersion !== 4 ||
      benchmarkBlocker.kind !== "performance-environment-blocker" ||
      benchmarkBlocker.status !== "blocked" ||
      benchmarkBlocker.exactReproduction !==
        "npm run check:runtime:bgi-browser-benchmark:prepared" ||
      firstEnvironmentFailure.reasonId !== benchmarkEnvironmentReasonIds[0] ||
      confirmationEnvironmentFailure.reasonId !==
        benchmarkEnvironmentReasonIds[0] ||
      firstEnvironmentFailure.observedConsecutiveSamples <
        firstEnvironmentFailure.requiredConsecutiveSamples ||
      confirmationEnvironmentFailure.observedConsecutiveSamples <
        confirmationEnvironmentFailure.requiredConsecutiveSamples ||
      firstEnvironmentFailure.requiredConsecutiveSamples !== 3 ||
      confirmationEnvironmentFailure.requiredConsecutiveSamples !== 3 ||
      firstEnvironmentFailure.thresholdPercent !== 12.5 ||
      confirmationEnvironmentFailure.thresholdPercent !== 12.5 ||
      firstEnvironmentFailure.maximumObservedPercent <=
        firstEnvironmentFailure.thresholdPercent ||
      confirmationEnvironmentFailure.maximumObservedPercent <=
        confirmationEnvironmentFailure.thresholdPercent ||
      !benchmarkBlockerEvidence.firstBenchmarkReport.startsWith(
        ".work/browser-runtime/",
      ) ||
      !benchmarkBlockerEvidence.confirmationBenchmarkReport.startsWith(
        ".work/browser-runtime/",
      ) ||
      benchmarkBlockerEvidence.firstEnvironmentStatus !==
        "invalid-environment" ||
      benchmarkBlockerEvidence.confirmationEnvironmentStatus !==
        bgiBenchmarkEvidence.environmentStatus ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportBytes !==
        bgiBenchmarkEvidence.reportBytes ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportSha256 !==
        bgiBenchmarkEvidence.reportSha256 ||
      benchmarkBlockerEvidence.confirmationEnvironmentSamples !==
        bgiBenchmarkEvidence.environmentSamples ||
      benchmarkBlockerEvidence.confirmationCompletedSessions !==
        bgiBenchmarkEvidence.completedSessions ||
      benchmarkBlockerEvidence.confirmationFailedSessions !==
        bgiBenchmarkEvidence.failedSessions ||
      benchmarkBlockerEvidence.confirmationThresholdEvaluations !==
        bgiBenchmarkEvidence.thresholdEvaluationCount ||
      benchmarkBlockerEvidence.confirmationThresholdFailures !==
        bgiBenchmarkEvidence.thresholdFailures ||
      benchmarkBlockerEvidence.confirmationRuntimeCoreLinearMemoryBytes !==
        bgiBenchmarkEvidence.runtimeCoreLinearMemoryBytes ||
      benchmarkBlockerEvidence.environmentUpdaterEvents !==
        bgiBenchmarkEvidence.updaterEvents ||
      benchmarkBlockerEvidence.environmentSustainedExcursions !==
        bgiBenchmarkEvidence.sustainedEnvironmentExcursions ||
      benchmarkBlockerEvidence.environmentQosChangesAfterWarmup !==
        bgiBenchmarkEvidence.qosChangesAfterWarmup ||
      benchmarkBlockerEvidence.chromeVersion !==
        bgiBenchmarkEvidence.chromeVersion ||
      attemptedOptions.length < 5 ||
      protectedInvariants.length < 5 ||
      !benchmarkBlocker.resumeCondition.includes("unchanged 180-second gate") ||
      !benchmarkBlocker.resumeCondition.includes(
        root.nextStructuredStop?.nextCheckpoint ?? "",
      )
    ) {
      errors.push(
        "browser BGI performance-environment blocker is invalid or incomplete",
      );
    }
  } else if (
    (dragAcceptFilesActive || archiveSourceReadActive) &&
    (browser.bgiBenchmark === "threshold-blocked" ||
      browser.bgiBenchmarkBlocker !== undefined)
  ) {
    const benchmarkBlocker = requireRecord(
      browser.bgiBenchmarkBlocker,
      "browserCheckpoint.bgiBenchmarkBlocker",
      errors,
    );
    for (const field of [
      "schema",
      "id",
      "kind",
      "reason",
      "status",
      "blockedWork",
      "exactReproduction",
      "requiredAuthority",
      "resumeCondition",
    ]) {
      requireString(
        benchmarkBlocker[field],
        `browserCheckpoint.bgiBenchmarkBlocker.${field}`,
        errors,
      );
    }
    requireInteger(
      benchmarkBlocker.schemaVersion,
      "browserCheckpoint.bgiBenchmarkBlocker.schemaVersion",
      errors,
    );
    const minimumSessionFailure = requireRecord(
      benchmarkBlocker.firstFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure",
      errors,
    );
    const sessionLatencyFailure = requireRecord(
      benchmarkBlocker.confirmationFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure",
      errors,
    );
    for (const [label, failure] of [
      ["firstFailure", minimumSessionFailure],
      ["confirmationFailure", sessionLatencyFailure],
    ]) {
      requireString(
        failure.thresholdId,
        `browserCheckpoint.bgiBenchmarkBlocker.${label}.thresholdId`,
        errors,
      );
      requireString(
        failure.comparison,
        `browserCheckpoint.bgiBenchmarkBlocker.${label}.comparison`,
        errors,
      );
      requireFiniteNumber(
        failure.observed,
        `browserCheckpoint.bgiBenchmarkBlocker.${label}.observed`,
        errors,
      );
      requireFiniteNumber(
        failure.threshold,
        `browserCheckpoint.bgiBenchmarkBlocker.${label}.threshold`,
        errors,
      );
    }
    const benchmarkBlockerEvidence = requireRecord(
      benchmarkBlocker.evidence,
      "browserCheckpoint.bgiBenchmarkBlocker.evidence",
      errors,
    );
    for (const field of [
      "benchmarkReport",
      "invalidEnvironmentReport",
      "invalidEnvironmentReason",
      "host",
      "cpu",
      "chromeVersion",
      "nodeVersion",
    ]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "benchmarkReportBytes",
      "environmentSamples",
      "passingThresholds",
      "thresholdEvaluations",
      "completedSessions",
      "failedSessions",
      "heartbeatSamples",
      "mainHeapSamples",
      "workerHeapSamples",
      "runtimeCoreLinearMemoryBytes",
      "executionEngineLinearMemoryBytes",
      "wasmBridgeCallsPerSession",
      "invalidEnvironmentReportBytes",
      "invalidEnvironmentCompletedSessions",
      "logicalCpus",
      "environmentUpdaterEvents",
      "environmentSustainedExcursions",
      "environmentQosChangesAfterWarmup",
    ]) {
      requireInteger(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "sessionP95Milliseconds",
      "sliceP99Milliseconds",
      "heartbeatP99Milliseconds",
      "maximumHeartbeatGapMilliseconds",
      "totalCpuMeanPercent",
      "totalCpuMaximumPercent",
      "externalCpuMeanPercent",
      "externalCpuMaximumPercent",
      "initialCandidateP95Milliseconds",
      "baselineBrowserElapsedMilliseconds",
      "optimizedBrowserElapsedMilliseconds",
      "zeroDelayDiagnosticElapsedMilliseconds",
      "invalidEnvironmentP95Milliseconds",
    ]) {
      requireFiniteNumber(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "benchmarkReportSha256",
      "invalidEnvironmentReportSha256",
    ]) {
      requireSha256(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    const attemptedOptions = requireStringArray(
      benchmarkBlocker.attemptedOptions,
      "browserCheckpoint.bgiBenchmarkBlocker.attemptedOptions",
      errors,
    );
    const protectedInvariants = requireStringArray(
      benchmarkBlocker.protectedInvariants,
      "browserCheckpoint.bgiBenchmarkBlocker.protectedInvariants",
      errors,
    );
    if (
      browser.bgiBenchmark !== "threshold-blocked" ||
      benchmarkBlocker.schema !== "vine.structured-blocker" ||
      benchmarkBlocker.schemaVersion !== 5 ||
      benchmarkBlocker.id !==
        (archiveSourceReadActive
          ? "kernel32-archive-source-read-browser-throughput-threshold-v1"
          : "shell32-drag-accept-files-browser-throughput-threshold-v1") ||
      benchmarkBlocker.kind !== "feasibility-blocker" ||
      benchmarkBlocker.reason !==
        "browser-session-throughput-and-p95-latency-exceed-versioned-threshold-after-bounded-timer-pacing-remediation" ||
      benchmarkBlocker.status !== "blocked" ||
      benchmarkBlocker.blockedWork !==
        (archiveSourceReadActive
          ? "accept-kernel32-archive-source-read-and-advance-to-user32-set-window-pos-resize"
          : "accept-shell32-drag-accept-files-and-advance-to-user32-message-box-a") ||
      benchmarkBlocker.exactReproduction !==
        "npm run check:runtime:bgi-browser-benchmark:prepared" ||
      benchmarkFailureIds.join(",") !==
        "minimum-completed-sessions,maximum-p95-session-milliseconds" ||
      minimumSessionFailure.thresholdId !== benchmarkFailureIds[0] ||
      minimumSessionFailure.observed !==
        bgiBenchmarkEvidence.completedSessions ||
      minimumSessionFailure.comparison !== ">=" ||
      minimumSessionFailure.threshold !== 24 ||
      sessionLatencyFailure.thresholdId !== benchmarkFailureIds[1] ||
      sessionLatencyFailure.observed !==
        bgiBenchmarkEvidence.sessionP95Milliseconds ||
      sessionLatencyFailure.comparison !== "<=" ||
      sessionLatencyFailure.threshold !== 8_000 ||
      benchmarkBlockerEvidence.benchmarkReport !==
        (archiveSourceReadActive
          ? ".work/browser-runtime/bgi-browser-benchmark-report-archive-source-read-valid-threshold-failure-r1.json"
          : ".work/browser-runtime/bgi-browser-benchmark-report-drag-accept-files-message-task-valid-threshold-failure-r2.json") ||
      benchmarkBlockerEvidence.benchmarkReportBytes !==
        bgiBenchmarkEvidence.reportBytes ||
      benchmarkBlockerEvidence.benchmarkReportSha256 !==
        bgiBenchmarkEvidence.reportSha256 ||
      benchmarkBlockerEvidence.environmentSamples !==
        bgiBenchmarkEvidence.environmentSamples ||
      benchmarkBlockerEvidence.passingThresholds !==
        bgiBenchmarkEvidence.thresholdEvaluationCount -
          bgiBenchmarkEvidence.thresholdFailures ||
      benchmarkBlockerEvidence.thresholdEvaluations !==
        bgiBenchmarkEvidence.thresholdEvaluationCount ||
      benchmarkBlockerEvidence.completedSessions !==
        bgiBenchmarkEvidence.completedSessions ||
      benchmarkBlockerEvidence.failedSessions !==
        bgiBenchmarkEvidence.failedSessions ||
      benchmarkBlockerEvidence.sessionP95Milliseconds !==
        bgiBenchmarkEvidence.sessionP95Milliseconds ||
      benchmarkBlockerEvidence.sliceP99Milliseconds !==
        bgiBenchmarkEvidence.sliceP99Milliseconds ||
      benchmarkBlockerEvidence.heartbeatP99Milliseconds !==
        bgiBenchmarkEvidence.heartbeatP99Milliseconds ||
      benchmarkBlockerEvidence.maximumHeartbeatGapMilliseconds !==
        bgiBenchmarkEvidence.maximumHeartbeatGapMilliseconds ||
      benchmarkBlockerEvidence.mainHeapSamples !==
        bgiBenchmarkEvidence.mainHeapSamples ||
      benchmarkBlockerEvidence.workerHeapSamples !==
        bgiBenchmarkEvidence.workerHeapSamples ||
      benchmarkBlockerEvidence.runtimeCoreLinearMemoryBytes !==
        bgiBenchmarkEvidence.runtimeCoreLinearMemoryBytes ||
      benchmarkBlockerEvidence.executionEngineLinearMemoryBytes !==
        bgiBenchmarkEvidence.executionEngineLinearMemoryBytes ||
      benchmarkBlockerEvidence.wasmBridgeCallsPerSession !==
        bgiBenchmarkEvidence.wasmBridgeCallsPerSession ||
      benchmarkBlockerEvidence.invalidEnvironmentReport !==
        ".work/browser-runtime/bgi-browser-benchmark-report-drag-accept-files-message-task-invalid-environment-r1.json" ||
      benchmarkBlockerEvidence.invalidEnvironmentReportBytes !== 2_272_012 ||
      benchmarkBlockerEvidence.invalidEnvironmentReportSha256 !==
        "8ecd55a4d132e53169fce5767b085d3bbbc7ca11033e9acd7b8cd509cc225605" ||
      benchmarkBlockerEvidence.invalidEnvironmentReason !==
        "sustained-external-cpu" ||
      benchmarkBlockerEvidence.invalidEnvironmentCompletedSessions !== 12 ||
      benchmarkBlockerEvidence.invalidEnvironmentP95Milliseconds !==
        16_552.305000007153 ||
      benchmarkBlockerEvidence.initialCandidateP95Milliseconds !== 77_652.04 ||
      benchmarkBlockerEvidence.baselineBrowserElapsedMilliseconds !==
        73_204.855 ||
      benchmarkBlockerEvidence.optimizedBrowserElapsedMilliseconds !==
        bgiBrowserEvidence.elapsedMilliseconds ||
      benchmarkBlockerEvidence.zeroDelayDiagnosticElapsedMilliseconds !==
        8_856.53 ||
      benchmarkBlockerEvidence.environmentUpdaterEvents !== 0 ||
      benchmarkBlockerEvidence.environmentSustainedExcursions !== 0 ||
      benchmarkBlockerEvidence.environmentQosChangesAfterWarmup !== 0 ||
      benchmarkBlockerEvidence.host !== "win32 10.0.26200 x64" ||
      benchmarkBlockerEvidence.cpu !==
        "AMD Ryzen 7 8840U w/ Radeon 780M Graphics" ||
      benchmarkBlockerEvidence.logicalCpus !== 16 ||
      benchmarkBlockerEvidence.chromeVersion !==
        bgiBenchmarkEvidence.chromeVersion ||
      benchmarkBlockerEvidence.nodeVersion !== "v24.18.0" ||
      attemptedOptions.length < 6 ||
      protectedInvariants.length < 5 ||
      benchmarkBlocker.requiredAuthority !==
        "profiled-correctness-preserving-rust-wasm-dispatch-fast-path-or-versioned-performance-contract-decision" ||
      !benchmarkBlocker.resumeCondition.includes("unchanged 180-second gate") ||
      !benchmarkBlocker.resumeCondition.includes(
        archiveSourceReadActive ? "SetWindowPos" : "MessageBoxA",
      )
    ) {
      errors.push(
        "browser BGI DragAcceptFiles structured performance blocker is invalid",
      );
    }
  } else if (
    browser.bgiBenchmark === "threshold-blocked" ||
    browser.bgiBenchmarkBlocker !== undefined
  ) {
    const benchmarkBlocker = requireRecord(
      browser.bgiBenchmarkBlocker,
      "browserCheckpoint.bgiBenchmarkBlocker",
      errors,
    );
    for (const field of [
      "schema",
      "id",
      "kind",
      "reason",
      "status",
      "blockedWork",
      "exactReproduction",
      "requiredAuthority",
      "resumeCondition",
    ]) {
      requireString(
        benchmarkBlocker[field],
        `browserCheckpoint.bgiBenchmarkBlocker.${field}`,
        errors,
      );
    }
    requireInteger(
      benchmarkBlocker.schemaVersion,
      "browserCheckpoint.bgiBenchmarkBlocker.schemaVersion",
      errors,
    );
    const firstBenchmarkFailure = requireRecord(
      benchmarkBlocker.firstFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure",
      errors,
    );
    requireString(
      firstBenchmarkFailure.thresholdId,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.thresholdId",
      errors,
    );
    requireString(
      firstBenchmarkFailure.comparison,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.comparison",
      errors,
    );
    requireFiniteNumber(
      firstBenchmarkFailure.observed,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.observed",
      errors,
    );
    requireFiniteNumber(
      firstBenchmarkFailure.threshold,
      "browserCheckpoint.bgiBenchmarkBlocker.firstFailure.threshold",
      errors,
    );
    const confirmationBenchmarkFailure = requireRecord(
      benchmarkBlocker.confirmationFailure,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure",
      errors,
    );
    requireString(
      confirmationBenchmarkFailure.thresholdId,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.thresholdId",
      errors,
    );
    requireString(
      confirmationBenchmarkFailure.comparison,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.comparison",
      errors,
    );
    requireFiniteNumber(
      confirmationBenchmarkFailure.observed,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.observed",
      errors,
    );
    requireFiniteNumber(
      confirmationBenchmarkFailure.threshold,
      "browserCheckpoint.bgiBenchmarkBlocker.confirmationFailure.threshold",
      errors,
    );
    const benchmarkBlockerEvidence = requireRecord(
      benchmarkBlocker.evidence,
      "browserCheckpoint.bgiBenchmarkBlocker.evidence",
      errors,
    );
    for (const field of [
      "firstBenchmarkReport",
      "confirmationBenchmarkReport",
      "nodeComparisonReport",
      "firstEnvironmentStatus",
      "confirmationEnvironmentStatus",
    ]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportBytes",
      "firstHeartbeatSamples",
      "firstHeartbeatSamplesOverThreshold",
      "firstHeartbeatSamplesOverThirtyMilliseconds",
      "firstEnvironmentSamples",
      "confirmationBenchmarkReportBytes",
      "passingThresholds",
      "thresholdEvaluations",
      "confirmationCompletedSessions",
      "confirmationFailedSessions",
      "confirmationHeartbeatSamples",
      "confirmationHeartbeatSamplesOverThreshold",
      "confirmationHeartbeatSamplesOverThirtyMilliseconds",
      "confirmationEnvironmentSamples",
      "environmentUpdaterEvents",
      "environmentSustainedExcursions",
      "environmentQosChangesAfterWarmup",
      "nodeComparisonReportBytes",
      "logicalCpus",
    ]) {
      requireInteger(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstHeartbeatP99Milliseconds",
      "firstMaximumHeartbeatGapMilliseconds",
      "firstTotalCpuMeanPercent",
      "firstTotalCpuMaximumPercent",
      "firstExternalCpuMeanPercent",
      "firstExternalCpuMaximumPercent",
      "confirmationHeartbeatP99Milliseconds",
      "confirmationMaximumHeartbeatGapMilliseconds",
      "confirmationTotalCpuMeanPercent",
      "confirmationTotalCpuMaximumPercent",
      "confirmationExternalCpuMeanPercent",
      "confirmationExternalCpuMaximumPercent",
      "queryPerformanceFrequencyMedianMilliseconds",
      "getMonitorInfoAMedianMilliseconds",
      "nodeMedianIncreasePercent",
    ]) {
      requireFiniteNumber(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of [
      "firstBenchmarkReportSha256",
      "confirmationBenchmarkReportSha256",
      "nodeComparisonReportSha256",
    ]) {
      requireSha256(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    for (const field of ["host", "cpu", "chromeVersion", "nodeVersion"]) {
      requireString(
        benchmarkBlockerEvidence[field],
        `browserCheckpoint.bgiBenchmarkBlocker.evidence.${field}`,
        errors,
      );
    }
    const attemptedOptions = requireStringArray(
      benchmarkBlocker.attemptedOptions,
      "browserCheckpoint.bgiBenchmarkBlocker.attemptedOptions",
      errors,
    );
    const protectedInvariants = requireStringArray(
      benchmarkBlocker.protectedInvariants,
      "browserCheckpoint.bgiBenchmarkBlocker.protectedInvariants",
      errors,
    );
    if (
      browser.bgiBenchmark !== "threshold-blocked" ||
      benchmarkBlocker.schema !== "vine.structured-blocker" ||
      benchmarkBlocker.schemaVersion !== 3 ||
      benchmarkBlocker.id !==
        "user32-get-monitor-info-a-heartbeat-threshold-v1" ||
      benchmarkBlocker.kind !== "feasibility-blocker" ||
      benchmarkBlocker.reason !==
        "browser-heartbeat-p99-repeats-above-versioned-threshold-in-valid-environment-without-node-cost-regression" ||
      benchmarkBlocker.status !== "blocked" ||
      benchmarkBlocker.blockedWork !==
        "accept-user32-get-monitor-info-a-and-advance-to-direct-input-8-create" ||
      benchmarkBlocker.exactReproduction !==
        "npm run check:runtime:bgi-browser-benchmark:prepared" ||
      firstBenchmarkFailure.thresholdId !== benchmarkFailureIds[0] ||
      firstBenchmarkFailure.observed !== 33.480000004172325 ||
      firstBenchmarkFailure.comparison !== "<=" ||
      firstBenchmarkFailure.threshold !== 25 ||
      confirmationBenchmarkFailure.thresholdId !== benchmarkFailureIds[0] ||
      confirmationBenchmarkFailure.observed !==
        bgiBenchmarkEvidence.heartbeatP99Milliseconds ||
      confirmationBenchmarkFailure.comparison !== "<=" ||
      confirmationBenchmarkFailure.threshold !== 25 ||
      benchmarkBlockerEvidence.firstBenchmarkReport !==
        ".work/browser-runtime/bgi-browser-get-monitor-info-a-first-failed-report.json" ||
      benchmarkBlockerEvidence.firstBenchmarkReportBytes !== 1_644_990 ||
      benchmarkBlockerEvidence.firstBenchmarkReportSha256 !==
        "5b4618c71bf6978a2a72f3e945c89dbc745d9548c981db5be09190d35cd9b468" ||
      benchmarkBlockerEvidence.firstHeartbeatSamples !== 21_643 ||
      benchmarkBlockerEvidence.firstHeartbeatSamplesOverThreshold !== 645 ||
      benchmarkBlockerEvidence.firstHeartbeatSamplesOverThirtyMilliseconds !==
        457 ||
      benchmarkBlockerEvidence.firstHeartbeatP99Milliseconds !==
        firstBenchmarkFailure.observed ||
      benchmarkBlockerEvidence.firstMaximumHeartbeatGapMilliseconds !==
        43.98999999463558 ||
      benchmarkBlockerEvidence.firstEnvironmentStatus !== "valid" ||
      benchmarkBlockerEvidence.firstEnvironmentSamples !== 178 ||
      benchmarkBlockerEvidence.firstTotalCpuMeanPercent !== 12.85375774390497 ||
      benchmarkBlockerEvidence.firstTotalCpuMaximumPercent !==
        31.875537768580987 ||
      benchmarkBlockerEvidence.firstExternalCpuMeanPercent !==
        6.168668629453616 ||
      benchmarkBlockerEvidence.firstExternalCpuMaximumPercent !==
        25.02570510532766 ||
      benchmarkBlockerEvidence.confirmationBenchmarkReport !==
        ".work/browser-runtime/bgi-browser-get-monitor-info-a-confirmation-failed-report.json" ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportBytes !==
        bgiBenchmarkEvidence.reportBytes ||
      benchmarkBlockerEvidence.confirmationBenchmarkReportSha256 !==
        bgiBenchmarkEvidence.reportSha256 ||
      benchmarkBlockerEvidence.passingThresholds !==
        bgiBenchmarkEvidence.thresholdEvaluationCount -
          bgiBenchmarkEvidence.thresholdFailures ||
      benchmarkBlockerEvidence.thresholdEvaluations !==
        bgiBenchmarkEvidence.thresholdEvaluationCount ||
      benchmarkBlockerEvidence.confirmationCompletedSessions !==
        bgiBenchmarkEvidence.completedSessions ||
      benchmarkBlockerEvidence.confirmationFailedSessions !==
        bgiBenchmarkEvidence.failedSessions ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamples !== 21_371 ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamplesOverThreshold !==
        657 ||
      benchmarkBlockerEvidence.confirmationHeartbeatSamplesOverThirtyMilliseconds !==
        488 ||
      benchmarkBlockerEvidence.confirmationHeartbeatP99Milliseconds !==
        bgiBenchmarkEvidence.heartbeatP99Milliseconds ||
      benchmarkBlockerEvidence.confirmationMaximumHeartbeatGapMilliseconds !==
        bgiBenchmarkEvidence.maximumHeartbeatGapMilliseconds ||
      benchmarkBlockerEvidence.confirmationEnvironmentStatus !== "valid" ||
      benchmarkBlockerEvidence.confirmationEnvironmentSamples !== 178 ||
      benchmarkBlockerEvidence.confirmationTotalCpuMeanPercent !==
        12.860080347033424 ||
      benchmarkBlockerEvidence.confirmationTotalCpuMaximumPercent !==
        23.85210915316297 ||
      benchmarkBlockerEvidence.confirmationExternalCpuMeanPercent !==
        6.388341606491718 ||
      benchmarkBlockerEvidence.confirmationExternalCpuMaximumPercent !==
        17.59194120952698 ||
      benchmarkBlockerEvidence.environmentUpdaterEvents !== 0 ||
      benchmarkBlockerEvidence.environmentSustainedExcursions !== 0 ||
      benchmarkBlockerEvidence.environmentQosChangesAfterWarmup !== 0 ||
      benchmarkBlockerEvidence.nodeComparisonReport !==
        ".work/hyperfine-query-performance-frequency-get-monitor-info-a-v1.json" ||
      benchmarkBlockerEvidence.nodeComparisonReportBytes !== 1_059 ||
      benchmarkBlockerEvidence.nodeComparisonReportSha256 !==
        "b4716f50ed7c495405154af8d5c2d71f0045414bcbefd5c6db1f7938be0ea19c" ||
      benchmarkBlockerEvidence.queryPerformanceFrequencyMedianMilliseconds !==
        27_192.872861999997 ||
      benchmarkBlockerEvidence.getMonitorInfoAMedianMilliseconds !==
        26_989.811961999996 ||
      benchmarkBlockerEvidence.nodeMedianIncreasePercent !==
        -0.7467430934219643 ||
      benchmarkBlockerEvidence.host !== "win32 10.0.26200 x64" ||
      benchmarkBlockerEvidence.cpu !==
        "AMD Ryzen 7 8840U w/ Radeon 780M Graphics" ||
      benchmarkBlockerEvidence.logicalCpus !== 16 ||
      benchmarkBlockerEvidence.chromeVersion !==
        bgiBenchmarkEvidence.chromeVersion ||
      benchmarkBlockerEvidence.nodeVersion !== "v24.18.0" ||
      attemptedOptions.length < 6 ||
      protectedInvariants.length < 5 ||
      benchmarkBlocker.requiredAuthority !==
        "isolated-or-quiescent-browser-host-or-profiled-browser-scheduling-remediation-or-versioned-performance-contract-decision" ||
      !benchmarkBlocker.resumeCondition.includes("unchanged 180-second gate") ||
      !benchmarkBlocker.resumeCondition.includes("DirectInput8Create")
    ) {
      errors.push("browser BGI structured performance blocker is invalid");
    }
  }

  const engine = requireRecord(
    root.engineDecisionEvidence,
    "engineDecisionEvidence",
    errors,
  );
  const engineGate = requireRecord(
    engine.v86EngineGate,
    "engineDecisionEvidence.v86EngineGate",
    errors,
  );
  if (!new Set(["go", "no-go"]).has(engineGate.decision)) {
    errors.push("engineDecisionEvidence.v86EngineGate.decision is invalid");
  }
  requireInteger(
    engineGate.thresholdFailures,
    "engineDecisionEvidence.v86EngineGate.thresholdFailures",
    errors,
  );
  if (
    engineGate.performanceClassification !==
      "repeated-distribution-threshold" ||
    engineGate.reportHashClassification !== "artifact-identity-only" ||
    engineGate.decisionReason !==
      (engineGate.thresholdFailures === 0
        ? "all-versioned-thresholds-passed"
        : `${engineGate.thresholdFailures}-versioned-thresholds-failed`) ||
    (engineGate.decision === "go" && engineGate.thresholdFailures !== 0) ||
    (engineGate.decision === "no-go" && engineGate.thresholdFailures === 0)
  ) {
    errors.push(
      "engineDecisionEvidence.v86EngineGate classification is invalid",
    );
  }
  requireSha256(
    engineGate.reportSha256,
    "engineDecisionEvidence.v86EngineGate.reportSha256",
    errors,
  );

  const active = requireRecord(
    root.activeRuntimeCheckpoint,
    "activeRuntimeCheckpoint",
    errors,
  );
  requireString(
    active.runtimeKernelCheckpoint,
    "activeRuntimeCheckpoint.runtimeKernelCheckpoint",
    errors,
  );
  requireString(active.profile, "activeRuntimeCheckpoint.profile", errors);
  const expectedActiveStatus =
    root.nextStructuredStop?.kind === "capability-checkpoint"
      ? "checkpoint-complete"
      : "structured-stop";
  if (active.status !== expectedActiveStatus) {
    errors.push(
      `activeRuntimeCheckpoint.status must be ${expectedActiveStatus}`,
    );
  }
  if (active.runtimeStateAuthority !== "rust") {
    errors.push("activeRuntimeCheckpoint.runtimeStateAuthority must be rust");
  }
  if (
    active.runtimeWindowActivationScope !==
    (setWindowPosRepeatActivationActive
      ? "same-owner-thread-single-window-same-geometry-initial-and-repeat-active-focus-foreground-callback-lifecycle"
      : "same-owner-thread-single-window-same-geometry-active-focus-foreground-callback-lifecycle")
  ) {
    errors.push("activeRuntimeCheckpoint window activation scope is invalid");
  }
  const APIs = requireStringArray(
    active.genericDispatcherApis,
    "activeRuntimeCheckpoint.genericDispatcherApis",
    errors,
  );
  requireInteger(
    active.genericDispatcherApiCount,
    "activeRuntimeCheckpoint.genericDispatcherApiCount",
    errors,
  );
  if (active.genericDispatcherApiCount !== APIs.length) {
    errors.push(
      "activeRuntimeCheckpoint.genericDispatcherApiCount must match genericDispatcherApis",
    );
  }
  requireInteger(
    active.genericDispatcherContractCount,
    "activeRuntimeCheckpoint.genericDispatcherContractCount",
    errors,
  );
  if (
    active.genericDispatcherContractCount < active.genericDispatcherApiCount
  ) {
    errors.push(
      "activeRuntimeCheckpoint genericDispatcherContractCount must cover every genericDispatcherApi",
    );
  }
  requireInteger(
    active.genericDispatcherCallOccurrenceCount,
    "activeRuntimeCheckpoint.genericDispatcherCallOccurrenceCount",
    errors,
  );
  if (
    active.genericDispatcherCallOccurrenceCount <
    active.genericDispatcherContractCount
  ) {
    errors.push(
      "activeRuntimeCheckpoint call occurrences must cover every API contract",
    );
  }
  if (active.allSupportedCallsUseGenericDispatcher !== true) {
    errors.push(
      "activeRuntimeCheckpoint.allSupportedCallsUseGenericDispatcher must be true",
    );
  }
  const kernelBoundary = requireRecord(
    active.runtimeKernelBoundary,
    "activeRuntimeCheckpoint.runtimeKernelBoundary",
    errors,
  );
  const kernelRoles = requireStringArray(
    kernelBoundary.aggregateRole,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.aggregateRole",
    errors,
  );
  const kernelFields = requireStringArray(
    kernelBoundary.directFields,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.directFields",
    errors,
  );
  const processIndependentOwners = requireStringArray(
    kernelBoundary.processIndependentSubsystemOwners,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.processIndependentSubsystemOwners",
    errors,
  );
  const representativeSubsystemTests = requireStringArray(
    kernelBoundary.representativeSubsystemTests,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.representativeSubsystemTests",
    errors,
  );
  const representativeAtomicTests = requireStringArray(
    kernelBoundary.representativeCrossSubsystemAtomicTests,
    "activeRuntimeCheckpoint.runtimeKernelBoundary.representativeCrossSubsystemAtomicTests",
    errors,
  );
  if (
    kernelBoundary.schema !== "vine.runtime-kernel-boundary.v1" ||
    kernelBoundary.checkpoint !== "S13" ||
    kernelBoundary.publicFacade !== "runtime.rs" ||
    kernelBoundary.aggregateType !== "runtime/kernel.rs::RuntimeKernel" ||
    kernelBoundary.compatibilityAlias !== "RuntimeState" ||
    kernelBoundary.compatibilityAliasKind !== "type-alias" ||
    kernelRoles.join(",") !==
      "private-subsystem-ownership,cross-subsystem-transaction-coordination,compatibility-serialization" ||
    kernelFields.join(",") !==
      "process-state,process-address-space-allocator,threads,scheduler-and-waits,messages,displays,input,user32,gdi,gdiplus,graphics,audio,timers,objects,com,heaps,files,module-records,synchronization,modules,registries" ||
    kernelBoundary.directMutableCollectionFields !== 0 ||
    kernelBoundary.directScalarAllocators !== 0 ||
    kernelBoundary.directSubsystemCollectionMutation !== false ||
    kernelBoundary.subsystemMutableCollectionVisibility !== "private" ||
    kernelBoundary.processStateOwner !==
      "runtime/process.rs::RuntimeProcessState" ||
    kernelBoundary.processMutableCollectionVisibility !== "private" ||
    processIndependentOwners.join(",") !==
      "displays,input,user32,gdi,gdiplus,graphics,audio,timers,heaps,files,module-records,synchronization,com" ||
    kernelBoundary.processSerializationView !==
      "runtime/kernel.rs::RuntimeProcessView" ||
    kernelBoundary.processSerializationViewAccess !== "immutable-borrow" ||
    kernelBoundary.processLifecyclePlansRejectStaleCommit !== true ||
    kernelBoundary.closeHandleObjectPlan !==
      "runtime/objects.rs::RuntimeCloseHandleObjectMutationPlan" ||
    kernelBoundary.closeHandleObjectPlanVisibility !== "opaque-to-kernel" ||
    kernelBoundary.payloadObjectSubsystemRootCloneOrReplacementFromKernel !==
      false ||
    representativeSubsystemTests
      .filter(
        (test) =>
          !test.startsWith("runtime::gdi::") &&
          !test.startsWith("runtime::gdiplus::") &&
          !test.startsWith("runtime::graphics::") &&
          !test.startsWith("runtime::audio::"),
      )
      .join(",") !==
      "runtime::displays::tests::configuration_is_single_owner_and_rejects_invalid_geometry_without_mutation,runtime::input::tests::configuration_and_swaps_are_planned_and_generation_tracks_only_changes,runtime::user32::tests::shared_icon_plans_reuse_identity_and_teardown_atomically,runtime::user32::tests::shared_cursor_identity_and_cross_resource_plans_are_atomic,runtime::user32::tests::shared_cursor_capacity_failure_preserves_identity_and_allocators,runtime::user32::tests::window_class_registration_is_module_scoped_case_folded_and_failure_atomic,runtime::user32::tests::invalid_window_class_metadata_and_atom_exhaustion_do_not_consume_identity,runtime::user32::tests::logical_window_creation_callbacks_are_ordered_and_abort_without_live_identity_leaks,runtime::user32::tests::small_icon_and_window_position_plans_are_atomic_and_reject_stale_commits,runtime::user32::tests::system_menu_identity_is_window_owned_reentrant_and_failure_atomic,runtime::timers::tests::begin_end_pairing_counts_duplicates_and_rejects_stale_plans,runtime::timers::tests::capacity_overflow_and_teardown_are_failure_atomic,runtime::timers::tests::waitable_timer_creation_retirement_and_capacity_are_planned,runtime::timers::tests::window_timer_registration_replacement_and_teardown_preserve_identity,runtime::timers::tests::window_timer_capacity_rejection_is_failure_atomic,runtime::heaps::tests::stale_or_duplicate_plans_preserve_all_heap_collections,runtime::files::tests::source_snapshot_paths_are_case_insensitive_and_mount_bounded,runtime::files::tests::read_plans_advance_cursor_only_after_matching_completion,runtime::modules::tests::dll_lifecycle_plan_commits_private_pending_callback_state,runtime::thread::tests::callback_and_exit_plans_commit_in_explicit_order,runtime::waits::tests::wake_plans_preserve_fifo_and_remove_only_during_commit,runtime::com::tests::stale_or_duplicate_plans_preserve_com_identity_and_refcounts" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::gdi::"))
      .join(",") !==
      "runtime::gdi::tests::device_context_acquisitions_are_distinct_and_release_is_relation_checked,runtime::gdi::tests::stock_brush_and_surface_fill_preserve_failure_atomicity,runtime::gdi::tests::device_context_serialization_preserves_window_shape_and_types_graphics_targets,runtime::gdi::tests::screen_device_context_acquisitions_are_distinct_typed_and_relation_checked,runtime::gdi::tests::graphics_surface_dc_is_single_outstanding_and_guards_com_retirement,runtime::gdi::tests::graphics_surface_dc_capacity_failure_preserves_allocator_state,runtime::gdi::tests::font_creation_owns_distinct_identity_and_rejects_stale_or_invalid_plans,runtime::gdi::tests::compatible_memory_dcs_share_defaults_and_delete_exact_owner_identity_atomically,runtime::gdi::tests::compatible_memory_dc_multi_object_preflight_preserves_all_allocators,runtime::gdi::tests::window_surface_resize_preserves_identity_clears_content_and_rejects_live_dc" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::gdiplus::"))
      .join(",") !==
      "runtime::gdiplus::tests::startup_and_shutdown_plans_are_mutation_free_and_relation_checked,runtime::gdiplus::tests::distinct_sessions_have_non_native_monotonic_tokens_and_bounded_capacity,runtime::gdiplus::tests::process_teardown_rejects_stale_plan_and_clears_both_indexes,runtime::gdiplus::tests::invalid_profile_fails_before_allocator_mutation" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::graphics::"))
      .join(",") !==
      "runtime::graphics::tests::creation_retirement_and_stale_plans_preserve_owner_invariants,runtime::graphics::tests::texture_creation_and_cascading_retirement_preserve_device_relation,runtime::graphics::tests::process_teardown_is_planned_and_rejects_stale_state,runtime::graphics::tests::device_capacity_failure_preserves_allocator_and_generation,runtime::graphics::tests::texture_capacity_failure_preserves_allocator_relations_and_generation,runtime::graphics::tests::texture_dirty_regions_are_bounded_coalesced_and_failure_atomic,runtime::graphics::tests::surface_capacity_and_duplicate_level_failures_preserve_owner_state,runtime::graphics::tests::surface_fill_mutates_texture_content_and_rejects_stale_or_invalid_plans,runtime::graphics::tests::texture_update_consumes_whole_dirty_content_after_surface_retirement_atomically" ||
    representativeSubsystemTests
      .filter((test) => test.startsWith("runtime::audio::"))
      .join(",") !==
      "runtime::audio::tests::device_cooperative_state_and_com_retirement_are_planned_and_owned,runtime::audio::tests::invalid_targets_and_capacity_fail_without_mutation,runtime::audio::tests::buffers_own_format_playback_and_parent_retirement_relations,runtime::audio::tests::invalid_and_duplicate_buffers_fail_without_mutation,runtime::audio::tests::secondary_lock_unlock_and_play_preserve_backing_and_generation_invariants" ||
    representativeAtomicTests
      .filter(
        (test) =>
          test !==
            "user32_dispatcher::set_window_long_a_updates_effective_style_visibility_and_preserves_failures" &&
          test !==
            "user32_dispatcher::set_window_pos_same_geometry_activation_commits_ordered_focus_and_ime_callbacks" &&
          test !==
            "user32_dispatcher::set_window_pos_resize_atomically_updates_window_and_gdi_surface_after_nccalc" &&
          test !==
            "user32_dispatcher::set_window_pos_resize_rejects_live_window_dc_before_window_or_surface_commit" &&
          test !==
            "user32_dispatcher::set_window_pos_frame_change_move_drives_show_and_update_paint_lifecycle_atomically" &&
          test !==
            "user32_dispatcher::show_window_rejects_unreached_command_handle_and_window_state_atomically" &&
          test !==
            "user32_dispatcher::update_window_rejects_invalid_handle_disabled_policy_and_missing_region_atomically" &&
          test !==
            "d3d9_dispatcher::create_device_rejects_invalid_or_unavailable_requests_without_partial_state" &&
          test !==
            "d3d9_dispatcher::d3d9_device_texture_and_surface_catalogs_fix_the_complete_x86_vtables" &&
          test !==
            "runtime::kernel::tests::d3d9_device_and_texture_commit_guest_com_and_graphics_state_as_one_lifetime" &&
          test !==
            "runtime::kernel::tests::direct_sound_cooperative_level_commits_the_live_window_relation" &&
          test !==
            "runtime::kernel::tests::direct_sound_secondary_lock_unlock_play_commits_memory_audio_and_com_lifetime" &&
          test !==
            "user32_dispatcher::set_timer_registers_and_replaces_owned_window_timer_atomically" &&
          test !==
            "source_file_gateway::create_directory_a_publishes_only_after_durable_host_completion" &&
          !test.startsWith("dinput8_dispatcher::") &&
          !test.startsWith("dsound_dispatcher::") &&
          !test.startsWith("gdi32_dispatcher::") &&
          !test.startsWith("gdiplus_dispatcher::") &&
          !test.startsWith("kernel32_virtual_memory_dispatcher::"),
      )
      .join(",") !==
      "runtime_heap_backing::private_heap_backing_is_atomic_reusable_and_process_owned,runtime_large_heap_materialization::large_heap_reserves_lazily_materializes_pages_and_reuses_its_permanent_slot,runtime_dll_lifecycle::guest_import_graph_drives_atomic_dll_process_lifecycle,runtime_system_module_load::dynamic_runtime_system_load_owns_identity_reference_and_attached_state,runtime_lifecycle::process_and_thread_lifecycle_separates_exit_request_from_waitable_completion,runtime_wait_queue::wait_queue_wakes_on_thread_signal_and_cancels_on_process_exit,com_iunknown_foundation::x86_iunknown_abi_owns_identity_refcounts_and_structured_stops,user32_dispatcher::callback_preparation_failures_preserve_memory_and_frame_allocator,user32_dispatcher::swap_mouse_button_policy_and_authority_failures_are_atomic,user32_dispatcher::load_icon_a_negative_paths_are_failure_atomic_and_do_not_consume_identity,user32_dispatcher::load_image_a_negative_variants_are_failure_atomic,user32_dispatcher::load_cursor_a_negative_variants_are_failure_atomic,user32_dispatcher::register_class_ex_a_negative_paths_are_failure_atomic_and_do_not_consume_atom,user32_dispatcher::get_system_metrics_rejects_unreached_or_inconsistent_profiles_without_mutation,user32_dispatcher::create_window_ex_a_preflight_and_callback_rejection_do_not_leave_live_windows,user32_dispatcher::set_window_pos_rejects_unreached_variants_and_invalid_state_atomically,user32_dispatcher::set_window_pos_callback_result_failures_do_not_publish_window_state,user32_dispatcher::system_menu_gateway_rejects_invalid_and_unreached_variants_before_mutation,user32_dispatcher::live_system_menu_identity_is_window_owned_reused_and_failure_atomic,user32_dispatcher::window_dc_fill_slice_preserves_identity_lifetime_and_failure_atomicity,user32_dispatcher::screen_dc_acquisition_and_release_preserve_null_window_native_lifetime,winmm_dispatcher::time_period_policy_and_lifecycle_failures_are_atomic,runtime_waitable_timer::waitable_timer_identity_wait_state_close_and_process_teardown_are_atomic,source_file_gateway::source_file_api_slice_opens_reads_seeks_and_closes_atomically,source_file_gateway::source_file_sharing_mount_and_pending_exit_paths_preserve_authority,source_file_gateway::get_file_attributes_a_projects_only_registered_vfs_paths,d3d9_dispatcher::direct3d_create9_installs_bounded_root_identity_and_retires_atomically,d3d9_dispatcher::get_adapter_display_mode_reads_logical_geometry_and_writes_one_atomic_mode,d3d9_dispatcher::get_adapter_display_mode_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::get_adapter_identifier_writes_one_policy_owned_virtual_identity,d3d9_dispatcher::get_adapter_identifier_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::get_device_caps_writes_one_bounded_virtual_capability_profile,d3d9_dispatcher::get_device_caps_negative_paths_preserve_memory_and_authority,d3d9_dispatcher::check_device_type_accepts_the_bounded_windowed_and_fullscreen_band,d3d9_dispatcher::check_device_type_failure_classes_preserve_memory_and_authority,d3d9_dispatcher::check_device_format_accepts_the_bounded_texture_usage_band,d3d9_dispatcher::check_device_format_failure_classes_preserve_memory_and_authority,d3d9_dispatcher::direct3d_create9_contract_and_resource_exhaustion_fail_before_publication" ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "source_file_gateway::create_directory_a_publishes_only_after_durable_host_completion",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_window_long_a_updates_effective_style_visibility_and_preserves_failures",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_window_pos_same_geometry_activation_commits_ordered_focus_and_ime_callbacks",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_window_pos_resize_atomically_updates_window_and_gdi_surface_after_nccalc",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_window_pos_resize_rejects_live_window_dc_before_window_or_surface_commit",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_window_pos_frame_change_move_drives_show_and_update_paint_lifecycle_atomically",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::show_window_rejects_unreached_command_handle_and_window_state_atomically",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::update_window_rejects_invalid_handle_disabled_policy_and_missing_region_atomically",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "d3d9_dispatcher::create_device_rejects_invalid_or_unavailable_requests_without_partial_state",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "d3d9_dispatcher::d3d9_device_texture_and_surface_catalogs_fix_the_complete_x86_vtables",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "runtime::kernel::tests::d3d9_device_and_texture_commit_guest_com_and_graphics_state_as_one_lifetime",
    ).length !== 1 ||
    representativeAtomicTests
      .filter((test) => test.startsWith("dinput8_dispatcher::"))
      .join(",") !==
      "dinput8_dispatcher::direct_input_8_create_installs_owned_identity_and_stops_at_reached_enum_devices,dinput8_dispatcher::enum_devices_returns_the_evidenced_empty_game_controller_catalog_without_callback_state,dinput8_dispatcher::enum_devices_rejects_invalid_arguments_and_stops_on_valid_unreached_variants_atomically,dinput8_dispatcher::enum_devices_policy_rejects_device_claims_before_dispatch,dinput8_dispatcher::factory_oracle_variants_clear_output_without_allocating_com_state,dinput8_dispatcher::factory_can_publish_iunknown_and_preserves_failure_atomicity,dinput8_dispatcher::factory_catalog_fixes_the_five_word_hresult_contract" ||
    representativeAtomicTests
      .filter((test) => test.startsWith("dsound_dispatcher::"))
      .join(",") !==
      "dsound_dispatcher::direct_sound_create_8_installs_one_owned_root_and_audio_lifetime,dsound_dispatcher::factory_oracle_failures_preserve_output_and_all_authorities,dsound_dispatcher::factory_resource_exhaustion_rolls_back_guest_com_and_audio_state,dsound_dispatcher::cooperative_level_rejects_invalid_or_unreached_relations_atomically,dsound_dispatcher::factory_catalog_fixes_the_ordinal_three_word_hresult_contract,dsound_dispatcher::buffer_oracle_argument_failures_preserve_output_and_authorities" ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "runtime::kernel::tests::direct_sound_cooperative_level_commits_the_live_window_relation",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "runtime::kernel::tests::direct_sound_secondary_lock_unlock_play_commits_memory_audio_and_com_lifetime",
    ).length !== 1 ||
    representativeAtomicTests.filter(
      (test) =>
        test ===
        "user32_dispatcher::set_timer_registers_and_replaces_owned_window_timer_atomically",
    ).length !== 1 ||
    representativeAtomicTests
      .filter((test) => test.startsWith("gdi32_dispatcher::"))
      .join(",") !==
      "gdi32_dispatcher::enum_font_families_ex_a_catalog_exposes_bounded_callback_contract,gdi32_dispatcher::enum_font_families_ex_a_materializes_primary_record_and_propagates_callback_result,gdi32_dispatcher::enum_font_families_ex_a_rejects_unreached_filters_flags_and_callbacks_atomically,gdi32_dispatcher::enum_font_families_ex_a_requires_live_same_thread_screen_dc_atomically,gdi32_dispatcher::font_callback_preparation_failure_preserves_memory_and_frame_identity,gdi32_dispatcher::create_font_a_catalog_exposes_bounded_windows_932_contract,gdi32_dispatcher::create_font_a_creates_distinct_rust_owned_objects_without_guest_writes,gdi32_dispatcher::create_font_a_rejects_unreached_or_malformed_variants_atomically,gdi32_dispatcher::compatible_dc_catalog_exposes_nullable_source_and_owned_delete_contracts,gdi32_dispatcher::compatible_dc_creation_shares_defaults_and_delete_retires_only_the_exact_dc,gdi32_dispatcher::compatible_dc_rejects_nonnull_or_disabled_creation_and_nonmemory_delete_atomically,gdi32_dispatcher::dib_object_catalog_exposes_bounded_x86_lifecycle_contracts,gdi32_dispatcher::dib_backing_and_gdi_objects_follow_primary_restore_before_delete_lifecycle,gdi32_dispatcher::create_dib_section_rejects_malformed_unreached_and_stale_inputs_atomically,gdi32_dispatcher::outline_text_metrics_catalog_exposes_nullable_size_query_and_bounded_output,gdi32_dispatcher::outline_text_metrics_matches_native_size_partial_failure_and_full_buffer_contracts,gdi32_dispatcher::outline_text_metrics_write_failure_preserves_guest_bytes_and_gdi_state,gdi32_dispatcher::outline_text_metrics_rejects_unprofiled_selection_or_disabled_policy_atomically,gdi32_dispatcher::text_out_catalog_exposes_nullable_counted_windows_932_input,gdi32_dispatcher::text_out_rasterizes_native_a_mask_into_selected_dib_and_zero_length_is_noop,gdi32_dispatcher::text_out_rejects_stale_selection_content_and_unwritable_backing_atomically" ||
    representativeAtomicTests
      .filter((test) => test.startsWith("gdiplus_dispatcher::"))
      .join(",") !==
      "gdiplus_dispatcher::catalog_exposes_x86_startup_and_shutdown_lifetime_contracts,gdiplus_dispatcher::startup_writes_distinct_logical_tokens_and_shutdown_retires_exact_session,gdiplus_dispatcher::unsupported_version_returns_status_without_writing_or_allocating,gdiplus_dispatcher::unreached_variants_and_bad_pointers_fail_before_authority_mutation" ||
    representativeAtomicTests
      .filter((test) => test.startsWith("kernel32_virtual_memory_dispatcher::"))
      .join(",") !==
      "kernel32_virtual_memory_dispatcher::catalog_exposes_bounded_x86_virtual_alloc_contract,kernel32_virtual_memory_dispatcher::reserve_top_down_registers_one_process_allocation_and_lazy_guest_region,kernel32_virtual_memory_dispatcher::unreached_variants_fail_before_runtime_or_memory_authority_mutation,kernel32_virtual_memory_dispatcher::owner_capacity_failure_is_atomic_across_allocator_and_guest_memory,kernel32_virtual_memory_dispatcher::guest_region_overlap_rejects_before_allocator_commit_or_owner_id_advance" ||
    kernelBoundary.capabilityExpansionPausedAt !==
      root.nextStructuredStop?.nextCheckpoint
  ) {
    errors.push("active runtime kernel responsibility boundary is invalid");
  }
  const objects = requireRecord(
    active.runtimeObjectModel,
    "activeRuntimeCheckpoint.runtimeObjectModel",
    errors,
  );
  const objectKinds = requireStringArray(
    objects.objectKinds,
    "activeRuntimeCheckpoint.runtimeObjectModel.objectKinds",
    errors,
  );
  const waitableKinds = requireStringArray(
    objects.waitableKinds,
    "activeRuntimeCheckpoint.runtimeObjectModel.waitableKinds",
    errors,
  );
  if (
    objects.schema !== "vine.runtime-object-table.v2" ||
    objects.stateOwner !== "runtime/objects.rs::RuntimeObjectTable" ||
    objects.mutableCollectionVisibility !== "private" ||
    objects.readInspection !== "immutable-borrow" ||
    objects.crossSubsystemMutation !== "plan-preflight-commit" ||
    objects.subsystemTests < 4 ||
    objects.crossSubsystemAtomicTests < 4 ||
    objects.failedRegistrationPreservesAllocator !== true ||
    objects.aggregateDirectCollectionMutation !== false ||
    objects.wholeRuntimeStateClone !== false ||
    objectKinds.join(",") !==
      "process,thread,mutex,waitable-timer,file,private-heap,module" ||
    objects.sourceFileHandleNamespace !== "vine.runtime-source-file.v1" ||
    objects.sourceFileClosePlan !==
      "payload-last-handle-retirement-plan-preflight-commit" ||
    objects.sourceFilePendingReadCloseRejected !== true ||
    objects.sourceFileProcessExitCancelsBeforeRetirement !== true ||
    waitableKinds.join(",") !== "process,thread,mutex,waitable-timer"
  ) {
    errors.push("active runtime object subsystem boundary is invalid");
  }
  const comFoundation = requireRecord(
    active.runtimeComFoundation,
    "activeRuntimeCheckpoint.runtimeComFoundation",
    errors,
  );
  const comMethods = requireStringArray(
    comFoundation.implementedMethods,
    "activeRuntimeCheckpoint.runtimeComFoundation.implementedMethods",
    errors,
  );
  const comReturnKinds = requireStringArray(
    comFoundation.returnKinds,
    "activeRuntimeCheckpoint.runtimeComFoundation.returnKinds",
    errors,
  );
  const comFaultIdentityFields = requireStringArray(
    comFoundation.faultIdentityFields,
    "activeRuntimeCheckpoint.runtimeComFoundation.faultIdentityFields",
    errors,
  );
  const concreteComInterfaces = requireStringArray(
    comFoundation.concreteInterfaces,
    "activeRuntimeCheckpoint.runtimeComFoundation.concreteInterfaces",
    errors,
  );
  if (
    comFoundation.schema !== "vine.runtime-com-foundation.v2" ||
    comFoundation.status !== "implemented-and-verified" ||
    comFoundation.stateOwner !== "runtime/com.rs::RuntimeCom" ||
    comFoundation.aggregateCoordinator !== "runtime/kernel.rs::RuntimeKernel" ||
    comFoundation.mutableCollectionVisibility !== "private" ||
    comFoundation.readInspection !== "immutable-borrow" ||
    comFoundation.logicalIdentityAuthority !== "rust" ||
    comFoundation.logicalIdentitySerialization !== "decimal-string" ||
    comFoundation.sharedReferenceCountAuthority !== "rust" ||
    comFoundation.apartmentStateOwner !== "runtime/thread.rs::RuntimeThreads" ||
    comFoundation.apartmentStateScope !==
      "per-thread-model-and-successful-initialization-count" ||
    comFoundation.apartmentMutation !==
      "plan-stale-check-targeted-thread-commit" ||
    comFoundation.apartmentThreadExitCleanup !== true ||
    comFoundation.canonicalIUnknownIid !==
      "00000000-0000-0000-c000-000000000046" ||
    comFoundation.abi !== "x86-com-stdcall" ||
    comMethods.join(",") !== "QueryInterface,AddRef,Release" ||
    comReturnKinds.join(",") !== "hresult,u32" ||
    comFoundation.guestLayout !== "two-page-readonly-data-rx-endpoints" ||
    comFoundation.endpointInstruction !== "ud2-vcom-interface-slot-marker" ||
    comFoundation.addressAllocationPurpose !== "com-endpoint" ||
    comFoundation.addressAllocationIdentity !== true ||
    comFoundation.queryInterfaceOutputAndRefcountAtomic !== true ||
    comFoundation.completionPreflightBeforeMutation !== true ||
    comFoundation.finalReleaseRetiresGuestMappingAndAllocation !== true ||
    comFoundation.childObjectRetainsParentInterfaceReference !== true ||
    comFoundation.childFinalReleaseCascadesParentRetirement !== true ||
    comFoundation.cascadeRetirementOrder !== "child-before-parent" ||
    comFoundation.crossSubsystemRetirementPreflight !==
      "com-address-space-guest-memory-runtime-graphics-runtime-audio" ||
    comFoundation.lastErrorPolicy !== "preserve" ||
    comFoundation.unsupportedMethodPolicy !==
      "structured-stop-before-mutation" ||
    comFaultIdentityFields.join(",") !==
      "interface,vtable-slot,method,call-site" ||
    comFoundation.browserResourceStateOwner !==
      "browser/runtime-worker/browser-resource-table.ts" ||
    comFoundation.browserIdentityNamespace !==
      "browser-resource:<positive-safe-integer>" ||
    comFoundation.browserAssociationOwnsComReference !== false ||
    comFoundation.subsystemTests < 3 ||
    comFoundation.fixtureTests < 6 ||
    comFoundation.atomicFailureTests < 5 ||
    comFoundation.wholeRuntimeStateClone !== false ||
    comFoundation.concreteGraphicsOrAudioInterfaces !== true ||
    concreteComInterfaces.join(",") !==
      "IDirect3D9,IDirect3DDevice9,IDirect3DTexture9,IDirect3DSurface9,IDirectSound8,IDirectSoundBuffer8" ||
    comFoundation.coInitializeApi !== "null-reserved-single-threaded-v1" ||
    comFoundation.coInitializeExApiImplemented !== false ||
    comFoundation.coUninitializeApiImplemented !== false ||
    comFoundation.classFactoryOrAutomationImplemented !== false
  ) {
    errors.push("active runtime COM foundation boundary is invalid");
  }
  const d3d9 = requireRecord(
    active.d3d9Frontend,
    "activeRuntimeCheckpoint.d3d9Frontend",
    errors,
  );
  const d3d9IUnknownMethods = requireStringArray(
    d3d9.iunknownMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.iunknownMethods",
    errors,
  );
  const d3d9ImplementedRootMethods = requireStringArray(
    d3d9.implementedRootMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.implementedRootMethods",
    errors,
  );
  const d3d9CompatibilityPolicy = requireRecord(
    d3d9.compatibilityPolicy,
    "activeRuntimeCheckpoint.d3d9Frontend.compatibilityPolicy",
    errors,
  );
  const d3d9AdapterCount = requireRecord(
    d3d9.getAdapterCount,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterCount",
    errors,
  );
  const d3d9NativeAdapterCount = requireRecord(
    d3d9.nativeAdapterCountEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterCountEvidence",
    errors,
  );
  const d3d9PrivateAdapterCount = requireRecord(
    d3d9.privateRuntimeAdapterCountEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterCountEvidence",
    errors,
  );
  const d3d9PrivateAdapterCountArguments = requireOrderedStringArray(
    d3d9PrivateAdapterCount.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterCountEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeAdapterCount.traceSha256,
      "nativeAdapterCountEvidence.traceSha256",
    ],
    [
      d3d9NativeAdapterCount.summarySha256,
      "nativeAdapterCountEvidence.summarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9AdapterMonitor = requireRecord(
    d3d9.getAdapterMonitor,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterMonitor",
    errors,
  );
  const d3d9NativeAdapterMonitor = requireRecord(
    d3d9.nativeAdapterMonitorEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterMonitorEvidence",
    errors,
  );
  const d3d9NativeAdapterMonitorOracleAdapters = requireIntegerArray(
    d3d9NativeAdapterMonitor.oracleAdapters,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterMonitorEvidence.oracleAdapters",
    errors,
  );
  const d3d9PrivateAdapterMonitor = requireRecord(
    d3d9.privateRuntimeAdapterMonitorEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence",
    errors,
  );
  const d3d9PrivateAdapterMonitorNodeSequences = requireIntegerArray(
    d3d9PrivateAdapterMonitor.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateAdapterMonitorNodeInstructionCounts = requireIntegerArray(
    d3d9PrivateAdapterMonitor.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateAdapterMonitorChromeSequences = requireIntegerArray(
    d3d9PrivateAdapterMonitor.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterMonitorEvidence.chromeSequences",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeAdapterMonitor.traceSha256,
      "nativeAdapterMonitorEvidence.traceSha256",
    ],
    [
      d3d9NativeAdapterMonitor.summarySha256,
      "nativeAdapterMonitorEvidence.summarySha256",
    ],
    [
      d3d9NativeAdapterMonitor.oracleSourceSha256,
      "nativeAdapterMonitorEvidence.oracleSourceSha256",
    ],
    [
      d3d9NativeAdapterMonitor.oracleExecutableSha256,
      "nativeAdapterMonitorEvidence.oracleExecutableSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9DisplayMode = requireRecord(
    d3d9.getAdapterDisplayMode,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterDisplayMode",
    errors,
  );
  const d3d9NativeDisplayMode = requireRecord(
    d3d9.nativeDisplayModeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDisplayModeEvidence",
    errors,
  );
  const d3d9NativeDisplayModeCallerRvas = requireStringArray(
    d3d9NativeDisplayMode.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDisplayModeEvidence.callerRvas",
    errors,
  );
  const d3d9PrivateDisplayMode = requireRecord(
    d3d9.privateRuntimeDisplayModeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDisplayModeEvidence",
    errors,
  );
  const d3d9PrivateDisplayModeArguments = requireStringArray(
    d3d9PrivateDisplayMode.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDisplayModeEvidence.arguments",
    errors,
  );
  const d3d9AdapterIdentifier = requireRecord(
    d3d9.getAdapterIdentifier,
    "activeRuntimeCheckpoint.d3d9Frontend.getAdapterIdentifier",
    errors,
  );
  const d3d9NativeAdapterIdentifier = requireRecord(
    d3d9.nativeAdapterIdentifierEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterIdentifierEvidence",
    errors,
  );
  const d3d9PrivateAdapterIdentifier = requireRecord(
    d3d9.privateRuntimeAdapterIdentifierEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence",
    errors,
  );
  const d3d9PrivateAdapterIdentifierArguments = requireOrderedStringArray(
    d3d9PrivateAdapterIdentifier.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence.arguments",
    errors,
  );
  for (const field of [
    "primaryTraceSha256",
    "confirmationTraceSha256",
    "inputSha256",
    "outputSha256",
  ]) {
    requireSha256(
      d3d9NativeAdapterIdentifier[field],
      `activeRuntimeCheckpoint.d3d9Frontend.nativeAdapterIdentifierEvidence.${field}`,
      errors,
    );
  }
  requireSha256(
    d3d9PrivateAdapterIdentifier.outputSha256,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAdapterIdentifierEvidence.outputSha256",
    errors,
  );
  const d3d9DeviceCaps = requireRecord(
    d3d9.getDeviceCaps,
    "activeRuntimeCheckpoint.d3d9Frontend.getDeviceCaps",
    errors,
  );
  const d3d9NativeDeviceCaps = requireRecord(
    d3d9.nativeDeviceCapsEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedFields = requireStringArray(
    d3d9NativeDeviceCaps.consumedFields,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.consumedFields",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedFieldOffsets = requireIntegerArray(
    d3d9NativeDeviceCaps.consumedFieldOffsets,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.consumedFieldOffsets",
    errors,
  );
  const d3d9NativeDeviceCapsConsumedValues = requireStringArray(
    d3d9NativeDeviceCaps.nativeConsumedValues,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceCapsEvidence.nativeConsumedValues",
    errors,
  );
  const d3d9PrivateDeviceCaps = requireRecord(
    d3d9.privateRuntimeDeviceCapsEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceCapsEvidence",
    errors,
  );
  const d3d9PrivateDeviceCapsArguments = requireOrderedStringArray(
    d3d9PrivateDeviceCaps.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceCapsEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeDeviceCaps.primaryTraceSha256,
      "nativeDeviceCapsEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeDeviceCaps.confirmationTraceSha256,
      "nativeDeviceCapsEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeDeviceCaps.outputSha256,
      "nativeDeviceCapsEvidence.outputSha256",
    ],
    [
      d3d9PrivateDeviceCaps.outputSha256,
      "privateRuntimeDeviceCapsEvidence.outputSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9CheckDeviceType = requireRecord(
    d3d9.checkDeviceType,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceType",
    errors,
  );
  const d3d9NativeDeviceType = requireRecord(
    d3d9.nativeDeviceTypeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence",
    errors,
  );
  const d3d9NativeDeviceTypeCallerRvas = requireStringArray(
    d3d9NativeDeviceType.directCallerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence.directCallerRvas",
    errors,
  );
  const d3d9NativeDeviceTypeWindowedValues = requireIntegerArray(
    d3d9NativeDeviceType.directWindowedValues,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceTypeEvidence.directWindowedValues",
    errors,
  );
  const d3d9PrivateDeviceType = requireRecord(
    d3d9.privateRuntimeDeviceTypeEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence",
    errors,
  );
  const d3d9PrivateDeviceTypeNodeSequences = requireIntegerArray(
    d3d9PrivateDeviceType.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateDeviceTypeInstructionCounts = requireIntegerArray(
    d3d9PrivateDeviceType.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateDeviceTypeChromeSequences = requireIntegerArray(
    d3d9PrivateDeviceType.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceTypeEvidence.chromeSequences",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeDeviceType.primaryTraceSha256,
      "nativeDeviceTypeEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeDeviceType.confirmationTraceSha256,
      "nativeDeviceTypeEvidence.confirmationTraceSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9CheckDeviceFormat = requireRecord(
    d3d9.checkDeviceFormat,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat",
    errors,
  );
  const d3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    d3d9CheckDeviceFormat.supportedUsages,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat.supportedUsages",
    errors,
  );
  const d3d9CheckDeviceFormatSupportedUsageNames = requireStringArray(
    d3d9CheckDeviceFormat.supportedUsageNames,
    "activeRuntimeCheckpoint.d3d9Frontend.checkDeviceFormat.supportedUsageNames",
    errors,
  );
  const d3d9NativeDeviceFormat = requireRecord(
    d3d9.nativeDeviceFormatEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceFormatEvidence",
    errors,
  );
  const d3d9NativeDeviceFormatSupportedUsages = requireIntegerArray(
    d3d9NativeDeviceFormat.supportedUsages,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeDeviceFormatEvidence.supportedUsages",
    errors,
  );
  const d3d9PrivateDeviceFormat = requireRecord(
    d3d9.privateRuntimeDeviceFormatEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceFormatEvidence",
    errors,
  );
  const d3d9PrivateDeviceFormatArguments = requireOrderedStringArray(
    d3d9PrivateDeviceFormat.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeDeviceFormatEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeDeviceFormat.primaryTraceSha256,
      "nativeDeviceFormatEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeDeviceFormat.confirmationTraceSha256,
      "nativeDeviceFormatEvidence.confirmationTraceSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9CreateDevice = requireRecord(
    d3d9.createDevice,
    "activeRuntimeCheckpoint.d3d9Frontend.createDevice",
    errors,
  );
  const d3d9CreateDeviceBehaviorFlagNames = requireStringArray(
    d3d9CreateDevice.behaviorFlagNames,
    "activeRuntimeCheckpoint.d3d9Frontend.createDevice.behaviorFlagNames",
    errors,
  );
  const d3d9DeviceInterface = requireRecord(
    d3d9.deviceInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.deviceInterface",
    errors,
  );
  const d3d9DeviceImplementedMethods = requireStringArray(
    d3d9DeviceInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.deviceInterface.implementedMethods",
    errors,
  );
  const runtimeGraphics = requireRecord(
    d3d9.runtimeGraphics,
    "activeRuntimeCheckpoint.d3d9Frontend.runtimeGraphics",
    errors,
  );
  const d3d9NativeCreateDevice = requireRecord(
    d3d9.nativeCreateDeviceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateDeviceEvidence",
    errors,
  );
  const d3d9NativeCreateDeviceInput = requireOrderedIntegerArray(
    d3d9NativeCreateDevice.inputWidthHeightFormat,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateDeviceEvidence.inputWidthHeightFormat",
    errors,
  );
  const d3d9PrivateCreateDevice = requireRecord(
    d3d9.privateRuntimeCreateDeviceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence",
    errors,
  );
  const d3d9PrivateCreateDeviceArguments = requireOrderedStringArray(
    d3d9PrivateCreateDevice.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence.arguments",
    errors,
  );
  const d3d9PrivateLogicalPresentation = requireOrderedIntegerArray(
    d3d9PrivateCreateDevice.logicalPresentation,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateDeviceEvidence.logicalPresentation",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeCreateDevice.primaryTraceSha256,
      "nativeCreateDeviceEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeCreateDevice.primarySummarySha256,
      "nativeCreateDeviceEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeCreateDevice.confirmationTraceSha256,
      "nativeCreateDeviceEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeCreateDevice.confirmationSummarySha256,
      "nativeCreateDeviceEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9CreateTexture = requireRecord(
    d3d9.createTexture,
    "activeRuntimeCheckpoint.d3d9Frontend.createTexture",
    errors,
  );
  const d3d9CreateTextureVariants = Array.isArray(d3d9CreateTexture.variants)
    ? d3d9CreateTexture.variants.map((variant, index) =>
        requireRecord(
          variant,
          `activeRuntimeCheckpoint.d3d9Frontend.createTexture.variants[${index}]`,
          errors,
        ),
      )
    : (errors.push(
        "activeRuntimeCheckpoint.d3d9Frontend.createTexture.variants must be an array",
      ),
      []);
  const d3d9TextureInterface = requireRecord(
    d3d9.textureInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.textureInterface",
    errors,
  );
  const d3d9TextureImplementedMethods = requireStringArray(
    d3d9TextureInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.textureInterface.implementedMethods",
    errors,
  );
  const d3d9NativeCreateTexture = requireRecord(
    d3d9.nativeCreateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateTextureEvidence",
    errors,
  );
  const d3d9NativeCreateTextureArguments = requireOrderedIntegerArray(
    d3d9NativeCreateTexture.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeCreateTextureEvidence.arguments",
    errors,
  );
  const d3d9NativeInitialTextures = requireRecord(
    d3d9.nativeInitialTextureSequenceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence",
    errors,
  );
  const d3d9NativeInitialTextureCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialTextures.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialTextureResults = requireOrderedStringArray(
    d3d9NativeInitialTextures.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.results",
    errors,
  );
  const d3d9NativeInitialTextureLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialTextures.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.lastErrors",
    errors,
  );
  const d3d9NativeInitialTextureOutputAddresses = requireOrderedStringArray(
    d3d9NativeInitialTextures.outputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.outputAddresses",
    errors,
  );
  const d3d9NativeInitialTextureNextArguments = requireOrderedIntegerArray(
    d3d9NativeInitialTextures.immediateNextArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialTextureSequenceEvidence.immediateNextArguments",
    errors,
  );
  const d3d9PrivateCreateTexture = requireRecord(
    d3d9.privateRuntimeCreateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence",
    errors,
  );
  const d3d9PrivateCreateTextureArguments = requireOrderedStringArray(
    d3d9PrivateCreateTexture.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence.arguments",
    errors,
  );
  const d3d9PrivateTextureDescriptor = requireOrderedIntegerArray(
    d3d9PrivateCreateTexture.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeCreateTextureEvidence.logicalDescriptor",
    errors,
  );
  const d3d9PrivateInitialTextures = requireRecord(
    d3d9.privateRuntimeInitialTextureSequenceEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence",
    errors,
  );
  const d3d9PrivateInitialTextureNodeSequences = requireIntegerArray(
    d3d9PrivateInitialTextures.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialTextureInstructionCounts = requireIntegerArray(
    d3d9PrivateInitialTextures.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateInitialTextureChromeSequences = requireIntegerArray(
    d3d9PrivateInitialTextures.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateInitialTextureReturnAddresses = requireOrderedStringArray(
    d3d9PrivateInitialTextures.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateInitialTextureResults = requireOrderedStringArray(
    d3d9PrivateInitialTextures.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.results",
    errors,
  );
  const d3d9PrivateInitialTextureOutputAddresses = requireOrderedStringArray(
    d3d9PrivateInitialTextures.textureOutputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.textureOutputAddresses",
    errors,
  );
  const d3d9PrivateInitialTexturePointers = requireOrderedStringArray(
    d3d9PrivateInitialTextures.texturePointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.texturePointers",
    errors,
  );
  const d3d9PrivateInitialTextureGuestWrites = requireOrderedIntegerArray(
    d3d9PrivateInitialTextures.guestWriteBytes,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialTextureSequenceEvidence.guestWriteBytes",
    errors,
  );
  const d3d9NativeRecreatedTextures = requireRecord(
    d3d9.nativeRecreatedTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRecreatedTextureEvidence",
    errors,
  );
  const d3d9NativeRecreatedTextureCallerRvas = requireOrderedStringArray(
    d3d9NativeRecreatedTextures.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRecreatedTextureEvidence.callerRvas",
    errors,
  );
  const d3d9NativeRecreatedTextureResults = requireOrderedStringArray(
    d3d9NativeRecreatedTextures.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRecreatedTextureEvidence.results",
    errors,
  );
  const d3d9NativeRecreatedTextureLastErrors = requireOrderedIntegerArray(
    d3d9NativeRecreatedTextures.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRecreatedTextureEvidence.lastErrors",
    errors,
  );
  const d3d9NativeRecreatedTextureOutputAddresses = requireOrderedStringArray(
    d3d9NativeRecreatedTextures.outputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRecreatedTextureEvidence.outputAddresses",
    errors,
  );
  const d3d9PrivateRecreatedTextures = requireRecord(
    d3d9.privateRuntimeRecreatedTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence",
    errors,
  );
  const d3d9PrivateRecreatedTextureNodeSequences = requireIntegerArray(
    d3d9PrivateRecreatedTextures.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateRecreatedTextureChromeSequences = requireIntegerArray(
    d3d9PrivateRecreatedTextures.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateRecreatedTextureMethods = requireOrderedStringArray(
    d3d9PrivateRecreatedTextures.orderedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.orderedMethods",
    errors,
  );
  const d3d9PrivateRecreatedTextureSequences = requireIntegerArray(
    d3d9PrivateRecreatedTextures.createTextureSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.createTextureSequences",
    errors,
  );
  const d3d9PrivateRecreatedTextureOccurrences = requireIntegerArray(
    d3d9PrivateRecreatedTextures.createTextureOccurrences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.createTextureOccurrences",
    errors,
  );
  const d3d9PrivateRecreatedTextureInstructionCounts = requireIntegerArray(
    d3d9PrivateRecreatedTextures.createTextureInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.createTextureInstructionCounts",
    errors,
  );
  const d3d9PrivateRecreatedTextureResults = requireOrderedStringArray(
    d3d9PrivateRecreatedTextures.createTextureResults,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.createTextureResults",
    errors,
  );
  const d3d9PrivateRecreatedTextureReturnAddresses = requireOrderedStringArray(
    d3d9PrivateRecreatedTextures.createTextureReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.createTextureReturnAddresses",
    errors,
  );
  const d3d9PrivateRecreatedTextureOutputAddresses = requireOrderedStringArray(
    d3d9PrivateRecreatedTextures.textureOutputAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.textureOutputAddresses",
    errors,
  );
  const d3d9PrivateRecreatedTexturePointers = requireOrderedStringArray(
    d3d9PrivateRecreatedTextures.texturePointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.texturePointers",
    errors,
  );
  const d3d9PrivateRecreatedTextureReferenceCounts = requireOrderedIntegerArray(
    d3d9PrivateRecreatedTextures.referenceCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.referenceCounts",
    errors,
  );
  const d3d9PrivateRecreatedTextureGuestWrites = requireOrderedIntegerArray(
    d3d9PrivateRecreatedTextures.guestWriteBytes,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.guestWriteBytes",
    errors,
  );
  const d3d9PrivateRecreatedTextureSurfaceGeometry = requireOrderedIntegerArray(
    d3d9PrivateRecreatedTextures.surfaceGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.surfaceGeometry",
    errors,
  );
  const d3d9PrivateRecreatedTextureFillRect = requireOrderedIntegerArray(
    d3d9PrivateRecreatedTextures.fillRect,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRecreatedTextureEvidence.fillRect",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeCreateTexture.primaryTraceSha256,
      "nativeCreateTextureEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeCreateTexture.primarySummarySha256,
      "nativeCreateTextureEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeCreateTexture.confirmationTraceSha256,
      "nativeCreateTextureEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeCreateTexture.confirmationSummarySha256,
      "nativeCreateTextureEvidence.confirmationSummarySha256",
    ],
    [
      d3d9NativeInitialTextures.primaryTraceSha256,
      "nativeInitialTextureSequenceEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeInitialTextures.primarySummarySha256,
      "nativeInitialTextureSequenceEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeInitialTextures.confirmationTraceSha256,
      "nativeInitialTextureSequenceEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeInitialTextures.confirmationSummarySha256,
      "nativeInitialTextureSequenceEvidence.confirmationSummarySha256",
    ],
    [
      d3d9NativeRecreatedTextures.primaryTraceSha256,
      "nativeRecreatedTextureEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeRecreatedTextures.primarySummarySha256,
      "nativeRecreatedTextureEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeRecreatedTextures.confirmationTraceSha256,
      "nativeRecreatedTextureEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeRecreatedTextures.confirmationSummarySha256,
      "nativeRecreatedTextureEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9CreateVertexBuffer = requireRecord(
    d3d9.createVertexBuffer,
    "activeRuntimeCheckpoint.d3d9Frontend.createVertexBuffer",
    errors,
  );
  const d3d9VertexBufferInterface = requireRecord(
    d3d9.vertexBufferInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferInterface",
    errors,
  );
  const d3d9VertexBufferImplementedMethods = requireStringArray(
    d3d9VertexBufferInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferInterface.implementedMethods",
    errors,
  );
  const d3d9VertexBufferLock = requireRecord(
    d3d9.vertexBufferLock,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferLock",
    errors,
  );
  const d3d9VertexBufferUnlock = requireRecord(
    d3d9.vertexBufferUnlock,
    "activeRuntimeCheckpoint.d3d9Frontend.vertexBufferUnlock",
    errors,
  );
  const d3d9NativeInitialVertexBuffer = requireRecord(
    d3d9.nativeInitialVertexBufferEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence",
    errors,
  );
  const d3d9NativeInitialVertexBufferCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialVertexBuffer.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialVertexBufferMethods = requireStringArray(
    d3d9NativeInitialVertexBuffer.methods,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.methods",
    errors,
  );
  const d3d9NativeInitialVertexBufferSlots = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.slots,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.slots",
    errors,
  );
  const d3d9NativeInitialVertexBufferCreateArguments =
    requireOrderedIntegerArray(
      d3d9NativeInitialVertexBuffer.createArguments,
      "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.createArguments",
      errors,
    );
  const d3d9NativeInitialVertexBufferLockArguments = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.lockArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.lockArguments",
    errors,
  );
  const d3d9NativeInitialVertexBufferResults = requireOrderedStringArray(
    d3d9NativeInitialVertexBuffer.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.results",
    errors,
  );
  const d3d9NativeInitialVertexBufferLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.lastErrors",
    errors,
  );
  const d3d9NativeInitialVertexBufferNextArguments = requireOrderedIntegerArray(
    d3d9NativeInitialVertexBuffer.immediateNextArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialVertexBufferEvidence.immediateNextArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBuffer = requireRecord(
    d3d9.privateRuntimeInitialVertexBufferEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence",
    errors,
  );
  const d3d9PrivateInitialVertexBufferNodeSequences = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialVertexBufferInstructionCounts = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateInitialVertexBufferChromeSequences = requireIntegerArray(
    d3d9PrivateInitialVertexBuffer.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateInitialVertexBufferReturnAddresses =
    requireOrderedStringArray(
      d3d9PrivateInitialVertexBuffer.callerReturnAddresses,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.callerReturnAddresses",
      errors,
    );
  const d3d9PrivateInitialVertexBufferCreateArguments =
    requireOrderedStringArray(
      d3d9PrivateInitialVertexBuffer.createArguments,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.createArguments",
      errors,
    );
  const d3d9PrivateInitialVertexBufferLockArguments = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.lockArguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.lockArguments",
    errors,
  );
  const d3d9PrivateInitialVertexBufferUnlockArguments =
    requireOrderedStringArray(
      d3d9PrivateInitialVertexBuffer.unlockArguments,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.unlockArguments",
      errors,
    );
  const d3d9PrivateInitialVertexBufferResults = requireOrderedStringArray(
    d3d9PrivateInitialVertexBuffer.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.results",
    errors,
  );
  const d3d9PrivateInitialVertexBufferDescriptor = requireOrderedIntegerArray(
    d3d9PrivateInitialVertexBuffer.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialVertexBufferEvidence.logicalDescriptor",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeInitialVertexBuffer.primaryTraceSha256,
      "nativeInitialVertexBufferEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeInitialVertexBuffer.primarySummarySha256,
      "nativeInitialVertexBufferEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeInitialVertexBuffer.confirmationTraceSha256,
      "nativeInitialVertexBufferEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeInitialVertexBuffer.confirmationSummarySha256,
      "nativeInitialVertexBufferEvidence.confirmationSummarySha256",
    ],
    [
      d3d9NativeInitialVertexBuffer.initialContentSha256,
      "nativeInitialVertexBufferEvidence.initialContentSha256",
    ],
    [
      d3d9NativeInitialVertexBuffer.writtenContentSha256,
      "nativeInitialVertexBufferEvidence.writtenContentSha256",
    ],
    [
      d3d9PrivateInitialVertexBuffer.contentSha256,
      "privateRuntimeInitialVertexBufferEvidence.contentSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9SetRenderState = requireRecord(
    d3d9.setRenderState,
    "activeRuntimeCheckpoint.d3d9Frontend.setRenderState",
    errors,
  );
  const d3d9SetRenderStateSettings = Array.isArray(
    d3d9SetRenderState.orderedSettings,
  )
    ? d3d9SetRenderState.orderedSettings
    : [];
  if (!Array.isArray(d3d9SetRenderState.orderedSettings)) {
    errors.push(
      "activeRuntimeCheckpoint.d3d9Frontend.setRenderState.orderedSettings must be an array",
    );
  }
  const d3d9NativeInitialRenderStates = requireRecord(
    d3d9.nativeInitialRenderStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence",
    errors,
  );
  const d3d9NativeInitialRenderStateCallerRvas = requireOrderedStringArray(
    d3d9NativeInitialRenderStates.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.callerRvas",
    errors,
  );
  const d3d9NativeInitialRenderStateStates = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.states,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.states",
    errors,
  );
  const d3d9NativeInitialRenderStateValues = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.values,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.values",
    errors,
  );
  const d3d9NativeInitialRenderStateResults = requireOrderedStringArray(
    d3d9NativeInitialRenderStates.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.results",
    errors,
  );
  const d3d9NativeInitialRenderStateLastErrors = requireOrderedIntegerArray(
    d3d9NativeInitialRenderStates.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeInitialRenderStateEvidence.lastErrors",
    errors,
  );
  const d3d9PrivateInitialRenderStates = requireRecord(
    d3d9.privateRuntimeInitialRenderStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence",
    errors,
  );
  const d3d9PrivateInitialRenderStateNodeSequences = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateInitialRenderStateInstructionCounts =
    requireOrderedIntegerArray(
      d3d9PrivateInitialRenderStates.nodeInstructionCounts,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.nodeInstructionCounts",
      errors,
    );
  const d3d9PrivateInitialRenderStateChromeSequences =
    requireOrderedIntegerArray(
      d3d9PrivateInitialRenderStates.chromeSequences,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.chromeSequences",
      errors,
    );
  const d3d9PrivateInitialRenderStateReturnAddresses =
    requireOrderedStringArray(
      d3d9PrivateInitialRenderStates.callerReturnAddresses,
      "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.callerReturnAddresses",
      errors,
    );
  const d3d9PrivateInitialRenderStateStates = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.states,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.states",
    errors,
  );
  const d3d9PrivateInitialRenderStateValues = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.values,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.values",
    errors,
  );
  const d3d9PrivateInitialRenderStateResults = requireOrderedStringArray(
    d3d9PrivateInitialRenderStates.results,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.results",
    errors,
  );
  const d3d9PrivateInitialRenderStateGuestWrites = requireOrderedIntegerArray(
    d3d9PrivateInitialRenderStates.guestWriteBytes,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeInitialRenderStateEvidence.guestWriteBytes",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeInitialRenderStates.primaryTraceSha256,
      "nativeInitialRenderStateEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeInitialRenderStates.primarySummarySha256,
      "nativeInitialRenderStateEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeInitialRenderStates.confirmationTraceSha256,
      "nativeInitialRenderStateEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeInitialRenderStates.confirmationSummarySha256,
      "nativeInitialRenderStateEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9GetRasterStatus = requireRecord(
    d3d9.getRasterStatus,
    "activeRuntimeCheckpoint.d3d9Frontend.getRasterStatus",
    errors,
  );
  const d3d9NativeRasterStatus = requireRecord(
    d3d9.nativeRasterStatusEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRasterStatusEvidence",
    errors,
  );
  const d3d9NativeRasterStatusResults = requireOrderedStringArray(
    d3d9NativeRasterStatus.results,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRasterStatusEvidence.results",
    errors,
  );
  const d3d9NativeRasterStatusLastErrors = requireOrderedIntegerArray(
    d3d9NativeRasterStatus.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRasterStatusEvidence.lastErrors",
    errors,
  );
  const d3d9NativeRasterStatusInVBlank = Array.isArray(
    d3d9NativeRasterStatus.observedInVBlank,
  )
    ? d3d9NativeRasterStatus.observedInVBlank
    : [];
  if (!Array.isArray(d3d9NativeRasterStatus.observedInVBlank)) {
    errors.push(
      "activeRuntimeCheckpoint.d3d9Frontend.nativeRasterStatusEvidence.observedInVBlank must be an array",
    );
  }
  const d3d9NativeRasterStatusScanLines = requireOrderedIntegerArray(
    d3d9NativeRasterStatus.observedScanLines,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeRasterStatusEvidence.observedScanLines",
    errors,
  );
  const d3d9PrivateRasterStatus = requireRecord(
    d3d9.privateRuntimeRasterStatusEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRasterStatusEvidence",
    errors,
  );
  const d3d9PrivateRasterStatusArguments = requireOrderedStringArray(
    d3d9PrivateRasterStatus.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeRasterStatusEvidence.arguments",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeRasterStatus.primaryTraceSha256,
      "nativeRasterStatusEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeRasterStatus.primarySummarySha256,
      "nativeRasterStatusEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeRasterStatus.confirmationTraceSha256,
      "nativeRasterStatusEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeRasterStatus.confirmationSummarySha256,
      "nativeRasterStatusEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9GetSurfaceLevel = requireRecord(
    d3d9.getSurfaceLevel,
    "activeRuntimeCheckpoint.d3d9Frontend.getSurfaceLevel",
    errors,
  );
  const d3d9SurfaceInterface = requireRecord(
    d3d9.surfaceInterface,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceInterface",
    errors,
  );
  const d3d9SurfaceImplementedMethods = requireStringArray(
    d3d9SurfaceInterface.implementedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceInterface.implementedMethods",
    errors,
  );
  const d3d9NativeGetSurfaceLevel = requireRecord(
    d3d9.nativeGetSurfaceLevelEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeGetSurfaceLevelEvidence",
    errors,
  );
  const d3d9ControlledGetSurfaceLevel = requireRecord(
    d3d9.controlledGetSurfaceLevelOracle,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledGetSurfaceLevelOracle",
    errors,
  );
  const d3d9PrivateGetSurfaceLevel = requireRecord(
    d3d9.privateRuntimeGetSurfaceLevelEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence",
    errors,
  );
  const d3d9PrivateGetSurfaceLevelArguments = requireOrderedStringArray(
    d3d9PrivateGetSurfaceLevel.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence.arguments",
    errors,
  );
  const d3d9PrivateSurfaceDescriptor = requireOrderedIntegerArray(
    d3d9PrivateGetSurfaceLevel.logicalDescriptor,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeGetSurfaceLevelEvidence.logicalDescriptor",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeGetSurfaceLevel.primaryTraceSha256,
      "nativeGetSurfaceLevelEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeGetSurfaceLevel.primarySummarySha256,
      "nativeGetSurfaceLevelEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeGetSurfaceLevel.confirmationTraceSha256,
      "nativeGetSurfaceLevelEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeGetSurfaceLevel.confirmationSummarySha256,
      "nativeGetSurfaceLevelEvidence.confirmationSummarySha256",
    ],
    [
      d3d9ControlledGetSurfaceLevel.sourceSha256,
      "controlledGetSurfaceLevelOracle.sourceSha256",
    ],
    [
      d3d9ControlledGetSurfaceLevel.executableSha256,
      "controlledGetSurfaceLevelOracle.executableSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9SurfaceDc = requireRecord(
    d3d9.surfaceDc,
    "activeRuntimeCheckpoint.d3d9Frontend.surfaceDc",
    errors,
  );
  const d3d9NativeSurfaceDc = requireRecord(
    d3d9.nativeSurfaceDcEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence",
    errors,
  );
  const d3d9ControlledSurfaceDc = requireRecord(
    d3d9.controlledSurfaceDcOracle,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledSurfaceDcOracle",
    errors,
  );
  const d3d9PrivateSurfaceDc = requireRecord(
    d3d9.privateRuntimeSurfaceDcEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence",
    errors,
  );
  const d3d9NativeSurfaceDcCallerRvas = requireOrderedStringArray(
    d3d9NativeSurfaceDc.callerRvas,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.callerRvas",
    errors,
  );
  const d3d9NativeSurfaceDcMethods = requireStringArray(
    d3d9NativeSurfaceDc.orderedMethods,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.orderedMethods",
    errors,
  );
  const d3d9NativeSurfaceDcRect = requireOrderedIntegerArray(
    d3d9NativeSurfaceDc.rect,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.rect",
    errors,
  );
  const d3d9NativeSurfaceDcLastErrors = requireOrderedIntegerArray(
    d3d9NativeSurfaceDc.lastErrors,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSurfaceDcEvidence.lastErrors",
    errors,
  );
  const d3d9ControlledSurfaceDcReferenceCounts = requireOrderedIntegerArray(
    d3d9ControlledSurfaceDc.surfaceReferenceCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.controlledSurfaceDcOracle.surfaceReferenceCounts",
    errors,
  );
  const d3d9PrivateSurfaceDcNodeSequences = requireIntegerArray(
    d3d9PrivateSurfaceDc.nodeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.nodeSequences",
    errors,
  );
  const d3d9PrivateSurfaceDcInstructionCounts = requireIntegerArray(
    d3d9PrivateSurfaceDc.nodeInstructionCounts,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.nodeInstructionCounts",
    errors,
  );
  const d3d9PrivateSurfaceDcChromeSequences = requireIntegerArray(
    d3d9PrivateSurfaceDc.chromeSequences,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.chromeSequences",
    errors,
  );
  const d3d9PrivateSurfaceDcReturnAddresses = requireOrderedStringArray(
    d3d9PrivateSurfaceDc.callerReturnAddresses,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.callerReturnAddresses",
    errors,
  );
  const d3d9PrivateSurfaceDcGeometry = requireOrderedIntegerArray(
    d3d9PrivateSurfaceDc.surfaceGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.surfaceGeometry",
    errors,
  );
  const d3d9PrivateSurfaceDcFillRect = requireOrderedIntegerArray(
    d3d9PrivateSurfaceDc.fillRect,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSurfaceDcEvidence.fillRect",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeSurfaceDc.primaryTraceSha256,
      "nativeSurfaceDcEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeSurfaceDc.primarySummarySha256,
      "nativeSurfaceDcEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeSurfaceDc.confirmationTraceSha256,
      "nativeSurfaceDcEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeSurfaceDc.confirmationSummarySha256,
      "nativeSurfaceDcEvidence.confirmationSummarySha256",
    ],
    [
      d3d9ControlledSurfaceDc.sourceSha256,
      "controlledSurfaceDcOracle.sourceSha256",
    ],
    [
      d3d9ControlledSurfaceDc.executableSha256,
      "controlledSurfaceDcOracle.executableSha256",
    ],
    [
      d3d9ControlledSurfaceDc.nullOutputExecutableSha256,
      "controlledSurfaceDcOracle.nullOutputExecutableSha256",
    ],
    [
      d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256,
      "controlledSurfaceDcOracle.readOnlyOutputExecutableSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9Clear = requireRecord(
    d3d9.clear,
    "activeRuntimeCheckpoint.d3d9Frontend.clear",
    errors,
  );
  const d3d9ClearFlagNames = requireStringArray(
    d3d9Clear.flagNames,
    "activeRuntimeCheckpoint.d3d9Frontend.clear.flagNames",
    errors,
  );
  const d3d9NativeClear = requireRecord(
    d3d9.nativeClearEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeClearEvidence",
    errors,
  );
  const d3d9NativeClearArguments = requireOrderedIntegerArray(
    d3d9NativeClear.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeClearEvidence.arguments",
    errors,
  );
  const d3d9PrivateClear = requireRecord(
    d3d9.privateRuntimeClearEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeClearEvidence",
    errors,
  );
  const d3d9PrivateClearArguments = requireOrderedStringArray(
    d3d9PrivateClear.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeClearEvidence.arguments",
    errors,
  );
  const d3d9PrivateClearPresentationGeometry = requireOrderedIntegerArray(
    d3d9PrivateClear.presentationGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeClearEvidence.presentationGeometry",
    errors,
  );
  const d3d9PrivateClearRect = requireOrderedIntegerArray(
    d3d9PrivateClear.backBufferClearRect,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeClearEvidence.backBufferClearRect",
    errors,
  );
  for (const [value, label] of [
    [d3d9NativeClear.primaryTraceSha256, "nativeClearEvidence.primaryTraceSha256"],
    [d3d9NativeClear.primarySummarySha256, "nativeClearEvidence.primarySummarySha256"],
    [
      d3d9NativeClear.confirmationTraceSha256,
      "nativeClearEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeClear.confirmationSummarySha256,
      "nativeClearEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9AddDirtyRect = requireRecord(
    d3d9.addDirtyRect,
    "activeRuntimeCheckpoint.d3d9Frontend.addDirtyRect",
    errors,
  );
  const d3d9NativeAddDirtyRect = requireRecord(
    d3d9.nativeAddDirtyRectEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAddDirtyRectEvidence",
    errors,
  );
  const d3d9PrivateAddDirtyRect = requireRecord(
    d3d9.privateRuntimeAddDirtyRectEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAddDirtyRectEvidence",
    errors,
  );
  const d3d9AddDirtyRectSourceTextureVariantIndices =
    requireOrderedIntegerArray(
      d3d9AddDirtyRect.sourceTextureVariantIndices,
      "activeRuntimeCheckpoint.d3d9Frontend.addDirtyRect.sourceTextureVariantIndices",
      errors,
    );
  const d3d9NativeAddDirtyRectPrimaryRect = requireOrderedIntegerArray(
    d3d9NativeAddDirtyRect.primaryRect,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeAddDirtyRectEvidence.primaryRect",
    errors,
  );
  const d3d9NativeAddDirtyRectValidInteriorRect =
    requireOrderedIntegerArray(
      d3d9NativeAddDirtyRect.validInteriorRect,
      "activeRuntimeCheckpoint.d3d9Frontend.nativeAddDirtyRectEvidence.validInteriorRect",
      errors,
    );
  const d3d9PrivateAddDirtyRectArguments = requireOrderedStringArray(
    d3d9PrivateAddDirtyRect.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAddDirtyRectEvidence.arguments",
    errors,
  );
  const d3d9PrivateAddDirtyRectStackPointers = requireOrderedStringArray(
    d3d9PrivateAddDirtyRect.stackPointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAddDirtyRectEvidence.stackPointers",
    errors,
  );
  const d3d9PrivateAddDirtyRectTextureGeometry = requireOrderedIntegerArray(
    d3d9PrivateAddDirtyRect.textureGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeAddDirtyRectEvidence.textureGeometry",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeAddDirtyRect.primaryTraceSha256,
      "nativeAddDirtyRectEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeAddDirtyRect.primarySummarySha256,
      "nativeAddDirtyRectEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeAddDirtyRect.confirmationTraceSha256,
      "nativeAddDirtyRectEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeAddDirtyRect.confirmationSummarySha256,
      "nativeAddDirtyRectEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9UpdateTexture = requireRecord(
    d3d9.updateTexture,
    "activeRuntimeCheckpoint.d3d9Frontend.updateTexture",
    errors,
  );
  const d3d9NativeUpdateTexture = requireRecord(
    d3d9.nativeUpdateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeUpdateTextureEvidence",
    errors,
  );
  const d3d9PrivateUpdateTexture = requireRecord(
    d3d9.privateRuntimeUpdateTextureEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeUpdateTextureEvidence",
    errors,
  );
  const d3d9UpdateTextureSourceVariantIndices = requireOrderedIntegerArray(
    d3d9UpdateTexture.sourceTextureVariantIndices,
    "activeRuntimeCheckpoint.d3d9Frontend.updateTexture.sourceTextureVariantIndices",
    errors,
  );
  const d3d9UpdateTextureDestinationVariantIndices =
    requireOrderedIntegerArray(
      d3d9UpdateTexture.destinationTextureVariantIndices,
      "activeRuntimeCheckpoint.d3d9Frontend.updateTexture.destinationTextureVariantIndices",
      errors,
    );
  const d3d9NativeUpdateTextureCreationCallerRvas =
    requireOrderedStringArray(
      d3d9NativeUpdateTexture.creationCallerRvas,
      "activeRuntimeCheckpoint.d3d9Frontend.nativeUpdateTextureEvidence.creationCallerRvas",
      errors,
    );
  const d3d9NativeUpdateTextureSuccessVariants = requireOrderedStringArray(
    d3d9NativeUpdateTexture.nonNullOracleSuccessVariants,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeUpdateTextureEvidence.nonNullOracleSuccessVariants",
    errors,
  );
  const d3d9NativeUpdateTextureInvalidVariants = requireOrderedStringArray(
    d3d9NativeUpdateTexture.nonNullOracleInvalidVariants,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeUpdateTextureEvidence.nonNullOracleInvalidVariants",
    errors,
  );
  const d3d9PrivateUpdateTextureArguments = requireOrderedStringArray(
    d3d9PrivateUpdateTexture.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeUpdateTextureEvidence.arguments",
    errors,
  );
  const d3d9PrivateUpdateTextureStackPointers = requireOrderedStringArray(
    d3d9PrivateUpdateTexture.stackPointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeUpdateTextureEvidence.stackPointers",
    errors,
  );
  const d3d9PrivateUpdateTextureFillRect = requireOrderedIntegerArray(
    d3d9PrivateUpdateTexture.logicalFillRect,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeUpdateTextureEvidence.logicalFillRect",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeUpdateTexture.primaryTraceSha256,
      "nativeUpdateTextureEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeUpdateTexture.primarySummarySha256,
      "nativeUpdateTextureEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeUpdateTexture.confirmationTraceSha256,
      "nativeUpdateTextureEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeUpdateTexture.confirmationSummarySha256,
      "nativeUpdateTextureEvidence.confirmationSummarySha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9BeginScene = requireRecord(
    d3d9.beginScene,
    "activeRuntimeCheckpoint.d3d9Frontend.beginScene",
    errors,
  );
  const d3d9NativeBeginScene = requireRecord(
    d3d9.nativeBeginSceneEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeBeginSceneEvidence",
    errors,
  );
  const d3d9PrivateBeginScene = requireRecord(
    d3d9.privateRuntimeBeginSceneEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeBeginSceneEvidence",
    errors,
  );
  const d3d9NativeBeginSceneArguments = requireOrderedStringArray(
    d3d9NativeBeginScene.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeBeginSceneEvidence.arguments",
    errors,
  );
  const d3d9NativeBeginSceneOracleExitCodes = requireOrderedIntegerArray(
    d3d9NativeBeginScene.oracleExitCodes,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeBeginSceneEvidence.oracleExitCodes",
    errors,
  );
  const d3d9NativeBeginSceneOracleDeviceReferenceCounts =
    requireOrderedIntegerArray(
      d3d9NativeBeginScene.oracleDeviceReferenceCounts,
      "activeRuntimeCheckpoint.d3d9Frontend.nativeBeginSceneEvidence.oracleDeviceReferenceCounts",
      errors,
    );
  const d3d9PrivateBeginSceneArguments = requireOrderedStringArray(
    d3d9PrivateBeginScene.arguments,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeBeginSceneEvidence.arguments",
    errors,
  );
  const d3d9PrivateBeginSceneStackPointers = requireOrderedStringArray(
    d3d9PrivateBeginScene.stackPointers,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeBeginSceneEvidence.stackPointers",
    errors,
  );
  const d3d9PrivateBeginScenePresentationGeometry = requireOrderedIntegerArray(
    d3d9PrivateBeginScene.presentationGeometry,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeBeginSceneEvidence.presentationGeometry",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeBeginScene.primaryTraceSha256,
      "nativeBeginSceneEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeBeginScene.primarySummarySha256,
      "nativeBeginSceneEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeBeginScene.confirmationTraceSha256,
      "nativeBeginSceneEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeBeginScene.confirmationSummarySha256,
      "nativeBeginSceneEvidence.confirmationSummarySha256",
    ],
    [
      d3d9NativeBeginScene.oracleSourceSha256,
      "nativeBeginSceneEvidence.oracleSourceSha256",
    ],
    [
      d3d9NativeBeginScene.oracleObjectSha256,
      "nativeBeginSceneEvidence.oracleObjectSha256",
    ],
    [
      d3d9NativeBeginScene.oracleExecutableSha256,
      "nativeBeginSceneEvidence.oracleExecutableSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  const d3d9SamplerState = requireRecord(
    d3d9.setSamplerState,
    "activeRuntimeCheckpoint.d3d9Frontend.setSamplerState",
    errors,
  );
  const d3d9NativeSamplerState = requireRecord(
    d3d9.nativeSamplerStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.nativeSamplerStateEvidence",
    errors,
  );
  const d3d9PrivateSamplerState = requireRecord(
    d3d9.privateRuntimeSamplerStateEvidence,
    "activeRuntimeCheckpoint.d3d9Frontend.privateRuntimeSamplerStateEvidence",
    errors,
  );
  for (const [value, label] of [
    [
      d3d9NativeSamplerState.primaryTraceSha256,
      "nativeSamplerStateEvidence.primaryTraceSha256",
    ],
    [
      d3d9NativeSamplerState.primarySummarySha256,
      "nativeSamplerStateEvidence.primarySummarySha256",
    ],
    [
      d3d9NativeSamplerState.confirmationTraceSha256,
      "nativeSamplerStateEvidence.confirmationTraceSha256",
    ],
    [
      d3d9NativeSamplerState.confirmationSummarySha256,
      "nativeSamplerStateEvidence.confirmationSummarySha256",
    ],
    [
      d3d9NativeSamplerState.oracleSourceSha256,
      "nativeSamplerStateEvidence.oracleSourceSha256",
    ],
    [
      d3d9NativeSamplerState.oracleObjectSha256,
      "nativeSamplerStateEvidence.oracleObjectSha256",
    ],
    [
      d3d9NativeSamplerState.oracleExecutableSha256,
      "nativeSamplerStateEvidence.oracleExecutableSha256",
    ],
  ]) {
    requireSha256(
      value,
      `activeRuntimeCheckpoint.d3d9Frontend.${label}`,
      errors,
    );
  }
  if (
    d3d9.schema !== "vine.d3d9-frontend.v1" ||
    d3d9.status !== "device-sampler-state-band-verified" ||
    d3d9.stateOwner !== "crates/vine-core/src/d3d9.rs" ||
    d3d9.comAuthority !== "runtime/com.rs::RuntimeCom" ||
    d3d9.sdkVersion !== 32 ||
    d3d9.factoryImplementationId !== "d3d9.direct3d-create9.v1" ||
    d3d9.rootClassIdentity !== "d3d9.idirect3d9-root.v1" ||
    d3d9.rootInterface !== "IDirect3D9" ||
    d3d9.rootInterfaceSlots !== 17 ||
    d3d9.rootInitialReferenceCount !== 1 ||
    d3d9.invalidSdkReturnsNullBeforeMutation !== true ||
    d3d9IUnknownMethods.join(",") !== "QueryInterface,AddRef,Release" ||
    d3d9ImplementedRootMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetAdapterCount,GetAdapterIdentifier,GetAdapterDisplayMode,GetDeviceCaps,GetAdapterMonitor,CheckDeviceType,CheckDeviceFormat,CreateDevice" ||
    d3d9.backingPages !== 2 ||
    d3d9.dataPageFinalProtection !== "read-only" ||
    d3d9.endpointPageFinalProtection !== "read-execute" ||
    d3d9.workerComResolutionAuthority !== "rust-live-endpoint-index" ||
    d3d9.browserOwnsComState !== false ||
    d3d9.versionSpecificFrontendBoundary !== true ||
    d3d9.frontendDispatchBoundary !==
      "version-neutral-com-gateway-to-versioned-d3d9-handler" ||
    d3d9CompatibilityPolicy.schema !== "vine.d3d9-compatibility-policy" ||
    d3d9CompatibilityPolicy.schemaVersion !== 22 ||
    d3d9CompatibilityPolicy.id !== "d3d9-primary-virtual-adapter-v22" ||
    d3d9CompatibilityPolicy.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    d3d9CompatibilityPolicy.surfaceDcProfileId !==
      "primary-surface-gdi-dc-v1" ||
    d3d9CompatibilityPolicy.rasterStatusProfileId !==
      "primary-stable-nonblank-scanline-v1" ||
    d3d9CompatibilityPolicy.clearProfileId !==
      "primary-full-render-target-black-v1" ||
    d3d9CompatibilityPolicy.addDirtyRectProfileId !==
      "primary-recreated-system-memory-texture-dirty-rect-v1" ||
    d3d9CompatibilityPolicy.updateTextureProfileId !==
      "primary-recreated-system-memory-to-static-default-texture-update-v1" ||
    d3d9CompatibilityPolicy.beginSceneProfileId !==
      "primary-first-frame-scene-begin-v1" ||
    d3d9CompatibilityPolicy.samplerStateProfileId !==
      "primary-first-frame-linear-sampler-states-v1" ||
    d3d9AdapterCount.slot !== 4 ||
    d3d9AdapterCount.explicitArgumentWords !== 0 ||
    d3d9AdapterCount.returnKind !== "u32" ||
    d3d9AdapterCount.profileId !== "single-logical-adapter-v1" ||
    d3d9AdapterCount.adapterCount !== 1 ||
    d3d9AdapterCount.countAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterCount.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterCount.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    d3d9AdapterCount.guestWriteBytes !== 0 ||
    d3d9AdapterCount.referenceCountMutation !== false ||
    d3d9AdapterCount.lastErrorPolicy !== "preserve" ||
    d3d9AdapterCount.hostGpuTopologyImported !== false ||
    d3d9AdapterCount.browserGpuTopologyImported !== false ||
    d3d9NativeAdapterCount.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterCount.traceBytes !== 2_267_972 ||
    d3d9NativeAdapterCount.traceLines !== 7_133 ||
    d3d9NativeAdapterCount.traceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    d3d9NativeAdapterCount.summaryBytes !== 58_021 ||
    d3d9NativeAdapterCount.summarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    d3d9NativeAdapterCount.diagnostics !== 0 ||
    d3d9NativeAdapterCount.directCallsPerTrace !== 6 ||
    d3d9NativeAdapterCount.callerRva !== "0x00059d36" ||
    d3d9NativeAdapterCount.result !== 1 ||
    d3d9NativeAdapterCount.lastError !== 0 ||
    d3d9NativeAdapterCount.consumer !==
      "store-count-in-edi-loop-adapter-indices-and-compare-monitor-identity" ||
    d3d9NativeAdapterCount.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterCount.nodeSequence !== 17_843 ||
    d3d9PrivateAdapterCount.nodeInstructionCount !== 11_807_197 ||
    d3d9PrivateAdapterCount.chromeSequence !== 17_843 ||
    d3d9PrivateAdapterCountArguments.join(",") !== "0x01000004" ||
    d3d9PrivateAdapterCount.returnAddress !== "0x10059d36" ||
    d3d9PrivateAdapterCount.result !== 1 ||
    d3d9PrivateAdapterCount.guestWriteBytes !== 0 ||
    d3d9PrivateAdapterCount.referenceCount !== 1 ||
    d3d9PrivateAdapterCount.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9AdapterMonitor.slot !== 15 ||
    d3d9AdapterMonitor.explicitArgumentWords !== 1 ||
    d3d9AdapterMonitor.returnKind !== "u32" ||
    d3d9AdapterMonitor.profileId !== "primary-adapter-monitor-v1" ||
    d3d9AdapterMonitor.adapter !== 0 ||
    d3d9AdapterMonitor.adapterMappingAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterMonitor.monitorIdentityAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterMonitor.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    d3d9AdapterMonitor.monitorHandleNamespace !== "vine.logical-monitor.v1" ||
    d3d9AdapterMonitor.primaryMonitorHandle !== "0x4f010001" ||
    d3d9AdapterMonitor.invalidAdapterResult !== "0x00000000" ||
    d3d9AdapterMonitor.guestWriteBytes !== 0 ||
    d3d9AdapterMonitor.referenceCountMutation !== false ||
    d3d9AdapterMonitor.lastErrorPolicy !== "preserve" ||
    d3d9AdapterMonitor.hostMonitorIdentityImported !== false ||
    d3d9AdapterMonitor.browserDisplayTopologyImported !== false ||
    d3d9NativeAdapterMonitor.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterMonitor.traceBytes !== 2_267_972 ||
    d3d9NativeAdapterMonitor.traceLines !== 7_133 ||
    d3d9NativeAdapterMonitor.traceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    d3d9NativeAdapterMonitor.summaryBytes !== 58_021 ||
    d3d9NativeAdapterMonitor.summarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    d3d9NativeAdapterMonitor.diagnostics !== 0 ||
    d3d9NativeAdapterMonitor.directCallsPerTrace !== 6 ||
    d3d9NativeAdapterMonitor.callerRva !== "0x00059d5e" ||
    d3d9NativeAdapterMonitor.adapter !== 0 ||
    d3d9NativeAdapterMonitor.resultOpaqueNonNull !== true ||
    d3d9NativeAdapterMonitor.lastError !== 0 ||
    d3d9NativeAdapterMonitor.oracleArchitecture !== "coff-i386" ||
    d3d9NativeAdapterMonitor.oracleSourceBytes !== 5_089 ||
    d3d9NativeAdapterMonitor.oracleSourceSha256 !==
      "337ddf83ec68ba80a25d70f1f9a9d0361a83833657ca7c852075043d16b57c67" ||
    d3d9NativeAdapterMonitor.oracleExecutableBytes !== 3_584 ||
    d3d9NativeAdapterMonitor.oracleExecutableSha256 !==
      "1d2681032fad258d9930c1a4a88ac6d4cf67d53b7d26ff447a17d5129bb436d5" ||
    d3d9NativeAdapterMonitor.oracleAdapterCount !== 1 ||
    d3d9NativeAdapterMonitorOracleAdapters.join(",") !== "0,1,4294967295" ||
    JSON.stringify(d3d9NativeAdapterMonitor.oracleResultsNonNull) !==
      JSON.stringify([true, false, false]) ||
    d3d9NativeAdapterMonitor.oraclePrimaryIdentityStable !== true ||
    d3d9NativeAdapterMonitor.oracleLastErrorSentinel !== 23_130 ||
    d3d9NativeAdapterMonitor.oracleLastErrorPreserved !== true ||
    d3d9NativeAdapterMonitor.consumer !==
      "compare-result-with-monitor-from-window-primary-identity" ||
    d3d9NativeAdapterMonitor.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterMonitorNodeSequences.join(",") !== "17844,17848" ||
    d3d9PrivateAdapterMonitorNodeInstructionCounts.join(",") !==
      "11807216,11807374" ||
    d3d9PrivateAdapterMonitorChromeSequences.join(",") !== "17844,17848" ||
    JSON.stringify(d3d9PrivateAdapterMonitor.arguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000"],
        ["0x01000004", "0x00000000"],
      ]) ||
    d3d9PrivateAdapterMonitor.returnAddresses?.join(",") !==
      "0x10059d5e,0x10059d5e" ||
    d3d9PrivateAdapterMonitor.results?.join(",") !== "0x4f010001,0x4f010001" ||
    d3d9PrivateAdapterMonitor.guestWriteBytes !== 0 ||
    d3d9PrivateAdapterMonitor.referenceCount !== 1 ||
    d3d9PrivateAdapterMonitor.matchesMonitorFromWindowIdentity !== true ||
    d3d9PrivateAdapterMonitor.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9DisplayMode.slot !== 8 ||
    d3d9DisplayMode.explicitArgumentWords !== 2 ||
    d3d9DisplayMode.returnKind !== "h-result" ||
    d3d9DisplayMode.adapter !== 0 ||
    d3d9DisplayMode.geometryAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9DisplayMode.width !== 1_920 ||
    d3d9DisplayMode.height !== 1_080 ||
    d3d9DisplayMode.refreshRateAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DisplayMode.refreshRate !== 60 ||
    d3d9DisplayMode.formatAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DisplayMode.format !== 22 ||
    d3d9DisplayMode.formatName !== "D3DFMT_X8R8G8B8" ||
    d3d9DisplayMode.outputStructure !== "D3DDISPLAYMODE" ||
    d3d9DisplayMode.outputBytes !== 16 ||
    d3d9DisplayMode.fullOutputOverwrite !== true ||
    d3d9DisplayMode.invalidAdapterOrNullOutputResult !== "0x8876086c" ||
    d3d9DisplayMode.invalidCallMutatesState !== false ||
    d3d9DisplayMode.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9DisplayMode.referenceCountMutation !== false ||
    d3d9DisplayMode.lastErrorPolicy !== "preserve" ||
    d3d9NativeDisplayMode.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDisplayMode.primaryTraceBytes !== 2_192_729 ||
    d3d9NativeDisplayMode.primaryTraceLines !== 6_886 ||
    d3d9NativeDisplayMode.primaryTraceSha256 !==
      "6b9bd17809ef235f1817fb08ccd4bfb92f27f3cd424bd0f15500470d73b05208" ||
    d3d9NativeDisplayMode.primaryBalancedApiPairs !== 3_405 ||
    d3d9NativeDisplayMode.confirmationTraceBytes !== 2_249_188 ||
    d3d9NativeDisplayMode.confirmationTraceLines !== 7_068 ||
    d3d9NativeDisplayMode.confirmationTraceSha256 !==
      "fac93ab62effd0228dc8c8a88b617f3aced5fcff9429c9e90c9347979c660f92" ||
    d3d9NativeDisplayMode.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDisplayMode.diagnostics !== 0 ||
    d3d9NativeDisplayMode.successfulCallsPerTrace !== 4 ||
    d3d9NativeDisplayModeCallerRvas.join(",") !== "0x0005b8d6,0x00059da5" ||
    d3d9NativeDisplayMode.callerCounts?.join(",") !== "1,3" ||
    d3d9NativeDisplayMode.firstCallInputZeroed !== true ||
    d3d9NativeDisplayMode.firstCallFullOverwrite !== true ||
    d3d9NativeDisplayMode.allCallsSameOutput !== true ||
    d3d9NativeDisplayMode.hresultSignConsumed !== true ||
    d3d9NativeDisplayMode.allOutputFieldsConsumed !== true ||
    d3d9NativeDisplayMode.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDisplayMode.nodeSequence !== 17_817 ||
    d3d9PrivateDisplayMode.nodeInstructionCount !== 11_806_455 ||
    d3d9PrivateDisplayMode.chromeSequence !== 17_817 ||
    d3d9PrivateDisplayModeArguments.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    d3d9PrivateDisplayMode.returnAddress !== "0x1005b8d6" ||
    d3d9PrivateDisplayMode.result !== "0x00000000" ||
    d3d9PrivateDisplayMode.outputHex !== "80070000380400003c00000016000000" ||
    d3d9PrivateDisplayMode.referenceCount !== 1 ||
    d3d9PrivateDisplayMode.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9AdapterIdentifier.slot !== 5 ||
    d3d9AdapterIdentifier.explicitArgumentWords !== 3 ||
    d3d9AdapterIdentifier.returnKind !== "h-result" ||
    d3d9AdapterIdentifier.adapter !== 0 ||
    d3d9AdapterIdentifier.flags !== 0 ||
    d3d9AdapterIdentifier.identityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9AdapterIdentifier.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9AdapterIdentifier.profileId !==
      "primary-virtual-adapter-identifier-v1" ||
    d3d9AdapterIdentifier.driver !== "vine-d3d9.dll" ||
    d3d9AdapterIdentifier.description !== "Vine Virtual Direct3D9 Adapter" ||
    d3d9AdapterIdentifier.deviceName !== "\\\\.\\DISPLAY1" ||
    d3d9AdapterIdentifier.driverVersion !== "0x0001000000000000" ||
    d3d9AdapterIdentifier.vendorId !== 0 ||
    d3d9AdapterIdentifier.deviceId !== 0 ||
    d3d9AdapterIdentifier.subSysId !== 0 ||
    d3d9AdapterIdentifier.revision !== 0 ||
    d3d9AdapterIdentifier.deviceIdentifier !==
      "56494e45-4433-4439-8000-000000000001" ||
    d3d9AdapterIdentifier.whqlLevel !== 0 ||
    d3d9AdapterIdentifier.outputStructure !== "D3DADAPTER_IDENTIFIER9" ||
    d3d9AdapterIdentifier.outputBytes !== 1_100 ||
    d3d9AdapterIdentifier.fullOutputOverwrite !== true ||
    d3d9AdapterIdentifier.invalidAdapterFlagsOrNullOutputResult !==
      "0x8876086c" ||
    d3d9AdapterIdentifier.whqlEnumerationPolicy !==
      "typed-unsupported-before-mutation" ||
    d3d9AdapterIdentifier.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9AdapterIdentifier.referenceCountMutation !== false ||
    d3d9AdapterIdentifier.hostGpuIdentityImported !== false ||
    d3d9AdapterIdentifier.lastErrorPolicy !== "preserve" ||
    d3d9NativeAdapterIdentifier.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAdapterIdentifier.primaryTraceBytes !== 2_207_106 ||
    d3d9NativeAdapterIdentifier.primaryTraceLines !== 6_888 ||
    d3d9NativeAdapterIdentifier.primaryTraceSha256 !==
      "c635bde508a727e5c42dd0d17b75d3e82e30d62f77ad06bf497133cb4f8bd7d3" ||
    d3d9NativeAdapterIdentifier.primaryBalancedApiPairs !== 3_406 ||
    d3d9NativeAdapterIdentifier.confirmationTraceBytes !== 2_263_088 ||
    d3d9NativeAdapterIdentifier.confirmationTraceLines !== 7_068 ||
    d3d9NativeAdapterIdentifier.confirmationTraceSha256 !==
      "d13217c0bc87789d61da46af87339e63bb0a92ac11dd406698a6b75982574da5" ||
    d3d9NativeAdapterIdentifier.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeAdapterIdentifier.diagnostics !== 0 ||
    d3d9NativeAdapterIdentifier.successfulCallsPerTrace !== 1 ||
    d3d9NativeAdapterIdentifier.callerRva !== "0x0005b91c" ||
    d3d9NativeAdapterIdentifier.adapter !== 0 ||
    d3d9NativeAdapterIdentifier.flags !== 0 ||
    d3d9NativeAdapterIdentifier.result !== "0x00000000" ||
    d3d9NativeAdapterIdentifier.inputZeroed !== true ||
    d3d9NativeAdapterIdentifier.inputSha256 !==
      "a805fd6f7ad225e289321ba6e2b079e92c3accc1675a7ba6626101da38f727f2" ||
    d3d9NativeAdapterIdentifier.outputSha256 !==
      "773a5d417dceeab588a8724b951f9926543e39b42fad71b58d1ba6efb6a8497c" ||
    d3d9NativeAdapterIdentifier.changedBytes !== 169 ||
    d3d9NativeAdapterIdentifier.hresultSignConsumed !== true ||
    d3d9NativeAdapterIdentifier.outputFieldsConsumed !== false ||
    d3d9NativeAdapterIdentifier.hostGpuIdentityImported !== false ||
    d3d9NativeAdapterIdentifier.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAdapterIdentifier.nodeSequence !== 17_818 ||
    d3d9PrivateAdapterIdentifier.nodeInstructionCount !== 11_806_466 ||
    d3d9PrivateAdapterIdentifier.chromeSequence !== 17_818 ||
    d3d9PrivateAdapterIdentifierArguments.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    d3d9PrivateAdapterIdentifier.returnAddress !== "0x1005b91c" ||
    d3d9PrivateAdapterIdentifier.result !== "0x00000000" ||
    d3d9PrivateAdapterIdentifier.outputAddress !== "0x10103410" ||
    d3d9PrivateAdapterIdentifier.outputBytes !== 1_100 ||
    d3d9PrivateAdapterIdentifier.outputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    d3d9PrivateAdapterIdentifier.referenceCount !== 1 ||
    d3d9PrivateAdapterIdentifier.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9DeviceCaps.slot !== 14 ||
    d3d9DeviceCaps.explicitArgumentWords !== 3 ||
    d3d9DeviceCaps.returnKind !== "h-result" ||
    d3d9DeviceCaps.adapter !== 0 ||
    d3d9DeviceCaps.deviceType !== 1 ||
    d3d9DeviceCaps.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9DeviceCaps.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9DeviceCaps.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9DeviceCaps.profileId !==
      "primary-hal-readscanline-dynamic-textures-ps3-v1" ||
    d3d9DeviceCaps.caps !== "0x00020000" ||
    d3d9DeviceCaps.capsNames?.join(",") !== "D3DCAPS_READ_SCANLINE" ||
    d3d9DeviceCaps.caps2 !== "0x20000000" ||
    d3d9DeviceCaps.caps2Names?.join(",") !== "D3DCAPS2_DYNAMICTEXTURES" ||
    d3d9DeviceCaps.pixelShaderVersion !== "0xffff0300" ||
    d3d9DeviceCaps.numberOfAdaptersInGroup !== 1 ||
    d3d9DeviceCaps.otherFieldsZero !== true ||
    d3d9DeviceCaps.outputStructure !== "D3DCAPS9" ||
    d3d9DeviceCaps.outputBytes !== 304 ||
    d3d9DeviceCaps.fullOutputOverwrite !== true ||
    d3d9DeviceCaps.invalidAdapterDeviceTypeOrNullOutputResult !==
      "0x8876086c" ||
    d3d9DeviceCaps.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9DeviceCaps.referenceCountMutation !== false ||
    d3d9DeviceCaps.hostGpuCapsImported !== false ||
    d3d9DeviceCaps.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceCaps.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceCaps.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceCaps.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceCaps.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceCaps.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceCaps.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceCaps.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceCaps.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceCaps.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceCaps.diagnostics !== 0 ||
    d3d9NativeDeviceCaps.successfulCallsPerTrace !== 1 ||
    d3d9NativeDeviceCaps.callerRva !== "0x0005b949" ||
    d3d9NativeDeviceCaps.adapter !== 0 ||
    d3d9NativeDeviceCaps.deviceType !== 1 ||
    d3d9NativeDeviceCaps.result !== "0x00000000" ||
    d3d9NativeDeviceCaps.outputBytes !== 304 ||
    d3d9NativeDeviceCaps.outputSha256 !==
      "5c48557292c6b254d6e9eb9612e3b24aae72f183bcbbcaf803d7a5ac16b41803" ||
    d3d9NativeDeviceCaps.outputEqualsCallerStackPointerAfterReturn !== true ||
    d3d9NativeDeviceCaps.hresultSignConsumed !== true ||
    d3d9NativeDeviceCapsConsumedFields.join(",") !==
      "Caps,Caps2,PixelShaderVersion" ||
    d3d9NativeDeviceCapsConsumedFieldOffsets.join(",") !== "8,12,204" ||
    d3d9NativeDeviceCapsConsumedValues.join(",") !==
      "0x00020000,0xe0010000,0xffff0300" ||
    d3d9NativeDeviceCaps.consumerInstructionCount !== 256 ||
    d3d9NativeDeviceCaps.hostGpuCapsImported !== false ||
    d3d9NativeDeviceCaps.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceCaps.nodeSequence !== 17_819 ||
    d3d9PrivateDeviceCaps.nodeInstructionCount !== 11_806_478 ||
    d3d9PrivateDeviceCaps.chromeSequence !== 17_819 ||
    d3d9PrivateDeviceCapsArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    d3d9PrivateDeviceCaps.returnAddress !== "0x1005b949" ||
    d3d9PrivateDeviceCaps.result !== "0x00000000" ||
    d3d9PrivateDeviceCaps.outputAddress !== "0x006ffe18" ||
    d3d9PrivateDeviceCaps.outputBytes !== 304 ||
    d3d9PrivateDeviceCaps.outputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    d3d9PrivateDeviceCaps.referenceCount !== 1 ||
    d3d9PrivateDeviceCaps.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CheckDeviceType.slot !== 9 ||
    d3d9CheckDeviceType.explicitArgumentWords !== 5 ||
    d3d9CheckDeviceType.returnKind !== "h-result" ||
    d3d9CheckDeviceType.adapter !== 0 ||
    d3d9CheckDeviceType.deviceType !== 1 ||
    d3d9CheckDeviceType.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CheckDeviceType.adapterFormat !== 22 ||
    d3d9CheckDeviceType.backBufferFormat !== 22 ||
    d3d9CheckDeviceType.formatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceType.windowedSupported !== true ||
    d3d9CheckDeviceType.fullscreenSupported !== true ||
    d3d9CheckDeviceType.boolNormalization !==
      "zero-fullscreen-nonzero-windowed" ||
    d3d9CheckDeviceType.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9CheckDeviceType.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CheckDeviceType.profileId !==
      "primary-hal-x8r8g8b8-windowed-fullscreen-v1" ||
    d3d9CheckDeviceType.invalidAdapterOrDeviceTypeResult !== "0x8876086c" ||
    d3d9CheckDeviceType.unsupportedFormatResult !== "0x8876086a" ||
    d3d9CheckDeviceType.guestWriteBytes !== 0 ||
    d3d9CheckDeviceType.referenceCountMutation !== false ||
    d3d9CheckDeviceType.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceType.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceType.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceType.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceType.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceType.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceType.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceType.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceType.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceType.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceType.diagnostics !== 0 ||
    d3d9NativeDeviceType.successfulCallsPerTrace !== 5 ||
    d3d9NativeDeviceType.directCallsPerTrace !== 2 ||
    d3d9NativeDeviceType.internalCallsPerTrace !== 3 ||
    d3d9NativeDeviceTypeCallerRvas.join(",") !== "0x0005b9d6,0x0005ba1d" ||
    d3d9NativeDeviceTypeWindowedValues.join(",") !== "1,0" ||
    d3d9NativeDeviceType.adapter !== 0 ||
    d3d9NativeDeviceType.deviceType !== 1 ||
    d3d9NativeDeviceType.adapterFormat !== 22 ||
    d3d9NativeDeviceType.backBufferFormat !== 22 ||
    d3d9NativeDeviceType.result !== "0x00000000" ||
    d3d9NativeDeviceType.hresultSignConsumed !== true ||
    d3d9NativeDeviceType.consumerOracle !==
      "immutable-pe-disassembly-and-device-caps-consumer-snapshot" ||
    d3d9NativeDeviceType.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceTypeNodeSequences.join(",") !== "17820,17821" ||
    d3d9PrivateDeviceTypeInstructionCounts.join(",") !== "11806497,11806510" ||
    d3d9PrivateDeviceTypeChromeSequences.join(",") !== "17820,17821" ||
    JSON.stringify(d3d9PrivateDeviceType.arguments) !==
      JSON.stringify([
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000001",
        ],
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000000",
        ],
      ]) ||
    d3d9PrivateDeviceType.returnAddresses?.join(",") !==
      "0x1005b9d6,0x1005ba1d" ||
    d3d9PrivateDeviceType.results?.join(",") !== "0x00000000,0x00000000" ||
    d3d9PrivateDeviceType.guestWriteBytes !== 0 ||
    d3d9PrivateDeviceType.referenceCount !== 1 ||
    d3d9PrivateDeviceType.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CheckDeviceFormat.slot !== 10 ||
    d3d9CheckDeviceFormat.explicitArgumentWords !== 6 ||
    d3d9CheckDeviceFormat.returnKind !== "h-result" ||
    d3d9CheckDeviceFormat.adapter !== 0 ||
    d3d9CheckDeviceFormat.deviceType !== 1 ||
    d3d9CheckDeviceFormat.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CheckDeviceFormat.adapterFormat !== 22 ||
    d3d9CheckDeviceFormat.adapterFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    d3d9CheckDeviceFormatSupportedUsageNames.join(",") !==
      "none,D3DUSAGE_DYNAMIC" ||
    d3d9CheckDeviceFormat.resourceType !== 3 ||
    d3d9CheckDeviceFormat.resourceTypeName !== "D3DRTYPE_TEXTURE" ||
    d3d9CheckDeviceFormat.checkFormat !== 22 ||
    d3d9CheckDeviceFormat.checkFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CheckDeviceFormat.capabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9CheckDeviceFormat.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CheckDeviceFormat.profileId !==
      "primary-hal-x8r8g8b8-texture-basic-dynamic-v1" ||
    d3d9CheckDeviceFormat.invalidAdapterOrDeviceTypeResult !== "0x8876086c" ||
    d3d9CheckDeviceFormat.unsupportedCombinationResult !== "0x8876086a" ||
    d3d9CheckDeviceFormat.guestWriteBytes !== 0 ||
    d3d9CheckDeviceFormat.referenceCountMutation !== false ||
    d3d9CheckDeviceFormat.lastErrorPolicy !== "preserve" ||
    d3d9NativeDeviceFormat.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeDeviceFormat.primaryTraceBytes !== 2_274_737 ||
    d3d9NativeDeviceFormat.primaryTraceLines !== 7_066 ||
    d3d9NativeDeviceFormat.primaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    d3d9NativeDeviceFormat.primaryBalancedApiPairs !== 3_495 ||
    d3d9NativeDeviceFormat.confirmationTraceBytes !== 2_282_110 ||
    d3d9NativeDeviceFormat.confirmationTraceLines !== 7_068 ||
    d3d9NativeDeviceFormat.confirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    d3d9NativeDeviceFormat.confirmationBalancedApiPairs !== 3_496 ||
    d3d9NativeDeviceFormat.diagnostics !== 0 ||
    d3d9NativeDeviceFormat.callsPerTrace !== 13 ||
    d3d9NativeDeviceFormat.directCallsPerTrace !== 1 ||
    d3d9NativeDeviceFormat.createDeviceParentedCallsPerTrace !== 3 ||
    d3d9NativeDeviceFormat.otherInternalCallsPerTrace !== 9 ||
    d3d9NativeDeviceFormat.directCallerRva !== "0x0005ba69" ||
    d3d9NativeDeviceFormat.adapter !== 0 ||
    d3d9NativeDeviceFormat.deviceType !== 1 ||
    d3d9NativeDeviceFormat.adapterFormat !== 22 ||
    d3d9NativeDeviceFormat.resourceType !== 3 ||
    d3d9NativeDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    d3d9NativeDeviceFormat.supportedCheckFormat !== 22 ||
    d3d9NativeDeviceFormat.basicUsageSuccessCallsPerTrace !== 6 ||
    d3d9NativeDeviceFormat.dynamicUsageSuccessCallsPerTrace !== 4 ||
    d3d9NativeDeviceFormat.unsupportedCheckFormat !== 118 ||
    d3d9NativeDeviceFormat.unsupportedCallsPerTrace !== 3 ||
    d3d9NativeDeviceFormat.unsupportedResult !== "0x8876086a" ||
    d3d9NativeDeviceFormat.directUsage !== 512 ||
    d3d9NativeDeviceFormat.directCheckFormat !== 22 ||
    d3d9NativeDeviceFormat.directResult !== "0x00000000" ||
    d3d9NativeDeviceFormat.hresultSignConsumed !== true ||
    d3d9NativeDeviceFormat.consumerOracle !== "immutable-pe-disassembly" ||
    d3d9NativeDeviceFormat.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateDeviceFormat.nodeSequence !== 17_822 ||
    d3d9PrivateDeviceFormat.nodeInstructionCount !== 11_806_524 ||
    d3d9PrivateDeviceFormat.chromeSequence !== 17_822 ||
    d3d9PrivateDeviceFormatArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    d3d9PrivateDeviceFormat.returnAddress !== "0x1005ba69" ||
    d3d9PrivateDeviceFormat.result !== "0x00000000" ||
    d3d9PrivateDeviceFormat.guestWriteBytes !== 0 ||
    d3d9PrivateDeviceFormat.referenceCount !== 1 ||
    d3d9PrivateDeviceFormat.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateDevice.slot !== 16 ||
    d3d9CreateDevice.explicitArgumentWords !== 6 ||
    d3d9CreateDevice.returnKind !== "h-result" ||
    d3d9CreateDevice.profileId !== "primary-windowed-hal-device-v1" ||
    d3d9CreateDevice.adapter !== 0 ||
    d3d9CreateDevice.deviceType !== 1 ||
    d3d9CreateDevice.deviceTypeName !== "D3DDEVTYPE_HAL" ||
    d3d9CreateDevice.behaviorFlags !== "0x00000044" ||
    d3d9CreateDeviceBehaviorFlagNames.join(",") !==
      "D3DCREATE_MULTITHREADED,D3DCREATE_HARDWARE_VERTEXPROCESSING" ||
    d3d9CreateDevice.focusWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    d3d9CreateDevice.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9CreateDevice.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    d3d9CreateDevice.presentationStructure !== "D3DPRESENT_PARAMETERS" ||
    d3d9CreateDevice.presentationBytes !== 56 ||
    d3d9CreateDevice.requestedBackBufferWidth !== 0 ||
    d3d9CreateDevice.requestedBackBufferHeight !== 0 ||
    d3d9CreateDevice.requestedBackBufferFormat !== 0 ||
    d3d9CreateDevice.requestedBackBufferFormatName !== "D3DFMT_UNKNOWN" ||
    d3d9CreateDevice.resolvedGeometryAuthority !==
      "live-window-client-geometry" ||
    d3d9CreateDevice.resolvedBackBufferFormat !== 22 ||
    d3d9CreateDevice.resolvedBackBufferFormatName !== "D3DFMT_X8R8G8B8" ||
    d3d9CreateDevice.backBufferCount !== 1 ||
    d3d9CreateDevice.multiSampleType !== 0 ||
    d3d9CreateDevice.multiSampleQuality !== 0 ||
    d3d9CreateDevice.swapEffect !== 3 ||
    d3d9CreateDevice.swapEffectName !== "D3DSWAPEFFECT_DISCARD" ||
    d3d9CreateDevice.deviceWindowMatchesFocusWindow !== true ||
    d3d9CreateDevice.windowed !== true ||
    d3d9CreateDevice.enableAutoDepthStencil !== false ||
    d3d9CreateDevice.autoDepthStencilFormat !== 80 ||
    d3d9CreateDevice.autoDepthStencilFormatName !== "D3DFMT_D16" ||
    d3d9CreateDevice.flags !== 0 ||
    d3d9CreateDevice.fullScreenRefreshRateInHz !== 0 ||
    d3d9CreateDevice.presentationInterval !== 0 ||
    d3d9CreateDevice.presentationPrefixWriteBytes !== 12 ||
    d3d9CreateDevice.deviceOutputWriteBytes !== 4 ||
    d3d9CreateDevice.guestWritesAndObjectPublicationAtomic !== true ||
    d3d9CreateDevice.invalidIdentityOrPointerResult !== "0x8876086c" ||
    d3d9CreateDevice.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateDevice.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateDevice.lastErrorPolicy !== "preserve" ||
    d3d9DeviceInterface.classIdentity !== "d3d9.idirect3ddevice9.v1" ||
    d3d9DeviceInterface.interface !== "IDirect3DDevice9" ||
    d3d9DeviceInterface.vtableSlots !== 119 ||
    d3d9DeviceImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,CreateTexture,CreateVertexBuffer,SetRenderState" ||
    d3d9DeviceInterface.initialReferenceCount !== 1 ||
    d3d9DeviceInterface.retainsRootInterfaceReference !== true ||
    d3d9DeviceInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9DeviceInterface.finalReleaseMayCascadeRootRetirement !== true ||
    runtimeGraphics.stateOwner !== "runtime/graphics.rs::RuntimeGraphics" ||
    runtimeGraphics.aggregateCoordinator !==
      "runtime/kernel.rs::RuntimeKernel" ||
    runtimeGraphics.deviceNamespace !== "vine.logical-graphics-device.v1" ||
    runtimeGraphics.deviceCapacity !== 4_096 ||
    runtimeGraphics.textureNamespace !== "vine.logical-graphics-texture.v1" ||
    runtimeGraphics.textureCapacity !== 4_096 ||
    runtimeGraphics.surfaceNamespace !== "vine.logical-graphics-surface.v1" ||
    runtimeGraphics.surfaceCapacity !== 4_096 ||
    runtimeGraphics.vertexBufferNamespace !==
      "vine.logical-graphics-vertex-buffer.v1" ||
    runtimeGraphics.vertexBufferCapacity !== 4_096 ||
    runtimeGraphics.frontend !== "d3d9" ||
    runtimeGraphics.windowRelation !==
      "non-owning-revalidated-live-window-id-and-owner-thread" ||
    runtimeGraphics.comRelation !== "non-owning-retired-with-com-object" ||
    runtimeGraphics.textureDeviceRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.surfaceTextureRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.vertexBufferDeviceRelation !==
      "owner-index-backed-by-com-parent-reference" ||
    runtimeGraphics.vertexBufferContentAuthority !==
      "rust-guest-memory-backing" ||
    runtimeGraphics.vertexBufferLockAuthority !==
      "runtime-graphics-single-active-lock" ||
    runtimeGraphics.deviceRenderStateAuthority !==
      "runtime-graphics-device-owned-typed-state" ||
    runtimeGraphics.deviceRenderStateShape !==
      "explicit-reached-values-without-inferred-windows-defaults" ||
    runtimeGraphics.deviceRenderStateMutation !==
      "owner-plan-preflight-stale-check-targeted-commit" ||
    runtimeGraphics.deviceRenderStateSameValuePolicy !==
      "success-without-generation-change" ||
    runtimeGraphics.surfaceFillCapacity !== 16_384 ||
    runtimeGraphics.surfaceContentAuthority !==
      "bounded-fill-command-history" ||
    runtimeGraphics.surfaceContentGeneration !== true ||
    runtimeGraphics.mutation !== "owner-plan-stale-check-targeted-commit" ||
    runtimeGraphics.processExitTeardown !== true ||
    runtimeGraphics.browserRendererNamespace !==
      "vine.browser-renderer-resource.v1" ||
    runtimeGraphics.browserRendererAssociationOwnership !== "non-owning" ||
    runtimeGraphics.browserOwnsLogicalGraphicsState !== false ||
    d3d9NativeCreateDevice.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeCreateDevice.primaryTraceBytes !== 2_333_677 ||
    d3d9NativeCreateDevice.primaryTraceLines !== 7_075 ||
    d3d9NativeCreateDevice.primaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    d3d9NativeCreateDevice.primaryBalancedApiPairs !== 3_498 ||
    d3d9NativeCreateDevice.primarySummaryBytes !== 275_843 ||
    d3d9NativeCreateDevice.primarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    d3d9NativeCreateDevice.confirmationTraceBytes !== 2_355_307 ||
    d3d9NativeCreateDevice.confirmationTraceLines !== 7_121 ||
    d3d9NativeCreateDevice.confirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    d3d9NativeCreateDevice.confirmationBalancedApiPairs !== 3_521 ||
    d3d9NativeCreateDevice.confirmationSummaryBytes !== 275_855 ||
    d3d9NativeCreateDevice.confirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    d3d9NativeCreateDevice.diagnostics !== 0 ||
    d3d9NativeCreateDevice.successfulCallsPerTrace !== 3 ||
    d3d9NativeCreateDevice.callerRva !== "0x0005aca5" ||
    d3d9NativeCreateDevice.adapter !== 0 ||
    d3d9NativeCreateDevice.deviceType !== 1 ||
    d3d9NativeCreateDevice.behaviorFlags !== "0x00000044" ||
    d3d9NativeCreateDeviceInput.join(",") !== "0,0,0" ||
    d3d9NativeCreateDevice.preservedPresentationFields?.join(",") !==
      "1,0,0,3,1,0,80,0,0,0" ||
    JSON.stringify(d3d9NativeCreateDevice.resolvedWidthHeightFormat) !==
      JSON.stringify([
        [800, 600, 22],
        [1280, 720, 22],
        [1280, 720, 22],
      ]) ||
    d3d9NativeCreateDevice.result !== "0x00000000" ||
    d3d9NativeCreateDevice.lastError !== 0 ||
    d3d9NativeCreateDevice.deviceOutputNonNull !== true ||
    d3d9NativeCreateDevice.hresultSignConsumed !== true ||
    d3d9NativeCreateDevice.firstReachedDeviceMethod !== "CreateTexture" ||
    d3d9NativeCreateDevice.firstReachedDeviceSlot !== 23 ||
    d3d9NativeCreateDevice.nestedDeviceAddRefResult !== 2 ||
    d3d9NativeCreateDevice.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateCreateDevice.nodeSequence !== 17_849 ||
    d3d9PrivateCreateDevice.nodeInstructionCount !== 11_807_387 ||
    d3d9PrivateCreateDevice.chromeSequence !== 17_849 ||
    d3d9PrivateCreateDeviceArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    d3d9PrivateCreateDevice.returnAddress !== "0x1005aca5" ||
    d3d9PrivateCreateDevice.result !== "0x00000000" ||
    d3d9PrivateCreateDevice.presentationAddress !== "0x101038f0" ||
    d3d9PrivateCreateDevice.presentationPrefixHex !==
      "200300005802000016000000" ||
    d3d9PrivateCreateDevice.deviceOutputAddress !== "0x101517c4" ||
    d3d9PrivateCreateDevice.devicePointer !== "0x01010004" ||
    d3d9PrivateCreateDevice.guestWriteBytes !== 16 ||
    d3d9PrivateCreateDevice.deviceObjectId !== 3 ||
    d3d9PrivateCreateDevice.parentInterfaceId !== 4 ||
    d3d9PrivateCreateDevice.deviceInterfaceId !== 6 ||
    d3d9PrivateCreateDevice.deviceVtable !== "0x0101001c" ||
    d3d9PrivateCreateDevice.deviceVtableSlots !== 119 ||
    d3d9PrivateCreateDevice.rootReferenceCount !== 2 ||
    d3d9PrivateCreateDevice.deviceReferenceCount !== 1 ||
    d3d9PrivateCreateDevice.logicalGraphicsDeviceId !== 1 ||
    d3d9PrivateCreateDevice.logicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    d3d9PrivateLogicalPresentation.join(",") !== "800,600,22,1,1,3,1" ||
    d3d9PrivateCreateDevice.browserRendererCreated !== false ||
    d3d9PrivateCreateDevice.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateTexture.slot !== 23 ||
    d3d9CreateTexture.explicitArgumentWords !== 8 ||
    d3d9CreateTexture.returnKind !== "h-result" ||
    d3d9CreateTexture.profileId !==
      "primary-initial-and-recreated-x8r8g8b8-textures-v2" ||
    JSON.stringify(
      d3d9CreateTextureVariants.map((variant) => [
        variant.width,
        variant.height,
        variant.levels,
        variant.usage,
        variant.usageName,
        variant.format,
        variant.formatName,
        variant.pool,
        variant.poolName,
      ]),
    ) !==
      JSON.stringify([
        [
          1_024,
          1_024,
          1,
          0,
          "none",
          22,
          "D3DFMT_X8R8G8B8",
          2,
          "D3DPOOL_SYSTEMMEM",
        ],
        [
          1_024,
          1_024,
          1,
          0,
          "none",
          22,
          "D3DFMT_X8R8G8B8",
          0,
          "D3DPOOL_DEFAULT",
        ],
        [
          1_024,
          1_024,
          1,
          512,
          "dynamic",
          22,
          "D3DFMT_X8R8G8B8",
          0,
          "D3DPOOL_DEFAULT",
        ],
        [
          2_048,
          1_024,
          1,
          0,
          "none",
          22,
          "D3DFMT_X8R8G8B8",
          2,
          "D3DPOOL_SYSTEMMEM",
        ],
        [
          2_048,
          1_024,
          1,
          0,
          "none",
          22,
          "D3DFMT_X8R8G8B8",
          0,
          "D3DPOOL_DEFAULT",
        ],
        [
          2_048,
          1_024,
          1,
          512,
          "dynamic",
          22,
          "D3DFMT_X8R8G8B8",
          0,
          "D3DPOOL_DEFAULT",
        ],
      ]) ||
    d3d9CreateTexture.sharedHandlePointer !== "NULL" ||
    d3d9CreateTexture.textureOutputWriteBytes !== 4 ||
    d3d9CreateTexture.guestComGraphicsPublicationAtomic !== true ||
    d3d9CreateTexture.invalidArgumentOrPointerResult !== "0x8876086c" ||
    d3d9CreateTexture.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateTexture.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateTexture.lastErrorPolicy !== "preserve" ||
    d3d9TextureInterface.classIdentity !== "d3d9.idirect3dtexture9.v1" ||
    d3d9TextureInterface.interface !== "IDirect3DTexture9" ||
    d3d9TextureInterface.vtableSlots !== 22 ||
    d3d9TextureImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetSurfaceLevel" ||
    d3d9TextureInterface.initialReferenceCount !== 1 ||
    d3d9TextureInterface.retainsDeviceInterfaceReference !== true ||
    d3d9TextureInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9TextureInterface.finalReleaseMayCascadeDeviceAndRootRetirement !==
      true ||
    d3d9NativeCreateTexture.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeCreateTexture.primaryTraceBytes !== 2_333_677 ||
    d3d9NativeCreateTexture.primaryTraceLines !== 7_075 ||
    d3d9NativeCreateTexture.primaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    d3d9NativeCreateTexture.primaryBalancedApiPairs !== 3_498 ||
    d3d9NativeCreateTexture.primarySummaryBytes !== 275_843 ||
    d3d9NativeCreateTexture.primarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    d3d9NativeCreateTexture.confirmationTraceBytes !== 2_355_307 ||
    d3d9NativeCreateTexture.confirmationTraceLines !== 7_121 ||
    d3d9NativeCreateTexture.confirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    d3d9NativeCreateTexture.confirmationBalancedApiPairs !== 3_521 ||
    d3d9NativeCreateTexture.confirmationSummaryBytes !== 275_855 ||
    d3d9NativeCreateTexture.confirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    d3d9NativeCreateTexture.diagnostics !== 0 ||
    d3d9NativeCreateTexture.successfulCallsPerTrace !== 1 ||
    d3d9NativeCreateTexture.callerRva !== "0x0005a715" ||
    d3d9NativeCreateTextureArguments.join(",") !== "1024,1024,1,0,22,2" ||
    d3d9NativeCreateTexture.sharedHandleNull !== true ||
    d3d9NativeCreateTexture.result !== "0x00000000" ||
    d3d9NativeCreateTexture.lastError !== 0 ||
    d3d9NativeCreateTexture.hresultSignConsumed !== true ||
    d3d9NativeCreateTexture.textureOutputSnapshotCaptured !== false ||
    d3d9NativeCreateTexture.nativeTexturePointerClaimed !== false ||
    d3d9NativeCreateTexture.immediateNextMethod !== "GetSurfaceLevel" ||
    d3d9NativeCreateTexture.immediateNextSlot !== 18 ||
    d3d9NativeCreateTexture.immediateNextLevel !== 0 ||
    d3d9NativeCreateTexture.sourceExecutableHashUnchanged !== true ||
    d3d9NativeInitialTextures.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialTextures.profileId !== "x86-bgi-initial-textures-v1" ||
    d3d9NativeInitialTextures.primaryTraceBytes !== 261_943 ||
    d3d9NativeInitialTextures.primaryTraceLines !== 505 ||
    d3d9NativeInitialTextures.primaryTraceSha256 !==
      "2fe16a38d7a8c8ee18b673d06f759ef3bfa3d412d3f688689692edb4d6d55ad4" ||
    d3d9NativeInitialTextures.primaryBalancedApiPairs !== 219 ||
    d3d9NativeInitialTextures.primarySummaryBytes !== 328_531 ||
    d3d9NativeInitialTextures.primarySummarySha256 !==
      "85db2bcea0fb895c1481a3af7330a3c79b430cbed997761cdb23d30e0fbf875d" ||
    d3d9NativeInitialTextures.confirmationTraceBytes !== 262_766 ||
    d3d9NativeInitialTextures.confirmationTraceLines !== 508 ||
    d3d9NativeInitialTextures.confirmationTraceSha256 !==
      "cdf646718a1a187e72129d9ae5562d44b4c5432c7c9c60ee1c217d30ae90179f" ||
    d3d9NativeInitialTextures.confirmationBalancedApiPairs !== 220 ||
    d3d9NativeInitialTextures.confirmationSummaryBytes !== 328_698 ||
    d3d9NativeInitialTextures.confirmationSummarySha256 !==
      "20eb05e775a84075aec1c48b2137c9d237f684e2e19d580b1aac5671a7039837" ||
    d3d9NativeInitialTextures.diagnostics !== 0 ||
    d3d9NativeInitialTextures.callsPerTrace !== 3 ||
    d3d9NativeInitialTextureCallerRvas.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    JSON.stringify(d3d9NativeInitialTextures.variants) !==
      JSON.stringify([
        [1_024, 1_024, 1, 0, 22, 2],
        [1_024, 1_024, 1, 0, 22, 0],
        [1_024, 1_024, 1, 512, 22, 0],
      ]) ||
    d3d9NativeInitialTextures.sharedHandlesNull !== true ||
    d3d9NativeInitialTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialTextureLastErrors.join(",") !== "0,0,0" ||
    d3d9NativeInitialTextures.sameDevicePerTrace !== true ||
    d3d9NativeInitialTextureOutputAddresses.join(",") !==
      "0x00d117cc,0x00d117d0,0x00d117d4" ||
    d3d9NativeInitialTextures.returnedTexturesNonNullAndDistinctPerTrace !==
      true ||
    d3d9NativeInitialTextures.nativeTexturePointersAreOpaqueArtifactIdentity !==
      true ||
    d3d9NativeInitialTextures.immediateNextMethod !== "CreateVertexBuffer" ||
    d3d9NativeInitialTextures.immediateNextSlot !== 26 ||
    d3d9NativeInitialTextureNextArguments.join(",") !== "112,8,324,1" ||
    d3d9NativeInitialTextures.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateCreateTexture.nodeSequence !== 17_850 ||
    d3d9PrivateCreateTexture.nodeInstructionCount !== 11_807_772 ||
    d3d9PrivateCreateTexture.chromeSequence !== 17_850 ||
    d3d9PrivateCreateTextureArguments.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    d3d9PrivateCreateTexture.returnAddress !== "0x1005a715" ||
    d3d9PrivateCreateTexture.result !== "0x00000000" ||
    d3d9PrivateCreateTexture.textureOutputAddress !== "0x101517cc" ||
    d3d9PrivateCreateTexture.texturePointer !== "0x01020004" ||
    d3d9PrivateCreateTexture.guestWriteBytes !== 4 ||
    d3d9PrivateCreateTexture.textureObjectId !== 4 ||
    d3d9PrivateCreateTexture.parentInterfaceId !== 6 ||
    d3d9PrivateCreateTexture.textureInterfaceId !== 8 ||
    d3d9PrivateCreateTexture.textureVtable !== "0x0102001c" ||
    d3d9PrivateCreateTexture.textureVtableSlots !== 22 ||
    d3d9PrivateCreateTexture.deviceReferenceCount !== 2 ||
    d3d9PrivateCreateTexture.textureReferenceCount !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsDeviceId !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureId !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureNamespace !==
      "vine.logical-graphics-texture.v1" ||
    d3d9PrivateTextureDescriptor.join(",") !== "1024,1024,1,0,22,2" ||
    d3d9PrivateCreateTexture.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateCreateTexture.logicalGraphicsTextureCount !== 1 ||
    d3d9PrivateCreateTexture.browserRendererCreated !== false ||
    d3d9PrivateCreateTexture.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9PrivateInitialTextureNodeSequences.join(",") !== "17850,17857,17858" ||
    d3d9PrivateInitialTextureInstructionCounts.join(",") !==
      "11807772,11807840,11807856" ||
    d3d9PrivateInitialTextureChromeSequences.join(",") !==
      "17850,17857,17858" ||
    d3d9PrivateInitialTextureReturnAddresses.join(",") !==
      "0x1005a715,0x1005a7b7,0x1005a7dc" ||
    JSON.stringify(d3d9PrivateInitialTextures.arguments) !==
      JSON.stringify([
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000002",
          "0x101517cc",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000000",
          "0x101517d0",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000200",
          "0x00000016",
          "0x00000000",
          "0x101517d4",
          "0x00000000",
        ],
      ]) ||
    d3d9PrivateInitialTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialTextureOutputAddresses.join(",") !==
      "0x101517cc,0x101517d0,0x101517d4" ||
    d3d9PrivateInitialTexturePointers.join(",") !==
      "0x01020004,0x01030004,0x01040004" ||
    d3d9PrivateInitialTextureGuestWrites.join(",") !== "4,4,4" ||
    JSON.stringify(d3d9PrivateInitialTextures.logicalDescriptors) !==
      JSON.stringify([
        [1_024, 1_024, 1, 0, 22, 2],
        [1_024, 1_024, 1, 0, 22, 0],
        [1_024, 1_024, 1, 512, 22, 0],
      ]) ||
    d3d9PrivateInitialTextures.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateInitialTextures.logicalGraphicsTextureCount !== 3 ||
    d3d9PrivateInitialTextures.deviceReferenceCountAtStop !== 4 ||
    d3d9PrivateInitialTextures.browserRendererCreated !== false ||
    d3d9PrivateInitialTextures.pixelStorageCreated !== false ||
    d3d9PrivateInitialTextures.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9CreateVertexBuffer.slot !== 26 ||
    d3d9CreateVertexBuffer.explicitArgumentWords !== 6 ||
    d3d9CreateVertexBuffer.returnKind !== "h-result" ||
    d3d9CreateVertexBuffer.profileId !==
      "primary-initial-xyzw-diffuse-tex1-quad-v1" ||
    d3d9CreateVertexBuffer.byteLength !== 112 ||
    d3d9CreateVertexBuffer.usage !== 8 ||
    d3d9CreateVertexBuffer.usageName !== "D3DUSAGE_WRITEONLY" ||
    d3d9CreateVertexBuffer.flexibleVertexFormat !== 324 ||
    d3d9CreateVertexBuffer.flexibleVertexFormatName !==
      "D3DFVF_XYZRHW|D3DFVF_DIFFUSE|D3DFVF_TEX1" ||
    d3d9CreateVertexBuffer.pool !== 1 ||
    d3d9CreateVertexBuffer.poolName !== "D3DPOOL_MANAGED" ||
    d3d9CreateVertexBuffer.sharedHandlePointer !== "NULL" ||
    d3d9CreateVertexBuffer.vertexBufferOutputWriteBytes !== 4 ||
    d3d9CreateVertexBuffer.initialContent !== "zeroed" ||
    d3d9CreateVertexBuffer.initialBackingProtection !== "read-only" ||
    d3d9CreateVertexBuffer.guestComAddressGraphicsMemoryPublicationAtomic !==
      true ||
    d3d9CreateVertexBuffer.invalidArgumentOrPointerResult !== "0x8876086c" ||
    d3d9CreateVertexBuffer.unsupportedVariantResult !== "0x8876086a" ||
    d3d9CreateVertexBuffer.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9CreateVertexBuffer.lastErrorPolicy !== "preserve" ||
    d3d9VertexBufferInterface.classIdentity !==
      "d3d9.idirect3dvertexbuffer9.v1" ||
    d3d9VertexBufferInterface.interface !== "IDirect3DVertexBuffer9" ||
    d3d9VertexBufferInterface.iid !== "b64bb1b5-fd70-4df6-bf91-19d0a12455e3" ||
    d3d9VertexBufferInterface.vtableSlots !== 14 ||
    d3d9VertexBufferImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,Lock,Unlock" ||
    d3d9VertexBufferInterface.initialReferenceCount !== 1 ||
    d3d9VertexBufferInterface.retainsDeviceInterfaceReference !== true ||
    d3d9VertexBufferInterface.finalReleaseRetiresComBackingGraphicsBackingAndAddressRecords !==
      true ||
    d3d9VertexBufferInterface.finalReleaseMayCascadeDeviceAndRootRetirement !==
      true ||
    d3d9VertexBufferLock.slot !== 11 ||
    d3d9VertexBufferLock.explicitArgumentWords !== 4 ||
    d3d9VertexBufferLock.returnKind !== "h-result" ||
    d3d9VertexBufferLock.supportedOffset !== 0 ||
    d3d9VertexBufferLock.supportedSize !== 0 ||
    d3d9VertexBufferLock.supportedFlags !== 0 ||
    d3d9VertexBufferLock.zeroSizeMeansRemainingBytes !== true ||
    d3d9VertexBufferLock.outputWriteBytes !== 4 ||
    d3d9VertexBufferLock.lockProtection !== "read-write" ||
    d3d9VertexBufferLock.duplicateOrUnsupportedLockResult !== "0x8876086c" ||
    d3d9VertexBufferLock.guestWriteAndProtectionTransitionAtomic !== true ||
    d3d9VertexBufferLock.lastErrorPolicy !== "preserve" ||
    d3d9VertexBufferUnlock.slot !== 12 ||
    d3d9VertexBufferUnlock.explicitArgumentWords !== 0 ||
    d3d9VertexBufferUnlock.returnKind !== "h-result" ||
    d3d9VertexBufferUnlock.unlockedProtection !== "read-only" ||
    d3d9VertexBufferUnlock.incrementsContentGeneration !== true ||
    d3d9VertexBufferUnlock.unlockWithoutLockResult !== "0x8876086c" ||
    d3d9VertexBufferUnlock.graphicsAndProtectionTransitionAtomic !== true ||
    d3d9VertexBufferUnlock.lastErrorPolicy !== "preserve" ||
    d3d9NativeInitialVertexBuffer.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialVertexBuffer.profileId !==
      "x86-bgi-initial-vertex-buffer-v1" ||
    d3d9NativeInitialVertexBuffer.primaryTraceBytes !== 286_932 ||
    d3d9NativeInitialVertexBuffer.primaryTraceLines !== 512 ||
    d3d9NativeInitialVertexBuffer.primaryTraceSha256 !==
      "0a1719739f86eaa42353e0c762e802119225e30975a318a29669e9aea2ea5c30" ||
    d3d9NativeInitialVertexBuffer.primaryBalancedApiPairs !== 222 ||
    d3d9NativeInitialVertexBuffer.primarySummaryBytes !== 282_623 ||
    d3d9NativeInitialVertexBuffer.primarySummarySha256 !==
      "68133eb819c3055d58580f2bc63dff5b56c116aff6d5ff9762c4428108e1c849" ||
    d3d9NativeInitialVertexBuffer.confirmationTraceBytes !== 285_889 ||
    d3d9NativeInitialVertexBuffer.confirmationTraceLines !== 509 ||
    d3d9NativeInitialVertexBuffer.confirmationTraceSha256 !==
      "e2f7c1cc1d9c073d9325bf05d7f46a44e80bbbce54b0f8ffe08684f6bf393ed8" ||
    d3d9NativeInitialVertexBuffer.confirmationBalancedApiPairs !== 220 ||
    d3d9NativeInitialVertexBuffer.confirmationSummaryBytes !== 282_874 ||
    d3d9NativeInitialVertexBuffer.confirmationSummarySha256 !==
      "7835288860d359c695d98bb7cbc19ecbc004f1d05ec6f54d384eaa7356ec3700" ||
    d3d9NativeInitialVertexBuffer.diagnostics !== 0 ||
    d3d9NativeInitialVertexBuffer.callsPerTrace !== 3 ||
    d3d9NativeInitialVertexBufferCallerRvas.join(",") !==
      "0x0005a890,0x0005aa96,0x0005aab2" ||
    d3d9NativeInitialVertexBufferMethods.join(",") !==
      "CreateVertexBuffer,Lock,Unlock" ||
    d3d9NativeInitialVertexBufferSlots.join(",") !== "26,11,12" ||
    d3d9NativeInitialVertexBufferCreateArguments.join(",") !== "112,8,324,1" ||
    d3d9NativeInitialVertexBuffer.createOutputInitiallyNull !== true ||
    d3d9NativeInitialVertexBuffer.createOutputNonNull !== true ||
    d3d9NativeInitialVertexBufferLockArguments.join(",") !== "0,0,0" ||
    d3d9NativeInitialVertexBuffer.lockDataPointerNonNull !== true ||
    d3d9NativeInitialVertexBuffer.initialContentBytes !== 112 ||
    d3d9NativeInitialVertexBuffer.initialContentSha256 !==
      "b5fdab78d8947eacc864bfeecb4d2100780e5afe1cd8efafb124887913ac49fa" ||
    d3d9NativeInitialVertexBuffer.writtenContentBytes !== 112 ||
    d3d9NativeInitialVertexBuffer.writtenContentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    d3d9NativeInitialVertexBufferResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialVertexBufferLastErrors.join(",") !== "0,0,0" ||
    d3d9NativeInitialVertexBuffer.sameDeviceAndVertexBufferPerTrace !== true ||
    d3d9NativeInitialVertexBuffer.nativePointersAreOpaqueArtifactIdentity !==
      true ||
    d3d9NativeInitialVertexBuffer.immediateNextMethod !== "SetRenderState" ||
    d3d9NativeInitialVertexBuffer.immediateNextSlot !== 57 ||
    d3d9NativeInitialVertexBufferNextArguments.join(",") !== "22,1" ||
    d3d9NativeInitialVertexBuffer.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateInitialVertexBufferNodeSequences.join(",") !==
      "17859,17860,17861" ||
    d3d9PrivateInitialVertexBufferInstructionCounts.join(",") !==
      "11807897,11808429,11808439" ||
    d3d9PrivateInitialVertexBufferChromeSequences.join(",") !==
      "17859,17860,17861" ||
    d3d9PrivateInitialVertexBufferReturnAddresses.join(",") !==
      "0x1005a890,0x1005aa96,0x1005aab2" ||
    d3d9PrivateInitialVertexBufferCreateArguments.join(",") !==
      "0x01010004,0x00000070,0x00000008,0x00000144,0x00000001,0x101517dc,0x00000000" ||
    d3d9PrivateInitialVertexBufferLockArguments.join(",") !==
      "0x01050004,0x00000000,0x00000000,0x006ffd54,0x00000000" ||
    d3d9PrivateInitialVertexBufferUnlockArguments.join(",") !== "0x01050004" ||
    d3d9PrivateInitialVertexBufferResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialVertexBuffer.vertexBufferOutputAddress !== "0x101517dc" ||
    d3d9PrivateInitialVertexBuffer.vertexBufferPointer !== "0x01050004" ||
    d3d9PrivateInitialVertexBuffer.lockOutputAddress !== "0x006ffd54" ||
    d3d9PrivateInitialVertexBuffer.backingAddress !== "0x44000000" ||
    d3d9PrivateInitialVertexBuffer.backingBytes !== 112 ||
    d3d9PrivateInitialVertexBufferDescriptor.join(",") !== "112,8,324,1" ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsDeviceCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsTextureCount !== 3 ||
    d3d9PrivateInitialVertexBuffer.logicalGraphicsVertexBufferCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.deviceReferenceCountAtStop !== 5 ||
    d3d9PrivateInitialVertexBuffer.vertexBufferReferenceCount !== 1 ||
    d3d9PrivateInitialVertexBuffer.contentGeneration !== 1 ||
    d3d9PrivateInitialVertexBuffer.contentSha256 !==
      "d432a6ed2623c7617983f6e92a6ea25431ad6703229be2248d60bcd1d0846c07" ||
    d3d9PrivateInitialVertexBuffer.backingProtectionAtStop !== "read-only" ||
    d3d9PrivateInitialVertexBuffer.browserRendererCreated !== false ||
    d3d9PrivateInitialVertexBuffer.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9SetRenderState.slot !== 57 ||
    d3d9SetRenderState.explicitArgumentWords !== 2 ||
    d3d9SetRenderState.returnKind !== "h-result" ||
    d3d9SetRenderState.profileId !==
      "primary-initial-fixed-function-render-states-v1" ||
    JSON.stringify(d3d9SetRenderStateSettings) !==
      JSON.stringify([
        {
          state: 22,
          stateName: "D3DRS_CULLMODE",
          value: 1,
          valueName: "D3DCULL_NONE",
        },
        { state: 7, stateName: "D3DRS_ZENABLE", value: 0, valueName: "FALSE" },
        {
          state: 137,
          stateName: "D3DRS_LIGHTING",
          value: 0,
          valueName: "FALSE",
        },
        {
          state: 27,
          stateName: "D3DRS_ALPHABLENDENABLE",
          value: 1,
          valueName: "TRUE",
        },
      ]) ||
    d3d9SetRenderState.stateAuthority !==
      "runtime/graphics.rs::RuntimeGraphicsDevice" ||
    d3d9SetRenderState.mutation !==
      "owner-plan-preflight-stale-check-targeted-commit" ||
    d3d9SetRenderState.sameValueResult !== "0x00000000" ||
    d3d9SetRenderState.sameValueGenerationChange !== 0 ||
    d3d9SetRenderState.unsupportedVariantResult !== "0x8876086a" ||
    d3d9SetRenderState.invalidDeviceFaultsBeforeMutation !== true ||
    d3d9SetRenderState.lastErrorPolicy !== "preserve" ||
    d3d9NativeInitialRenderStates.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeInitialRenderStates.profileId !==
      "x86-bgi-initial-render-states-v1" ||
    d3d9NativeInitialRenderStates.primaryTraceBytes !== 208_326 ||
    d3d9NativeInitialRenderStates.primaryTraceLines !== 511 ||
    d3d9NativeInitialRenderStates.primaryTraceSha256 !==
      "db86df8455db6ed3e2c36b13618730b7d40b6750012136308148a7fc302d86e9" ||
    d3d9NativeInitialRenderStates.primaryBalancedApiPairs !== 222 ||
    d3d9NativeInitialRenderStates.primarySummaryBytes !== 180_986 ||
    d3d9NativeInitialRenderStates.primarySummarySha256 !==
      "31202c168e9f22f0cec2570d41cdadd95ef9eab602e3bab3e1c8b409f2bafa4b" ||
    d3d9NativeInitialRenderStates.confirmationTraceBytes !== 208_999 ||
    d3d9NativeInitialRenderStates.confirmationTraceLines !== 512 ||
    d3d9NativeInitialRenderStates.confirmationTraceSha256 !==
      "66e7b34f2900bbf5470c6433bcd0d7ea0818e87396cc2108d5ee72a163b8a704" ||
    d3d9NativeInitialRenderStates.confirmationBalancedApiPairs !== 222 ||
    d3d9NativeInitialRenderStates.confirmationSummaryBytes !== 181_209 ||
    d3d9NativeInitialRenderStates.confirmationSummarySha256 !==
      "1d04f1ec63c1cf3769f97267bf06bc6133e21a08aedd9173fa6af5a59c934100" ||
    d3d9NativeInitialRenderStates.diagnostics !== 0 ||
    d3d9NativeInitialRenderStates.callsPerTrace !== 4 ||
    d3d9NativeInitialRenderStateCallerRvas.join(",") !==
      "0x0005ad13,0x0005ad26,0x0005ad3c,0x0005ad50" ||
    d3d9NativeInitialRenderStateStates.join(",") !== "22,7,137,27" ||
    d3d9NativeInitialRenderStateValues.join(",") !== "1,0,0,1" ||
    d3d9NativeInitialRenderStateResults.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    d3d9NativeInitialRenderStateLastErrors.join(",") !== "0,0,0,0" ||
    d3d9NativeInitialRenderStates.contiguousSameDeviceAndThreadPerTrace !==
      true ||
    d3d9NativeInitialRenderStates.nativeDeviceAndThreadValuesAreOpaqueArtifactIdentity !==
      true ||
    d3d9NativeInitialRenderStates.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateInitialRenderStateNodeSequences.join(",") !==
      "17862,17863,17864,17865" ||
    d3d9PrivateInitialRenderStateInstructionCounts.join(",") !==
      "11808458,11808466,11808474,11808482" ||
    d3d9PrivateInitialRenderStateChromeSequences.join(",") !==
      "17862,17863,17864,17865" ||
    d3d9PrivateInitialRenderStateReturnAddresses.join(",") !==
      "0x1005ad13,0x1005ad26,0x1005ad3c,0x1005ad50" ||
    d3d9PrivateInitialRenderStates.thisPointer !== "0x01010004" ||
    d3d9PrivateInitialRenderStateStates.join(",") !== "22,7,137,27" ||
    d3d9PrivateInitialRenderStateValues.join(",") !== "1,0,0,1" ||
    d3d9PrivateInitialRenderStateResults.join(",") !==
      "0x00000000,0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateInitialRenderStateGuestWrites.join(",") !== "0,0,0,0" ||
    d3d9PrivateInitialRenderStates.deviceReferenceCountAtStop !== 5 ||
    d3d9PrivateInitialRenderStates.logicalCullMode !== "none" ||
    d3d9PrivateInitialRenderStates.logicalDepthTestEnabled !== false ||
    d3d9PrivateInitialRenderStates.logicalLightingEnabled !== false ||
    d3d9PrivateInitialRenderStates.logicalAlphaBlendEnabled !== true ||
    d3d9PrivateInitialRenderStates.renderStateGenerationDelta !== 4 ||
    d3d9PrivateInitialRenderStates.browserOwnsRenderState !== false ||
    d3d9PrivateInitialRenderStates.browserRendererCreated !== false ||
    d3d9PrivateInitialRenderStates.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9PrivateInitialRenderStates.nextMethod !== "GetRasterStatus" ||
    d3d9PrivateInitialRenderStates.nextSlot !== 19 ||
    d3d9GetRasterStatus.slot !== 19 ||
    d3d9GetRasterStatus.explicitArgumentWords !== 2 ||
    d3d9GetRasterStatus.returnKind !== "h-result" ||
    d3d9GetRasterStatus.profileId !== "primary-stable-nonblank-scanline-v1" ||
    d3d9GetRasterStatus.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    d3d9GetRasterStatus.swapChain !== 0 ||
    d3d9GetRasterStatus.outputStructure !== "D3DRASTER_STATUS" ||
    d3d9GetRasterStatus.outputBytes !== 8 ||
    d3d9GetRasterStatus.inVBlank !== false ||
    d3d9GetRasterStatus.scanLine !== 1 ||
    d3d9GetRasterStatus.statusAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    d3d9GetRasterStatus.displayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    d3d9GetRasterStatus.liveDeviceRelation !==
      "runtime/graphics.rs::RuntimeGraphicsDevice-to-live-COM-object" ||
    d3d9GetRasterStatus.invalidSwapChainOrNullOutputResult !== "0x8876086c" ||
    d3d9GetRasterStatus.policyDisabledFault !==
      "raster-status-policy-disabled" ||
    d3d9GetRasterStatus.unwritableOutputFaultsBeforeMutation !== true ||
    d3d9GetRasterStatus.guestWriteAtomic !== true ||
    d3d9GetRasterStatus.runtimeStateMutation !== false ||
    d3d9GetRasterStatus.referenceCountMutation !== false ||
    d3d9GetRasterStatus.lastErrorPolicy !== "preserve" ||
    d3d9GetRasterStatus.hostScanoutImported !== false ||
    d3d9GetRasterStatus.browserScanoutImported !== false ||
    d3d9NativeRasterStatus.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeRasterStatus.profileId !== "x86-bgi-first-raster-status-v1" ||
    d3d9NativeRasterStatus.primaryTraceBytes !== 261_147 ||
    d3d9NativeRasterStatus.primaryTraceLines !== 505 ||
    d3d9NativeRasterStatus.primaryTraceSha256 !==
      "31ae84a21f5a70c4ae25f3234f52bf229b08e06ca309bbac1567509e7573362a" ||
    d3d9NativeRasterStatus.primarySummaryBytes !== 179_845 ||
    d3d9NativeRasterStatus.primarySummarySha256 !==
      "2d22afec1516f9cb1c2637eb641c6dc54c99f9d9fb3817add90d6fd40f76c724" ||
    d3d9NativeRasterStatus.confirmationTraceBytes !== 263_079 ||
    d3d9NativeRasterStatus.confirmationTraceLines !== 510 ||
    d3d9NativeRasterStatus.confirmationTraceSha256 !==
      "491b6e252c957cc676b8d6dda981b15e74b5796ad9f7b327fbe9bd861b5eea7d" ||
    d3d9NativeRasterStatus.confirmationSummaryBytes !== 180_079 ||
    d3d9NativeRasterStatus.confirmationSummarySha256 !==
      "e72d2b789c77f2005408dd4517179858939ac02559679fd11e5b20301f428069" ||
    d3d9NativeRasterStatus.diagnostics !== 0 ||
    d3d9NativeRasterStatus.callsPerTrace !== 1 ||
    d3d9NativeRasterStatus.callerRva !== "0x0005aedd" ||
    d3d9NativeRasterStatus.slot !== 19 ||
    d3d9NativeRasterStatus.swapChain !== 0 ||
    d3d9NativeRasterStatusResults.join(",") !== "0x00000000,0x00000000" ||
    d3d9NativeRasterStatusLastErrors.join(",") !== "0,0" ||
    d3d9NativeRasterStatusInVBlank.join(",") !== "false,true" ||
    d3d9NativeRasterStatusScanLines.join(",") !== "1200,0" ||
    d3d9NativeRasterStatus.outputFullyOverwritten !== true ||
    d3d9NativeRasterStatus.immediateConsumerChecksHresult !== true ||
    d3d9NativeRasterStatus.immediateConsumerChecksVBlank !== true ||
    d3d9NativeRasterStatus.immediateConsumerConsumesScanLine !== true ||
    d3d9NativeRasterStatus.vBlankResult !== "0x81000000" ||
    d3d9NativeRasterStatus.outerConsumerRva !== "0x0006a263" ||
    d3d9NativeRasterStatus.outerConsumerAcceptsSuccessOrVBlank !== true ||
    d3d9NativeRasterStatus.outerConsumerRepeatsRasterStatusRead !== true ||
    d3d9NativeRasterStatus.outerConsumerYieldsWithSleepZero !== true ||
    d3d9NativeRasterStatus.nativeDeviceAndOutputPointersAreOpaqueArtifactIdentity !==
      true ||
    d3d9NativeRasterStatus.hostScanoutValuesAreArtifactObservations !== true ||
    d3d9NativeRasterStatus.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateRasterStatus.nodeSequence !== 17_866 ||
    d3d9PrivateRasterStatus.nodeInstructionCount !== 11_808_586 ||
    d3d9PrivateRasterStatus.chromeSequence !== 17_866 ||
    d3d9PrivateRasterStatusArguments.join(",") !==
      "0x01010004,0x00000000,0x006ffcf0" ||
    d3d9PrivateRasterStatus.returnAddress !== "0x1005aedd" ||
    d3d9PrivateRasterStatus.result !== "0x00000000" ||
    d3d9PrivateRasterStatus.lastError !== 0 ||
    d3d9PrivateRasterStatus.outputAddress !== "0x006ffcf0" ||
    d3d9PrivateRasterStatus.outputBytes !== 8 ||
    d3d9PrivateRasterStatus.outputHex !== "0000000001000000" ||
    d3d9PrivateRasterStatus.inVBlank !== false ||
    d3d9PrivateRasterStatus.scanLine !== 1 ||
    d3d9PrivateRasterStatus.deviceReferenceCount !== 5 ||
    d3d9PrivateRasterStatus.objectRetired !== false ||
    d3d9PrivateRasterStatus.runtimeStateMutation !== false ||
    d3d9PrivateRasterStatus.hostScanoutImported !== false ||
    d3d9PrivateRasterStatus.browserScanoutImported !== false ||
    d3d9PrivateRasterStatus.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9GetSurfaceLevel.slot !== 18 ||
    d3d9GetSurfaceLevel.explicitArgumentWords !== 2 ||
    d3d9GetSurfaceLevel.returnKind !== "h-result" ||
    d3d9GetSurfaceLevel.profileId !==
      "primary-system-memory-texture-level-zero-surfaces-v2" ||
    d3d9GetSurfaceLevel.level !== 0 ||
    d3d9GetSurfaceLevel.sourceTextureVariantIndices?.join(",") !== "0,3" ||
    d3d9GetSurfaceLevel.otherTextureVariantFault !==
      "get-surface-level-texture-variant-unsupported" ||
    d3d9GetSurfaceLevel.surfaceOutputWriteBytes !== 4 ||
    d3d9GetSurfaceLevel.repeatReturnsSameIdentityAndAddsReference !== true ||
    d3d9GetSurfaceLevel.guestComGraphicsPublicationAtomic !== true ||
    d3d9GetSurfaceLevel.invalidLevelOrNullOutputResult !== "0x8876086c" ||
    d3d9GetSurfaceLevel.unwritableOrExhaustedFaultsBeforeMutation !== true ||
    d3d9GetSurfaceLevel.lastErrorPolicy !== "preserve" ||
    d3d9SurfaceInterface.classIdentity !== "d3d9.idirect3dsurface9.v1" ||
    d3d9SurfaceInterface.interface !== "IDirect3DSurface9" ||
    d3d9SurfaceInterface.vtableSlots !== 17 ||
    d3d9SurfaceImplementedMethods.join(",") !==
      "QueryInterface,AddRef,Release,GetDC,ReleaseDC" ||
    d3d9SurfaceInterface.initialReferenceCount !== 1 ||
    d3d9SurfaceInterface.retainsTextureInterfaceReference !== true ||
    d3d9SurfaceInterface.liveDeviceContextBlocksRetirement !== true ||
    d3d9SurfaceInterface.finalReleaseRetiresGraphicsAssociation !== true ||
    d3d9SurfaceInterface.finalReleaseMayCascadeTextureDeviceAndRootRetirement !==
      true ||
    d3d9NativeGetSurfaceLevel.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeGetSurfaceLevel.primaryTraceBytes !== 228_637 ||
    d3d9NativeGetSurfaceLevel.primaryTraceLines !== 514 ||
    d3d9NativeGetSurfaceLevel.primaryTraceSha256 !==
      "54aa56bade26d675f864162bb7a7a2f8b4fc30f8df8dbd0835681cc09f9eced2" ||
    d3d9NativeGetSurfaceLevel.primaryBalancedApiPairs !== 222 ||
    d3d9NativeGetSurfaceLevel.primarySummaryBytes !== 230_104 ||
    d3d9NativeGetSurfaceLevel.primarySummarySha256 !==
      "c68ef403d864c89ad466ed3bda3cef750065d7555b815fb23f3ad99a43697c8e" ||
    d3d9NativeGetSurfaceLevel.confirmationTraceBytes !== 229_203 ||
    d3d9NativeGetSurfaceLevel.confirmationTraceLines !== 514 ||
    d3d9NativeGetSurfaceLevel.confirmationTraceSha256 !==
      "e3f6d853293cfa8f395695c2de21cb4675cc71f27efa4b3715f2171575ad1422" ||
    d3d9NativeGetSurfaceLevel.confirmationBalancedApiPairs !== 222 ||
    d3d9NativeGetSurfaceLevel.confirmationSummaryBytes !== 230_131 ||
    d3d9NativeGetSurfaceLevel.confirmationSummarySha256 !==
      "1ed3b5f247a8b6dd82efd2cdaa7cb0393e68d7db9639d07eaa1afa2244b4a81f" ||
    d3d9NativeGetSurfaceLevel.diagnostics !== 0 ||
    d3d9NativeGetSurfaceLevel.successfulCallsPerTrace !== 1 ||
    d3d9NativeGetSurfaceLevel.callerRva !== "0x0005a731" ||
    d3d9NativeGetSurfaceLevel.level !== 0 ||
    d3d9NativeGetSurfaceLevel.result !== "0x00000000" ||
    d3d9NativeGetSurfaceLevel.lastError !== 0 ||
    d3d9NativeGetSurfaceLevel.surfaceOutputNonNull !== true ||
    d3d9NativeGetSurfaceLevel.hresultSignConsumed !== true ||
    d3d9NativeGetSurfaceLevel.immediateNextMethod !== "GetDC" ||
    d3d9NativeGetSurfaceLevel.immediateNextSlot !== 15 ||
    d3d9NativeGetSurfaceLevel.immediateNextCallerRva !== "0x0005a746" ||
    d3d9NativeGetSurfaceLevel.sourceExecutableHashUnchanged !== true ||
    d3d9ControlledGetSurfaceLevel.architecture !== "coff-i386" ||
    d3d9ControlledGetSurfaceLevel.sourceBytes !== 13_977 ||
    d3d9ControlledGetSurfaceLevel.sourceSha256 !==
      "1ec0c191bb9783abcbfae7b92ba6d7136018111c4b1da7c860d7dd343daca562" ||
    d3d9ControlledGetSurfaceLevel.executableBytes !== 12_288 ||
    d3d9ControlledGetSurfaceLevel.executableSha256 !==
      "7ca28333d06b7352b032b0af9ceb22a016e03d5830f17a74231569ef2b2fee9e" ||
    d3d9ControlledGetSurfaceLevel.repeatReturnsSameSurfacePointer !== true ||
    d3d9ControlledGetSurfaceLevel.repeatReturnsSameIunknownIdentity !== true ||
    d3d9ControlledGetSurfaceLevel.surfaceIunknownDistinctFromTextureIunknown !==
      true ||
    d3d9ControlledGetSurfaceLevel.getContainerReturnsOriginalTexture !== true ||
    d3d9ControlledGetSurfaceLevel.invalidLevelResult !== "0x8876086c" ||
    d3d9ControlledGetSurfaceLevel.invalidLevelZerosOutput !== true ||
    d3d9ControlledGetSurfaceLevel.nullOutputException !== "0xc0000005" ||
    d3d9ControlledGetSurfaceLevel.successPreservesLastError !== true ||
    d3d9PrivateGetSurfaceLevel.nodeSequence !== 17_851 ||
    d3d9PrivateGetSurfaceLevel.nodeInstructionCount !== 11_807_783 ||
    d3d9PrivateGetSurfaceLevel.chromeSequence !== 17_851 ||
    d3d9PrivateGetSurfaceLevelArguments.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    d3d9PrivateGetSurfaceLevel.returnAddress !== "0x1005a731" ||
    d3d9PrivateGetSurfaceLevel.result !== "0x00000000" ||
    d3d9PrivateGetSurfaceLevel.surfaceOutputAddress !== "0x006ffd38" ||
    d3d9PrivateGetSurfaceLevel.surfacePointer !== "0x01030004" ||
    d3d9PrivateGetSurfaceLevel.guestWriteBytes !== 4 ||
    d3d9PrivateGetSurfaceLevel.surfaceObjectId !== 5 ||
    d3d9PrivateGetSurfaceLevel.parentInterfaceId !== 8 ||
    d3d9PrivateGetSurfaceLevel.surfaceInterfaceId !== 10 ||
    d3d9PrivateGetSurfaceLevel.surfaceVtable !== "0x0103001c" ||
    d3d9PrivateGetSurfaceLevel.surfaceVtableSlots !== 17 ||
    d3d9PrivateGetSurfaceLevel.textureReferenceCount !== 2 ||
    d3d9PrivateGetSurfaceLevel.surfaceReferenceCount !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsTextureId !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceId !== 1 ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceNamespace !==
      "vine.logical-graphics-surface.v1" ||
    d3d9PrivateSurfaceDescriptor.join(",") !== "0,1024,1024,22" ||
    d3d9PrivateGetSurfaceLevel.logicalGraphicsSurfaceCount !== 1 ||
    d3d9PrivateGetSurfaceLevel.browserRendererCreated !== false ||
    d3d9PrivateGetSurfaceLevel.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9SurfaceDc.profileId !== "primary-surface-gdi-dc-v1" ||
    d3d9SurfaceDc.getDcSlot !== 15 ||
    d3d9SurfaceDc.getDcExplicitArgumentWords !== 1 ||
    d3d9SurfaceDc.releaseDcSlot !== 16 ||
    d3d9SurfaceDc.releaseDcExplicitArgumentWords !== 1 ||
    d3d9SurfaceDc.returnKind !== "h-result" ||
    d3d9SurfaceDc.outputWriteBytes !== 4 ||
    d3d9SurfaceDc.deviceContextAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    d3d9SurfaceDc.graphicsSurfaceAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    d3d9SurfaceDc.surfaceRelation !==
      "non-owning-revalidated-live-com-and-graphics-identity" ||
    d3d9SurfaceDc.singleOutstandingDeviceContextPerSurface !== true ||
    d3d9SurfaceDc.acquisitionAndGuestWriteAtomic !== true ||
    d3d9SurfaceDc.repeatGetDcResult !== "0x8876086c" ||
    d3d9SurfaceDc.wrongOrRepeatedReleaseDcResult !== "0x8876086c" ||
    d3d9SurfaceDc.liveDeviceContextBlocksSurfaceRetirement !== true ||
    d3d9SurfaceDc.fillRectWritesBoundedCommandHistory !== true ||
    d3d9SurfaceDc.lastErrorPolicy !== "preserve" ||
    d3d9SurfaceDc.pixelStorageCreated !== false ||
    d3d9SurfaceDc.browserRendererCreated !== false ||
    d3d9NativeSurfaceDc.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeSurfaceDc.profileId !==
      "x86-bgi-system-memory-surface-dc-fill-v1" ||
    d3d9NativeSurfaceDc.primaryTraceBytes !== 259_841 ||
    d3d9NativeSurfaceDc.primaryTraceLines !== 519 ||
    d3d9NativeSurfaceDc.primaryTraceSha256 !==
      "3d0010cace0f0e13e3f0c6918b57d3aaac46afb8b2fb399d16ec2197de41acef" ||
    d3d9NativeSurfaceDc.primaryBalancedApiPairs !== 225 ||
    d3d9NativeSurfaceDc.primarySummaryBytes !== 235_461 ||
    d3d9NativeSurfaceDc.primarySummarySha256 !==
      "0a08225c4a15cd82f15d5401a8247b2b301bad1e787551fcb1f34e34c26b4192" ||
    d3d9NativeSurfaceDc.confirmationTraceBytes !== 258_887 ||
    d3d9NativeSurfaceDc.confirmationTraceLines !== 518 ||
    d3d9NativeSurfaceDc.confirmationTraceSha256 !==
      "8e5109f42a539e04421bf2ac33fa85a98b27a084b52e2b2f2738f1861393c03c" ||
    d3d9NativeSurfaceDc.confirmationBalancedApiPairs !== 224 ||
    d3d9NativeSurfaceDc.confirmationSummaryBytes !== 235_576 ||
    d3d9NativeSurfaceDc.confirmationSummarySha256 !==
      "2b8e16261cdc17afc3b8779b94ecae3de9a4265277105aba0dad82277872010c" ||
    d3d9NativeSurfaceDc.diagnostics !== 0 ||
    d3d9NativeSurfaceDc.callsPerTrace !== 5 ||
    d3d9NativeSurfaceDcCallerRvas.join(",") !==
      "0x0005a746,0x0005a76c,0x0005a77d,0x0005a78e,0x0005a79a" ||
    d3d9NativeSurfaceDcMethods.join(",") !==
      "GetDC,GetStockObject,FillRect,ReleaseDC,Release" ||
    d3d9NativeSurfaceDc.getDcResult !== "0x00000000" ||
    d3d9NativeSurfaceDc.getDcOutputNonNull !== true ||
    d3d9NativeSurfaceDc.stockObjectIndex !== 4 ||
    d3d9NativeSurfaceDc.stockObjectResultNonNull !== true ||
    d3d9NativeSurfaceDcRect.join(",") !== "0,0,800,600" ||
    d3d9NativeSurfaceDc.fillRectResult !== 1 ||
    d3d9NativeSurfaceDc.releaseDcResult !== "0x00000000" ||
    d3d9NativeSurfaceDc.surfaceReleaseResult !== 1 ||
    d3d9NativeSurfaceDcLastErrors.join(",") !== "0,0,0,0,0" ||
    d3d9NativeSurfaceDc.sourceExecutableHashUnchanged !== true ||
    d3d9ControlledSurfaceDc.architecture !== "coff-i386" ||
    d3d9ControlledSurfaceDc.sourceBytes !== 16_434 ||
    d3d9ControlledSurfaceDc.sourceSha256 !==
      "33d79e003ff050f8e22787629b375d60c1650a331148d7e7ef40ed68654ad685" ||
    d3d9ControlledSurfaceDc.executableBytes !== 8_704 ||
    d3d9ControlledSurfaceDc.executableSha256 !==
      "056d6fd36e5cfc14840b63137219e8e3fa36bcfb396104d1d1cba6faf7e6bd00" ||
    d3d9ControlledSurfaceDc.nullOutputExecutableSha256 !==
      "f533018f6f7619ce6d67b27ebee5e0b2f9ca8401f7e4c5abf1a7585e70d68e6f" ||
    d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256 !==
      "ad7b34ea2ddd5206d050cd8550b0fb8074f9ceafacbc9819d330590c0759f6eb" ||
    d3d9ControlledSurfaceDc.firstGetDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.repeatGetDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.repeatGetDcPreservesOutput !== true ||
    d3d9ControlledSurfaceDc.reacquireGetDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDcReferenceCounts.join(",") !== "2,2,2" ||
    d3d9ControlledSurfaceDc.wrongReleaseDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.repeatedReleaseDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.correctReleaseDcResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.lockRectWhileDcResult !== "0x8876086c" ||
    d3d9ControlledSurfaceDc.lockRectAfterReleaseResult !== "0x00000000" ||
    d3d9ControlledSurfaceDc.nullOutputException !== "0xc0000005" ||
    d3d9ControlledSurfaceDc.readOnlyOutputException !== "0xc0000005" ||
    d3d9ControlledSurfaceDc.successPreservesLastError !== true ||
    d3d9PrivateSurfaceDcNodeSequences.join(",") !==
      "17852,17853,17854,17855,17856" ||
    d3d9PrivateSurfaceDcInstructionCounts.join(",") !==
      "11807793,11807805,11807812,11807820,11807826" ||
    d3d9PrivateSurfaceDcChromeSequences.join(",") !==
      "17852,17853,17854,17855,17856" ||
    d3d9PrivateSurfaceDcReturnAddresses.join(",") !==
      "0x1005a746,0x1005a76c,0x1005a77d,0x1005a78e,0x1005a79a" ||
    d3d9PrivateSurfaceDc.surfacePointer !== "0x01030004" ||
    d3d9PrivateSurfaceDc.getDcOutputAddress !== "0x006ffd3c" ||
    d3d9PrivateSurfaceDc.deviceContextHandle !== "0x47010002" ||
    d3d9PrivateSurfaceDc.deviceContextId !== 2 ||
    d3d9PrivateSurfaceDc.deviceContextNamespace !== "vine.logical-gdi-dc.v1" ||
    d3d9PrivateSurfaceDc.deviceContextTargetKind !== "graphics-surface" ||
    d3d9PrivateSurfaceDc.logicalGraphicsSurfaceId !== 1 ||
    d3d9PrivateSurfaceDc.surfaceComObjectId !== 5 ||
    d3d9PrivateSurfaceDcGeometry.join(",") !== "1024,1024" ||
    d3d9PrivateSurfaceDc.surfaceColorFormat !== "b8g8r8x8-unorm" ||
    d3d9PrivateSurfaceDc.stockBrushHandle !== "0x47020001" ||
    d3d9PrivateSurfaceDcFillRect.join(",") !== "0,0,800,600" ||
    d3d9PrivateSurfaceDc.fillColorRef !== 0 ||
    d3d9PrivateSurfaceDc.surfaceContentGeneration !== 1 ||
    d3d9PrivateSurfaceDc.surfaceGeneration !== 4 ||
    d3d9PrivateSurfaceDc.surfaceReleaseReferenceCount !== 0 ||
    d3d9PrivateSurfaceDc.surfaceObjectRetired !== true ||
    d3d9PrivateSurfaceDc.logicalGraphicsSurfaceCountAfterRelease !== 0 ||
    d3d9PrivateSurfaceDc.secondCreateTextureSequence !== 17_857 ||
    d3d9PrivateSurfaceDc.secondCreateTextureInstructionCount !== 11_807_840 ||
    d3d9PrivateSurfaceDc.secondCreateTexturePool !== 0 ||
    d3d9PrivateSurfaceDc.secondCreateTextureResult !== "0x00000000" ||
    d3d9PrivateSurfaceDc.secondCreateTextureOutputAddress !== "0x101517d0" ||
    d3d9PrivateSurfaceDc.secondCreateTexturePointer !== "0x01030004" ||
    d3d9PrivateSurfaceDc.secondCreateTextureGuestWriteBytes !== 4 ||
    d3d9PrivateSurfaceDc.browserRendererCreated !== false ||
    d3d9PrivateSurfaceDc.pixelStorageCreated !== false ||
    d3d9PrivateSurfaceDc.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9Clear.slot !== 43 ||
    d3d9Clear.explicitArgumentWords !== 6 ||
    d3d9Clear.returnKind !== "h-result" ||
    d3d9Clear.profileId !== "primary-full-render-target-black-v1" ||
    d3d9Clear.count !== 0 ||
    d3d9Clear.rectanglesPointer !== "0x00000000" ||
    d3d9Clear.flags !== "0x00000001" ||
    d3d9ClearFlagNames.join(",") !== "D3DCLEAR_TARGET" ||
    d3d9Clear.colorArgb !== "0xff000000" ||
    d3d9Clear.depthBits !== "0x3f800000" ||
    d3d9Clear.stencil !== 0 ||
    d3d9Clear.targetAuthority !== "runtime/graphics.rs::RuntimeGraphics" ||
    d3d9Clear.targetScope !== "full-presentation-back-buffer" ||
    d3d9Clear.contentModel !== "uniform-clear-command" ||
    d3d9Clear.mutation !== "validate-plan-preflight-targeted-commit" ||
    d3d9Clear.staleOrDuplicatePlanRejected !== true ||
    d3d9Clear.invalidVariantResult !== "0x8876086c" ||
    d3d9Clear.invalidVariantMutation !== false ||
    d3d9Clear.guestWriteBytes !== 0 ||
    d3d9Clear.lastErrorPolicy !== "preserve" ||
    d3d9Clear.depthStencilCleared !== false ||
    d3d9Clear.partialRectanglesImplemented !== false ||
    d3d9Clear.browserRendererCreated !== false ||
    d3d9NativeClear.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeClear.profileId !== "x86-bgi-d3d9-clear-full-target-v1" ||
    d3d9NativeClear.primaryTraceBytes !== 16_493_258 ||
    d3d9NativeClear.primaryTraceLines !== 46_186 ||
    d3d9NativeClear.primaryTraceSha256 !==
      "bc85120088c46632ab7a484832c377643d1352c4a05ec94b14a5108122f7c06f" ||
    d3d9NativeClear.primaryBalancedApiPairs !== 23_054 ||
    d3d9NativeClear.primarySummaryBytes !== 7_958_205 ||
    d3d9NativeClear.primarySummarySha256 !==
      "e0c07cccfdd148b1445c6065162fd9042032ed6d687f2d4edafad17e7666b27b" ||
    d3d9NativeClear.primaryCalls !== 402 ||
    d3d9NativeClear.confirmationTraceBytes !== 16_412_273 ||
    d3d9NativeClear.confirmationTraceLines !== 46_117 ||
    d3d9NativeClear.confirmationTraceSha256 !==
      "641f78dcc0e7d5dcc82b6778b6f5e27c5a5e1e88744b1cc11fbf6867272b20c3" ||
    d3d9NativeClear.confirmationBalancedApiPairs !== 23_019 ||
    d3d9NativeClear.confirmationSummaryBytes !== 7_897_741 ||
    d3d9NativeClear.confirmationSummarySha256 !==
      "1a7ed22a846c84bb16f9ec6ca38709652fbc749848dcec6c601b672985f23150" ||
    d3d9NativeClear.confirmationCalls !== 404 ||
    d3d9NativeClear.diagnostics !== 0 ||
    d3d9NativeClear.callerRva !== "0x0005af9e" ||
    d3d9NativeClearArguments.join(",") !==
      "0,0,1,4278190080,1065353216,0" ||
    d3d9NativeClear.result !== "0x00000000" ||
    d3d9NativeClear.lastError !== 0 ||
    d3d9NativeClear.allCallsMatchProfile !== true ||
    d3d9NativeClear.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateClear.nodeSequence !== 127_355 ||
    d3d9PrivateClear.nodeInstructionCount !== 28_761_128 ||
    d3d9PrivateClear.chromeSequence !== 127_355 ||
    d3d9PrivateClearArguments.join(",") !==
      "0x01010004,0x00000000,0x00000000,0x00000001,0xff000000,0x3f800000,0x00000000" ||
    d3d9PrivateClear.returnAddress !== "0x1005af9e" ||
    d3d9PrivateClear.result !== "0x00000000" ||
    d3d9PrivateClear.guestWriteBytes !== 0 ||
    d3d9PrivateClear.deviceReferenceCount !== 5 ||
    d3d9PrivateClear.deviceObjectRetired !== false ||
    d3d9PrivateClear.logicalGraphicsDeviceId !== 3 ||
    d3d9PrivateClear.logicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    d3d9PrivateClear.logicalGraphicsComObjectId !== "21" ||
    d3d9PrivateClearPresentationGeometry.join(",") !== "1280,720" ||
    d3d9PrivateClear.backBufferContentGeneration !== 1 ||
    d3d9PrivateClear.backBufferClearSequence !== 1 ||
    d3d9PrivateClearRect.join(",") !== "0,0,1280,720" ||
    d3d9PrivateClear.backBufferClearColorArgb !== "0xff000000" ||
    d3d9PrivateClear.logicalGraphicsDeviceGeneration !== 44 ||
    d3d9PrivateClear.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9AddDirtyRect.slot !== 21 ||
    d3d9AddDirtyRect.explicitArgumentWords !== 1 ||
    d3d9AddDirtyRect.returnKind !== "h-result" ||
    d3d9AddDirtyRect.profileId !==
      "primary-recreated-system-memory-texture-dirty-rect-v1" ||
    d3d9AddDirtyRectSourceTextureVariantIndices.join(",") !== "3" ||
    d3d9AddDirtyRect.textureAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    d3d9AddDirtyRect.initialDirtyRegion !== "whole-texture" ||
    d3d9AddDirtyRect.mergePolicy !== "bounding-union" ||
    d3d9AddDirtyRect.nullRectMeaning !== "whole-texture" ||
    d3d9AddDirtyRect.validResult !== "0x00000000" ||
    d3d9AddDirtyRect.invalidResult !== "0x8876086c" ||
    d3d9AddDirtyRect.mutation !==
      "validate-plan-preflight-targeted-commit" ||
    d3d9AddDirtyRect.failureAtomicity !== true ||
    d3d9AddDirtyRect.lastErrorPolicy !== "preserve" ||
    d3d9AddDirtyRect.guestWriteBytes !== 0 ||
    d3d9AddDirtyRect.dirtyRegionConsumed !== false ||
    d3d9AddDirtyRect.browserRendererCreated !== false ||
    d3d9NativeAddDirtyRect.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeAddDirtyRect.profileId !==
      "x86-bgi-first-recreated-texture-add-dirty-rect-v1" ||
    d3d9NativeAddDirtyRect.oracleProfileId !==
      "x86-system-memory-texture-rect-variants-v1" ||
    d3d9NativeAddDirtyRect.primaryTraceBytes !== 509_016 ||
    d3d9NativeAddDirtyRect.primaryTraceLines !== 1_193 ||
    d3d9NativeAddDirtyRect.primaryTraceSha256 !==
      "5677e484c6650c33f3757265d4185649816ee92b823b9339b44bd15fac838680" ||
    d3d9NativeAddDirtyRect.primaryBalancedApiPairs !== 556 ||
    d3d9NativeAddDirtyRect.primarySummaryBytes !== 303_427 ||
    d3d9NativeAddDirtyRect.primarySummarySha256 !==
      "c39f201c909ec5f6b9cb242a389d382e6f27fe427a03a156eb9580cd08c6a209" ||
    d3d9NativeAddDirtyRect.confirmationTraceBytes !== 508_597 ||
    d3d9NativeAddDirtyRect.confirmationTraceLines !== 1_192 ||
    d3d9NativeAddDirtyRect.confirmationTraceSha256 !==
      "a8eb8bb0d19b84392b0543ebdae66f76d938b997bf7856e36726b54c790b89bf" ||
    d3d9NativeAddDirtyRect.confirmationBalancedApiPairs !== 555 ||
    d3d9NativeAddDirtyRect.confirmationSummaryBytes !== 303_587 ||
    d3d9NativeAddDirtyRect.confirmationSummarySha256 !==
      "5071dc1b418c0c173e4deae4f62f23b0ee9f1d1e7450b896e7bcac12f03bf3fa" ||
    d3d9NativeAddDirtyRect.diagnostics !== 0 ||
    d3d9NativeAddDirtyRect.callsPerTrace !== 8 ||
    d3d9NativeAddDirtyRect.creationCallerRva !== "0x0005a715" ||
    d3d9NativeAddDirtyRect.callerRva !== "0x0005b01d" ||
    d3d9NativeAddDirtyRectPrimaryRect.join(",") !== "0,0,1280,720" ||
    d3d9NativeAddDirtyRectValidInteriorRect.join(",") !== "1,2,17,19" ||
    d3d9NativeAddDirtyRect.nullRectAccepted !== true ||
    d3d9NativeAddDirtyRect.validResult !== "0x00000000" ||
    d3d9NativeAddDirtyRect.invalidResult !== "0x8876086c" ||
    d3d9NativeAddDirtyRect.lastErrorPreserved !== true ||
    d3d9NativeAddDirtyRect.immediateConsumer !==
      "IDirect3DDevice9::UpdateTexture-slot-31" ||
    d3d9NativeAddDirtyRect.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateAddDirtyRect.nodeSequence !== 127_357 ||
    d3d9PrivateAddDirtyRect.nodeInstructionCount !== 28_761_165 ||
    d3d9PrivateAddDirtyRect.chromeSequence !== 127_357 ||
    d3d9PrivateAddDirtyRectArguments.join(",") !==
      "0x01020004,0x006ff29c" ||
    d3d9PrivateAddDirtyRect.returnAddress !== "0x1005b01d" ||
    d3d9PrivateAddDirtyRectStackPointers.join(",") !==
      "0x006ff25c,0x006ff268" ||
    d3d9PrivateAddDirtyRect.result !== "0x00000000" ||
    d3d9PrivateAddDirtyRect.guestWriteBytes !== 0 ||
    d3d9PrivateAddDirtyRect.textureReferenceCount !== 1 ||
    d3d9PrivateAddDirtyRect.textureObjectRetired !== false ||
    d3d9PrivateAddDirtyRect.logicalGraphicsTextureId !== 7 ||
    d3d9PrivateAddDirtyRect.logicalGraphicsTextureNamespace !==
      "vine.logical-graphics-texture.v1" ||
    d3d9PrivateAddDirtyRect.logicalGraphicsComObjectId !== "22" ||
    d3d9PrivateAddDirtyRectTextureGeometry.join(",") !== "2048,1024" ||
    d3d9PrivateAddDirtyRect.texturePool !== "system-memory" ||
    d3d9PrivateAddDirtyRect.dirtyRegion !== "whole-texture" ||
    d3d9PrivateAddDirtyRect.textureGeneration !== 41 ||
    d3d9PrivateAddDirtyRect.textureGenerationChanged !== false ||
    d3d9PrivateAddDirtyRect.browserRendererResourceCreated !== false ||
    d3d9PrivateAddDirtyRect.nodeChromeOrderedSemanticsMatched !== true ||
    d3d9.sharedGraphicsBoundary !==
      "RuntimeGraphics-owned-logical-device-texture-content-dirty-region-update-scene-sampler-state-lifecycle-surface-view-vertex-buffer-render-state-and-back-buffer-content-at-D3D9-gateways" ||
    d3d9.runtimeGraphicsCreated !== true ||
    d3d9.runtimeTextureCreated !== true ||
    d3d9.runtimeSurfaceCreated !== true ||
    d3d9.runtimeVertexBufferCreated !== true ||
    d3d9.runtimeRenderStatesSet !== true ||
    d3d9.runtimeRasterStatusQueried !== true ||
    d3d9.runtimeBackBufferCleared !== true ||
    d3d9.runtimeTextureDirtyRegionTracked !== true ||
    d3d9.runtimeTextureContentUpdated !== true ||
    d3d9.runtimeSceneBegan !== true ||
    d3d9.runtimeSamplerStatesSet !== true ||
    d3d9.rendererResourceCreated !== false ||
    d3d9.d3d8Implemented !== false ||
    d3d9.d3d10Or11Implemented !== false ||
    d3d9.generalD3d9CompatibilityClaimed !== false
  ) {
    errors.push("active D3D9 frontend boundary is invalid");
  }
  if (
    d3d9UpdateTexture.slot !== 31 ||
    d3d9UpdateTexture.explicitArgumentWords !== 2 ||
    d3d9UpdateTexture.returnKind !== "h-result" ||
    d3d9UpdateTexture.profileId !==
      "primary-recreated-system-memory-to-static-default-texture-update-v1" ||
    d3d9UpdateTextureSourceVariantIndices.join(",") !== "3" ||
    d3d9UpdateTextureDestinationVariantIndices.join(",") !== "4" ||
    d3d9UpdateTexture.contentAuthority !==
      "runtime/graphics.rs::RuntimeGraphicsTexture" ||
    d3d9UpdateTexture.transferScope !==
      "whole-texture-level-zero-logical-content" ||
    d3d9UpdateTexture.sourcePool !== "system-memory" ||
    d3d9UpdateTexture.destinationPool !== "default" ||
    d3d9UpdateTexture.sourceUsage !== "none" ||
    d3d9UpdateTexture.destinationUsage !== "none" ||
    d3d9UpdateTexture.validResult !== "0x00000000" ||
    d3d9UpdateTexture.invalidResult !== "0x8876086c" ||
    d3d9UpdateTexture.mutation !== "dual-texture-preflight-targeted-commit" ||
    d3d9UpdateTexture.failureAtomicity !== true ||
    d3d9UpdateTexture.lastErrorPolicy !== "preserve" ||
    d3d9UpdateTexture.guestWriteBytes !== 0 ||
    d3d9UpdateTexture.sourceDirtyRegionConsumed !== true ||
    d3d9UpdateTexture.partialDirtyRegionsImplemented !== false ||
    d3d9UpdateTexture.dynamicDestinationImplemented !== false ||
    d3d9UpdateTexture.browserRendererCreated !== false ||
    d3d9NativeUpdateTexture.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeUpdateTexture.profileId !==
      "x86-bgi-first-recreated-texture-update-v1" ||
    d3d9NativeUpdateTexture.oracleProfileId !==
      "x86-recreated-texture-update-pool-and-identity-variants-v1" ||
    d3d9NativeUpdateTexture.primaryTraceBytes !== 550_638 ||
    d3d9NativeUpdateTexture.primaryTraceLines !== 1_219 ||
    d3d9NativeUpdateTexture.primaryTraceSha256 !==
      "f24e8be66f5f6e2507c455e396cee1aa43eea9e42ff888eb20ec3896ac698601" ||
    d3d9NativeUpdateTexture.primaryBalancedApiPairs !== 568 ||
    d3d9NativeUpdateTexture.primarySummaryBytes !== 382_225 ||
    d3d9NativeUpdateTexture.primarySummarySha256 !==
      "1c2cb3f39db3609f91306fde02da4c3e8485649ea7610cbb68fc20380e5d793f" ||
    d3d9NativeUpdateTexture.confirmationTraceBytes !== 549_350 ||
    d3d9NativeUpdateTexture.confirmationTraceLines !== 1_214 ||
    d3d9NativeUpdateTexture.confirmationTraceSha256 !==
      "499d6fdfa93f1f7f225e345b8270589e5b93e691452f70069edfecaba78d7f5e" ||
    d3d9NativeUpdateTexture.confirmationBalancedApiPairs !== 566 ||
    d3d9NativeUpdateTexture.confirmationSummaryBytes !== 382_103 ||
    d3d9NativeUpdateTexture.confirmationSummarySha256 !==
      "0af54fd50a108ade2364e17789184fd8d3b56968c076cf7af0dffc898bbd39cb" ||
    d3d9NativeUpdateTexture.diagnostics !== 0 ||
    d3d9NativeUpdateTextureCreationCallerRvas.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    d3d9NativeUpdateTexture.callerRva !== "0x0005b038" ||
    d3d9NativeUpdateTexture.sourceVariantIndex !== 3 ||
    d3d9NativeUpdateTexture.destinationVariantIndex !== 4 ||
    d3d9NativeUpdateTexture.dynamicDestinationVariantIndex !== 5 ||
    d3d9NativeUpdateTextureSuccessVariants.join(",") !==
      "primary,repeat,dynamic-destination" ||
    d3d9NativeUpdateTextureInvalidVariants.join(",") !==
      "reversed-pools,default-source,same-resource" ||
    d3d9NativeUpdateTexture.nullVariantClaimed !== false ||
    d3d9NativeUpdateTexture.validResult !== "0x00000000" ||
    d3d9NativeUpdateTexture.invalidResult !== "0x8876086c" ||
    d3d9NativeUpdateTexture.lastErrorPreserved !== true ||
    d3d9NativeUpdateTexture.immediateConsumer !==
      "IDirect3DDevice9::BeginScene-slot-41" ||
    d3d9NativeUpdateTexture.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateUpdateTexture.nodeSequence !== 127_358 ||
    d3d9PrivateUpdateTexture.nodeInstructionCount !== 28_761_175 ||
    d3d9PrivateUpdateTexture.chromeSequence !== 127_358 ||
    d3d9PrivateUpdateTextureArguments.join(",") !==
      "0x01010004,0x01020004,0x01030004" ||
    d3d9PrivateUpdateTexture.returnAddress !== "0x1005b038" ||
    d3d9PrivateUpdateTextureStackPointers.join(",") !==
      "0x006ff258,0x006ff268" ||
    d3d9PrivateUpdateTexture.result !== "0x00000000" ||
    d3d9PrivateUpdateTexture.guestWriteBytes !== 0 ||
    d3d9PrivateUpdateTexture.deviceReferenceCount !== 5 ||
    d3d9PrivateUpdateTexture.deviceObjectRetired !== false ||
    d3d9PrivateUpdateTexture.sourceTextureId !== 7 ||
    d3d9PrivateUpdateTexture.sourceComObjectId !== "22" ||
    d3d9PrivateUpdateTexture.sourcePool !== "system-memory" ||
    d3d9PrivateUpdateTexture.sourceGeneration !== 42 ||
    d3d9PrivateUpdateTexture.sourceDirtyRegionAfter !== "clean" ||
    d3d9PrivateUpdateTexture.destinationTextureId !== 8 ||
    d3d9PrivateUpdateTexture.destinationComObjectId !== "24" ||
    d3d9PrivateUpdateTexture.destinationPool !== "default" ||
    d3d9PrivateUpdateTexture.destinationGeneration !== 45 ||
    d3d9PrivateUpdateTexture.contentGeneration !== 1 ||
    d3d9PrivateUpdateTexture.logicalFillSequence !== 3 ||
    d3d9PrivateUpdateTextureFillRect.join(",") !== "0,0,1280,720" ||
    d3d9PrivateUpdateTexture.logicalFillColorRef !== "0x00000000" ||
    d3d9PrivateUpdateTexture.updateSequence !== 1 ||
    d3d9PrivateUpdateTexture.browserRendererResourceCreated !== false ||
    d3d9PrivateUpdateTexture.nodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("active D3D9 UpdateTexture boundary is invalid");
  }
  if (
    d3d9BeginScene.slot !== 41 ||
    d3d9BeginScene.explicitArgumentWords !== 0 ||
    d3d9BeginScene.returnKind !== "h-result" ||
    d3d9BeginScene.profileId !== "primary-first-frame-scene-begin-v1" ||
    d3d9BeginScene.sceneAuthority !==
      "runtime/graphics.rs::RuntimeGraphicsDevice" ||
    d3d9BeginScene.lifecycle !== "inactive-to-active" ||
    d3d9BeginScene.validResult !== "0x00000000" ||
    d3d9BeginScene.invalidResult !== "0x8876086c" ||
    d3d9BeginScene.mutation !==
      "validate-plan-preflight-targeted-commit" ||
    d3d9BeginScene.failureAtomicity !== true ||
    d3d9BeginScene.lastErrorPolicy !== "preserve" ||
    d3d9BeginScene.guestWriteBytes !== 0 ||
    d3d9BeginScene.nestedBeginReturnsInvalidCall !== true ||
    d3d9BeginScene.endSceneImplemented !== false ||
    d3d9BeginScene.browserRendererCreated !== false ||
    d3d9NativeBeginScene.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeBeginScene.profileId !== "x86-bgi-first-begin-scene-v1" ||
    d3d9NativeBeginScene.oracleProfileId !==
      "x86-d3d9-scene-lifecycle-v1" ||
    d3d9NativeBeginScene.primaryTraceBytes !== 470_012 ||
    d3d9NativeBeginScene.primaryTraceLines !== 1_171 ||
    d3d9NativeBeginScene.primaryTraceSha256 !==
      "04d6a33b19276408e408478c916ac27e41565c3516bb8f5cc43136955236b150" ||
    d3d9NativeBeginScene.primaryBalancedApiPairs !== 546 ||
    d3d9NativeBeginScene.primarySummaryBytes !== 273_814 ||
    d3d9NativeBeginScene.primarySummarySha256 !==
      "d3243dbdad2e54f543b1fefa6bce4080f955b102f1c21f196cffaacf45de9843" ||
    d3d9NativeBeginScene.confirmationTraceBytes !== 469_761 ||
    d3d9NativeBeginScene.confirmationTraceLines !== 1_174 ||
    d3d9NativeBeginScene.confirmationTraceSha256 !==
      "f7dd018d61e247e42f18ad315107c8755753462b697bfdf6deb3f7c2f8c90d52" ||
    d3d9NativeBeginScene.confirmationBalancedApiPairs !== 547 ||
    d3d9NativeBeginScene.confirmationSummaryBytes !== 274_066 ||
    d3d9NativeBeginScene.confirmationSummarySha256 !==
      "2fbd178a0cea82d37fccacb350294ca934be5b85841fd91dda6952661b157054" ||
    d3d9NativeBeginScene.diagnostics !== 0 ||
    d3d9NativeBeginScene.callerRva !== "0x0005b048" ||
    d3d9NativeBeginSceneArguments.length !== 0 ||
    d3d9NativeBeginScene.result !== "0x00000000" ||
    d3d9NativeBeginScene.lastErrorBefore !== 0 ||
    d3d9NativeBeginScene.lastErrorAfter !== 0 ||
    d3d9NativeBeginScene.immediateConsumer !==
      "hresult-sign-test-then-first-frame-state-setup" ||
    d3d9NativeBeginScene.oracleArchitecture !== "coff-i386-pe32" ||
    d3d9NativeBeginScene.oracleSourceBytes !== 8_025 ||
    d3d9NativeBeginScene.oracleSourceSha256 !==
      "47e39e28ca77f7e6164e39afd3ae5448930c221aa84c7711d2e19b57230b8e77" ||
    d3d9NativeBeginScene.oracleObjectBytes !== 8_639 ||
    d3d9NativeBeginScene.oracleObjectSha256 !==
      "db268f3154fa5fd5d55763b61da9c5f768997f70a95158f3fc2b54ff73be7c34" ||
    d3d9NativeBeginScene.oracleExecutableBytes !== 5_120 ||
    d3d9NativeBeginScene.oracleExecutableSha256 !==
      "092ae273c871747c88cd76fac5b476169e479ac04fad50de1ffcf30a0870c1b5" ||
    d3d9NativeBeginScene.oracleRuns !== 2 ||
    d3d9NativeBeginSceneOracleExitCodes.join(",") !== "0,0" ||
    d3d9NativeBeginScene.oracleLastErrorSentinel !== 23_130 ||
    d3d9NativeBeginScene.oracleLastErrorPreserved !== true ||
    d3d9NativeBeginScene.oracleBeginFirstResult !== "0x00000000" ||
    d3d9NativeBeginScene.oracleBeginNestedResult !== "0x8876086c" ||
    d3d9NativeBeginScene.oracleEndFirstResult !== "0x00000000" ||
    d3d9NativeBeginScene.oracleEndOutsideResult !== "0x8876086c" ||
    d3d9NativeBeginScene.oracleEndRepeatedResult !== "0x8876086c" ||
    d3d9NativeBeginScene.oracleSecondCycleSucceeded !== true ||
    d3d9NativeBeginSceneOracleDeviceReferenceCounts.join(",") !== "1,1" ||
    d3d9NativeBeginScene.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateBeginScene.nodeSequence !== 127_359 ||
    d3d9PrivateBeginScene.nodeInstructionCount !== 28_761_181 ||
    d3d9PrivateBeginScene.chromeSequence !== 127_359 ||
    d3d9PrivateBeginSceneArguments.join(",") !== "0x01010004" ||
    d3d9PrivateBeginScene.returnAddress !== "0x1005b048" ||
    d3d9PrivateBeginSceneStackPointers.join(",") !==
      "0x006ff260,0x006ff268" ||
    d3d9PrivateBeginScene.result !== "0x00000000" ||
    d3d9PrivateBeginScene.guestWriteBytes !== 0 ||
    d3d9PrivateBeginScene.deviceReferenceCount !== 5 ||
    d3d9PrivateBeginScene.deviceObjectRetired !== false ||
    d3d9PrivateBeginScene.logicalGraphicsDeviceId !== 3 ||
    d3d9PrivateBeginScene.logicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    d3d9PrivateBeginScene.logicalGraphicsComObjectId !== "21" ||
    d3d9PrivateBeginScenePresentationGeometry.join(",") !== "1280,720" ||
    d3d9PrivateBeginScene.backBufferContentGeneration !== 1 ||
    d3d9PrivateBeginScene.backBufferClearSequence !== 1 ||
    d3d9PrivateBeginScene.activeSceneSequence !== 1 ||
    d3d9PrivateBeginScene.logicalGraphicsDeviceGeneration !== 45 ||
    d3d9PrivateBeginScene.browserRendererResourceCreated !== false ||
    d3d9PrivateBeginScene.nodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("active D3D9 BeginScene boundary is invalid");
  }
  if (
    d3d9InitialSamplerStatesActive &&
    (d3d9SamplerState.slot !== 69 ||
      d3d9SamplerState.explicitArgumentWords !== 3 ||
      d3d9SamplerState.returnKind !== "h-result" ||
      d3d9SamplerState.profileId !==
        "primary-first-frame-linear-sampler-states-v1" ||
      d3d9SamplerState.stateAuthority !==
        "runtime/graphics.rs::RuntimeGraphicsDevice" ||
      d3d9SamplerState.supportedSampler !== 0 ||
      JSON.stringify(d3d9SamplerState.supportedStates) !==
        JSON.stringify([
          { state: 6, value: 2, meaning: "minification-linear" },
          { state: 5, value: 2, meaning: "magnification-linear" },
        ]) ||
      d3d9SamplerState.validResult !== "0x00000000" ||
      d3d9SamplerState.unsupportedResult !== "0x8876086a" ||
      d3d9SamplerState.mutation !==
        "validate-plan-preflight-targeted-idempotent-commit" ||
      d3d9SamplerState.failureAtomicity !== true ||
      d3d9SamplerState.sameValueGenerationChange !== 0 ||
      d3d9SamplerState.lastErrorPolicy !== "preserve" ||
      d3d9SamplerState.guestWriteBytes !== 0 ||
      d3d9SamplerState.browserRendererCreated !== false ||
      d3d9NativeSamplerState.traceSchema !== "vine.win32-trace.v2" ||
      d3d9NativeSamplerState.profileId !==
        "x86-bgi-first-frame-linear-sampler-states-v1" ||
      d3d9NativeSamplerState.oracleProfileId !==
        "x86-d3d9-sampler-state-acceptance-and-readback-v1" ||
      d3d9NativeSamplerState.primaryTraceBytes !== 478_010 ||
      d3d9NativeSamplerState.primaryTraceLines !== 1_170 ||
      d3d9NativeSamplerState.primaryTraceSha256 !==
        "0d1e0cc5771ddec935bb0063479077195c92fd956b06ddc5abdeff32369d9fac" ||
      d3d9NativeSamplerState.primaryBalancedApiPairs !== 545 ||
      d3d9NativeSamplerState.primarySummaryBytes !== 313_234 ||
      d3d9NativeSamplerState.primarySummarySha256 !==
        "97beca5386ef8e61c6d511ea6c57abc88d74afb82c0d7ff0b7821ea57ee6da93" ||
      d3d9NativeSamplerState.confirmationTraceBytes !== 478_518 ||
      d3d9NativeSamplerState.confirmationTraceLines !== 1_172 ||
      d3d9NativeSamplerState.confirmationTraceSha256 !==
        "af43a004b3133798645aded796a574cfedaf1855fc58d1a06e5665d644f35a4f" ||
      d3d9NativeSamplerState.confirmationBalancedApiPairs !== 546 ||
      d3d9NativeSamplerState.confirmationSummaryBytes !== 313_197 ||
      d3d9NativeSamplerState.confirmationSummarySha256 !==
        "1d6cde0f9972d5c7c2c8b5dffd6b707bb744a4919bf7de2d30eda97b018a9f3a" ||
      d3d9NativeSamplerState.diagnostics !== 0 ||
      d3d9NativeSamplerState.callerRvas?.join(",") !==
        "0x0005b18a,0x0005b1a0" ||
      JSON.stringify(d3d9NativeSamplerState.arguments) !==
        JSON.stringify([
          ["0x00000000", "0x00000006", "0x00000002"],
          ["0x00000000", "0x00000005", "0x00000002"],
        ]) ||
      d3d9NativeSamplerState.results?.join(",") !==
        "0x00000000,0x00000000" ||
      d3d9NativeSamplerState.lastErrors?.join(",") !== "0,0" ||
      d3d9NativeSamplerState.immediateConsumer !==
        "return-value-ignored-then-next-first-frame-state-call" ||
      d3d9NativeSamplerState.oracleArchitecture !== "coff-i386-pe32" ||
      d3d9NativeSamplerState.oracleSourceBytes !== 9_891 ||
      d3d9NativeSamplerState.oracleSourceSha256 !==
        "16b3be86c15f66d25ae46aac89b9fbbc1459ce8861328fc434195de285d853b8" ||
      d3d9NativeSamplerState.oracleObjectBytes !== 10_116 ||
      d3d9NativeSamplerState.oracleObjectSha256 !==
        "610cd8647523645f74041434e22dbc82ce8112344c04b14f3518647b215ef10d" ||
      d3d9NativeSamplerState.oracleExecutableBytes !== 6_144 ||
      d3d9NativeSamplerState.oracleExecutableSha256 !==
        "ad584e3233c5cc7cfae2cd1768942dfa3c69a41c2e4ac9feae54cc4b22ec9d26" ||
      d3d9NativeSamplerState.oracleRuns !== 2 ||
      d3d9NativeSamplerState.oracleExitCodes?.join(",") !== "0,0" ||
      d3d9NativeSamplerState.oracleLastErrorSentinel !== 23_130 ||
      d3d9NativeSamplerState.oracleLastErrorPreserved !== true ||
      d3d9NativeSamplerState.oracleInitialMinFilter !== 1 ||
      d3d9NativeSamplerState.oracleMinFilterAfterLinear !== 2 ||
      d3d9NativeSamplerState.oracleMagFilterAfterLinear !== 2 ||
      d3d9NativeSamplerState.oracleRepeatedMinFilterResult !== "0x00000000" ||
      d3d9NativeSamplerState.oracleInvalidSamplerResult !== "0x00000000" ||
      d3d9NativeSamplerState.oracleInvalidStateResult !== "0x00000000" ||
      d3d9NativeSamplerState.oracleInvalidFilterResult !== "0x00000000" ||
      d3d9NativeSamplerState.oracleDeviceReferenceCounts?.join(",") !== "1,1" ||
      d3d9NativeSamplerState.runtimePolicyNarrowsNativePermissiveness !== true ||
      d3d9NativeSamplerState.sourceExecutableHashUnchanged !== true ||
      d3d9PrivateSamplerState.nodeSequences?.join(",") !== "127360,127361" ||
      d3d9PrivateSamplerState.nodeInstructionCounts?.join(",") !==
        "28761195,28761204" ||
      d3d9PrivateSamplerState.chromeSequences?.join(",") !==
        "127360,127361" ||
      JSON.stringify(d3d9PrivateSamplerState.arguments) !==
        JSON.stringify([
          ["0x01010004", "0x00000000", "0x00000006", "0x00000002"],
          ["0x01010004", "0x00000000", "0x00000005", "0x00000002"],
        ]) ||
      d3d9PrivateSamplerState.returnAddresses?.join(",") !==
        "0x1005b18a,0x1005b1a0" ||
      JSON.stringify(d3d9PrivateSamplerState.stackPointers) !==
        JSON.stringify([
          ["0x006ff254", "0x006ff268"],
          ["0x006ff254", "0x006ff268"],
        ]) ||
      d3d9PrivateSamplerState.results?.join(",") !==
        "0x00000000,0x00000000" ||
      d3d9PrivateSamplerState.guestWriteBytes?.join(",") !== "0,0" ||
      d3d9PrivateSamplerState.deviceReferenceCounts?.join(",") !== "5,5" ||
      d3d9PrivateSamplerState.deviceObjectsRetired?.join(",") !==
        "false,false" ||
      d3d9PrivateSamplerState.logicalGraphicsDeviceId !== 3 ||
      d3d9PrivateSamplerState.logicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      d3d9PrivateSamplerState.logicalGraphicsComObjectId !== "21" ||
      JSON.stringify(d3d9PrivateSamplerState.filtersAfterCalls) !==
        JSON.stringify([
          { min: "linear", mag: null },
          { min: "linear", mag: "linear" },
        ]) ||
      d3d9PrivateSamplerState.logicalGraphicsDeviceGenerations?.join(",") !==
        "46,47" ||
      d3d9PrivateSamplerState.postSamplerRenderStateRejectedSequences?.join(",") !==
        "127362,127363,127364" ||
      d3d9PrivateSamplerState.postSamplerRenderStateRejectedInstructionCounts?.join(
        ",",
      ) !== "28761212,28761220,28761228" ||
      d3d9PrivateSamplerState.postSamplerRenderStateRejectedResults?.join(",") !==
        "0x8876086a,0x8876086a,0x8876086a" ||
      d3d9PrivateSamplerState.postSamplerRenderStateCallerIgnoredResults !== true ||
      d3d9PrivateSamplerState.browserRendererResourceCreated !== false ||
      d3d9PrivateSamplerState.nodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("active D3D9 sampler-state boundary is invalid");
  }
  if (
    d3d9NativeRecreatedTextures.traceSchema !== "vine.win32-trace.v2" ||
    d3d9NativeRecreatedTextures.profileId !== "x86-bgi-recreated-textures-v1" ||
    d3d9NativeRecreatedTextures.primaryTraceBytes !== 520_239 ||
    d3d9NativeRecreatedTextures.primaryTraceLines !== 1_175 ||
    d3d9NativeRecreatedTextures.primaryTraceSha256 !==
      "b5a1ecbda79c5f59b38d3cfcb3a40c6aeacaadd0925ee515a1a4e73be3714a08" ||
    d3d9NativeRecreatedTextures.primaryBalancedApiPairs !== 547 ||
    d3d9NativeRecreatedTextures.primarySummaryBytes !== 456_252 ||
    d3d9NativeRecreatedTextures.primarySummarySha256 !==
      "7cbd49226942b0a332bde07d3a0c3c2bfa8bfab59027a1fd5de705a4e759336f" ||
    d3d9NativeRecreatedTextures.primaryDurationMilliseconds !== 3_633 ||
    d3d9NativeRecreatedTextures.confirmationTraceBytes !== 519_513 ||
    d3d9NativeRecreatedTextures.confirmationTraceLines !== 1_173 ||
    d3d9NativeRecreatedTextures.confirmationTraceSha256 !==
      "9503f828d4fb971f72fac0b20de60e482e0eda070860ec12fe961f7ac584d377" ||
    d3d9NativeRecreatedTextures.confirmationBalancedApiPairs !== 546 ||
    d3d9NativeRecreatedTextures.confirmationSummaryBytes !== 456_367 ||
    d3d9NativeRecreatedTextures.confirmationSummarySha256 !==
      "8bc54d43b9125b3faf469a1e84427f55598c058b7a2053b4bc136152662be569" ||
    d3d9NativeRecreatedTextures.confirmationDurationMilliseconds !== 3_368 ||
    d3d9NativeRecreatedTextures.diagnostics !== 0 ||
    d3d9NativeRecreatedTextures.callsPerTrace !== 3 ||
    d3d9NativeRecreatedTextureCallerRvas.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    JSON.stringify(d3d9NativeRecreatedTextures.variants) !==
      JSON.stringify([
        [2_048, 1_024, 1, 0, 22, 2],
        [2_048, 1_024, 1, 0, 22, 0],
        [2_048, 1_024, 1, 512, 22, 0],
      ]) ||
    d3d9NativeRecreatedTextures.sharedHandlesNull !== true ||
    d3d9NativeRecreatedTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9NativeRecreatedTextureLastErrors.join(",") !== "0,0,0" ||
    d3d9NativeRecreatedTextureOutputAddresses.join(",") !==
      "0x008f17cc,0x008f17d0,0x008f17d4" ||
    d3d9NativeRecreatedTextures.outputBytesWrittenPerCall !== 4 ||
    d3d9NativeRecreatedTextures.returnedTexturesNonNullAndDistinctPerTrace !==
      true ||
    d3d9NativeRecreatedTextures.hresultSignConsumedPerCall !== true ||
    d3d9NativeRecreatedTextures.sourceExecutableHashUnchanged !== true ||
    d3d9PrivateRecreatedTextureNodeSequences.join(",") !==
      "108735,108736,108737,108738,108739,108740,108741,108742,108743" ||
    d3d9PrivateRecreatedTextureChromeSequences.join(",") !==
      "108735,108736,108737,108738,108739,108740,108741,108742,108743" ||
    d3d9PrivateRecreatedTextureMethods.join(",") !==
      "IDirect3DDevice9.CreateTexture,IDirect3DTexture9.GetSurfaceLevel,IDirect3DSurface9.GetDC,GetStockObject,FillRect,IDirect3DSurface9.ReleaseDC,IDirect3DSurface9.Release,IDirect3DDevice9.CreateTexture,IDirect3DDevice9.CreateTexture" ||
    d3d9PrivateRecreatedTextureSequences.join(",") !== "108735,108742,108743" ||
    d3d9PrivateRecreatedTextureOccurrences.join(",") !== "7,8,9" ||
    d3d9PrivateRecreatedTextureInstructionCounts.join(",") !==
      "28274782,28274850,28274866" ||
    JSON.stringify(d3d9PrivateRecreatedTextures.createTextureArguments) !==
      JSON.stringify([
        [
          "0x01010004",
          "0x00000800",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000002",
          "0x101517cc",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000800",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000000",
          "0x101517d0",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000800",
          "0x00000400",
          "0x00000001",
          "0x00000200",
          "0x00000016",
          "0x00000000",
          "0x101517d4",
          "0x00000000",
        ],
      ]) ||
    d3d9PrivateRecreatedTextureResults.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    d3d9PrivateRecreatedTextureReturnAddresses.join(",") !==
      "0x1005a715,0x1005a7b7,0x1005a7dc" ||
    d3d9PrivateRecreatedTextureOutputAddresses.join(",") !==
      "0x101517cc,0x101517d0,0x101517d4" ||
    d3d9PrivateRecreatedTexturePointers.join(",") !==
      "0x01020004,0x01030004,0x01040004" ||
    d3d9PrivateRecreatedTextureReferenceCounts.join(",") !== "1,2,3" ||
    d3d9PrivateRecreatedTextureGuestWrites.join(",") !== "4,4,4" ||
    d3d9PrivateRecreatedTextureSurfaceGeometry.join(",") !== "2048,1024" ||
    d3d9PrivateRecreatedTextureFillRect.join(",") !== "0,0,1280,720" ||
    d3d9PrivateRecreatedTextures.compatibilityPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    d3d9PrivateRecreatedTextures.createTextureProfileId !==
      "primary-initial-and-recreated-x8r8g8b8-textures-v2" ||
    d3d9PrivateRecreatedTextures.getSurfaceLevelProfileId !==
      "primary-system-memory-texture-level-zero-surfaces-v2" ||
    d3d9PrivateRecreatedTextures.textureAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    d3d9PrivateRecreatedTextures.comAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    d3d9PrivateRecreatedTextures.browserRendererCreated !== false ||
    d3d9PrivateRecreatedTextures.mutation !==
      "cross-owner-preflight-transactional-commit" ||
    d3d9PrivateRecreatedTextures.failureAtomicity !== true ||
    d3d9PrivateRecreatedTextures.nodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("active D3D9 recreated-texture evidence is invalid");
  }
  const gdi = requireRecord(
    active.runtimeGdiFrontend,
    "activeRuntimeCheckpoint.runtimeGdiFrontend",
    errors,
  );
  const gdiStockObjectIndices = requireIntegerArray(
    gdi.implementedStockObjectIndices,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.implementedStockObjectIndices",
    errors,
  );
  const gdiSurfaceGeometry = requireIntegerArray(
    gdi.surfaceGeometry,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.surfaceGeometry",
    errors,
  );
  const gdiTeardownOrder = requireStringArray(
    gdi.processTeardownOrder,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.processTeardownOrder",
    errors,
  );
  const gdiImplementedApis = requireStringArray(
    gdi.implementedApis,
    "activeRuntimeCheckpoint.runtimeGdiFrontend.implementedApis",
    errors,
  );
  if (
    gdi.schema !== "vine.runtime-gdi-frontend.v6" ||
    gdi.status !==
      "window-screen-memory-font-enumeration-font-object-dib-lifecycle-graphics-surface-dc-and-bounded-window-surface-resize-slices-implemented-and-verified" ||
    gdi.stateOwner !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi.aggregateCoordinator !== "runtime/kernel.rs::RuntimeKernel" ||
    gdi.mutableCollectionVisibility !== "private" ||
    gdi.readInspection !== "immutable-borrow" ||
    gdi.windowAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    gdi.windowRelation !==
      "non-owning-revalidated-before-each-cross-subsystem-operation" ||
    gdi.displayAuthority !== "runtime/displays.rs::RuntimeDisplays" ||
    gdi.screenRelation !==
      "non-owning-primary-monitor-and-acquiring-thread-revalidated-before-commit-and-release" ||
    gdi.graphicsAuthority !== "runtime/graphics.rs::RuntimeGraphics" ||
    gdi.graphicsSurfaceRelation !==
      "non-owning-revalidated-live-com-and-graphics-identity" ||
    gdi.deviceContextHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    gdi.deviceContextHandleBase !== "0x47010001" ||
    gdi.deviceContextAcquisitionIdentity !==
      "distinct-monotonic-per-acquisition" ||
    gdi.deviceContextTargetVariants?.join(",") !==
      "window,screen,memory,graphics-surface" ||
    gdi.screenOutstandingDeviceContexts !== "distinct-per-acquisition" ||
    gdi.screenSurfaceCreated !== false ||
    gdi.screenRendererResourceCreated !== false ||
    gdi.screenDrawingImplemented !== false ||
    gdi.screenOperationValidation !==
      "live-same-thread-primary-monitor-relation" ||
    gdi.fontCatalogAuthority !== "gdi32-compatibility-policy" ||
    gdi.fontEnumerationProfileId !== "primary-cp932-ms-gothic-regular-v1" ||
    gdi.fontEnumerationRecordCount !== 1 ||
    gdi.fontEnumerationCallbackScratchBytes !== 284 ||
    gdi.fontEnumerationCallbackResultPropagation !== "outer-eax" ||
    gdi.fontEnumerationCanonicalTailInitialization !== "zero" ||
    gdi.nativeFontRecordTailImported !== false ||
    gdi.hostFontDiscoveryImported !== false ||
    gdi.browserFontDiscoveryImported !== false ||
    gdi.fontCreationProfileId !== "primary-cp932-ms-gothic-64x32-thin-v1" ||
    gdi.fontObjectCreationImplemented !== true ||
    gdi.fontObjectSelectionImplemented !== true ||
    gdi.fontObjectIntrospectionImplemented !== true ||
    gdi.fontObjectDeletionImplemented !== true ||
    gdi.fontObjectIdentity !== "distinct-monotonic-nonshared-per-create" ||
    gdi.fontObjectState !== "exact-60-byte-logfonta-semantic-fields" ||
    gdi.fontObjectBrowserResourceCreated !== false ||
    gdi.glyphRasterizationImplemented !== false ||
    gdi.compatibleMemoryDcProfileId !== "primary-null-screen-memory-dc-v1" ||
    gdi.compatibleMemoryDcCreationImplemented !== true ||
    gdi.compatibleMemoryDcDeletionImplemented !== true ||
    gdi.compatibleMemoryDcSourceVariants?.join(",") !== "null-primary-screen" ||
    gdi.compatibleMemoryDcInitialFontStockIndex !== 13 ||
    gdi.compatibleMemoryDcInitialBitmapStockIndex !== 21 ||
    gdi.compatibleMemoryDcInitialBitmapGeometry?.join(",") !== "1,1,1" ||
    gdi.compatibleMemoryDcDefaultObjectsShared !== true ||
    gdi.compatibleMemoryDcOwnerRelation !== "creating-thread-exact-delete" ||
    gdi.compatibleMemoryDcSelectionState !==
      "selected-font-id-selected-bitmap-id-generation" ||
    gdi.compatibleMemoryDcPixelStorageCreated !== true ||
    gdi.compatibleMemoryDcPixelStorageOwnership !==
      "selected-dib-object-guest-memory-backing" ||
    gdi.compatibleMemoryDcBrowserResourceCreated !== false ||
    gdi.compatibleMemoryDcReleaseDcImplemented !== false ||
    gdi.dibSectionProfileId !== "primary-128x-96-8bpp-reverse-grayscale-v1" ||
    gdi.dibSectionCreationImplemented !== true ||
    gdi.dibSectionIntrospectionImplemented !== true ||
    gdi.dibSectionSelectionImplemented !== true ||
    gdi.dibSectionDeletionImplemented !== true ||
    gdi.dibSectionDescriptor?.join(",") !== "128,-96,1,8,0,128,12288" ||
    gdi.dibSectionPalette !== "256-entry-reverse-grayscale-rgbquad" ||
    gdi.dibSectionObjectLifetime !== "nonshared-explicit-delete" ||
    gdi.dibSectionBackingAuthority !== "rust-guest-memory" ||
    gdi.dibSectionAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    gdi.dibSectionAddressClassification !==
      "graphics-object-explicit-release" ||
    gdi.dibSectionMemoryRegionKind !== "gdi-resource" ||
    gdi.dibSectionSelectedDeletePolicy !==
      "reject-mutation-free-until-restored" ||
    gdi.dibSectionBrowserResourceCreated !== false ||
    gdi.graphicsSurfaceOutstandingDeviceContexts !==
      "at-most-one-per-surface" ||
    gdi.graphicsSurfaceDeviceContextRetainsComReference !== false ||
    gdi.deviceContextSerializationCompatibility !==
      "window-shape-preserved-screen-memory-and-graphics-targets-explicitly-typed" ||
    gdi.nativeNumericHandleReuseImported !== false ||
    gdi.gdiObjectHandleNamespace !== "vine.logical-gdi-object.v1" ||
    gdi.gdiObjectHandleBase !== "0x47020001" ||
    gdiStockObjectIndices.join(",") !== "4" ||
    gdi.stockObjectLifetime !== "process-shared-until-teardown" ||
    gdi.surfaceIdentity !== "one-logical-surface-per-window" ||
    gdiSurfaceGeometry.join(",") !== "800,600" ||
    gdi.windowSurfaceResizeImplemented !== true ||
    gdi.windowSurfaceResizeVariant !==
      "primary-client-800x600-to-1280x720-nocopybits" ||
    gdi.windowSurfaceResizeIdentity !==
      "preserve-existing-logical-surface-id-or-no-op-when-absent" ||
    gdi.windowSurfaceResizeContentPolicy !==
      "clear-bounded-fill-history-and-increment-surface-generation" ||
    gdi.windowSurfaceResizeLiveDcPolicy !== "reject-before-mutation" ||
    gdi.windowSurfaceResizeTransaction !==
      "opaque-plan-stale-check-preflighted-infallible-commit" ||
    gdi.windowSurfaceResizeCoordinator !==
      "runtime/kernel.rs::RuntimeKernel-after-wm-nccalcsize" ||
    gdi.surfaceFillHistoryBounded !== true ||
    gdi.surfaceGeneration !== 1 ||
    gdi.graphicsSurfaceFillAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    gdi.graphicsSurfaceFillHistoryBounded !== true ||
    gdi.graphicsSurfaceFillCapacity !== 16_384 ||
    gdi.graphicsSurfaceContentGeneration !== true ||
    gdi.graphicsSurfacePixelStorageCreated !== false ||
    gdi.releaseLifetime !== "retire-only-matching-acquisition-hdc" ||
    gdi.liveGraphicsSurfaceDeviceContextBlocksComRetirement !== true ||
    gdi.mutation !== "owner-plan-stale-check-targeted-commit" ||
    gdi.failureAtomicity !== true ||
    gdiTeardownOrder.join(",") !== "gdi,graphics,user32" ||
    gdi.browserOwnsWindowsState !== false ||
    gdi.runtimeGraphicsCreated !== true ||
    gdi.browserRendererResourceCreated !== false ||
    gdi.sharedGraphicsBoundary !==
      "RuntimeGraphics-logical-surface-fill-command-history" ||
    gdi.gdiAndDirect3dFrontendsRemainSeparate !== true ||
    gdiImplementedApis.join(",") !==
      "GetDC,GetStockObject,EnumFontFamiliesExA,CreateFontA,CreateCompatibleDC,CreateDIBSection,GetObjectA,SelectObject,DeleteObject,DeleteDC,FillRect,ReleaseDC" ||
    gdi.generalGdiRenderingClaimed !== false
  ) {
    errors.push("active GDI frontend boundary is invalid");
  }
  const registries = requireRecord(
    active.runtimeRegistries,
    "activeRuntimeCheckpoint.runtimeRegistries",
    errors,
  );
  if (
    registries.schema !== "vine.runtime-registries.v1" ||
    registries.stateOwner !== "runtime/registries.rs::RuntimeRegistries" ||
    registries.mutableCollectionVisibility !== "private" ||
    registries.readInspection !== "immutable-borrow" ||
    !Array.isArray(registries.serializedRegistryFields) ||
    registries.serializedRegistryFields.join(",") !==
      "dynamicEndpoints,encodedPointers" ||
    !Array.isArray(registries.serializedRuntimeCompatibilityFields) ||
    registries.serializedRuntimeCompatibilityFields.join(",") !==
      "encodedPointerNamespace,encodedPointerTokenPrefix,encodedPointerSources,nextEncodedPointerId" ||
    registries.hostRequestCompatibilityField !== "registries.hostRequests" ||
    registries.hostRequestAuthority !==
      "runtime/scheduler.rs::RuntimeScheduler" ||
    !Array.isArray(registries.ownedCollections) ||
    registries.ownedCollections.join(",") !==
      "dynamic-endpoint-table,encoded-pointer-token-table,encoded-pointer-source-index,encoded-pointer-id-allocator,encoded-pointer-policy" ||
    registries.dynamicEndpointTransition !==
      "registry-plan-aggregate-address-preflight-infallible-commit" ||
    registries.encodedPointerTransition !==
      "registry-plan-infallible-targeted-commit" ||
    registries.subsystemTests < 5 ||
    registries.crossSubsystemAtomicTests < 3 ||
    registries.directCollectionMutationFromAggregate !== false ||
    registries.encodedPointerIndexAtomic !== true ||
    registries.duplicateEndpointMutationFree !== true ||
    registries.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime registry subsystem boundary is invalid");
  }
  const modules = requireRecord(
    active.runtimeModules,
    "activeRuntimeCheckpoint.runtimeModules",
    errors,
  );
  if (
    modules.schema !== "vine.runtime-modules.v2" ||
    modules.stateOwner !== "runtime/modules.rs::RuntimeModules" ||
    modules.processRecordOwner !== "runtime/modules.rs::RuntimeModuleRecords" ||
    modules.dllLifecycleOperationOwner !==
      "runtime/modules.rs::RuntimeDllLifecycle" ||
    modules.mutableCollectionVisibility !== "private" ||
    modules.readInspection !== "immutable-borrow" ||
    modules.serializedRuntimeField !== "moduleGraph" ||
    modules.serializedProcessField !== "modules" ||
    modules.serializationAdapter !== "serde-flatten-private-runtime-modules" ||
    modules.lifecycleSerialized !== false ||
    !Array.isArray(modules.ownedCollections) ||
    modules.ownedCollections.join(",") !==
      "process-module-records,module-graph,module-images,module-import-bindings,runtime-system-exports,dll-lifecycle-operation-state" ||
    modules.crossSubsystemMutation !==
      "module-lifecycle-plan-object-memory-callback-preflight-targeted-commit" ||
    modules.lifecycleTransition !==
      "module-plan-thread-scheduler-preflight-infallible-commit" ||
    modules.subsystemTests < 8 ||
    modules.crossSubsystemAtomicTests < 5 ||
    modules.registrationRollbackMutation !== false ||
    modules.dynamicRuntimeSystemModuleLoad !==
      "owner-plan-object-preflight-targeted-commit" ||
    modules.dynamicRuntimeSystemModuleHandleAllocation !==
      "module-owner-monotonic-slot" ||
    modules.dynamicRuntimeSystemModuleStaleGuard !==
      "nonserialized-module-graph-mutation-generation" ||
    modules.dynamicRuntimeSystemModuleGraphRootCopies !== false ||
    modules.dynamicRuntimeSystemModuleExistingIdentityReuse !== true ||
    modules.dynamicRuntimeSystemModuleThreadReferenceCounting !== true ||
    modules.dynamicRuntimeSystemModuleCallbackExecution !== false ||
    modules.dynamicRuntimeSystemModuleFailureAtomicity !== true ||
    modules.callbackReplacementAtomic !== true ||
    modules.activeLifecycleBlocksModuleMutation !== true ||
    modules.directCollectionMutationFromAggregate !== false ||
    modules.directLifecycleMutationFromAggregate !== false ||
    modules.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime module subsystem boundary is invalid");
  }
  const threads = requireRecord(
    active.runtimeThreadRegistry,
    "activeRuntimeCheckpoint.runtimeThreadRegistry",
    errors,
  );
  if (
    threads.schema !== "vine.runtime-thread-registry.v7" ||
    threads.stateOwner !== "runtime/thread.rs::RuntimeThreads" ||
    threads.mutableCollectionVisibility !== "private" ||
    threads.serializedRuntimeField !== "threads" ||
    !Array.isArray(threads.serializedProcessCompatibilityFields) ||
    threads.serializedProcessCompatibilityFields.join(",") !==
      "allocatedTlsIndices,flsSlots" ||
    !Array.isArray(threads.serializedRuntimeCompatibilityFields) ||
    threads.serializedRuntimeCompatibilityFields.join(",") !==
      "nextTlsIndex,nextFlsIndex,nextCallbackFrameId,nextThreadId" ||
    threads.serializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(threads.processWideState) ||
    threads.processWideState.join(",") !==
      "tls-index-registry,fls-slot-registry,tls-index-allocator,fls-index-allocator,callback-frame-id-allocator,thread-id-allocator" ||
    threads.crossSubsystemMutation !==
      "thread-resource-plan-aggregate-lifecycle-scheduler-preflight-infallible-commit" ||
    threads.subsystemTests < 8 ||
    threads.crossSubsystemAtomicTests < 6 ||
    threads.directRecordMutationFromAggregate !== false ||
    threads.directResourceMutationFromAggregate !== false ||
    threads.resourceAllocatorFailureMutationFree !== true ||
    threads.callbackAllocatorAtomic !== true ||
    threads.stackMetadataPlanCommit !== true ||
    threads.comApartmentPlanCommit !== true ||
    threads.comApartmentOverflowMutationFree !== true ||
    threads.comApartmentThreadExitCleanup !== true ||
    threads.firstApiCreatedThreadId !== 8_193 ||
    threads.nextThreadIdAfterReachedCreate !== 8_194 ||
    threads.maximumThreads !== 256 ||
    threads.dynamicStackArenaBase !== "0x50000000" ||
    threads.dynamicStackArenaBytes !== 0x1000_0000 ||
    threads.dynamicEnvironmentArenaBase !== "0x7f030000" ||
    threads.dynamicEnvironmentArenaBytes !== 0x00c0_0000 ||
    threads.dynamicThreadStackAndEnvironmentPlanCommit !== true ||
    threads.dynamicThreadExitReleasesStackAndEnvironment !== true ||
    threads.dynamicThreadAddressReuseVerified !== true ||
    !Array.isArray(threads.perThreadState) ||
    threads.perThreadState.join(",") !==
      "last-error,tls-values,fls-values,callback-frames,stack-metadata,environment-metadata,guest-bootstrap,logical-affinity-mask,logical-priority,com-apartment" ||
    threads.priorityDefault !== "normal" ||
    threads.priorityMutation !== "owner-plan-stale-check-targeted-commit" ||
    threads.priorityPolicyAllowedValues?.join(",") !== "1" ||
    threads.priorityAffectsCooperativeSelection !== false ||
    threads.priorityHostSchedulerImported !== false ||
    threads.priorityBrowserSchedulerImported !== false ||
    threads.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime thread subsystem boundary is invalid");
  }
  const scheduler = requireRecord(
    active.runtimeScheduler,
    "activeRuntimeCheckpoint.runtimeScheduler",
    errors,
  );
  if (
    scheduler.schema !== "vine.runtime-scheduler.v5" ||
    scheduler.stateOwner !== "runtime/scheduler.rs::RuntimeScheduler" ||
    scheduler.mutableCollectionVisibility !== "private" ||
    scheduler.serializedRuntimeField !== "scheduler" ||
    !Array.isArray(scheduler.serializedFields) ||
    scheduler.serializedFields.join(",") !==
      "currentThreadId,threads,waitRegistrations,objectWaiters,events" ||
    !Array.isArray(scheduler.hostRequestSerializedRuntimeFields) ||
    scheduler.hostRequestSerializedRuntimeFields.join(",") !==
      "registries.hostRequests,nextHostRequestId" ||
    scheduler.hostRequestSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(scheduler.ownedCollections) ||
    scheduler.ownedCollections.join(",") !==
      "current-thread-selector,thread-lanes,event-fifo,host-request-table,host-request-id-allocator,api-boundary-quantum-counter" ||
    scheduler.waitSubsystemAdapter !== "serde-flatten-private-runtime-waits" ||
    scheduler.crossSubsystemMutation !==
      "scheduler-host-plan-aggregate-lifecycle-preflight-targeted-commit" ||
    scheduler.hostRequestTransition !==
      "scheduler-plan-infallible-targeted-commit" ||
    scheduler.subsystemTests < 9 ||
    scheduler.crossSubsystemAtomicTests < 8 ||
    scheduler.directCollectionMutationFromAggregate !== false ||
    scheduler.directHostRequestMutationFromAggregate !== false ||
    scheduler.hostRequestCancellationPlan !== true ||
    scheduler.hostCompletionPreflightBeforeRemoval !== true ||
    scheduler.positiveSuspendCountOwnedByScheduler !== true ||
    scheduler.resumeToRunnablePlanCommit !== true ||
    scheduler.synchronousResumePreemptsCaller !== false ||
    scheduler.apiBoundaryQuantum !== 64 ||
    scheduler.apiBoundaryCountsOnlyWithRunnableCompetitor !== true ||
    scheduler.apiBoundaryDisposition !==
      "continue-quantum-or-yield-current-thread" ||
    scheduler.explicitYieldRequiresRunnableCompetitor !== true ||
    scheduler.explicitYieldResetsQuantum !== true ||
    scheduler.explicitYieldCreatesHostWait !== false ||
    scheduler.apiBoundaryCallbackNonInterleaving !== true ||
    scheduler.apiBoundaryPlanMutationFree !== true ||
    scheduler.apiBoundaryStaleCommitRejected !== true ||
    scheduler.apiBoundaryDuplicateCommitRejected !== true ||
    scheduler.apiBoundaryRunnableSetStaleRejected !== true ||
    scheduler.apiBoundaryPlanRevisionPrivate !== true ||
    scheduler.apiBoundarySelectionOrder !== "round-robin-runtime-thread-id" ||
    scheduler.privateNodeChromeRegisteredThreadIds?.join(",") !== "8192,8193" ||
    scheduler.privateNodeChromeExecutedThreadIds?.join(",") !== "8192,8193" ||
    scheduler.privateNodeChromeContextSwitches !== 1 ||
    scheduler.privateSleepZeroNodeChromeSelectionCount !== sleepZeroCallCount ||
    scheduler.privateSleepZeroNodeChromeOrderedSemanticsMatched !== true ||
    scheduler.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime scheduler subsystem boundary is invalid");
  }
  const waits = requireRecord(
    active.runtimeWaitQueue,
    "activeRuntimeCheckpoint.runtimeWaitQueue",
    errors,
  );
  if (
    waits.schema !== "vine.runtime-wait-queue.v3" ||
    waits.stateOwner !== "runtime/waits.rs::RuntimeWaits" ||
    waits.mutableCollectionVisibility !== "private" ||
    waits.readInspection !== "immutable-borrow" ||
    !Array.isArray(waits.serializedSchedulerFields) ||
    waits.serializedSchedulerFields.join(",") !==
      "waitRegistrations,objectWaiters" ||
    waits.allocatorSerializedRuntimeField !== "nextWaitRegistrationId" ||
    waits.allocatorSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(waits.ownedCollections) ||
    waits.ownedCollections.join(",") !==
      "registration-table,object-fifo,registration-id-allocator" ||
    waits.crossSubsystemMutation !==
      "wait-allocation-plan-aggregate-validation-scheduler-plan-infallible-commit" ||
    waits.subsystemTests < 5 ||
    waits.crossSubsystemAtomicTests < 4 ||
    waits.directCollectionMutationFromAggregate !== false ||
    waits.directAllocatorMutationFromAggregate !== false ||
    waits.allocatorOverflowMutationFree !== true ||
    waits.excludedWakeCandidatesValidated !== true ||
    waits.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime wait subsystem boundary is invalid");
  }
  const messages = requireRecord(
    active.runtimeMessageQueue,
    "activeRuntimeCheckpoint.runtimeMessageQueue",
    errors,
  );
  if (
    messages.schema !== "vine.runtime-message-queue.v2" ||
    messages.mutableCollectionVisibility !== "private" ||
    messages.crossSubsystemMutation !== "plan-preflight-commit"
  ) {
    errors.push("active runtime message subsystem boundary is invalid");
  }
  const synchronization = requireRecord(
    active.runtimeSynchronization,
    "activeRuntimeCheckpoint.runtimeSynchronization",
    errors,
  );
  if (
    synchronization.schema !== "vine.runtime-synchronization.v2" ||
    synchronization.stateOwner !==
      "runtime/synchronization.rs::RuntimeSynchronization" ||
    synchronization.mutableCollectionVisibility !== "private" ||
    !Array.isArray(synchronization.serializedProcessFields) ||
    synchronization.serializedProcessFields.join(",") !==
      "criticalSections,mutexes" ||
    synchronization.crossSubsystemMutation !==
      "lifecycle-preflight-then-subsystem-plan-commit" ||
    synchronization.subsystemTests < 2 ||
    synchronization.crossSubsystemAtomicTests < 2 ||
    synchronization.criticalSectionLimit !== 4096 ||
    synchronization.mutexLimit !== 4096 ||
    synchronization.wholeRuntimeStateClone !== false
  ) {
    errors.push("active runtime synchronization subsystem boundary is invalid");
  }
  const heaps = requireRecord(
    active.runtimeHeapBacking,
    "activeRuntimeCheckpoint.runtimeHeapBacking",
    errors,
  );
  if (
    heaps.schema !== "vine.runtime-private-heap-backing.v5" ||
    heaps.stateOwner !== "runtime/heaps.rs::RuntimeHeaps" ||
    heaps.mutableCollectionVisibility !== "private" ||
    heaps.readInspection !== "immutable-borrow" ||
    heaps.serializedProcessField !== "heaps" ||
    heaps.serializationAdapter !== "serde-flatten-private-runtime-heaps" ||
    !Array.isArray(heaps.serializedRuntimeCompatibilityFields) ||
    heaps.serializedRuntimeCompatibilityFields.join(",") !==
      "nextPrivateHeapSlot" ||
    heaps.allocatorSerializationAdapter !==
      "runtime-state-borrowed-compatibility-view" ||
    !Array.isArray(heaps.ownedState) ||
    heaps.ownedState.join(",") !==
      "logical-heap-map,guest-memory-backing-map,private-heap-slot-allocator,per-heap-allocation-id-allocators,process-large-allocation-backing,large-allocation-materialized-page-index" ||
    heaps.crossSubsystemMutation !==
      "heap-slot-plan-object-memory-preflight-targeted-commit" ||
    heaps.subsystemTests < 7 ||
    heaps.crossSubsystemAtomicTests < 3 ||
    heaps.logicalAndBackingCollectionsCoOwned !== true ||
    heaps.directCollectionMutationFromAggregate !== false ||
    heaps.directAllocatorMutationFromAggregate !== false ||
    heaps.automaticAllocatorCommitAtomic !== true ||
    heaps.explicitCreationPreservesAllocator !== true ||
    heaps.allocatorExhaustionMutationFree !== true ||
    heaps.failedCreationPreservesSlotAllocator !== true ||
    heaps.runtimeStateDirectScalarAllocators !== 0 ||
    heaps.wholeRuntimeStateClone !== false ||
    heaps.largeSlotBase !== "0x40000000" ||
    heaps.largeSlotEndExclusive !== "0x44000000" ||
    heaps.largeSlotBytes !== 64 * 1024 * 1024 ||
    heaps.largeSlotAddressAllocationPurpose !== "private-heap-large-slot" ||
    heaps.largeSlotAddressAllocationLifetime !== "process-permanent" ||
    heaps.largeSlotConcurrentLiveAllocations !== 1 ||
    heaps.largeAllocationSizePolicy !== "exact-67108864-only" ||
    heaps.largeAllocationMaterializationStateOwner !==
      "runtime/heaps.rs::RuntimeHeaps" ||
    heaps.largeAllocationMemoryAuthority !== "rust-guest-memory" ||
    heaps.largeAllocationFailuresMutationFree !== true ||
    heaps.guestHeapAllocOrdinaryMaximumBytes !== 16 * 1024 * 1024 ||
    heaps.guestHeapAllocLargeExactBytes?.join(",") !== `${64 * 1024 * 1024}` ||
    heaps.guestHeapAllocIntermediateLargeSizes !== false
  ) {
    errors.push("active runtime heap subsystem boundary is invalid");
  }
  const addressSpace = requireRecord(
    active.processAddressSpaceAllocator,
    "activeRuntimeCheckpoint.processAddressSpaceAllocator",
    errors,
  );
  const addressPurposes = requireStringArray(
    addressSpace.registeredPurposes,
    "activeRuntimeCheckpoint.processAddressSpaceAllocator.registeredPurposes",
    errors,
  );
  if (
    addressSpace.schema !== "vine.process-address-space-allocator.v1" ||
    addressSpace.authority !== "rust-runtime-kernel" ||
    addressSpace.rootAllocationCount !== 7 ||
    addressSpace.heapSlotAllocationCount !== 32 ||
    addressSpace.largeHeapSlotAllocationCount !== 1 ||
    addressSpace.totalAllocationCount !==
      addressSpace.rootAllocationCount +
        addressSpace.heapSlotAllocationCount +
        addressSpace.largeHeapSlotAllocationCount ||
    addressPurposes.join(",") !==
      "import-thunks,callback-returns,dynamic-exports,private-heap-band,private-heap-slot,private-heap-large-slot,process-environment,thread-stack,thread-environment,com-endpoint,graphics-resource-arena,graphics-resource,gdi-resource,audio-resource-arena,audio-resource,virtual-memory" ||
    addressSpace.heapBackingUsesAllocationId !== true ||
    addressSpace.largeHeapBackingUsesAllocationId !== true ||
    addressSpace.largeHeapAddressAllocationPermanent !== true ||
    addressSpace.comBackingUsesAllocationId !== true ||
    addressSpace.comFinalReleaseRetiresAllocation !== true ||
    addressSpace.graphicsResourceArenaBase !== "0x44000000" ||
    addressSpace.graphicsResourceArenaBytes !== 16 * 1024 * 1024 ||
    addressSpace.graphicsResourceArenaPermanent !== true ||
    addressSpace.graphicsResourceBackingUsesAllocationId !== true ||
    addressSpace.graphicsResourceBatchReservationWithComBacking !== true ||
    addressSpace.graphicsResourceFinalReleaseRetiresAllocation !== true ||
    addressSpace.gdiResourceBackingUsesAllocationId !== true ||
    addressSpace.gdiResourceCreationWithGuestOutputAtomic !== true ||
    addressSpace.gdiResourceDeleteRetiresAllocation !== true ||
    addressSpace.audioResourceArenaBase !== "0x45000000" ||
    addressSpace.audioResourceArenaBytes !== 16 * 1024 * 1024 ||
    addressSpace.audioResourceArenaPermanent !== true ||
    addressSpace.audioResourceBackingUsesAllocationId !== true ||
    addressSpace.audioResourceLockReservationWithGuestOutputs !== true ||
    addressSpace.audioResourceFinalReleaseRetiresAllocation !== true ||
    addressSpace.controlMappingCommitUsesAllocationId !== true ||
    addressSpace.mappingDeltaBinaryVersion !== 2 ||
    addressSpace.mappingDeltaAllocationIdWidthBits !== 64 ||
    addressSpace.browserMirrorValidatesAllocationId !== true ||
    addressSpace.processEnvironmentReservationCommit !==
      "memory-runtime-atomic-plan" ||
    addressSpace.dynamicThreadStackRegistered !== true ||
    addressSpace.dynamicThreadEnvironmentRegistered !== true ||
    addressSpace.dynamicThreadReleaseAndReuseVerified !== true ||
    addressSpace.topDownReservation !==
      "highest-aligned-gap-within-bounded-range" ||
    addressSpace.topDownPlansRejectStaleCommit !== true ||
    addressSpace.virtualMemoryAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    addressSpace.virtualMemoryByteAuthority !== "rust-guest-memory" ||
    addressSpace.virtualMemoryRegionKind !== "anonymous" ||
    addressSpace.virtualMemoryReservationCommit !==
      "cross-owner-preflight-transactional-commit"
  ) {
    errors.push("active process address-space allocator boundary is invalid");
  }
  const kernel32Policy = requireRecord(
    active.kernel32CompatibilityPolicy,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy",
    errors,
  );
  requireString(
    kernel32Policy.source,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source",
    errors,
  );
  const heapAllocator = requireRecord(
    kernel32Policy.heapAllocator,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.heapAllocator",
    errors,
  );
  const systemInfo = requireRecord(
    kernel32Policy.systemInfo,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.systemInfo",
    errors,
  );
  const memoryStatus = requireRecord(
    kernel32Policy.memoryStatus,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.memoryStatus",
    errors,
  );
  const osVersionInfoA = requireRecord(
    kernel32Policy.osVersionInfoA,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.osVersionInfoA",
    errors,
  );
  const sourceFiles = requireRecord(
    kernel32Policy.sourceFiles,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.sourceFiles",
    errors,
  );
  const getFileAttributesA = getFileAttributesAActive
    ? requireRecord(
        sourceFiles.getFileAttributesA,
        "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.sourceFiles.getFileAttributesA",
        errors,
      )
    : null;
  const loadLibraryA = requireRecord(
    kernel32Policy.loadLibraryA,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.loadLibraryA",
    errors,
  );
  const virtualAlloc = requireRecord(
    kernel32Policy.virtualAlloc,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.virtualAlloc",
    errors,
  );
  const createThread = requireRecord(
    kernel32Policy.createThread,
    "activeRuntimeCheckpoint.kernel32CompatibilityPolicy.createThread",
    errors,
  );
  if (
    kernel32Policy.schema !== "vine.kernel32-compatibility-policy" ||
    kernel32Policy.schemaVersion !== 2 ||
    kernel32Policy.id !==
      (createDirectoryAActive
        ? "kernel32-create-directory-a-v1"
        : getFileAttributesAActive
          ? "kernel32-get-file-attributes-a-v1"
          : globalMemoryStatusActive
            ? "kernel32-global-memory-status-v1"
            : sleepZeroActive
              ? "kernel32-sleep-zero-v1"
              : "kernel32-wait-for-single-object-ex-v1") ||
    kernel32Policy.source !==
      (createDirectoryAActive
        ? "fixtures/runtime-policies/kernel32-create-directory-a-v1.json"
        : getFileAttributesAActive
          ? "fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json"
          : globalMemoryStatusActive
            ? "fixtures/runtime-policies/kernel32-global-memory-status-v1.json"
            : sleepZeroActive
              ? "fixtures/runtime-policies/kernel32-sleep-zero-v1.json"
              : "fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json") ||
    kernel32Policy.handlerDispatch !==
      "single-semantic-implementation-id-with-policy" ||
    kernel32Policy.evidenceSelectsImplementationBranch !== false ||
    kernel32Policy.semanticTestsRequireEvidenceProfile !== false ||
    kernel32Policy.activeAnsiCodePage !== 932 ||
    kernel32Policy.multiByteAllowActiveCodePageSelector !== true ||
    !Array.isArray(kernel32Policy.multiByteAllowedFlags) ||
    kernel32Policy.multiByteAllowedFlags.join(",") !== "0,1" ||
    kernel32Policy.setCurrentDirectoryProcessImageDirectoryOnly !== true ||
    kernel32Policy.setCurrentDirectoryMaximumPathBytes !== 32768 ||
    kernel32Policy.getDriveTypeProcessImageDriveOnly !== true ||
    kernel32Policy.getDriveTypeMaximumPathBytes !== 32768 ||
    kernel32Policy.getDriveTypeAuthority !==
      "runtime/process.rs::RuntimeProcessDirectories" ||
    kernel32Policy.hostDriveTopologyImported !== false ||
    kernel32Policy.browserDriveTopologyImported !== false ||
    kernel32Policy.createMutexAllowNamed !== true ||
    kernel32Policy.createMutexAllowUnnamed !== false ||
    kernel32Policy.createMutexMaximumNameBytes !== 32768 ||
    kernel32Policy.createWaitableTimerAllowManualReset !== true ||
    kernel32Policy.createWaitableTimerAllowAutoReset !== false ||
    kernel32Policy.createWaitableTimerAllowUnnamed !== true ||
    kernel32Policy.setWaitableTimerMinimumRelativeDueTime100ns !== 1 ||
    kernel32Policy.setWaitableTimerMaximumRelativeDueTime100ns !== 100_000 ||
    kernel32Policy.setWaitableTimerAllowOneShot !== true ||
    kernel32Policy.setWaitableTimerAllowCompletionRoutine !== false ||
    kernel32Policy.setWaitableTimerAllowResume !== false ||
    kernel32Policy.waitForSingleObjectExMaximumTimeoutMilliseconds !== 10 ||
    kernel32Policy.waitForSingleObjectExAllowAlertable !== true ||
    kernel32Policy.waitForSingleObjectExAllowNonalertable !== false ||
    kernel32Policy.waitForSingleObjectExWaitableTimerOnly !== true ||
    kernel32Policy.sleepZero !== sleepZeroActive ||
    memoryStatus.memoryLoadPercent !== 0 ||
    memoryStatus.totalPhysicalBytes !== 0x7fff_ffff ||
    memoryStatus.availablePhysicalBytes !== 0x7fff_ffff ||
    memoryStatus.totalPageFileBytes !== 0xffff_ffff ||
    memoryStatus.availablePageFileBytes !== 0xffff_ffff ||
    memoryStatus.availableVirtualBytes !== 0x6000_0000 ||
    sourceFiles.authority !== "runtime/files.rs::RuntimeFiles" ||
    sourceFiles.sourceManifestAuthority !== "rust-runtime-files" ||
    sourceFiles.browserRole !== "same-origin-byte-service" ||
    sourceFiles.allowedDesiredAccess?.join(",") !== `${0x8000_0000}` ||
    sourceFiles.allowedShareModes?.join(",") !== "1" ||
    sourceFiles.allowedCreationDispositions?.join(",") !== "3" ||
    sourceFiles.allowedFlagsAndAttributes?.join(",") !== `${0x0800_0020}` ||
    sourceFiles.maximumPathBytes !== 32_768 ||
    sourceFiles.maximumReadBytes !== 1_048_576 ||
    sourceFiles.handleNamespace !== "vine.runtime-source-file.v1" ||
    sourceFiles.mutation !== "owner-plan-stale-check-targeted-commit" ||
    sourceFiles.completionValidation !==
      "request-thread-generation-lifetime-before-commit" ||
    sourceFiles.immutableSourceAndWritableOverlaySeparated !== true ||
    sourceFiles.browserOwnsWindowsFileState !== false ||
    (getFileAttributesAActive &&
      (getFileAttributesA?.regularFileAttributes !== 0x20 ||
        getFileAttributesA?.directoryAttributes !== 0x30)) ||
    (createDirectoryAActive
      ? sourceFiles.createDirectoryA !== true
      : sourceFiles.createDirectoryA !== undefined) ||
    loadLibraryA.maximumModuleNameBytes !== 260 ||
    JSON.stringify(loadLibraryA.runtimeSystemModules) !==
      JSON.stringify([
        {
          canonicalName: "d3dx9_43.dll",
          exports: ["D3DXCreateEffect"],
        },
      ]) ||
    loadLibraryA.moduleAuthority !== "runtime/modules.rs::RuntimeModules" ||
    loadLibraryA.objectAuthority !== "runtime/objects.rs::RuntimeObjectTable" ||
    loadLibraryA.exportAuthority !==
      "runtime/registries.rs::RuntimeRegistries" ||
    loadLibraryA.browserOwnsWindowsModuleState !== false ||
    loadLibraryA.nativeImageMapping !== false ||
    loadLibraryA.generalSearch !== false ||
    loadLibraryA.dllMainCallbacks !== false ||
    loadLibraryA.unload !== false ||
    createThread.defaultStackReserveBytes !== 1_048_576 ||
    createThread.defaultStackCommitBytes !== 4_096 ||
    createThread.allowedCreationFlags?.join(",") !== "0,4" ||
    createThread.maximumThreads !== 256 ||
    createThread.threadAuthority !== "runtime/thread.rs::RuntimeThreads" ||
    createThread.schedulerAuthority !==
      "runtime/scheduler.rs::RuntimeScheduler" ||
    createThread.objectAuthority !== "runtime/objects.rs::RuntimeObjectTable" ||
    createThread.memoryAuthority !== "rust-guest-memory" ||
    createThread.browserOwnsWindowsThreadState !== false ||
    kernel32Policy.threadAffinityProcessMask !== 1 ||
    kernel32Policy.threadPriorityAllowedPriorities?.join(",") !== "1" ||
    systemInfo.processorArchitecture !== 0 ||
    systemInfo.pageSize !== 4096 ||
    systemInfo.minimumApplicationAddress !== 0x00010000 ||
    systemInfo.maximumApplicationAddress !== 0x7ffeffff ||
    systemInfo.processorType !== 586 ||
    systemInfo.allocationGranularity !== 65536 ||
    systemInfo.processorLevel !== 6 ||
    systemInfo.processorRevision !== 0 ||
    kernel32Policy.systemInfoActiveProcessorMaskSource !==
      "thread-affinity-process-mask" ||
    kernel32Policy.systemInfoProcessorCountSource !==
      "active-processor-mask-popcount" ||
    virtualAlloc.requireNullPreferredAddress !== true ||
    virtualAlloc.allowedReserveByteLengths?.join(",") !==
      `${96 * 1024 * 1024}` ||
    virtualAlloc.allowedAllocationTypes?.join(",") !== `${0x0010_2000}` ||
    virtualAlloc.allowedProtections?.join(",") !== "4" ||
    virtualAlloc.addressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    virtualAlloc.memoryAuthority !== "rust-guest-memory" ||
    virtualAlloc.regionKind !== "anonymous" ||
    virtualAlloc.reservationAlignment !== 65_536 ||
    virtualAlloc.commitment !== "reserve-only-no-physical-pages" ||
    virtualAlloc.mutation !== "cross-owner-preflight-transactional-commit" ||
    virtualAlloc.hostAddressImported !== false ||
    virtualAlloc.browserAddressImported !== false ||
    kernel32Policy.systemDefaultLangId !== 0x0411 ||
    kernel32Policy.systemDefaultLangIdSource !== "explicit-policy-field" ||
    kernel32Policy.systemDefaultLangIdDerivedFromAllowedLocales !== false ||
    kernel32Policy.hostLocaleImported !== false ||
    osVersionInfoA.majorVersion !== 10 ||
    osVersionInfoA.minorVersion !== 0 ||
    osVersionInfoA.buildNumber !== 19045 ||
    osVersionInfoA.platformId !== 2 ||
    osVersionInfoA.servicePack !== "" ||
    osVersionInfoA.servicePackMajor !== 0 ||
    osVersionInfoA.servicePackMinor !== 0 ||
    osVersionInfoA.suiteMask !== 0x0100 ||
    osVersionInfoA.productType !== 1 ||
    kernel32Policy.osVersionInfoAAuthority !==
      "Kernel32CompatibilityPolicy.os_version_info_a" ||
    kernel32Policy.osVersionInfoASupportedDeclaredSizes?.join(",") !==
      "148,156" ||
    kernel32Policy.osVersionInfoAHostVersionImported !== false ||
    kernel32Policy.osVersionInfoABrowserVersionImported !== false ||
    kernel32Policy.tickCountIncrementPerRead !== 1 ||
    kernel32Policy.performanceCounterFrequency !== 10_000_000 ||
    kernel32Policy.performanceCounterIncrementPerRead !== 10_000 ||
    kernel32Policy.performanceCounterAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    kernel32Policy.performanceCounterHostClockImported !== false ||
    kernel32Policy.performanceCounterBrowserClockImported !== false ||
    !Array.isArray(heapAllocator.allowedFlags) ||
    heapAllocator.allowedFlags.join(",") !== "0,8" ||
    heapAllocator.minimumAllocationBytes !== 1 ||
    heapAllocator.maximumAllocationBytes !== 16 * 1024 * 1024 ||
    !Array.isArray(heapAllocator.largeAllocationBytes) ||
    heapAllocator.largeAllocationBytes.join(",") !== `${64 * 1024 * 1024}` ||
    heapAllocator.sizePolicy !==
      "ordinary-inclusive-nonzero-range-plus-explicit-large-variants" ||
    kernel32Policy.historicalExactHeapPolicySchemaVersion !== 1 ||
    kernel32Policy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active Kernel32 compatibility policy boundary is invalid");
  }
  const writableProfile = requireRecord(
    active.writableProfile,
    "activeRuntimeCheckpoint.writableProfile",
    errors,
  );
  if (
    writableProfile.schema !== "vine.writable-profile-snapshot" ||
    writableProfile.schemaVersion !== 3 ||
    writableProfile.titleFingerprint !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    writableProfile.profileId !== "default" ||
    writableProfile.storagePathTemplate !==
      ".work/private-profiles/<title-fingerprint>/default.sqlite" ||
    writableProfile.rootPathA?.join(",") !== "67,58,92,86,105,110,101,0" ||
    writableProfile.quotaBytes !== 16 * 1024 * 1024 ||
    writableProfile.fileCount !== 1 ||
    writableProfile.objectId !== 1 ||
    writableProfile.relativePathA?.join(",") !== "66,71,73,46,103,100,98" ||
    writableProfile.directoryCount !== 1 ||
    writableProfile.directoryId !== 1 ||
    writableProfile.directoryRelativePathA?.join(",") !==
      "85,115,101,114,68,97,116,97" ||
    writableProfile.directoryInitialExists !== false ||
    writableProfile.directoryInitialGeneration !== 0 ||
    writableProfile.directoryStateAuthority !== "rust-runtime-vfs" ||
    writableProfile.directoryDurableStateAuthority !== "same-origin-sqlite" ||
    writableProfile.directoryCreateService !==
      "storage.profile-directory-create.v1" ||
    writableProfile.directoryCommit !==
      "validate-plan-host-publish-targeted-commit" ||
    writableProfile.reservedReadSourceId !== 0 ||
    writableProfile.initialization !==
      "distribution-seed-only-when-database-is-new" ||
    writableProfile.stagedStateAuthority !== "rust-runtime-vfs" ||
    writableProfile.durableStateAuthority !== "same-origin-sqlite" ||
    writableProfile.httpApi !== "same-origin-profile-v3" ||
    writableProfile.bootstrap !==
      "metadata-json-plus-bounded-binary-object-hydration" ||
    writableProfile.maximumObjectTransferBytes !== 1024 * 1024 ||
    writableProfile.sessionLease !==
      "sqlite-epoch-lease-transaction-revalidated" ||
    writableProfile.terminalCommitBarrier !==
      "drain-then-revoke-before-terminal" ||
    writableProfile.recoverableFailureCompletion !==
      "win32-binary-preserve-dirty-staging" ||
    writableProfile.distributionFileWritable !== false ||
    writableProfile.profileDatabaseGitCommitted !== false
  ) {
    errors.push("active writable profile boundary is invalid");
  }
  const gdi32Policy = requireRecord(
    active.gdi32CompatibilityPolicy,
    "activeRuntimeCheckpoint.gdi32CompatibilityPolicy",
    errors,
  );
  const gdi32CallbackArguments = requireStringArray(
    gdi32Policy.callbackArgumentOrder,
    "activeRuntimeCheckpoint.gdi32CompatibilityPolicy.callbackArgumentOrder",
    errors,
  );
  const gdi32CreationArguments = requireOrderedIntegerArray(
    gdi32Policy.creationArguments,
    "activeRuntimeCheckpoint.gdi32CompatibilityPolicy.creationArguments",
    errors,
  );
  if (
    gdi32Policy.schema !== "vine.gdi32-compatibility-policy" ||
    gdi32Policy.schemaVersion !== 6 ||
    gdi32Policy.id !== "gdi32-primary-cp932-text-raster-v6" ||
    gdi32Policy.source !==
      "crates/vine-core/src/gdi32.rs::Gdi32CompatibilityPolicy::controlled_v6" ||
    gdi32Policy.handlerDispatch !==
      "single-semantic-implementation-id-with-policy" ||
    gdi32Policy.enumerationProfileId !== "primary-cp932-ms-gothic-regular-v1" ||
    gdi32Policy.inputCharacterSet !== 128 ||
    gdi32Policy.inputFaceNameWindows932Hex !== "826c827220835383568362834e" ||
    gdi32Policy.flags !== 0 ||
    gdi32Policy.recordCount !== 1 ||
    gdi32Policy.fontType !== 4 ||
    gdi32Policy.enumLogFontExABytes !== 188 ||
    gdi32Policy.newTextMetricExABytes !== 96 ||
    gdi32Policy.callbackCallingConvention !== "x86-stdcall" ||
    gdi32CallbackArguments.join(",") !==
      "enum-log-font,text-metric,font-type,lparam" ||
    gdi32Policy.callbackResultPropagation !== "outer-result" ||
    gdi32Policy.lastErrorPolicy !== "preserve" ||
    gdi32Policy.screenHdcValidation !==
      "live-same-thread-primary-monitor-relation" ||
    gdi32Policy.canonicalTailInitialization !== "zero" ||
    gdi32Policy.nativeUninitializedTailImported !== false ||
    gdi32Policy.hostFontDiscoveryImported !== false ||
    gdi32Policy.browserFontDiscoveryImported !== false ||
    gdi32Policy.creationProfileId !== "primary-cp932-ms-gothic-64x32-thin-v1" ||
    gdi32CreationArguments.join(",") !== "64,32,0,0,100,0,0,0,128,4,0,2,0" ||
    gdi32Policy.creationFaceNameWindows932Hex !==
      "826c827220835383568362834e" ||
    gdi32Policy.creationCallingConvention !== "x86-stdcall" ||
    gdi32Policy.creationLastErrorPolicy !== "preserve" ||
    gdi32Policy.creationObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi32Policy.creationHostFontResolutionImported !== false ||
    gdi32Policy.creationBrowserFontResolutionImported !== false ||
    gdi32Policy.compatibleDcProfileId !== "primary-null-screen-memory-dc-v1" ||
    gdi32Policy.compatibleDcSourceDeviceContext !== "0x00000000" ||
    gdi32Policy.compatibleDcInitialFontStockIndex !== 13 ||
    gdi32Policy.compatibleDcInitialBitmapStockIndex !== 21 ||
    gdi32Policy.compatibleDcInitialBitmapGeometry?.join(",") !== "1,1,1" ||
    gdi32Policy.compatibleDcObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi32Policy.compatibleDcDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    gdi32Policy.compatibleDcOwnerThreadRelation !==
      "creating-thread-exact-delete" ||
    gdi32Policy.compatibleDcLastErrorPolicy !==
      "preserve-success-and-failure" ||
    gdi32Policy.compatibleDcHostDeviceContextImported !== false ||
    gdi32Policy.compatibleDcBrowserResourceImported !== false ||
    gdi32Policy.dibSectionProfileId !==
      "primary-128x-96-8bpp-reverse-grayscale-v1" ||
    gdi32Policy.dibSectionHeaderBytes !== 40 ||
    gdi32Policy.dibSectionWidth !== 128 ||
    gdi32Policy.dibSectionHeight !== -96 ||
    gdi32Policy.dibSectionPlanes !== 1 ||
    gdi32Policy.dibSectionBitsPerPixel !== 8 ||
    gdi32Policy.dibSectionCompression !== 0 ||
    gdi32Policy.dibSectionColorsUsed !== 256 ||
    gdi32Policy.dibSectionPalette !== "reverse-grayscale-rgbquad" ||
    gdi32Policy.dibSectionStrideBytes !== 128 ||
    gdi32Policy.dibSectionPixelBytes !== 12_288 ||
    gdi32Policy.dibSectionUsage !== 0 ||
    gdi32Policy.dibSectionSectionHandle !== "0x00000000" ||
    gdi32Policy.dibSectionOffset !== 0 ||
    gdi32Policy.dibSectionObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi32Policy.dibSectionByteAuthority !== "rust-guest-memory" ||
    gdi32Policy.dibSectionAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    gdi32Policy.dibSectionBrowserResourceImported !== false ||
    gdi32Policy.outlineTextMetricsProfileId !==
      "primary-cp932-ms-gothic-outline-metrics-v1" ||
    gdi32Policy.outlineTextMetricsFixedBytes !== 212 ||
    gdi32Policy.outlineTextMetricsRequiredBytes !== 269 ||
    gdi32Policy.outlineTextMetricsPartialSizeMarker !== 290 ||
    gdi32Policy.outlineTextMetricsStringOffsets?.join(",") !==
      "212,226,240,245" ||
    gdi32Policy.outlineTextMetricsOutputSha256 !==
      "d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d" ||
    gdi32Policy.outlineTextMetricsSelectedFontAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    gdi32Policy.outlineTextMetricsByteAuthority !== "rust-guest-memory" ||
    gdi32Policy.outlineTextMetricsNativePaddingPolicy !==
      "zero-four-nondeterministic-padding-bytes" ||
    gdi32Policy.outlineTextMetricsHostFontMeasurementImported !== false ||
    gdi32Policy.outlineTextMetricsBrowserFontMeasurementImported !== false ||
    gdi32Policy.textRasterProfileId !==
      "primary-cp932-ms-gothic-a-8bpp-raster-v1" ||
    gdi32Policy.textRasterPosition?.join(",") !== "0,0" ||
    gdi32Policy.textRasterTextWindows932Hex !== "61" ||
    gdi32Policy.textRasterCellTop !== 20 ||
    gdi32Policy.textRasterCellWidth !== 32 ||
    gdi32Policy.textRasterRowCount !== 35 ||
    gdi32Policy.textRasterChangedBytes !== 476 ||
    gdi32Policy.textRasterInitialPixel !== 0 ||
    gdi32Policy.textRasterForegroundPixel !== 255 ||
    gdi32Policy.textRasterInitialPixelPlaneFnv1a32 !== "0x52965dc5" ||
    gdi32Policy.textRasterOutputPixelPlaneFnv1a32 !== "0xe6fc8461" ||
    gdi32Policy.textRasterObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    gdi32Policy.textRasterByteAuthority !== "rust-guest-memory" ||
    gdi32Policy.textRasterMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    gdi32Policy.textRasterHostFontRasterizationImported !== false ||
    gdi32Policy.textRasterBrowserFontRasterizationImported !== false ||
    gdi32Policy.textRasterBrowserBitmapResourceImported !== false ||
    gdi32Policy.failureAtomicity !== true
  ) {
    errors.push("active GDI32 compatibility policy boundary is invalid");
  }
  const gdiplusPolicy = requireRecord(
    active.gdiplusCompatibilityPolicy,
    "activeRuntimeCheckpoint.gdiplusCompatibilityPolicy",
    errors,
  );
  if (
    gdiplusPolicy.schema !== "vine.gdiplus-compatibility-policy" ||
    gdiplusPolicy.schemaVersion !== 1 ||
    gdiplusPolicy.id !== "gdiplus-primary-startup-v1" ||
    gdiplusPolicy.source !==
      "crates/vine-core/src/gdiplus.rs::GdiplusCompatibilityPolicy::controlled_v1" ||
    gdiplusPolicy.handlerDispatch !==
      "single-semantic-implementation-id-with-policy" ||
    gdiplusPolicy.startupProfileId !== "primary-x86-callback-free-v1" ||
    gdiplusPolicy.x86InputBytes !== 16 ||
    gdiplusPolicy.version !== 1 ||
    gdiplusPolicy.debugEventCallback !== "0x00000000" ||
    gdiplusPolicy.suppressBackgroundThread !== false ||
    gdiplusPolicy.suppressExternalCodecs !== false ||
    gdiplusPolicy.notificationOutput !== "0x00000000" ||
    gdiplusPolicy.statusOk !== 0 ||
    gdiplusPolicy.lastErrorPolicy !== "preserve" ||
    gdiplusPolicy.sessionAuthority !== "runtime/gdiplus.rs::RuntimeGdiplus" ||
    gdiplusPolicy.tokenNamespace !== "vine.logical-gdiplus-session-token.v1" ||
    gdiplusPolicy.tokenPrefix !== "0x47500000" ||
    gdiplusPolicy.tokenMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    gdiplusPolicy.shutdownRetiresExactSession !== true ||
    gdiplusPolicy.processExitRetiresSessions !== true ||
    gdiplusPolicy.nativeTokenImported !== false ||
    gdiplusPolicy.browserResourceCreated !== false ||
    gdiplusPolicy.imageCodecImported !== false ||
    gdiplusPolicy.failureAtomicity !== true
  ) {
    errors.push("active GDI+ compatibility policy boundary is invalid");
  }
  const user32Policy = requireRecord(
    active.user32CompatibilityPolicy,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy",
    errors,
  );
  const primaryMonitorRect = requireRecord(
    user32Policy.primaryMonitorRect,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.primaryMonitorRect",
    errors,
  );
  const primaryWorkRect = requireRecord(
    user32Policy.primaryWorkRect,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.primaryWorkRect",
    errors,
  );
  const enumerationVariants = requireStringArray(
    user32Policy.enumerationVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.enumerationVariants",
    errors,
  );
  const monitorFromWindowVariants = requireStringArray(
    user32Policy.monitorFromWindowVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.monitorFromWindowVariants",
    errors,
  );
  const swapMouseButtonVariants = requireStringArray(
    user32Policy.swapMouseButtonVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.swapMouseButtonVariants",
    errors,
  );
  const loadIconVariants = requireStringArray(
    user32Policy.loadIconVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadIconVariants",
    errors,
  );
  const loadImageAVariants = requireStringArray(
    user32Policy.loadImageAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadImageAVariants",
    errors,
  );
  const sendMessageAVariants = requireStringArray(
    user32Policy.sendMessageAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.sendMessageAVariants",
    errors,
  );
  const setWindowPosVariants = requireStringArray(
    user32Policy.setWindowPosVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.setWindowPosVariants",
    errors,
  );
  const setWindowLongAVariants = requireStringArray(
    user32Policy.setWindowLongAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.setWindowLongAVariants",
    errors,
  );
  const loadCursorVariants = requireStringArray(
    user32Policy.loadCursorVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.loadCursorVariants",
    errors,
  );
  const registerClassExAVariants = requireStringArray(
    user32Policy.registerClassExAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.registerClassExAVariants",
    errors,
  );
  const registerClassExAAllowedStyles = requireIntegerArray(
    user32Policy.registerClassExAAllowedStyles,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.registerClassExAAllowedStyles",
    errors,
  );
  const getSystemMetricsIndices = requireIntegerArray(
    user32Policy.getSystemMetricsIndices,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.getSystemMetricsIndices",
    errors,
  );
  const createWindowExAVariants = requireStringArray(
    user32Policy.createWindowExAVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.createWindowExAVariants",
    errors,
  );
  const windowCreationMessages = requireStringArray(
    user32Policy.windowCreationMessages,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.windowCreationMessages",
    errors,
  );
  const callbackScratchArena = requireRecord(
    user32Policy.callbackScratchArena,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.callbackScratchArena",
    errors,
  );
  const defWindowProcAImplementedMessages = requireIntegerArray(
    user32Policy.defWindowProcAImplementedMessages,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.defWindowProcAImplementedMessages",
    errors,
  );
  const systemMenuVariants = requireStringArray(
    user32Policy.systemMenuVariants,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.systemMenuVariants",
    errors,
  );
  const getSystemMenuFailure = requireRecord(
    user32Policy.getSystemMenuFailure,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.getSystemMenuFailure",
    errors,
  );
  const enableMenuItemFailure = requireRecord(
    user32Policy.enableMenuItemFailure,
    "activeRuntimeCheckpoint.user32CompatibilityPolicy.enableMenuItemFailure",
    errors,
  );
  if (
    user32Policy.schema !== "vine.user32-compatibility-policy" ||
    user32Policy.schemaVersion !== 26 ||
    user32Policy.id !==
      "user32-single-monitor-window-frame-change-show-update-paint-v26" ||
    user32Policy.source !==
      "crates/vine-core/src/user32.rs::User32CompatibilityPolicy::controlled_v26" ||
    user32Policy.handlerDispatch !==
      "single-semantic-implementation-id-with-policy" ||
    user32Policy.logicalMonitorAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    user32Policy.displayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    user32Policy.monitorHandleNamespace !== "vine.logical-monitor.v1" ||
    user32Policy.primaryMonitorHandle !== "0x4f010001" ||
    primaryMonitorRect.left !== 0 ||
    primaryMonitorRect.top !== 0 ||
    primaryMonitorRect.right !== 1920 ||
    primaryMonitorRect.bottom !== 1080 ||
    primaryWorkRect.left !== 0 ||
    primaryWorkRect.top !== 0 ||
    primaryWorkRect.right !== 1920 ||
    primaryWorkRect.bottom !== 1032 ||
    enumerationVariants.join(",") !== "null-hdc-null-clip" ||
    user32Policy.monitorFromWindowProfileId !==
      "live-window-default-to-primary-v1" ||
    user32Policy.monitorFromWindowFlags !== 1 ||
    monitorFromWindowVariants.join(",") !== "live-window-default-to-primary" ||
    user32Policy.monitorFromWindowReturnNamespace !==
      "vine.logical-monitor.v1" ||
    user32Policy.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    user32Policy.monitorFromWindowMutation !==
      "logical-display-authority-read-only" ||
    user32Policy.monitorFromWindowFailureAtomicity !== true ||
    user32Policy.monitorFromWindowHostDisplayTopologyImported !== false ||
    user32Policy.monitorFromWindowBrowserDisplayTopologyImported !== false ||
    user32Policy.getMonitorInfoAProfileId !==
      "logical-primary-monitor-info-40-v1" ||
    user32Policy.getMonitorInfoAStructureBytes !== 40 ||
    user32Policy.getMonitorInfoAExtendedVariantImplemented !== false ||
    user32Policy.getMonitorInfoAInvalidHandleLastError !== 1_461 ||
    user32Policy.getMonitorInfoAInvalidSizeLastError !== 87 ||
    user32Policy.getMonitorInfoAMutation !==
      "atomic-guest-structure-publication" ||
    user32Policy.getMonitorInfoAFailureAtomicity !== true ||
    user32Policy.getMonitorInfoAHostDisplayTopologyImported !== false ||
    user32Policy.getMonitorInfoABrowserDisplayTopologyImported !== false ||
    user32Policy.screenDcProfileId !==
      "null-window-primary-screen-same-thread-v1" ||
    user32Policy.screenDcRequirePrimaryMonitor !== true ||
    user32Policy.screenDcRequireSameThreadRelease !== true ||
    user32Policy.screenDcLastErrorPolicy !==
      "preserve-success-and-failed-release" ||
    user32Policy.screenDcHostDisplayImported !== false ||
    user32Policy.screenDcBrowserDisplayImported !== false ||
    user32Policy.screenDcFailureAtomicity !== true ||
    user32Policy.logicalInputAuthority !== "runtime/input.rs::RuntimeInput" ||
    user32Policy.inputProfileId !== "normal-mouse-buttons-v1" ||
    user32Policy.initialMouseButtonsSwapped !== false ||
    swapMouseButtonVariants.join(",") !== "zero-normalize,nonzero-swap" ||
    user32Policy.swapMouseButtonReturn !== "previous-mapping-state" ||
    user32Policy.swapMouseButtonMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.idempotentSwapPreservesGeneration !== true ||
    user32Policy.callbackCompletion !== "zero-false-nonzero-true" ||
    user32Policy.lastErrorPolicy !== "preserve" ||
    user32Policy.configuration !== "configure-once-plan-commit" ||
    user32Policy.hostDisplayTopologyImported !== false ||
    user32Policy.browserDisplayTopologyImported !== false ||
    user32Policy.hostMouseButtonMappingImported !== false ||
    user32Policy.browserMouseButtonMappingImported !== false ||
    user32Policy.logicalIconAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.loadIconProfileId !== "module-shared-large-icon-32x32-v1" ||
    user32Policy.loadIconTargetWidth !== 32 ||
    user32Policy.loadIconTargetHeight !== 32 ||
    user32Policy.loadIconMaximumGroupEntries !== 64 ||
    loadIconVariants.join(",") !== "mapped-module-integer-resource" ||
    user32Policy.loadIconLastErrorPolicy !==
      "handler-success-zero-specific-failure-code" ||
    user32Policy.iconHandleNamespace !== "vine.logical-icon.v1" ||
    user32Policy.firstIconHandle !== "0x49010001" ||
    user32Policy.loadImageAProfileId !== "module-shared-small-icon-16x16-v1" ||
    user32Policy.loadImageAImageType !== 1 ||
    user32Policy.loadImageATargetWidth !== 16 ||
    user32Policy.loadImageATargetHeight !== 16 ||
    user32Policy.loadImageAFlags !== 32_768 ||
    user32Policy.loadImageAMaximumGroupEntries !== 64 ||
    loadImageAVariants.join(",") !==
      "mapped-module-integer-resource-image-icon-lr-shared" ||
    user32Policy.loadImageALastErrorPolicy !==
      "handler-success-zero-specific-failure-code" ||
    user32Policy.sendMessageAProfileId !== "same-thread-wm-seticon-small-v1" ||
    sendMessageAVariants.join(",") !==
      "same-thread-created-window-wm-seticon-icon-small-live-shared-icon" ||
    user32Policy.sendMessageAMessage !== 128 ||
    user32Policy.sendMessageAWParam !== 0 ||
    user32Policy.sendMessageAIconWidth !== 16 ||
    user32Policy.sendMessageAIconHeight !== 16 ||
    user32Policy.sendMessageACallbackContract !==
      "registered-class-wndproc-stdcall-four-u32-no-scratch-return-u32" ||
    user32Policy.sendMessageALastErrorPolicy !== "preserve" ||
    user32Policy.sendMessageAMutation !==
      "defwindowproc-wm-seticon-owner-plan-targeted-commit" ||
    user32Policy.sendMessageAFailureAtomicity !== true ||
    user32Policy.setWindowPosProfileId !==
      "initial-offscreen-show-hide-same-geometry-activation-resize-frame-change-move-v4" ||
    setWindowPosVariants.join(",") !==
      "initial-offscreen-show,initial-hide-preserve-position,same-geometry-activation,primary-1286x749-client-1280x720-resize,primary-frame-change-move-to-317x154" ||
    user32Policy.setWindowPosInsertAfter !== "0xfffffffe" ||
    user32Policy.setWindowPosInitialShowScreenMultiplier !== 8 ||
    user32Policy.setWindowPosInitialShowFlags !== 1_369 ||
    user32Policy.setWindowPosInitialHideFlags !== 1_435 ||
    user32Policy.setWindowPosInitialShowCallbackFlags !== 268_438_873 ||
    user32Policy.setWindowPosInitialHideCallbackFlags !== 7_583 ||
    user32Policy.setWindowPosGetIconWParam !== 2 ||
    user32Policy.setWindowPosActivationFlags !== 266 ||
    user32Policy.setWindowPosActivationInternalFlags !== 3 ||
    user32Policy.setWindowPosActivationInsertAfter !== "0x00000000" ||
    user32Policy.setWindowPosResizeOuterGeometry?.join(",") !== "1286,749" ||
    user32Policy.setWindowPosResizeClientGeometry?.join(",") !== "1280,720" ||
    user32Policy.setWindowPosResizeNccalcWindowPosFlags !== "0x0000190e" ||
    user32Policy.setWindowPosResizeChangedFlags !== "0x0000110e" ||
    user32Policy.setWindowPosResizeCommitPoint !==
      "after-wm-nccalcsize-before-wm-size" ||
    user32Policy.setWindowPosResizeSurfacePolicy !==
      "preserve-logical-surface-identity-clear-fill-history-nocopybits" ||
    user32Policy.setWindowPosResizeCrossSubsystemAtomicity !==
      "user32-and-gdi-preflight-before-infallible-targeted-commit" ||
    user32Policy.setWindowPosResizeLiveWindowDcPolicy !==
      "reject-before-user32-or-gdi-mutation" ||
    user32Policy.setWindowPosFrameChangeInsertAfter !== "0x00000000" ||
    user32Policy.setWindowPosFrameChangeMovePosition?.join(",") !== "317,154" ||
    user32Policy.setWindowPosFrameChangeMoveFlags !== "0x00000025" ||
    user32Policy.setWindowPosFrameChangeNccalcWindowPosFlags !== "0x00001825" ||
    user32Policy.setWindowPosFrameChangeChangedFlags !== "0x00000825" ||
    user32Policy.setWindowPosFrameChangeCommitPoint !==
      "after-wm-nccalcsize-before-wm-windowposchanged" ||
    user32Policy.setWindowPosFrameChangeRelationPolicy !==
      "preserve-active-focus-foreground-and-topmost" ||
    user32Policy.setWindowPosFrameChangeSurfacePolicy !==
      "preserve-logical-surface-and-fill-history" ||
    user32Policy.setWindowPosFrameChangeNonClientPaintAndEraseModeled !==
      false ||
    user32Policy.setWindowPosActivateAppOtherThreadId !== 0 ||
    user32Policy.setWindowPosImeSetContextFlags !== "0xc000000f" ||
    user32Policy.setWindowPosImeNotifyCommand !== 2 ||
    user32Policy.setWindowPosCallbackContract !==
      "initial-wm-windowposchanged-depth-two-wm-move-same-depth-wm-geticon-or-activation-two-wm-windowposchanging-wm-activateapp-wm-ncactivate-wm-activate-depth-three-ime-and-focus-or-resize-wm-windowposchanging-wm-nccalcsize-wm-size-wm-windowposchanged-or-frame-change-wm-windowposchanging-wm-nccalcsize-wm-windowposchanged-nested-wm-move" ||
    user32Policy.setWindowPosLastErrorPolicy !==
      "preserve-success-invalid-window-1400" ||
    user32Policy.setWindowPosMutation !==
      "owner-operation-generation-callback-plan-targeted-commit" ||
    user32Policy.setWindowPosActivationAuthority !==
      "runtime/user32.rs::RuntimeUser32-thread-active-focus-process-foreground-relations" ||
    user32Policy.setWindowPosActivationCommitOrder !==
      "first-windowposchanging-return-active-and-foreground-then-wm-activate-default-focus" ||
    user32Policy.setWindowPosHostIdentityNormalization !==
      "native-z-order-hwnd-and-external-thread-id-not-imported" ||
    user32Policy.setWindowPosFailureAtomicity !== true ||
    user32Policy.showWindowProfileId !==
      "already-visible-active-normal-noop-v1" ||
    user32Policy.showWindowNormalCommand !== 1 ||
    user32Policy.showWindowRequiredRelations !==
      "visible-active-focused-foreground" ||
    user32Policy.showWindowMutation !== false ||
    user32Policy.showWindowCallbackCount !== 0 ||
    user32Policy.showWindowFailureAtomicity !== true ||
    user32Policy.updateWindowProfileId !==
      "visible-active-full-client-paint-v1" ||
    user32Policy.updateWindowMessage !== 15 ||
    user32Policy.updateWindowCallbackResult !== 0 ||
    user32Policy.updateWindowPaintStructBytes !== 64 ||
    user32Policy.updateWindowErasePending !== true ||
    user32Policy.updateWindowRequiredRelations !==
      "visible-active-focused-foreground" ||
    user32Policy.updateWindowUpdateRegionAuthority !==
      "runtime/user32.rs::RuntimeUser32-window-update-region" ||
    user32Policy.updateWindowPaintSessionAuthority !==
      "runtime/user32.rs::RuntimeUser32-paint-session" ||
    user32Policy.updateWindowDeviceContextAuthority !==
      "runtime/gdi.rs::RuntimeGdi-device-context" ||
    user32Policy.updateWindowTransaction !==
      "user32-gdi-guest-memory-preflight-before-targeted-commit" ||
    user32Policy.updateWindowFailureAtomicity !== true ||
    user32Policy.setWindowLongAProfileId !==
      "main-window-gwl-style-visible-v1" ||
    setWindowLongAVariants.join(",") !==
      "created-window-gwl-style-reached-value,invalid-window,invalid-index" ||
    user32Policy.setWindowLongAStyleIndex !== -16 ||
    user32Policy.setWindowLongARequestedStyle !== "0x90ca0000" ||
    user32Policy.setWindowLongARetainedStyleMask !== "0x04000000" ||
    user32Policy.setWindowLongAInvalidIndex !== -100 ||
    user32Policy.setWindowLongASuccessLastErrorPolicy !== "preserve" ||
    user32Policy.setWindowLongAMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.setWindowLongAIdempotentUpdatePreservesGeneration !== true ||
    user32Policy.setWindowLongAFailureAtomicity !== true ||
    user32Policy.secondIconHandle !== "0x49010002" ||
    user32Policy.largeAndSmallIconResourcesUseDistinctIdentity !== true ||
    user32Policy.sharedIconMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedResourceSharesIdentity !== true ||
    user32Policy.processExitRetiresIcons !== true ||
    user32Policy.hostIconImported !== false ||
    user32Policy.browserIconImported !== false ||
    user32Policy.systemIconImplemented !== false ||
    user32Policy.stringResourceNameImplemented !== false ||
    user32Policy.pngIconImplemented !== false ||
    user32Policy.logicalCursorAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.loadCursorProfileId !==
      "system-arrow-and-module-shared-cursor-v2" ||
    user32Policy.loadCursorArrowResourceId !== 32_512 ||
    user32Policy.loadCursorTargetWidth !== 32 ||
    user32Policy.loadCursorTargetHeight !== 32 ||
    user32Policy.loadCursorMaximumGroupEntries !== 64 ||
    loadCursorVariants.join(",") !==
      "null-instance-idc-arrow,mapped-module-integer-resource" ||
    user32Policy.loadCursorSuccessLastErrorPolicy !==
      "system-preserve-module-zero" ||
    user32Policy.loadCursorMissingResourceId !== 65_535 ||
    user32Policy.loadCursorMissingResourceLastError !== 1_814 ||
    user32Policy.loadCursorModuleMissingResourceId !== 65_535 ||
    user32Policy.loadCursorModuleMissingResourceLastError !== 1_813 ||
    user32Policy.loadCursorModuleWrongTypeResourceId !== 101 ||
    user32Policy.loadCursorModuleWrongTypeLastError !== 1_813 ||
    user32Policy.cursorHandleNamespace !== "vine.logical-cursor.v1" ||
    user32Policy.firstCursorHandle !== "0x43010001" ||
    user32Policy.secondCursorHandle !== "0x43010002" ||
    user32Policy.sharedCursorMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.repeatedSystemCursorSharesIdentity !== true ||
    user32Policy.repeatedModuleCursorSharesIdentity !== true ||
    user32Policy.processExitRetiresCursors !== true ||
    user32Policy.hostCursorImported !== false ||
    user32Policy.browserCursorImported !== false ||
    user32Policy.moduleCursorImplemented !== true ||
    user32Policy.moduleCursorResourceAuthority !==
      "mapped-module-pe-resource-catalog" ||
    user32Policy.otherPredefinedCursorImplemented !== false ||
    user32Policy.stringCursorNameImplemented !== false ||
    user32Policy.logicalWindowClassAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.registerClassExAProfileId !== "local-ascii-module-class-v1" ||
    user32Policy.registerClassExAStructureBytes !== 48 ||
    user32Policy.registerClassExAMaximumNameBytes !== 64 ||
    registerClassExAAllowedStyles.join(",") !== "40,12320" ||
    registerClassExAVariants.join(",") !==
      "mapped-local-module-ascii-main-and-child" ||
    user32Policy.windowClassAtomNamespace !==
      "vine.logical-window-class-atom.v1" ||
    user32Policy.firstWindowClassAtom !== "0xc000" ||
    user32Policy.windowClassMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.windowClassNameIndex !== "ascii-case-folded-module-and-name" ||
    user32Policy.windowClassAtomIndex !== "logical-atom-to-class-identity" ||
    user32Policy.windowClassReferences !==
      "copy-owned-non-owning-logical-icon-and-cursor-identities" ||
    user32Policy.windowProcedureContract !==
      "same-module-executable-x86-stdcall-four-arguments" ||
    user32Policy.registerClassExASuccessLastErrorPolicy !== "preserve" ||
    user32Policy.registerClassExADuplicateLastError !== 1_410 ||
    user32Policy.registerClassExAInvalidSizeLastError !== 87 ||
    user32Policy.processExitRetiresWindowClasses !== true ||
    user32Policy.hostWindowClassImported !== false ||
    user32Policy.browserWindowClassImported !== false ||
    user32Policy.globalWindowClassImplemented !== false ||
    user32Policy.integerWindowClassNameImplemented !== false ||
    user32Policy.nonAsciiWindowClassNameImplemented !== false ||
    user32Policy.classQueryOrUnregisterApiImplemented !== false ||
    user32Policy.logicalWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.createWindowExAProfileId !== "main-local-window-800x600-v1" ||
    createWindowExAVariants.join(",") !==
      "mapped-local-main-class-top-level-null-relations" ||
    user32Policy.createWindowExARequestedStyle !== "0x80ca0000" ||
    user32Policy.createWindowExAEffectiveStyle !== "0x84ca0000" ||
    user32Policy.windowHandleNamespace !== "vine.logical-window.v1" ||
    user32Policy.firstWindowHandle !== "0x48010001" ||
    user32Policy.windowCreationMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    windowCreationMessages.join(",") !==
      "wm-nccreate,wm-nccalcsize,wm-create,wm-size,wm-move" ||
    user32Policy.windowCreationCallbackReplacement !==
      "same-depth-retire-and-enter-after-preflight" ||
    callbackScratchArena.base !== "0x00501000" ||
    callbackScratchArena.bytes !== 65_536 ||
    callbackScratchArena.slotBytes !== 4_096 ||
    user32Policy.callbackScratchMutation !==
      "address-memory-stack-and-frame-preflight-transaction" ||
    defWindowProcAImplementedMessages.join(",") !==
      "129,131,5,3,128,71,70,28,134,6,641,642,7" ||
    user32Policy.defWindowProcALparamContract !==
      "bounded-message-dependent-scalar-or-pointer-union" ||
    user32Policy.systemMenuProfileId !==
      "live-window-system-menu-close-item-enabled-v1" ||
    user32Policy.logicalMenuAuthority !== "runtime/user32.rs::RuntimeUser32" ||
    user32Policy.menuHandleNamespace !== "vine.logical-menu.v1" ||
    user32Policy.firstMenuHandle !== "0x4e010001" ||
    systemMenuVariants.join(",") !==
      "null-window-failure,created-window-false-revert,logical-close-item-enable" ||
    user32Policy.systemMenuRevert !== 0 ||
    user32Policy.systemMenuCloseItemCommand !== "0x0000f060" ||
    user32Policy.systemMenuCloseItemFlags !== 0 ||
    user32Policy.systemMenuCloseItemInitialEnabled !== true ||
    user32Policy.systemMenuWindowRelation !==
      "one-system-menu-per-created-window" ||
    user32Policy.systemMenuMutation !==
      "separate-menu-generation-plan-stale-check-targeted-commit" ||
    user32Policy.systemMenuIdempotentEnablePreservesGeneration !== true ||
    user32Policy.getSystemMenuSuccessLastErrorPolicy !== "preserve" ||
    getSystemMenuFailure.result !== "0x00000000" ||
    getSystemMenuFailure.lastError !== 1_400 ||
    user32Policy.enableMenuItemSuccessLastErrorPolicy !== "preserve" ||
    enableMenuItemFailure.item !== "0x0000f060" ||
    enableMenuItemFailure.flags !== 0 ||
    enableMenuItemFailure.result !== "0xffffffff" ||
    enableMenuItemFailure.lastError !== 1_401 ||
    user32Policy.processExitRetiresSystemMenus !== true ||
    user32Policy.hostMenuImported !== false ||
    user32Policy.browserMenuImported !== false ||
    user32Policy.boundedSystemMenuIdentityImplemented !== true ||
    user32Policy.processExitRetiresWindows !== true ||
    user32Policy.hostWindowImported !== false ||
    user32Policy.browserWindowImported !== false ||
    user32Policy.generalMenuIdentityImplemented !== false ||
    user32Policy.windowVisibilityImplemented !== true ||
    user32Policy.windowEffectiveStyleMutationImplemented !== true ||
    user32Policy.windowGeometryMutationImplemented !== true ||
    user32Policy.windowTopmostStateImplemented !== true ||
    user32Policy.windowActivationRelationsImplemented !== true ||
    user32Policy.generalWindowActivationImplemented !== false ||
    user32Policy.multiWindowActivationImplemented !== false ||
    user32Policy.boundedPrimaryWindowResizeImplemented !== true ||
    user32Policy.generalWindowResizeImplemented !== false ||
    user32Policy.generalWindowZOrderImplemented !== false ||
    user32Policy.setTimerProfileId !== "main-window-id-1-20ms-timerproc-v1" ||
    user32Policy.setTimerVariants?.join(",") !==
      "live-owner-thread-window-id-1-20ms-executable-timerproc" ||
    user32Policy.setTimerMessage !== 275 ||
    user32Policy.setTimerTimerId !== 1 ||
    user32Policy.setTimerRequestedIntervalMilliseconds !== 20 ||
    user32Policy.setTimerMinimumIntervalMilliseconds !== 10 ||
    user32Policy.setTimerSuccessLastErrorPolicy !== "preserve" ||
    user32Policy.setTimerInvalidWindowLastError !== 1_400 ||
    user32Policy.logicalWindowTimerAuthority !==
      "runtime/timers.rs::RuntimeTimers" ||
    user32Policy.logicalWindowTimerNamespace !==
      "vine.logical-window-timer.v1" ||
    user32Policy.logicalWindowTimerLimit !== 4_096 ||
    user32Policy.logicalWindowTimerMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    user32Policy.logicalWindowTimerReplacement !==
      "same-window-and-id-preserves-internal-identity-and-replaces-registration" ||
    user32Policy.logicalWindowTimerProcessExitTeardown !== true ||
    user32Policy.logicalWindowTimerCallbackDeliveryImplemented !== false ||
    user32Policy.logicalWindowTimerBrowserClockImported !== false ||
    user32Policy.setTimerFailureAtomicity !== true ||
    user32Policy.windowMessageQueueIntegrated !== false ||
    user32Policy.logicalWindowIconMutationImplemented !== true ||
    user32Policy.browserWindowIconMutationImplemented !== false ||
    user32Policy.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-small-icon-16x16-v3" ||
    getSystemMetricsIndices.join(",") !== "0,1,4,7,8,49,50" ||
    user32Policy.getSystemMetricsCaptionHeight !== 23 ||
    user32Policy.getSystemMetricsFixedFrameWidth !== 3 ||
    user32Policy.getSystemMetricsFixedFrameHeight !== 3 ||
    user32Policy.getSystemMetricsSmallIconWidth !== 16 ||
    user32Policy.getSystemMetricsSmallIconHeight !== 16 ||
    user32Policy.getSystemMetricsScreenDimensionsSource !==
      "runtime-displays-primary-monitor-rect" ||
    user32Policy.getSystemMetricsLastErrorPolicy !== "preserve" ||
    user32Policy.getSystemMetricsGuestWrites !== 0 ||
    user32Policy.hostSystemMetricsImported !== false ||
    user32Policy.browserSystemMetricsImported !== false ||
    user32Policy.unreachedSystemMetricIndicesImplemented !== false ||
    user32Policy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active User32 compatibility policy boundary is invalid");
  }
  const winmmPolicy = requireRecord(
    active.winmmCompatibilityPolicy,
    "activeRuntimeCheckpoint.winmmCompatibilityPolicy",
    errors,
  );
  if (
    winmmPolicy.schema !== "vine.winmm-compatibility-policy" ||
    winmmPolicy.schemaVersion !== 1 ||
    winmmPolicy.id !== "winmm-timecaps-1-1000000-period-1-tick-1-v1" ||
    winmmPolicy.source !==
      "crates/vine-core/src/winmm.rs::WinmmCompatibilityPolicy::controlled_v1" ||
    winmmPolicy.handlerDispatch !==
      "single-semantic-implementation-id-with-policy" ||
    winmmPolicy.timeCapsProfileId !== "timecaps-1-1000000-v1" ||
    winmmPolicy.periodMinimumMilliseconds !== 1 ||
    winmmPolicy.periodMaximumMilliseconds !== 1_000_000 ||
    winmmPolicy.x86TimeCapsBytes !== 8 ||
    winmmPolicy.shortSizeResult !== 97 ||
    winmmPolicy.shortSizeMutation !== "none" ||
    winmmPolicy.acceptedSizeRange !== "eight-or-larger" ||
    winmmPolicy.successfulWriteBytes !== 8 ||
    winmmPolicy.timerResolutionProfileId !== "timer-resolution-period-1-v1" ||
    !Array.isArray(winmmPolicy.allowedPeriodsMilliseconds) ||
    winmmPolicy.allowedPeriodsMilliseconds.join(",") !== "1" ||
    winmmPolicy.timerResolutionStateOwner !==
      "runtime/timers.rs::RuntimeTimers" ||
    winmmPolicy.timerResolutionMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    winmmPolicy.duplicateBeginRequiresMatchingEnds !== true ||
    winmmPolicy.unmatchedEndResult !== 97 ||
    winmmPolicy.processExitRetiresRequests !== true ||
    winmmPolicy.timeGetTimeProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    winmmPolicy.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    winmmPolicy.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    winmmPolicy.timeGetTimeIncrementPerRead !== 1 ||
    winmmPolicy.timerResolutionAffectsSampledClock !== false ||
    winmmPolicy.lastErrorPolicy !== "preserve" ||
    winmmPolicy.hostTimerCapabilitiesImported !== false ||
    winmmPolicy.browserTimerCapabilitiesImported !== false ||
    winmmPolicy.hostTimerResolutionImported !== false ||
    winmmPolicy.browserTimerResolutionImported !== false ||
    winmmPolicy.hostClockImported !== false ||
    winmmPolicy.browserClockImported !== false ||
    winmmPolicy.clockAdvancedByQuery !== false ||
    winmmPolicy.unsupportedBeforeMutation !== true
  ) {
    errors.push("active WinMM compatibility policy boundary is invalid");
  }
  const latestEvidence = requireRecord(
    active.latestEvidence,
    "activeRuntimeCheckpoint.latestEvidence",
    errors,
  );
  if (
    latestEvidence.implementedApiContractCount !==
    active.genericDispatcherContractCount
  ) {
    errors.push(
      "latest active evidence API count does not match the active summary",
    );
  }
  for (const field of [
    "nativeTimeGetTimePrimaryTraceSha256",
    "nativeTimeGetTimePrimarySummarySha256",
    "nativeTimeGetTimeConfirmationTraceSha256",
    "nativeTimeGetTimeConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.genericSupportedCallCount !==
    active.genericDispatcherCallOccurrenceCount
  ) {
    errors.push(
      "latest active evidence call count does not match the active summary",
    );
  }
  if (
    latestEvidence.getDriveTypeAImplemented !== true ||
    latestEvidence.getDriveTypeACallCount !== 26 ||
    latestEvidence.getDriveTypeAStateOwner !==
      "runtime/process.rs::RuntimeProcessDirectories" ||
    latestEvidence.getDriveTypeAHostDriveTopologyImported !== false ||
    latestEvidence.getDriveTypeABrowserDriveTopologyImported !== false ||
    latestEvidence.vfsBackendImplemented !== true
  ) {
    errors.push("latest drive-type authority evidence is invalid");
  }
  requireSha256(
    latestEvidence.sourceFileReadPayloadSha256,
    "activeRuntimeCheckpoint.latestEvidence.sourceFileReadPayloadSha256",
    errors,
  );
  requireInteger(
    latestEvidence.sourceFileReadRequestedBytes,
    "activeRuntimeCheckpoint.latestEvidence.sourceFileReadRequestedBytes",
    errors,
    1,
  );
  requireInteger(
    latestEvidence.writableProfileObjectGeneration,
    "activeRuntimeCheckpoint.latestEvidence.writableProfileObjectGeneration",
    errors,
    1,
  );
  if (
    latestEvidence.sourceFileCreateImplemented !== true ||
    latestEvidence.sourceFileGetSizeImplemented !== true ||
    latestEvidence.sourceFileReadImplemented !== true ||
    latestEvidence.sourceFileSetPointerImplemented !== true ||
    latestEvidence.sourceFileCloseImplemented !== true ||
    latestEvidence.sourceFileCreateCallCount !== 2 ||
    latestEvidence.sourceFileGetSizeCallCount !== 2 ||
    latestEvidence.sourceFileReadCallCount !== 2 ||
    latestEvidence.sourceFileSetPointerCallCount !== 0 ||
    latestEvidence.sourceFileCloseCallCount !== 2 ||
    latestEvidence.sourceFileReadSequences?.join(",") !== "1362,1378" ||
    latestEvidence.sourceFileReadTransferredBytes !==
      latestEvidence.sourceFileReadRequestedBytes ||
    latestEvidence.sourceFileReadOffsets?.join(",") !== "0,0" ||
    latestEvidence.sourceFileReadRequestIds?.join(",") !== "1,2" ||
    latestEvidence.sourceFileReadObjectId !== 1 ||
    latestEvidence.sourceFileReadSourceId !== 0 ||
    latestEvidence.sourceFileMappingOperationCount !== 43 ||
    latestEvidence.sourceFileHostDirtyPageCount !== 958 ||
    latestEvidence.sourceFileHeapPageMaterializationCount !== 31 ||
    latestEvidence.sourceFileCallbackEventCount !== 2 ||
    latestEvidence.sourceFileReadFailureAtomicity !== true ||
    latestEvidence.sourceFileReadPendingCancellation !== true ||
    latestEvidence.sourceFileReadProcessExitTeardown !== true ||
    latestEvidence.sourceFileReadBrowserOwnsWindowsState !== false ||
    latestEvidence.writableProfileId !== "default" ||
    latestEvidence.writableProfileStoragePathTemplate !==
      ".work/private-profiles/<title-fingerprint>/default.sqlite" ||
    latestEvidence.writableProfileWriteImplemented !== true ||
    latestEvidence.writableProfileDurableCommitImplemented !== true ||
    latestEvidence.writableProfileWriteCallCount !== 0 ||
    latestEvidence.writableProfileCommitCallCount !== 0 ||
    latestEvidence.writableProfileStagedStateAuthority !== "rust-runtime-vfs" ||
    latestEvidence.writableProfileDurableStateAuthority !==
      "same-origin-sqlite" ||
    latestEvidence.writableProfileDistributionFileUnchangedAcrossRun !== true ||
    latestEvidence.writableProfileDatabaseGitCommitted !== false
  ) {
    errors.push("latest writable-profile file evidence is invalid");
  }
  if (
    latestEvidence.swapMouseButtonImplemented !== true ||
    latestEvidence.swapMouseButtonCallCount !== 2 ||
    latestEvidence.swapMouseButtonStateOwner !==
      "runtime/input.rs::RuntimeInput" ||
    latestEvidence.swapMouseButtonFinalSwapped !== false ||
    latestEvidence.swapMouseButtonFinalGeneration !== 0 ||
    latestEvidence.swapMouseButtonHostStateImported !== false ||
    latestEvidence.swapMouseButtonBrowserStateImported !== false
  ) {
    errors.push("latest mouse-button mapping authority evidence is invalid");
  }
  for (const field of [
    "nativeTimeGetDevCapsPrimaryTraceSha256",
    "nativeTimeGetDevCapsConfirmationTraceSha256",
    "nativeTimePeriodPrimaryTraceSha256",
    "nativeTimePeriodConfirmationTraceSha256",
    "nativeGetVersionExAPrimaryTraceSha256",
    "nativeGetVersionExAConfirmationTraceSha256",
    "nativeCreateWaitableTimerAPrimaryTraceSha256",
    "nativeCreateWaitableTimerAConfirmationTraceSha256",
    "nativeSetWaitableTimerPrimaryTraceSha256",
    "nativeSetWaitableTimerConfirmationTraceSha256",
    "nativeLoadIconAPrimaryTraceSha256",
    "nativeLoadIconAConfirmationTraceSha256",
    "nativeLoadCursorAPrimaryTraceSha256",
    "nativeLoadCursorAConfirmationTraceSha256",
    "nativeRegisterClassExAPrimaryTraceSha256",
    "nativeRegisterClassExAConfirmationTraceSha256",
    "nativeGetSystemMetricsPrimaryTraceSha256",
    "nativeGetSystemMetricsConfirmationTraceSha256",
    "nativeGetSystemMetricsSmallIconPrimaryTraceSha256",
    "nativeGetSystemMetricsSmallIconConfirmationTraceSha256",
    "nativeLoadImageAPrimaryTraceSha256",
    "nativeLoadImageAConfirmationTraceSha256",
    "nativeSendMessageAPrimaryTraceSha256",
    "nativeSendMessageAConfirmationTraceSha256",
    "nativeSetWindowPosPrimaryTraceSha256",
    "nativeSetWindowPosConfirmationTraceSha256",
    "nativeSetWindowPosActivationPrimaryTraceSha256",
    "nativeSetWindowPosActivationConfirmationTraceSha256",
    "nativeSetWindowPosResizePrimaryTraceSha256",
    "nativeSetWindowPosResizePrimarySummarySha256",
    "nativeSetWindowPosResizeConfirmationTraceSha256",
    "nativeSetWindowPosResizeConfirmationSummarySha256",
    "nativeMonitorFromWindowOracleSourceSha256",
    "nativeMonitorFromWindowOracleExecutableSha256",
    "nativeD3d9GetAdapterCountTraceSha256",
    "nativeD3d9GetAdapterCountSummarySha256",
    "nativeD3d9GetAdapterMonitorTraceSha256",
    "nativeD3d9GetAdapterMonitorSummarySha256",
    "nativeD3d9GetAdapterMonitorOracleSourceSha256",
    "nativeD3d9GetAdapterMonitorOracleExecutableSha256",
    "nativeLargeHeapPrimaryTraceSha256",
    "nativeLargeHeapConfirmationTraceSha256",
    "nativeCreateWindowExAPrimaryTraceSha256",
    "nativeCreateWindowExAConfirmationTraceSha256",
    "nativeCreateWindowExAPayloadTraceSha256",
    "nativeInvalidMenuTraceSha256",
    "nativeDefWindowProcTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeLargeHeapPrimaryTraceBytes !== 419_374 ||
    latestEvidence.nativeLargeHeapPrimaryTraceLines !== 204 ||
    latestEvidence.nativeLargeHeapPrimaryBalancedApiPairs !== 75 ||
    latestEvidence.nativeLargeHeapConfirmationTraceBytes !== 419_375 ||
    latestEvidence.nativeLargeHeapConfirmationTraceLines !== 204 ||
    latestEvidence.nativeLargeHeapConfirmationBalancedApiPairs !== 75 ||
    latestEvidence.nativeLargeHeapTraceDiagnostics !== 0 ||
    latestEvidence.nativeLargeHeapProfile !==
      "main-module-large-heap-allocation-lifetime-v1" ||
    latestEvidence.nativeLargeHeapFlags !== 0 ||
    latestEvidence.nativeLargeHeapRequestedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.nativeLargeHeapRequestedPages !== 16_384 ||
    latestEvidence.nativeLargeHeapAccessedPages !== 30 ||
    latestEvidence.nativeLargeHeapUntouchedPages !== 16_354 ||
    latestEvidence.nativeLargeHeapAccessedPageRun?.join(",") !== "0,29" ||
    latestEvidence.nativeLargeHeapAccessedExtentBytes !== 122_880 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.read !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.write !== 30 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.execute !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessOperations?.other !== 0 ||
    latestEvidence.nativeLargeHeapFirstAccessRva !== "0x00061aef" ||
    latestEvidence.nativeLargeHeapLastNewPageAccessRva !== "0x000a3f37" ||
    latestEvidence.nativeLargeHeapLastNewPageOffset !== 122_848 ||
    latestEvidence.nativeLargeHeapFreeResult !== 1 ||
    latestEvidence.nativeLargeHeapLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLargeHeapPointerClassification !==
      "opaque-native-artifact-identity" ||
    latestEvidence.nativeLargeHeapExecutableHashUnchanged !== true
  ) {
    errors.push("latest native large-heap lifetime evidence is invalid");
  }
  if (
    latestEvidence.largeHeapAllocImplemented !== true ||
    latestEvidence.largeHeapAllocSequence !== 1_350 ||
    latestEvidence.largeHeapAllocOccurrence !== 12 ||
    latestEvidence.largeHeapAllocInstructionCount !== 50_105 ||
    latestEvidence.largeHeapAllocArguments?.join(",") !==
      "0x68010001,0x00000000,0x04000000" ||
    latestEvidence.largeHeapAllocReturnAddress !== "0x100a4a33" ||
    latestEvidence.largeHeapAllocResult !== "0x40000000" ||
    latestEvidence.largeHeapAllocLastError !== 0 ||
    latestEvidence.largeHeapAllocGuestWrites !== 0 ||
    latestEvidence.largeHeapAllocImplementationId !==
      "kernel32.heap-alloc.v2" ||
    latestEvidence.largeHeapAllocAllocationId !== 12 ||
    latestEvidence.largeHeapAllocRequestedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.largeHeapAllocReservedBytes !== 64 * 1024 * 1024 ||
    latestEvidence.largeHeapAllocInitialization !==
      "deterministic-uninitialized" ||
    latestEvidence.largeHeapAllocLiveAllocations !== 10 ||
    latestEvidence.largeHeapReserveFromGeneration !== 1_469 ||
    latestEvidence.largeHeapReserveGeneration !== 1_470 ||
    latestEvidence.largeHeapReserveMappingOperations !== 1 ||
    latestEvidence.largeHeapReserveDirtyPages !== 0 ||
    latestEvidence.largeHeapReservePhysicalPageDelta !== 0 ||
    latestEvidence.largeHeapMaterializationInstructionCount !== 50_123 ||
    latestEvidence.largeHeapMaterializationFaultAddress !== "0x40000000" ||
    latestEvidence.largeHeapMaterializationPageFaultErrorCode !== 6 ||
    latestEvidence.largeHeapMaterializationMemoryOwner !== 11 ||
    latestEvidence.largeHeapMaterializationAddressAllocationId !== 37 ||
    latestEvidence.largeHeapMaterializationCommittedPage !== "0x40000000" ||
    latestEvidence.largeHeapMaterializationCommittedBytes !== 4_096 ||
    latestEvidence.largeHeapMaterializationFromGeneration !== 1_470 ||
    latestEvidence.largeHeapMaterializationGeneration !== 1_472 ||
    latestEvidence.largeHeapMaterializationPayloadSha256 !==
      "f600eca824e84a43f0691b267bd620e462c50da165c5b80e17aecb7a924f1fa8" ||
    latestEvidence.largeHeapMaterializationFillByte !== 0xa5 ||
    latestEvidence.largeHeapMaterializedPages !== 1 ||
    latestEvidence.largeHeapReservedPagesBefore !== 4_784 ||
    latestEvidence.largeHeapReservedPagesAfter !== 21_168 ||
    latestEvidence.largeHeapPhysicalPagesBefore !== 395 ||
    latestEvidence.largeHeapPhysicalPagesAfter !== 396 ||
    latestEvidence.largeHeapSecondLiveRejectedBeforeMutation !== true ||
    latestEvidence.largeHeapDuplicateStaleOutsideProtectionQuotaRejectedBeforeMutation !==
      true ||
    latestEvidence.largeHeapAddressRecordPermanentAcrossFree !== true
  ) {
    errors.push(
      "latest large-heap allocation and materialization evidence is invalid",
    );
  }
  if (
    latestEvidence.nativeTimeGetDevCapsTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimeGetDevCapsOracleLastErrorsPreserved !== true ||
    latestEvidence.nativeTimeGetDevCapsOracleWritesBounded !== true ||
    latestEvidence.nativeTimePeriodTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimePeriodOracleProfile !==
      "x86-time-period-range-and-pairing-v1" ||
    latestEvidence.nativeTimePeriodOracleCallCount !== 17 ||
    latestEvidence.nativeTimePeriodOracleAllPreserveLastError !== true ||
    latestEvidence.nativeTimePeriodUnmatchedEndResult !== 97 ||
    latestEvidence.nativeTimePeriodZeroBeginResult !== 97 ||
    latestEvidence.nativeTimePeriodDuplicatePairingExact !== true ||
    latestEvidence.nativeTimeBeginPeriodReached !== true ||
    latestEvidence.timeGetDevCapsImplemented !== true ||
    latestEvidence.timeGetDevCapsSequence !== 1334 ||
    latestEvidence.timeGetDevCapsInstructionCount !== 49731 ||
    latestEvidence.timeGetDevCapsResult !== 0 ||
    latestEvidence.timeGetDevCapsLastError !== 0 ||
    latestEvidence.timeGetDevCapsGuestWriteBytes !== 8 ||
    latestEvidence.timeGetDevCapsOutputHex !== "0100000040420f00" ||
    latestEvidence.timeGetDevCapsPeriodMinimumMilliseconds !== 1 ||
    latestEvidence.timeGetDevCapsPeriodMaximumMilliseconds !== 1_000_000 ||
    latestEvidence.timeGetDevCapsPolicyOwner !==
      "WinmmCompatibilityPolicy.time_get_dev_caps" ||
    latestEvidence.timeGetDevCapsHostCapabilitiesImported !== false ||
    latestEvidence.timeGetDevCapsBrowserCapabilitiesImported !== false ||
    latestEvidence.timeGetDevCapsAdvancesClock !== false ||
    latestEvidence.timeBeginPeriodImplemented !== true ||
    latestEvidence.timeBeginPeriodSequence !== 1335 ||
    latestEvidence.timeBeginPeriodInstructionCount !== 49737 ||
    latestEvidence.timeBeginPeriodArgument !== 1 ||
    latestEvidence.timeBeginPeriodResult !== 0 ||
    latestEvidence.timeBeginPeriodLastError !== 0 ||
    latestEvidence.timeBeginPeriodGuestWrites !== 0 ||
    latestEvidence.timeBeginPeriodStateOwner !==
      "runtime/timers.rs::RuntimeTimers" ||
    latestEvidence.timeBeginPeriodRequestCount !== 1 ||
    latestEvidence.timeBeginPeriodTotalRequestCount !== 1 ||
    latestEvidence.timeBeginPeriodEffectivePeriodMilliseconds !== 1 ||
    latestEvidence.timeBeginPeriodTimerGeneration !== 1 ||
    latestEvidence.timeBeginPeriodHostTimerResolutionImported !== false ||
    latestEvidence.timeBeginPeriodBrowserTimerResolutionImported !== false ||
    latestEvidence.timeBeginPeriodAdvancesClock !== false ||
    latestEvidence.timeEndPeriodImplemented !== true ||
    latestEvidence.timeEndPeriodReached !== false ||
    latestEvidence.timerResolutionProcessExitTeardown !== true ||
    latestEvidence.nativeTimeGetTimePrimaryTraceBytes !== 304_054 ||
    latestEvidence.nativeTimeGetTimePrimaryTraceLines !== 854 ||
    latestEvidence.nativeTimeGetTimePrimaryTraceSha256 !==
      "8560ea8389627cfc2c85b4a98f30f835ab53ff3ec13038ac55b25ad736790a29" ||
    latestEvidence.nativeTimeGetTimePrimaryBalancedApiPairs !== 390 ||
    latestEvidence.nativeTimeGetTimePrimarySummaryBytes !== 51_478 ||
    latestEvidence.nativeTimeGetTimePrimarySummarySha256 !==
      "5fa32c69a605e1cd173eed042522669f253534a1d993d943ee239b12d16fc777" ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceBytes !== 303_051 ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceLines !== 851 ||
    latestEvidence.nativeTimeGetTimeConfirmationTraceSha256 !==
      "8f3a8847793282b4eadb4f4e45c6a142610b9f3ea1f3e4437e0b8349b314cc5e" ||
    latestEvidence.nativeTimeGetTimeConfirmationBalancedApiPairs !== 388 ||
    latestEvidence.nativeTimeGetTimeConfirmationSummaryBytes !== 51_652 ||
    latestEvidence.nativeTimeGetTimeConfirmationSummarySha256 !==
      "83d7a4ca65a6b109379b796838cc1d97718083a28657bc70d395d636ef9604c3" ||
    latestEvidence.nativeTimeGetTimeTraceDiagnostics !== 0 ||
    latestEvidence.nativeTimeGetTimeProfile !== "x86-bgi-time-get-time-v1" ||
    latestEvidence.nativeTimeGetTimeOracleProfile !==
      "x86-time-get-time-shared-tick-epoch-v1" ||
    latestEvidence.nativeTimeGetTimeOracleSentinels?.join(",") !==
      "0,87,305419896" ||
    latestEvidence.nativeTimeGetTimeOracleCallsPerTrace !== 3 ||
    latestEvidence.nativeTimeGetTimeOracleResultsMatchHooks !== true ||
    latestEvidence.nativeTimeGetTimeOraclePreservesLastError !== true ||
    latestEvidence.nativeTimeGetTimeOracleResultsWithinGetTickCountBrackets !==
      true ||
    latestEvidence.nativeTimeGetTimeMainCallsPerTrace !== 1 ||
    latestEvidence.nativeTimeGetTimeCallerRva !== "0x00091ebf" ||
    latestEvidence.nativeTimeGetTimeLastError !== 0 ||
    latestEvidence.nativeTimeGetTimeConsumerProfile !==
      "wrap-aware-eax-delta-v1" ||
    latestEvidence.nativeTimeGetTimeConsumerFrameCallerRva !== "0x00083144" ||
    latestEvidence.nativeTimeGetTimeExecutableHashUnchanged !== true ||
    latestEvidence.timeGetTimeImplemented !== true ||
    latestEvidence.timeGetTimeReached !== false ||
    latestEvidence.timeGetTimeImplementationId !== "winmm.time-get-time.v1" ||
    latestEvidence.timeGetTimePolicyProfileId !==
      "shared-process-tick-count-increment-1-v1" ||
    latestEvidence.timeGetTimeClockAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    latestEvidence.timeGetTimeClockRelation !==
      "shared-with-kernel32-get-tick-count" ||
    latestEvidence.timeGetTimeIncrementPerRead !== 1 ||
    latestEvidence.timeGetTimeTimerResolutionAffectsClock !== false ||
    latestEvidence.timeGetTimeHostClockImported !== false ||
    latestEvidence.timeGetTimeBrowserClockImported !== false ||
    latestEvidence.timeGetTimeFailureAtomicity !== true ||
    latestEvidence.timeGetTimeNodeCompletions !== 0 ||
    latestEvidence.timeGetTimeChromeCompletions !== 0 ||
    latestEvidence.timeGetTimeNodeChromeReachabilityMatched !== true ||
    latestEvidence.tickCountNextValue !== 3_601_004
  ) {
    errors.push("latest WinMM timer and shared-clock evidence is invalid");
  }
  if (
    latestEvidence.nativeGetVersionExATraceDiagnostics !== 0 ||
    latestEvidence.nativeGetVersionExAOracleProfile !==
      "x86-get-version-ex-a-size-and-output-v1" ||
    latestEvidence.nativeGetVersionExABaseStructureBytes !== 148 ||
    latestEvidence.nativeGetVersionExAExtendedStructureBytes !== 156 ||
    latestEvidence.nativeGetVersionExAInvalidSizeResult !== 0 ||
    latestEvidence.nativeGetVersionExAInvalidSizeLastError !== 122 ||
    latestEvidence.nativeGetVersionExAInvalidSizeMutation !== false ||
    latestEvidence.nativeGetVersionExASuccessPreservesLastError !== true ||
    latestEvidence.nativeGetVersionExABgiDeclaredSize !== 148 ||
    latestEvidence.nativeGetVersionExABgiResult !== 1 ||
    latestEvidence.nativeGetVersionExABgiConsumer !==
      "compare-platform-id-with-ver-platform-win32-nt" ||
    latestEvidence.getVersionExAImplemented !== true ||
    latestEvidence.getVersionExASequence !== 1337 ||
    latestEvidence.getVersionExAInstructionCount !== 49890 ||
    latestEvidence.getVersionExAResult !== 1 ||
    latestEvidence.getVersionExALastError !== 0 ||
    latestEvidence.getVersionExAGuestWriteBytes !== 148 ||
    latestEvidence.getVersionExAPlatformId !== 2 ||
    latestEvidence.getVersionExAHostVersionImported !== false ||
    latestEvidence.getVersionExABrowserVersionImported !== false
  ) {
    errors.push("latest ANSI OS-version evidence is invalid");
  }
  if (
    latestEvidence.schema !== profileExpectations.latestSchema ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryTraceBytes !== 256_897 ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryTraceLines !== 779 ||
    latestEvidence.nativeCreateWaitableTimerAPrimaryBalancedApiPairs !== 354 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationTraceBytes !==
      255_564 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationTraceLines !== 777 ||
    latestEvidence.nativeCreateWaitableTimerAConfirmationBalancedApiPairs !==
      353 ||
    latestEvidence.nativeCreateWaitableTimerATraceDiagnostics !== 0 ||
    latestEvidence.nativeCreateWaitableTimerAOracleProfile !==
      "x86-create-waitable-timer-initial-state-v1" ||
    latestEvidence.nativeCreateWaitableTimerAOracleManualAndAutoCreated !==
      true ||
    latestEvidence.nativeCreateWaitableTimerAOracleSuccessLastError !== 0 ||
    latestEvidence.nativeCreateWaitableTimerAOracleInitialWaitResult !== 258 ||
    latestEvidence.nativeCreateWaitableTimerAOracleAllInitiallyNonsignaled !==
      true ||
    latestEvidence.nativeCreateWaitableTimerAOracleAllClosed !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiCallerReturnRva !==
      "0x0005053b" ||
    latestEvidence.nativeCreateWaitableTimerABgiSecurityAttributes !==
      "0x00000000" ||
    latestEvidence.nativeCreateWaitableTimerABgiManualReset !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiNameAddress !== "0x00000000" ||
    latestEvidence.nativeCreateWaitableTimerABgiResultNonNull !== true ||
    latestEvidence.nativeCreateWaitableTimerABgiLastError !== 0 ||
    latestEvidence.nativeCreateWaitableTimerABgiConsumer !==
      "store-eax-directly-at-object-offset-4" ||
    latestEvidence.createWaitableTimerAImplemented !== true ||
    latestEvidence.createWaitableTimerASequence !== 1338 ||
    latestEvidence.createWaitableTimerAInstructionCount !== 49_897 ||
    latestEvidence.createWaitableTimerAArguments?.join(",") !==
      "0x00000000,0x00000001,0x00000000" ||
    latestEvidence.createWaitableTimerAReturnAddress !== "0x1005053b" ||
    latestEvidence.createWaitableTimerAResult !== "0x57010001" ||
    latestEvidence.createWaitableTimerALastError !== 0 ||
    latestEvidence.createWaitableTimerAGuestWrites !== 0 ||
    latestEvidence.createWaitableTimerAStateOwner !==
      "runtime/timers.rs::RuntimeTimers" ||
    latestEvidence.createWaitableTimerATimerId !== 1 ||
    latestEvidence.createWaitableTimerAManualReset !== true ||
    latestEvidence.createWaitableTimerAObjectTableId !== 20 ||
    latestEvidence.createWaitableTimerAObjectNamespace !==
      "vine.runtime-waitable-timer-object.v1" ||
    latestEvidence.createWaitableTimerAInitialWaitState !== "nonsignaled" ||
    latestEvidence.createWaitableTimerAOpenHandles !== 1 ||
    latestEvidence.createWaitableTimerALastCloseRetiresPayloadAndObject !==
      true ||
    latestEvidence.createWaitableTimerAProcessExitRetiresPayloadAndObject !==
      true ||
    latestEvidence.createWaitableTimerAHostTimerImported !== false ||
    latestEvidence.createWaitableTimerABrowserTimerImported !== false ||
    latestEvidence.nativeSetWaitableTimerPrimaryTraceBytes !== 1_615_252 ||
    latestEvidence.nativeSetWaitableTimerPrimaryTraceLines !== 5_015 ||
    latestEvidence.nativeSetWaitableTimerPrimaryTraceSha256 !==
      "aa172689c3033568807fb57635248f9dfb9b3f0a0bd33797e214fad947162174" ||
    latestEvidence.nativeSetWaitableTimerPrimaryBalancedApiPairs !== 2_469 ||
    latestEvidence.nativeSetWaitableTimerConfirmationTraceBytes !== 1_615_361 ||
    latestEvidence.nativeSetWaitableTimerConfirmationTraceLines !== 5_015 ||
    latestEvidence.nativeSetWaitableTimerConfirmationTraceSha256 !==
      "9342206813c8a06d09a49934d7e2ff33fb1ada00885b0db4436021a76ded5d47" ||
    latestEvidence.nativeSetWaitableTimerConfirmationBalancedApiPairs !==
      2_469 ||
    latestEvidence.nativeSetWaitableTimerTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWaitableTimerBgiCallerReturnRva !== "0x00050607" ||
    latestEvidence.nativeSetWaitableTimerBgiDueTime100ns !== -5_000 ||
    latestEvidence.nativeSetWaitableTimerBgiPeriodMilliseconds !== 0 ||
    latestEvidence.nativeSetWaitableTimerBgiCompletionRoutineAddress !==
      "0x00000000" ||
    latestEvidence.nativeSetWaitableTimerBgiCompletionContextAddress !==
      "0x00000000" ||
    latestEvidence.nativeSetWaitableTimerBgiResume !== false ||
    latestEvidence.nativeSetWaitableTimerBgiResult !== 1 ||
    latestEvidence.nativeSetWaitableTimerBgiLastError !== 0 ||
    latestEvidence.nativeSetWaitableTimerBgiConsumer !==
      "wait-for-single-object-ex-timeout-10-alertable-true" ||
    latestEvidence.nativeSetWaitableTimerOracleProfile !==
      "x86-set-waitable-timer-relative-one-shot-v1" ||
    latestEvidence.nativeSetWaitableTimerOracleRelativeDueTime100ns !==
      10_000 ||
    latestEvidence.nativeSetWaitableTimerOracleManualWaitResults?.join(",") !==
      "0,0" ||
    latestEvidence.nativeSetWaitableTimerOracleAutoWaitResults?.join(",") !==
      "0,258" ||
    latestEvidence.nativeSetWaitableTimerOracleInvalidHandleResult !== 0 ||
    latestEvidence.nativeSetWaitableTimerOracleInvalidHandleLastError !== 6 ||
    latestEvidence.nativeSetWaitableTimerOracleAllHookSystemResultsMatched !==
      true ||
    latestEvidence.setWaitableTimerImplemented !== true ||
    latestEvidence.setWaitableTimerSequence !== 18_681 ||
    latestEvidence.setWaitableTimerInstructionCount !== 11_973_553 ||
    latestEvidence.setWaitableTimerArguments?.join(",") !==
      "0x57010002,0x500fff98,0x00000000,0x00000000,0x00000000,0x00000000" ||
    latestEvidence.setWaitableTimerReturnAddress !== "0x10050607" ||
    latestEvidence.setWaitableTimerResult !== 1 ||
    latestEvidence.setWaitableTimerLastError !== 0 ||
    latestEvidence.setWaitableTimerGuestWrites !== 0 ||
    latestEvidence.setWaitableTimerImplementationId !==
      "kernel32.set-waitable-timer.relative-one-shot.v1" ||
    latestEvidence.setWaitableTimerStateAuthority !==
      "runtime-timers-and-typed-object-table" ||
    latestEvidence.setWaitableTimerHandle !== "0x57010002" ||
    latestEvidence.setWaitableTimerTimerId !== 2 ||
    latestEvidence.setWaitableTimerManualReset !== true ||
    latestEvidence.setWaitableTimerRelativeDueTime100ns !== 5_000 ||
    latestEvidence.setWaitableTimerPeriodMilliseconds !== 0 ||
    latestEvidence.setWaitableTimerArmGeneration !== 1 ||
    latestEvidence.setWaitableTimerObjectTableId !== 24 ||
    latestEvidence.setWaitableTimerObjectNamespace !==
      "vine.runtime-waitable-timer-object.v1" ||
    latestEvidence.setWaitableTimerWaitState !== "nonsignaled" ||
    latestEvidence.setWaitableTimerOpenHandles !== 1 ||
    latestEvidence.setWaitableTimerMutation !==
      "cross-owner-preflight-targeted-commit" ||
    latestEvidence.setWaitableTimerFailureAtomicity !== true ||
    latestEvidence.setWaitableTimerClockAuthorityUsed !== true ||
    latestEvidence.setWaitableTimerHostTimerImported !== false ||
    latestEvidence.setWaitableTimerBrowserTimerImported !== false ||
    latestEvidence.setWaitableTimerExpirationImplemented !== true ||
    latestEvidence.setWaitableTimerNodeCompletions !== timerArmCount ||
    latestEvidence.setWaitableTimerChromeCompletions !== timerArmCount ||
    latestEvidence.setWaitableTimerNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push(
      "latest waitable-timer identity, arm, and lifetime evidence is invalid",
    );
  }
  if (
    latestEvidence.nativeWaitForSingleObjectExPrimaryTraceBytes !== 1_615_447 ||
    latestEvidence.nativeWaitForSingleObjectExPrimaryTraceLines !== 5_018 ||
    latestEvidence.nativeWaitForSingleObjectExPrimaryTraceSha256 !==
      "1ad73149838b453939c1e5c638775f645ddd42d032358bbd6ff0aceee2a0a1eb" ||
    latestEvidence.nativeWaitForSingleObjectExPrimaryBalancedApiPairs !==
      2_471 ||
    latestEvidence.nativeWaitForSingleObjectExConfirmationTraceBytes !==
      1_610_933 ||
    latestEvidence.nativeWaitForSingleObjectExConfirmationTraceLines !==
      5_019 ||
    latestEvidence.nativeWaitForSingleObjectExConfirmationTraceSha256 !==
      "e851ed15088768643823fbbb14d3d0a87372b19ee770ff936c41bf0a4fa7c0c0" ||
    latestEvidence.nativeWaitForSingleObjectExConfirmationBalancedApiPairs !==
      2_471 ||
    latestEvidence.nativeWaitForSingleObjectExTraceDiagnostics !== 0 ||
    latestEvidence.nativeWaitForSingleObjectExBgiProfile !==
      "x86-bgi-waitable-timer-wait-v1" ||
    latestEvidence.nativeWaitForSingleObjectExBgiCallerReturnRva !==
      "0x00050615" ||
    latestEvidence.nativeWaitForSingleObjectExBgiTimeoutMilliseconds !== 10 ||
    latestEvidence.nativeWaitForSingleObjectExBgiAlertable !== true ||
    latestEvidence.nativeWaitForSingleObjectExBgiResult !== 0 ||
    latestEvidence.nativeWaitForSingleObjectExBgiLastError !== 0 ||
    latestEvidence.nativeWaitForSingleObjectExBgiResultPreservedInEax !==
      true ||
    latestEvidence.nativeWaitForSingleObjectExOracleProfile !==
      "x86-wait-for-single-object-ex-timer-v1" ||
    latestEvidence.nativeWaitForSingleObjectExOracleRelativeDueTime100ns !==
      -5_000 ||
    latestEvidence.nativeWaitForSingleObjectExOracleManualWaitResults?.join(
      ",",
    ) !== "0,0" ||
    latestEvidence.nativeWaitForSingleObjectExOracleTimeoutResult !== 258 ||
    latestEvidence.nativeWaitForSingleObjectExOracleInvalidHandleResult !==
      0xffff_ffff ||
    latestEvidence.nativeWaitForSingleObjectExOracleInvalidHandleLastError !==
      6 ||
    latestEvidence.nativeWaitForSingleObjectExOracleSuccessAndTimeoutPreserveLastError !==
      true ||
    latestEvidence.nativeWaitForSingleObjectExOracleAllHookSystemResultsMatched !==
      true ||
    latestEvidence.nativeWaitForSingleObjectExExecutableHashUnchanged !==
      true ||
    latestEvidence.waitForSingleObjectExImplemented !== true ||
    latestEvidence.waitForSingleObjectExSequences?.join(",") !==
      timerWaitSequences ||
    latestEvidence.waitForSingleObjectExInstructionCounts?.join(",") !==
      timerWaitInstructionCounts ||
    latestEvidence.waitForSingleObjectExArguments?.join(",") !==
      "0x57010002,0x0000000a,0x00000001" ||
    latestEvidence.waitForSingleObjectExResults?.join(",") !==
      timerRepeatedZeroes ||
    latestEvidence.waitForSingleObjectExReturnAddress !== "0x10050615" ||
    latestEvidence.waitForSingleObjectExLastErrors?.join(",") !==
      timerRepeatedZeroes ||
    latestEvidence.waitForSingleObjectExGuestWrites !== 0 ||
    latestEvidence.waitForSingleObjectExImplementationId !==
      "kernel32.wait-for-single-object-ex.waitable-timer.v1" ||
    latestEvidence.waitForSingleObjectExArmGenerations?.join(",") !==
      timerGenerations ||
    latestEvidence.waitForSingleObjectExDeadlineTimes100ns?.join(",") !==
      timerDeadlines ||
    latestEvidence.waitForSingleObjectExClockTimes100ns?.join(",") !==
      timerDeadlines ||
    latestEvidence.waitForSingleObjectExClockGenerations?.join(",") !==
      timerGenerations ||
    latestEvidence.waitForSingleObjectExStateAuthority !==
      "runtime-timers-scheduler-and-typed-object-table" ||
    latestEvidence.waitForSingleObjectExClockAuthority !==
      "runtime-timers-logical-clock" ||
    latestEvidence.waitForSingleObjectExMutation !==
      "cross-owner-preflight-targeted-commit" ||
    latestEvidence.waitForSingleObjectExFailureAtomicity !== true ||
    latestEvidence.waitForSingleObjectExApcDeliveryImplemented !== false ||
    latestEvidence.waitForSingleObjectExPacingService !==
      "runtime.timer-delay.v1" ||
    latestEvidence.waitForSingleObjectExPacingPayloadVersion !== 1 ||
    latestEvidence.waitForSingleObjectExPacingRequestIds?.join(",") !==
      timerPacingRequestIds ||
    latestEvidence.waitForSingleObjectExPacingRegistrationIds?.join(",") !==
      timerPacingRegistrationIds ||
    latestEvidence.waitForSingleObjectExPacingDelayMilliseconds?.join(",") !==
      timerRepeatedOnes ||
    latestEvidence.waitForSingleObjectExPacingTargetTimes100ns?.join(",") !==
      timerDeadlines ||
    latestEvidence.waitForSingleObjectExPacingClockGenerations?.join(",") !==
      timerPacingClockGenerations ||
    latestEvidence.waitForSingleObjectExPacingArmGenerations?.join(",") !==
      timerGenerations ||
    latestEvidence.waitForSingleObjectExPacingOutcome !== "object-signaled" ||
    latestEvidence.waitForSingleObjectExNodePacingMode !==
      "deterministic-node-ack" ||
    latestEvidence.waitForSingleObjectExLogicalCompletionAuthority !==
      "rust-runtime" ||
    latestEvidence.waitForSingleObjectExPageLifecyclePolicy !==
      "freeze-resume-preserve-remaining-bounded-catch-up" ||
    latestEvidence.waitForSingleObjectExRuntimeWidePacingBarrier !== true ||
    latestEvidence.waitForSingleObjectExOwnerResumeAuthority !==
      "rust-host-completion" ||
    latestEvidence.waitForSingleObjectExOtherGuestLaneExecutionDuringPacing !==
      false ||
    latestEvidence.waitForSingleObjectExHostClockImported !== false ||
    latestEvidence.waitForSingleObjectExBrowserTimerImported !== false ||
    latestEvidence.waitForSingleObjectExBrowserPacingRequests !==
      timerCycleCount ||
    latestEvidence.waitForSingleObjectExBrowserPacingCompletions !==
      timerCycleCount ||
    latestEvidence.waitForSingleObjectExBrowserPacingCancellations !== 0 ||
    latestEvidence.waitForSingleObjectExBrowserPacingStaleCallbacks !== 0 ||
    latestEvidence.waitForSingleObjectExBrowserPacingPending !== 0 ||
    latestEvidence.waitForSingleObjectExNodeCompletions !== timerCycleCount ||
    latestEvidence.waitForSingleObjectExChromeCompletions !== timerCycleCount ||
    latestEvidence.waitForSingleObjectExNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest waitable-timer wait evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadIconAPrimaryTraceBytes !== 285_147 ||
    latestEvidence.nativeLoadIconAPrimaryTraceLines !== 779 ||
    latestEvidence.nativeLoadIconAPrimaryBalancedApiPairs !== 354 ||
    latestEvidence.nativeLoadIconAConfirmationTraceBytes !== 2_257_599 ||
    latestEvidence.nativeLoadIconAConfirmationTraceLines !== 6_952 ||
    latestEvidence.nativeLoadIconAConfirmationBalancedApiPairs !== 3_438 ||
    latestEvidence.nativeLoadIconATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadIconAOracleProfile !==
      "x86-module-group-icon-shared-identity-v1" ||
    latestEvidence.nativeLoadIconAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadIconAOracleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadIconAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadIconAOracleMissingResourceLastError !== 1_813 ||
    latestEvidence.nativeLoadIconABgiProfile !== "x86-bgi-load-icon-a-v1" ||
    latestEvidence.nativeLoadIconABgiCallCount !== 2 ||
    latestEvidence.nativeLoadIconABgiCallerReturnRvas?.join(",") !==
      "0x00067cf3,0x00067d7c" ||
    latestEvidence.nativeLoadIconABgiResourceId !== 101 ||
    latestEvidence.nativeLoadIconABgiResultsNonNull !== true ||
    latestEvidence.nativeLoadIconABgiIdentityShared !== true ||
    latestEvidence.nativeLoadIconABgiLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLoadIconABgiConsumer !==
      "wndclassexa-hicon-with-null-hiconsm" ||
    latestEvidence.nativeLoadIconASelectedWidth !== 32 ||
    latestEvidence.nativeLoadIconASelectedHeight !== 32 ||
    latestEvidence.nativeLoadIconASelectedPlanes !== 1 ||
    latestEvidence.nativeLoadIconASelectedBitsPerPixel !== 32 ||
    latestEvidence.nativeLoadIconALargeSystemMetrics?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadIconASmallSystemMetrics?.join(",") !== "16,16" ||
    latestEvidence.nativeLoadIconAGroupResourceId !== 101 ||
    latestEvidence.nativeLoadIconAGroupLanguageId !== 1_041 ||
    latestEvidence.nativeLoadIconAGroupDataBytes !== 62 ||
    latestEvidence.nativeLoadIconASelectedIconResourceId !== 3 ||
    latestEvidence.nativeLoadIconASelectedIconDataBytes !== 4_264 ||
    latestEvidence.loadIconAImplemented !== true ||
    latestEvidence.loadIconASequence !== 1_339 ||
    latestEvidence.loadIconAInstructionCount !== 49_942 ||
    latestEvidence.loadIconAArguments?.join(",") !== "0x10000000,0x00000065" ||
    latestEvidence.loadIconAReturnAddress !== "0x10067cf3" ||
    latestEvidence.loadIconAResult !== "0x49010001" ||
    latestEvidence.loadIconALastError !== 0 ||
    latestEvidence.loadIconAGuestWrites !== 0 ||
    latestEvidence.loadIconAStateOwner !== "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadIconAIconId !== 1 ||
    latestEvidence.loadIconAHandleNamespace !== "vine.logical-icon.v1" ||
    latestEvidence.loadIconAShared !== true ||
    latestEvidence.loadIconAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadIconAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadIconAGroupResourceId !== 101 ||
    latestEvidence.loadIconALanguageId !== 1_041 ||
    latestEvidence.loadIconAIconResourceId !== 3 ||
    latestEvidence.loadIconAGroupDataRva !== "0x0016ea08" ||
    latestEvidence.loadIconAGroupDataBytes !== 62 ||
    latestEvidence.loadIconAIconDataRva !== "0x0016bf5c" ||
    latestEvidence.loadIconAIconDataBytes !== 4_264 ||
    latestEvidence.loadIconAWidth !== 32 ||
    latestEvidence.loadIconAHeight !== 32 ||
    latestEvidence.loadIconAPlanes !== 1 ||
    latestEvidence.loadIconABitsPerPixel !== 32 ||
    latestEvidence.loadIconARepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadIconAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadIconAHostIconImported !== false ||
    latestEvidence.loadIconABrowserIconImported !== false
  ) {
    errors.push("latest module-icon identity and resource evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadCursorAPrimaryTraceBytes !== 336_703 ||
    latestEvidence.nativeLoadCursorAPrimaryTraceLines !== 812 ||
    latestEvidence.nativeLoadCursorAPrimaryBalancedApiPairs !== 370 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceBytes !== 338_341 ||
    latestEvidence.nativeLoadCursorAConfirmationTraceLines !== 815 ||
    latestEvidence.nativeLoadCursorAConfirmationBalancedApiPairs !== 371 ||
    latestEvidence.nativeLoadCursorATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadCursorAOracleProfile !==
      "x86-system-arrow-and-module-cursor-shared-identity-v2" ||
    latestEvidence.nativeLoadCursorAOracleArrowResourceId !== 32_512 ||
    latestEvidence.nativeLoadCursorAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadCursorAOracleRepeatedIdentityShared !== true ||
    latestEvidence.nativeLoadCursorAOracleSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadCursorAOracleMissingResourceLastError !== 1_814 ||
    latestEvidence.nativeLoadCursorAOracleModuleResourceId !== 106 ||
    latestEvidence.nativeLoadCursorAOracleModuleValidCallsSucceeded !== true ||
    latestEvidence.nativeLoadCursorAOracleModuleRepeatedIdentityShared !==
      true ||
    latestEvidence.nativeLoadCursorAOracleModuleSuccessLastError !== 0 ||
    latestEvidence.nativeLoadCursorAOracleModuleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadCursorAOracleModuleMissingResourceLastError !==
      1_813 ||
    latestEvidence.nativeLoadCursorAOracleModuleWrongTypeResourceId !== 101 ||
    latestEvidence.nativeLoadCursorAOracleModuleWrongTypeLastError !== 1_813 ||
    latestEvidence.nativeLoadCursorASystemMetrics?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadCursorAModuleGroupResourceType !== 12 ||
    latestEvidence.nativeLoadCursorAModuleGroupResourceId !== 106 ||
    latestEvidence.nativeLoadCursorAModuleLanguageId !== 1_041 ||
    latestEvidence.nativeLoadCursorAModuleGroupDataRva !== "0x0016e9f4" ||
    latestEvidence.nativeLoadCursorAModuleGroupDataBytes !== 20 ||
    latestEvidence.nativeLoadCursorAModuleCursorResourceType !== 1 ||
    latestEvidence.nativeLoadCursorAModuleCursorResourceId !== 5 ||
    latestEvidence.nativeLoadCursorAModuleCursorDataRva !== "0x00154f88" ||
    latestEvidence.nativeLoadCursorAModuleCursorDataBytes !== 2_220 ||
    latestEvidence.nativeLoadCursorAModuleDimensions?.join(",") !== "32,32" ||
    latestEvidence.nativeLoadCursorAModulePlanes !== 1 ||
    latestEvidence.nativeLoadCursorAModuleBitsPerPixel !== 8 ||
    latestEvidence.nativeLoadCursorAModuleHotspot?.join(",") !== "0,0" ||
    latestEvidence.nativeLoadCursorABgiProfile !== "x86-bgi-load-cursor-a-v1" ||
    latestEvidence.nativeLoadCursorABgiCallCount !== 4 ||
    latestEvidence.nativeLoadCursorABgiSystemArrowCallCount !== 3 ||
    latestEvidence.nativeLoadCursorABgiCallerReturnRvas?.join(",") !==
      "0x00067d09,0x00067d8c,0x00067fd2,0x00067fe1" ||
    latestEvidence.nativeLoadCursorABgiSystemArrowResultsShared !== true ||
    latestEvidence.nativeLoadCursorABgiLastErrors?.join(",") !== "0,0,0,0" ||
    latestEvidence.nativeLoadCursorABgiFirstConsumer !==
      "wndclassexa-hcursor" ||
    latestEvidence.nativeLoadCursorABgiModuleResourceId !== 106 ||
    latestEvidence.nativeLoadCursorABgiModuleResultNonNull !== true ||
    latestEvidence.loadCursorAImplemented !== true ||
    latestEvidence.loadCursorASequence !== 1_340 ||
    latestEvidence.loadCursorAInstructionCount !== 49_949 ||
    latestEvidence.loadCursorAArguments?.join(",") !==
      "0x00000000,0x00007f00" ||
    latestEvidence.loadCursorAReturnAddress !== "0x10067d09" ||
    latestEvidence.loadCursorAResult !== "0x43010001" ||
    latestEvidence.loadCursorALastError !== 0 ||
    latestEvidence.loadCursorASuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.loadCursorAGuestWrites !== 0 ||
    latestEvidence.loadCursorAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadCursorACursorId !== 1 ||
    latestEvidence.loadCursorAHandleNamespace !== "vine.logical-cursor.v1" ||
    latestEvidence.loadCursorAShared !== true ||
    latestEvidence.loadCursorASourceKind !== "system-predefined" ||
    latestEvidence.loadCursorAResourceId !== 32_512 ||
    latestEvidence.loadCursorACursorKind !== "arrow" ||
    latestEvidence.loadCursorARepeatedSystemResourceSharesIdentity !== true ||
    latestEvidence.loadCursorAModuleImplemented !== true ||
    latestEvidence.loadCursorAModuleSequence !== 17_815 ||
    latestEvidence.loadCursorAModuleOccurrence !== 4 ||
    latestEvidence.loadCursorAModuleInstructionCount !== 11_806_421 ||
    latestEvidence.loadCursorAModuleArguments?.join(",") !==
      "0x10000000,0x0000006a" ||
    latestEvidence.loadCursorAModuleReturnAddress !== "0x10067fe1" ||
    latestEvidence.loadCursorAModuleResult !== "0x43010002" ||
    latestEvidence.loadCursorAModuleLastError !== 0 ||
    latestEvidence.loadCursorAModuleSuccessLastErrorPolicy !== "zero" ||
    latestEvidence.loadCursorAModuleGuestWrites !== 0 ||
    latestEvidence.loadCursorAModuleCursorId !== 2 ||
    latestEvidence.loadCursorAModuleHandleNamespace !==
      "vine.logical-cursor.v1" ||
    latestEvidence.loadCursorAModuleShared !== true ||
    latestEvidence.loadCursorAModuleSourceKind !== "module-resource" ||
    latestEvidence.loadCursorAModuleId !== 15 ||
    latestEvidence.loadCursorAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadCursorAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadCursorAModuleGroupResourceId !== 106 ||
    latestEvidence.loadCursorAModuleLanguageId !== 1_041 ||
    latestEvidence.loadCursorAModuleCursorResourceId !== 5 ||
    latestEvidence.loadCursorAModuleGroupDataRva !== "0x0016e9f4" ||
    latestEvidence.loadCursorAModuleGroupDataBytes !== 20 ||
    latestEvidence.loadCursorAModuleCursorDataRva !== "0x00154f88" ||
    latestEvidence.loadCursorAModuleCursorDataBytes !== 2_220 ||
    latestEvidence.loadCursorAModuleWidth !== 32 ||
    latestEvidence.loadCursorAModuleHeight !== 32 ||
    latestEvidence.loadCursorAModulePlanes !== 1 ||
    latestEvidence.loadCursorAModuleBitsPerPixel !== 8 ||
    latestEvidence.loadCursorAModuleHotspotX !== 0 ||
    latestEvidence.loadCursorAModuleHotspotY !== 0 ||
    latestEvidence.loadCursorAModuleDistinctFromSystemIdentity !== true ||
    latestEvidence.loadCursorAModuleRepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadCursorAModuleFailureAtomicity !== true ||
    latestEvidence.loadCursorAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadCursorAHostCursorImported !== false ||
    latestEvidence.loadCursorABrowserCursorImported !== false
  ) {
    errors.push("latest system and module cursor identity evidence is invalid");
  }
  if (
    latestEvidence.nativeRegisterClassExAPrimaryTraceBytes !== 326_543 ||
    latestEvidence.nativeRegisterClassExAPrimaryTraceLines !== 804 ||
    latestEvidence.nativeRegisterClassExAPrimaryBalancedApiPairs !== 366 ||
    latestEvidence.nativeRegisterClassExAConfirmationTraceBytes !== 327_416 ||
    latestEvidence.nativeRegisterClassExAConfirmationTraceLines !== 805 ||
    latestEvidence.nativeRegisterClassExAConfirmationBalancedApiPairs !== 366 ||
    latestEvidence.nativeRegisterClassExATraceDiagnostics !== 0 ||
    latestEvidence.nativeRegisterClassExAOracleProfile !==
      "x86-register-class-ex-a-identity-lifetime-v1" ||
    latestEvidence.nativeRegisterClassExAOracleValidCallsSucceeded !== true ||
    latestEvidence.nativeRegisterClassExAOracleValidAtomsDistinct !== true ||
    latestEvidence.nativeRegisterClassExAOracleDuplicateRejected !== true ||
    latestEvidence.nativeRegisterClassExAOracleDuplicateLastError !== 1_410 ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeRejected !== true ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeLastError !== 87 ||
    latestEvidence.nativeRegisterClassExAOracleInvalidSizeDidNotReserveIdentity !==
      true ||
    latestEvidence.nativeRegisterClassExAOracleSuccessLastErrorsPreserved !==
      true ||
    latestEvidence.nativeRegisterClassExAOracleInputsPreserved !== true ||
    latestEvidence.nativeRegisterClassExAOracleNameAndAtomQueriesSucceeded !==
      true ||
    latestEvidence.nativeRegisterClassExAOracleCleanupSucceeded !== true ||
    latestEvidence.nativeRegisterClassExAOracleQueryAfterCleanupRejected !==
      true ||
    latestEvidence.nativeRegisterClassExABgiProfile !==
      "x86-bgi-register-class-ex-a-v1" ||
    latestEvidence.nativeRegisterClassExABgiCallCount !== 2 ||
    latestEvidence.nativeRegisterClassExABgiCallerReturnRvas?.join(",") !==
      "0x00067d28,0x00067dab" ||
    latestEvidence.nativeRegisterClassExABgiStructureBytes?.join(",") !==
      "48,48" ||
    latestEvidence.nativeRegisterClassExABgiStyles?.join(",") !== "40,12320" ||
    latestEvidence.nativeRegisterClassExABgiWindowProcedureRvas?.join(",") !==
      "0x00092550,0x0002e6e0" ||
    latestEvidence.nativeRegisterClassExABgiClassNames?.join(",") !==
      "BGI - Main window,BGI - Child window" ||
    latestEvidence.nativeRegisterClassExABgiSameMainModule !== true ||
    latestEvidence.nativeRegisterClassExABgiSharedIconAndCursorInputs !==
      true ||
    latestEvidence.nativeRegisterClassExABgiOptionalFieldsNull !== true ||
    latestEvidence.nativeRegisterClassExABgiResultsNonzero !== true ||
    latestEvidence.nativeRegisterClassExABgiLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeRegisterClassExABgiInputsPreserved !== true
  ) {
    errors.push("latest native window-class registration evidence is invalid");
  }
  if (
    latestEvidence.registerClassExAImplemented !== true ||
    latestEvidence.registerClassExACallCount !== 2 ||
    latestEvidence.registerClassExASequences?.join(",") !== "1341,1344" ||
    latestEvidence.registerClassExAInstructionCounts?.join(",") !==
      "49957,49984" ||
    latestEvidence.registerClassExAArguments?.join(",") !==
      "0x006ffb08,0x006ffb08" ||
    latestEvidence.registerClassExAReturnAddresses?.join(",") !==
      "0x10067d28,0x10067dab" ||
    latestEvidence.registerClassExAResults?.join(",") !==
      "0x0000c000,0x0000c001" ||
    latestEvidence.registerClassExALastErrors?.join(",") !== "0,0" ||
    latestEvidence.registerClassExASuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.registerClassExAGuestWrites !== 0 ||
    latestEvidence.registerClassExAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.registerClassExAClassIds?.join(",") !== "1,2" ||
    latestEvidence.registerClassExAAtomNamespace !==
      "vine.logical-window-class-atom.v1" ||
    latestEvidence.registerClassExAAtoms?.join(",") !== "49152,49153" ||
    latestEvidence.registerClassExALocal !== true ||
    latestEvidence.registerClassExAModuleId !== 15 ||
    latestEvidence.registerClassExAModuleLoadBase !== "0x10000000" ||
    latestEvidence.registerClassExAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.registerClassExAStyles?.join(",") !== "40,12320" ||
    latestEvidence.registerClassExAWindowProcedureAddresses?.join(",") !==
      "0x10092550,0x1002e6e0" ||
    latestEvidence.registerClassExAWindowProcedureRvas?.join(",") !==
      "0x00092550,0x0002e6e0" ||
    latestEvidence.registerClassExAWindowProcedureCallingConvention !==
      "stdcall" ||
    latestEvidence.registerClassExAWindowProcedureArgumentBytes !== 16 ||
    latestEvidence.registerClassExAClassExtraBytes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAWindowExtraBytes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAIconId !== 1 ||
    latestEvidence.registerClassExAIconHandle !== "0x49010001" ||
    latestEvidence.registerClassExACursorId !== 1 ||
    latestEvidence.registerClassExACursorHandle !== "0x43010001" ||
    latestEvidence.registerClassExABackgroundBrushes?.join(",") !== "0,0" ||
    latestEvidence.registerClassExAClassNames?.join(",") !==
      "BGI - Main window,BGI - Child window" ||
    latestEvidence.registerClassExAClassNameEncoding !== "windows-932-v1" ||
    latestEvidence.registerClassExAMenuNamesNull !== true ||
    latestEvidence.registerClassExASmallIconsNull !== true ||
    latestEvidence.registerClassExASecondIconAndCursorReuseIdentity !== true ||
    latestEvidence.registerClassExADuplicateSameModuleNameLastError !== 1_410 ||
    latestEvidence.registerClassExAInvalidSizeLastError !== 87 ||
    latestEvidence.registerClassExAProcessExitRetiresIdentity !== true ||
    latestEvidence.registerClassExAHostClassImported !== false ||
    latestEvidence.registerClassExABrowserClassImported !== false
  ) {
    errors.push(
      "latest logical window-class identity and lifetime evidence is invalid",
    );
  }
  if (
    latestEvidence.nativeGetSystemMetricsPrimaryTraceBytes !== 55_901 ||
    latestEvidence.nativeGetSystemMetricsPrimaryTraceLines !== 99 ||
    latestEvidence.nativeGetSystemMetricsPrimaryBalancedApiPairs !== 22 ||
    latestEvidence.nativeGetSystemMetricsConfirmationTraceBytes !== 55_839 ||
    latestEvidence.nativeGetSystemMetricsConfirmationTraceLines !== 99 ||
    latestEvidence.nativeGetSystemMetricsConfirmationBalancedApiPairs !== 22 ||
    latestEvidence.nativeGetSystemMetricsTraceDiagnostics !== 0 ||
    latestEvidence.nativeGetSystemMetricsOracleProfile !==
      "x86-system-metrics-last-error-v1" ||
    latestEvidence.nativeGetSystemMetricsOracleIndices?.join(",") !==
      "7,8,4,0,1" ||
    latestEvidence.nativeGetSystemMetricsOracleValues?.join(",") !==
      "3,3,23,1920,1080" ||
    latestEvidence.nativeGetSystemMetricsOracleAllResultsMatchHooks !== true ||
    latestEvidence.nativeGetSystemMetricsOracleSuccessLastErrorsPreserved !==
      true ||
    latestEvidence.nativeGetSystemMetricsBgiProfile !==
      "x86-bgi-initial-window-metrics-v1" ||
    latestEvidence.nativeGetSystemMetricsBgiCallCount !== 5 ||
    latestEvidence.nativeGetSystemMetricsBgiIndices?.join(",") !==
      "7,8,4,0,1" ||
    latestEvidence.nativeGetSystemMetricsBgiValues?.join(",") !==
      "3,3,23,1920,1080" ||
    latestEvidence.nativeGetSystemMetricsBgiCallerReturnRvas?.join(",") !==
      "0x00067ddb,0x00067de6,0x00067dee,0x00067e26,0x00067e2e" ||
    latestEvidence.nativeGetSystemMetricsBgiLastErrors?.join(",") !==
      "0,0,0,0,0" ||
    latestEvidence.nativeGetSystemMetricsBgiFrameWidth !== 6 ||
    latestEvidence.nativeGetSystemMetricsBgiFrameHeight !== 29 ||
    latestEvidence.nativeGetSystemMetricsSmallIconPrimaryTraceBytes !==
      372_414 ||
    latestEvidence.nativeGetSystemMetricsSmallIconPrimaryTraceLines !== 865 ||
    latestEvidence.nativeGetSystemMetricsSmallIconConfirmationTraceBytes !==
      372_020 ||
    latestEvidence.nativeGetSystemMetricsSmallIconConfirmationTraceLines !==
      864 ||
    latestEvidence.nativeGetSystemMetricsSmallIconProfile !==
      "x86-bgi-small-icon-metrics-v1" ||
    latestEvidence.nativeGetSystemMetricsSmallIconIndices?.join(",") !==
      "50,49" ||
    latestEvidence.nativeGetSystemMetricsSmallIconValues?.join(",") !==
      "16,16" ||
    latestEvidence.nativeGetSystemMetricsSmallIconCallerReturnRvas?.join(
      ",",
    ) !== "0x00067f2d,0x00067f32" ||
    latestEvidence.nativeGetSystemMetricsSmallIconLastErrors?.join(",") !==
      "1401,1401" ||
    latestEvidence.nativeGetSystemMetricsSmallIconConsumer !==
      "loadimagea-resource-101-image-icon-width-height-lr-shared" ||
    latestEvidence.nativeGetSystemMetricsSmallIconExecutableHashUnchanged !==
      true ||
    latestEvidence.getSystemMetricsImplemented !== true ||
    latestEvidence.getSystemMetricsCallCount !== 11 ||
    latestEvidence.getSystemMetricsSequences?.join(",") !==
      "1345,1346,1347,1348,1349,1395,1396,1400,1401,1405,1406" ||
    latestEvidence.getSystemMetricsInstructionCounts?.join(",") !==
      "49990,49995,50000,50020,50024,11476518,11476522,11476595,11476602,11476761,11476765" ||
    latestEvidence.getSystemMetricsArguments?.join(",") !==
      "7,8,4,0,1,50,49,1,0,50,49" ||
    latestEvidence.getSystemMetricsReturnAddresses?.join(",") !==
      "0x10067ddb,0x10067de6,0x10067dee,0x10067e26,0x10067e2e,0x10067f2d,0x10067f32,0x10067f65,0x10067f70,0x100929d2,0x100929d7" ||
    latestEvidence.getSystemMetricsResults?.join(",") !==
      "3,3,23,1920,1080,16,16,1080,1920,16,16" ||
    latestEvidence.getSystemMetricsLastErrors?.join(",") !==
      "0,0,0,0,0,0,0,0,0,0,0" ||
    latestEvidence.getSystemMetricsSuccessLastErrorPolicy !== "preserve" ||
    latestEvidence.getSystemMetricsGuestWrites !== 0 ||
    latestEvidence.getSystemMetricsDisplayStateOwner !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.getSystemMetricsNonclientPolicyOwner !==
      "User32CompatibilityPolicy.get_system_metrics" ||
    latestEvidence.getSystemMetricsProfileId !==
      "initial-window-metrics-1920x1080-small-icon-16x16-v3" ||
    latestEvidence.getSystemMetricsSupportedIndices?.join(",") !==
      "0,1,4,7,8,49,50" ||
    latestEvidence.getSystemMetricsSmallIconWidth !== 16 ||
    latestEvidence.getSystemMetricsSmallIconWidthConsumer !==
      "paired-loadimagea-small-icon-width" ||
    latestEvidence.getSystemMetricsSmallIconHeight !== 16 ||
    latestEvidence.getSystemMetricsSmallIconHeightConsumer !==
      "paired-loadimagea-small-icon-height" ||
    latestEvidence.getSystemMetricsHostValuesImported !== false ||
    latestEvidence.getSystemMetricsBrowserValuesImported !== false
  ) {
    errors.push("latest controlled system-metrics evidence is invalid");
  }
  if (
    latestEvidence.nativeLoadImageAPrimaryTraceBytes !== 295_589 ||
    latestEvidence.nativeLoadImageAPrimaryTraceLines !== 803 ||
    latestEvidence.nativeLoadImageAPrimaryBalancedApiPairs !== 365 ||
    latestEvidence.nativeLoadImageAConfirmationTraceBytes !== 295_572 ||
    latestEvidence.nativeLoadImageAConfirmationTraceLines !== 803 ||
    latestEvidence.nativeLoadImageAConfirmationBalancedApiPairs !== 365 ||
    latestEvidence.nativeLoadImageATraceDiagnostics !== 0 ||
    latestEvidence.nativeLoadImageABgiProfile !==
      "x86-bgi-load-image-a-small-icon-v1" ||
    latestEvidence.nativeLoadImageABgiCallCount !== 2 ||
    latestEvidence.nativeLoadImageABgiCallerReturnRvas?.join(",") !==
      "0x00067f43,0x000929e9" ||
    latestEvidence.nativeLoadImageABgiArguments?.join(",") !==
      "main-module,101,1,16,16,0x00008000" ||
    latestEvidence.nativeLoadImageABgiResultsNonNull !== true ||
    latestEvidence.nativeLoadImageABgiIdentityShared !== true ||
    latestEvidence.nativeLoadImageABgiEntryLastErrors?.join(",") !== "1401,0" ||
    latestEvidence.nativeLoadImageABgiExitLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeLoadImageABgiConsumer !==
      "sendmessagea-wm-seticon-icon-small-then-repeat-null-check" ||
    latestEvidence.nativeLoadImageAOracleProfile !==
      "x86-module-small-icon-load-image-shared-identity-v1" ||
    latestEvidence.nativeLoadImageAOracleSuccessLastError !== 0 ||
    latestEvidence.nativeLoadImageAOracleMissingResourceId !== 65_535 ||
    latestEvidence.nativeLoadImageAOracleMissingResourceLastError !== 1_813 ||
    latestEvidence.nativeLoadImageASelectedWidth !== 16 ||
    latestEvidence.nativeLoadImageASelectedHeight !== 16 ||
    latestEvidence.nativeLoadImageASelectedPlanes !== 1 ||
    latestEvidence.nativeLoadImageASelectedBitsPerPixel !== 32 ||
    latestEvidence.nativeLoadImageAExecutableHashUnchanged !== true ||
    latestEvidence.loadImageAImplemented !== true ||
    latestEvidence.loadImageACallCount !== 2 ||
    latestEvidence.loadImageASequences?.join(",") !== "1397,1407" ||
    latestEvidence.loadImageASequence !== 1_397 ||
    latestEvidence.loadImageAInstructionCount !== 11_476_529 ||
    latestEvidence.loadImageAArguments?.join(",") !==
      "0x10000000,0x00000065,0x00000001,0x00000010,0x00000010,0x00008000" ||
    latestEvidence.loadImageAReturnAddress !== "0x10067f43" ||
    latestEvidence.loadImageAResult !== "0x49010002" ||
    latestEvidence.loadImageALastError !== 0 ||
    latestEvidence.loadImageAGuestWrites !== 0 ||
    latestEvidence.loadImageAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.loadImageAIconId !== 2 ||
    latestEvidence.loadImageAHandleNamespace !== "vine.logical-icon.v1" ||
    latestEvidence.loadImageAShared !== true ||
    latestEvidence.loadImageAModuleId !== 15 ||
    latestEvidence.loadImageAModuleLoadBase !== "0x10000000" ||
    latestEvidence.loadImageAModuleSourceSha256 !==
      "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1" ||
    latestEvidence.loadImageAGroupResourceId !== 101 ||
    latestEvidence.loadImageALanguageId !== 1_041 ||
    latestEvidence.loadImageAIconResourceId !== 4 ||
    latestEvidence.loadImageAGroupDataRva !== "0x0016ea08" ||
    latestEvidence.loadImageAGroupDataBytes !== 62 ||
    latestEvidence.loadImageAIconDataRva !== "0x0016d004" ||
    latestEvidence.loadImageAIconDataBytes !== 1_128 ||
    latestEvidence.loadImageAWidth !== 16 ||
    latestEvidence.loadImageAHeight !== 16 ||
    latestEvidence.loadImageAPlanes !== 1 ||
    latestEvidence.loadImageABitsPerPixel !== 32 ||
    latestEvidence.loadImageALargeIconUsesDistinctIdentity !== true ||
    latestEvidence.loadImageARepeatedResourceSharesIdentity !== true ||
    latestEvidence.loadImageAMissingResourceLastError !== 1_813 ||
    latestEvidence.loadImageAUnsupportedVariantsFailBeforeMutation !== true ||
    latestEvidence.loadImageAProcessExitRetiresIdentity !== true ||
    latestEvidence.loadImageAHostIconImported !== false ||
    latestEvidence.loadImageABrowserIconImported !== false ||
    latestEvidence.writebackBatches !==
      profileExpectations.latestWritebackBatches ||
    latestEvidence.writebackDirtyPages !==
      profileExpectations.latestWritebackDirtyPages ||
    latestEvidence.writebackChangedBytes !==
      profileExpectations.latestWritebackChangedBytes ||
    latestEvidence.hostDeltaBatches !==
      profileExpectations.latestHostDeltaBatches ||
    latestEvidence.finalMemoryGeneration !==
      profileExpectations.latestFinalMemoryGeneration
  ) {
    errors.push("latest LoadImageA small-icon evidence is invalid");
  }
  if (
    latestEvidence.nativeSendMessageAPrimaryTraceBytes !== 320_348 ||
    latestEvidence.nativeSendMessageAPrimaryTraceLines !== 808 ||
    latestEvidence.nativeSendMessageAPrimaryBalancedApiPairs !== 367 ||
    latestEvidence.nativeSendMessageAConfirmationTraceBytes !== 320_408 ||
    latestEvidence.nativeSendMessageAConfirmationTraceLines !== 806 ||
    latestEvidence.nativeSendMessageAConfirmationBalancedApiPairs !== 366 ||
    latestEvidence.nativeSendMessageATraceDiagnostics !== 0 ||
    latestEvidence.nativeSendMessageAProfile !==
      "x86-bgi-send-message-a-wm-seticon-v1" ||
    latestEvidence.nativeSendMessageACallerReturnRva !== "0x00067f58" ||
    latestEvidence.nativeSendMessageAMessage !== 128 ||
    latestEvidence.nativeSendMessageAWParam !== 0 ||
    latestEvidence.nativeSendMessageAResult !== 0 ||
    latestEvidence.nativeSendMessageAEntryLastError !== 0 ||
    latestEvidence.nativeSendMessageAExitLastError !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureProfile !==
      "bgi-main-window-procedure-rva-00092550-v1" ||
    latestEvidence.nativeSendMessageAWindowProcedureRva !== "0x00092550" ||
    latestEvidence.nativeSendMessageAWindowProcedureParentRelation !==
      "direct-child-same-thread" ||
    latestEvidence.nativeSendMessageAWindowProcedureArgumentsMatch !== true ||
    latestEvidence.nativeSendMessageAWindowProcedureResult !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureEntryLastError !== 0 ||
    latestEvidence.nativeSendMessageAWindowProcedureExitLastError !== 0 ||
    latestEvidence.nativeSendMessageAConsumer !==
      "lresult-ignored-before-window-position-argument-setup" ||
    latestEvidence.nativeSendMessageASecondLoadImageAfterCallback !== true ||
    latestEvidence.nativeSendMessageAExecutableHashUnchanged !== true ||
    latestEvidence.sendMessageAImplemented !== true ||
    latestEvidence.sendMessageASequence !== 1_398 ||
    latestEvidence.sendMessageAInstructionCount !== 11_476_536 ||
    latestEvidence.sendMessageAArguments?.join(",") !==
      "0x48010001,0x00000080,0x00000000,0x49010002" ||
    latestEvidence.sendMessageAReturnAddress !== "0x10067f58" ||
    latestEvidence.sendMessageAResult !== "0x00000000" ||
    latestEvidence.sendMessageALastError !== 0 ||
    latestEvidence.sendMessageAGuestWrites !== 0 ||
    latestEvidence.sendMessageAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.sendMessageAWindowId !== 1 ||
    latestEvidence.sendMessageAWindowHandle !== "0x48010001" ||
    latestEvidence.sendMessageAOwnerThreadId !== 8_192 ||
    latestEvidence.sendMessageAClassId !== 1 ||
    latestEvidence.sendMessageAIconId !== 2 ||
    latestEvidence.sendMessageAIconHandle !== "0x49010002" ||
    latestEvidence.sendMessageACallbackFrameId !== 7 ||
    latestEvidence.sendMessageACallbackTarget !== "0x10092550" ||
    latestEvidence.sendMessageACallbackArguments?.join(",") !==
      "0x48010001,0x00000080,0x00000000,0x49010002" ||
    latestEvidence.sendMessageACallbackAllocationStart !== "0x006ffac8" ||
    latestEvidence.sendMessageACallbackStackPointer !== "0x006ffac8" ||
    latestEvidence.sendMessageACallbackScratchAddress !== "0x00000000" ||
    latestEvidence.sendMessageACallbackScratchBytes !== 0 ||
    latestEvidence.sendMessageACallbackFrameBytes !== 20 ||
    latestEvidence.sendMessageACallbackReturnSentinel !== "0x7f010000" ||
    latestEvidence.sendMessageACallbackReturnInstructionCount !== 11_476_589 ||
    latestEvidence.sendMessageACallbackResult !== "0x00000000" ||
    latestEvidence.sendMessageACallbackDepthAfterReturn !== 0 ||
    latestEvidence.sendMessageANestedDefWindowProcSequence !== 1_399 ||
    latestEvidence.sendMessageANestedDefWindowProcInstructionCount !==
      11_476_576 ||
    latestEvidence.sendMessageANestedDefWindowProcResult !== "0x00000000" ||
    latestEvidence.sendMessageAFailureAtomicity !== true ||
    latestEvidence.sendMessageAWindowIconMutationImplemented !== true ||
    latestEvidence.sendMessageAMessageQueueIntegrated !== false ||
    latestEvidence.sendMessageAHostDispatchImported !== false ||
    latestEvidence.sendMessageABrowserDispatchImported !== false
  ) {
    errors.push("latest synchronous small-icon message evidence is invalid");
  }
  const setWindowPosShowGeometry = requireRecord(
    latestEvidence.setWindowPosShowGeometry,
    "activeRuntimeCheckpoint.latestEvidence.setWindowPosShowGeometry",
    errors,
  );
  if (
    latestEvidence.nativeSetWindowPosPrimaryTraceBytes !== 95_895 ||
    latestEvidence.nativeSetWindowPosPrimaryTraceLines !== 132 ||
    latestEvidence.nativeSetWindowPosPrimaryBalancedApiPairs !== 39 ||
    latestEvidence.nativeSetWindowPosConfirmationTraceBytes !== 95_912 ||
    latestEvidence.nativeSetWindowPosConfirmationTraceLines !== 132 ||
    latestEvidence.nativeSetWindowPosConfirmationBalancedApiPairs !== 39 ||
    latestEvidence.nativeSetWindowPosTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowPosProfile !==
      "x86-bgi-set-window-pos-initial-placement-v1" ||
    latestEvidence.nativeSetWindowPosCallerRvas?.join(",") !==
      "0x00067f88,0x00067f9f" ||
    latestEvidence.nativeSetWindowPosResults?.join(",") !== "1,1" ||
    latestEvidence.nativeSetWindowPosEntryLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeSetWindowPosExitLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeSetWindowPosWindowPosFlags?.join(",") !==
      "0x10000d59,0x00001d9f" ||
    latestEvidence.nativeSetWindowPosShowWindowPos?.join(",") !==
      "15360,8640,806,629" ||
    latestEvidence.nativeSetWindowPosMoveLparam !== "0x21da3c03" ||
    latestEvidence.nativeSetWindowPosCallbackMessages?.join(",") !==
      "71,3,127,71" ||
    latestEvidence.nativeSetWindowPosGetIconWParam !== 2 ||
    latestEvidence.nativeSetWindowPosGetIconRelation !==
      "same-shared-16x16-loadimagea-hicon" ||
    latestEvidence.nativeSetWindowPosConsumer !==
      "bool-results-ignored-and-second-call-returns" ||
    latestEvidence.nativeSetWindowPosExecutableHashUnchanged !== true ||
    latestEvidence.nativeSetWindowPosActivationPrimaryTraceBytes !== 83_322 ||
    latestEvidence.nativeSetWindowPosActivationPrimaryTraceLines !== 97 ||
    latestEvidence.nativeSetWindowPosActivationPrimaryBalancedApiPairs !== 17 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationTraceBytes !==
      83_309 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationTraceLines !== 97 ||
    latestEvidence.nativeSetWindowPosActivationConfirmationBalancedApiPairs !==
      17 ||
    latestEvidence.nativeSetWindowPosActivationTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowPosActivationProfile !==
      "x86-bgi-first-set-window-pos-size-v1" ||
    latestEvidence.nativeSetWindowPosActivationCallsPerTrace !== 1 ||
    latestEvidence.nativeSetWindowPosActivationCallerRva !== "0x0005cb4b" ||
    latestEvidence.nativeSetWindowPosActivationResult !== 1 ||
    latestEvidence.nativeSetWindowPosActivationEntryLastError !== 0 ||
    latestEvidence.nativeSetWindowPosActivationExitLastError !== 0 ||
    latestEvidence.nativeSetWindowPosActivationInvalidWindowResult !== 0 ||
    latestEvidence.nativeSetWindowPosActivationInvalidWindowLastError !==
      1_400 ||
    latestEvidence.nativeSetWindowPosActivationCallbackMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    latestEvidence.nativeSetWindowPosActivationCallbackResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    latestEvidence.nativeSetWindowPosActivationInitialRelations?.join(",") !==
      "null,null,external" ||
    latestEvidence.nativeSetWindowPosActivationSecondWindowPosRelations?.join(
      ",",
    ) !== "target,null,target" ||
    latestEvidence.nativeSetWindowPosActivationImeAndFocusRelations?.join(
      ",",
    ) !== "target,target,target" ||
    latestEvidence.nativeSetWindowPosActivationGeometryStyleVisibilityUnchanged !==
      true ||
    latestEvidence.nativeSetWindowPosActivationWindowPosChangedSizeMoveAbsent !==
      true ||
    latestEvidence.nativeSetWindowPosActivationConsumer !==
      "bool-overwritten-by-immediate-helper-call" ||
    latestEvidence.nativeSetWindowPosActivationExecutableHashUnchanged !==
      true ||
    latestEvidence.nativeSetWindowPosResizePrimaryTraceBytes !== 79_078 ||
    latestEvidence.nativeSetWindowPosResizePrimaryTraceLines !== 92 ||
    latestEvidence.nativeSetWindowPosResizePrimaryTraceSha256 !==
      "311da5561cecb67836cfdce773cfca084e3388aa0394d19c6e5c05178b6bf080" ||
    latestEvidence.nativeSetWindowPosResizePrimarySummaryBytes !== 15_755 ||
    latestEvidence.nativeSetWindowPosResizePrimarySummarySha256 !==
      "e67cce9f835c65816c5a81f9d2bfa85c9772b644670c24280d5a2f9ef66ebc45" ||
    latestEvidence.nativeSetWindowPosResizePrimaryBalancedApiPairs !== 9 ||
    latestEvidence.nativeSetWindowPosResizeConfirmationTraceBytes !== 79_365 ||
    latestEvidence.nativeSetWindowPosResizeConfirmationTraceLines !== 93 ||
    latestEvidence.nativeSetWindowPosResizeConfirmationTraceSha256 !==
      "259a6669030c8e2cd8203a2dbb0b423ba9760115c0d81de68f343c29fba1a555" ||
    latestEvidence.nativeSetWindowPosResizeConfirmationSummaryBytes !==
      15_929 ||
    latestEvidence.nativeSetWindowPosResizeConfirmationSummarySha256 !==
      "fba73c1f2384a8bc340e29982b3625ac43d9c0406818a95c28053af15f21fa1b" ||
    latestEvidence.nativeSetWindowPosResizeConfirmationBalancedApiPairs !== 9 ||
    latestEvidence.nativeSetWindowPosResizeTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowPosResizeProfile !==
      "x86-bgi-first-set-window-pos-resize-v1" ||
    latestEvidence.nativeSetWindowPosResizeCallsPerTrace !== 1 ||
    latestEvidence.nativeSetWindowPosResizeCallerRva !== "0x0005cb4b" ||
    latestEvidence.nativeSetWindowPosResizeArguments?.join(",") !==
      "window,hwnd-notopmost,0,0,1286,749,0x0000010a" ||
    latestEvidence.nativeSetWindowPosResizeResult !== 1 ||
    latestEvidence.nativeSetWindowPosResizeEntryLastError !== 0 ||
    latestEvidence.nativeSetWindowPosResizeExitLastError !== 0 ||
    latestEvidence.nativeSetWindowPosResizeInvalidWindowResult !== 0 ||
    latestEvidence.nativeSetWindowPosResizeInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeSetWindowPosResizeBeforeGeometry?.join(",") !==
      "15360,8640,806,629,15363,8666,800,600" ||
    latestEvidence.nativeSetWindowPosResizeAfterGeometry?.join(",") !==
      "15360,8640,1286,749,15363,8666,1280,720" ||
    latestEvidence.nativeSetWindowPosResizeCallbackMessages?.join(",") !==
      "70,131,5,71" ||
    latestEvidence.nativeSetWindowPosResizeCallbackResults?.join(",") !==
      "0,0,0,0" ||
    latestEvidence.nativeSetWindowPosResizeNccalcInputRects?.join(",") !==
      "15360,8640,16646,9389,15360,8640,16166,9269,15363,8666,16163,9266" ||
    latestEvidence.nativeSetWindowPosResizeNccalcOutputFirstRect?.join(",") !==
      "15363,8666,16643,9386" ||
    latestEvidence.nativeSetWindowPosResizeNccalcWindowPosFlags !==
      "0x0000190e" ||
    latestEvidence.nativeSetWindowPosResizeChangedFlags !== "0x0000110e" ||
    latestEvidence.nativeSetWindowPosResizeOldGeometryThroughNccalc !== true ||
    latestEvidence.nativeSetWindowPosResizeNewGeometryAtSize !== true ||
    latestEvidence.nativeSetWindowPosResizeMoveMessageAbsent !== true ||
    latestEvidence.nativeSetWindowPosResizeRelationsStyleVisibilityPreserved !==
      true ||
    latestEvidence.nativeSetWindowPosResizeConsumer !==
      "bool-overwritten-by-immediate-helper-call" ||
    latestEvidence.nativeSetWindowPosResizeExecutableHashUnchanged !== true ||
    latestEvidence.nativeSystemMenuPrimaryTraceBytes !== 2_257_147 ||
    latestEvidence.nativeSystemMenuPrimaryTraceLines !== 7_048 ||
    latestEvidence.nativeSystemMenuPrimaryTraceSha256 !==
      "baab2b367da0083c0d3b9e61e0022657ff72216d0e037ff1ee2a5a73ffbc2283" ||
    latestEvidence.nativeSystemMenuPrimarySummaryBytes !== 52_845 ||
    latestEvidence.nativeSystemMenuPrimarySummarySha256 !==
      "a419c9fb267e088878cc5d3160c2c60d49a5be9bc1e1fa8b61b3d0e3a11cf7c2" ||
    latestEvidence.nativeSystemMenuPrimaryBalancedApiPairs !== 3_487 ||
    latestEvidence.nativeSystemMenuConfirmationTraceBytes !== 2_248_758 ||
    latestEvidence.nativeSystemMenuConfirmationTraceLines !== 7_043 ||
    latestEvidence.nativeSystemMenuConfirmationTraceSha256 !==
      "7b43593011cf2835a02c2574ae20fa84d71fc4f283829c8ed66665f2aeafcf57" ||
    latestEvidence.nativeSystemMenuConfirmationSummaryBytes !== 53_083 ||
    latestEvidence.nativeSystemMenuConfirmationSummarySha256 !==
      "a48772b3f5d11f420bd3cc8ae014e150ab969d2175280be4d85a7d60b75ebfb3" ||
    latestEvidence.nativeSystemMenuConfirmationBalancedApiPairs !== 3_484 ||
    latestEvidence.nativeSystemMenuTraceDiagnostics !== 0 ||
    latestEvidence.nativeSystemMenuProfile !==
      "x86-bgi-live-window-system-menu-close-item-enabled-v1" ||
    latestEvidence.nativeSystemMenuGetCallsPerTrace !== 2 ||
    latestEvidence.nativeSystemMenuEnableCallsPerTrace !== 2 ||
    latestEvidence.nativeSystemMenuCallerRvas?.join(",") !==
      "0x0009274c,0x00092753" ||
    latestEvidence.nativeSystemMenuInitialNullGetResult !== "0x00000000" ||
    latestEvidence.nativeSystemMenuInitialNullGetLastError !== 1_400 ||
    latestEvidence.nativeSystemMenuInitialNullEnableResult !== "0xffffffff" ||
    latestEvidence.nativeSystemMenuInitialNullEnableLastError !== 1_401 ||
    latestEvidence.nativeSystemMenuLiveGetResultNonzero !== true ||
    latestEvidence.nativeSystemMenuLiveGetLastError !== 0 ||
    latestEvidence.nativeSystemMenuLiveEnableItem !== "0x0000f060" ||
    latestEvidence.nativeSystemMenuLiveEnableFlags !== 0 ||
    latestEvidence.nativeSystemMenuLiveEnablePreviousState !== 0 ||
    latestEvidence.nativeSystemMenuLiveEnableLastError !== 0 ||
    latestEvidence.nativeSystemMenuLiveHandleRelation !==
      "nonzero-get-result-passed-unchanged-to-enable" ||
    latestEvidence.nativeSystemMenuNativeHandlesArtifactOnly !== true ||
    latestEvidence.nativeSystemMenuConsumer !==
      "enable-result-not-branch-consumed-before-global-state-write" ||
    latestEvidence.nativeSystemMenuExecutableHashUnchanged !== true ||
    latestEvidence.setWindowPosImplemented !== true ||
    latestEvidence.setWindowPosCallCount !== setWindowPosCallCount ||
    latestEvidence.setWindowPosSequences?.join(",") !== setWindowPosSequences ||
    latestEvidence.setWindowPosInstructionCounts?.join(",") !==
      setWindowPosInstructionCounts ||
    latestEvidence.setWindowPosReturnAddresses?.join(",") !==
      setWindowPosReturnAddresses ||
    latestEvidence.setWindowPosResults?.join(",") !== setWindowPosResults ||
    latestEvidence.setWindowPosLastErrors?.join(",") !==
      setWindowPosLastErrors ||
    latestEvidence.setWindowPosGuestWrites !== 0 ||
    latestEvidence.setWindowPosImplementationId !==
      setWindowPosImplementationId ||
    latestEvidence.setWindowPosStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.setWindowPosWindowId !== 1 ||
    latestEvidence.setWindowPosWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosOwnerThreadId !== 8_192 ||
    latestEvidence.setWindowPosSmallIconId !== 2 ||
    latestEvidence.setWindowPosSmallIconHandle !== "0x49010002" ||
    setWindowPosShowGeometry.x !== 15_360 ||
    setWindowPosShowGeometry.y !== 8_640 ||
    setWindowPosShowGeometry.width !== 806 ||
    setWindowPosShowGeometry.height !== 629 ||
    setWindowPosShowGeometry.clientX !== 15_363 ||
    setWindowPosShowGeometry.clientY !== 8_666 ||
    setWindowPosShowGeometry.clientWidth !== 800 ||
    setWindowPosShowGeometry.clientHeight !== 600 ||
    latestEvidence.setWindowPosFinalVisible !== true ||
    latestEvidence.setWindowPosFinalTopmost !== false ||
    latestEvidence.setWindowPosCallbackFrameIds?.join(",") !== "8,9,10,11" ||
    latestEvidence.setWindowPosCallbackMessages?.join(",") !== "71,3,127,71" ||
    latestEvidence.setWindowPosCallbackResults?.join(",") !==
      "0,0,0x49010002,0" ||
    latestEvidence.setWindowPosCallbackReturnInstructionCounts?.join(",") !==
      "11476718,11476705,11476787,11476850" ||
    latestEvidence.setWindowPosMaximumCallbackDepth !== 2 ||
    latestEvidence.setWindowPosSameDepthReplacement !== true ||
    latestEvidence.setWindowPosActivationCallbackFrameIds?.join(",") !==
      "12,13,14,15,16,17,18,19" ||
    latestEvidence.setWindowPosActivationCallbackMessages?.join(",") !==
      "70,70,28,134,6,641,642,7" ||
    latestEvidence.setWindowPosActivationCallbackResults?.join(",") !==
      "0,0,0,1,0,0,0,0" ||
    latestEvidence.setWindowPosActivationCallbackReturnInstructionCounts?.join(
      ",",
    ) !==
      "11806700,11806755,11806808,11806864,11807100,11807034,11807021,11807087" ||
    latestEvidence.setWindowPosActivationMaximumCallbackDepth !== 3 ||
    latestEvidence.setWindowPosActivationReplacementCount !== 5 ||
    latestEvidence.setWindowPosActivationGeometryStyleVisibilityUnchanged !==
      true ||
    latestEvidence.setWindowPosActivationActiveWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosActivationFocusWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosActivationForegroundWindowHandle !==
      "0x48010001" ||
    latestEvidence.setWindowPosActivationHostZOrderIdentityImported !== false ||
    latestEvidence.setWindowPosActivationExternalThreadIdentityImported !==
      false ||
    latestEvidence.setWindowPosResizeReached !== true ||
    latestEvidence.setWindowPosResizeOuterCompletionPending !== false ||
    latestEvidence.setWindowPosResizeOuterCompletionResult !== 1 ||
    latestEvidence.setWindowPosResizeOuterCompletionLastError !== 0 ||
    latestEvidence.setWindowPosResizeSequence !== 89_387 ||
    latestEvidence.setWindowPosResizeOccurrence !== 4 ||
    latestEvidence.setWindowPosResizeInstructionCount !== 15_951_782 ||
    latestEvidence.setWindowPosResizeArguments?.join(",") !==
      "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000506,0x000002ed,0x0000010a" ||
    latestEvidence.setWindowPosResizeReturnAddress !== "0x1005cb4b" ||
    latestEvidence.setWindowPosResizeImplementationId !==
      setWindowPosImplementationId ||
    latestEvidence.setWindowPosResizePolicySchemaVersion !== 23 ||
    latestEvidence.setWindowPosResizePolicyId !==
      "user32-single-monitor-window-frame-change-show-update-paint-v26" ||
    latestEvidence.setWindowPosResizeProfileId !==
      "initial-offscreen-show-hide-same-geometry-activation-resize-frame-change-move-v4" ||
    latestEvidence.setWindowPosResizeCallbackFrameIds?.join(",") !==
      "21,22,23,24" ||
    latestEvidence.setWindowPosResizeCallbackMessages?.join(",") !==
      "70,131,5,71" ||
    latestEvidence.setWindowPosResizeCompletedCallbackResults?.join(",") !==
      "0,0,0,0" ||
    latestEvidence.setWindowPosResizeCallbackReturnInstructionCounts?.join(
      ",",
    ) !== "15951884,15951937,15952027,15952080" ||
    latestEvidence.setWindowPosResizeActiveCallbackDepth !== 0 ||
    latestEvidence.setWindowPosResizeScratchAddress !== "0x00501000" ||
    latestEvidence.setWindowPosResizeScratchBytes !== 80 ||
    latestEvidence.setWindowPosResizeScratchWindowPosPointerOffset !== 48 ||
    latestEvidence.setWindowPosResizeScratchWindowPosTargetOffset !== 52 ||
    latestEvidence.setWindowPosResizeNccalcOutputHex !==
      "033c0000da21000003410000aa240000003c0000c0210000263f000035240000033c0000da210000233f0000322400003410500001000148feffffff003c0000c021000006050000ed0200000e190000" ||
    latestEvidence.setWindowPosResizeCommittedGeometry?.join(",") !==
      "15360,8640,1286,749,15363,8666,1280,720" ||
    latestEvidence.setWindowPosResizeActiveWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosResizeFocusWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosResizeForegroundWindowHandle !== "0x48010001" ||
    latestEvidence.setWindowPosResizeGdiSurfaceIdentityPreserved !== true ||
    latestEvidence.setWindowPosResizeGdiFillHistoryCleared !== true ||
    latestEvidence.setWindowPosResizeNocopybitsApplied !== true ||
    latestEvidence.setWindowPosResizeUser32GdiCommitPoint !==
      "after-wm-nccalcsize-before-wm-size" ||
    latestEvidence.setWindowPosResizeLiveWindowDcAtomicRejectionVerified !==
      true ||
    latestEvidence.liveSystemMenuCompletions !== 2 ||
    latestEvidence.liveSystemMenuSequences?.join(",") !== "89390,89391" ||
    latestEvidence.liveSystemMenuInstructionCounts?.join(",") !==
      "15952000,15952003" ||
    latestEvidence.liveGetSystemMenuArguments?.join(",") !==
      "0x48010001,0x00000000" ||
    latestEvidence.liveGetSystemMenuReturnAddress !== "0x1009274c" ||
    latestEvidence.liveGetSystemMenuResult !== "0x4e010001" ||
    latestEvidence.liveGetSystemMenuLastError !== 0 ||
    latestEvidence.liveGetSystemMenuGuestWrites !== 0 ||
    latestEvidence.liveGetSystemMenuImplementationId !==
      "user32.get-system-menu.logical-system-menu.v2" ||
    latestEvidence.liveEnableMenuItemArguments?.join(",") !==
      "0x4e010001,0x0000f060,0x00000000" ||
    latestEvidence.liveEnableMenuItemReturnAddress !== "0x10092753" ||
    latestEvidence.liveEnableMenuItemResult !== 0 ||
    latestEvidence.liveEnableMenuItemLastError !== 0 ||
    latestEvidence.liveEnableMenuItemGuestWrites !== 0 ||
    latestEvidence.liveEnableMenuItemImplementationId !==
      "user32.enable-menu-item.logical-system-menu.v2" ||
    latestEvidence.systemMenuStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.systemMenuHandleNamespace !== "vine.logical-menu.v1" ||
    latestEvidence.systemMenuOwnerWindowHandle !== "0x48010001" ||
    latestEvidence.systemMenuCloseItemCommand !== "0x0000f060" ||
    latestEvidence.systemMenuCloseItemEnabled !== true ||
    latestEvidence.systemMenuPreviousCloseItemState !== 0 ||
    latestEvidence.systemMenuMutation !==
      "separate-menu-generation-plan-stale-check-targeted-commit" ||
    latestEvidence.systemMenuWindowMutationGenerationIndependent !== true ||
    latestEvidence.systemMenuRepeatedGetReusesIdentity !== true ||
    latestEvidence.systemMenuIdempotentEnablePreservesGeneration !== true ||
    latestEvidence.systemMenuFailureAtomicity !== true ||
    latestEvidence.systemMenuProcessExitRetiresIdentity !== true ||
    latestEvidence.systemMenuHostIdentityImported !== false ||
    latestEvidence.systemMenuBrowserIdentityImported !== false ||
    latestEvidence.systemMenuBrowserResourceCreated !== false ||
    latestEvidence.systemMenuNodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.setWindowPosBoundedPrimaryResizeImplemented !== true ||
    latestEvidence.setWindowPosGeneralResizeImplemented !== false ||
    latestEvidence.setWindowPosGeneralZOrderImplemented !== false ||
    latestEvidence.setWindowPosMultiWindowActivationImplemented !== false ||
    latestEvidence.setWindowPosFailureAtomicity !== true ||
    latestEvidence.setWindowPosBrowserOwnsLogicalWindowState !== false ||
    latestEvidence.nativeCriticalSectionCapacityTraceBytes !== 66_177_215 ||
    latestEvidence.nativeCriticalSectionCapacityTraceLines !== 116_186 ||
    latestEvidence.nativeCriticalSectionCapacityTraceSha256 !==
      "119b818f617206c9abcb63c2777279aaf8596f1b522b094c99b5f65960358b45" ||
    latestEvidence.nativeCriticalSectionCapacityProfile !==
      "bgi-main-module-live-critical-section-capacity-v1" ||
    latestEvidence.nativeCriticalSectionCapacityDurationMilliseconds !==
      10_000 ||
    latestEvidence.nativeCriticalSectionCapacitySpinCountCalls !== 17 ||
    latestEvidence.nativeCriticalSectionCapacityPlainCalls !== 16_517 ||
    latestEvidence.nativeCriticalSectionCapacityDeleteCalls !== 0 ||
    latestEvidence.nativeCriticalSectionCapacityEnterCalls !== 4_156 ||
    latestEvidence.nativeCriticalSectionCapacityLeaveCalls !== 4_066 ||
    latestEvidence.nativeCriticalSectionCapacityMaximumLive !== 16_534 ||
    latestEvidence.nativeCriticalSectionCapacityFinalLive !== 16_534 ||
    latestEvidence.nativeCriticalSectionCapacityLastInitializeAfterModuleLoadMilliseconds !==
      8_966 ||
    latestEvidence.nativeCriticalSectionCapacityRepeatMaximumLive?.join(",") !==
      "5325,5147" ||
    latestEvidence.nativeCriticalSectionCapacityExecutableHashUnchanged !==
      true ||
    latestEvidence.criticalSectionCapacityLimit !== 32_768 ||
    latestEvidence.criticalSectionNodeLiveCount !== 16_408 ||
    latestEvidence.criticalSectionNodeSpinCountCalls !== 17 ||
    latestEvidence.criticalSectionNodePlainCalls !== 16_391 ||
    latestEvidence.criticalSectionNodeRemainingCapacity !== 16_360 ||
    latestEvidence.criticalSectionCapacityBounded !== true ||
    latestEvidence.criticalSectionCapacityFailureAtomicity !== true
  ) {
    errors.push(
      "latest SetWindowPos and critical-section capacity evidence is invalid",
    );
  }
  if (setWindowPosRepeatActivationActive) {
    for (const field of [
      "nativeSetWindowPosRepeatPrimaryTraceSha256",
      "nativeSetWindowPosRepeatPrimarySummarySha256",
      "nativeSetWindowPosRepeatConfirmationTraceSha256",
      "nativeSetWindowPosRepeatConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeSetWindowPosRepeatPrimaryTraceBytes !== 142_214 ||
      latestEvidence.nativeSetWindowPosRepeatPrimaryTraceLines !== 134 ||
      latestEvidence.nativeSetWindowPosRepeatPrimaryTraceSha256 !==
        "9953a39947b5ae2d9654f96fb79521598bbf4b51c7701e54e8fbf48b2af10b36" ||
      latestEvidence.nativeSetWindowPosRepeatPrimarySummaryBytes !== 15_736 ||
      latestEvidence.nativeSetWindowPosRepeatPrimarySummarySha256 !==
        "43a173207c33f05e8cb9f2f03d66bd06a1969dc57a157eb55590b6c91c5ad940" ||
      latestEvidence.nativeSetWindowPosRepeatPrimaryBalancedApiPairs !== 29 ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationTraceBytes !==
        142_590 ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationTraceLines !== 135 ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationTraceSha256 !==
        "4ae8efe2ebaa6512ecaf394e070bebdd652b5d21721f0dceb3721de247513208" ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationSummaryBytes !==
        15_911 ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationSummarySha256 !==
        "a9cadc23fc4f9a20c1682e161c5b31c646d48280705048c0405433805e786bfc" ||
      latestEvidence.nativeSetWindowPosRepeatConfirmationBalancedApiPairs !==
        29 ||
      latestEvidence.nativeSetWindowPosRepeatTraceDiagnostics !== 0 ||
      latestEvidence.nativeSetWindowPosRepeatProfile !==
        "x86-bgi-set-window-pos-repeat-activation-v1" ||
      latestEvidence.nativeSetWindowPosRepeatCallsPerTrace !== 1 ||
      latestEvidence.nativeSetWindowPosRepeatCallerRva !== "0x0005cb4b" ||
      latestEvidence.nativeSetWindowPosRepeatArguments?.join(",") !==
        "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000506,0x000002ed,0x0000010a" ||
      latestEvidence.nativeSetWindowPosRepeatResult !== 1 ||
      latestEvidence.nativeSetWindowPosRepeatEntryLastError !== 0 ||
      latestEvidence.nativeSetWindowPosRepeatExitLastError !== 0 ||
      latestEvidence.nativeSetWindowPosRepeatGeometryUnchanged !== true ||
      latestEvidence.nativeSetWindowPosRepeatRelationsRemainTarget !== true ||
      latestEvidence.nativeSetWindowPosRepeatCallbackMessages?.join(",") !==
        "70" ||
      latestEvidence.nativeSetWindowPosRepeatCallbackResults?.join(",") !==
        "0" ||
      latestEvidence.nativeSetWindowPosRepeatActivationMessagesAbsent !==
        true ||
      latestEvidence.nativeSetWindowPosRepeatResizeMessagesAbsent !== true ||
      latestEvidence.nativeSetWindowPosRepeatConsumer !==
        "bool-overwritten-by-immediate-helper-call" ||
      latestEvidence.nativeSetWindowPosRepeatExecutableHashUnchanged !== true ||
      latestEvidence.setWindowPosRepeatImplemented !== true ||
      latestEvidence.setWindowPosRepeatSequence !== 108_718 ||
      latestEvidence.setWindowPosRepeatOccurrence !== 5 ||
      latestEvidence.setWindowPosRepeatInstructionCount !== 28_273_977 ||
      latestEvidence.setWindowPosRepeatArguments?.join(",") !==
        "0x48010001,0xfffffffe,0x00000000,0x00000000,0x00000506,0x000002ed,0x0000010a" ||
      latestEvidence.setWindowPosRepeatReturnAddress !== "0x1005cb4b" ||
      latestEvidence.setWindowPosRepeatResult !== 1 ||
      latestEvidence.setWindowPosRepeatLastError !== 0 ||
      latestEvidence.setWindowPosRepeatGuestWriteBytes !== 0 ||
      latestEvidence.setWindowPosRepeatImplementationId !==
        "user32.set-window-pos.v5" ||
      latestEvidence.setWindowPosRepeatCallbackFrameId !== 25 ||
      latestEvidence.setWindowPosRepeatCallbackTarget !== "0x10092550" ||
      latestEvidence.setWindowPosRepeatCallbackAllocationStart !==
        "0x006ff5cc" ||
      latestEvidence.setWindowPosRepeatScratchAddress !== "0x00501000" ||
      latestEvidence.setWindowPosRepeatScratchBytes !== 28 ||
      latestEvidence.setWindowPosRepeatCallbackFrameBytes !== 20 ||
      latestEvidence.setWindowPosRepeatCallbackReturnSentinel !==
        "0x7f010000" ||
      latestEvidence.setWindowPosRepeatCallbackMessage !== 70 ||
      latestEvidence.setWindowPosRepeatCallbackResult !== 0 ||
      latestEvidence.setWindowPosRepeatCallbackReturnInstructionCount !==
        28_274_079 ||
      latestEvidence.setWindowPosRepeatGeometry?.join(",") !==
        "15360,8640,1286,749,15363,8666,1280,720" ||
      latestEvidence.setWindowPosRepeatActiveWindowHandle !== "0x48010001" ||
      latestEvidence.setWindowPosRepeatFocusWindowHandle !== "0x48010001" ||
      latestEvidence.setWindowPosRepeatForegroundWindowHandle !==
        "0x48010001" ||
      latestEvidence.setWindowPosRepeatWindowAuthority !==
        "rust-runtime-user32-logical-window" ||
      latestEvidence.setWindowPosRepeatRelationAuthority !==
        "rust-runtime-user32-thread-and-process-window-relations" ||
      latestEvidence.setWindowPosRepeatCallbackDisposition !==
        "validate-only" ||
      latestEvidence.setWindowPosRepeatMutation !== false ||
      latestEvidence.setWindowPosRepeatFailureAtomicity !== true ||
      latestEvidence.setWindowPosRepeatBrowserOwnsLogicalWindowState !==
        false ||
      latestEvidence.setWindowPosRepeatBrowserResourceCreated !== false ||
      latestEvidence.setWindowPosRepeatNodeCompletions !== 1 ||
      latestEvidence.setWindowPosRepeatChromeCompletions !== 1 ||
      latestEvidence.setWindowPosRepeatNodeChromeOrderedSemanticsMatched !==
        true
    ) {
      errors.push("latest repeated SetWindowPos evidence is invalid");
    }
  }
  const nativeMonitorFromWindowInvalidWindowFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowInvalidWindowFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowInvalidWindowFlags",
    errors,
  );
  const nativeMonitorFromWindowValidTargets = requireStringArray(
    latestEvidence.nativeMonitorFromWindowValidTargets,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowValidTargets",
    errors,
  );
  const nativeMonitorFromWindowValidFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowValidFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowValidFlags",
    errors,
  );
  const nativeMonitorFromWindowNullFlags = requireIntegerArray(
    latestEvidence.nativeMonitorFromWindowNullFlags,
    "activeRuntimeCheckpoint.latestEvidence.nativeMonitorFromWindowNullFlags",
    errors,
  );
  if (
    latestEvidence.nativeMonitorFromWindowOracleArchitecture !== "coff-i386" ||
    latestEvidence.nativeMonitorFromWindowOracleSourceBytes !== 1_920 ||
    latestEvidence.nativeMonitorFromWindowOracleSourceSha256 !==
      "9443dcc8744ff0a39ee22485f7ad3f15a3413a1c51306361f117605bb82d271a" ||
    latestEvidence.nativeMonitorFromWindowOracleExecutableBytes !== 78_848 ||
    latestEvidence.nativeMonitorFromWindowOracleExecutableSha256 !==
      "c499b514839719a8536b663b02f09d98ed098c66cc27b6a8bd8979f53e1cdda9" ||
    latestEvidence.nativeMonitorFromWindowSentinelLastError !== 23_130 ||
    nativeMonitorFromWindowInvalidWindowFlags.join(",") !== "0,1,2" ||
    latestEvidence.nativeMonitorFromWindowInvalidWindowResult !==
      "0x00000000" ||
    latestEvidence.nativeMonitorFromWindowInvalidWindowLastError !== 1_400 ||
    nativeMonitorFromWindowValidTargets.join(",") !==
      "hidden-window,desktop-window" ||
    nativeMonitorFromWindowValidFlags.join(",") !== "0,1,2" ||
    latestEvidence.nativeMonitorFromWindowValidResultsNonNull !== true ||
    latestEvidence.nativeMonitorFromWindowValidIdentityShared !== true ||
    latestEvidence.nativeMonitorFromWindowValidLastErrorPreserved !== true ||
    latestEvidence.nativeMonitorFromWindowInvalidFlags !== 3 ||
    latestEvidence.nativeMonitorFromWindowInvalidFlagsResult !== "0x00000000" ||
    latestEvidence.nativeMonitorFromWindowInvalidFlagsLastError !== 1_004 ||
    nativeMonitorFromWindowNullFlags.join(",") !== "0,1,2" ||
    JSON.stringify(latestEvidence.nativeMonitorFromWindowNullResultsNonNull) !==
      JSON.stringify([false, true, true]) ||
    latestEvidence.nativeMonitorFromWindowNullLastErrorPreserved !== true ||
    latestEvidence.nativeMonitorFromWindowNullOutsideReachedContract !== true ||
    latestEvidence.nativeMonitorFromWindowCallerReturnRva !== "0x00059d19" ||
    latestEvidence.nativeMonitorFromWindowReachedFlags !== 1 ||
    latestEvidence.nativeMonitorFromWindowConsumer !==
      "return-hmonitor-store-ebx-compare-with-idirect3d9-get-adapter-monitor" ||
    latestEvidence.nativeMonitorFromWindowExecutableHashUnchanged !== true ||
    latestEvidence.monitorFromWindowImplemented !== true ||
    latestEvidence.monitorFromWindowSequence !== 17_842 ||
    latestEvidence.monitorFromWindowInstructionCount !== 11_807_189 ||
    latestEvidence.monitorFromWindowArguments?.join(",") !==
      "0x48010001,0x00000001" ||
    latestEvidence.monitorFromWindowReturnAddress !== "0x10059d19" ||
    latestEvidence.monitorFromWindowResult !== "0x4f010001" ||
    latestEvidence.monitorFromWindowLastError !== 0 ||
    latestEvidence.monitorFromWindowGuestWrites !== 0 ||
    latestEvidence.monitorFromWindowImplementationId !==
      "user32.monitor-from-window.v1" ||
    latestEvidence.monitorFromWindowPolicyId !==
      "live-window-default-to-primary-v1" ||
    latestEvidence.monitorFromWindowStateAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.monitorFromWindowWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.monitorFromWindowWindowHandle !== "0x48010001" ||
    latestEvidence.monitorFromWindowMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    latestEvidence.monitorFromWindowPrimaryMonitorHandle !== "0x4f010001" ||
    latestEvidence.monitorFromWindowDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.monitorFromWindowFlags !== 1 ||
    latestEvidence.monitorFromWindowInvalidWindowResult !== "0x00000000" ||
    latestEvidence.monitorFromWindowInvalidWindowLastError !== 1_400 ||
    latestEvidence.monitorFromWindowUnsupportedFlagsFault !==
      "monitor-from-window-flags-unsupported" ||
    latestEvidence.monitorFromWindowFailureAtomicity !== true ||
    latestEvidence.monitorFromWindowHostDisplayTopologyImported !== false ||
    latestEvidence.monitorFromWindowBrowserDisplayTopologyImported !== false
  ) {
    errors.push("latest MonitorFromWindow evidence is invalid");
  }
  if (
    latestEvidence.nativeD3d9GetAdapterCountTraceBytes !== 2_267_972 ||
    latestEvidence.nativeD3d9GetAdapterCountTraceLines !== 7_133 ||
    latestEvidence.nativeD3d9GetAdapterCountTraceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    latestEvidence.nativeD3d9GetAdapterCountSummaryBytes !== 58_021 ||
    latestEvidence.nativeD3d9GetAdapterCountSummarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    latestEvidence.nativeD3d9GetAdapterCountTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetAdapterCountCallsPerTrace !== 6 ||
    latestEvidence.nativeD3d9GetAdapterCountCallerRva !== "0x00059d36" ||
    latestEvidence.nativeD3d9GetAdapterCountResult !== 1 ||
    latestEvidence.nativeD3d9GetAdapterCountLastError !== 0 ||
    latestEvidence.nativeD3d9GetAdapterCountConsumer !==
      "store-count-in-edi-loop-adapter-indices-and-compare-monitor-identity" ||
    latestEvidence.nativeD3d9GetAdapterCountExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterCountImplemented !== true ||
    latestEvidence.d3d9GetAdapterCountSequence !== 17_843 ||
    latestEvidence.d3d9GetAdapterCountInstructionCount !== 11_807_197 ||
    latestEvidence.d3d9GetAdapterCountArguments?.join(",") !== "0x01000004" ||
    latestEvidence.d3d9GetAdapterCountReturnAddress !== "0x10059d36" ||
    latestEvidence.d3d9GetAdapterCountResult !== 1 ||
    latestEvidence.d3d9GetAdapterCountLastError !== 0 ||
    latestEvidence.d3d9GetAdapterCountGuestWriteBytes !== 0 ||
    latestEvidence.d3d9GetAdapterCountPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterCountPolicyVersion !== 22 ||
    latestEvidence.d3d9GetAdapterCountPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetAdapterCountProfileId !==
      "single-logical-adapter-v1" ||
    latestEvidence.d3d9GetAdapterCountDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetAdapterCountCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterCountDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterCountReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterCountReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterCountFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterCountBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterCountRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterCountRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterCountHostGpuTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterCountBrowserGpuTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterCountNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterCountChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterCountNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 adapter-count evidence is invalid");
  }
  const nativeD3d9GetAdapterMonitorOracleAdapters = requireIntegerArray(
    latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapters,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapters",
    errors,
  );
  const d3d9GetAdapterMonitorSequences = requireIntegerArray(
    latestEvidence.d3d9GetAdapterMonitorSequences,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterMonitorSequences",
    errors,
  );
  const d3d9GetAdapterMonitorInstructionCounts = requireIntegerArray(
    latestEvidence.d3d9GetAdapterMonitorInstructionCounts,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterMonitorInstructionCounts",
    errors,
  );
  if (
    latestEvidence.nativeD3d9GetAdapterMonitorTraceBytes !== 2_267_972 ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceLines !== 7_133 ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceSha256 !==
      "cfefd83bc32f2de6e8b7feda96a09240b2ac7aedb3bc76ded41064a0d224c898" ||
    latestEvidence.nativeD3d9GetAdapterMonitorSummaryBytes !== 58_021 ||
    latestEvidence.nativeD3d9GetAdapterMonitorSummarySha256 !==
      "3e8f8b52e23131b8efea737e3dfa94bdb10d9f14526fe2bf44bdb56715959aa9" ||
    latestEvidence.nativeD3d9GetAdapterMonitorTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorCallsPerTrace !== 6 ||
    latestEvidence.nativeD3d9GetAdapterMonitorCallerRva !== "0x00059d5e" ||
    latestEvidence.nativeD3d9GetAdapterMonitorAdapter !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorResultOpaqueNonNull !== true ||
    latestEvidence.nativeD3d9GetAdapterMonitorLastError !== 0 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleArchitecture !==
      "coff-i386" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleSourceBytes !== 5_089 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleSourceSha256 !==
      "337ddf83ec68ba80a25d70f1f9a9d0361a83833657ca7c852075043d16b57c67" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleExecutableBytes !== 3_584 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleExecutableSha256 !==
      "1d2681032fad258d9930c1a4a88ac6d4cf67d53b7d26ff447a17d5129bb436d5" ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleAdapterCount !== 1 ||
    nativeD3d9GetAdapterMonitorOracleAdapters.join(",") !== "0,1,4294967295" ||
    JSON.stringify(
      latestEvidence.nativeD3d9GetAdapterMonitorOracleResultsNonNull,
    ) !== JSON.stringify([true, false, false]) ||
    latestEvidence.nativeD3d9GetAdapterMonitorOraclePrimaryIdentityStable !==
      true ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleLastErrorSentinel !==
      23_130 ||
    latestEvidence.nativeD3d9GetAdapterMonitorOracleLastErrorPreserved !==
      true ||
    latestEvidence.nativeD3d9GetAdapterMonitorConsumer !==
      "compare-result-with-monitor-from-window-primary-identity" ||
    latestEvidence.nativeD3d9GetAdapterMonitorExecutableHashUnchanged !==
      true ||
    latestEvidence.d3d9GetAdapterMonitorImplemented !== true ||
    d3d9GetAdapterMonitorSequences.join(",") !== "17844,17848" ||
    d3d9GetAdapterMonitorInstructionCounts.join(",") !== "11807216,11807374" ||
    JSON.stringify(latestEvidence.d3d9GetAdapterMonitorArguments) !==
      JSON.stringify([
        ["0x01000004", "0x00000000"],
        ["0x01000004", "0x00000000"],
      ]) ||
    latestEvidence.d3d9GetAdapterMonitorReturnAddresses?.join(",") !==
      "0x10059d5e,0x10059d5e" ||
    latestEvidence.d3d9GetAdapterMonitorResults?.join(",") !==
      "0x4f010001,0x4f010001" ||
    latestEvidence.d3d9GetAdapterMonitorLastError !== 0 ||
    latestEvidence.d3d9GetAdapterMonitorGuestWriteBytes !== 0 ||
    latestEvidence.d3d9GetAdapterMonitorPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterMonitorPolicyVersion !== 22 ||
    latestEvidence.d3d9GetAdapterMonitorPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetAdapterMonitorProfileId !==
      "primary-adapter-monitor-v1" ||
    latestEvidence.d3d9GetAdapterMonitorDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetAdapterMonitorAdapterMappingAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterMonitorIdentityAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    latestEvidence.d3d9GetAdapterMonitorPrimaryHandle !== "0x4f010001" ||
    latestEvidence.d3d9GetAdapterMonitorMatchesMonitorFromWindowIdentity !==
      true ||
    latestEvidence.d3d9GetAdapterMonitorReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterMonitorReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterMonitorFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterMonitorBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterMonitorRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterMonitorRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterMonitorHostIdentityImported !== false ||
    latestEvidence.d3d9GetAdapterMonitorBrowserTopologyImported !== false ||
    latestEvidence.d3d9GetAdapterMonitorNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterMonitorChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterMonitorNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest D3D9 adapter-monitor evidence is invalid");
  }
  if (
    latestEvidence.d3d9RootFactoryImplemented !== true ||
    latestEvidence.d3d9RootFactorySequence !== 17_816 ||
    latestEvidence.d3d9RootFactoryInstructionCount !== 11_806_445 ||
    latestEvidence.d3d9RootFactoryArguments?.join(",") !== "0x00000020" ||
    latestEvidence.d3d9RootFactoryReturnAddress !== "0x1005b892" ||
    latestEvidence.d3d9RootFactoryResult !== "0x01000004" ||
    latestEvidence.d3d9RootFactoryLastError !== 0 ||
    latestEvidence.d3d9RootFactoryGuestWrites !== 0 ||
    latestEvidence.d3d9RootFactoryImplementationId !==
      "d3d9.direct3d-create9.v1" ||
    latestEvidence.d3d9RootObjectId !== "1" ||
    latestEvidence.d3d9RootClassIdentity !== "d3d9.idirect3d9-root.v1" ||
    latestEvidence.d3d9RootReferenceCount !== 1 ||
    latestEvidence.d3d9RootCanonicalIunknownInterfaceId !== "1" ||
    latestEvidence.d3d9RootInterfaceId !== "2" ||
    latestEvidence.d3d9RootInterfaceName !== "IDirect3D9" ||
    latestEvidence.d3d9RootInterfacePointer !== "0x01000004" ||
    latestEvidence.d3d9RootInterfaceVtable !== "0x0100001c" ||
    latestEvidence.d3d9RootInterfaceSlots !== 17 ||
    latestEvidence.d3d9RootBackingAddressAllocationId !== 41 ||
    latestEvidence.d3d9RootBackingMemoryOwner !== 13 ||
    latestEvidence.d3d9RootBackingBase !== "0x01000000" ||
    latestEvidence.d3d9RootBackingBytes !== 8_192 ||
    latestEvidence.d3d9RootBackingMappingOperations !== 5 ||
    latestEvidence.d3d9RootBackingDirtyPages !== 2 ||
    latestEvidence.d3d9RootBackingFailureAtomicity !== true ||
    latestEvidence.d3d9RootBrowserOwnsComState !== false
  ) {
    errors.push("latest D3D9 root factory evidence is invalid");
  }
  requireSha256(
    latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256",
    errors,
  );
  requireSha256(
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256",
    errors,
  );
  const nativeD3d9DisplayModeCallerRvas = requireStringArray(
    latestEvidence.nativeD3d9DisplayModeCallerRvas,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DisplayModeCallerRvas",
    errors,
  );
  const d3d9GetAdapterDisplayModeArguments = requireStringArray(
    latestEvidence.d3d9GetAdapterDisplayModeArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterDisplayModeArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9DisplayModePrimaryTraceBytes !== 2_192_729 ||
    latestEvidence.nativeD3d9DisplayModePrimaryTraceLines !== 6_886 ||
    latestEvidence.nativeD3d9DisplayModePrimaryTraceSha256 !==
      "6b9bd17809ef235f1817fb08ccd4bfb92f27f3cd424bd0f15500470d73b05208" ||
    latestEvidence.nativeD3d9DisplayModePrimaryBalancedApiPairs !== 3_405 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceBytes !== 2_249_188 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9DisplayModeConfirmationTraceSha256 !==
      "fac93ab62effd0228dc8c8a88b617f3aced5fcff9429c9e90c9347979c660f92" ||
    latestEvidence.nativeD3d9DisplayModeConfirmationBalancedApiPairs !==
      3_496 ||
    latestEvidence.nativeD3d9DisplayModeTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9DisplayModeCallsPerTrace !== 4 ||
    nativeD3d9DisplayModeCallerRvas.join(",") !== "0x0005b8d6,0x00059da5" ||
    latestEvidence.nativeD3d9DisplayModeCallerCounts?.join(",") !== "1,3" ||
    latestEvidence.nativeD3d9DisplayModeAdapter !== 0 ||
    latestEvidence.nativeD3d9DisplayModeResult !== "0x00000000" ||
    latestEvidence.nativeD3d9DisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    latestEvidence.nativeD3d9DisplayModeFirstInputZeroed !== true ||
    latestEvidence.nativeD3d9DisplayModeFirstCallFullOverwrite !== true ||
    latestEvidence.nativeD3d9DisplayModeHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9DisplayModeAllFieldsConsumed !== true ||
    latestEvidence.nativeD3d9DisplayModeExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeImplemented !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeSequence !== 17_817 ||
    latestEvidence.d3d9GetAdapterDisplayModeInstructionCount !== 11_806_455 ||
    d3d9GetAdapterDisplayModeArguments.join(",") !==
      "0x01000004,0x00000000,0x101038e0" ||
    latestEvidence.d3d9GetAdapterDisplayModeReturnAddress !== "0x1005b8d6" ||
    latestEvidence.d3d9GetAdapterDisplayModeResult !== "0x00000000" ||
    latestEvidence.d3d9GetAdapterDisplayModeLastError !== 0 ||
    latestEvidence.d3d9GetAdapterDisplayModeGuestWriteBytes !== 16 ||
    latestEvidence.d3d9GetAdapterDisplayModeOutputHex !==
      "80070000380400003c00000016000000" ||
    latestEvidence.d3d9GetAdapterDisplayModeWidth !== 1_920 ||
    latestEvidence.d3d9GetAdapterDisplayModeHeight !== 1_080 ||
    latestEvidence.d3d9GetAdapterDisplayModeRefreshRate !== 60 ||
    latestEvidence.d3d9GetAdapterDisplayModeFormat !== 22 ||
    latestEvidence.d3d9GetAdapterDisplayModePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterDisplayModePolicyVersion !== 22 ||
    latestEvidence.d3d9GetAdapterDisplayModePolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetAdapterDisplayModeProfileId !==
      "primary-adapter-1920x1080-60-x8r8g8b8-v1" ||
    latestEvidence.d3d9GetAdapterDisplayModeDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetAdapterDisplayModeGeometryAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterDisplayModeReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterDisplayModeReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterDisplayModeBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterDisplayModeNodeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterDisplayModeChromeCompletions !== 2 ||
    latestEvidence.d3d9GetAdapterDisplayModeNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest D3D9 display-mode evidence is invalid");
  }
  for (const field of [
    "nativeD3d9AdapterIdentifierPrimaryTraceSha256",
    "nativeD3d9AdapterIdentifierConfirmationTraceSha256",
    "nativeD3d9AdapterIdentifierInputSha256",
    "nativeD3d9AdapterIdentifierOutputSha256",
    "d3d9GetAdapterIdentifierOutputSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const d3d9GetAdapterIdentifierArguments = requireOrderedStringArray(
    latestEvidence.d3d9GetAdapterIdentifierArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetAdapterIdentifierArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceBytes !== 2_207_106 ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceLines !== 6_888 ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryTraceSha256 !==
      "c635bde508a727e5c42dd0d17b75d3e82e30d62f77ad06bf497133cb4f8bd7d3" ||
    latestEvidence.nativeD3d9AdapterIdentifierPrimaryBalancedApiPairs !==
      3_406 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceBytes !==
      2_263_088 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceLines !==
      7_068 ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationTraceSha256 !==
      "d13217c0bc87789d61da46af87339e63bb0a92ac11dd406698a6b75982574da5" ||
    latestEvidence.nativeD3d9AdapterIdentifierConfirmationBalancedApiPairs !==
      3_496 ||
    latestEvidence.nativeD3d9AdapterIdentifierTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9AdapterIdentifierCallerRva !== "0x0005b91c" ||
    latestEvidence.nativeD3d9AdapterIdentifierAdapter !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierFlags !== 0 ||
    latestEvidence.nativeD3d9AdapterIdentifierResult !== "0x00000000" ||
    latestEvidence.nativeD3d9AdapterIdentifierInputZeroed !== true ||
    latestEvidence.nativeD3d9AdapterIdentifierInputSha256 !==
      "a805fd6f7ad225e289321ba6e2b079e92c3accc1675a7ba6626101da38f727f2" ||
    latestEvidence.nativeD3d9AdapterIdentifierOutputSha256 !==
      "773a5d417dceeab588a8724b951f9926543e39b42fad71b58d1ba6efb6a8497c" ||
    latestEvidence.nativeD3d9AdapterIdentifierChangedBytes !== 169 ||
    latestEvidence.nativeD3d9AdapterIdentifierHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9AdapterIdentifierOutputFieldsConsumed !== false ||
    latestEvidence.nativeD3d9AdapterIdentifierHostGpuIdentityImported !==
      false ||
    latestEvidence.nativeD3d9AdapterIdentifierExecutableHashUnchanged !==
      true ||
    latestEvidence.d3d9GetAdapterIdentifierImplemented !== true ||
    latestEvidence.d3d9GetAdapterIdentifierSequence !== 17_818 ||
    latestEvidence.d3d9GetAdapterIdentifierInstructionCount !== 11_806_466 ||
    d3d9GetAdapterIdentifierArguments.join(",") !==
      "0x01000004,0x00000000,0x00000000,0x10103410" ||
    latestEvidence.d3d9GetAdapterIdentifierReturnAddress !== "0x1005b91c" ||
    latestEvidence.d3d9GetAdapterIdentifierResult !== "0x00000000" ||
    latestEvidence.d3d9GetAdapterIdentifierLastError !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierGuestWriteBytes !== 1_100 ||
    latestEvidence.d3d9GetAdapterIdentifierOutputSha256 !==
      "84221a6160034deedbe815da29fafb40676dc23879aca0c585ff40ec2e4f11a4" ||
    latestEvidence.d3d9GetAdapterIdentifierDriver !== "vine-d3d9.dll" ||
    latestEvidence.d3d9GetAdapterIdentifierDescription !==
      "Vine Virtual Direct3D9 Adapter" ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceName !== "\\\\.\\DISPLAY1" ||
    latestEvidence.d3d9GetAdapterIdentifierDriverVersion !==
      "0x0001000000000000" ||
    latestEvidence.d3d9GetAdapterIdentifierVendorId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierSubSysId !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierRevision !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierDeviceIdentifier !==
      "56494e45-4433-4439-8000-000000000001" ||
    latestEvidence.d3d9GetAdapterIdentifierWhqlLevel !== 0 ||
    latestEvidence.d3d9GetAdapterIdentifierPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetAdapterIdentifierPolicyVersion !== 22 ||
    latestEvidence.d3d9GetAdapterIdentifierPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetAdapterIdentifierProfileId !==
      "primary-virtual-adapter-identifier-v1" ||
    latestEvidence.d3d9GetAdapterIdentifierDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetAdapterIdentifierIdentityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetAdapterIdentifierDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetAdapterIdentifierReferenceCount !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierReferenceCountMutation !== false ||
    latestEvidence.d3d9GetAdapterIdentifierFailureAtomicity !== true ||
    latestEvidence.d3d9GetAdapterIdentifierBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetAdapterIdentifierRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetAdapterIdentifierRendererResourceCreated !== false ||
    latestEvidence.d3d9GetAdapterIdentifierHostGpuIdentityImported !== false ||
    latestEvidence.d3d9GetAdapterIdentifierNodeCompletions !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierChromeCompletions !== 1 ||
    latestEvidence.d3d9GetAdapterIdentifierNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest D3D9 adapter-identifier evidence is invalid");
  }
  for (const field of [
    "nativeD3d9DeviceCapsPrimaryTraceSha256",
    "nativeD3d9DeviceCapsConfirmationTraceSha256",
    "nativeD3d9DeviceCapsOutputSha256",
    "d3d9GetDeviceCapsOutputSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9DeviceCapsConsumedFields = requireStringArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedFields,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedFields",
    errors,
  );
  const nativeD3d9DeviceCapsConsumedFieldOffsets = requireIntegerArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedFieldOffsets,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedFieldOffsets",
    errors,
  );
  const nativeD3d9DeviceCapsConsumedValues = requireStringArray(
    latestEvidence.nativeD3d9DeviceCapsConsumedValues,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9DeviceCapsConsumedValues",
    errors,
  );
  const d3d9GetDeviceCapsArguments = requireOrderedStringArray(
    latestEvidence.d3d9GetDeviceCapsArguments,
    "activeRuntimeCheckpoint.latestEvidence.d3d9GetDeviceCapsArguments",
    errors,
  );
  if (
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9DeviceCapsPrimaryBalancedApiPairs !== 3_495 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceBytes !== 2_282_110 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9DeviceCapsConfirmationBalancedApiPairs !== 3_496 ||
    latestEvidence.nativeD3d9DeviceCapsTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9DeviceCapsCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9DeviceCapsCallerRva !== "0x0005b949" ||
    latestEvidence.nativeD3d9DeviceCapsAdapter !== 0 ||
    latestEvidence.nativeD3d9DeviceCapsDeviceType !== 1 ||
    latestEvidence.nativeD3d9DeviceCapsResult !== "0x00000000" ||
    latestEvidence.nativeD3d9DeviceCapsOutputBytes !== 304 ||
    latestEvidence.nativeD3d9DeviceCapsOutputSha256 !==
      "5c48557292c6b254d6e9eb9612e3b24aae72f183bcbbcaf803d7a5ac16b41803" ||
    latestEvidence.nativeD3d9DeviceCapsOutputEqualsCallerStackPointerAfterReturn !==
      true ||
    latestEvidence.nativeD3d9DeviceCapsHresultSignConsumed !== true ||
    nativeD3d9DeviceCapsConsumedFields.join(",") !==
      "Caps,Caps2,PixelShaderVersion" ||
    nativeD3d9DeviceCapsConsumedFieldOffsets.join(",") !== "8,12,204" ||
    nativeD3d9DeviceCapsConsumedValues.join(",") !==
      "0x00020000,0xe0010000,0xffff0300" ||
    latestEvidence.nativeD3d9DeviceCapsConsumerInstructionCount !== 256 ||
    latestEvidence.nativeD3d9DeviceCapsHostGpuCapsImported !== false ||
    latestEvidence.nativeD3d9DeviceCapsExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetDeviceCapsImplemented !== true ||
    latestEvidence.d3d9GetDeviceCapsSequence !== 17_819 ||
    latestEvidence.d3d9GetDeviceCapsInstructionCount !== 11_806_478 ||
    d3d9GetDeviceCapsArguments.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x006ffe18" ||
    latestEvidence.d3d9GetDeviceCapsReturnAddress !== "0x1005b949" ||
    latestEvidence.d3d9GetDeviceCapsResult !== "0x00000000" ||
    latestEvidence.d3d9GetDeviceCapsLastError !== 0 ||
    latestEvidence.d3d9GetDeviceCapsGuestWriteBytes !== 304 ||
    latestEvidence.d3d9GetDeviceCapsOutputSha256 !==
      "ae29928ba206a3c56fdd5de4c266f68553c9e5e9057ba0d80425b6e60baff667" ||
    latestEvidence.d3d9GetDeviceCapsDeviceType !== 1 ||
    latestEvidence.d3d9GetDeviceCapsAdapterOrdinal !== 0 ||
    latestEvidence.d3d9GetDeviceCapsCaps !== "0x00020000" ||
    latestEvidence.d3d9GetDeviceCapsCaps2 !== "0x20000000" ||
    latestEvidence.d3d9GetDeviceCapsPixelShaderVersion !== "0xffff0300" ||
    latestEvidence.d3d9GetDeviceCapsNumberOfAdaptersInGroup !== 1 ||
    latestEvidence.d3d9GetDeviceCapsOtherFieldsZero !== true ||
    latestEvidence.d3d9GetDeviceCapsPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetDeviceCapsPolicyVersion !== 22 ||
    latestEvidence.d3d9GetDeviceCapsPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetDeviceCapsProfileId !==
      "primary-hal-readscanline-dynamic-textures-ps3-v1" ||
    latestEvidence.d3d9GetDeviceCapsDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetDeviceCapsCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetDeviceCapsDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9GetDeviceCapsReferenceCount !== 1 ||
    latestEvidence.d3d9GetDeviceCapsReferenceCountMutation !== false ||
    latestEvidence.d3d9GetDeviceCapsFailureAtomicity !== true ||
    latestEvidence.d3d9GetDeviceCapsBrowserOwnsComState !== false ||
    latestEvidence.d3d9GetDeviceCapsRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9GetDeviceCapsRendererResourceCreated !== false ||
    latestEvidence.d3d9GetDeviceCapsHostGpuCapsImported !== false ||
    latestEvidence.d3d9GetDeviceCapsNodeCompletions !== 1 ||
    latestEvidence.d3d9GetDeviceCapsChromeCompletions !== 1 ||
    latestEvidence.d3d9GetDeviceCapsNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 device-caps evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CheckDeviceTypePrimaryTraceSha256",
    "nativeD3d9CheckDeviceTypeConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9CheckDeviceTypeCallerRvas = requireStringArray(
    latestEvidence.nativeD3d9CheckDeviceTypeDirectCallerRvas,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceTypeDirectCallerRvas",
    errors,
  );
  const nativeD3d9CheckDeviceTypeWindowedValues = requireIntegerArray(
    latestEvidence.nativeD3d9CheckDeviceTypeDirectWindowedValues,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceTypeDirectWindowedValues",
    errors,
  );
  const d3d9CheckDeviceTypeSequences = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeSequences,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeSequences",
    errors,
  );
  const d3d9CheckDeviceTypeInstructionCounts = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeInstructionCounts,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeInstructionCounts",
    errors,
  );
  const d3d9CheckDeviceTypeWindowedValues = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceTypeWindowedValues,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceTypeWindowedValues",
    errors,
  );
  if (
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9CheckDeviceTypePrimaryBalancedApiPairs !== 3_495 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceBytes !==
      2_282_110 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceLines !== 7_068 ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9CheckDeviceTypeConfirmationBalancedApiPairs !==
      3_496 ||
    latestEvidence.nativeD3d9CheckDeviceTypeTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceTypeCallsPerTrace !== 5 ||
    latestEvidence.nativeD3d9CheckDeviceTypeDirectCallsPerTrace !== 2 ||
    latestEvidence.nativeD3d9CheckDeviceTypeInternalCallsPerTrace !== 3 ||
    nativeD3d9CheckDeviceTypeCallerRvas.join(",") !== "0x0005b9d6,0x0005ba1d" ||
    nativeD3d9CheckDeviceTypeWindowedValues.join(",") !== "1,0" ||
    latestEvidence.nativeD3d9CheckDeviceTypeAdapter !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceTypeDeviceType !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceTypeAdapterFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceTypeBackBufferFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceTypeResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CheckDeviceTypeHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CheckDeviceTypeConsumerOracle !==
      "immutable-pe-disassembly-and-device-caps-consumer-snapshot" ||
    latestEvidence.nativeD3d9CheckDeviceTypeExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CheckDeviceTypeImplemented !== true ||
    d3d9CheckDeviceTypeSequences.join(",") !== "17820,17821" ||
    d3d9CheckDeviceTypeInstructionCounts.join(",") !== "11806497,11806510" ||
    JSON.stringify(latestEvidence.d3d9CheckDeviceTypeArguments) !==
      JSON.stringify([
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000001",
        ],
        [
          "0x01000004",
          "0x00000000",
          "0x00000001",
          "0x00000016",
          "0x00000016",
          "0x00000000",
        ],
      ]) ||
    latestEvidence.d3d9CheckDeviceTypeReturnAddresses?.join(",") !==
      "0x1005b9d6,0x1005ba1d" ||
    latestEvidence.d3d9CheckDeviceTypeResults?.join(",") !==
      "0x00000000,0x00000000" ||
    latestEvidence.d3d9CheckDeviceTypeLastError !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeGuestWriteBytes !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeAdapter !== 0 ||
    latestEvidence.d3d9CheckDeviceTypeDeviceType !== 1 ||
    latestEvidence.d3d9CheckDeviceTypeAdapterFormat !== 22 ||
    latestEvidence.d3d9CheckDeviceTypeBackBufferFormat !== 22 ||
    d3d9CheckDeviceTypeWindowedValues.join(",") !== "1,0" ||
    latestEvidence.d3d9CheckDeviceTypeInvalidCallResult !== "0x8876086c" ||
    latestEvidence.d3d9CheckDeviceTypeNotAvailableResult !== "0x8876086a" ||
    latestEvidence.d3d9CheckDeviceTypeBoolNormalization !==
      "zero-fullscreen-nonzero-windowed" ||
    latestEvidence.d3d9CheckDeviceTypePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CheckDeviceTypePolicyVersion !== 22 ||
    latestEvidence.d3d9CheckDeviceTypePolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9CheckDeviceTypeProfileId !==
      "primary-hal-x8r8g8b8-windowed-fullscreen-v1" ||
    latestEvidence.d3d9CheckDeviceTypeDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9CheckDeviceTypeCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9CheckDeviceTypeDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CheckDeviceTypeReferenceCount !== 1 ||
    latestEvidence.d3d9CheckDeviceTypeReferenceCountMutation !== false ||
    latestEvidence.d3d9CheckDeviceTypeFailureAtomicity !== true ||
    latestEvidence.d3d9CheckDeviceTypeBrowserOwnsComState !== false ||
    latestEvidence.d3d9CheckDeviceTypeRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9CheckDeviceTypeRendererResourceCreated !== false ||
    latestEvidence.d3d9CheckDeviceTypeNodeCompletions !== 2 ||
    latestEvidence.d3d9CheckDeviceTypeChromeCompletions !== 2 ||
    latestEvidence.d3d9CheckDeviceTypeNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 device-type evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CheckDeviceFormatPrimaryTraceSha256",
    "nativeD3d9CheckDeviceFormatConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const nativeD3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    latestEvidence.nativeD3d9CheckDeviceFormatSupportedUsages,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3d9CheckDeviceFormatSupportedUsages",
    errors,
  );
  const latestD3d9CheckDeviceFormatSupportedUsages = requireIntegerArray(
    latestEvidence.d3d9CheckDeviceFormatSupportedUsages,
    "activeRuntimeCheckpoint.latestEvidence.d3d9CheckDeviceFormatSupportedUsages",
    errors,
  );
  if (
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceBytes !== 2_274_737 ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceLines !== 7_066 ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryTraceSha256 !==
      "69655d319b4cd31f814690094a9c108f18b59ca92e1d31e6095cc7344f87d361" ||
    latestEvidence.nativeD3d9CheckDeviceFormatPrimaryBalancedApiPairs !==
      3_495 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceBytes !==
      2_282_110 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceLines !==
      7_068 ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationTraceSha256 !==
      "56379b0c6267fa8e919a89b11222b547d21f374d634ed59138c78a3612ffb09f" ||
    latestEvidence.nativeD3d9CheckDeviceFormatConfirmationBalancedApiPairs !==
      3_496 ||
    latestEvidence.nativeD3d9CheckDeviceFormatTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceFormatCallsPerTrace !== 13 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceFormatCreateDeviceParentedCallsPerTrace !==
      3 ||
    latestEvidence.nativeD3d9CheckDeviceFormatOtherInternalCallsPerTrace !==
      9 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCallerRva !==
      "0x0005ba69" ||
    latestEvidence.nativeD3d9CheckDeviceFormatAdapter !== 0 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDeviceType !== 1 ||
    latestEvidence.nativeD3d9CheckDeviceFormatAdapterFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatResourceType !== 3 ||
    nativeD3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    latestEvidence.nativeD3d9CheckDeviceFormatSupportedCheckFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatBasicUsageSuccessCallsPerTrace !==
      6 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDynamicUsageSuccessCallsPerTrace !==
      4 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedCheckFormat !== 118 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9CheckDeviceFormatUnsupportedResult !==
      "0x8876086a" ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectUsage !== 512 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectCheckFormat !== 22 ||
    latestEvidence.nativeD3d9CheckDeviceFormatDirectResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CheckDeviceFormatHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CheckDeviceFormatConsumerOracle !==
      "immutable-pe-disassembly" ||
    latestEvidence.nativeD3d9CheckDeviceFormatExecutableHashUnchanged !==
      true ||
    latestEvidence.d3d9CheckDeviceFormatImplemented !== true ||
    latestEvidence.d3d9CheckDeviceFormatSequence !== 17_822 ||
    latestEvidence.d3d9CheckDeviceFormatInstructionCount !== 11_806_524 ||
    latestEvidence.d3d9CheckDeviceFormatArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x00000016,0x00000200,0x00000003,0x00000016" ||
    latestEvidence.d3d9CheckDeviceFormatReturnAddress !== "0x1005ba69" ||
    latestEvidence.d3d9CheckDeviceFormatResult !== "0x00000000" ||
    latestEvidence.d3d9CheckDeviceFormatLastError !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatGuestWriteBytes !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatAdapter !== 0 ||
    latestEvidence.d3d9CheckDeviceFormatDeviceType !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatAdapterFormat !== 22 ||
    latestEvidence.d3d9CheckDeviceFormatUsage !== 512 ||
    latestEvidence.d3d9CheckDeviceFormatResourceType !== 3 ||
    latestEvidence.d3d9CheckDeviceFormatCheckFormat !== 22 ||
    latestD3d9CheckDeviceFormatSupportedUsages.join(",") !== "0,512" ||
    latestEvidence.d3d9CheckDeviceFormatInvalidCallResult !== "0x8876086c" ||
    latestEvidence.d3d9CheckDeviceFormatNotAvailableResult !== "0x8876086a" ||
    latestEvidence.d3d9CheckDeviceFormatPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CheckDeviceFormatPolicyVersion !== 22 ||
    latestEvidence.d3d9CheckDeviceFormatPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9CheckDeviceFormatProfileId !==
      "primary-hal-x8r8g8b8-texture-basic-dynamic-v1" ||
    latestEvidence.d3d9CheckDeviceFormatDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9CheckDeviceFormatCapabilityAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9CheckDeviceFormatDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CheckDeviceFormatReferenceCount !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatReferenceCountMutation !== false ||
    latestEvidence.d3d9CheckDeviceFormatFailureAtomicity !== true ||
    latestEvidence.d3d9CheckDeviceFormatBrowserOwnsComState !== false ||
    latestEvidence.d3d9CheckDeviceFormatRuntimeGraphicsCreated !== false ||
    latestEvidence.d3d9CheckDeviceFormatRendererResourceCreated !== false ||
    latestEvidence.d3d9CheckDeviceFormatNodeCompletions !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatChromeCompletions !== 1 ||
    latestEvidence.d3d9CheckDeviceFormatNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest D3D9 device-format evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CreateDevicePrimaryTraceSha256",
    "nativeD3d9CreateDevicePrimarySummarySha256",
    "nativeD3d9CreateDeviceConfirmationTraceSha256",
    "nativeD3d9CreateDeviceConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceBytes !== 2_333_677 ||
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceLines !== 7_075 ||
    latestEvidence.nativeD3d9CreateDevicePrimaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    latestEvidence.nativeD3d9CreateDevicePrimaryBalancedApiPairs !== 3_498 ||
    latestEvidence.nativeD3d9CreateDevicePrimarySummaryBytes !== 275_843 ||
    latestEvidence.nativeD3d9CreateDevicePrimarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceBytes !== 2_355_307 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceLines !== 7_121 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationBalancedApiPairs !==
      3_521 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationSummaryBytes !== 275_855 ||
    latestEvidence.nativeD3d9CreateDeviceConfirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    latestEvidence.nativeD3d9CreateDeviceTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9CreateDeviceCallerRva !== "0x0005aca5" ||
    latestEvidence.nativeD3d9CreateDeviceAdapter !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceDeviceType !== 1 ||
    latestEvidence.nativeD3d9CreateDeviceBehaviorFlags !== "0x00000044" ||
    JSON.stringify(
      latestEvidence.nativeD3d9CreateDeviceInputWidthHeightFormat,
    ) !== JSON.stringify([0, 0, 0]) ||
    JSON.stringify(
      latestEvidence.nativeD3d9CreateDeviceResolvedWidthHeightFormat,
    ) !==
      JSON.stringify([
        [800, 600, 22],
        [1280, 720, 22],
        [1280, 720, 22],
      ]) ||
    latestEvidence.nativeD3d9CreateDevicePreservedBackBufferCount !== 1 ||
    latestEvidence.nativeD3d9CreateDevicePreservedSwapEffect !== 3 ||
    latestEvidence.nativeD3d9CreateDevicePreservedWindowed !== 1 ||
    latestEvidence.nativeD3d9CreateDevicePreservedAutoDepthStencilFormat !==
      80 ||
    latestEvidence.nativeD3d9CreateDeviceResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CreateDeviceLastError !== 0 ||
    latestEvidence.nativeD3d9CreateDeviceOutputNonNull !== true ||
    latestEvidence.nativeD3d9CreateDeviceHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CreateDeviceFirstReachedDeviceMethod !==
      "CreateTexture" ||
    latestEvidence.nativeD3d9CreateDeviceFirstReachedDeviceSlot !== 23 ||
    latestEvidence.nativeD3d9CreateDeviceNestedAddRefResult !== 2 ||
    latestEvidence.nativeD3d9CreateDeviceExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CreateDeviceImplemented !== true ||
    latestEvidence.d3d9CreateDeviceSequence !== 17_849 ||
    latestEvidence.d3d9CreateDeviceInstructionCount !== 11_807_387 ||
    latestEvidence.d3d9CreateDeviceArguments?.join(",") !==
      "0x01000004,0x00000000,0x00000001,0x48010001,0x00000044,0x101038f0,0x101517c4" ||
    latestEvidence.d3d9CreateDeviceReturnAddress !== "0x1005aca5" ||
    latestEvidence.d3d9CreateDeviceResult !== "0x00000000" ||
    latestEvidence.d3d9CreateDeviceLastError !== 0 ||
    latestEvidence.d3d9CreateDeviceGuestWriteBytes !== 16 ||
    latestEvidence.d3d9CreateDevicePresentationPrefixHex !==
      "200300005802000016000000" ||
    latestEvidence.d3d9CreateDeviceOutputPointer !== "0x01010004" ||
    latestEvidence.d3d9CreateDevicePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CreateDevicePolicyVersion !== 22 ||
    latestEvidence.d3d9CreateDevicePolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9CreateDeviceProfileId !==
      "primary-windowed-hal-device-v1" ||
    latestEvidence.d3d9CreateDeviceDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9CreateDeviceWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.d3d9CreateDeviceDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.d3d9CreateDeviceGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9CreateDeviceComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9CreateDeviceDeviceClassIdentity !==
      "d3d9.idirect3ddevice9.v1" ||
    latestEvidence.d3d9CreateDeviceDeviceInterface !== "IDirect3DDevice9" ||
    latestEvidence.d3d9CreateDeviceDeviceVtableSlots !== 119 ||
    latestEvidence.d3d9CreateDeviceDeviceReferenceCount !== 1 ||
    latestEvidence.d3d9CreateDeviceRootReferenceCount !== 2 ||
    latestEvidence.d3d9CreateDeviceRetainsRootReference !== true ||
    latestEvidence.d3d9CreateDeviceLogicalGraphicsDeviceId !== 1 ||
    latestEvidence.d3d9CreateDeviceLogicalGraphicsDeviceNamespace !==
      "vine.logical-graphics-device.v1" ||
    JSON.stringify(latestEvidence.d3d9CreateDeviceLogicalPresentation) !==
      JSON.stringify([800, 600, 22, 1, 1, 3, 1]) ||
    latestEvidence.d3d9CreateDeviceGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9CreateDeviceFailureAtomicity !== true ||
    latestEvidence.d3d9CreateDeviceFinalReleaseRetiresGraphicsAndMayCascadeRoot !==
      true ||
    latestEvidence.d3d9CreateDeviceBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9CreateDeviceRendererResourceCreated !== false ||
    latestEvidence.d3d9CreateDeviceNodeCompletions !== 1 ||
    latestEvidence.d3d9CreateDeviceChromeCompletions !== 1 ||
    latestEvidence.d3d9CreateDeviceNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 CreateDevice evidence is invalid");
  }
  for (const field of [
    "nativeD3d9CreateTexturePrimaryTraceSha256",
    "nativeD3d9CreateTexturePrimarySummarySha256",
    "nativeD3d9CreateTextureConfirmationTraceSha256",
    "nativeD3d9CreateTextureConfirmationSummarySha256",
    "nativeD3d9InitialTexturesPrimaryTraceSha256",
    "nativeD3d9InitialTexturesPrimarySummarySha256",
    "nativeD3d9InitialTexturesConfirmationTraceSha256",
    "nativeD3d9InitialTexturesConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceBytes !== 2_333_677 ||
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceLines !== 7_075 ||
    latestEvidence.nativeD3d9CreateTexturePrimaryTraceSha256 !==
      "df65b1108c813c948a7092db20bff85748f7654a1f7c2a98459980ebfc3f4071" ||
    latestEvidence.nativeD3d9CreateTexturePrimaryBalancedApiPairs !== 3_498 ||
    latestEvidence.nativeD3d9CreateTexturePrimarySummaryBytes !== 275_843 ||
    latestEvidence.nativeD3d9CreateTexturePrimarySummarySha256 !==
      "b49c7ee4179bbcd1f7df888c81e7a8fea1ffeb5fd14201fbaf6cd444bf0f8f60" ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceBytes !==
      2_355_307 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceLines !== 7_121 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationTraceSha256 !==
      "2b2da627c61b0a337a28912d77bdece390d45e965254a27b8e58ccb3130c95c3" ||
    latestEvidence.nativeD3d9CreateTextureConfirmationBalancedApiPairs !==
      3_521 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationSummaryBytes !==
      275_855 ||
    latestEvidence.nativeD3d9CreateTextureConfirmationSummarySha256 !==
      "a419a38f4fbb66deeddb634ab99e71841c23e624903d41e6a817e163c5d47e7b" ||
    latestEvidence.nativeD3d9CreateTextureTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9CreateTextureCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9CreateTextureCallerRva !== "0x0005a715" ||
    latestEvidence.nativeD3d9CreateTextureArguments?.join(",") !==
      "1024,1024,1,0,22,2" ||
    latestEvidence.nativeD3d9CreateTextureSharedHandleNull !== true ||
    latestEvidence.nativeD3d9CreateTextureResult !== "0x00000000" ||
    latestEvidence.nativeD3d9CreateTextureLastError !== 0 ||
    latestEvidence.nativeD3d9CreateTextureHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9CreateTextureOutputSnapshotCaptured !== false ||
    latestEvidence.nativeD3d9CreateTexturePointerClaimed !== false ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextMethod !==
      "GetSurfaceLevel" ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextSlot !== 18 ||
    latestEvidence.nativeD3d9CreateTextureImmediateNextLevel !== 0 ||
    latestEvidence.nativeD3d9CreateTextureExecutableHashUnchanged !== true ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceBytes !== 261_943 ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceLines !== 505 ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryTraceSha256 !==
      "2fe16a38d7a8c8ee18b673d06f759ef3bfa3d412d3f688689692edb4d6d55ad4" ||
    latestEvidence.nativeD3d9InitialTexturesPrimaryBalancedApiPairs !== 219 ||
    latestEvidence.nativeD3d9InitialTexturesPrimarySummaryBytes !== 328_531 ||
    latestEvidence.nativeD3d9InitialTexturesPrimarySummarySha256 !==
      "85db2bcea0fb895c1481a3af7330a3c79b430cbed997761cdb23d30e0fbf875d" ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceBytes !==
      262_766 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceLines !== 508 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationTraceSha256 !==
      "cdf646718a1a187e72129d9ae5562d44b4c5432c7c9c60ee1c217d30ae90179f" ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationBalancedApiPairs !==
      220 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationSummaryBytes !==
      328_698 ||
    latestEvidence.nativeD3d9InitialTexturesConfirmationSummarySha256 !==
      "20eb05e775a84075aec1c48b2137c9d237f684e2e19d580b1aac5671a7039837" ||
    latestEvidence.nativeD3d9InitialTexturesTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9InitialTexturesProfileId !==
      "x86-bgi-initial-textures-v1" ||
    latestEvidence.nativeD3d9InitialTexturesCallsPerTrace !== 3 ||
    latestEvidence.nativeD3d9InitialTexturesCallerRvas?.join(",") !==
      "0x0005a715,0x0005a7b7,0x0005a7dc" ||
    JSON.stringify(latestEvidence.nativeD3d9InitialTexturesVariants) !==
      JSON.stringify([
        [1_024, 1_024, 1, 0, 22, 2],
        [1_024, 1_024, 1, 0, 22, 0],
        [1_024, 1_024, 1, 512, 22, 0],
      ]) ||
    latestEvidence.nativeD3d9InitialTexturesSharedHandlesNull !== true ||
    latestEvidence.nativeD3d9InitialTexturesResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    latestEvidence.nativeD3d9InitialTexturesLastErrors?.join(",") !== "0,0,0" ||
    latestEvidence.nativeD3d9InitialTexturesSameDevicePerTrace !== true ||
    latestEvidence.nativeD3d9InitialTexturesOutputAddresses?.join(",") !==
      "0x00d117cc,0x00d117d0,0x00d117d4" ||
    latestEvidence.nativeD3d9InitialTexturesReturnedPointersDistinct !== true ||
    latestEvidence.nativeD3d9InitialTexturesPointersOpaqueArtifactIdentity !==
      true ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextMethod !==
      "CreateVertexBuffer" ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextSlot !== 26 ||
    latestEvidence.nativeD3d9InitialTexturesImmediateNextArguments?.join(
      ",",
    ) !== "112,8,324,1" ||
    latestEvidence.nativeD3d9InitialTexturesExecutableHashUnchanged !== true ||
    latestEvidence.d3d9CreateTextureImplemented !== true ||
    latestEvidence.d3d9CreateTextureSequence !== 17_850 ||
    latestEvidence.d3d9CreateTextureInstructionCount !== 11_807_772 ||
    latestEvidence.d3d9CreateTextureArguments?.join(",") !==
      "0x01010004,0x00000400,0x00000400,0x00000001,0x00000000,0x00000016,0x00000002,0x101517cc,0x00000000" ||
    latestEvidence.d3d9CreateTextureReturnAddress !== "0x1005a715" ||
    latestEvidence.d3d9CreateTextureResult !== "0x00000000" ||
    latestEvidence.d3d9CreateTextureLastError !== 0 ||
    latestEvidence.d3d9CreateTextureGuestWriteBytes !== 4 ||
    latestEvidence.d3d9CreateTextureOutputPointer !== "0x01020004" ||
    latestEvidence.d3d9CreateTexturePolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9CreateTexturePolicyVersion !== 22 ||
    latestEvidence.d3d9CreateTexturePolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9CreateTextureProfileId !==
      "primary-initial-and-recreated-x8r8g8b8-textures-v2" ||
    latestEvidence.d3d9CreateTextureGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9CreateTextureComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9CreateTextureClassIdentity !==
      "d3d9.idirect3dtexture9.v1" ||
    latestEvidence.d3d9CreateTextureInterface !== "IDirect3DTexture9" ||
    latestEvidence.d3d9CreateTextureVtableSlots !== 22 ||
    latestEvidence.d3d9CreateTextureDeviceReferenceCount !== 2 ||
    latestEvidence.d3d9CreateTextureReferenceCount !== 1 ||
    latestEvidence.d3d9CreateTextureRetainsDeviceReference !== true ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsDeviceId !== 1 ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsTextureId !== 1 ||
    latestEvidence.d3d9CreateTextureLogicalGraphicsTextureNamespace !==
      "vine.logical-graphics-texture.v1" ||
    latestEvidence.d3d9CreateTextureLogicalDescriptor?.join(",") !==
      "1024,1024,1,0,22,2" ||
    latestEvidence.d3d9CreateTextureGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9CreateTextureFailureAtomicity !== true ||
    latestEvidence.d3d9CreateTextureFinalReleaseRetiresGraphicsAndMayCascadeDeviceRoot !==
      true ||
    latestEvidence.d3d9CreateTextureBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9CreateTextureRendererResourceCreated !== false ||
    latestEvidence.d3d9CreateTextureNodeCompletions !== 3 ||
    latestEvidence.d3d9CreateTextureChromeCompletions !== 3 ||
    latestEvidence.d3d9CreateTextureNodeChromeOrderedSemanticsMatched !==
      true ||
    latestEvidence.d3d9InitialTexturesNodeSequences?.join(",") !==
      "17850,17857,17858" ||
    latestEvidence.d3d9InitialTexturesNodeInstructionCounts?.join(",") !==
      "11807772,11807840,11807856" ||
    latestEvidence.d3d9InitialTexturesChromeSequences?.join(",") !==
      "17850,17857,17858" ||
    latestEvidence.d3d9InitialTexturesCallerReturnAddresses?.join(",") !==
      "0x1005a715,0x1005a7b7,0x1005a7dc" ||
    JSON.stringify(latestEvidence.d3d9InitialTexturesArguments) !==
      JSON.stringify([
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000002",
          "0x101517cc",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000000",
          "0x00000016",
          "0x00000000",
          "0x101517d0",
          "0x00000000",
        ],
        [
          "0x01010004",
          "0x00000400",
          "0x00000400",
          "0x00000001",
          "0x00000200",
          "0x00000016",
          "0x00000000",
          "0x101517d4",
          "0x00000000",
        ],
      ]) ||
    latestEvidence.d3d9InitialTexturesResults?.join(",") !==
      "0x00000000,0x00000000,0x00000000" ||
    latestEvidence.d3d9InitialTexturesOutputAddresses?.join(",") !==
      "0x101517cc,0x101517d0,0x101517d4" ||
    latestEvidence.d3d9InitialTexturesPointers?.join(",") !==
      "0x01020004,0x01030004,0x01040004" ||
    latestEvidence.d3d9InitialTexturesGuestWriteBytes?.join(",") !== "4,4,4" ||
    JSON.stringify(latestEvidence.d3d9InitialTexturesLogicalDescriptors) !==
      JSON.stringify([
        [1_024, 1_024, 1, 0, 22, 2],
        [1_024, 1_024, 1, 0, 22, 0],
        [1_024, 1_024, 1, 512, 22, 0],
      ]) ||
    latestEvidence.d3d9InitialTexturesLogicalGraphicsDeviceCount !== 1 ||
    latestEvidence.d3d9InitialTexturesLogicalGraphicsTextureCount !== 3 ||
    latestEvidence.d3d9InitialTexturesDeviceReferenceCountAtStop !== 4 ||
    latestEvidence.d3d9InitialTexturesBrowserRendererCreated !== false ||
    latestEvidence.d3d9InitialTexturesPixelStorageCreated !== false ||
    latestEvidence.d3d9InitialTexturesNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 CreateTexture evidence is invalid");
  }
  for (const field of [
    "nativeD3d9RasterStatusPrimaryTraceSha256",
    "nativeD3d9RasterStatusPrimarySummarySha256",
    "nativeD3d9RasterStatusConfirmationTraceSha256",
    "nativeD3d9RasterStatusConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9RasterStatusPrimaryTraceBytes !== 261_147 ||
    latestEvidence.nativeD3d9RasterStatusPrimaryTraceLines !== 505 ||
    latestEvidence.nativeD3d9RasterStatusPrimaryTraceSha256 !==
      "31ae84a21f5a70c4ae25f3234f52bf229b08e06ca309bbac1567509e7573362a" ||
    latestEvidence.nativeD3d9RasterStatusPrimarySummaryBytes !== 179_845 ||
    latestEvidence.nativeD3d9RasterStatusPrimarySummarySha256 !==
      "2d22afec1516f9cb1c2637eb641c6dc54c99f9d9fb3817add90d6fd40f76c724" ||
    latestEvidence.nativeD3d9RasterStatusConfirmationTraceBytes !== 263_079 ||
    latestEvidence.nativeD3d9RasterStatusConfirmationTraceLines !== 510 ||
    latestEvidence.nativeD3d9RasterStatusConfirmationTraceSha256 !==
      "491b6e252c957cc676b8d6dda981b15e74b5796ad9f7b327fbe9bd861b5eea7d" ||
    latestEvidence.nativeD3d9RasterStatusConfirmationSummaryBytes !== 180_079 ||
    latestEvidence.nativeD3d9RasterStatusConfirmationSummarySha256 !==
      "e72d2b789c77f2005408dd4517179858939ac02559679fd11e5b20301f428069" ||
    latestEvidence.nativeD3d9RasterStatusTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9RasterStatusProfileId !==
      "x86-bgi-first-raster-status-v1" ||
    latestEvidence.nativeD3d9RasterStatusCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9RasterStatusCallerRva !== "0x0005aedd" ||
    latestEvidence.nativeD3d9RasterStatusSlot !== 19 ||
    latestEvidence.nativeD3d9RasterStatusSwapChain !== 0 ||
    latestEvidence.nativeD3d9RasterStatusResults?.join(",") !==
      "0x00000000,0x00000000" ||
    latestEvidence.nativeD3d9RasterStatusLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeD3d9RasterStatusObservedInVBlank?.join(",") !==
      "false,true" ||
    latestEvidence.nativeD3d9RasterStatusObservedScanLines?.join(",") !==
      "1200,0" ||
    latestEvidence.nativeD3d9RasterStatusOutputFullyOverwritten !== true ||
    latestEvidence.nativeD3d9RasterStatusImmediateConsumerChecksHresult !==
      true ||
    latestEvidence.nativeD3d9RasterStatusImmediateConsumerChecksVBlank !==
      true ||
    latestEvidence.nativeD3d9RasterStatusImmediateConsumerConsumesScanLine !==
      true ||
    latestEvidence.nativeD3d9RasterStatusVBlankResult !== "0x81000000" ||
    latestEvidence.nativeD3d9RasterStatusOuterConsumerRva !== "0x0006a263" ||
    latestEvidence.nativeD3d9RasterStatusOuterConsumerAcceptsSuccessOrVBlank !==
      true ||
    latestEvidence.nativeD3d9RasterStatusOuterConsumerRepeatsRead !== true ||
    latestEvidence.nativeD3d9RasterStatusOuterConsumerYieldsWithSleepZero !==
      true ||
    latestEvidence.nativeD3d9RasterStatusPointersOpaqueArtifactIdentity !==
      true ||
    latestEvidence.nativeD3d9RasterStatusHostValuesArtifactObservations !==
      true ||
    latestEvidence.nativeD3d9RasterStatusExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetRasterStatusImplemented !== true ||
    latestEvidence.d3d9GetRasterStatusNodeSequence !== 17_866 ||
    latestEvidence.d3d9GetRasterStatusNodeInstructionCount !== 11_808_586 ||
    latestEvidence.d3d9GetRasterStatusChromeSequence !== 17_866 ||
    latestEvidence.d3d9GetRasterStatusArguments?.join(",") !==
      "0x01010004,0x00000000,0x006ffcf0" ||
    latestEvidence.d3d9GetRasterStatusReturnAddress !== "0x1005aedd" ||
    latestEvidence.d3d9GetRasterStatusResult !== "0x00000000" ||
    latestEvidence.d3d9GetRasterStatusLastError !== 0 ||
    latestEvidence.d3d9GetRasterStatusGuestWriteBytes !== 8 ||
    latestEvidence.d3d9GetRasterStatusOutputAddress !== "0x006ffcf0" ||
    latestEvidence.d3d9GetRasterStatusOutputHex !== "0000000001000000" ||
    latestEvidence.d3d9GetRasterStatusInVBlank !== false ||
    latestEvidence.d3d9GetRasterStatusScanLine !== 1 ||
    latestEvidence.d3d9GetRasterStatusDeviceReferenceCount !== 5 ||
    latestEvidence.d3d9GetRasterStatusObjectRetired !== false ||
    latestEvidence.d3d9GetRasterStatusPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetRasterStatusPolicyVersion !== 22 ||
    latestEvidence.d3d9GetRasterStatusPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetRasterStatusProfileId !==
      "primary-stable-nonblank-scanline-v1" ||
    latestEvidence.d3d9GetRasterStatusDisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.d3d9GetRasterStatusStateAuthority !==
      "crates/vine-core/src/d3d9.rs::D3d9CompatibilityPolicy" ||
    latestEvidence.d3d9GetRasterStatusRuntimeStateMutation !== false ||
    latestEvidence.d3d9GetRasterStatusFailureAtomicity !== true ||
    latestEvidence.d3d9GetRasterStatusHostScanoutImported !== false ||
    latestEvidence.d3d9GetRasterStatusBrowserScanoutImported !== false ||
    latestEvidence.d3d9GetRasterStatusNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 GetRasterStatus evidence is invalid");
  }
  for (const field of [
    "nativeQueryPerformanceFrequencyPrimaryTraceSha256",
    "nativeQueryPerformanceFrequencyPrimarySummarySha256",
    "nativeQueryPerformanceFrequencyConfirmationTraceSha256",
    "nativeQueryPerformanceFrequencyConfirmationSummarySha256",
    "nativeQueryPerformanceCounterPrimaryTraceSha256",
    "nativeQueryPerformanceCounterPrimarySummarySha256",
    "nativeQueryPerformanceCounterConfirmationTraceSha256",
    "nativeQueryPerformanceCounterConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeQueryPerformanceFrequencyPrimaryTraceBytes !==
      186_666 ||
    latestEvidence.nativeQueryPerformanceFrequencyPrimaryTraceLines !== 469 ||
    latestEvidence.nativeQueryPerformanceFrequencyPrimaryTraceSha256 !==
      "d8d40e8dece88280ab1d35283b143359a447d3608b4abc5bf80e705198d86790" ||
    latestEvidence.nativeQueryPerformanceFrequencyPrimarySummaryBytes !==
      36_690 ||
    latestEvidence.nativeQueryPerformanceFrequencyPrimarySummarySha256 !==
      "5a7b742f190a59cd9d50df2deca1283e3f88356fc98de88119815e97964177ab" ||
    latestEvidence.nativeQueryPerformanceFrequencyConfirmationTraceBytes !==
      185_056 ||
    latestEvidence.nativeQueryPerformanceFrequencyConfirmationTraceLines !==
      464 ||
    latestEvidence.nativeQueryPerformanceFrequencyConfirmationTraceSha256 !==
      "1d3b500b87d2482ffbce59f0363c1506e81683b89aba476adc53184fad637a03" ||
    latestEvidence.nativeQueryPerformanceFrequencyConfirmationSummaryBytes !==
      36_866 ||
    latestEvidence.nativeQueryPerformanceFrequencyConfirmationSummarySha256 !==
      "5e144ad4a2b037ddc1ebc15a9151134df430b0d9fff18237cdeaa060c45fc40d" ||
    latestEvidence.nativeQueryPerformanceFrequencyTraceDiagnostics !== 0 ||
    latestEvidence.nativeQueryPerformanceFrequencyProfile !==
      "x86-bgi-query-performance-frequency-v1" ||
    latestEvidence.nativeQueryPerformanceFrequencyOracleProfile !==
      "x86-stable-performance-frequency-v1" ||
    latestEvidence.nativeQueryPerformanceFrequencyOracleCallsPerTrace !== 3 ||
    latestEvidence.nativeQueryPerformanceFrequencyOracleAllSucceeded !== true ||
    latestEvidence.nativeQueryPerformanceFrequencyOracleAllPreservedLastError !==
      true ||
    latestEvidence.nativeQueryPerformanceFrequencyOracleGuardsPreserved !==
      true ||
    latestEvidence.nativeQueryPerformanceFrequencyValue !== 10_000_000 ||
    latestEvidence.nativeQueryPerformanceFrequencyStableAcrossCallsAndRuns !==
      true ||
    latestEvidence.nativeQueryPerformanceFrequencyCallerRva !== "0x00091fd7" ||
    latestEvidence.nativeQueryPerformanceFrequencyConsumer !==
      "fallback-to-1000-on-failure-and-store-frequency-for-counter-ratio" ||
    latestEvidence.nativeQueryPerformanceFrequencyExecutableHashUnchanged !==
      true ||
    latestEvidence.nativeQueryPerformanceCounterPrimaryTraceBytes !== 241_192 ||
    latestEvidence.nativeQueryPerformanceCounterPrimaryTraceLines !== 477 ||
    latestEvidence.nativeQueryPerformanceCounterPrimaryTraceSha256 !==
      "976b08a862fb847f55460ad0c4c6fa00b50c71dcd250beda60310c5fd60f0398" ||
    latestEvidence.nativeQueryPerformanceCounterPrimarySummaryBytes !==
      38_763 ||
    latestEvidence.nativeQueryPerformanceCounterPrimarySummarySha256 !==
      "9d13cdace9dc15366319fa8570efc451d66481a81779acbf3f733515623cedf0" ||
    latestEvidence.nativeQueryPerformanceCounterConfirmationTraceBytes !==
      284_763 ||
    latestEvidence.nativeQueryPerformanceCounterConfirmationTraceLines !==
      477 ||
    latestEvidence.nativeQueryPerformanceCounterConfirmationTraceSha256 !==
      "7f77b57c662d2bcacb3c5db417bb7de07412df1388eea390eb1a9a5b1c1f71d9" ||
    latestEvidence.nativeQueryPerformanceCounterConfirmationSummaryBytes !==
      39_582 ||
    latestEvidence.nativeQueryPerformanceCounterConfirmationSummarySha256 !==
      "0fd05c7688e7330b9644ad9d908e2e1ecddb15a4f7513e57a3e5f8cd9134e32a" ||
    latestEvidence.nativeQueryPerformanceCounterTraceDiagnostics !== 0 ||
    latestEvidence.nativeQueryPerformanceCounterProfile !==
      "x86-bgi-query-performance-counter-progression-v1" ||
    latestEvidence.nativeQueryPerformanceCounterOracleProfile !==
      "x86-monotonic-performance-counter-v1" ||
    latestEvidence.nativeQueryPerformanceCounterOracleCallsPerTrace !== 4 ||
    latestEvidence.nativeQueryPerformanceCounterOracleDeltasPrimary?.join(
      ",",
    ) !== "4396,4020,3984" ||
    latestEvidence.nativeQueryPerformanceCounterOracleDeltasConfirmation?.join(
      ",",
    ) !== "3791,3117,3349" ||
    latestEvidence.nativeQueryPerformanceCounterOracleStrictlyIncreasing !==
      true ||
    latestEvidence.nativeQueryPerformanceCounterOracleAllSucceeded !== true ||
    latestEvidence.nativeQueryPerformanceCounterOracleAllPreservedLastError !==
      true ||
    latestEvidence.nativeQueryPerformanceCounterOracleGuardsPreserved !==
      true ||
    latestEvidence.nativeQueryPerformanceCounterInitialCallerRva !==
      "0x000a88fa" ||
    latestEvidence.nativeQueryPerformanceCounterTargetCallerRva !==
      "0x00092017" ||
    latestEvidence.nativeQueryPerformanceCounterTargetSampleDelta !== 17_953 ||
    latestEvidence.nativeQueryPerformanceCounterTargetSampleDeltaMillisecondsAtFrequency !==
      1.7953 ||
    latestEvidence.nativeQueryPerformanceCounterConsumer !==
      "divide-counter-by-frequency-with-get-tick-count-fallback-on-failure" ||
    latestEvidence.nativeQueryPerformanceCounterExecutableHashUnchanged !==
      true ||
    latestEvidence.queryPerformanceFrequencyImplemented !== true ||
    latestEvidence.queryPerformanceFrequencySequence !== 17_867 ||
    latestEvidence.queryPerformanceFrequencyInstructionCount !== 11_808_626 ||
    latestEvidence.queryPerformanceFrequencyArguments?.join(",") !==
      "0x006ffcf0" ||
    latestEvidence.queryPerformanceFrequencyReturnAddress !== "0x10091fd7" ||
    latestEvidence.queryPerformanceFrequencyResult !== "0x00000001" ||
    latestEvidence.queryPerformanceFrequencyLastError !== 0 ||
    latestEvidence.queryPerformanceFrequencyGuestWriteBytes !== 8 ||
    latestEvidence.queryPerformanceFrequencyOutputHex !== "8096980000000000" ||
    latestEvidence.queryPerformanceFrequencyValue !== 10_000_000 ||
    latestEvidence.queryPerformanceFrequencyImplementationId !==
      "kernel32.query-performance-frequency.v1" ||
    latestEvidence.queryPerformanceFrequencyStateAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    latestEvidence.queryPerformanceFrequencyRuntimeStateMutation !== false ||
    latestEvidence.queryPerformanceFrequencyFailureAtomicity !== true ||
    latestEvidence.queryPerformanceFrequencyHostClockImported !== false ||
    latestEvidence.queryPerformanceFrequencyBrowserClockImported !== false ||
    latestEvidence.queryPerformanceFrequencyNodeChromeOrderedSemanticsMatched !==
      true ||
    latestEvidence.queryPerformanceCounterCallCount !== 301 ||
    latestEvidence.queryPerformanceCounterFirstSequence !== 5 ||
    latestEvidence.queryPerformanceCounterFirstInstructionCount !== 34 ||
    latestEvidence.queryPerformanceCounterFirstArguments?.join(",") !==
      "0x006fffe4" ||
    latestEvidence.queryPerformanceCounterFirstReturnAddress !== "0x100a88fa" ||
    latestEvidence.queryPerformanceCounterFirstResult !== "0x00000001" ||
    latestEvidence.queryPerformanceCounterFirstLastError !== 87 ||
    latestEvidence.queryPerformanceCounterFirstValue !== "12345678901" ||
    latestEvidence.queryPerformanceCounterSecondSequence !== 17_868 ||
    latestEvidence.queryPerformanceCounterSecondInstructionCount !==
      11_808_693 ||
    latestEvidence.queryPerformanceCounterSecondArguments?.join(",") !==
      "0x006ffd40" ||
    latestEvidence.queryPerformanceCounterSecondReturnAddress !==
      "0x10092017" ||
    latestEvidence.queryPerformanceCounterSecondLastError !== 0 ||
    latestEvidence.queryPerformanceCounterSecondValue !== "12345688901" ||
    latestEvidence.queryPerformanceCounterLastSequence !== 18_437 ||
    latestEvidence.queryPerformanceCounterLastInstructionCount !== 11_824_434 ||
    latestEvidence.queryPerformanceCounterLastArguments?.join(",") !==
      "0x006ffd48" ||
    latestEvidence.queryPerformanceCounterLastReturnAddress !== "0x10092017" ||
    latestEvidence.queryPerformanceCounterLastValue !== "12348678901" ||
    latestEvidence.queryPerformanceCounterLastGuestWriteGeneration !== 36_067 ||
    latestEvidence.queryPerformanceCounterIncrementPerRead !== 10_000 ||
    latestEvidence.queryPerformanceCounterImplementationId !==
      "kernel32.query-performance-counter.v1" ||
    latestEvidence.queryPerformanceCounterStateAuthority !==
      "runtime/process.rs::RuntimeProcessState" ||
    latestEvidence.queryPerformanceCounterMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.queryPerformanceCounterRangeFailureAtomicity !== true ||
    latestEvidence.queryPerformanceCounterHostClockImported !== false ||
    latestEvidence.queryPerformanceCounterBrowserClockImported !== false ||
    latestEvidence.queryPerformanceCounterNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest high-resolution performance-clock evidence is invalid");
  }
  for (const field of [
    "nativeGetMonitorInfoAPrimaryTraceSha256",
    "nativeGetMonitorInfoAPrimarySummarySha256",
    "nativeGetMonitorInfoAConfirmationTraceSha256",
    "nativeGetMonitorInfoAConfirmationSummarySha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeGetMonitorInfoAPrimaryTraceBytes !== 412_244 ||
    latestEvidence.nativeGetMonitorInfoAPrimaryTraceLines !== 816 ||
    latestEvidence.nativeGetMonitorInfoAPrimaryBalancedApiPairs !== 371 ||
    latestEvidence.nativeGetMonitorInfoAPrimaryTraceSha256 !==
      "01cfd734a384bf3678263af7312e3354688194e79ef21ee019b4fd527ec64577" ||
    latestEvidence.nativeGetMonitorInfoAPrimarySummaryBytes !== 70_362 ||
    latestEvidence.nativeGetMonitorInfoAPrimarySummarySha256 !==
      "39ebd444eb1bc674bf71c7a2a869cf8af42a6f948d8c33be3ccaa46949da0c0b" ||
    latestEvidence.nativeGetMonitorInfoAConfirmationTraceBytes !== 370_979 ||
    latestEvidence.nativeGetMonitorInfoAConfirmationTraceLines !== 677 ||
    latestEvidence.nativeGetMonitorInfoAConfirmationBalancedApiPairs !== 303 ||
    latestEvidence.nativeGetMonitorInfoAConfirmationTraceSha256 !==
      "f13219e019c4eecc5b383d8c2a79093338226a2ae19e00d0828d28fab8ef1a08" ||
    latestEvidence.nativeGetMonitorInfoAConfirmationSummaryBytes !== 65_244 ||
    latestEvidence.nativeGetMonitorInfoAConfirmationSummarySha256 !==
      "f4c0c49198b5b8aceaf282871c29f215611e584a272a23111c0d01f0e5367c13" ||
    latestEvidence.nativeGetMonitorInfoATraceDiagnostics !== 0 ||
    latestEvidence.nativeGetMonitorInfoAProfile !==
      "x86-bgi-first-monitor-info-a-v1" ||
    latestEvidence.nativeGetMonitorInfoAOracleProfile !==
      "x86-live-monitor-size-and-handle-matrix-v1" ||
    latestEvidence.nativeGetMonitorInfoAOracleCallsPerTrace !== 11 ||
    latestEvidence.nativeGetMonitorInfoAValidStructureSizes?.join(",") !==
      "40,72" ||
    latestEvidence.nativeGetMonitorInfoAInvalidStructureSizes?.join(",") !==
      "0,39,41,71,73" ||
    latestEvidence.nativeGetMonitorInfoAInvalidSizeLastError !== 87 ||
    latestEvidence.nativeGetMonitorInfoAInvalidHandleLastError !== 1_461 ||
    latestEvidence.nativeGetMonitorInfoAHandleValidatedBeforeSize !== true ||
    latestEvidence.nativeGetMonitorInfoAFailureBuffersUnchanged !== true ||
    latestEvidence.nativeGetMonitorInfoASuccessPreservesLastError !== true ||
    latestEvidence.nativeGetMonitorInfoAMonitorRect?.join(",") !==
      "0,0,1920,1080" ||
    latestEvidence.nativeGetMonitorInfoAWorkRect?.join(",") !==
      "0,0,1920,1032" ||
    latestEvidence.nativeGetMonitorInfoAFlags !== 1 ||
    latestEvidence.nativeGetMonitorInfoADeviceName !== "\\\\.\\DISPLAY1" ||
    latestEvidence.nativeGetMonitorInfoACallerRva !== "0x0005c913" ||
    latestEvidence.nativeGetMonitorInfoAFrameCallerRva !== "0x0009384c" ||
    latestEvidence.nativeGetMonitorInfoAConsumer !==
      "monitor-left-top-used-for-window-placement-work-area-not-consumed" ||
    latestEvidence.nativeGetMonitorInfoAExecutableHashUnchanged !== true ||
    latestEvidence.getMonitorInfoAImplemented !== true ||
    latestEvidence.getMonitorInfoASequence !== 18_439 ||
    latestEvidence.getMonitorInfoAInstructionCount !== 11_824_580 ||
    latestEvidence.getMonitorInfoAArguments?.join(",") !==
      "0x4f010001,0x006ffd44" ||
    latestEvidence.getMonitorInfoAReturnAddress !== "0x1005c913" ||
    latestEvidence.getMonitorInfoAResult !== "0x00000001" ||
    latestEvidence.getMonitorInfoALastError !== 0 ||
    latestEvidence.getMonitorInfoAGuestWriteBytes !== 40 ||
    latestEvidence.getMonitorInfoAOutputAddress !== "0x006ffd44" ||
    latestEvidence.getMonitorInfoAOutputHex !==
      "28000000000000000000000080070000380400000000000000000000800700000804000001000000" ||
    latestEvidence.getMonitorInfoAImplementationId !==
      "user32.get-monitor-info-a.v1" ||
    latestEvidence.getMonitorInfoAStateAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.getMonitorInfoADisplayProfileId !==
      "single-monitor-1920x1080-work-1920x1032-v2" ||
    latestEvidence.getMonitorInfoAMonitorHandleNamespace !==
      "vine.logical-monitor.v1" ||
    latestEvidence.getMonitorInfoAProfileId !==
      "logical-primary-monitor-info-40-v1" ||
    latestEvidence.getMonitorInfoAExtendedVariantImplemented !== false ||
    latestEvidence.getMonitorInfoAMutation !==
      "atomic-guest-structure-publication" ||
    latestEvidence.getMonitorInfoAFailureAtomicity !== true ||
    latestEvidence.getMonitorInfoAHostDisplayTopologyImported !== false ||
    latestEvidence.getMonitorInfoABrowserDisplayTopologyImported !== false ||
    latestEvidence.getMonitorInfoAFollowingMetricSequences?.join(",") !==
      "18440,18441" ||
    latestEvidence.getMonitorInfoAFollowingMetricValues?.join(",") !== "23,3" ||
    latestEvidence.getMonitorInfoANodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest logical monitor-information evidence is invalid");
  }
  for (const field of [
    "nativeDirectInput8RootPrimaryTraceSha256",
    "nativeDirectInput8RootConfirmationTraceSha256",
    "directInput8CreateOracleSourceSha256",
    "directInput8CreateOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeDirectInput8RootPrimaryTraceBytes !== 234_028 ||
    latestEvidence.nativeDirectInput8RootPrimaryTraceLines !== 698 ||
    latestEvidence.nativeDirectInput8RootPrimaryTraceSha256 !==
      "3fc837713e87b990da6576e19e9bd13e965581e2ab971437fab995a1e9fffe84" ||
    latestEvidence.nativeDirectInput8RootConfirmationTraceBytes !== 232_973 ||
    latestEvidence.nativeDirectInput8RootConfirmationTraceLines !== 695 ||
    latestEvidence.nativeDirectInput8RootConfirmationTraceSha256 !==
      "5216fa3642e2324fec4412579b9847a244585c3a4301c321a3a860e1ddac6c5c" ||
    latestEvidence.nativeDirectInput8RootProfile !==
      "x86-bgi-direct-input-8-root-v1" ||
    latestEvidence.nativeDirectInput8RootFactoryCallsPerTrace !== 1 ||
    latestEvidence.nativeDirectInput8RootEnumDevicesCallsPerTrace !== 1 ||
    latestEvidence.nativeDirectInput8RootFactoryCallerRva !== "0x0005c234" ||
    latestEvidence.nativeDirectInput8RootFactoryArguments?.join(",") !==
      "main-module-hinstance,0x00000800,IID_IDirectInput8A,non-null-output,0x00000000" ||
    latestEvidence.nativeDirectInput8RootIidBytes?.join(",") !==
      "48,128,121,191,58,72,162,77,170,153,93,100,237,54,151,0" ||
    latestEvidence.nativeDirectInput8RootFactoryResult !== "0x00000000" ||
    latestEvidence.nativeDirectInput8RootFactoryOutputNonNull !== true ||
    latestEvidence.nativeDirectInput8RootFactoryLastErrorBefore !== 0 ||
    latestEvidence.nativeDirectInput8RootFactoryLastErrorAfter !== 122 ||
    latestEvidence.nativeDirectInput8RootFactoryConsumer !==
      "test-hresult-sign-call-helper-and-use-root-interface" ||
    latestEvidence.nativeDirectInput8RootFirstMethod !== "EnumDevices" ||
    latestEvidence.nativeDirectInput8RootFirstMethodSlot !== 4 ||
    latestEvidence.nativeDirectInput8RootFirstMethodCallerRva !==
      "0x0005c1ce" ||
    latestEvidence.nativeDirectInput8RootFirstMethodArguments?.join(",") !==
      "0x00000004,0x1005bef0,0x00000000,0x00000001" ||
    latestEvidence.nativeDirectInput8RootFirstMethodResult !== "0x00000000" ||
    latestEvidence.nativeDirectInput8RootFirstMethodLastError !== 0 ||
    latestEvidence.nativeDirectInput8RootExecutableHashUnchanged !== true ||
    latestEvidence.directInput8CreateOracleArchitecture !== "coff-i386" ||
    latestEvidence.directInput8CreateOracleSourceBytes !== 6_196 ||
    latestEvidence.directInput8CreateOracleSourceSha256 !==
      "718ebcf0e4fda1985c0ed77b5f4e3f3c8aa4bcbe40ba86743fb1cd8094587595" ||
    latestEvidence.directInput8CreateOracleExecutableBytes !== 5_120 ||
    latestEvidence.directInput8CreateOracleExecutableSha256 !==
      "f1b98b895b09193b186e52377e8c7dceb91995a61c6998fc4fa8e59a2d06d5c9" ||
    latestEvidence.directInput8CreateOracleValidIids?.join(",") !==
      "IID_IDirectInput8A,IID_IUnknown" ||
    latestEvidence.directInput8CreateOracleValidResult !== "0x00000000" ||
    latestEvidence.directInput8CreateOracleUnknownIidResult !== "0x80004002" ||
    latestEvidence.directInput8CreateOracleOldVersionResult !== "0x80070481" ||
    latestEvidence.directInput8CreateOracleNewVersionResult !== "0x8007047e" ||
    latestEvidence.directInput8CreateOracleFailuresClearOutput !== true ||
    latestEvidence.directInput8CreateNativeAggregationObserved !==
      "success-with-null-output" ||
    latestEvidence.directInput8CreateAggregationPolicy !==
      "class-e-noaggregation-before-mutation" ||
    latestEvidence.directInput8CreateSuccessLastErrorPolicy !==
      "preserve-caller-last-error" ||
    latestEvidence.directInput8CreateImplemented !== true ||
    latestEvidence.directInput8CreateSequence !== 18_442 ||
    latestEvidence.directInput8CreateInstructionCount !== 11_825_212 ||
    latestEvidence.directInput8CreateArguments?.join(",") !==
      "0x10000000,0x00000800,0x100d22b8,0x10151800,0x00000000" ||
    latestEvidence.directInput8CreateReturnAddress !== "0x1005c234" ||
    latestEvidence.directInput8CreateResult !== "0x00000000" ||
    latestEvidence.directInput8CreateLastError !== 0 ||
    latestEvidence.directInput8CreateGuestWriteBytes !== 4 ||
    latestEvidence.directInput8CreateOutputAddress !== "0x10151800" ||
    latestEvidence.directInput8CreateOutputPointer !== "0x01060004" ||
    latestEvidence.directInput8CreateImplementationId !==
      "dinput8.direct-input-8-create.v1" ||
    latestEvidence.directInput8CreateStateAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.directInput8CreateClassIdentity !==
      "dinput8.idirectinput8a-root.v1" ||
    latestEvidence.directInput8CreateInterface !== "IDirectInput8A" ||
    latestEvidence.directInput8CreateInterfaceIidBytes?.join(",") !==
      "48,128,121,191,58,72,162,77,170,153,93,100,237,54,151,0" ||
    latestEvidence.directInput8CreateVtableSlots !== 11 ||
    latestEvidence.directInput8CreateObjectId !== 9 ||
    latestEvidence.directInput8CreateInterfaceId !== 18 ||
    latestEvidence.directInput8CreateReferenceCount !== 1 ||
    latestEvidence.directInput8CreateBackingAddress !== "0x01060000" ||
    latestEvidence.directInput8CreateBackingBytes !== 8_192 ||
    latestEvidence.directInput8CreateHostDeltaFromGeneration !== 36_073 ||
    latestEvidence.directInput8CreateHostDeltaGeneration !== 36_099 ||
    latestEvidence.directInput8CreateHostDeltaMappingOperations !== 5 ||
    latestEvidence.directInput8CreateHostDeltaDirtyPages?.join(",") !==
      "0x01060000:22,0x01061000:22,0x10151000:1" ||
    latestEvidence.directInput8CreateSupportsIunknown !== true ||
    latestEvidence.directInput8CreateGuestComAtomicity !== true ||
    latestEvidence.directInput8CreateFailureAtomicity !== true ||
    latestEvidence.directInput8CreateBrowserOwnsComState !== false ||
    latestEvidence.directInput8CreateDeviceEnumerationImplemented !== true ||
    latestEvidence.directInput8CreateBrowserInputImported !== false ||
    latestEvidence.directInput8CreateNodeCompletions !== 1 ||
    latestEvidence.directInput8CreateChromeCompletions !== 1 ||
    latestEvidence.directInput8CreateNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest DirectInput 8 root-factory evidence is invalid");
  }
  for (const field of [
    "nativeDirectInput8EnumDevicesPrimaryTraceSha256",
    "nativeDirectInput8EnumDevicesConfirmationTraceSha256",
    "directInput8EnumDevicesOracleSourceSha256",
    "directInput8EnumDevicesOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeDirectInput8EnumDevicesPrimaryTraceBytes !== 263_408 ||
    latestEvidence.nativeDirectInput8EnumDevicesPrimaryTraceLines !== 700 ||
    latestEvidence.nativeDirectInput8EnumDevicesPrimaryTraceSha256 !==
      "f4a04103c290ccd4f3ec8e19150d9480be60214f1904cc4ba082bb47784d29b1" ||
    latestEvidence.nativeDirectInput8EnumDevicesConfirmationTraceBytes !==
      261_749 ||
    latestEvidence.nativeDirectInput8EnumDevicesConfirmationTraceLines !==
      697 ||
    latestEvidence.nativeDirectInput8EnumDevicesConfirmationTraceSha256 !==
      "2c859ebf886caede34331c0ddfedd4c7591e9b51ad873038e810d88f765a037d" ||
    latestEvidence.nativeDirectInput8EnumDevicesProfile !==
      "x86-bgi-direct-input-8-enum-devices-v1" ||
    latestEvidence.nativeDirectInput8EnumDevicesCallsPerTrace !== 1 ||
    latestEvidence.nativeDirectInput8EnumDevicesCallerRva !== "0x0005c1ce" ||
    latestEvidence.nativeDirectInput8EnumDevicesArguments?.join(",") !==
      "game-controller,0x1005bef0,0x00000000,attached-only" ||
    latestEvidence.nativeDirectInput8EnumDevicesResult !== "0x00000000" ||
    latestEvidence.nativeDirectInput8EnumDevicesLastError !== 0 ||
    latestEvidence.nativeDirectInput8EnumDevicesCallbackCount !== 0 ||
    latestEvidence.nativeDirectInput8EnumDevicesConsumer !==
      "callback-would-link-device-entry-then-caller-walks-empty-list" ||
    latestEvidence.nativeDirectInput8EnumDevicesExecutableHashUnchanged !==
      true ||
    latestEvidence.directInput8EnumDevicesOracleArchitecture !== "coff-i386" ||
    latestEvidence.directInput8EnumDevicesOracleSourceBytes !== 10_528 ||
    latestEvidence.directInput8EnumDevicesOracleSourceSha256 !==
      "2a255635c7f5f5b27cae442cead6eb050f4f63e4306237ad13b1001b2aa4d908" ||
    latestEvidence.directInput8EnumDevicesOracleExecutableBytes !== 10_752 ||
    latestEvidence.directInput8EnumDevicesOracleExecutableSha256 !==
      "ef3eabd2738fb4723b3c96f78798e52c73e556a9ec73c8e12dc7953d133c46da" ||
    latestEvidence.directInput8EnumDevicesOraclePrimaryDeviceClass !== 4 ||
    latestEvidence.directInput8EnumDevicesOraclePrimaryFlags !== "0x00000001" ||
    latestEvidence.directInput8EnumDevicesOraclePrimaryResult !==
      "0x00000000" ||
    latestEvidence.directInput8EnumDevicesOraclePrimaryCallbackCount !== 0 ||
    latestEvidence.directInput8EnumDevicesOracleReachedCallbackCounts?.join(
      ",",
    ) !== "1,1,1,1" ||
    latestEvidence.directInput8EnumDevicesOracleReachedCallbackClasses?.join(
      ",",
    ) !== "3,2,0,1" ||
    latestEvidence.directInput8EnumDevicesOracleInstanceBytes !== 580 ||
    latestEvidence.directInput8EnumDevicesOracleCallbackContextForwarded !==
      true ||
    latestEvidence.directInput8EnumDevicesOracleInvalidResult !==
      "0x80070057" ||
    latestEvidence.directInput8EnumDevicesOracleInvalidCallbackCount !== 0 ||
    latestEvidence.directInput8EnumDevicesOracleInvalidLastErrorPreserved !==
      true ||
    latestEvidence.directInput8EnumDevicesImplemented !== true ||
    latestEvidence.directInput8EnumDevicesSequence !== 18_443 ||
    latestEvidence.directInput8EnumDevicesInstructionCount !== 11_825_229 ||
    latestEvidence.directInput8EnumDevicesArguments?.join(",") !==
      "0x01060004,0x00000004,0x1005bef0,0x00000000,0x00000001" ||
    latestEvidence.directInput8EnumDevicesReturnAddress !== "0x1005c1ce" ||
    latestEvidence.directInput8EnumDevicesResult !== "0x00000000" ||
    latestEvidence.directInput8EnumDevicesLastError !== 0 ||
    latestEvidence.directInput8EnumDevicesGuestWriteBytes !== 0 ||
    latestEvidence.directInput8EnumDevicesCallbackInvocations !== 0 ||
    latestEvidence.directInput8EnumDevicesDeviceCount !== 0 ||
    latestEvidence.directInput8EnumDevicesReferenceCount !== 1 ||
    latestEvidence.directInput8EnumDevicesObjectRetired !== false ||
    latestEvidence.directInput8EnumDevicesPolicySchema !==
      "vine.dinput8-compatibility-policy" ||
    latestEvidence.directInput8EnumDevicesPolicySchemaVersion !== 1 ||
    latestEvidence.directInput8EnumDevicesPolicyProfileId !==
      "empty-attached-game-controller-enumeration-v1" ||
    latestEvidence.directInput8EnumDevicesPolicyDeviceClass !== 4 ||
    latestEvidence.directInput8EnumDevicesPolicyFlags !== "0x00000001" ||
    latestEvidence.directInput8EnumDevicesStateMutation !== false ||
    latestEvidence.directInput8EnumDevicesFailureAtomicity !== true ||
    latestEvidence.directInput8EnumDevicesValidUnreachedVariantsStructuredUnsupported !==
      true ||
    latestEvidence.directInput8EnumDevicesInvalidArgumentsReturnInvalidArg !==
      true ||
    latestEvidence.directInput8EnumDevicesCallbackProducingVariantsImplemented !==
      false ||
    latestEvidence.directInput8EnumDevicesBrowserInputImported !== false ||
    latestEvidence.directInput8EnumDevicesNodeCompletions !== 1 ||
    latestEvidence.directInput8EnumDevicesChromeCompletions !== 1 ||
    latestEvidence.directInput8EnumDevicesNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest DirectInput 8 EnumDevices evidence is invalid");
  }
  for (const field of [
    "nativeDirectSoundRootPrimaryTraceSha256",
    "nativeDirectSoundRootConfirmationTraceSha256",
    "directSoundRootOracleSourceSha256",
    "directSoundRootOracleExecutableSha256",
    "directSoundCreate8HostDeltaPayloadSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeDirectSoundRootPrimaryTraceBytes !== 275_508 ||
    latestEvidence.nativeDirectSoundRootPrimaryTraceLines !== 794 ||
    latestEvidence.nativeDirectSoundRootPrimaryTraceSha256 !==
      "58cc3787dfad88a3fcfc14dbd66238a69fed0ee9605cbedcc70b6586d9e8ece4" ||
    latestEvidence.nativeDirectSoundRootConfirmationTraceBytes !== 276_375 ||
    latestEvidence.nativeDirectSoundRootConfirmationTraceLines !== 797 ||
    latestEvidence.nativeDirectSoundRootConfirmationTraceSha256 !==
      "48e6026581b4e726edad4443ed932bdee437829bfb5ce1f96fb7ee560062189f" ||
    latestEvidence.nativeDirectSoundRootTraceDiagnostics !== 0 ||
    latestEvidence.nativeDirectSoundRootProfile !==
      "x86-bgi-direct-sound-8-root-v1" ||
    latestEvidence.nativeDirectSoundRootFactoryCallsPerTrace !== 1 ||
    latestEvidence.nativeDirectSoundRootFactoryCallerRva !== "0x0009cccc" ||
    latestEvidence.nativeDirectSoundRootFactoryArguments?.join(",") !==
      "default-device,non-null-output,null-outer" ||
    latestEvidence.nativeDirectSoundRootFactoryResult !== "0x00000000" ||
    latestEvidence.nativeDirectSoundRootFactoryOutputNonNull !== true ||
    latestEvidence.nativeDirectSoundRootFactoryLastError !== 0 ||
    latestEvidence.nativeDirectSoundRootFactoryConsumer !==
      "test-hresult-sign-and-use-idirectsound8-root" ||
    latestEvidence.nativeDirectSoundRootInterface !== "IDirectSound8" ||
    latestEvidence.nativeDirectSoundRootVtableSlots !== 12 ||
    latestEvidence.nativeDirectSoundRootFirstMethod !== "SetCooperativeLevel" ||
    latestEvidence.nativeDirectSoundRootFirstMethodSlot !== 6 ||
    latestEvidence.nativeDirectSoundRootFirstMethodCallerRva !== "0x0009cd0e" ||
    latestEvidence.nativeDirectSoundRootFirstMethodArguments?.join(",") !==
      "live-top-level-window,0x00000002" ||
    latestEvidence.nativeDirectSoundRootFirstMethodResult !== "0x00000000" ||
    latestEvidence.nativeDirectSoundRootFirstMethodLastError !== 14_007 ||
    latestEvidence.nativeDirectSoundRootFirstMethodConsumer !==
      "test-hresult-sign-and-continue-to-buffer-creation" ||
    latestEvidence.nativeDirectSoundRootExecutableHashUnchanged !== true ||
    latestEvidence.directSoundRootOracleArchitecture !== "coff-i386" ||
    latestEvidence.directSoundRootOracleSourceBytes !== 10_848 ||
    latestEvidence.directSoundRootOracleSourceSha256 !==
      "2d099f107eb012b1ee4828ba3fff6df999c844e442b027d71aeb454c547579ea" ||
    latestEvidence.directSoundRootOracleExecutableBytes !== 7_680 ||
    latestEvidence.directSoundRootOracleExecutableSha256 !==
      "6d8d9296987643fa01702c0a0a96f7379e59f172269100e95a44a95f5186c51e" ||
    latestEvidence.directSoundRootOracleValidFactoryResult !== "0x00000000" ||
    latestEvidence.directSoundRootOracleAggregationResult !== "0x80040110" ||
    latestEvidence.directSoundRootOracleAggregationPreservesOutput !== true ||
    latestEvidence.directSoundRootOracleNullOutputResult !== "0x80070057" ||
    latestEvidence.directSoundRootOracleIunknownDistinctPointer !== true ||
    latestEvidence.directSoundRootOracleLegacyAndVersion8SamePointer !== true ||
    latestEvidence.directSoundRootOracleUnknownIidResult !== "0x80004002" ||
    latestEvidence.directSoundRootOracleUnknownIidClearsOutput !== true ||
    latestEvidence.directSoundRootOracleValidCooperativeLevels?.join(",") !==
      "1,2" ||
    latestEvidence.directSoundRootOracleInvalidLevelResult !== "0x80070057" ||
    latestEvidence.directSoundRootOracleNullWindowResult !== "0x80070057" ||
    latestEvidence.directSoundRootOracleInvalidNonzeroWindowObservedResult !==
      "0x00000000" ||
    latestEvidence.directSoundRootOracleInvalidNonzeroWindowLastError !==
      1_400 ||
    latestEvidence.directSoundCreate8Implemented !== true ||
    latestEvidence.directSoundCreate8Ordinal !== 11 ||
    latestEvidence.directSoundCreate8Sequence !== 18_453 ||
    latestEvidence.directSoundCreate8InstructionCount !== 11_921_739 ||
    latestEvidence.directSoundCreate8Arguments?.join(",") !==
      "0x00000000,0x100feb10,0x00000000" ||
    latestEvidence.directSoundCreate8ReturnAddress !== "0x1009cccc" ||
    latestEvidence.directSoundCreate8Result !== "0x00000000" ||
    latestEvidence.directSoundCreate8LastError !== 0 ||
    latestEvidence.directSoundCreate8GuestWriteBytes !== 4 ||
    latestEvidence.directSoundCreate8OutputAddress !== "0x100feb10" ||
    latestEvidence.directSoundCreate8OutputPointer !== "0x01070004" ||
    latestEvidence.directSoundCreate8ImplementationId !==
      "dsound.direct-sound-create-8.v1" ||
    latestEvidence.directSoundCreate8ComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.directSoundCreate8AudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    latestEvidence.directSoundCreate8ClassIdentity !==
      "dsound.idirectsound8-root.v1" ||
    latestEvidence.directSoundCreate8Interface !== "IDirectSound8" ||
    latestEvidence.directSoundCreate8InterfaceIidBytes?.join(",") !==
      "147,126,10,197,149,243,52,72,158,246,127,169,157,229,9,102" ||
    latestEvidence.directSoundCreate8VtableSlots !== 12 ||
    latestEvidence.directSoundCreate8ObjectId !== 10 ||
    latestEvidence.directSoundCreate8InterfaceId !== 20 ||
    latestEvidence.directSoundCreate8ReferenceCount !== 1 ||
    latestEvidence.directSoundCreate8BackingAddress !== "0x01070000" ||
    latestEvidence.directSoundCreate8BackingBytes !== 8_192 ||
    latestEvidence.directSoundCreate8HostDeltaFromGeneration !== 36_120 ||
    latestEvidence.directSoundCreate8HostDeltaGeneration !== 36_147 ||
    latestEvidence.directSoundCreate8HostDeltaMappingOperations !== 5 ||
    latestEvidence.directSoundCreate8HostDeltaDirtyPages?.join(",") !==
      "0x01070000:23,0x01071000:23,0x100fe000:1" ||
    latestEvidence.directSoundCreate8HostDeltaPayloadSha256 !==
      "6f167f178ff1014579575cb8e3bbc4daa6b2b5ef6264569ba795d35beb1665a2" ||
    latestEvidence.directSoundCreate8SupportsIunknown !== true ||
    latestEvidence.directSoundCreate8LegacyInterfaceImplemented !== false ||
    latestEvidence.directSoundCreate8GuestComAudioAtomicity !== true ||
    latestEvidence.directSoundCreate8FailureAtomicity !== true ||
    latestEvidence.directSoundCreate8BrowserOwnsComOrAudioState !== false ||
    latestEvidence.directSoundCreate8NodeCompletions !== 1 ||
    latestEvidence.directSoundCreate8ChromeCompletions !== 1 ||
    latestEvidence.directSoundCreate8NodeChromeOrderedSemanticsMatched !==
      true ||
    latestEvidence.directSoundSetCooperativeLevelImplemented !== true ||
    latestEvidence.directSoundSetCooperativeLevelSequence !== 18_454 ||
    latestEvidence.directSoundSetCooperativeLevelInstructionCount !==
      11_921_749 ||
    latestEvidence.directSoundSetCooperativeLevelArguments?.join(",") !==
      "0x01070004,0x48010001,0x00000002" ||
    latestEvidence.directSoundSetCooperativeLevelReturnAddress !==
      "0x1009cd0e" ||
    latestEvidence.directSoundSetCooperativeLevelResult !== "0x00000000" ||
    latestEvidence.directSoundSetCooperativeLevelGuestWriteBytes !== 0 ||
    latestEvidence.directSoundSetCooperativeLevelReferenceCount !== 1 ||
    latestEvidence.directSoundSetCooperativeLevelObjectRetired !== false ||
    latestEvidence.directSoundSetCooperativeLevelPolicySchema !==
      "vine.dsound-compatibility-policy" ||
    latestEvidence.directSoundSetCooperativeLevelPolicySchemaVersion !== 1 ||
    latestEvidence.directSoundSetCooperativeLevelPolicyId !==
      "dsound-primary-default-device-v1" ||
    latestEvidence.directSoundSetCooperativeLevelProfileId !==
      "primary-live-window-priority-v1" ||
    latestEvidence.directSoundSetCooperativeLevelLevel !== 2 ||
    latestEvidence.directSoundSetCooperativeLevelWindowHandle !==
      "0x48010001" ||
    latestEvidence.directSoundSetCooperativeLevelRequiresLiveTopLevelWindow !==
      true ||
    latestEvidence.directSoundSetCooperativeLevelRequiresOwnerThread !== true ||
    latestEvidence.directSoundSetCooperativeLevelStateMutation !==
      "audio-owned-targeted-plan-commit" ||
    latestEvidence.directSoundSetCooperativeLevelFailureAtomicity !== true ||
    latestEvidence.directSoundSetCooperativeLevelBrowserAudioImported !==
      false ||
    latestEvidence.directSoundSetCooperativeLevelNodeCompletions !== 1 ||
    latestEvidence.directSoundSetCooperativeLevelChromeCompletions !== 1 ||
    latestEvidence.directSoundSetCooperativeLevelNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push(
      "latest DirectSound 8 root and cooperative-level evidence is invalid",
    );
  }
  for (const field of [
    "semanticCallDigestSha256",
    "memoryContentSha256",
    "nativeWaitForSingleObjectExPrimaryTraceSha256",
    "nativeWaitForSingleObjectExConfirmationTraceSha256",
    "nativeDirectSoundInitialBuffersPrimaryTraceSha256",
    "nativeDirectSoundInitialBuffersConfirmationTraceSha256",
    "directSoundBufferOracleSourceSha256",
    "directSoundBufferOracleExecutableSha256",
    "directSoundPrimaryBufferHostDeltaPayloadSha256",
    "directSoundSecondaryBufferHostDeltaPayloadSha256",
    "directSoundBuffer8QueryInterfaceHostDeltaPayloadSha256",
    "nativeDirectSoundBufferTransferPrimaryTraceSha256",
    "nativeDirectSoundBufferTransferConfirmationTraceSha256",
    "directSoundLockOracleSourceSha256",
    "directSoundLockOracleExecutableSha256",
    "directSoundBufferLockHostDeltaPayloadSha256",
    "directSoundBufferUnlockWritebackContentSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.semanticCallDigestAlgorithm !== "sha256-canonical-json-v1" ||
    latestEvidence.semanticCallDigestSha256 !==
      profileExpectations.semanticCallDigestSha256 ||
    latestEvidence.semanticCallDigestSha256 !==
      bgiBrowserEvidence.callSemanticsSha256 ||
    latestEvidence.memoryContentSha256 !==
      profileExpectations.memoryContentSha256 ||
    latestEvidence.nativeDirectSoundInitialBuffersPrimaryTraceBytes !==
      382_086 ||
    latestEvidence.nativeDirectSoundInitialBuffersPrimaryTraceLines !== 801 ||
    latestEvidence.nativeDirectSoundInitialBuffersPrimaryTraceSha256 !==
      "6d924745fb6b61efa7992ec4ccda6cd553bab625348b0734876216e12af37f33" ||
    latestEvidence.nativeDirectSoundInitialBuffersConfirmationTraceBytes !==
      382_124 ||
    latestEvidence.nativeDirectSoundInitialBuffersConfirmationTraceLines !==
      801 ||
    latestEvidence.nativeDirectSoundInitialBuffersConfirmationTraceSha256 !==
      "adb5fc01b96004ca5b571fa7dbfff5008f79c02d6ae34d37d5068fc830be669c" ||
    latestEvidence.nativeDirectSoundInitialBuffersTraceDiagnostics !== 0 ||
    latestEvidence.nativeDirectSoundInitialBuffersProfile !==
      "x86-bgi-direct-sound-create-buffer-v1" ||
    latestEvidence.nativeDirectSoundInitialBuffersCallsPerTrace !== 7 ||
    latestEvidence.nativeDirectSoundInitialBuffersOrderedMethods?.join(",") !==
      "IDirectSound8::CreateSoundBuffer(primary),IDirectSoundBuffer8::SetFormat,IDirectSoundBuffer8::Play,IDirectSound8::CreateSoundBuffer(secondary),IDirectSoundBuffer8::QueryInterface,IDirectSoundBuffer8::Release,IDirectSoundBuffer8::Lock" ||
    latestEvidence.nativeDirectSoundPrimaryBufferCallerRva !== "0x0009c916" ||
    latestEvidence.nativeDirectSoundPrimaryBufferDescriptorBytes !== 36 ||
    latestEvidence.nativeDirectSoundPrimaryBufferFlags !== "0x00000001" ||
    latestEvidence.nativeDirectSoundPrimaryBufferBytes !== 0 ||
    latestEvidence.nativeDirectSoundPrimaryBufferFormatPointerNull !== true ||
    latestEvidence.nativeDirectSoundPrimarySetFormatCallerRva !==
      "0x0009c988" ||
    latestEvidence.nativeDirectSoundPrimaryPlayCallerRva !== "0x0009c99b" ||
    latestEvidence.nativeDirectSoundPrimaryPlayFlags !== "0x00000001" ||
    latestEvidence.nativeDirectSoundSecondaryBufferCallerRva !== "0x0009ca37" ||
    latestEvidence.nativeDirectSoundSecondaryBufferDescriptorBytes !== 36 ||
    latestEvidence.nativeDirectSoundSecondaryBufferFlags !== "0x000080a8" ||
    latestEvidence.nativeDirectSoundSecondaryBufferBytes !== 88_200 ||
    latestEvidence.nativeDirectSoundPcmFormat?.join(",") !==
      "1,2,44100,176400,4,16" ||
    latestEvidence.nativeDirectSoundBuffer8Iid !==
      "6825a449-7524-4d82-920f-50e36ab3ab1e" ||
    latestEvidence.nativeDirectSoundBuffer8QueryInterfaceSamePointer !== true ||
    latestEvidence.nativeDirectSoundBuffer8ReleaseResult !== 1 ||
    latestEvidence.nativeDirectSoundBuffer8LockCallerRva !== "0x0009cab2" ||
    latestEvidence.nativeDirectSoundBuffer8LockOffset !== 0 ||
    latestEvidence.nativeDirectSoundBuffer8LockBytes !== 88_200 ||
    latestEvidence.nativeDirectSoundBuffer8LockFlags !== 0 ||
    latestEvidence.nativeDirectSoundBuffer8LockFirstSegmentNonNull !== true ||
    latestEvidence.nativeDirectSoundBuffer8LockFirstSegmentBytes !== 88_200 ||
    latestEvidence.nativeDirectSoundBuffer8LockSecondSegmentNullAndZero !==
      true ||
    latestEvidence.nativeDirectSoundInitialBuffersExecutableHashUnchanged !==
      true ||
    latestEvidence.directSoundBufferOracleArchitecture !== "coff-i386" ||
    latestEvidence.directSoundBufferOracleSourceBytes !== 11_546 ||
    latestEvidence.directSoundBufferOracleSourceSha256 !==
      "2a4a5a2607d48f62ac56b317164a55bf6db4e74d871f537353d926c54dd7d462" ||
    latestEvidence.directSoundBufferOracleExecutableBytes !== 6_656 ||
    latestEvidence.directSoundBufferOracleExecutableSha256 !==
      "52b332822134f18dcfd8e0ecb5329187eccddcf96082a9c383311370a69b71d2" ||
    latestEvidence.directSoundBufferOracleInvalidDescriptorResult !==
      "0x80070057" ||
    latestEvidence.directSoundBufferOracleInvalidDescriptorPreservesOutput !==
      true ||
    latestEvidence.directSoundBufferOracleAggregationResult !== "0x80040110" ||
    latestEvidence.directSoundBufferOracleAggregationPreservesOutput !== true ||
    latestEvidence.directSoundBufferOraclePrimarySetFormatResult !==
      "0x00000000" ||
    latestEvidence.directSoundBufferOraclePrimaryLoopingPlayResult !==
      "0x00000000" ||
    latestEvidence.directSoundBufferOraclePrimaryNonLoopingPlayResult !==
      "0x80070057" ||
    latestEvidence.directSoundBufferOracleSecondarySetFormatResult !==
      "0x88780032" ||
    latestEvidence.directSoundBufferOracleSecondarySetFormatResultName !==
      "DSERR_INVALIDCALL" ||
    latestEvidence.directSoundBufferOracleBuffer8QueryInterfaceSamePointer !==
      true ||
    latestEvidence.directSoundInitialBuffersImplemented !== true ||
    latestEvidence.directSoundInitialBuffersPolicySchema !==
      "vine.dsound-compatibility-policy" ||
    latestEvidence.directSoundInitialBuffersPolicySchemaVersion !== 1 ||
    latestEvidence.directSoundInitialBuffersPolicyId !==
      "dsound-primary-default-device-v1" ||
    latestEvidence.directSoundInitialBuffersProfileId !==
      "primary-stereo-pcm-and-secondary-88200-v1" ||
    latestEvidence.directSoundInitialBuffersAudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    latestEvidence.directSoundInitialBuffersComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.directSoundInitialBuffersGuestComAudioAtomicity !== true ||
    latestEvidence.directSoundInitialBuffersFailureAtomicity !== true ||
    latestEvidence.directSoundInitialBuffersBrowserOwnsComOrAudioState !==
      false ||
    latestEvidence.directSoundPrimaryBufferSequence !== 18_455 ||
    latestEvidence.directSoundPrimaryBufferInstructionCount !== 11_921_781 ||
    latestEvidence.directSoundPrimaryBufferArguments?.join(",") !==
      "0x01070004,0x100feae8,0x100feb18,0x00000000" ||
    latestEvidence.directSoundPrimaryBufferReturnAddress !== "0x1009c916" ||
    latestEvidence.directSoundPrimaryBufferResult !== "0x00000000" ||
    latestEvidence.directSoundPrimaryBufferOutputPointer !== "0x01080004" ||
    latestEvidence.directSoundPrimaryBufferObjectId !== 11 ||
    latestEvidence.directSoundPrimaryBufferInterfaceId !== 22 ||
    latestEvidence.directSoundPrimaryBufferLogicalAudioBufferId !== 1 ||
    latestEvidence.directSoundPrimaryBufferBackingAddress !== "0x01080000" ||
    latestEvidence.directSoundPrimaryBufferBackingBytes !== 8_192 ||
    latestEvidence.directSoundPrimaryBufferHostDeltaFromGeneration !== 36_149 ||
    latestEvidence.directSoundPrimaryBufferHostDeltaGeneration !== 36_187 ||
    latestEvidence.directSoundPrimaryBufferHostDeltaMappingOperations !== 5 ||
    latestEvidence.directSoundPrimaryBufferHostDeltaDirtyPages?.join(",") !==
      "0x01080000:24,0x01081000:24,0x100fe000:1" ||
    latestEvidence.directSoundPrimaryBufferHostDeltaPayloadSha256 !==
      "8fe4c8666bb45ab4a98a5e8ea80b0549ee858ec12866e9ee4d6f0a0262fc9fe9" ||
    latestEvidence.directSoundPrimarySetFormatSequence !== 18_456 ||
    latestEvidence.directSoundPrimarySetFormatInstructionCount !== 11_921_800 ||
    latestEvidence.directSoundPrimarySetFormatArguments?.join(",") !==
      "0x01080004,0x006ffe58" ||
    latestEvidence.directSoundPrimarySetFormatReturnAddress !== "0x1009c988" ||
    latestEvidence.directSoundPrimarySetFormatResult !== "0x00000000" ||
    latestEvidence.directSoundPrimaryPlaySequence !== 18_457 ||
    latestEvidence.directSoundPrimaryPlayInstructionCount !== 11_921_809 ||
    latestEvidence.directSoundPrimaryPlayArguments?.join(",") !==
      "0x01080004,0x00000000,0x00000000,0x00000001" ||
    latestEvidence.directSoundPrimaryPlayReturnAddress !== "0x1009c99b" ||
    latestEvidence.directSoundPrimaryPlayResult !== "0x00000000" ||
    latestEvidence.directSoundPrimaryPlaybackState !== "looping" ||
    latestEvidence.directSoundSecondaryBufferSequence !== 18_458 ||
    latestEvidence.directSoundSecondaryBufferInstructionCount !== 11_921_862 ||
    latestEvidence.directSoundSecondaryBufferArguments?.join(",") !==
      "0x01070004,0x006ffe34,0x006ffe24,0x00000000" ||
    latestEvidence.directSoundSecondaryBufferReturnAddress !== "0x1009ca37" ||
    latestEvidence.directSoundSecondaryBufferResult !== "0x00000000" ||
    latestEvidence.directSoundSecondaryBufferOutputPointer !== "0x01090004" ||
    latestEvidence.directSoundSecondaryBufferObjectId !== 12 ||
    latestEvidence.directSoundSecondaryBufferInterfaceId !== 24 ||
    latestEvidence.directSoundSecondaryBufferLogicalAudioBufferId !== 2 ||
    latestEvidence.directSoundSecondaryBufferBackingAddress !== "0x01090000" ||
    latestEvidence.directSoundSecondaryBufferBackingBytes !== 8_192 ||
    latestEvidence.directSoundSecondaryBufferHostDeltaFromGeneration !==
      36_190 ||
    latestEvidence.directSoundSecondaryBufferHostDeltaGeneration !== 36_228 ||
    latestEvidence.directSoundSecondaryBufferHostDeltaMappingOperations !== 5 ||
    latestEvidence.directSoundSecondaryBufferHostDeltaDirtyPages?.join(",") !==
      "0x006ff000:4,0x01090000:25,0x01091000:25" ||
    latestEvidence.directSoundSecondaryBufferHostDeltaPayloadSha256 !==
      "61691975341102def2aae557fa743a195fb8f0fd4d61c0e93630e2abbbfeff5b" ||
    latestEvidence.directSoundBuffer8QueryInterfaceSequence !== 18_459 ||
    latestEvidence.directSoundBuffer8QueryInterfaceInstructionCount !==
      11_921_872 ||
    latestEvidence.directSoundBuffer8QueryInterfaceArguments?.join(",") !==
      "0x01090004,0x100d2228,0x100feb14" ||
    latestEvidence.directSoundBuffer8QueryInterfaceReturnAddress !==
      "0x1009ca82" ||
    latestEvidence.directSoundBuffer8QueryInterfaceResult !== "0x00000000" ||
    latestEvidence.directSoundBuffer8QueryInterfaceOutputPointer !==
      "0x01090004" ||
    latestEvidence.directSoundBuffer8QueryInterfaceReferenceCount !== 2 ||
    latestEvidence.directSoundBuffer8QueryInterfaceHostDeltaFromGeneration !==
      36_229 ||
    latestEvidence.directSoundBuffer8QueryInterfaceHostDeltaGeneration !==
      36_230 ||
    latestEvidence.directSoundBuffer8QueryInterfaceHostDeltaPayloadSha256 !==
      "6983da45720d05c632ba127e2fab52a41242391a41c7166ac0a3258cb35e6fae" ||
    latestEvidence.directSoundBuffer8ReleaseSequence !== 18_460 ||
    latestEvidence.directSoundBuffer8ReleaseInstructionCount !== 11_921_878 ||
    latestEvidence.directSoundBuffer8ReleaseArguments?.join(",") !==
      "0x01090004" ||
    latestEvidence.directSoundBuffer8ReleaseReturnAddress !== "0x1009ca8d" ||
    latestEvidence.directSoundBuffer8ReleaseResult !== 1 ||
    latestEvidence.directSoundBuffer8ReferenceCountAtStop !== 1 ||
    latestEvidence.directSoundInitialBuffersNodeCompletions !== 6 ||
    latestEvidence.directSoundInitialBuffersChromeCompletions !== 6 ||
    latestEvidence.directSoundInitialBuffersNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest DirectSound initial-buffer evidence is invalid");
  }
  if (
    latestEvidence.nativeDirectSoundBufferTransferPrimaryTraceBytes !==
      425_777 ||
    latestEvidence.nativeDirectSoundBufferTransferPrimaryTraceLines !== 847 ||
    latestEvidence.nativeDirectSoundBufferTransferPrimaryTraceSha256 !==
      "c0993be13360eb7515c6ceac90b228b58943c818d5973d53131929c51fe6d72d" ||
    latestEvidence.nativeDirectSoundBufferTransferConfirmationTraceBytes !==
      424_704 ||
    latestEvidence.nativeDirectSoundBufferTransferConfirmationTraceLines !==
      848 ||
    latestEvidence.nativeDirectSoundBufferTransferConfirmationTraceSha256 !==
      "76fab2a79d5ca148209f6ce9df060650f67d9f6ab7f79de88d6ea6d3eb60dbda" ||
    latestEvidence.nativeDirectSoundBufferTransferTraceDiagnostics !== 0 ||
    latestEvidence.nativeDirectSoundBufferTransferProfile !==
      "x86-bgi-direct-sound-buffer-lock-unlock-play-v1" ||
    latestEvidence.nativeDirectSoundBufferTransferOrderedMethods?.join(",") !==
      "IDirectSoundBuffer8::Lock,IDirectSoundBuffer8::Unlock,IDirectSoundBuffer8::Play" ||
    latestEvidence.nativeDirectSoundBufferTransferLockCallerRva !==
      "0x0009cab2" ||
    latestEvidence.nativeDirectSoundBufferTransferLockArguments?.join(",") !==
      "0x00000000,0x00015888,non-null-output,non-null-output,non-null-output,non-null-output,0x00000000" ||
    latestEvidence.nativeDirectSoundBufferTransferLockResult !== "0x00000000" ||
    latestEvidence.nativeDirectSoundBufferTransferFirstSegmentBytes !==
      88_200 ||
    latestEvidence.nativeDirectSoundBufferTransferSecondSegmentNullAndZero !==
      true ||
    latestEvidence.nativeDirectSoundBufferTransferUnlockCallerRva !==
      "0x0009caf3" ||
    latestEvidence.nativeDirectSoundBufferTransferUnlockResult !==
      "0x00000000" ||
    latestEvidence.nativeDirectSoundBufferTransferUnlockFirstSegmentZeroBytes !==
      88_200 ||
    latestEvidence.nativeDirectSoundBufferTransferUnlockFirstSegmentFnv1a32 !==
      "0xcb8d2b65" ||
    latestEvidence.nativeDirectSoundBufferTransferPlayCallerRva !==
      "0x0009cb06" ||
    latestEvidence.nativeDirectSoundBufferTransferPlayFlags !== "0x00000001" ||
    latestEvidence.nativeDirectSoundBufferTransferPlayResult !== "0x00000000" ||
    latestEvidence.nativeDirectSoundBufferTransferConsumer !==
      "unlock-then-looping-play-and-return-success" ||
    latestEvidence.nativeDirectSoundBufferTransferExecutableHashUnchanged !==
      true ||
    latestEvidence.directSoundLockOracleArchitecture !== "coff-i386" ||
    latestEvidence.directSoundLockOracleSourceBytes !== 11_532 ||
    latestEvidence.directSoundLockOracleSourceSha256 !==
      "84596e1660f8c8e8701e5b94d4d9ab8683688ae81ced4de987d3982fdbcaf287" ||
    latestEvidence.directSoundLockOracleExecutableBytes !== 6_656 ||
    latestEvidence.directSoundLockOracleExecutableSha256 !==
      "1615936f0f42ec6352c0c54a9a0373da0775b7837312eac3a0479678a0717d19" ||
    latestEvidence.directSoundLockOracleLastErrorSentinel !== 23_130 ||
    latestEvidence.directSoundLockOracleNullData1Result !== "0x80070057" ||
    latestEvidence.directSoundLockOracleNullData1PreservesOutputs !== true ||
    latestEvidence.directSoundLockOracleNullBytes1Result !== "0x80070057" ||
    latestEvidence.directSoundLockOracleNullBytes1ZerosData1 !== true ||
    latestEvidence.directSoundLockOracleValidResult !== "0x00000000" ||
    latestEvidence.directSoundLockOracleValidFirstSegmentBytes !== 88_200 ||
    latestEvidence.directSoundLockOracleValidSecondSegmentNullAndZero !==
      true ||
    latestEvidence.directSoundLockOracleDoubleLockResult !== "0x80070057" ||
    latestEvidence.directSoundLockOracleUnlockBeforeLockResult !==
      "0x00000000" ||
    latestEvidence.directSoundLockOracleMismatchedUnlockResult !==
      "0x00000000" ||
    latestEvidence.directSoundLockOracleMismatchedUnlockConsumesLock !== true ||
    latestEvidence.directSoundLockOracleExactUnlockAfterMismatchResult !==
      "0x80070057" ||
    latestEvidence.directSoundLockOracleRepeatedUnlockResult !== "0x80070057" ||
    latestEvidence.directSoundLockOracleLoopingPlayResult !== "0x00000000" ||
    latestEvidence.directSoundBufferTransferImplemented !== true ||
    latestEvidence.directSoundBufferTransferPolicySchema !==
      "vine.dsound-compatibility-policy" ||
    latestEvidence.directSoundBufferTransferPolicySchemaVersion !== 1 ||
    latestEvidence.directSoundBufferTransferPolicyId !==
      "dsound-primary-default-device-v1" ||
    latestEvidence.directSoundBufferTransferProfileId !==
      "primary-secondary-full-lock-looping-play-v1" ||
    latestEvidence.directSoundBufferTransferAudioAuthority !==
      "runtime/audio.rs::RuntimeAudio" ||
    latestEvidence.directSoundBufferTransferComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.directSoundBufferTransferMemoryAuthority !==
      "rust-guest-memory" ||
    latestEvidence.directSoundBufferTransferAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    latestEvidence.directSoundBufferTransferGuestMemoryAudioAtomicity !==
      true ||
    latestEvidence.directSoundBufferTransferFailureAtomicity !== true ||
    latestEvidence.directSoundBufferTransferExactUnlockSegmentsRequired !==
      true ||
    latestEvidence.directSoundBufferTransferNativeLooseUnlockGeneralized !==
      false ||
    latestEvidence.directSoundBufferTransferBrowserOwnsComOrAudioState !==
      false ||
    latestEvidence.directSoundBufferLockSequence !== 18_461 ||
    latestEvidence.directSoundBufferLockInstructionCount !== 11_921_896 ||
    latestEvidence.directSoundBufferLockArguments?.join(",") !==
      "0x01090004,0x00000000,0x00015888,0x006ffe2c,0x006ffe30,0x006ffe20,0x006ffe28,0x00000000" ||
    latestEvidence.directSoundBufferLockReturnAddress !== "0x1009cab2" ||
    latestEvidence.directSoundBufferLockResult !== "0x00000000" ||
    latestEvidence.directSoundBufferLockGuestWriteBytes !== 16 ||
    latestEvidence.directSoundBufferLockOutputValues?.join(",") !==
      "0x45000000,88200,0x00000000,0" ||
    latestEvidence.directSoundBufferLockSampleBackingAddress !== "0x45000000" ||
    latestEvidence.directSoundBufferLockSampleBackingUsableBytes !== 88_200 ||
    latestEvidence.directSoundBufferLockSampleBackingReservedBytes !== 90_112 ||
    latestEvidence.directSoundBufferLockSampleBackingMemoryOwner !== 26 ||
    latestEvidence.directSoundBufferLockBackingProtection !== "read-write" ||
    latestEvidence.directSoundBufferLockHostDeltaFromGeneration !== 36_232 ||
    latestEvidence.directSoundBufferLockHostDeltaGeneration !== 36_238 ||
    latestEvidence.directSoundBufferLockHostDeltaMappingOperations !== 2 ||
    latestEvidence.directSoundBufferLockHostDeltaDirtyPages !== 23 ||
    latestEvidence.directSoundBufferLockHostDeltaPayloadSha256 !==
      "0d791cb0e27d7edf9580170880bb87155c2dd11b7818609b172d5f4c062a4de6" ||
    latestEvidence.directSoundBufferUnlockSequence !== 18_462 ||
    latestEvidence.directSoundBufferUnlockInstructionCount !== 11_929_559 ||
    latestEvidence.directSoundBufferUnlockArguments?.join(",") !==
      "0x01090004,0x45000000,0x00015888,0x00000000,0x00000000" ||
    latestEvidence.directSoundBufferUnlockReturnAddress !== "0x1009caf3" ||
    latestEvidence.directSoundBufferUnlockResult !== "0x00000000" ||
    latestEvidence.directSoundBufferUnlockWritebackTouchedPages !== 23 ||
    latestEvidence.directSoundBufferUnlockWritebackChangedPages !== 1 ||
    latestEvidence.directSoundBufferUnlockWritebackUnchangedSamplePages !==
      22 ||
    latestEvidence.directSoundBufferUnlockWritebackChangedBytes !== 22 ||
    latestEvidence.directSoundBufferUnlockWritebackContentSha256 !==
      "7b5baf93dae387ed6c806d22add656648fc92cb53e4d2e71f3330f374aa43d5e" ||
    latestEvidence.directSoundBufferUnlockContentGeneration !== 1 ||
    latestEvidence.directSoundBufferUnlockBackingProtection !== "read-only" ||
    latestEvidence.directSoundBufferUnlockHostDeltaFromGeneration !== 36_239 ||
    latestEvidence.directSoundBufferUnlockHostDeltaGeneration !== 36_240 ||
    latestEvidence.directSoundBufferUnlockHostDeltaMappingOperations !== 1 ||
    latestEvidence.directSoundSecondaryPlaySequence !== 18_463 ||
    latestEvidence.directSoundSecondaryPlayInstructionCount !== 11_929_568 ||
    latestEvidence.directSoundSecondaryPlayArguments?.join(",") !==
      "0x01090004,0x00000000,0x00000000,0x00000001" ||
    latestEvidence.directSoundSecondaryPlayReturnAddress !== "0x1009cb06" ||
    latestEvidence.directSoundSecondaryPlayResult !== "0x00000000" ||
    latestEvidence.directSoundSecondaryPlaybackState !== "looping" ||
    latestEvidence.directSoundBufferTransferNodeCompletions !== 3 ||
    latestEvidence.directSoundBufferTransferChromeCompletions !== 3 ||
    latestEvidence.directSoundBufferTransferNodeChromeOrderedSemanticsMatched !==
      true
  ) {
    errors.push("latest DirectSound buffer-transfer evidence is invalid");
  }
  for (const field of [
    "nativeSetTimerPrimaryTraceSha256",
    "nativeSetTimerConfirmationTraceSha256",
    "setTimerOracleSourceSha256",
    "setTimerOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeSetTimerPrimaryTraceBytes !== 1_610_985 ||
    latestEvidence.nativeSetTimerPrimaryTraceLines !== 5_007 ||
    latestEvidence.nativeSetTimerPrimaryTraceSha256 !==
      "d848db2ee4c8b9a356baa1f77325614ca8047c8d8e84738a81f3cc4d60230636" ||
    latestEvidence.nativeSetTimerConfirmationTraceBytes !== 1_611_913 ||
    latestEvidence.nativeSetTimerConfirmationTraceLines !== 5_010 ||
    latestEvidence.nativeSetTimerConfirmationTraceSha256 !==
      "4fa263adc08521a7f82df1b8e586843a8eef4e338cfe5f9d2bdc00f906c1f1ab" ||
    latestEvidence.nativeSetTimerTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetTimerHookErrors !== 0 ||
    latestEvidence.nativeSetTimerProfile !==
      "x86-bgi-live-window-id-1-20ms-timerproc-v1" ||
    latestEvidence.nativeSetTimerCallerReturnRva !== "0x00099d4b" ||
    latestEvidence.nativeSetTimerArguments?.join(",") !==
      "live-window,0x00000001,0x00000014,bgi+0x00099600" ||
    latestEvidence.nativeSetTimerResult !== "0x00000001" ||
    latestEvidence.nativeSetTimerLastErrors?.join(",") !== "0,0" ||
    latestEvidence.nativeSetTimerConsumer !==
      "return-value-discarded-by-zero-eax-wrapper" ||
    latestEvidence.nativeSetTimerCallbacksPerTrace !== 16 ||
    latestEvidence.nativeSetTimerCallbackMessage !== "0x00000113" ||
    latestEvidence.nativeSetTimerCallbackTimerId !== 1 ||
    latestEvidence.nativeSetTimerCallbackOwnerThreadMatches !== true ||
    latestEvidence.nativeSetTimerKillObserved !== false ||
    latestEvidence.nativeSetTimerExecutableHashUnchanged !== true ||
    latestEvidence.setTimerOracleArchitecture !== "coff-i386" ||
    latestEvidence.setTimerOracleSourceBytes !== 11_122 ||
    latestEvidence.setTimerOracleSourceSha256 !==
      "27dab91a5f2d579a4e491f628aae78f420a9920d700a2dd06f0596a7e3bdd894" ||
    latestEvidence.setTimerOracleExecutableBytes !== 8_192 ||
    latestEvidence.setTimerOracleExecutableSha256 !==
      "896cb0ea2adc8f49984a2e8e6faced2977bbec5439ed3c96970f993e85c4fa2f" ||
    latestEvidence.setTimerOracleLastErrorSentinel !== 23_130 ||
    latestEvidence.setTimerOracleLiveWindowResult !== 1 ||
    latestEvidence.setTimerOracleLiveWindowPreservesLastError !== true ||
    latestEvidence.setTimerOracleReplacementPreservesTimerId !== true ||
    latestEvidence.setTimerOracleReplacementStopsOldCallback !== true ||
    latestEvidence.setTimerOracleReplacementCallbackOwnerThreadMatches !==
      true ||
    latestEvidence.setTimerOracleFirstKillResult !== 1 ||
    latestEvidence.setTimerOracleRepeatedKillResult !== 0 ||
    latestEvidence.setTimerOracleKillPreservesLastError !== true ||
    latestEvidence.setTimerOracleInvalidWindowResult !== 0 ||
    latestEvidence.setTimerOracleInvalidWindowLastError !== 1_400 ||
    latestEvidence.setTimerOracleNullWindowAllocatesTimerId !== true ||
    latestEvidence.setTimerOracleNullWindowIgnoresRequestedTimerId !== true ||
    latestEvidence.setTimerOracleLiveWindowZeroIdAllocatesTimerId !== true ||
    latestEvidence.setTimerImplemented !== true ||
    latestEvidence.setTimerImplementationId !==
      "user32.set-timer.live-window-callback-registration.v1" ||
    latestEvidence.setTimerPolicySchema !==
      "vine.user32-compatibility-policy" ||
    latestEvidence.setTimerPolicySchemaVersion !== 19 ||
    latestEvidence.setTimerPolicyId !==
      "user32-single-monitor-work-area-window-input-resources-monitor-info-screen-dc-v19" ||
    latestEvidence.setTimerProfileId !== "main-window-id-1-20ms-timerproc-v1" ||
    latestEvidence.setTimerStateAuthority !==
      "runtime/timers.rs::RuntimeTimers" ||
    latestEvidence.setTimerWindowAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.setTimerNamespace !== "vine.logical-window-timer.v1" ||
    latestEvidence.setTimerLimit !== 4_096 ||
    latestEvidence.setTimerMinimumIntervalMilliseconds !== 10 ||
    latestEvidence.setTimerSequence !== 18_577 ||
    latestEvidence.setTimerInstructionCount !== 11_945_256 ||
    latestEvidence.setTimerArguments?.join(",") !==
      "0x48010001,0x00000001,0x00000014,0x10099600" ||
    latestEvidence.setTimerReturnAddress !== "0x10099d4b" ||
    latestEvidence.setTimerResult !== "0x00000001" ||
    latestEvidence.setTimerLastError !== 0 ||
    latestEvidence.setTimerGuestWriteBytes !== 0 ||
    latestEvidence.setTimerInternalId !== 1 ||
    latestEvidence.setTimerWindowId !== 1 ||
    latestEvidence.setTimerWindowHandle !== "0x48010001" ||
    latestEvidence.setTimerTimerId !== 1 ||
    latestEvidence.setTimerOwnerThreadId !== 8_192 ||
    latestEvidence.setTimerRequestedIntervalMilliseconds !== 20 ||
    latestEvidence.setTimerEffectiveIntervalMilliseconds !== 20 ||
    latestEvidence.setTimerCallbackAddress !== "0x10099600" ||
    latestEvidence.setTimerRegistrationGeneration !== 1 ||
    latestEvidence.setTimerRegistrationOnly !== true ||
    latestEvidence.setTimerCallbackDeliveryImplemented !== false ||
    latestEvidence.setTimerHostClockImported !== false ||
    latestEvidence.setTimerBrowserClockImported !== false ||
    latestEvidence.setTimerFailureAtomicity !== true ||
    latestEvidence.setTimerNodeCompletions !== 1 ||
    latestEvidence.setTimerChromeCompletions !== 1 ||
    latestEvidence.setTimerNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest SetTimer registration evidence is invalid");
  }
  for (const field of [
    "nativeCreateThreadPrimaryTraceSha256",
    "nativeCreateThreadConfirmationTraceSha256",
    "createThreadOracleSourceSha256",
    "createThreadOracleExecutableSha256",
    "createThreadDeltaPayloadSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeCreateThreadPrimaryTraceBytes !== 1_780_985 ||
    latestEvidence.nativeCreateThreadPrimaryTraceLines !== 886 ||
    latestEvidence.nativeCreateThreadPrimaryTraceSha256 !==
      "326bdfe6341df2596ae0ce888cc051fdd957805d30a2a477295c1d3fe3339261" ||
    latestEvidence.nativeCreateThreadConfirmationTraceBytes !== 1_780_897 ||
    latestEvidence.nativeCreateThreadConfirmationTraceLines !== 889 ||
    latestEvidence.nativeCreateThreadConfirmationTraceSha256 !==
      "dc2f8fa5f4138ef04640a92a1cc9134d8b3fc0bba4e88939e8a76550ef393792" ||
    latestEvidence.nativeCreateThreadTraceDiagnostics !== 0 ||
    latestEvidence.nativeCreateThreadHookErrors !== 0 ||
    latestEvidence.nativeCreateThreadProfile !==
      "x86-bgi-suspended-create-resume-v1" ||
    latestEvidence.nativeCreateThreadCallerReturnRva !== "0x000a1fad" ||
    latestEvidence.nativeCreateThreadArguments?.join(",") !==
      "0x00000000,0x00000000,bgi+0x000a1ee8,0x20139940,0x00000004,0x20139940" ||
    latestEvidence.nativeCreateThreadResultNonNull !== true ||
    latestEvidence.nativeCreateThreadOutputTidNonzero !== true ||
    latestEvidence.nativeCreateThreadOutputAliasesParameter !== true ||
    latestEvidence.nativeCreateThreadLastError !== 0 ||
    latestEvidence.nativeCreateThreadConsumer !==
      "store-handle-test-nonzero-resume-and-test-not-minus-one" ||
    latestEvidence.nativeResumeThreadCallerReturnRva !== "0x000a1fbd" ||
    latestEvidence.nativeResumeThreadResult !== 1 ||
    latestEvidence.nativeResumeThreadLastError !== 0 ||
    latestEvidence.nativeCreateThreadWorkerParameterMatches !== true ||
    latestEvidence.nativeCreateThreadWorkerObservesTidAndHandle !== true ||
    latestEvidence.nativeCreateThreadExecutableHashUnchanged !== true ||
    latestEvidence.createThreadOracleArchitecture !== "coff-i386" ||
    latestEvidence.createThreadOracleSourceBytes !== 8_552 ||
    latestEvidence.createThreadOracleSourceSha256 !==
      "8a983c11d492872926a4197462e524e4f0b154832655e5ed15a4f9ba430647e7" ||
    latestEvidence.createThreadOracleExecutableBytes !== 8_192 ||
    latestEvidence.createThreadOracleExecutableSha256 !==
      "1172ed1294e8d39327540052722f81fd24a2868dcadad0ff1056ec52dde2b014" ||
    latestEvidence.createThreadOracleLastErrorSentinel !== 23_130 ||
    latestEvidence.createThreadOracleSuspendedWorkerStartsAfterResume !==
      true ||
    latestEvidence.createThreadOracleResumePreviousSuspendCount !== 1 ||
    latestEvidence.createThreadOracleImmediateWorkerStarts !== true ||
    latestEvidence.createThreadOracleSuccessPreservesLastError !== true ||
    latestEvidence.createThreadOracleOversizedStackResult !== "0x00000000" ||
    latestEvidence.createThreadOracleOversizedStackLastError !== 87 ||
    latestEvidence.createThreadOracleOversizedStackOutputPreserved !== true ||
    latestEvidence.createThreadOracleInvalidResumeResult !== "0xffffffff" ||
    latestEvidence.createThreadOracleInvalidResumeLastError !== 6 ||
    latestEvidence.createThreadImplemented !== true ||
    latestEvidence.createThreadImplementationId !==
      "kernel32.create-thread.v1" ||
    latestEvidence.resumeThreadImplemented !== true ||
    latestEvidence.resumeThreadImplementationId !==
      "kernel32.resume-thread.v1" ||
    latestEvidence.closeHandleImplementationId !== "kernel32.close-handle.v2" ||
    latestEvidence.createThreadPolicySchema !==
      "vine.kernel32-compatibility-policy" ||
    latestEvidence.createThreadPolicySchemaVersion !== 2 ||
    latestEvidence.createThreadPolicyId !== "kernel32-create-thread-v1" ||
    latestEvidence.createThreadDefaultStackReserveBytes !== 1_048_576 ||
    latestEvidence.createThreadDefaultStackCommitBytes !== 4_096 ||
    latestEvidence.createThreadAllowedCreationFlags?.join(",") !== "0,4" ||
    latestEvidence.createThreadMaximumThreads !== 256 ||
    latestEvidence.createThreadStateAuthority !==
      "runtime/thread.rs::RuntimeThreads" ||
    latestEvidence.createThreadSchedulerAuthority !==
      "runtime/scheduler.rs::RuntimeScheduler" ||
    latestEvidence.createThreadObjectAuthority !==
      "runtime/objects.rs::RuntimeObjectTable" ||
    latestEvidence.createThreadAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    latestEvidence.createThreadMemoryAuthority !== "rust-guest-memory" ||
    latestEvidence.createThreadSequence !== 18_598 ||
    latestEvidence.createThreadInstructionCount !== 11_945_954 ||
    latestEvidence.createThreadArguments?.join(",") !==
      "0x00000000,0x00000000,0x100a1ee8,0x20139940,0x00000004,0x20139940" ||
    latestEvidence.createThreadReturnAddress !== "0x100a1fad" ||
    latestEvidence.createThreadResult !== "0x54010002" ||
    latestEvidence.createThreadLastError !== 0 ||
    latestEvidence.createThreadGuestWriteBytes !== 4 ||
    latestEvidence.createThreadOutputAddress !== "0x20139940" ||
    latestEvidence.createThreadOutputHex !== "01200000" ||
    latestEvidence.createThreadThreadId !== 8_193 ||
    latestEvidence.createThreadObjectId !== 25 ||
    latestEvidence.createThreadHandleNamespace !==
      "vine.runtime-thread-object.v1" ||
    latestEvidence.createThreadStackAddressAllocationId !== 57 ||
    latestEvidence.createThreadStackMemoryOwner !== 27 ||
    latestEvidence.createThreadStackReserveBase !== "0x50000000" ||
    latestEvidence.createThreadStackReserveBytes !== 1_048_576 ||
    latestEvidence.createThreadStackCommitBase !== "0x500ff000" ||
    latestEvidence.createThreadStackCommitBytes !== 4_096 ||
    latestEvidence.createThreadStackGuardPage !== "0x500fe000" ||
    latestEvidence.createThreadStackTop !== "0x50100000" ||
    latestEvidence.createThreadInitialStackPointer !== "0x500ffff8" ||
    latestEvidence.createThreadReturnSentinel !== "0x7f010010" ||
    latestEvidence.createThreadEnvironmentAddressAllocationId !== 58 ||
    latestEvidence.createThreadEnvironmentMemoryOwner !== 28 ||
    latestEvidence.createThreadTebAddress !== "0x7f030000" ||
    latestEvidence.createThreadPebAddress !== "0x7ffdf000" ||
    latestEvidence.createThreadEntryPoint !== "0x100a1ee8" ||
    latestEvidence.createThreadParameter !== "0x20139940" ||
    latestEvidence.createThreadInitialSchedulerState !== "suspended" ||
    latestEvidence.createThreadInitialSuspendCount !== 1 ||
    latestEvidence.createThreadDeltaFromGeneration !== 36_502 ||
    latestEvidence.createThreadDeltaGeneration !== 36_511 ||
    latestEvidence.createThreadDeltaMappingOperations !== 4 ||
    latestEvidence.createThreadDeltaDirtyPages !== 3 ||
    latestEvidence.createThreadDeltaControlBytes !== 264 ||
    latestEvidence.createThreadDeltaPayloadBytes !== 12_288 ||
    latestEvidence.createThreadDeltaPayloadSha256 !==
      "1e958a4c95b466754ec28a482c317c674175fb5ec5ce4a389406325684acb054" ||
    latestEvidence.resumeThreadSequence !== 18_599 ||
    latestEvidence.resumeThreadInstructionCount !== 11_945_961 ||
    latestEvidence.resumeThreadArguments?.join(",") !== "0x54010002" ||
    latestEvidence.resumeThreadReturnAddress !== "0x100a1fbd" ||
    latestEvidence.resumeThreadResult !== 1 ||
    latestEvidence.resumeThreadLastError !== 0 ||
    latestEvidence.resumeThreadGuestWriteBytes !== 0 ||
    latestEvidence.resumeThreadThreadId !== 8_193 ||
    latestEvidence.resumeThreadPreviousSuspendCount !== 1 ||
    latestEvidence.resumeThreadFinalSchedulerState !== "runnable" ||
    latestEvidence.createThreadSchedulingPolicy !==
      "synchronous-resume-preserves-caller-until-runnable-contention-api-boundary-quantum-64-expires" ||
    latestEvidence.createThreadNodeExecutedThreadIds?.join(",") !==
      "8192,8193" ||
    latestEvidence.createThreadChromeRegisteredThreadIds?.join(",") !==
      "8192,8193" ||
    latestEvidence.createThreadChromeExecutedThreadIds?.join(",") !==
      "8192,8193" ||
    latestEvidence.createThreadContextSwitchesBeforeNextStop !==
      (mainLoopContinuationActive ? 2 : 1) ||
    latestEvidence.createThreadDynamicExitReleasesMappings !== true ||
    latestEvidence.createThreadDynamicAddressReuseVerified !== true ||
    latestEvidence.createThreadFailureAtomicity !== true ||
    latestEvidence.createThreadBrowserOwnsWindowsState !== false ||
    latestEvidence.createThreadNodeCompletions !== 1 ||
    latestEvidence.createThreadChromeCompletions !== 1 ||
    latestEvidence.resumeThreadNodeCompletions !== 1 ||
    latestEvidence.resumeThreadChromeCompletions !== 1 ||
    latestEvidence.createThreadNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest CreateThread and ResumeThread evidence is invalid");
  }
  const imm32Context = requireRecord(
    latestEvidence.imm32ContextEvidence,
    "activeRuntimeCheckpoint.latestEvidence.imm32ContextEvidence",
    errors,
  );
  if (
    imm32Context.calls !== 3 ||
    imm32Context.sequences?.join(",") !== "18643,18644,18645" ||
    imm32Context.instructionCounts?.join(",") !==
      "11971315,11971323,11971329" ||
    imm32Context.apis?.join(",") !==
      "IMM32.dll!ImmGetDefaultIMEWnd,USER32.dll!SendMessageA,IMM32.dll!ImmAssociateContext" ||
    imm32Context.appWindowHandle !== "0x48010001" ||
    imm32Context.defaultImeWindowHandle !== "0x48010002" ||
    imm32Context.defaultContextHandle !== "0x494d0001" ||
    imm32Context.statusWindowRequest !== "close" ||
    imm32Context.associateRequestedContextHandle !== "0x00000000" ||
    imm32Context.associatePreviousContextHandle !== "0x494d0001" ||
    imm32Context.finalAssociatedContext !== null ||
    imm32Context.windowAuthority !== "runtime-user32-logical-window" ||
    imm32Context.contextAuthority !== "runtime-imm32-logical-context" ||
    imm32Context.associationAuthority !== "runtime-imm32-window-association" ||
    imm32Context.nativeHandleImported !== false ||
    imm32Context.nativeMessageForwarded !== false ||
    imm32Context.browserResourceCreated !== false ||
    imm32Context.nodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest IMM32 default context evidence is invalid");
  }
  for (const field of [
    "nativeSetThreadPriorityPrimaryTraceSha256",
    "nativeSetThreadPriorityConfirmationTraceSha256",
    "setThreadPriorityOracleSourceSha256",
    "setThreadPriorityOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeSetThreadPriorityPrimaryTraceBytes !== 2_773_117 ||
    latestEvidence.nativeSetThreadPriorityPrimaryTraceLines !== 953 ||
    latestEvidence.nativeSetThreadPriorityPrimaryTraceSha256 !==
      "dd98137e23b5c9cdf34dff09eb14e39b15d653086c0334a557768f5008e7caac" ||
    latestEvidence.nativeSetThreadPriorityConfirmationTraceBytes !==
      2_774_186 ||
    latestEvidence.nativeSetThreadPriorityConfirmationTraceLines !== 953 ||
    latestEvidence.nativeSetThreadPriorityConfirmationTraceSha256 !==
      "9370aec65046b6269eb27a77c37c40ae31c9dd9e5b34aaeeed63a8210d87cd2f" ||
    latestEvidence.nativeSetThreadPriorityTraceDiagnostics !== 0 ||
    latestEvidence.nativeSetThreadPriorityHookErrors !== 0 ||
    latestEvidence.nativeSetThreadPriorityProfile !==
      "x86-bgi-created-thread-priority-v1" ||
    latestEvidence.nativeSetThreadPriorityCallsPerTrace !== 31 ||
    latestEvidence.nativeSetThreadPriorityTargetCallsPerTrace !== 1 ||
    latestEvidence.nativeSetThreadPriorityCallerReturnRva !== "0x00091b77" ||
    latestEvidence.nativeSetThreadPriorityValue !== 1 ||
    latestEvidence.nativeSetThreadPriorityCreatedHandleRelation !== true ||
    latestEvidence.nativeSetThreadPriorityResult !== 1 ||
    latestEvidence.nativeSetThreadPriorityLastError !== 0 ||
    latestEvidence.nativeSetThreadPrioritySuccessLastErrorPreserved !== true ||
    latestEvidence.nativeSetThreadPriorityConsumer !==
      "restore-seh-frame-and-return-without-reading-eax" ||
    latestEvidence.nativeSetThreadPriorityExecutableHashUnchanged !== true ||
    latestEvidence.setThreadPriorityOracleArchitecture !== "coff-i386" ||
    latestEvidence.setThreadPriorityOracleSourceBytes !== 7_253 ||
    latestEvidence.setThreadPriorityOracleSourceSha256 !==
      "5a79901aa8a5c77d17d9ba9d916c418dc204c52504118118225e3eddbfffa79c" ||
    latestEvidence.setThreadPriorityOracleExecutableBytes !== 8_704 ||
    latestEvidence.setThreadPriorityOracleExecutableSha256 !==
      "a181fe64363e2970393123a2389719fcaee689db89dc3c3132662fd8aa08f76f" ||
    latestEvidence.setThreadPriorityOracleLastErrorSentinel !== 23_130 ||
    latestEvidence.setThreadPriorityOracleInitialPriority !== 0 ||
    latestEvidence.setThreadPriorityOracleAboveNormalPriority !== 1 ||
    latestEvidence.setThreadPriorityOracleRepeatPreservesPriority !== true ||
    latestEvidence.setThreadPriorityOracleRestorePriority !== 0 ||
    latestEvidence.setThreadPriorityOracleSuccessPreservesLastError !== true ||
    latestEvidence.setThreadPriorityOracleInvalidPriorityResult !== 0 ||
    latestEvidence.setThreadPriorityOracleInvalidPriorityLastError !== 87 ||
    latestEvidence.setThreadPriorityOracleInvalidPriorityPreservesState !==
      true ||
    latestEvidence.setThreadPriorityOracleInvalidHandleResult !== 0 ||
    latestEvidence.setThreadPriorityOracleInvalidHandleLastError !== 6 ||
    latestEvidence.setThreadPriorityOracleClosedHandleResult !== 0 ||
    latestEvidence.setThreadPriorityOracleClosedHandleLastError !== 6 ||
    latestEvidence.setThreadPriorityImplemented !== true ||
    latestEvidence.setThreadPriorityImplementationId !==
      "kernel32.set-thread-priority.v1" ||
    latestEvidence.setThreadPriorityPolicySchema !==
      "vine.kernel32-compatibility-policy" ||
    latestEvidence.setThreadPriorityPolicySchemaVersion !== 2 ||
    latestEvidence.setThreadPriorityPolicyId !==
      "kernel32-set-thread-priority-v1" ||
    latestEvidence.setThreadPriorityAllowedPriorities?.join(",") !== "1" ||
    latestEvidence.setThreadPrioritySequence !== 18_600 ||
    latestEvidence.setThreadPriorityInstructionCount !== 11_945_977 ||
    latestEvidence.setThreadPriorityArguments?.join(",") !==
      "0x54010002,0x00000001" ||
    latestEvidence.setThreadPriorityReturnAddress !== "0x10091b77" ||
    latestEvidence.setThreadPriorityResult !== 1 ||
    latestEvidence.setThreadPriorityLastError !== 0 ||
    latestEvidence.setThreadPriorityGuestWriteBytes !== 0 ||
    latestEvidence.setThreadPriorityHandle !== "0x54010002" ||
    latestEvidence.setThreadPriorityThreadId !== 8_193 ||
    latestEvidence.setThreadPriorityValue !== "above-normal" ||
    latestEvidence.setThreadPriorityStateAuthority !==
      "runtime/thread.rs::RuntimeThreads" ||
    latestEvidence.setThreadPriorityMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.setThreadPrioritySchedulerPolicy !==
      "logical-state-only-cooperative-selection-unchanged" ||
    latestEvidence.setThreadPriorityHostSchedulerImported !== false ||
    latestEvidence.setThreadPriorityBrowserSchedulerImported !== false ||
    latestEvidence.setThreadPriorityFailureAtomicity !== true ||
    latestEvidence.setThreadPriorityNodeCompletions !== 1 ||
    latestEvidence.setThreadPriorityChromeCompletions !== 1 ||
    latestEvidence.setThreadPriorityNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest SetThreadPriority evidence is invalid");
  }
  for (const field of [
    "nativeScreenDcPrimaryTraceSha256",
    "nativeScreenDcConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeScreenDcPrimaryTraceBytes !== 46_976 ||
    latestEvidence.nativeScreenDcPrimaryTraceLines !== 91 ||
    latestEvidence.nativeScreenDcPrimaryTraceSha256 !==
      "874c9744b015213963164d37073bc9edbfed25301501febcbccf0f3c4409ff80" ||
    latestEvidence.nativeScreenDcConfirmationTraceBytes !== 46_849 ||
    latestEvidence.nativeScreenDcConfirmationTraceLines !== 91 ||
    latestEvidence.nativeScreenDcConfirmationTraceSha256 !==
      "b06ec143734cb4d93fb5df3f5fe9849ddf95a4e804ba1a23e8648181ab32ee59" ||
    latestEvidence.nativeScreenDcTraceDiagnostics !== 0 ||
    latestEvidence.nativeScreenDcHookErrors !== 0 ||
    latestEvidence.nativeScreenDcProfile !==
      "x86-bgi-screen-dc-font-enumeration-lifetime-v1" ||
    latestEvidence.nativeScreenDcCallsPerTrace !== 3 ||
    latestEvidence.nativeScreenDcCallerReturnRvas?.join(",") !==
      "0x0002c155,0x0002c16a,0x0002c177" ||
    latestEvidence.nativeScreenDcGetDcResultNonNull !== true ||
    latestEvidence.nativeScreenDcEnumFontFamiliesExAResult !== 0 ||
    latestEvidence.nativeScreenDcReleaseDcResult !== 1 ||
    latestEvidence.nativeScreenDcLastErrors?.join(",") !== "0,0,0" ||
    latestEvidence.nativeScreenDcEnumFontCallbackRva !== "0x0002d2e0" ||
    latestEvidence.nativeScreenDcEnumFontLparamRva !== "0x000dce7c" ||
    latestEvidence.nativeScreenDcEnumFontFlags !== 0 ||
    latestEvidence.nativeScreenDcLogFontBytes !== 60 ||
    latestEvidence.nativeScreenDcLogFontCharset !== 128 ||
    latestEvidence.nativeScreenDcLogFontFaceWindows932 !== "ＭＳ ゴシック" ||
    latestEvidence.nativeScreenDcConsumer !==
      "enum-zero-normalized-to-bool-true" ||
    latestEvidence.nativeScreenDcExecutableHashUnchanged !== true ||
    latestEvidence.screenDcOracleProfile !== "x86-screen-dc-lifetime-v1" ||
    latestEvidence.screenDcOracleCallCount !== 5 ||
    latestEvidence.screenDcOracleConcurrentAcquisitionsDistinct !== true ||
    latestEvidence.screenDcOracleReleaseResults?.join(",") !== "1,0,1" ||
    latestEvidence.screenDcOracleSuccessAndFailurePreserveLastError !== true ||
    latestEvidence.screenDcOracleResultsMatchHooks !== true ||
    latestEvidence.screenDcImplemented !== true ||
    latestEvidence.screenDcImplementationId !==
      "user32.get-dc.window-or-screen-acquisition.v2" ||
    latestEvidence.screenDcPolicySchema !==
      "vine.user32-compatibility-policy" ||
    latestEvidence.screenDcPolicySchemaVersion !== 19 ||
    latestEvidence.screenDcPolicyId !==
      "user32-single-monitor-work-area-window-input-resources-monitor-info-screen-dc-v19" ||
    latestEvidence.screenDcProfileId !==
      "null-window-primary-screen-same-thread-v1" ||
    latestEvidence.screenDcSequence !== 18_608 ||
    latestEvidence.screenDcInstructionCount !== 11_957_736 ||
    latestEvidence.screenDcArguments?.join(",") !== "0x00000000" ||
    latestEvidence.screenDcReturnAddress !== "0x1002c155" ||
    latestEvidence.screenDcResult !== "0x47010003" ||
    latestEvidence.screenDcLastError !== 0 ||
    latestEvidence.screenDcGuestWriteBytes !== 0 ||
    latestEvidence.screenDcDeviceContextId !== 3 ||
    latestEvidence.screenDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    latestEvidence.screenDcOwnerThreadId !== 8_192 ||
    latestEvidence.screenDcTargetKind !== "screen" ||
    latestEvidence.screenDcMonitorHandle !== "0x4f010001" ||
    latestEvidence.screenDcRect?.join(",") !== "0,0,1920,1080" ||
    latestEvidence.screenDcLiveDeviceContexts !== 1 ||
    latestEvidence.screenDcSurfaceCreated !== false ||
    latestEvidence.screenDcStateAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.screenDcDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.screenDcRelation !==
      "non-owning-primary-monitor-and-acquiring-thread" ||
    latestEvidence.screenDcMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.screenDcFailureAtomicity !== true ||
    latestEvidence.screenDcBrowserOwnsWindowsState !== false ||
    latestEvidence.screenDcRendererResourceCreated !== false ||
    latestEvidence.screenDcNodeCompletions !== 1 ||
    latestEvidence.screenDcChromeCompletions !== 1 ||
    latestEvidence.screenDcNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest primary-screen device-context evidence is invalid");
  }
  for (const field of [
    "nativeFontEnumerationPrimaryTraceSha256",
    "nativeFontEnumerationConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeFontEnumerationPrimaryTraceBytes !== 98_030 ||
    latestEvidence.nativeFontEnumerationPrimaryTraceLines !== 95 ||
    latestEvidence.nativeFontEnumerationPrimaryTraceSha256 !==
      "eb09cfc2a18967f74e6054beb57b92688e08c2d01556841cea94fa59e30d3027" ||
    latestEvidence.nativeFontEnumerationConfirmationTraceBytes !== 97_732 ||
    latestEvidence.nativeFontEnumerationConfirmationTraceLines !== 95 ||
    latestEvidence.nativeFontEnumerationConfirmationTraceSha256 !==
      "9a337ac82e09ec7c6c6c9a3e9ac9def7042e7af03be6e05c286ab81b9f0fbb33" ||
    latestEvidence.nativeFontEnumerationTraceDiagnostics !== 0 ||
    latestEvidence.nativeFontEnumerationHookErrors !== 0 ||
    latestEvidence.nativeFontEnumerationProfile !==
      "x86-bgi-screen-dc-font-enumeration-callback-v2" ||
    latestEvidence.nativeFontEnumerationOracleProfile !==
      "x86-screen-dc-and-font-enumeration-v2" ||
    latestEvidence.nativeFontEnumerationCallbackOracleProfile !==
      "x86-cp932-ms-gothic-callback-enumeration-v1" ||
    latestEvidence.nativeFontEnumerationCallerReturnRva !== "0x0002c16a" ||
    latestEvidence.nativeFontEnumerationInputCharacterSet !== 128 ||
    latestEvidence.nativeFontEnumerationInputFaceWindows932Hex !==
      "826c827220835383568362834e" ||
    latestEvidence.nativeFontEnumerationFlags !== 0 ||
    latestEvidence.nativeFontEnumerationCallbackRva !== "0x0002d2e0" ||
    latestEvidence.nativeFontEnumerationLparamRva !== "0x000dce7c" ||
    latestEvidence.nativeFontEnumerationCallbackCount !== 1 ||
    latestEvidence.nativeFontEnumerationCallbackResult !== 0 ||
    latestEvidence.nativeFontEnumerationResult !== 0 ||
    latestEvidence.nativeFontEnumerationLastError !== 0 ||
    latestEvidence.nativeFontEnumerationFontType !== 4 ||
    latestEvidence.nativeFontEnumerationEnumLogFontExABytes !== 188 ||
    latestEvidence.nativeFontEnumerationNewTextMetricExABytes !== 96 ||
    latestEvidence.nativeFontEnumerationFaceAndFullNameWindows932Hex !==
      "826c827220835383568362834e" ||
    latestEvidence.nativeFontEnumerationStyleWindows932Hex !== "95578f80" ||
    latestEvidence.nativeFontEnumerationScriptWindows932Hex !==
      "93fa967b8cea" ||
    latestEvidence.nativeFontEnumerationLogFontFields?.join(",") !==
      "32,16,400,128,3,2,1,49" ||
    latestEvidence.nativeFontEnumerationTextMetricFields?.join(",") !==
      "32,28,4,16,63,400,96,96,0,255,165,2,54,128,2359360,256,256,128" ||
    latestEvidence.nativeFontEnumerationFontSignatureHex !==
      "ff0200e0fbfdc76a12000008000000009f0002400000d7df" ||
    latestEvidence.nativeFontEnumerationConsumer !==
      "face-plus-lparam-then-zero-normalized-to-bool-true" ||
    latestEvidence.nativeFontEnumerationRawFixedFieldTailsStable !== false ||
    latestEvidence.nativeFontEnumerationSemanticFieldsStable !== true ||
    latestEvidence.nativeFontEnumerationExecutableHashUnchanged !== true ||
    latestEvidence.fontEnumerationOracleStopResult !== 0 ||
    latestEvidence.fontEnumerationOracleContinueResult !== 1 ||
    latestEvidence.fontEnumerationOracleCallbackCounts?.join(",") !== "1,1" ||
    latestEvidence.fontEnumerationOracleCallbackDataRoundTrips !== true ||
    latestEvidence.fontEnumerationOracleInputAndGuardsUnchanged !== true ||
    latestEvidence.fontEnumerationOracleLastErrorPreserved !== true ||
    latestEvidence.fontEnumerationImplemented !== true ||
    latestEvidence.fontEnumerationImplementationId !==
      "gdi32.enum-font-families-ex-a.cp932-screen.v1" ||
    latestEvidence.fontEnumerationPolicySchema !==
      "vine.gdi32-compatibility-policy" ||
    latestEvidence.fontEnumerationPolicySchemaVersion !== 6 ||
    latestEvidence.fontEnumerationPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.fontEnumerationProfileId !==
      "primary-cp932-ms-gothic-regular-v1" ||
    latestEvidence.fontEnumerationSequence !== 18_609 ||
    latestEvidence.fontEnumerationInstructionCount !== 11_957_745 ||
    latestEvidence.fontEnumerationArguments?.join(",") !==
      "0x47010003,0x006ffd00,0x1002d2e0,0x100dce7c,0x00000000" ||
    latestEvidence.fontEnumerationReturnAddress !== "0x1002c16a" ||
    latestEvidence.fontEnumerationResult !== 0 ||
    latestEvidence.fontEnumerationLastError !== 0 ||
    latestEvidence.fontEnumerationGuestWriteBytes !== 0 ||
    latestEvidence.fontEnumerationCallbackFrameId !== 20 ||
    latestEvidence.fontEnumerationCallbackTarget !== "0x1002d2e0" ||
    latestEvidence.fontEnumerationCallbackResult !== 0 ||
    latestEvidence.fontEnumerationCallbackReturnInstructionCount !==
      11_957_981 ||
    latestEvidence.fontEnumerationCallbackScratchBytes !== 284 ||
    latestEvidence.fontEnumerationCanonicalTailInitialization !== "zero" ||
    latestEvidence.fontEnumerationCatalogAuthority !==
      "gdi32-compatibility-policy" ||
    latestEvidence.fontEnumerationScreenHdcValidation !==
      "live-same-thread-primary-monitor-relation" ||
    latestEvidence.fontEnumerationFailureAtomicity !== true ||
    latestEvidence.fontEnumerationHostFontDiscoveryImported !== false ||
    latestEvidence.fontEnumerationBrowserFontDiscoveryImported !== false ||
    latestEvidence.fontEnumerationBrowserFontResourceCreated !== false ||
    latestEvidence.fontEnumerationNodeCompletions !== 1 ||
    latestEvidence.fontEnumerationChromeCompletions !== 1 ||
    latestEvidence.fontEnumerationNodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.screenReleaseDcSequence !== 18_610 ||
    latestEvidence.screenReleaseDcInstructionCount !== 11_957_987 ||
    latestEvidence.screenReleaseDcArguments?.join(",") !==
      "0x00000000,0x47010003" ||
    latestEvidence.screenReleaseDcReturnAddress !== "0x1002c177" ||
    latestEvidence.screenReleaseDcResult !== 1 ||
    latestEvidence.screenReleaseDcLastError !== 0 ||
    latestEvidence.screenReleaseDcLiveDeviceContexts !== 0
  ) {
    errors.push("latest bounded font-enumeration evidence is invalid");
  }
  for (const field of [
    "nativeFontCreationPrimaryTraceSha256",
    "nativeFontCreationConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeFontCreationPrimaryTraceBytes !== 479_331 ||
    latestEvidence.nativeFontCreationPrimaryTraceLines !== 118 ||
    latestEvidence.nativeFontCreationPrimaryTraceSha256 !==
      "e247e276261c0ed0fd03874c45e1b7f51fa51a02163b427b58c7e6b5b7b0fcd8" ||
    latestEvidence.nativeFontCreationConfirmationTraceBytes !== 479_085 ||
    latestEvidence.nativeFontCreationConfirmationTraceLines !== 117 ||
    latestEvidence.nativeFontCreationConfirmationTraceSha256 !==
      "eade672e0bb09f6b14952059cbc092280ef760cc7138aa845dd2f1c3527f6642" ||
    latestEvidence.nativeFontCreationTraceDiagnostics !== 0 ||
    latestEvidence.nativeFontCreationHookErrors !== 0 ||
    latestEvidence.nativeFontCreationProfile !==
      "x86-bgi-gdi-font-object-lifecycle-v1" ||
    latestEvidence.nativeFontCreationOracleProfile !==
      "x86-create-font-a-object-lifecycle-v1" ||
    latestEvidence.nativeFontCreationCallerReturnRva !== "0x0002c838" ||
    latestEvidence.nativeFontCreationLogFontFields?.join(",") !==
      "64,32,0,0,100,0,0,0,128,4,0,2,0" ||
    latestEvidence.nativeFontCreationFaceWindows932Hex !==
      "826c827220835383568362834e" ||
    latestEvidence.nativeFontCreationResultNonNull !== true ||
    latestEvidence.nativeFontCreationLastError !== 0 ||
    latestEvidence.nativeFontCreationConsumer !==
      "store-test-nonzero-then-create-compatible-dc" ||
    latestEvidence.nativeFontCreationOrderedApis?.join(",") !==
      "CreateFontA,CreateCompatibleDC,CreateDIBSection,GetObjectA,SelectObject,SelectObject,GetOutlineTextMetricsA,GetOutlineTextMetricsA,TextOutA,SelectObject,SelectObject,DeleteObject,DeleteObject,DeleteDC" ||
    latestEvidence.nativeFontCreationDibWidth !== 128 ||
    latestEvidence.nativeFontCreationDibHeight !== -96 ||
    latestEvidence.nativeFontCreationDibBitsPerPixel !== 8 ||
    latestEvidence.nativeFontCreationDibCompression !== 0 ||
    latestEvidence.nativeFontCreationDibColorCount !== 256 ||
    latestEvidence.nativeFontCreationDibStride !== 128 ||
    latestEvidence.nativeFontCreationDibPixelBytes !== 12_288 ||
    latestEvidence.nativeFontCreationDibPalette !== "reverse-grayscale" ||
    latestEvidence.nativeFontCreationRasterTextWindows932Hex !== "61" ||
    latestEvidence.nativeFontCreationRasterBeforeFnv1a32 !== "0x52965dc5" ||
    latestEvidence.nativeFontCreationRasterAfterFnv1a32 !== "0xe6fc8461" ||
    latestEvidence.nativeFontCreationRasterChangedBytes !== 476 ||
    latestEvidence.nativeFontCreationRasterChangedValues?.join(",") !==
      "0,255" ||
    latestEvidence.nativeFontCreationRasterBoundingBox?.join(",") !==
      "3,20,28,54" ||
    latestEvidence.nativeFontCreationRasterConsumerScansDibBytes !== true ||
    latestEvidence.nativeFontCreationExecutableHashUnchanged !== true ||
    latestEvidence.fontCreationOracleGetObjectResultBytes !== 60 ||
    latestEvidence.fontCreationOracleExactLogFontRoundTrip !== true ||
    latestEvidence.fontCreationOracleOutputGuardsUnchanged !== true ||
    latestEvidence.fontCreationOracleSelectRestoreIdentity !== true ||
    latestEvidence.fontCreationOracleDeleteResults?.join(",") !== "1,0" ||
    latestEvidence.fontCreationOracleStaleGetObjectResult !== 0 ||
    latestEvidence.fontCreationOracleStaleGetObjectPreservesOutput !== true ||
    latestEvidence.fontCreationOracleSuccessAndFailurePreserveLastError !==
      true ||
    latestEvidence.fontCreationImplemented !== true ||
    latestEvidence.fontCreationImplementationId !==
      "gdi32.create-font-a.cp932-logical-object.v1" ||
    latestEvidence.fontCreationPolicySchema !==
      "vine.gdi32-compatibility-policy" ||
    latestEvidence.fontCreationPolicySchemaVersion !== 6 ||
    latestEvidence.fontCreationPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.fontCreationProfileId !==
      "primary-cp932-ms-gothic-64x32-thin-v1" ||
    latestEvidence.fontCreationSequence !== 18_611 ||
    latestEvidence.fontCreationInstructionCount !== 11_958_023 ||
    latestEvidence.fontCreationArguments?.join(",") !==
      "0x00000040,0x00000020,0x00000000,0x00000000,0x00000064,0x00000000,0x00000000,0x00000000,0x00000080,0x00000004,0x00000000,0x00000002,0x00000000,0x100dce7c" ||
    latestEvidence.fontCreationReturnAddress !== "0x1002c838" ||
    latestEvidence.fontCreationResult !== "0x47020002" ||
    latestEvidence.fontCreationLastError !== 0 ||
    latestEvidence.fontCreationGuestWriteBytes !== 0 ||
    latestEvidence.fontCreationObjectId !== 2 ||
    latestEvidence.fontCreationHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    latestEvidence.fontCreationShared !== false ||
    latestEvidence.fontCreationLogFontFields?.join(",") !==
      "64,32,0,0,100,0,0,0,128,4,0,2,0" ||
    latestEvidence.fontCreationFaceWindows932Hex !==
      "826c827220835383568362834e" ||
    latestEvidence.fontCreationLiveObjects !== 2 ||
    latestEvidence.fontCreationStateAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.fontCreationMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.fontCreationFailureAtomicity !== true ||
    latestEvidence.fontCreationHostFontResolutionImported !== false ||
    latestEvidence.fontCreationBrowserFontResolutionImported !== false ||
    latestEvidence.fontCreationBrowserFontResourceCreated !== false ||
    latestEvidence.fontCreationNodeCompletions !== 1 ||
    latestEvidence.fontCreationChromeCompletions !== 1 ||
    latestEvidence.fontCreationNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest bounded logical font-creation evidence is invalid");
  }
  for (const field of [
    "nativeCompatibleDcPrimaryTraceSha256",
    "nativeCompatibleDcConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeCompatibleDcPrimaryTraceBytes !== 489_171 ||
    latestEvidence.nativeCompatibleDcPrimaryTraceLines !== 132 ||
    latestEvidence.nativeCompatibleDcPrimaryTraceSha256 !==
      "4e7015df75b77ad133c8eb28437bfe26e45f5171e692aa913148cf3d74da1e72" ||
    latestEvidence.nativeCompatibleDcConfirmationTraceBytes !== 489_137 ||
    latestEvidence.nativeCompatibleDcConfirmationTraceLines !== 132 ||
    latestEvidence.nativeCompatibleDcConfirmationTraceSha256 !==
      "48ac1ae635eeb927b0da807bafa565c96d6905ada1dbb50dac53b282ca276008" ||
    latestEvidence.nativeCompatibleDcTraceDiagnostics !== 0 ||
    latestEvidence.nativeCompatibleDcHookErrors !== 0 ||
    latestEvidence.nativeCompatibleDcProfile !==
      "x86-bgi-gdi-font-object-lifecycle-v2" ||
    latestEvidence.nativeCompatibleDcOuterOracleProfile !==
      "x86-create-font-a-object-lifecycle-v2" ||
    latestEvidence.nativeCompatibleDcOracleProfile !==
      "x86-create-compatible-dc-null-lifecycle-v1" ||
    latestEvidence.nativeCompatibleDcCallerReturnRva !== "0x0002c8ad" ||
    latestEvidence.nativeCompatibleDcResultNonNull !== true ||
    latestEvidence.nativeCompatibleDcLastError !== 0 ||
    latestEvidence.nativeCompatibleDcConsumer !==
      "store-directly-then-create-dib-section" ||
    latestEvidence.nativeCompatibleDcOrderedApis?.join(",") !==
      "CreateFontA,CreateCompatibleDC,CreateDIBSection,GetObjectA,SelectObject,SelectObject,GetOutlineTextMetricsA,GetOutlineTextMetricsA,TextOutA,SelectObject,SelectObject,DeleteObject,DeleteObject,DeleteDC" ||
    latestEvidence.nativeCompatibleDcExecutableHashUnchanged !== true ||
    latestEvidence.compatibleDcOracleCreateCount !== 3 ||
    latestEvidence.compatibleDcOracleHandlesDistinct !== true ||
    latestEvidence.compatibleDcOracleObjectType !== 10 ||
    latestEvidence.compatibleDcOracleInitialFontType !== 6 ||
    latestEvidence.compatibleDcOracleInitialBitmapType !== 7 ||
    latestEvidence.compatibleDcOracleInitialFontShared !== true ||
    latestEvidence.compatibleDcOracleInitialBitmapShared !== true ||
    latestEvidence.compatibleDcOracleWrongReleaseResult !== 0 ||
    latestEvidence.compatibleDcOracleWrongReleaseLastError !== 1_425 ||
    latestEvidence.compatibleDcOracleTypeAfterWrongRelease !== 10 ||
    latestEvidence.compatibleDcOracleDeleteResults?.join(",") !== "1,0,1" ||
    latestEvidence.compatibleDcOracleTypeAfterDelete !== 0 ||
    latestEvidence.compatibleDcOracleDeleteObjectVariantResult !== 1 ||
    latestEvidence.compatibleDcOracleDeleteObjectVariantTypeAfter !== 0 ||
    latestEvidence.compatibleDcOracleDeleteObjectVariantClaimed !== false ||
    latestEvidence.compatibleDcOracleResultsMatchHooks !== true ||
    latestEvidence.compatibleDcOracleSuccessAndFailurePreserveLastError !==
      true ||
    latestEvidence.compatibleDcImplemented !== true ||
    latestEvidence.compatibleDcImplementationId !==
      "gdi32.create-compatible-dc.null-screen-memory.v1" ||
    latestEvidence.deleteDcContractImplemented !== true ||
    latestEvidence.deleteDcImplementationId !==
      "gdi32.delete-dc.logical-memory.v1" ||
    latestEvidence.deleteDcReachedBeforeNextStop !== true ||
    latestEvidence.deleteDcSequence !== 18_628 ||
    latestEvidence.deleteDcInstructionCount !== 12_072_567 ||
    latestEvidence.deleteDcResult !== 1 ||
    latestEvidence.deleteDcReturnAddress !== "0x1002c472" ||
    latestEvidence.compatibleDcPolicySchema !==
      "vine.gdi32-compatibility-policy" ||
    latestEvidence.compatibleDcPolicySchemaVersion !== 6 ||
    latestEvidence.compatibleDcPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.compatibleDcProfileId !==
      "primary-null-screen-memory-dc-v1" ||
    latestEvidence.compatibleDcSequence !== 18_613 ||
    latestEvidence.compatibleDcInstructionCount !== 11_960_140 ||
    latestEvidence.compatibleDcArguments?.join(",") !== "0x00000000" ||
    latestEvidence.compatibleDcReturnAddress !== "0x1002c8ad" ||
    latestEvidence.compatibleDcResult !== "0x47010004" ||
    latestEvidence.compatibleDcLastError !== 0 ||
    latestEvidence.compatibleDcGuestWriteBytes !== 0 ||
    latestEvidence.compatibleDcDeviceContextId !== 4 ||
    latestEvidence.compatibleDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    latestEvidence.compatibleDcOwnerThreadId !== 8_192 ||
    latestEvidence.compatibleDcTargetKind !== "memory" ||
    latestEvidence.compatibleDcMonitorHandle !== "0x4f010001" ||
    latestEvidence.compatibleDcRect?.join(",") !== "0,0,1920,1080" ||
    latestEvidence.compatibleDcSelectedFontId !== 3 ||
    latestEvidence.compatibleDcSelectedBitmapId !== 4 ||
    latestEvidence.compatibleDcSelectionGeneration !== 0 ||
    latestEvidence.compatibleDcInitialFontObjectId !== 3 ||
    latestEvidence.compatibleDcInitialFontHandle !== "0x47020003" ||
    latestEvidence.compatibleDcInitialFontStockIndex !== 13 ||
    latestEvidence.compatibleDcInitialFontShared !== true ||
    latestEvidence.compatibleDcInitialBitmapObjectId !== 4 ||
    latestEvidence.compatibleDcInitialBitmapHandle !== "0x47020004" ||
    latestEvidence.compatibleDcInitialBitmapStockIndex !== 21 ||
    latestEvidence.compatibleDcInitialBitmapGeometry?.join(",") !== "1,1,1" ||
    latestEvidence.compatibleDcInitialBitmapShared !== true ||
    latestEvidence.compatibleDcLiveDeviceContexts !== 1 ||
    latestEvidence.compatibleDcLiveObjects !== 4 ||
    latestEvidence.compatibleDcStateAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.compatibleDcDisplayAuthority !==
      "runtime/displays.rs::RuntimeDisplays" ||
    latestEvidence.compatibleDcRelation !==
      "non-owning-primary-monitor-and-creating-thread" ||
    latestEvidence.compatibleDcMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.compatibleDcFailureAtomicity !== true ||
    latestEvidence.compatibleDcHostDeviceContextImported !== false ||
    latestEvidence.compatibleDcBrowserResourceCreated !== false ||
    latestEvidence.compatibleDcPixelStorageCreated !== false ||
    latestEvidence.compatibleDcNodeCompletions !== 1 ||
    latestEvidence.compatibleDcChromeCompletions !== 1 ||
    latestEvidence.compatibleDcNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest compatible memory-DC evidence is invalid");
  }
  for (const field of [
    "nativeDibLifecyclePrimaryTraceSha256",
    "nativeDibLifecycleConfirmationTraceSha256",
    "dibCreateDeltaPayloadSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeDibLifecyclePrimaryTraceBytes !== 624_813 ||
    latestEvidence.nativeDibLifecyclePrimaryTraceLines !== 155 ||
    latestEvidence.nativeDibLifecyclePrimaryTraceSha256 !==
      "7088a2950ad7337d2e08e33134ec3aebda929912e5517a6956cd00ab24709abc" ||
    latestEvidence.nativeDibLifecycleConfirmationTraceBytes !== 624_940 ||
    latestEvidence.nativeDibLifecycleConfirmationTraceLines !== 156 ||
    latestEvidence.nativeDibLifecycleConfirmationTraceSha256 !==
      "e8ff31fe0e6fdfb6c2115e41ccd0462cac149d6f01c44750b8aff3677787d7a3" ||
    latestEvidence.nativeDibLifecycleTraceDiagnostics !== 0 ||
    latestEvidence.nativeDibLifecycleHookErrors !== 0 ||
    latestEvidence.nativeDibLifecycleProfile !==
      "x86-bgi-gdi-font-object-lifecycle-v3" ||
    latestEvidence.nativeDibLifecycleOuterOracleProfile !==
      "x86-create-font-a-object-lifecycle-v3" ||
    latestEvidence.nativeDibLifecycleOracleProfile !==
      "x86-create-dib-section-8bpp-lifecycle-v1" ||
    latestEvidence.nativeDibLifecycleOrderedApis?.join(",") !==
      "CreateFontA,CreateCompatibleDC,CreateDIBSection,GetObjectA,SelectObject,SelectObject,GetOutlineTextMetricsA,GetOutlineTextMetricsA,TextOutA,SelectObject,SelectObject,DeleteObject,DeleteObject,DeleteDC" ||
    latestEvidence.nativeDibCreateCallerReturnRva !== "0x0002c8c5" ||
    latestEvidence.nativeDibCreateHeaderBytes !== 40 ||
    latestEvidence.nativeDibCreateDescriptor?.join(",") !==
      "128,-96,1,8,0,0,0,0,256,0" ||
    latestEvidence.nativeDibCreateUsage !== 0 ||
    latestEvidence.nativeDibCreateSectionHandleNull !== true ||
    latestEvidence.nativeDibCreateOffset !== 0 ||
    latestEvidence.nativeDibCreateResultNonNull !== true ||
    latestEvidence.nativeDibCreateBitsNonNull !== true ||
    latestEvidence.nativeDibCreateLastError !== 0 ||
    latestEvidence.nativeDibPaletteEntries !== 256 ||
    latestEvidence.nativeDibPalette !== "reverse-grayscale-rgbquad" ||
    latestEvidence.nativeDibStrideBytes !== 128 ||
    latestEvidence.nativeDibPixelBytes !== 12_288 ||
    latestEvidence.nativeDibInitialPixelsZero !== true ||
    latestEvidence.nativeDibInitialPixelsFnv1a32 !== "0x52965dc5" ||
    latestEvidence.nativeDibGetObjectCallerReturnRva !== "0x0002c8e7" ||
    latestEvidence.nativeDibGetObjectResultBytes !== 24 ||
    latestEvidence.nativeDibGetObjectBitmapFields?.join(",") !==
      "0,128,96,128,1,8" ||
    latestEvidence.nativeDibGetObjectBitsMatchCreateOutput !== true ||
    latestEvidence.nativeDibGetObjectOutputGuardsUnchanged !== true ||
    latestEvidence.nativeDibSelectRestoreIdentity !== true ||
    latestEvidence.nativeDibDeleteAfterRestoreResults?.join(",") !== "1,1" ||
    latestEvidence.nativeDibStaleGetObjectResult !== 0 ||
    latestEvidence.nativeDibStaleGetObjectPreservesOutput !== true ||
    latestEvidence.nativeDibSuccessAndFailurePreserveLastError !== true ||
    latestEvidence.nativeDibMalformedHeaderResultNull !== true ||
    latestEvidence.nativeDibMalformedHeaderClearsBitsOutput !== true ||
    latestEvidence.nativeDibDeletedHdcCreateSucceeded !== true ||
    latestEvidence.nativeDibDeletedHdcVariantClaimed !== false ||
    latestEvidence.nativeDibSelectedDeleteResult !== 1 ||
    latestEvidence.nativeDibSelectedDeleteDeferredLifetimeImported !== false ||
    latestEvidence.nativeDibExecutableHashUnchanged !== true ||
    latestEvidence.dibCreateImplemented !== true ||
    latestEvidence.dibCreateImplementationId !==
      "gdi32.create-dib-section.primary-8bpp.v1" ||
    latestEvidence.dibGetObjectImplemented !== true ||
    latestEvidence.dibGetObjectImplementationId !==
      "gdi32.get-object-a.font-or-dib.v1" ||
    latestEvidence.dibSelectImplemented !== true ||
    latestEvidence.dibSelectImplementationId !==
      "gdi32.select-object.memory-font-bitmap.v1" ||
    latestEvidence.dibDeleteObjectImplemented !== true ||
    latestEvidence.dibDeleteObjectImplementationId !==
      "gdi32.delete-object.font-or-dib.v1" ||
    latestEvidence.dibDeleteObjectReachedBeforeNextStop !== true ||
    latestEvidence.dibDeleteObjectSequence !== 18_626 ||
    latestEvidence.dibDeleteObjectInstructionCount !== 12_072_559 ||
    latestEvidence.dibDeleteObjectResult !== 1 ||
    latestEvidence.dibDeleteObjectReturnAddress !== "0x1002c45c" ||
    latestEvidence.fontDeleteObjectSequence !== 18_627 ||
    latestEvidence.fontDeleteObjectInstructionCount !== 12_072_563 ||
    latestEvidence.fontDeleteObjectResult !== 1 ||
    latestEvidence.fontDeleteObjectReturnAddress !== "0x1002c465" ||
    latestEvidence.dibPolicySchema !== "vine.gdi32-compatibility-policy" ||
    latestEvidence.dibPolicySchemaVersion !== 6 ||
    latestEvidence.dibPolicyId !== "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.dibProfileId !==
      "primary-128x-96-8bpp-reverse-grayscale-v1" ||
    latestEvidence.dibCreateSequence !== 18_614 ||
    latestEvidence.dibCreateInstructionCount !== 11_960_150 ||
    latestEvidence.dibCreateArguments?.join(",") !==
      "0x47010004,0x2013a250,0x00000000,0x006ffef4,0x00000000,0x00000000" ||
    latestEvidence.dibCreateReturnAddress !== "0x1002c8c5" ||
    latestEvidence.dibCreateResult !== "0x47020005" ||
    latestEvidence.dibCreateLastError !== 0 ||
    latestEvidence.dibCreateGuestWriteBytes !== 4 ||
    latestEvidence.dibCreateOutputAddress !== "0x006ffef4" ||
    latestEvidence.dibCreateOutputHex !== "00100044" ||
    latestEvidence.dibObjectId !== 5 ||
    latestEvidence.dibHandleNamespace !== "vine.logical-gdi-object.v1" ||
    latestEvidence.dibShared !== false ||
    latestEvidence.dibDescriptor?.join(",") !== "128,-96,1,8,0,128,12288" ||
    latestEvidence.dibPaletteEntries !== 256 ||
    latestEvidence.dibPalette !== "reverse-grayscale-rgbquad" ||
    latestEvidence.dibBackingAddress !== "0x44001000" ||
    latestEvidence.dibBackingBytes !== 12_288 ||
    latestEvidence.dibBackingAddressAllocationId !== 59 ||
    latestEvidence.dibBackingMemoryOwner !== 29 ||
    latestEvidence.dibBackingMemoryRegionKind !== "gdi-resource" ||
    latestEvidence.dibBackingAddressClassification !== "graphics" ||
    latestEvidence.dibBackingAddressLifetime !== "object-explicit-release" ||
    latestEvidence.dibCreateDeltaFromGeneration !== 36_537 ||
    latestEvidence.dibCreateDeltaGeneration !== 36_540 ||
    latestEvidence.dibCreateDeltaMappingOperations !== 2 ||
    latestEvidence.dibCreateDeltaDirtyPages?.join(",") !==
      "0x006ff000:4,0x44001000:29,0x44002000:29,0x44003000:29" ||
    latestEvidence.dibCreateDeltaControlBytes !== 200 ||
    latestEvidence.dibCreateDeltaPayloadBytes !== 16_384 ||
    latestEvidence.dibCreateDeltaPayloadSha256 !==
      "fcd36d346f9edd7098ec07a2d73a83319e75c08e9ac1c9ded494bf2470dcabe2" ||
    latestEvidence.dibGetObjectSequence !== 18_616 ||
    latestEvidence.dibGetObjectInstructionCount !== 11_960_180 ||
    latestEvidence.dibGetObjectArguments?.join(",") !==
      "0x47020005,0x00000018,0x006ffd78" ||
    latestEvidence.dibGetObjectReturnAddress !== "0x1002c8e7" ||
    latestEvidence.dibGetObjectResult !== 24 ||
    latestEvidence.dibGetObjectLastError !== 0 ||
    latestEvidence.dibGetObjectGuestWriteBytes !== 24 ||
    latestEvidence.dibGetObjectOutputHex !==
      "000000008000000060000000800000000100080000100044" ||
    latestEvidence.dibSelectSequences?.join(",") !==
      "18617,18618,18624,18625" ||
    latestEvidence.dibSelectInstructionCounts?.join(",") !==
      "11960192,11960199,12072548,12072554" ||
    latestEvidence.dibSelectHandles?.join(",") !==
      "0x47020005,0x47020002,0x47020004,0x47020003" ||
    latestEvidence.dibSelectPreviousHandles?.join(",") !==
      "0x47020004,0x47020003,0x47020005,0x47020002" ||
    latestEvidence.dibSelectReturnAddresses?.join(",") !==
      "0x1002c90c,0x1002c922,0x1002c43d,0x1002c44d" ||
    latestEvidence.dibSelectLastErrors?.join(",") !== "0,0,0,0" ||
    latestEvidence.dibSelectionGenerations?.join(",") !== "1,2,3,4" ||
    latestEvidence.dibObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.dibByteAuthority !== "rust-guest-memory" ||
    latestEvidence.dibAddressAuthority !==
      "runtime/address_space.rs::ProcessAddressSpaceAllocator" ||
    latestEvidence.dibMutation !==
      "address-gdi-memory-preflight-targeted-commit" ||
    latestEvidence.dibFailureAtomicity !== true ||
    latestEvidence.dibSelectedDeleteRejectedMutationFree !== true ||
    latestEvidence.dibStaleGetObjectPreservesOutput !== true ||
    latestEvidence.dibBrowserBitmapResourceCreated !== false ||
    latestEvidence.dibNodeCompletions !== 7 ||
    latestEvidence.dibChromeCompletions !== 7 ||
    latestEvidence.dibNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest bounded DIB-section lifecycle evidence is invalid");
  }
  for (const field of [
    "nativeOutlineTextMetricsPrimaryTraceSha256",
    "nativeOutlineTextMetricsConfirmationTraceSha256",
    "nativeOutlineTextMetricsNormalizedOutputSha256",
    "outlineTextMetricsOutputSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeOutlineTextMetricsPrimaryTraceBytes !== 673_869 ||
    latestEvidence.nativeOutlineTextMetricsPrimaryTraceLines !== 180 ||
    latestEvidence.nativeOutlineTextMetricsPrimaryTraceSha256 !==
      "0b46e3c68be56b8948bd9a748350f85d68ddbf8751e3362d146686a3639eac6e" ||
    latestEvidence.nativeOutlineTextMetricsConfirmationTraceBytes !== 674_077 ||
    latestEvidence.nativeOutlineTextMetricsConfirmationTraceLines !== 180 ||
    latestEvidence.nativeOutlineTextMetricsConfirmationTraceSha256 !==
      "e7f57e51fab0399c2a53197ab30a187451eeee28388217e5b6a507df473fe002" ||
    latestEvidence.nativeOutlineTextMetricsTraceDiagnostics !== 0 ||
    latestEvidence.nativeOutlineTextMetricsHookErrors !== 0 ||
    latestEvidence.nativeOutlineTextMetricsLifecycleProfile !==
      "x86-bgi-gdi-font-object-lifecycle-v4" ||
    latestEvidence.nativeOutlineTextMetricsOuterOracleProfile !==
      "x86-create-font-a-object-lifecycle-v4" ||
    latestEvidence.nativeOutlineTextMetricsOracleProfile !==
      "x86-get-outline-text-metrics-a-cp932-v1" ||
    latestEvidence.nativeOutlineTextMetricsCallerReturnRvas?.join(",") !==
      "0x0002c51f,0x0002c539" ||
    latestEvidence.nativeOutlineTextMetricsBoundaryRequests?.join(",") !==
      "0,269,1,2,3,4,211,212,213,268,285" ||
    latestEvidence.nativeOutlineTextMetricsBoundaryResults?.join(",") !==
      "269,269,1,2,3,4,211,212,0,0,269" ||
    latestEvidence.nativeOutlineTextMetricsFixedBytes !== 212 ||
    latestEvidence.nativeOutlineTextMetricsRequiredBytes !== 269 ||
    latestEvidence.nativeOutlineTextMetricsPartialSizeMarker !== 290 ||
    latestEvidence.nativeOutlineTextMetricsPartialPrefixesHex?.join(",") !==
      "22,2201,220100" ||
    latestEvidence.nativeOutlineTextMetricsIncompleteLastError !== 1_003 ||
    latestEvidence.nativeOutlineTextMetricsDeletedDcLastError !== 6 ||
    latestEvidence.nativeOutlineTextMetricsCoreFields?.join(",") !==
      "269,64,55,9" ||
    latestEvidence.nativeOutlineTextMetricsStringOffsets?.join(",") !==
      "212,226,240,245" ||
    latestEvidence.nativeOutlineTextMetricsFamilyAndFaceWindows932Hex !==
      "826c827220835383568362834e00" ||
    latestEvidence.nativeOutlineTextMetricsStyleWindows932Hex !==
      "95578f8000" ||
    latestEvidence.nativeOutlineTextMetricsFullNameWindows932Hex !==
      "4d6963726f736f66743a826c827220835383568362834e00" ||
    latestEvidence.nativeOutlineTextMetricsNormalizedOutputSha256 !==
      "d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d" ||
    latestEvidence.nativeOutlineTextMetricsPaddingOffsets?.join(",") !==
      "57,58,59,71" ||
    latestEvidence.nativeOutlineTextMetricsOutputGuardsUnchanged !== true ||
    latestEvidence.nativeOutlineTextMetricsSuccessPreservesLastError !== true ||
    latestEvidence.nativeOutlineTextMetricsExecutableHashUnchanged !== true ||
    latestEvidence.outlineTextMetricsImplemented !== true ||
    latestEvidence.outlineTextMetricsImplementationId !==
      "gdi32.get-outline-text-metrics-a.cp932-selected-font.v1" ||
    latestEvidence.outlineTextMetricsPolicySchema !==
      "vine.gdi32-compatibility-policy" ||
    latestEvidence.outlineTextMetricsPolicySchemaVersion !== 6 ||
    latestEvidence.outlineTextMetricsPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.outlineTextMetricsProfileId !==
      "primary-cp932-ms-gothic-outline-metrics-v1" ||
    latestEvidence.outlineTextMetricsSequences?.join(",") !== "18619,18621" ||
    latestEvidence.outlineTextMetricsInstructionCounts?.join(",") !==
      "11960243,11960296" ||
    JSON.stringify(latestEvidence.outlineTextMetricsArguments) !==
      JSON.stringify([
        ["0x47010004", "0x00000000", "0x00000000"],
        ["0x47010004", "0x0000010d", "0x2013a250"],
      ]) ||
    latestEvidence.outlineTextMetricsReturnAddresses?.join(",") !==
      "0x1002c51f,0x1002c539" ||
    latestEvidence.outlineTextMetricsResults?.join(",") !== "269,269" ||
    latestEvidence.outlineTextMetricsLastErrors?.join(",") !== "0,0" ||
    latestEvidence.outlineTextMetricsGuestWriteBytes?.join(",") !== "0,269" ||
    latestEvidence.outlineTextMetricsOutputAddress !== "0x2013a250" ||
    latestEvidence.outlineTextMetricsOutputBytes !== 269 ||
    latestEvidence.outlineTextMetricsOutputSha256 !==
      "d1ac50ba2ccbb40beb818d7ec02d980906bdfd341ffc66c8abf577e8c1ff2c5d" ||
    latestEvidence.outlineTextMetricsDeviceContext !== "0x47010004" ||
    latestEvidence.outlineTextMetricsDeviceContextId !== 4 ||
    latestEvidence.outlineTextMetricsSelectedFont !== "0x47020002" ||
    latestEvidence.outlineTextMetricsSelectedFontId !== 2 ||
    latestEvidence.outlineTextMetricsSelectionGeneration !== 2 ||
    latestEvidence.outlineTextMetricsMetricsAuthority !==
      "gdi32-compatibility-policy" ||
    latestEvidence.outlineTextMetricsSelectedFontAuthority !==
      "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.outlineTextMetricsByteAuthority !== "rust-guest-memory" ||
    latestEvidence.outlineTextMetricsNativePaddingPolicy !==
      "zero-four-nondeterministic-padding-bytes" ||
    latestEvidence.outlineTextMetricsHostFontMeasurementImported !== false ||
    latestEvidence.outlineTextMetricsBrowserFontMeasurementImported !== false ||
    latestEvidence.outlineTextMetricsFailureAtomicity !== true ||
    latestEvidence.outlineTextMetricsNodeCompletions !== 2 ||
    latestEvidence.outlineTextMetricsChromeCompletions !== 2 ||
    latestEvidence.outlineTextMetricsNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest outline-text-metrics evidence is invalid");
  }
  for (const field of [
    "nativeTextRasterPrimaryTraceSha256",
    "nativeTextRasterConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeTextRasterPrimaryTraceBytes !== 971_658 ||
    latestEvidence.nativeTextRasterPrimaryTraceLines !== 196 ||
    latestEvidence.nativeTextRasterPrimaryTraceSha256 !==
      "e0dd6369700f5845aaa10784b0f52b7ad72be10d94a32c6ea734b91255830db1" ||
    latestEvidence.nativeTextRasterConfirmationTraceBytes !== 971_761 ||
    latestEvidence.nativeTextRasterConfirmationTraceLines !== 196 ||
    latestEvidence.nativeTextRasterConfirmationTraceSha256 !==
      "6cab9c4794792f7cca125ea4839f7d3ca6ef0f7211fea8c63ec718d8a4a2f2c6" ||
    latestEvidence.nativeTextRasterTraceDiagnostics !== 0 ||
    latestEvidence.nativeTextRasterHookErrors !== 0 ||
    latestEvidence.nativeTextRasterLifecycleProfile !==
      "x86-bgi-gdi-font-object-lifecycle-v5" ||
    latestEvidence.nativeTextRasterOuterOracleProfile !==
      "x86-create-font-a-object-lifecycle-v5" ||
    latestEvidence.nativeTextRasterOracleProfile !==
      "x86-text-out-a-8bpp-raster-v1" ||
    latestEvidence.nativeTextRasterCallerReturnRva !== "0x0002cab1" ||
    latestEvidence.nativeTextRasterTextWindows932Hex !== "61" ||
    latestEvidence.nativeTextRasterPosition?.join(",") !== "0,0" ||
    latestEvidence.nativeTextRasterResult !== 1 ||
    latestEvidence.nativeTextRasterLastError !== 0 ||
    latestEvidence.nativeTextRasterInitialPixelPlaneFnv1a32 !== "0x52965dc5" ||
    latestEvidence.nativeTextRasterOutputPixelPlaneFnv1a32 !== "0xe6fc8461" ||
    latestEvidence.nativeTextRasterChangedBytes !== 476 ||
    latestEvidence.nativeTextRasterBoundingBox?.join(",") !== "3,28,20,54" ||
    latestEvidence.nativeTextRasterChangedValues?.join(",") !== "0,255" ||
    latestEvidence.nativeTextRasterRepeatChangedBytes !== 0 ||
    latestEvidence.nativeTextRasterZeroLengthChangedBytes !== 0 ||
    latestEvidence.nativeTextRasterZeroLengthPreservesLastError !== true ||
    latestEvidence.nativeTextRasterDeletedDcResult !== 0 ||
    latestEvidence.nativeTextRasterDeletedDcLastError !== 0 ||
    latestEvidence.nativeTextRasterHookResultsMatchSystem !== true ||
    latestEvidence.nativeTextRasterHookLastErrorsMatchSystem !== true ||
    latestEvidence.nativeTextRasterExecutableHashUnchanged !== true ||
    latestEvidence.textRasterImplemented !== true ||
    latestEvidence.textRasterImplementationId !==
      "gdi32.text-out-a.cp932-8bpp-raster.v1" ||
    latestEvidence.textRasterPolicySchema !==
      "vine.gdi32-compatibility-policy" ||
    latestEvidence.textRasterPolicySchemaVersion !== 6 ||
    latestEvidence.textRasterPolicyId !==
      "gdi32-primary-cp932-text-raster-v6" ||
    latestEvidence.textRasterProfileId !==
      "primary-cp932-ms-gothic-a-8bpp-raster-v1" ||
    latestEvidence.textRasterSequence !== 18_623 ||
    latestEvidence.textRasterInstructionCount !== 11_961_619 ||
    latestEvidence.textRasterArguments?.join(",") !==
      "0x47010004,0x00000000,0x00000000,0x006ffd6c,0x00000001" ||
    latestEvidence.textRasterReturnAddress !== "0x1002cab1" ||
    latestEvidence.textRasterResult !== 1 ||
    latestEvidence.textRasterLastError !== 0 ||
    latestEvidence.textRasterGuestWriteAddress !== "0x44001a00" ||
    latestEvidence.textRasterGuestWriteBytes !== 4_384 ||
    latestEvidence.textRasterChangedBytes !== 476 ||
    latestEvidence.textRasterDeviceContext !== "0x47010004" ||
    latestEvidence.textRasterDeviceContextId !== 4 ||
    latestEvidence.textRasterSelectedFont !== "0x47020002" ||
    latestEvidence.textRasterSelectedFontId !== 2 ||
    latestEvidence.textRasterSelectedDib !== "0x47020005" ||
    latestEvidence.textRasterSelectedDibId !== 5 ||
    latestEvidence.textRasterSelectionGeneration !== 2 ||
    latestEvidence.textRasterContentGeneration !== 1 ||
    latestEvidence.textRasterInitialPixelPlaneFnv1a32 !== "0x52965dc5" ||
    latestEvidence.textRasterOutputPixelPlaneFnv1a32 !== "0xe6fc8461" ||
    latestEvidence.textRasterObjectAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.textRasterByteAuthority !== "rust-guest-memory" ||
    latestEvidence.textRasterMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    latestEvidence.textRasterFailureAtomicity !== true ||
    latestEvidence.textRasterHostFontRasterizationImported !== false ||
    latestEvidence.textRasterBrowserFontRasterizationImported !== false ||
    latestEvidence.textRasterBrowserBitmapResourceCreated !== false ||
    latestEvidence.textRasterNodeCompletions !== 1 ||
    latestEvidence.textRasterChromeCompletions !== 1 ||
    latestEvidence.textRasterNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest bounded TextOutA raster evidence is invalid");
  }
  for (const field of [
    "nativeGdiplusStartupPrimaryTraceSha256",
    "nativeGdiplusStartupConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeGdiplusStartupPrimaryTraceBytes !== 51_006 ||
    latestEvidence.nativeGdiplusStartupPrimaryTraceLines !== 77 ||
    latestEvidence.nativeGdiplusStartupPrimaryTraceSha256 !==
      "456f66d1242523aa5a2d351ad426c01db23f3c0d3c75909f5e135ebc4f4aa3a6" ||
    latestEvidence.nativeGdiplusStartupConfirmationTraceBytes !== 51_076 ||
    latestEvidence.nativeGdiplusStartupConfirmationTraceLines !== 77 ||
    latestEvidence.nativeGdiplusStartupConfirmationTraceSha256 !==
      "800aef21184c47b9d9052006618070457c1d3301972fa24c5d72a2168ec8c4f0" ||
    latestEvidence.nativeGdiplusStartupTraceDiagnostics !== 0 ||
    latestEvidence.nativeGdiplusStartupHookErrors !== 0 ||
    latestEvidence.nativeGdiplusStartupProfile !==
      "x86-bgi-gdiplus-startup-v1" ||
    latestEvidence.nativeGdiplusStartupCallsPerTrace !== 1 ||
    latestEvidence.nativeGdiplusStartupCallerReturnRva !== "0x000652c8" ||
    latestEvidence.nativeGdiplusStartupInputBytes !== 16 ||
    latestEvidence.nativeGdiplusStartupVersion !== 1 ||
    latestEvidence.nativeGdiplusStartupDebugEventCallback !== "0x00000000" ||
    latestEvidence.nativeGdiplusStartupSuppressBackgroundThread !== false ||
    latestEvidence.nativeGdiplusStartupSuppressExternalCodecs !== false ||
    latestEvidence.nativeGdiplusStartupOutputNull !== true ||
    latestEvidence.nativeGdiplusStartupTokenInitiallyZero !== true ||
    latestEvidence.nativeGdiplusStartupTokenNonzero !== true ||
    latestEvidence.nativeGdiplusStartupTokenVariesAcrossRuns !== true ||
    latestEvidence.nativeGdiplusStartupResult !== 0 ||
    latestEvidence.nativeGdiplusStartupLastError !== 0 ||
    latestEvidence.nativeGdiplusStartupConsumer !==
      "wrapper-return-without-eax-branch-cleanup-load-token-shutdown-and-zero-global" ||
    latestEvidence.nativeGdiplusStartupDownstreamCallsBeforeFocusedCompletion !==
      0 ||
    latestEvidence.nativeGdiplusStartupExecutableHashUnchanged !== true ||
    latestEvidence.gdiplusStartupImplemented !== true ||
    latestEvidence.gdiplusStartupImplementationId !==
      "gdiplus.startup.primary-v1" ||
    latestEvidence.gdiplusStartupPolicySchema !==
      "vine.gdiplus-compatibility-policy" ||
    latestEvidence.gdiplusStartupPolicySchemaVersion !== 1 ||
    latestEvidence.gdiplusStartupPolicyId !== "gdiplus-primary-startup-v1" ||
    latestEvidence.gdiplusStartupProfileId !== "primary-x86-callback-free-v1" ||
    latestEvidence.gdiplusStartupSequence !== 18_638 ||
    latestEvidence.gdiplusStartupInstructionCount !== 12_073_229 ||
    latestEvidence.gdiplusStartupArguments?.join(",") !==
      "0x10151f8c,0x006fff44,0x00000000" ||
    latestEvidence.gdiplusStartupReturnAddress !== "0x100652c8" ||
    latestEvidence.gdiplusStartupResult !== 0 ||
    latestEvidence.gdiplusStartupLastError !== 0 ||
    latestEvidence.gdiplusStartupGuestWriteAddress !== "0x10151f8c" ||
    latestEvidence.gdiplusStartupGuestWriteBytes !== 4 ||
    latestEvidence.gdiplusStartupGuestWriteHex !== "01005047" ||
    latestEvidence.gdiplusStartupGuestWriteGeneration !== 36_579 ||
    latestEvidence.gdiplusStartupTokenNamespace !==
      "vine.logical-gdiplus-session-token.v1" ||
    latestEvidence.gdiplusStartupToken !== "0x47500001" ||
    latestEvidence.gdiplusStartupTokenId !== 1 ||
    latestEvidence.gdiplusStartupSessionId !== 1 ||
    latestEvidence.gdiplusStartupSessionGeneration !== 1 ||
    latestEvidence.gdiplusStartupLiveSessions !== 1 ||
    latestEvidence.gdiplusStartupPolicyAuthority !==
      "rust-gdiplus-compatibility-policy" ||
    latestEvidence.gdiplusStartupSessionAuthority !==
      "runtime-gdiplus-logical-session" ||
    latestEvidence.gdiplusStartupMutation !==
      "owner-plan-stale-check-transactional-commit" ||
    latestEvidence.gdiplusStartupFailureAtomicity !== true ||
    latestEvidence.gdiplusStartupNativeTokenImported !== false ||
    latestEvidence.gdiplusStartupBrowserResourceCreated !== false ||
    latestEvidence.gdiplusStartupImageCodecImported !== false ||
    latestEvidence.gdiplusStartupNodeCompletions !== 1 ||
    latestEvidence.gdiplusStartupChromeCompletions !== 1 ||
    latestEvidence.gdiplusStartupNodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.gdiplusShutdownContractImplemented !== true ||
    latestEvidence.gdiplusShutdownImplementationId !==
      "gdiplus.shutdown.logical-session-v1" ||
    latestEvidence.gdiplusShutdownReachedBeforeNextStop !== false ||
    latestEvidence.gdiplusProcessExitRetiresSessions !== true
  ) {
    errors.push("latest GDI+ startup session evidence is invalid");
  }
  for (const field of [
    "nativeVirtualAllocPrimaryTraceSha256",
    "nativeVirtualAllocConfirmationTraceSha256",
    "nativeVirtualMemoryLifecycleTraceSha256",
    "virtualAllocDeltaPayloadSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeVirtualAllocPrimaryTraceBytes !== 48_303 ||
    latestEvidence.nativeVirtualAllocPrimaryTraceLines !== 77 ||
    latestEvidence.nativeVirtualAllocPrimaryTraceSha256 !==
      "661e5a17058890dca9e9c6a7217334fb6f13e66d4ac5cb589dc84b87d0a99e2c" ||
    latestEvidence.nativeVirtualAllocConfirmationTraceBytes !== 48_324 ||
    latestEvidence.nativeVirtualAllocConfirmationTraceLines !== 77 ||
    latestEvidence.nativeVirtualAllocConfirmationTraceSha256 !==
      "725d02d0f16f969ef79c25d64c824243940b40c50496189759cb1dc003c2d198" ||
    latestEvidence.nativeVirtualMemoryLifecycleTraceBytes !== 48_586 ||
    latestEvidence.nativeVirtualMemoryLifecycleTraceLines !== 78 ||
    latestEvidence.nativeVirtualMemoryLifecycleTraceSha256 !==
      "e4e1d08da69b0b04d44e0426d024abcd7520d3fe3c7b9ff8a3be78d793aec9f7" ||
    latestEvidence.nativeVirtualAllocTraceDiagnostics !== 0 ||
    latestEvidence.nativeVirtualAllocHookErrors !== 0 ||
    latestEvidence.nativeVirtualAllocProfile !==
      "x86-bgi-virtual-alloc-reserve-top-down-v1" ||
    latestEvidence.nativeVirtualAllocCallsPerTrace !== 1 ||
    latestEvidence.nativeVirtualAllocCallerReturnRva !== "0x0008fba8" ||
    latestEvidence.nativeVirtualAllocPreferredAddress !== "0x00000000" ||
    latestEvidence.nativeVirtualAllocRequestedBytes !== 96 * 1024 * 1024 ||
    latestEvidence.nativeVirtualAllocAllocationType !== "0x00102000" ||
    latestEvidence.nativeVirtualAllocProtection !== "0x00000004" ||
    latestEvidence.nativeVirtualAllocResultsNonNull !== true ||
    latestEvidence.nativeVirtualAllocResultsVaryAcrossRuns !== true ||
    latestEvidence.nativeVirtualAllocLastError !== 0 ||
    latestEvidence.nativeVirtualAllocMemoryState !== "0x00002000" ||
    latestEvidence.nativeVirtualAllocMemoryType !== "0x00020000" ||
    latestEvidence.nativeVirtualAllocMemoryProtect !== "0x00000000" ||
    latestEvidence.nativeVirtualAllocAllocationProtect !== "0x00000004" ||
    latestEvidence.nativeVirtualAllocRegionBytes !== 96 * 1024 * 1024 ||
    latestEvidence.nativeVirtualAllocConsumer !==
      "store-eax-global-and-return" ||
    latestEvidence.nativeVirtualAllocCommitDecommitReleaseReachedDuringFocusedTrace !==
      false ||
    latestEvidence.nativeVirtualAllocExecutableHashUnchanged !== true ||
    latestEvidence.virtualAllocImplemented !== true ||
    latestEvidence.virtualAllocImplementationId !==
      "kernel32.virtual-alloc.reserve-top-down.v1" ||
    latestEvidence.virtualAllocPolicySchema !==
      "vine.kernel32-compatibility-policy" ||
    latestEvidence.virtualAllocPolicySchemaVersion !== 2 ||
    latestEvidence.virtualAllocPolicyId !==
      "kernel32-virtual-alloc-reserve-v1" ||
    latestEvidence.virtualAllocSequence !== 18_641 ||
    latestEvidence.virtualAllocInstructionCount !== 12_073_370 ||
    latestEvidence.virtualAllocArguments?.join(",") !==
      "0x00000000,0x06000000,0x00102000,0x00000004" ||
    latestEvidence.virtualAllocReturnAddress !== "0x1008fba8" ||
    latestEvidence.virtualAllocResult !== "0x79000000" ||
    latestEvidence.virtualAllocLastError !== 0 ||
    latestEvidence.virtualAllocGuestWriteBytes !== 0 ||
    latestEvidence.virtualAllocAddressAllocationId !== 60 ||
    latestEvidence.virtualAllocAddressOwner !== "runtime-virtual-memory" ||
    latestEvidence.virtualAllocAddressBase !== "0x79000000" ||
    latestEvidence.virtualAllocReservedBytes !== 96 * 1024 * 1024 ||
    latestEvidence.virtualAllocAlignment !== 65_536 ||
    latestEvidence.virtualAllocMemoryOwner !== 30 ||
    latestEvidence.virtualAllocRegionKind !== "anonymous" ||
    latestEvidence.virtualAllocReserveOnly !== true ||
    latestEvidence.virtualAllocTopDown !== true ||
    latestEvidence.virtualAllocDeltaSequence !== 18_641 ||
    latestEvidence.virtualAllocDeltaFromGeneration !== 36_584 ||
    latestEvidence.virtualAllocDeltaGeneration !== 36_585 ||
    latestEvidence.virtualAllocDeltaMappingOperations !== 1 ||
    latestEvidence.virtualAllocDeltaDirtyPages !== 0 ||
    latestEvidence.virtualAllocDeltaControlBytes !== 96 ||
    latestEvidence.virtualAllocDeltaPayloadBytes !== 0 ||
    latestEvidence.virtualAllocDeltaPayloadSha256 !==
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ||
    latestEvidence.virtualAllocReservedPageIncrease !== 24_576 ||
    latestEvidence.virtualAllocPhysicalPageIncrease !== 0 ||
    latestEvidence.virtualAllocPolicyAuthority !==
      "rust-kernel32-compatibility-policy" ||
    latestEvidence.virtualAllocAddressAuthority !==
      "runtime-process-address-space" ||
    latestEvidence.virtualAllocByteAuthority !== "rust-guest-memory" ||
    latestEvidence.virtualAllocMutation !==
      "cross-owner-preflight-transactional-commit" ||
    latestEvidence.virtualAllocFailureAtomicity !== true ||
    latestEvidence.virtualAllocHostAddressImported !== false ||
    latestEvidence.virtualAllocBrowserAddressImported !== false ||
    latestEvidence.virtualAllocNodeCompletions !== 1 ||
    latestEvidence.virtualAllocChromeCompletions !== 1 ||
    latestEvidence.virtualAllocNodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.virtualAllocCommitVariantImplemented !== false ||
    latestEvidence.virtualFreeImplemented !== false
  ) {
    errors.push("latest VirtualAlloc reserve evidence is invalid");
  }
  for (const field of [
    "nativeD3d9GetSurfaceLevelPrimaryTraceSha256",
    "nativeD3d9GetSurfaceLevelPrimarySummarySha256",
    "nativeD3d9GetSurfaceLevelConfirmationTraceSha256",
    "nativeD3d9GetSurfaceLevelConfirmationSummarySha256",
    "d3d9GetSurfaceLevelOracleSourceSha256",
    "d3d9GetSurfaceLevelOracleExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceBytes !== 228_637 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceLines !== 514 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryTraceSha256 !==
      "54aa56bade26d675f864162bb7a7a2f8b4fc30f8df8dbd0835681cc09f9eced2" ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimaryBalancedApiPairs !== 222 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimarySummaryBytes !== 230_104 ||
    latestEvidence.nativeD3d9GetSurfaceLevelPrimarySummarySha256 !==
      "c68ef403d864c89ad466ed3bda3cef750065d7555b815fb23f3ad99a43697c8e" ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceBytes !==
      229_203 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceLines !== 514 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationTraceSha256 !==
      "e3f6d853293cfa8f395695c2de21cb4675cc71f27efa4b3715f2171575ad1422" ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationBalancedApiPairs !==
      222 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationSummaryBytes !==
      230_131 ||
    latestEvidence.nativeD3d9GetSurfaceLevelConfirmationSummarySha256 !==
      "1ed3b5f247a8b6dd82efd2cdaa7cb0393e68d7db9639d07eaa1afa2244b4a81f" ||
    latestEvidence.nativeD3d9GetSurfaceLevelTraceDiagnostics !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelCallsPerTrace !== 1 ||
    latestEvidence.nativeD3d9GetSurfaceLevelCallerRva !== "0x0005a731" ||
    latestEvidence.nativeD3d9GetSurfaceLevelLevel !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelResult !== "0x00000000" ||
    latestEvidence.nativeD3d9GetSurfaceLevelLastError !== 0 ||
    latestEvidence.nativeD3d9GetSurfaceLevelOutputNonNull !== true ||
    latestEvidence.nativeD3d9GetSurfaceLevelHresultSignConsumed !== true ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextMethod !== "GetDC" ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextSlot !== 15 ||
    latestEvidence.nativeD3d9GetSurfaceLevelImmediateNextCallerRva !==
      "0x0005a746" ||
    latestEvidence.nativeD3d9GetSurfaceLevelExecutableHashUnchanged !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleArchitecture !== "coff-i386" ||
    latestEvidence.d3d9GetSurfaceLevelOracleSourceBytes !== 13_977 ||
    latestEvidence.d3d9GetSurfaceLevelOracleSourceSha256 !==
      "1ec0c191bb9783abcbfae7b92ba6d7136018111c4b1da7c860d7dd343daca562" ||
    latestEvidence.d3d9GetSurfaceLevelOracleExecutableBytes !== 12_288 ||
    latestEvidence.d3d9GetSurfaceLevelOracleExecutableSha256 !==
      "7ca28333d06b7352b032b0af9ceb22a016e03d5830f17a74231569ef2b2fee9e" ||
    latestEvidence.d3d9GetSurfaceLevelOracleRepeatSamePointer !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleRepeatSameIunknown !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleSurfaceIunknownDistinctFromTexture !==
      true ||
    latestEvidence.d3d9GetSurfaceLevelOracleContainerIsTexture !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleInvalidLevelResult !==
      "0x8876086c" ||
    latestEvidence.d3d9GetSurfaceLevelOracleInvalidLevelZerosOutput !== true ||
    latestEvidence.d3d9GetSurfaceLevelOracleNullOutputException !==
      "0xc0000005" ||
    latestEvidence.d3d9GetSurfaceLevelOracleSuccessPreservesLastError !==
      true ||
    latestEvidence.d3d9GetSurfaceLevelImplemented !== true ||
    latestEvidence.d3d9GetSurfaceLevelSequence !== 17_851 ||
    latestEvidence.d3d9GetSurfaceLevelInstructionCount !== 11_807_783 ||
    latestEvidence.d3d9GetSurfaceLevelArguments?.join(",") !==
      "0x01020004,0x00000000,0x006ffd38" ||
    latestEvidence.d3d9GetSurfaceLevelReturnAddress !== "0x1005a731" ||
    latestEvidence.d3d9GetSurfaceLevelResult !== "0x00000000" ||
    latestEvidence.d3d9GetSurfaceLevelLastError !== 0 ||
    latestEvidence.d3d9GetSurfaceLevelGuestWriteBytes !== 4 ||
    latestEvidence.d3d9GetSurfaceLevelOutputPointer !== "0x01030004" ||
    latestEvidence.d3d9GetSurfaceLevelPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9GetSurfaceLevelPolicyVersion !== 22 ||
    latestEvidence.d3d9GetSurfaceLevelPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9GetSurfaceLevelProfileId !==
      "primary-system-memory-texture-level-zero-surfaces-v2" ||
    latestEvidence.d3d9GetSurfaceLevelGraphicsAuthority !==
      "runtime/graphics.rs::RuntimeGraphics" ||
    latestEvidence.d3d9GetSurfaceLevelComAuthority !==
      "runtime/com.rs::RuntimeCom" ||
    latestEvidence.d3d9GetSurfaceLevelClassIdentity !==
      "d3d9.idirect3dsurface9.v1" ||
    latestEvidence.d3d9GetSurfaceLevelInterface !== "IDirect3DSurface9" ||
    latestEvidence.d3d9GetSurfaceLevelVtableSlots !== 17 ||
    latestEvidence.d3d9GetSurfaceLevelTextureReferenceCount !== 2 ||
    latestEvidence.d3d9GetSurfaceLevelReferenceCount !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelRetainsTextureReference !== true ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsTextureId !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceId !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelLogicalGraphicsSurfaceNamespace !==
      "vine.logical-graphics-surface.v1" ||
    latestEvidence.d3d9GetSurfaceLevelLogicalDescriptor?.join(",") !==
      "0,1024,1024,22" ||
    latestEvidence.d3d9GetSurfaceLevelRepeatReusesIdentityAndAddsReference !==
      true ||
    latestEvidence.d3d9GetSurfaceLevelGuestComGraphicsAtomicity !== true ||
    latestEvidence.d3d9GetSurfaceLevelFailureAtomicity !== true ||
    latestEvidence.d3d9GetSurfaceLevelFinalReleaseRetiresGraphicsAndMayCascadeTextureDeviceRoot !==
      true ||
    latestEvidence.d3d9GetSurfaceLevelBrowserOwnsComOrGraphicsState !== false ||
    latestEvidence.d3d9GetSurfaceLevelRendererResourceCreated !== false ||
    latestEvidence.d3d9GetSurfaceLevelNodeCompletions !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelChromeCompletions !== 1 ||
    latestEvidence.d3d9GetSurfaceLevelNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest D3D9 GetSurfaceLevel evidence is invalid");
  }
  for (const field of [
    "nativeD3d9SurfaceDcPrimaryTraceSha256",
    "nativeD3d9SurfaceDcPrimarySummarySha256",
    "nativeD3d9SurfaceDcConfirmationTraceSha256",
    "nativeD3d9SurfaceDcConfirmationSummarySha256",
    "d3d9SurfaceDcOracleSourceSha256",
    "d3d9SurfaceDcOracleExecutableSha256",
    "d3d9SurfaceDcOracleNullOutputExecutableSha256",
    "d3d9SurfaceDcOracleReadOnlyOutputExecutableSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  const latestSurfaceDcEvidencePairs = [
    [
      latestEvidence.nativeD3d9SurfaceDcPrimaryTraceBytes,
      d3d9NativeSurfaceDc.primaryTraceBytes,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcPrimaryTraceLines,
      d3d9NativeSurfaceDc.primaryTraceLines,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcPrimaryTraceSha256,
      d3d9NativeSurfaceDc.primaryTraceSha256,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcPrimaryBalancedApiPairs,
      d3d9NativeSurfaceDc.primaryBalancedApiPairs,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcPrimarySummaryBytes,
      d3d9NativeSurfaceDc.primarySummaryBytes,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcPrimarySummarySha256,
      d3d9NativeSurfaceDc.primarySummarySha256,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationTraceBytes,
      d3d9NativeSurfaceDc.confirmationTraceBytes,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationTraceLines,
      d3d9NativeSurfaceDc.confirmationTraceLines,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationTraceSha256,
      d3d9NativeSurfaceDc.confirmationTraceSha256,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationBalancedApiPairs,
      d3d9NativeSurfaceDc.confirmationBalancedApiPairs,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationSummaryBytes,
      d3d9NativeSurfaceDc.confirmationSummaryBytes,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcConfirmationSummarySha256,
      d3d9NativeSurfaceDc.confirmationSummarySha256,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcTraceDiagnostics,
      d3d9NativeSurfaceDc.diagnostics,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcProfileId,
      d3d9NativeSurfaceDc.profileId,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcCallsPerTrace,
      d3d9NativeSurfaceDc.callsPerTrace,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcCallerRvas,
      d3d9NativeSurfaceDc.callerRvas,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcOrderedMethods,
      d3d9NativeSurfaceDc.orderedMethods,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcGetDcResult,
      d3d9NativeSurfaceDc.getDcResult,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcGetDcOutputNonNull,
      d3d9NativeSurfaceDc.getDcOutputNonNull,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcStockObjectIndex,
      d3d9NativeSurfaceDc.stockObjectIndex,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcStockObjectResultNonNull,
      d3d9NativeSurfaceDc.stockObjectResultNonNull,
    ],
    [latestEvidence.nativeD3d9SurfaceDcRect, d3d9NativeSurfaceDc.rect],
    [
      latestEvidence.nativeD3d9SurfaceDcFillRectResult,
      d3d9NativeSurfaceDc.fillRectResult,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcReleaseDcResult,
      d3d9NativeSurfaceDc.releaseDcResult,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcSurfaceReleaseResult,
      d3d9NativeSurfaceDc.surfaceReleaseResult,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcLastErrors,
      d3d9NativeSurfaceDc.lastErrors,
    ],
    [
      latestEvidence.nativeD3d9SurfaceDcExecutableHashUnchanged,
      d3d9NativeSurfaceDc.sourceExecutableHashUnchanged,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleArchitecture,
      d3d9ControlledSurfaceDc.architecture,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleSourceBytes,
      d3d9ControlledSurfaceDc.sourceBytes,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleSourceSha256,
      d3d9ControlledSurfaceDc.sourceSha256,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleExecutableBytes,
      d3d9ControlledSurfaceDc.executableBytes,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleExecutableSha256,
      d3d9ControlledSurfaceDc.executableSha256,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleNullOutputExecutableSha256,
      d3d9ControlledSurfaceDc.nullOutputExecutableSha256,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleReadOnlyOutputExecutableSha256,
      d3d9ControlledSurfaceDc.readOnlyOutputExecutableSha256,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleFirstGetDcResult,
      d3d9ControlledSurfaceDc.firstGetDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleRepeatGetDcResult,
      d3d9ControlledSurfaceDc.repeatGetDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleRepeatOutputPreserved,
      d3d9ControlledSurfaceDc.repeatGetDcPreservesOutput,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleReacquireResult,
      d3d9ControlledSurfaceDc.reacquireGetDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleReferenceCounts,
      d3d9ControlledSurfaceDc.surfaceReferenceCounts,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleWrongReleaseResult,
      d3d9ControlledSurfaceDc.wrongReleaseDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleRepeatedReleaseResult,
      d3d9ControlledSurfaceDc.repeatedReleaseDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleCorrectReleaseResult,
      d3d9ControlledSurfaceDc.correctReleaseDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleLockWhileDcResult,
      d3d9ControlledSurfaceDc.lockRectWhileDcResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleLockAfterReleaseResult,
      d3d9ControlledSurfaceDc.lockRectAfterReleaseResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleNullOutputException,
      d3d9ControlledSurfaceDc.nullOutputException,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleReadOnlyOutputException,
      d3d9ControlledSurfaceDc.readOnlyOutputException,
    ],
    [
      latestEvidence.d3d9SurfaceDcOracleSuccessPreservesLastError,
      d3d9ControlledSurfaceDc.successPreservesLastError,
    ],
    [latestEvidence.d3d9SurfaceDcProfileId, d3d9SurfaceDc.profileId],
    [
      latestEvidence.d3d9SurfaceDcNodeSequences,
      d3d9PrivateSurfaceDc.nodeSequences,
    ],
    [
      latestEvidence.d3d9SurfaceDcNodeInstructionCounts,
      d3d9PrivateSurfaceDc.nodeInstructionCounts,
    ],
    [
      latestEvidence.d3d9SurfaceDcChromeSequences,
      d3d9PrivateSurfaceDc.chromeSequences,
    ],
    [
      latestEvidence.d3d9SurfaceDcCallerReturnAddresses,
      d3d9PrivateSurfaceDc.callerReturnAddresses,
    ],
    [
      latestEvidence.d3d9SurfaceDcSurfacePointer,
      d3d9PrivateSurfaceDc.surfacePointer,
    ],
    [
      latestEvidence.d3d9SurfaceDcOutputAddress,
      d3d9PrivateSurfaceDc.getDcOutputAddress,
    ],
    [
      latestEvidence.d3d9SurfaceDcHandle,
      d3d9PrivateSurfaceDc.deviceContextHandle,
    ],
    [
      latestEvidence.d3d9SurfaceDcDeviceContextId,
      d3d9PrivateSurfaceDc.deviceContextId,
    ],
    [
      latestEvidence.d3d9SurfaceDcHandleNamespace,
      d3d9PrivateSurfaceDc.deviceContextNamespace,
    ],
    [
      latestEvidence.d3d9SurfaceDcTargetKind,
      d3d9PrivateSurfaceDc.deviceContextTargetKind,
    ],
    [
      latestEvidence.d3d9SurfaceDcGraphicsAuthority,
      d3d9SurfaceDc.graphicsSurfaceAuthority,
    ],
    [
      latestEvidence.d3d9SurfaceDcGdiAuthority,
      d3d9SurfaceDc.deviceContextAuthority,
    ],
    [
      latestEvidence.d3d9SurfaceDcLogicalGraphicsSurfaceId,
      d3d9PrivateSurfaceDc.logicalGraphicsSurfaceId,
    ],
    [
      latestEvidence.d3d9SurfaceDcSurfaceComObjectId,
      d3d9PrivateSurfaceDc.surfaceComObjectId,
    ],
    [
      latestEvidence.d3d9SurfaceDcSurfaceGeometry,
      d3d9PrivateSurfaceDc.surfaceGeometry,
    ],
    [
      latestEvidence.d3d9SurfaceDcColorFormat,
      d3d9PrivateSurfaceDc.surfaceColorFormat,
    ],
    [latestEvidence.d3d9SurfaceDcFillRect, d3d9PrivateSurfaceDc.fillRect],
    [
      latestEvidence.d3d9SurfaceDcFillColorRef,
      d3d9PrivateSurfaceDc.fillColorRef,
    ],
    [
      latestEvidence.d3d9SurfaceDcContentGeneration,
      d3d9PrivateSurfaceDc.surfaceContentGeneration,
    ],
    [
      latestEvidence.d3d9SurfaceDcSurfaceGeneration,
      d3d9PrivateSurfaceDc.surfaceGeneration,
    ],
    [
      latestEvidence.d3d9SurfaceDcSingleOutstandingPerSurface,
      d3d9SurfaceDc.singleOutstandingDeviceContextPerSurface,
    ],
    [
      latestEvidence.d3d9SurfaceDcGuestWriteAndGdiPublicationAtomic,
      d3d9SurfaceDc.acquisitionAndGuestWriteAtomic,
    ],
    [
      latestEvidence.d3d9SurfaceDcLiveHdcBlocksComRetirement,
      d3d9SurfaceDc.liveDeviceContextBlocksSurfaceRetirement,
    ],
    [
      latestEvidence.d3d9SurfaceDcFinalReferenceCount,
      d3d9PrivateSurfaceDc.surfaceReleaseReferenceCount,
    ],
    [
      latestEvidence.d3d9SurfaceDcObjectRetired,
      d3d9PrivateSurfaceDc.surfaceObjectRetired,
    ],
    [
      latestEvidence.d3d9SurfaceDcLogicalGraphicsSurfaceCountAfterRelease,
      d3d9PrivateSurfaceDc.logicalGraphicsSurfaceCountAfterRelease,
    ],
    [
      latestEvidence.d3d9SurfaceDcSecondCreateTextureSequence,
      d3d9PrivateSurfaceDc.secondCreateTextureSequence,
    ],
    [
      latestEvidence.d3d9SurfaceDcSecondCreateTextureInstructionCount,
      d3d9PrivateSurfaceDc.secondCreateTextureInstructionCount,
    ],
    [
      latestEvidence.d3d9SurfaceDcSecondCreateTexturePool,
      d3d9PrivateSurfaceDc.secondCreateTexturePool,
    ],
    [
      latestEvidence.d3d9SurfaceDcSecondCreateTextureResult,
      d3d9PrivateSurfaceDc.secondCreateTextureResult,
    ],
    [
      latestEvidence.d3d9SurfaceDcRendererResourceCreated,
      d3d9PrivateSurfaceDc.browserRendererCreated,
    ],
    [
      latestEvidence.d3d9SurfaceDcPixelStorageCreated,
      d3d9PrivateSurfaceDc.pixelStorageCreated,
    ],
    [
      latestEvidence.d3d9SurfaceDcNodeChromeOrderedSemanticsMatched,
      d3d9PrivateSurfaceDc.nodeChromeOrderedSemanticsMatched,
    ],
  ];
  if (
    latestSurfaceDcEvidencePairs.some(
      ([actual, expected]) =>
        JSON.stringify(actual) !== JSON.stringify(expected),
    ) ||
    latestEvidence.d3d9SurfaceDcImplemented !== true ||
    latestEvidence.d3d9SurfaceDcPolicySchema !==
      "vine.d3d9-compatibility-policy" ||
    latestEvidence.d3d9SurfaceDcPolicyVersion !== 22 ||
    latestEvidence.d3d9SurfaceDcPolicyId !==
      "d3d9-primary-virtual-adapter-v22" ||
    latestEvidence.d3d9SurfaceDcFailureAtomicity !== true ||
    latestEvidence.d3d9SurfaceDcBrowserOwnsWindowsOrGraphicsState !== false
  ) {
    errors.push("latest D3D9 Surface DC evidence is invalid");
  }
  requireSha256(
    latestEvidence.nativeD3dxSystemDllSha256,
    "activeRuntimeCheckpoint.latestEvidence.nativeD3dxSystemDllSha256",
    errors,
  );
  if (
    latestEvidence.nativeD3dxRuntimeModuleProfile !==
      "x86-d3dx9-43-load-and-export-v1" ||
    latestEvidence.nativeD3dxGameDirectoryDllPresent !== false ||
    latestEvidence.nativeD3dxSystemDllBytes !== 1_998_168 ||
    latestEvidence.nativeD3dxSystemDllSha256 !==
      "0b28546be22c71834501f7d7185ede5d79742457331c7ee09efc14490dd64f5f" ||
    latestEvidence.nativeD3dxSystemDllEntryPointRva !== "0x001bec0d" ||
    latestEvidence.nativeD3dxSystemDllImports?.join(",") !==
      "msvcrt.dll,GDI32.dll,KERNEL32.dll,ADVAPI32.dll" ||
    latestEvidence.nativeLoadLibraryARequestedName !== "d3dx9_43.dll" ||
    latestEvidence.nativeLoadLibraryACallerReturnRva !== "0x0005bab0" ||
    latestEvidence.nativeLoadLibraryAResultNonNull !== true ||
    latestEvidence.nativeLoadLibraryAEntryLastError !== 0 ||
    latestEvidence.nativeLoadLibraryAExitLastError !== 0 ||
    latestEvidence.nativeLoadLibraryANullLastError !== 87 ||
    latestEvidence.nativeLoadLibraryAEmptyLastError !== 87 ||
    latestEvidence.nativeLoadLibraryAMissingBareDllLastError !== 126 ||
    latestEvidence.nativeLoadLibraryAConsumer !==
      "store-global-test-nonzero-pass-hmodule-to-get-proc-address" ||
    latestEvidence.nativeD3dxGetProcAddressName !== "D3DXCreateEffect" ||
    latestEvidence.nativeD3dxGetProcAddressCallerReturnRva !== "0x0005bac5" ||
    latestEvidence.nativeD3dxGetProcAddressResultNonNull !== true ||
    latestEvidence.nativeD3dxGetProcAddressLastErrorPreserved !== true ||
    latestEvidence.nativeD3dxGetProcAddressConsumer !==
      "store-global-not-invoked-before-next-structured-stop" ||
    latestEvidence.nativeLoadLibraryAExecutableHashUnchanged !== true ||
    latestEvidence.loadLibraryAImplemented !== true ||
    latestEvidence.loadLibraryASequence !== 17_823 ||
    latestEvidence.loadLibraryAInstructionCount !== 11_806_531 ||
    latestEvidence.loadLibraryAArguments?.join(",") !== "0x100dc9c8" ||
    latestEvidence.loadLibraryAReturnAddress !== "0x1005bab0" ||
    latestEvidence.loadLibraryARequestedName !== "d3dx9_43.dll" ||
    latestEvidence.loadLibraryAResult !== "0x4d010010" ||
    latestEvidence.loadLibraryALastError !== 0 ||
    latestEvidence.loadLibraryAGuestWriteBytes !== 0 ||
    latestEvidence.loadLibraryAImplementationId !==
      "kernel32.load-library-a.runtime-system.v1" ||
    latestEvidence.loadLibraryAPolicyId !== "kernel32-d3dx-runtime-module-v1" ||
    latestEvidence.loadLibraryAModuleAuthority !==
      "runtime/modules.rs::RuntimeModules" ||
    latestEvidence.loadLibraryAObjectAuthority !==
      "runtime/objects.rs::RuntimeObjectTable" ||
    latestEvidence.loadLibraryAModuleHandleNamespace !==
      "vine.logical-loaded-module.v1" ||
    latestEvidence.loadLibraryAModuleHandle !== "0x4d010010" ||
    latestEvidence.loadLibraryAModuleId !== 16 ||
    latestEvidence.loadLibraryAModuleClass !== "runtime-system-module" ||
    latestEvidence.loadLibraryAModuleState !== "process-attached" ||
    latestEvidence.loadLibraryAAttachSequence !== 16 ||
    latestEvidence.loadLibraryARuntimeServiceReferenceCount !== 1 ||
    latestEvidence.loadLibraryADynamicThreadId !== 8_192 ||
    latestEvidence.loadLibraryADynamicThreadReferenceCount !== 1 ||
    latestEvidence.loadLibraryAObjectId !== 23 ||
    latestEvidence.loadLibraryAObjectOpenHandles !== 1 ||
    latestEvidence.loadLibraryARepeatedLoadReusesIdentity !== true ||
    latestEvidence.loadLibraryAFailureAtomicity !== true ||
    latestEvidence.loadLibraryABrowserOwnsWindowsModuleState !== false ||
    latestEvidence.loadLibraryANativeImageMapped !== false ||
    latestEvidence.loadLibraryADllMainInvoked !== false ||
    latestEvidence.loadLibraryAGeneralSearchImplemented !== false ||
    latestEvidence.loadLibraryAUnloadImplemented !== false ||
    latestEvidence.d3dxDynamicEndpointId !== 5 ||
    latestEvidence.d3dxDynamicEndpointAddress !== "0x7f020040" ||
    latestEvidence.d3dxDynamicEndpointNamespace !==
      "vine.dynamic-export-endpoint.v1" ||
    latestEvidence.d3dxDynamicEndpointModule !== "d3dx9_43.dll" ||
    latestEvidence.d3dxDynamicEndpointName !== "D3DXCreateEffect" ||
    latestEvidence.d3dxGetProcAddressSequence !== 17_824 ||
    latestEvidence.d3dxGetProcAddressInstructionCount !== 11_806_538 ||
    latestEvidence.d3dxGetProcAddressArguments?.join(",") !==
      "0x4d010010,0x100dc9d8" ||
    latestEvidence.d3dxGetProcAddressReturnAddress !== "0x1005bac5" ||
    latestEvidence.d3dxGetProcAddressResult !== "0x7f020040" ||
    latestEvidence.d3dxGetProcAddressLastError !== 0 ||
    latestEvidence.d3dxGetProcAddressGuestWriteBytes !== 0 ||
    latestEvidence.d3dxCreateEffectInvoked !== false ||
    latestEvidence.loadLibraryANodeCompletions !== 1 ||
    latestEvidence.loadLibraryAChromeCompletions !== 1 ||
    latestEvidence.loadLibraryANodeChromeOrderedSemanticsMatched !== true ||
    latestEvidence.reflectionSha256 !==
      "65f3b728ff9570a0f3081571c05996df74232711142bbfc5045d2c780dcecf49"
  ) {
    errors.push("latest runtime-system module load evidence is invalid");
  }
  for (const field of [
    "nativeGetClientRectPrimaryTraceSha256",
    "nativeGetClientRectConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeGetClientRectPrimaryTraceBytes !== 98_812 ||
    latestEvidence.nativeGetClientRectPrimaryTraceLines !== 259 ||
    latestEvidence.nativeGetClientRectPrimaryTraceSha256 !==
      "f1fc35c2afb505d39ed8514c8bf4a969d78bc013ee2fb15c890677ad8afe42a1" ||
    latestEvidence.nativeGetClientRectPrimaryBalancedApiPairs !== 99 ||
    latestEvidence.nativeGetClientRectConfirmationTraceBytes !== 98_817 ||
    latestEvidence.nativeGetClientRectConfirmationTraceLines !== 259 ||
    latestEvidence.nativeGetClientRectConfirmationTraceSha256 !==
      "151395f4a93715778a75d0262627cf63958a532cf3b26a478dc2c47ce408f05a" ||
    latestEvidence.nativeGetClientRectConfirmationBalancedApiPairs !== 99 ||
    latestEvidence.nativeGetClientRectTraceDiagnostics !== 0 ||
    latestEvidence.nativeGetClientRectCallsPerTrace !== 1 ||
    latestEvidence.nativeGetClientRectCallerRva !== "0x0005c997" ||
    latestEvidence.nativeGetClientRectResult !== "0x00000001" ||
    latestEvidence.nativeGetClientRectLastErrorBefore !== 0 ||
    latestEvidence.nativeGetClientRectLastErrorAfter !== 0 ||
    latestEvidence.nativeGetClientRectOutputRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.nativeGetClientRectSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeGetClientRectInvalidWindowResult !== "0x00000000" ||
    latestEvidence.nativeGetClientRectInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeGetClientRectInvalidWindowOutputPreserved !== true ||
    latestEvidence.nativeGetClientRectConsumer !==
      "bool-result-ignored-immediate-getdc-call" ||
    latestEvidence.nativeGetClientRectExecutableHashUnchanged !== true ||
    latestEvidence.getClientRectImplemented !== true ||
    latestEvidence.getClientRectSequence !== 17_825 ||
    latestEvidence.getClientRectInstructionCount !== 11_806_582 ||
    latestEvidence.getClientRectArguments?.join(",") !==
      "0x48010001,0x006ffdc4" ||
    latestEvidence.getClientRectReturnAddress !== "0x1005c997" ||
    latestEvidence.getClientRectResult !== "0x00000001" ||
    latestEvidence.getClientRectLastError !== 0 ||
    latestEvidence.getClientRectGuestWriteBytes !== 16 ||
    latestEvidence.getClientRectOutputHex !==
      "00000000000000002003000058020000" ||
    latestEvidence.getClientRectStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.getClientRectWindowId !== 1 ||
    latestEvidence.getClientRectWindowHandleNamespace !==
      "vine.logical-window.v1" ||
    latestEvidence.getClientRectGeometrySource !==
      "live-window-client-geometry" ||
    latestEvidence.getClientRectPreservesLastError !== true ||
    latestEvidence.getClientRectWindowStateMutation !== false ||
    latestEvidence.getClientRectFailureAtomicity !== true ||
    latestEvidence.getClientRectBrowserOwnsLogicalWindowState !== false ||
    latestEvidence.getClientRectHostGeometryImported !== false ||
    latestEvidence.getClientRectNodeCompletions !== 1 ||
    latestEvidence.getClientRectChromeCompletions !== 1 ||
    latestEvidence.getClientRectNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest GetClientRect evidence is invalid");
  }
  for (const field of [
    "nativeWindowDcPrimaryTraceSha256",
    "nativeWindowDcConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeWindowDcPrimaryTraceBytes !== 63_019 ||
    latestEvidence.nativeWindowDcPrimaryTraceLines !== 95 ||
    latestEvidence.nativeWindowDcPrimaryTraceSha256 !==
      "7a793d995671f44e083df46ce77da221600f3cce546ee21b03c3a4e163fbbdae" ||
    latestEvidence.nativeWindowDcPrimaryBalancedApiPairs !== 17 ||
    latestEvidence.nativeWindowDcConfirmationTraceBytes !== 63_231 ||
    latestEvidence.nativeWindowDcConfirmationTraceLines !== 96 ||
    latestEvidence.nativeWindowDcConfirmationTraceSha256 !==
      "9c554473a4fda0abc78f4bf4cdb65170db0ea6c3972d76b64f95b47478dd4d23" ||
    latestEvidence.nativeWindowDcConfirmationBalancedApiPairs !== 17 ||
    latestEvidence.nativeWindowDcTraceDiagnostics !== 0 ||
    latestEvidence.nativeWindowDcProfile !== "x86-bgi-window-dc-fill-v1" ||
    latestEvidence.nativeWindowDcCallerReturnRvas?.join(",") !==
      "0x0005c9a4,0x0005c9ae,0x0005c9bb,0x0005c9cb" ||
    latestEvidence.nativeWindowDcClientRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.nativeWindowDcStockObjectIndex !== 4 ||
    latestEvidence.nativeWindowDcGetDcResultNonNull !== true ||
    latestEvidence.nativeWindowDcFillRectResult !== 1 ||
    latestEvidence.nativeWindowDcReleaseDcResult !== 1 ||
    latestEvidence.nativeWindowDcSuccessLastErrorPreserved !== true ||
    latestEvidence.nativeWindowDcConcurrentAcquisitionsDistinct !== true ||
    latestEvidence.nativeWindowDcNumericHandleReuseImported !== false ||
    latestEvidence.nativeWindowDcInvalidWindowGetDcLastError !== 1_400 ||
    latestEvidence.nativeWindowDcInvalidHdcFillRectLastError !== 6 ||
    latestEvidence.nativeWindowDcInvalidHdcReleaseDcLastError !== 1_425 ||
    latestEvidence.nativeWindowDcInvalidStockObjectResult !== "0x00000000" ||
    latestEvidence.nativeWindowDcInvalidStockObjectLastErrorPreserved !==
      true ||
    latestEvidence.nativeWindowDcExecutableHashUnchanged !== true ||
    latestEvidence.getDcImplemented !== true ||
    latestEvidence.getDcSequence !== 17_826 ||
    latestEvidence.getDcInstructionCount !== 11_806_586 ||
    latestEvidence.getDcArguments?.join(",") !== "0x48010001" ||
    latestEvidence.getDcReturnAddress !== "0x1005c9a4" ||
    latestEvidence.getDcResult !== "0x47010001" ||
    latestEvidence.getDcLastError !== 0 ||
    latestEvidence.getDcGuestWriteBytes !== 0 ||
    latestEvidence.getDcDeviceContextId !== 1 ||
    latestEvidence.getDcHandleNamespace !== "vine.logical-gdi-dc.v1" ||
    latestEvidence.getDcSurfaceId !== 1 ||
    latestEvidence.getDcSurfaceCreated !== true ||
    latestEvidence.getStockObjectImplemented !== true ||
    latestEvidence.getStockObjectSequence !== 17_827 ||
    latestEvidence.getStockObjectInstructionCount !== 11_806_590 ||
    latestEvidence.getStockObjectArguments?.join(",") !== "0x00000004" ||
    latestEvidence.getStockObjectReturnAddress !== "0x1005c9ae" ||
    latestEvidence.getStockObjectResult !== "0x47020001" ||
    latestEvidence.getStockObjectLastError !== 0 ||
    latestEvidence.getStockObjectGuestWriteBytes !== 0 ||
    latestEvidence.getStockObjectId !== 1 ||
    latestEvidence.getStockObjectHandleNamespace !==
      "vine.logical-gdi-object.v1" ||
    latestEvidence.getStockObjectShared !== true ||
    latestEvidence.getStockObjectColorRef !== 0 ||
    latestEvidence.fillRectImplemented !== true ||
    latestEvidence.fillRectSequence !== 17_828 ||
    latestEvidence.fillRectInstructionCount !== 11_806_596 ||
    latestEvidence.fillRectArguments?.join(",") !==
      "0x47010001,0x006ffdc4,0x47020001" ||
    latestEvidence.fillRectReturnAddress !== "0x1005c9bb" ||
    latestEvidence.fillRectResult !== "0x00000001" ||
    latestEvidence.fillRectLastError !== 0 ||
    latestEvidence.fillRectGuestWriteBytes !== 0 ||
    latestEvidence.fillRectSurfaceId !== 1 ||
    latestEvidence.fillRectSurfaceGeometry?.join(",") !== "800,600" ||
    latestEvidence.fillRectSurfaceGeneration !== 1 ||
    latestEvidence.fillRectRect?.join(",") !== "0,0,800,600" ||
    latestEvidence.fillRectColorRef !== 0 ||
    latestEvidence.releaseDcImplemented !== true ||
    latestEvidence.releaseDcSequence !== 17_829 ||
    latestEvidence.releaseDcInstructionCount !== 11_806_602 ||
    latestEvidence.releaseDcArguments?.join(",") !== "0x48010001,0x47010001" ||
    latestEvidence.releaseDcReturnAddress !== "0x1005c9cb" ||
    latestEvidence.releaseDcResult !== "0x00000001" ||
    latestEvidence.releaseDcLastError !== 0 ||
    latestEvidence.releaseDcGuestWriteBytes !== 0 ||
    latestEvidence.releaseDcLiveDeviceContexts !== 0 ||
    latestEvidence.releaseDcLiveSurfaces !== 1 ||
    latestEvidence.gdiStateAuthority !== "runtime/gdi.rs::RuntimeGdi" ||
    latestEvidence.gdiWindowRelationOwnership !==
      "non-owning-revalidated-through-runtime-user32" ||
    latestEvidence.gdiMutation !== "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.gdiProcessExitTeardown !== true ||
    latestEvidence.gdiFailureAtomicity !== true ||
    latestEvidence.gdiBrowserOwnsWindowsState !== false ||
    latestEvidence.gdiRuntimeGraphicsCreated !== false ||
    latestEvidence.gdiBrowserRendererResourceCreated !== false ||
    latestEvidence.windowDcNodeCompletions !== 4 ||
    latestEvidence.windowDcChromeCompletions !== 4 ||
    latestEvidence.windowDcNodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest window DC fill evidence is invalid");
  }
  for (const field of [
    "nativeSetWindowLongAPrimaryTraceSha256",
    "nativeSetWindowLongAConfirmationTraceSha256",
  ]) {
    requireSha256(
      latestEvidence[field],
      `activeRuntimeCheckpoint.latestEvidence.${field}`,
      errors,
    );
  }
  if (
    latestEvidence.nativeSetWindowLongAPrimaryTraceBytes !== 67_283 ||
    latestEvidence.nativeSetWindowLongAPrimaryTraceLines !== 88 ||
    latestEvidence.nativeSetWindowLongAPrimaryTraceSha256 !==
      "c7ec66fdcce747246943c4cc98b1edfebb9f5b8748ccfd023916a8d4ed1c6283" ||
    latestEvidence.nativeSetWindowLongAPrimaryBalancedApiPairs !== 6 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceBytes !== 67_579 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceLines !== 89 ||
    latestEvidence.nativeSetWindowLongAConfirmationTraceSha256 !==
      "1b7d6816599493a0db1b5b7d4ff716a5fdc6192ed96f3c2fe51856f81d0c18a6" ||
    latestEvidence.nativeSetWindowLongAConfirmationBalancedApiPairs !== 6 ||
    latestEvidence.nativeSetWindowLongATraceDiagnostics !== 0 ||
    latestEvidence.nativeSetWindowLongAProfile !==
      "x86-bgi-first-gwl-style-update-v1" ||
    latestEvidence.nativeSetWindowLongAOracleProfile !==
      "x86-live-gwl-style-and-invalid-arguments-v1" ||
    latestEvidence.nativeSetWindowLongAMainCallsPerTrace !== 3 ||
    latestEvidence.nativeSetWindowLongACallerRva !== "0x0005cae3" ||
    latestEvidence.nativeSetWindowLongAStyleIndex !== -16 ||
    latestEvidence.nativeSetWindowLongARequestedStyle !== "0x90ca0000" ||
    latestEvidence.nativeSetWindowLongAInitialEffectiveStyle !== "0x84ca0000" ||
    latestEvidence.nativeSetWindowLongAFirstResult !== "0x84ca0000" ||
    latestEvidence.nativeSetWindowLongAFinalEffectiveStyle !== "0x94ca0000" ||
    latestEvidence.nativeSetWindowLongAVisibilityTransition?.join(",") !==
      "false,true" ||
    latestEvidence.nativeSetWindowLongASuccessLastErrorPreserved !== true ||
    latestEvidence.nativeSetWindowLongARepeatResult !== "0x94ca0000" ||
    latestEvidence.nativeSetWindowLongARepeatStateChanged !== false ||
    latestEvidence.nativeSetWindowLongAInvalidWindowResult !== "0x00000000" ||
    latestEvidence.nativeSetWindowLongAInvalidWindowLastError !== 1_400 ||
    latestEvidence.nativeSetWindowLongAInvalidIndex !== -100 ||
    latestEvidence.nativeSetWindowLongAInvalidIndexResult !== "0x00000000" ||
    latestEvidence.nativeSetWindowLongAInvalidIndexLastError !== 1_413 ||
    latestEvidence.nativeSetWindowLongAConsumer !==
      "return-eax-unread-before-set-window-pos-argument-setup" ||
    latestEvidence.nativeSetWindowLongAExecutableHashUnchanged !== true ||
    latestEvidence.setWindowLongAImplemented !== true ||
    latestEvidence.setWindowLongASequence !== 17_830 ||
    latestEvidence.setWindowLongAInstructionCount !== 11_806_624 ||
    latestEvidence.setWindowLongAArguments?.join(",") !==
      "0x48010001,0xfffffff0,0x90ca0000" ||
    latestEvidence.setWindowLongAReturnAddress !== "0x1005cae3" ||
    latestEvidence.setWindowLongAResult !== "0x84ca0000" ||
    latestEvidence.setWindowLongALastError !== 0 ||
    latestEvidence.setWindowLongAGuestWriteBytes !== 0 ||
    latestEvidence.setWindowLongAImplementationId !==
      "user32.set-window-long-a.gwl-style.v1" ||
    latestEvidence.setWindowLongAPolicyId !==
      "main-window-gwl-style-visible-v1" ||
    latestEvidence.setWindowLongAStateAuthority !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.setWindowLongAWindowId !== 1 ||
    latestEvidence.setWindowLongAWindowHandleNamespace !==
      "vine.logical-window.v1" ||
    latestEvidence.setWindowLongARequestedCreationStyle !== "0x80ca0000" ||
    latestEvidence.setWindowLongAPreviousEffectiveStyle !== "0x84ca0000" ||
    latestEvidence.setWindowLongARetainedStyleMask !== "0x04000000" ||
    latestEvidence.setWindowLongAEffectiveStyle !== "0x94ca0000" ||
    latestEvidence.setWindowLongAVisibleBefore !== false ||
    latestEvidence.setWindowLongAVisibleAfter !== true ||
    latestEvidence.setWindowLongATopmostAfter !== false ||
    latestEvidence.setWindowLongAMutation !==
      "owner-plan-stale-check-targeted-commit" ||
    latestEvidence.setWindowLongAIdempotentUpdatePreservesGeneration !== true ||
    latestEvidence.setWindowLongAFailureAtomicity !== true ||
    latestEvidence.setWindowLongABrowserOwnsLogicalWindowState !== false ||
    latestEvidence.setWindowLongAHostStyleImported !== false ||
    latestEvidence.setWindowLongANodeCompletions !== 1 ||
    latestEvidence.setWindowLongAChromeCompletions !== 1 ||
    latestEvidence.setWindowLongANodeChromeOrderedSemanticsMatched !== true
  ) {
    errors.push("latest SetWindowLongA GWL_STYLE evidence is invalid");
  }
  const createWindowExAGeometry = requireRecord(
    latestEvidence.createWindowExAGeometry,
    "activeRuntimeCheckpoint.latestEvidence.createWindowExAGeometry",
    errors,
  );
  if (
    latestEvidence.nativeCreateWindowExAPrimaryTraceBytes !== 316_971 ||
    latestEvidence.nativeCreateWindowExAPrimaryTraceLines !== 793 ||
    latestEvidence.nativeCreateWindowExAConfirmationTraceBytes !== 316_932 ||
    latestEvidence.nativeCreateWindowExAConfirmationTraceLines !== 791 ||
    latestEvidence.nativeCreateWindowExAPayloadTraceBytes !== 319_849 ||
    latestEvidence.nativeCreateWindowExAPayloadTraceLines !== 794 ||
    latestEvidence.nativeInvalidMenuTraceBytes !== 330_210 ||
    latestEvidence.nativeInvalidMenuTraceLines !== 794 ||
    latestEvidence.nativeDefWindowProcTraceBytes !== 342_059 ||
    latestEvidence.nativeDefWindowProcTraceLines !== 833 ||
    latestEvidence.nativeCreateWindowExAProfile !==
      "x86-bgi-create-window-ex-a-v1" ||
    latestEvidence.nativeCreateWindowExACallerReturnRva !== "0x00067efa" ||
    latestEvidence.nativeCreateWindowExAArguments?.join(",") !==
      "0x00040000,BGI - Main window,Ethornell title,0x80ca0000,317,154,806,629,0x00000000,0x00000000,main-module,0x00000000" ||
    latestEvidence.nativeCreateWindowExAResultNonNull !== true ||
    latestEvidence.nativeCreateWindowExAConsumer !==
      "store-global-test-nonzero-and-continue-initialization" ||
    latestEvidence.nativeCreateWindowExACallbackMessages?.join(",") !==
      "129,131,1,5,3" ||
    latestEvidence.nativeCreateWindowExACallbackResults?.join(",") !==
      "1,0,0,0,0" ||
    latestEvidence.nativeCreateWindowExANonClientRectBefore?.join(",") !==
      "317,154,1123,783" ||
    latestEvidence.nativeCreateWindowExANonClientRectAfter?.join(",") !==
      "320,180,1120,780" ||
    latestEvidence.nativeCreateWindowExASizeLparam !== "0x02580320" ||
    latestEvidence.nativeCreateWindowExAMoveLparam !== "0x00b40140" ||
    latestEvidence.nativeDefWindowProcCallerRva !== "0x000933bf" ||
    latestEvidence.nativeDefWindowProcCreationMessages?.join(",") !==
      "129,131,5,3" ||
    latestEvidence.nativeDefWindowProcCreationResults?.join(",") !==
      "1,0,0,0" ||
    latestEvidence.nativeGetSystemMenuArguments?.join(",") !==
      "0x00000000,0x00000000" ||
    latestEvidence.nativeGetSystemMenuResult !== "0x00000000" ||
    latestEvidence.nativeGetSystemMenuLastError !== 1_400 ||
    latestEvidence.nativeEnableMenuItemArguments?.join(",") !==
      "0x00000000,0x0000f060,0x00000000" ||
    latestEvidence.nativeEnableMenuItemResult !== "0xffffffff" ||
    latestEvidence.nativeEnableMenuItemLastError !== 1_401 ||
    latestEvidence.nativeInvalidMenuConsumer !==
      "result-ignored-global-flag-set-and-continue" ||
    latestEvidence.nativeCreateWindowExAExecutableHashUnchanged !== true ||
    latestEvidence.implementation !==
      (setWindowPosFrameChangeMoveActive
        ? "Rust-owned SetWindowPos frame-change move callback transaction over RuntimeUser32 geometry authority"
        : d3d9RecreatedTexturesActive
          ? "Rust-owned D3D9 recreated-texture band over transactional RuntimeGraphics and RuntimeCom authority"
          : setWindowPosRepeatActivationActive
            ? "Rust-owned same-active-window repeated SetWindowPos validation-only callback over RuntimeUser32 authority"
            : createDirectoryAActive
              ? "Rust-owned durable CreateDirectoryA over RuntimeFiles and SQLite writable-profile authority"
              : getFileAttributesAActive
                ? "Rust-owned GetFileAttributesA over RuntimeFiles registered VFS and derived directory authority"
                : globalMemoryStatusActive
                  ? "Rust-owned deterministic x86 MEMORYSTATUS over Kernel32CompatibilityPolicy and system-info virtual-span authority"
                  : sleepZeroActive
                    ? "Rust-owned Sleep(0) cooperative yield over RuntimeScheduler authority"
                    : archiveSourceReadActive
                      ? "Rust-owned window-scoped logical system-menu identity and bounded SC_CLOSE state over RuntimeUser32 authority"
                      : dragAcceptFilesActive
                        ? "Rust-owned DragAcceptFiles over logical-window extended-style authority with failure-atomic plan/commit"
                        : "Rust-owned bounded WaitForSingleObjectEx over logical waitable-timer clock, scheduler, wait, and typed-object authority") ||
    latestEvidence.createWindowExAImplemented !== true ||
    latestEvidence.createWindowExASequence !== 1_388 ||
    latestEvidence.createWindowExAInstructionCount !== 11_476_236 ||
    latestEvidence.createWindowExAArguments?.join(",") !==
      "0x00040000,0x100fba4c,0x006ffb48,0x80ca0000,317,154,806,629,0x00000000,0x00000000,0x10000000,0x00000000" ||
    latestEvidence.createWindowExAReturnAddress !== "0x10067efa" ||
    latestEvidence.createWindowExAResult !== "0x48010001" ||
    latestEvidence.createWindowExALastError !== 0 ||
    latestEvidence.createWindowExAGuestWrites !== 0 ||
    latestEvidence.createWindowExAStateOwner !==
      "runtime/user32.rs::RuntimeUser32" ||
    latestEvidence.createWindowExAWindowId !== 1 ||
    latestEvidence.createWindowExAHandleNamespace !==
      "vine.logical-window.v1" ||
    latestEvidence.createWindowExAOwnerThreadId !== 8_192 ||
    latestEvidence.createWindowExAClassId !== 1 ||
    latestEvidence.createWindowExAClassAtom !== "0x0000c000" ||
    latestEvidence.createWindowExANameEncoding !== "windows-932-v1" ||
    latestEvidence.createWindowExAEffectiveExtendedStyle !== "0x00040100" ||
    latestEvidence.createWindowExARequestedStyle !== "0x80ca0000" ||
    latestEvidence.createWindowExAEffectiveStyle !== "0x84ca0000" ||
    createWindowExAGeometry.x !== 317 ||
    createWindowExAGeometry.y !== 154 ||
    createWindowExAGeometry.width !== 806 ||
    createWindowExAGeometry.height !== 629 ||
    createWindowExAGeometry.clientX !== 320 ||
    createWindowExAGeometry.clientY !== 180 ||
    createWindowExAGeometry.clientWidth !== 800 ||
    createWindowExAGeometry.clientHeight !== 600 ||
    latestEvidence.createWindowExAFinalLifecycle !== "created" ||
    latestEvidence.createWindowExAVisible !== false ||
    latestEvidence.createWindowExACallbackFrameIds?.join(",") !== "2,3,4,5,6" ||
    latestEvidence.createWindowExACallbackMessages?.join(",") !==
      "129,131,1,5,3" ||
    latestEvidence.createWindowExACallbackResults?.join(",") !== "1,0,0,0,0" ||
    latestEvidence.createWindowExACallbackReturnInstructionCounts?.join(",") !==
      "11476289,11476342,11476388,11476458,11476511" ||
    latestEvidence.createWindowExACallbackDepthAfterReturn !== 0 ||
    latestEvidence.callbackScratchArenaBase !== "0x00501000" ||
    latestEvidence.callbackScratchArenaBytes !== 65_536 ||
    latestEvidence.callbackScratchSlotBytes !== 4_096 ||
    latestEvidence.callbackScratchAddressAllocationPurpose !==
      "callback-scratch" ||
    latestEvidence.callbackScratchSeparatedFromDescendingStack !== true ||
    latestEvidence.defWindowProcAImplemented !== true ||
    latestEvidence.defWindowProcACallCount !== 20 ||
    latestEvidence.defWindowProcASequences?.join(",") !==
      "1389,1390,1393,1394,1399,1403,1404,1409,17832,17833,17834,17835,17836,17837,17838,17839,89388,89389,89392,89393" ||
    latestEvidence.defWindowProcAMessages?.join(",") !==
      "129,131,5,3,128,71,3,71,70,70,28,134,6,641,642,7,70,131,5,71" ||
    latestEvidence.defWindowProcAResults?.join(",") !==
      "1,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0" ||
    latestEvidence.defWindowProcALastErrors?.join(",") !==
      "0,0,1401,1401,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0" ||
    latestEvidence.defWindowProcANonClientGuestWriteBytes !== 16 ||
    latestEvidence.defWindowProcAScalarLparamsDereferenced !== false ||
    latestEvidence.getSystemMenuImplemented !== true ||
    latestEvidence.getSystemMenuInitialNullSequence !== 1_391 ||
    latestEvidence.getSystemMenuInitialNullResult !== "0x00000000" ||
    latestEvidence.getSystemMenuInitialNullLastError !== 1_400 ||
    latestEvidence.enableMenuItemImplemented !== true ||
    latestEvidence.enableMenuItemInitialNullSequence !== 1_392 ||
    latestEvidence.enableMenuItemInitialNullResult !== "0xffffffff" ||
    latestEvidence.enableMenuItemInitialNullLastError !== 1_401 ||
    latestEvidence.initialMenuIdentityAllocated !== false ||
    latestEvidence.windowCreationFailureAtomicity !== true ||
    latestEvidence.windowCallbackReplacementFailureAtomicity !== true ||
    latestEvidence.browserOwnsLogicalWindowState !== false
  ) {
    errors.push("latest logical window creation evidence is invalid");
  }
  if (dragAcceptFilesActive) {
    for (const field of [
      "nativeDragAcceptFilesPrimaryTraceSha256",
      "nativeDragAcceptFilesPrimarySummarySha256",
      "nativeDragAcceptFilesConfirmationTraceSha256",
      "nativeDragAcceptFilesConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeDragAcceptFilesPrimaryTraceBytes !== 23_308 ||
      latestEvidence.nativeDragAcceptFilesPrimaryTraceLines !== 77 ||
      latestEvidence.nativeDragAcceptFilesPrimaryTraceSha256 !==
        "939c2094e929ba80e867ec73586e41b0d411d6f657def08d11c44212795ae412" ||
      latestEvidence.nativeDragAcceptFilesPrimarySummaryBytes !== 14_754 ||
      latestEvidence.nativeDragAcceptFilesPrimarySummarySha256 !==
        "97b082b95f58eff078da27360a054dfc8f3dde0671877de0c752bbb50a7e7754" ||
      latestEvidence.nativeDragAcceptFilesPrimaryBalancedApiPairs !== 1 ||
      latestEvidence.nativeDragAcceptFilesConfirmationTraceBytes !== 23_236 ||
      latestEvidence.nativeDragAcceptFilesConfirmationTraceLines !== 77 ||
      latestEvidence.nativeDragAcceptFilesConfirmationTraceSha256 !==
        "05924e0335698b3081a79b36d62f0a9a9f3c519e673d61a63229b2fad52b1834" ||
      latestEvidence.nativeDragAcceptFilesConfirmationSummaryBytes !== 14_754 ||
      latestEvidence.nativeDragAcceptFilesConfirmationSummarySha256 !==
        "1ec8cd27a53165fd871d075610f7608e693e297c6aeadc37d1a924f66a225ead" ||
      latestEvidence.nativeDragAcceptFilesConfirmationBalancedApiPairs !== 1 ||
      latestEvidence.nativeDragAcceptFilesTraceDiagnostics !== 0 ||
      latestEvidence.nativeDragAcceptFilesProfile !==
        "x86-bgi-drag-accept-files-v1" ||
      latestEvidence.nativeDragAcceptFilesOracleProfile !==
        "x86-live-window-drag-accept-files-v1" ||
      latestEvidence.nativeDragAcceptFilesCallerRva !== "0x0005f272" ||
      latestEvidence.nativeDragAcceptFilesAccept !== false ||
      latestEvidence.nativeDragAcceptFilesLastErrorBefore !== 0 ||
      latestEvidence.nativeDragAcceptFilesLastErrorAfter !== 0 ||
      latestEvidence.nativeDragAcceptFilesVoidResultConsumed !== false ||
      latestEvidence.nativeDragAcceptFilesOracleInitialAcceptsFiles !== false ||
      latestEvidence.nativeDragAcceptFilesOracleAcceptValues?.join(",") !==
        "true,false" ||
      latestEvidence.nativeDragAcceptFilesOracleSentinelLastErrors?.join(
        ",",
      ) !== "1432778632,2053856333" ||
      latestEvidence.nativeDragAcceptFilesOracleObservedLastErrors?.join(
        ",",
      ) !== "0,0" ||
      latestEvidence.nativeDragAcceptFilesOracleObservedStates?.join(",") !==
        "true,false" ||
      latestEvidence.nativeDragAcceptFilesOracleFinalAcceptsFiles !== false ||
      latestEvidence.nativeDragAcceptFilesOracleClearsSentinelLastError !==
        true ||
      latestEvidence.nativeDragAcceptFilesExecutableHashUnchanged !== true ||
      latestEvidence.dragAcceptFilesImplemented !== true ||
      latestEvidence.dragAcceptFilesSequence !== 89_249 ||
      latestEvidence.dragAcceptFilesInstructionCount !== 15_601_759 ||
      latestEvidence.dragAcceptFilesArguments?.join(",") !==
        "0x48010001,0x00000000" ||
      latestEvidence.dragAcceptFilesReturnAddress !== "0x1005f272" ||
      latestEvidence.dragAcceptFilesResult !== "void-eax-preserved" ||
      latestEvidence.dragAcceptFilesLastError !== 0 ||
      latestEvidence.dragAcceptFilesGuestWriteBytes !== 0 ||
      latestEvidence.dragAcceptFilesImplementationId !==
        "shell32.drag-accept-files.logical-window-style-v1" ||
      latestEvidence.dragAcceptFilesStateAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.dragAcceptFilesWindowId !== 1 ||
      latestEvidence.dragAcceptFilesWindowHandleNamespace !==
        "vine.logical-window.v1" ||
      latestEvidence.dragAcceptFilesWindowLifecycle !== "created" ||
      latestEvidence.dragAcceptFilesEffectiveExtendedStyle !== "0x00040100" ||
      latestEvidence.dragAcceptFilesAcceptsFiles !== false ||
      latestEvidence.dragAcceptFilesMutation !==
        "owner-plan-stale-check-targeted-commit" ||
      latestEvidence.dragAcceptFilesIdempotentUpdatePreservesGeneration !==
        true ||
      latestEvidence.dragAcceptFilesFailureAtomicity !== true ||
      latestEvidence.dragAcceptFilesBrowserOwnsLogicalWindowState !== false ||
      latestEvidence.dragAcceptFilesBrowserResourceCreated !== false ||
      latestEvidence.dragAcceptFilesWmDropFilesImplemented !== false ||
      latestEvidence.dragAcceptFilesNodeCompletions !== 1 ||
      latestEvidence.dragAcceptFilesChromeCompletions !== 1 ||
      latestEvidence.dragAcceptFilesNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push("latest DragAcceptFiles evidence is invalid");
    }
  }
  if (
    d3d9RecreatedTexturesActive &&
    (latestEvidence.d3d9RecreatedTexturesImplemented !== true ||
      latestEvidence.d3d9RecreatedTexturesEvidenceProfileId !==
        d3d9NativeRecreatedTextures.profileId ||
      JSON.stringify(latestEvidence.d3d9RecreatedTexturesNodeSequences) !==
        JSON.stringify(d3d9PrivateRecreatedTextures.nodeSequences) ||
      JSON.stringify(latestEvidence.d3d9RecreatedTexturesChromeSequences) !==
        JSON.stringify(d3d9PrivateRecreatedTextures.chromeSequences) ||
      latestEvidence.d3d9RecreatedTexturesCompatibilityPolicyId !==
        d3d9CompatibilityPolicy.id ||
      latestEvidence.d3d9RecreatedTexturesCreateTextureProfileId !==
        d3d9CreateTexture.profileId ||
      latestEvidence.d3d9RecreatedTexturesGetSurfaceLevelProfileId !==
        d3d9GetSurfaceLevel.profileId ||
      latestEvidence.d3d9RecreatedTexturesFailureAtomicity !== true ||
      latestEvidence.d3d9RecreatedTexturesNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push("latest D3D9 recreated-texture checkpoint evidence is invalid");
  }
  if (
    setWindowPosFrameChangeMoveActive &&
    (latestEvidence.nativeSetWindowPosFrameChangeMovePrimaryTraceBytes !==
      390_979 ||
      latestEvidence.nativeSetWindowPosFrameChangeMovePrimaryTraceLines !==
        1_089 ||
      latestEvidence.nativeSetWindowPosFrameChangeMovePrimaryTraceSha256 !==
        "fb45116bc693efca694f434038389f386946ce5c1f238276717a258d085cae47" ||
      latestEvidence.nativeSetWindowPosFrameChangeMovePrimarySummaryBytes !==
        51_851 ||
      latestEvidence.nativeSetWindowPosFrameChangeMovePrimarySummarySha256 !==
        "872c2b3268c90a1ba4a0f2483befbff00c3d70d27939de875349492b911058ee" ||
      latestEvidence.nativeSetWindowPosFrameChangeMovePrimaryBalancedApiPairs !==
        506 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationTraceBytes !==
        392_371 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationTraceLines !==
        1_093 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationTraceSha256 !==
        "55041bd50d378fbf1af7ffba4cf324e34077a7fe57be677718a0b640db2d690f" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationSummaryBytes !==
        52_080 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationSummarySha256 !==
        "c0c94f363a7b4aae52602d4337455d1c4eae0a82660bfcdda030df5a70b6251b" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConfirmationBalancedApiPairs !==
        508 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveTraceDiagnostics !== 0 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveProfile !==
        "x86-bgi-set-window-pos-frame-change-move-v1" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveOracleProfile !==
        "x86-set-window-pos-frame-change-move-invalid-window-v1" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveCallsPerTrace !== 1 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveCallerRva !==
        "0x000938c2" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveArguments?.join(",") !==
        "window,hwnd-top,317,154,0,0,0x00000025" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveResult !== 1 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveEntryLastError !== 0 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveExitLastError !== 0 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveInvalidWindowResult !==
        0 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveInvalidWindowLastError !==
        1_400 ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveBeforeGeometry?.join(
        ",",
      ) !== "15360,8640,1286,749,15363,8666,1280,720" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveAfterGeometry?.join(
        ",",
      ) !== "317,154,1286,749,320,180,1280,720" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveStyle !== "0x94ca0000" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveExtendedStyle !==
        "0x00040100" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveCallbackMessages?.join(
        ",",
      ) !== "70,131,133,20,71,3" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveCallbackResults?.join(
        ",",
      ) !== "0,0,0,0,0,0" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveNccalcInputRects?.join(
        ",",
      ) !== "317,154,1603,903,15360,8640,16646,9389,15363,8666,16643,9386" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveNccalcOutputFirstRect?.join(
        ",",
      ) !== "320,180,1600,900" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveNccalcWindowPosFlags !==
        "0x00001825" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveChangedFlags !==
        "0x00000825" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveNestedMoveLParam !==
        "0x00b40140" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveRelationsPreserved !==
        true ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveClientExtentPreserved !==
        true ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveConsumer !==
        "bool-overwritten-by-immediate-helper-call" ||
      latestEvidence.nativeSetWindowPosFrameChangeMoveExecutableHashUnchanged !==
        true ||
      latestEvidence.setWindowPosFrameChangeMoveImplemented !== true ||
      latestEvidence.setWindowPosFrameChangeMoveSequence !== 127_346 ||
      latestEvidence.setWindowPosFrameChangeMoveOccurrence !== 6 ||
      latestEvidence.setWindowPosFrameChangeMoveInstructionCount !==
        28_760_801 ||
      latestEvidence.setWindowPosFrameChangeMoveArguments?.join(",") !==
        "0x48010001,0x00000000,0x0000013d,0x0000009a,0x00000000,0x00000000,0x00000025" ||
      latestEvidence.setWindowPosFrameChangeMoveReturnAddress !==
        "0x100938c2" ||
      latestEvidence.setWindowPosFrameChangeMoveResult !== 1 ||
      latestEvidence.setWindowPosFrameChangeMoveLastError !== 0 ||
      latestEvidence.setWindowPosFrameChangeMoveLastErrorPolicy !==
        "preserve" ||
      latestEvidence.setWindowPosFrameChangeMoveGuestWriteBytes !== 0 ||
      latestEvidence.setWindowPosFrameChangeMoveImplementationId !==
        "user32.set-window-pos.v5" ||
      latestEvidence.setWindowPosFrameChangeMoveCallbackFrameIds?.join(",") !==
        "26,27,28" ||
      latestEvidence.setWindowPosFrameChangeMoveCallbackMessages?.join(",") !==
        "70,131,71" ||
      latestEvidence.setWindowPosFrameChangeMoveCallbackResults?.join(",") !==
        "0,0,0" ||
      latestEvidence.setWindowPosFrameChangeMoveCallbackInstructionCounts?.join(
        ",",
      ) !== "28760801,28760858,28760858,28760911,28760911,28761017" ||
      latestEvidence.setWindowPosFrameChangeMoveNestedMoveSequence !==
        127_349 ||
      latestEvidence.setWindowPosFrameChangeMoveNestedMoveFrameId !== 29 ||
      latestEvidence.setWindowPosFrameChangeMoveNestedMoveInstructionCounts?.join(
        ",",
      ) !== "28760951,28761004" ||
      latestEvidence.setWindowPosFrameChangeMoveNestedMoveLParam !==
        "0x00b40140" ||
      latestEvidence.setWindowPosFrameChangeMoveCommittedGeometry?.join(",") !==
        "317,154,1286,749,320,180,1280,720" ||
      latestEvidence.setWindowPosFrameChangeMoveWindowAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.setWindowPosFrameChangeMoveRelationAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.setWindowPosFrameChangeMoveGeometryCommit !==
        "after-wm-nccalcsize-before-wm-windowposchanged" ||
      latestEvidence.setWindowPosFrameChangeMoveRelationMutation !== false ||
      latestEvidence.setWindowPosFrameChangeMoveGdiResourceMutation !== false ||
      latestEvidence.setWindowPosFrameChangeMoveNonClientPaintAndEraseModeled !==
        false ||
      latestEvidence.setWindowPosFrameChangeMoveInvalidWindowFailureAtomicity !==
        true ||
      latestEvidence.setWindowPosFrameChangeMoveBrowserOwnsLogicalWindowState !==
        false ||
      latestEvidence.setWindowPosFrameChangeMoveBrowserResourceCreated !==
        false ||
      latestEvidence.setWindowPosFrameChangeMoveNodeCompletions !== 1 ||
      latestEvidence.setWindowPosFrameChangeMoveChromeCompletions !== 1 ||
      latestEvidence.setWindowPosFrameChangeMoveNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push(
      "latest native and private frame-change SetWindowPos evidence is invalid",
    );
  }
  if (
    showWindowNormalActive &&
    (latestEvidence.nativeShowWindowNormalTraceBytes !== 358_552 ||
      latestEvidence.nativeShowWindowNormalTraceLines !== 1_071 ||
      latestEvidence.nativeShowWindowNormalTraceSha256 !==
        "cfa4de77d32dc9a593d790f3e7f1d364bbcd474c9835d68a6fdd18f20b68edd8" ||
      latestEvidence.nativeShowWindowNormalSummaryBytes !== 51_083 ||
      latestEvidence.nativeShowWindowNormalSummarySha256 !==
        "b956032467e1c0911f503b9ebaf452c0f7711d17486d655253f561280e6ec9e7" ||
      latestEvidence.nativeShowWindowNormalBalancedApiPairs !== 497 ||
      latestEvidence.nativeShowWindowNormalTraceDiagnostics !== 0 ||
      latestEvidence.nativeShowWindowNormalProfile !==
        "x86-bgi-show-window-normal-v1" ||
      latestEvidence.nativeShowWindowNormalOracleProfile !==
        "x86-show-window-normal-invalid-window-v1" ||
      latestEvidence.nativeShowWindowNormalCallerRva !== "0x000938da" ||
      latestEvidence.nativeShowWindowNormalCommand !== 1 ||
      latestEvidence.nativeShowWindowNormalResult !== 24 ||
      latestEvidence.nativeShowWindowNormalResultNormalized !== 1 ||
      latestEvidence.nativeShowWindowNormalLastError !== 0 ||
      latestEvidence.nativeShowWindowNormalStateUnchanged !== true ||
      latestEvidence.nativeShowWindowNormalCallbackCount !== 0 ||
      latestEvidence.nativeShowWindowNormalInvalidWindowResult !== 0 ||
      latestEvidence.nativeShowWindowNormalInvalidWindowLastError !== 1_400 ||
      latestEvidence.nativeShowWindowNormalExecutableHashUnchanged !== true ||
      latestEvidence.showWindowNormalImplemented !== true ||
      latestEvidence.showWindowNormalSequence !== 127_351 ||
      latestEvidence.showWindowNormalInstructionCount !== 28_761_027 ||
      latestEvidence.showWindowNormalArguments?.join(",") !==
        "0x48010001,0x00000001" ||
      latestEvidence.showWindowNormalResult !== 1 ||
      latestEvidence.showWindowNormalImplementationId !==
        "user32.show-window.visible-normal-noop.v1" ||
      latestEvidence.showWindowNormalReturnAddress !== "0x100938da" ||
      latestEvidence.showWindowNormalStackPointers?.join(",") !==
        "0x006ff684,0x006ff690" ||
      latestEvidence.showWindowNormalLastError !== 0 ||
      latestEvidence.showWindowNormalGuestWriteBytes !== 0 ||
      latestEvidence.showWindowNormalWindowAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.showWindowNormalRelationAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.showWindowNormalMutation !== false ||
      latestEvidence.showWindowNormalCallbackCount !== 0 ||
      latestEvidence.showWindowNormalFailureAtomicity !== true ||
      latestEvidence.showWindowNormalBrowserOwnsLogicalWindowState !== false ||
      latestEvidence.showWindowNormalBrowserResourceCreated !== false ||
      latestEvidence.showWindowNormalNodeCompletions !== 1 ||
      latestEvidence.showWindowNormalChromeCompletions !== 1 ||
      latestEvidence.showWindowNormalNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("latest native and private ShowWindow normal evidence is invalid");
  }
  if (
    updateWindowActive &&
    (latestEvidence.nativeUpdateWindowPaintTraceBytes !== 498_871 ||
      latestEvidence.nativeUpdateWindowPaintTraceLines !== 1_463 ||
      latestEvidence.nativeUpdateWindowPaintTraceSha256 !==
        "c670f314315d2fed87e4fff5be2a2fca719e5c969e0155f6169594e3b60ffd27" ||
      latestEvidence.nativeUpdateWindowPaintSummaryBytes !== 53_324 ||
      latestEvidence.nativeUpdateWindowPaintSummarySha256 !==
        "88bdde2c8480bbb0ea1ad2b9a3b82aa9b1a69f5842eb4921fc9e34be6bc6f50c" ||
      latestEvidence.nativeUpdateWindowPaintBalancedApiPairs !== 693 ||
      latestEvidence.nativeUpdateWindowPaintTraceDiagnostics !== 0 ||
      latestEvidence.nativeUpdateWindowPaintProfile !==
        "x86-bgi-update-window-full-client-paint-v1" ||
      latestEvidence.nativeUpdateWindowPaintOracleProfile !==
        "x86-update-window-paint-invalid-window-v1" ||
      latestEvidence.nativeUpdateWindowCallerRva !== "0x000938e7" ||
      latestEvidence.nativeUpdateWindowResult !== 1 ||
      latestEvidence.nativeUpdateWindowLastError !== 0 ||
      latestEvidence.nativeUpdateWindowMessage !== 15 ||
      latestEvidence.nativeUpdateWindowCallbackResult !== 0 ||
      latestEvidence.nativeUpdateWindowUpdateRectBefore?.join(",") !==
        "0,0,1280,720" ||
      latestEvidence.nativeUpdateWindowUpdateRectConsumed !== true ||
      latestEvidence.nativeBeginPaintCallerRva !== "0x000928d9" ||
      latestEvidence.nativeBeginPaintDeviceContextNonzero !== true ||
      latestEvidence.nativeBeginPaintLastError !== 0 ||
      latestEvidence.nativeBeginPaintStructBytes !== 64 ||
      latestEvidence.nativeBeginPaintErase !== 1 ||
      latestEvidence.nativeBeginPaintRect?.join(",") !== "0,0,1280,720" ||
      latestEvidence.nativeBeginPaintRestore !== 0 ||
      latestEvidence.nativeBeginPaintIncrementalUpdate !== 0 ||
      latestEvidence.nativeBeginPaintReservedZeroBytes !== 32 ||
      latestEvidence.nativeEndPaintCallerRva !== "0x000928e5" ||
      latestEvidence.nativeEndPaintResult !== 1 ||
      latestEvidence.nativeEndPaintLastError !== 0 ||
      latestEvidence.nativeEndPaintStructUnchanged !== true ||
      latestEvidence.nativeUpdateWindowInvalidWindowResult !== 0 ||
      latestEvidence.nativeBeginPaintInvalidWindowResult !== 0 ||
      latestEvidence.nativeEndPaintInvalidWindowResult !== 0 ||
      latestEvidence.nativeUpdateWindowInvalidWindowLastError !== 1_400 ||
      latestEvidence.nativeBeginPaintInvalidWindowLastError !== 1_400 ||
      latestEvidence.nativeEndPaintInvalidWindowLastError !== 1_400 ||
      latestEvidence.nativeBeginPaintInvalidWindowStructUnchanged !== true ||
      latestEvidence.nativeEndPaintInvalidWindowStructUnchanged !== true ||
      latestEvidence.nativeUpdateWindowExecutableHashUnchanged !== true ||
      latestEvidence.updateWindowImplemented !== true ||
      latestEvidence.updateWindowSequence !== 127_352 ||
      latestEvidence.updateWindowInstructionCount !== 28_761_031 ||
      latestEvidence.updateWindowArguments?.join(",") !== "0x48010001" ||
      latestEvidence.updateWindowPendingCallback !== true ||
      latestEvidence.updateWindowCallbackFrameId !== 30 ||
      latestEvidence.updateWindowCallbackMessage !== 15 ||
      latestEvidence.updateWindowCallbackTarget !== "0x10092550" ||
      latestEvidence.updateWindowCallbackStackPointer !== "0x006ff674" ||
      latestEvidence.beginPaintImplemented !== true ||
      latestEvidence.beginPaintSequence !== 127_353 ||
      latestEvidence.beginPaintInstructionCount !== 28_761_068 ||
      latestEvidence.beginPaintArguments?.join(",") !==
        "0x48010001,0x006ff318" ||
      latestEvidence.beginPaintResult !== "0x47010009" ||
      latestEvidence.beginPaintImplementationId !==
        "user32.begin-paint.full-client-logical-dc.v1" ||
      latestEvidence.beginPaintReturnAddress !== "0x100928d9" ||
      latestEvidence.beginPaintStackPointers?.join(",") !==
        "0x006ff2cc,0x006ff2d8" ||
      latestEvidence.beginPaintLastError !== 0 ||
      latestEvidence.beginPaintGuestWriteBytes !== 64 ||
      latestEvidence.beginPaintPaintStructBytes !== 64 ||
      latestEvidence.beginPaintErase !== 1 ||
      latestEvidence.beginPaintUpdateRect?.join(",") !== "0,0,1280,720" ||
      latestEvidence.beginPaintGeneration !== 1 ||
      latestEvidence.beginPaintLivePaintSessions !== 1 ||
      latestEvidence.beginPaintLiveDeviceContexts !== 1 ||
      latestEvidence.endPaintImplemented !== true ||
      latestEvidence.endPaintSequence !== 127_354 ||
      latestEvidence.endPaintInstructionCount !== 28_761_073 ||
      latestEvidence.endPaintArguments?.join(",") !==
        "0x48010001,0x006ff318" ||
      latestEvidence.endPaintResult !== 1 ||
      latestEvidence.endPaintImplementationId !==
        "user32.end-paint.matching-session.v1" ||
      latestEvidence.endPaintReturnAddress !== "0x100928e5" ||
      latestEvidence.endPaintStackPointers?.join(",") !==
        "0x006ff2cc,0x006ff2d8" ||
      latestEvidence.endPaintLastError !== 0 ||
      latestEvidence.endPaintGuestWriteBytes !== 0 ||
      latestEvidence.endPaintLivePaintSessions !== 0 ||
      latestEvidence.endPaintLiveDeviceContexts !== 0 ||
      latestEvidence.updateWindowWindowAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.updateWindowUpdateRegionAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.updateWindowUpdateRegionSource !==
        "set-window-pos-frame-change-nccalcsize-completion" ||
      latestEvidence.updateWindowUpdateRegionConsumption !==
        "begin-paint-targeted-commit" ||
      latestEvidence.updateWindowPaintSessionAuthority !==
        "runtime/user32.rs::RuntimeUser32" ||
      latestEvidence.updateWindowDeviceContextAuthority !==
        "runtime/gdi.rs::RuntimeGdi" ||
      latestEvidence.updateWindowTransaction !==
        "user32-gdi-guest-memory-preflight-before-targeted-commit" ||
      latestEvidence.updateWindowPaintSessionRetired !== true ||
      latestEvidence.updateWindowDeviceContextRetired !== true ||
      latestEvidence.updateWindowFailureAtomicity !== true ||
      latestEvidence.updateWindowCallbackActiveAtStop !== true ||
      latestEvidence.updateWindowBrowserOwnsLogicalWindowState !== false ||
      latestEvidence.updateWindowBrowserResourceCreated !== false ||
      latestEvidence.updateWindowNodeCompletions !== 3 ||
      latestEvidence.updateWindowChromeCompletions !== 3 ||
      latestEvidence.updateWindowNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push(
      "latest native and private UpdateWindow paint lifecycle evidence is invalid",
    );
  }
  if (d3d9ClearActive) {
    for (const field of [
      "nativeD3d9ClearPrimaryTraceSha256",
      "nativeD3d9ClearPrimarySummarySha256",
      "nativeD3d9ClearConfirmationTraceSha256",
      "nativeD3d9ClearConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    const latestClearEvidencePairs = [
      [latestEvidence.nativeD3d9ClearProfileId, d3d9NativeClear.profileId],
      [latestEvidence.nativeD3d9ClearPrimaryTraceBytes, d3d9NativeClear.primaryTraceBytes],
      [latestEvidence.nativeD3d9ClearPrimaryTraceLines, d3d9NativeClear.primaryTraceLines],
      [latestEvidence.nativeD3d9ClearPrimaryTraceSha256, d3d9NativeClear.primaryTraceSha256],
      [latestEvidence.nativeD3d9ClearPrimarySummaryBytes, d3d9NativeClear.primarySummaryBytes],
      [latestEvidence.nativeD3d9ClearPrimarySummarySha256, d3d9NativeClear.primarySummarySha256],
      [
        latestEvidence.nativeD3d9ClearPrimaryBalancedApiPairs,
        d3d9NativeClear.primaryBalancedApiPairs,
      ],
      [latestEvidence.nativeD3d9ClearPrimaryCalls, d3d9NativeClear.primaryCalls],
      [
        latestEvidence.nativeD3d9ClearConfirmationTraceBytes,
        d3d9NativeClear.confirmationTraceBytes,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationTraceLines,
        d3d9NativeClear.confirmationTraceLines,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationTraceSha256,
        d3d9NativeClear.confirmationTraceSha256,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationSummaryBytes,
        d3d9NativeClear.confirmationSummaryBytes,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationSummarySha256,
        d3d9NativeClear.confirmationSummarySha256,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationBalancedApiPairs,
        d3d9NativeClear.confirmationBalancedApiPairs,
      ],
      [
        latestEvidence.nativeD3d9ClearConfirmationCalls,
        d3d9NativeClear.confirmationCalls,
      ],
      [latestEvidence.nativeD3d9ClearTraceDiagnostics, d3d9NativeClear.diagnostics],
      [latestEvidence.nativeD3d9ClearCallerRva, d3d9NativeClear.callerRva],
      [latestEvidence.nativeD3d9ClearArguments, d3d9NativeClear.arguments],
      [latestEvidence.nativeD3d9ClearResult, d3d9NativeClear.result],
      [latestEvidence.nativeD3d9ClearLastError, d3d9NativeClear.lastError],
      [
        latestEvidence.nativeD3d9ClearAllCallsMatchProfile,
        d3d9NativeClear.allCallsMatchProfile,
      ],
      [
        latestEvidence.nativeD3d9ClearExecutableHashUnchanged,
        d3d9NativeClear.sourceExecutableHashUnchanged,
      ],
      [latestEvidence.d3d9ClearProfileId, d3d9Clear.profileId],
      [latestEvidence.d3d9ClearSequence, d3d9PrivateClear.nodeSequence],
      [
        latestEvidence.d3d9ClearInstructionCount,
        d3d9PrivateClear.nodeInstructionCount,
      ],
      [latestEvidence.d3d9ClearArguments, d3d9PrivateClear.arguments],
      [latestEvidence.d3d9ClearReturnAddress, d3d9PrivateClear.returnAddress],
      [latestEvidence.d3d9ClearResult, d3d9PrivateClear.result],
      [latestEvidence.d3d9ClearGuestWriteBytes, d3d9PrivateClear.guestWriteBytes],
      [
        latestEvidence.d3d9ClearDeviceReferenceCount,
        d3d9PrivateClear.deviceReferenceCount,
      ],
      [
        latestEvidence.d3d9ClearDeviceObjectRetired,
        d3d9PrivateClear.deviceObjectRetired,
      ],
      [latestEvidence.d3d9ClearTargetAuthority, d3d9Clear.targetAuthority],
      [latestEvidence.d3d9ClearTargetScope, d3d9Clear.targetScope],
      [latestEvidence.d3d9ClearMutation, d3d9Clear.mutation],
      [
        latestEvidence.d3d9ClearLogicalGraphicsDeviceId,
        d3d9PrivateClear.logicalGraphicsDeviceId,
      ],
      [
        latestEvidence.d3d9ClearLogicalGraphicsDeviceNamespace,
        d3d9PrivateClear.logicalGraphicsDeviceNamespace,
      ],
      [
        latestEvidence.d3d9ClearLogicalGraphicsComObjectId,
        d3d9PrivateClear.logicalGraphicsComObjectId,
      ],
      [
        latestEvidence.d3d9ClearPresentationGeometry,
        d3d9PrivateClear.presentationGeometry,
      ],
      [
        latestEvidence.d3d9ClearBackBufferContentGeneration,
        d3d9PrivateClear.backBufferContentGeneration,
      ],
      [
        latestEvidence.d3d9ClearBackBufferSequence,
        d3d9PrivateClear.backBufferClearSequence,
      ],
      [latestEvidence.d3d9ClearBackBufferRect, d3d9PrivateClear.backBufferClearRect],
      [
        latestEvidence.d3d9ClearBackBufferColorArgb,
        d3d9PrivateClear.backBufferClearColorArgb,
      ],
      [
        latestEvidence.d3d9ClearLogicalGraphicsDeviceGeneration,
        d3d9PrivateClear.logicalGraphicsDeviceGeneration,
      ],
      [latestEvidence.d3d9ClearFailureAtomicity, true],
      [latestEvidence.d3d9ClearDepthStencilCleared, false],
      [latestEvidence.d3d9ClearPartialRectanglesImplemented, false],
      [latestEvidence.d3d9ClearBrowserOwnsGraphicsState, false],
      [latestEvidence.d3d9ClearBrowserRendererResourceCreated, false],
      [latestEvidence.d3d9ClearNodeCompletions, 1],
      [latestEvidence.d3d9ClearChromeCompletions, 1],
      [latestEvidence.d3d9ClearNodeChromeOrderedSemanticsMatched, true],
    ];
    if (
      latestClearEvidencePairs.some(
        ([actual, expected]) =>
          JSON.stringify(actual) !== JSON.stringify(expected),
      ) ||
      latestEvidence.d3d9ClearImplemented !== true ||
      latestEvidence.d3d9ClearPolicySchema !==
        "vine.d3d9-compatibility-policy" ||
      latestEvidence.d3d9ClearPolicyVersion !== 22 ||
      latestEvidence.d3d9ClearPolicyId !==
        "d3d9-primary-virtual-adapter-v22"
    ) {
      errors.push("latest native and private D3D9 Clear evidence is invalid");
    }
  }
  if (setRectActive) {
    for (const field of [
      "nativeSetRectPrimaryTraceSha256",
      "nativeSetRectPrimarySummarySha256",
      "nativeSetRectConfirmationTraceSha256",
      "nativeSetRectConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeSetRectProfileId !==
        "x86-bgi-first-set-rect-full-presentation-v1" ||
      latestEvidence.nativeSetRectOracleProfileId !==
        "x86-primary-signed-and-null-output-v1" ||
      latestEvidence.nativeSetRectPrimaryTraceBytes !== 348_849 ||
      latestEvidence.nativeSetRectPrimaryTraceLines !== 1_073 ||
      latestEvidence.nativeSetRectPrimaryTraceSha256 !==
        "5d61cb2fcd3af82a0cb57e0d2906d62d4fd3b61b428c5fa8d9f794cdc38eb823" ||
      latestEvidence.nativeSetRectPrimarySummaryBytes !== 50_944 ||
      latestEvidence.nativeSetRectPrimarySummarySha256 !==
        "68c351ae87d7405e602154c7029e7afb3a4bb3ed1306bd996eb6050badbbbc1f" ||
      latestEvidence.nativeSetRectPrimaryBalancedApiPairs !== 498 ||
      latestEvidence.nativeSetRectConfirmationTraceBytes !== 348_929 ||
      latestEvidence.nativeSetRectConfirmationTraceLines !== 1_073 ||
      latestEvidence.nativeSetRectConfirmationTraceSha256 !==
        "32816981a39a1b5fc6bcac889c54bf2853d853db4c18b3cc2aaba1e218e64f98" ||
      latestEvidence.nativeSetRectConfirmationSummaryBytes !== 51_117 ||
      latestEvidence.nativeSetRectConfirmationSummarySha256 !==
        "285db80fb06213bf213beed0b7ddfa289bbbf28d24df2bd9fb5b223d3bde83c1" ||
      latestEvidence.nativeSetRectConfirmationBalancedApiPairs !== 498 ||
      latestEvidence.nativeSetRectTraceDiagnostics !== 0 ||
      latestEvidence.nativeSetRectCallsPerTrace !== 4 ||
      latestEvidence.nativeSetRectCallerRva !== "0x0005b00b" ||
      latestEvidence.nativeSetRectCoordinates?.join(",") !== "0,0,1280,720" ||
      latestEvidence.nativeSetRectResult !== 1 ||
      latestEvidence.nativeSetRectLastError !== 0 ||
      latestEvidence.nativeSetRectOutputRect?.join(",") !== "0,0,1280,720" ||
      latestEvidence.nativeSetRectSignedCoordinates?.join(",") !==
        "-7,9,123,-456" ||
      latestEvidence.nativeSetRectNullOutputResult !== 0 ||
      latestEvidence.nativeSetRectSuccessLastErrorPreserved !== true ||
      latestEvidence.nativeSetRectNullLastErrorPreserved !== true ||
      latestEvidence.nativeSetRectConsumer !==
        "result-ignored-texture-add-dirty-rect" ||
      latestEvidence.nativeSetRectExecutableHashUnchanged !== true ||
      latestEvidence.setRectImplemented !== true ||
      latestEvidence.setRectSequence !== 127_356 ||
      latestEvidence.setRectInstructionCount !== 28_761_157 ||
      latestEvidence.setRectArguments?.join(",") !==
        "0x006ff29c,0x00000000,0x00000000,0x00000500,0x000002d0" ||
      latestEvidence.setRectReturnAddress !== "0x1005b00b" ||
      latestEvidence.setRectStackPointers?.join(",") !==
        "0x006ff250,0x006ff268" ||
      latestEvidence.setRectResult !== 1 ||
      latestEvidence.setRectLastError !== 0 ||
      latestEvidence.setRectGuestWriteBytes !== 16 ||
      latestEvidence.setRectOutputAddress !== "0x006ff29c" ||
      latestEvidence.setRectOutputHex !==
        "000000000000000000050000d0020000" ||
      latestEvidence.setRectImplementationId !== "user32.set-rect.v1" ||
      latestEvidence.setRectMutation !==
        "guest-memory-preflight-atomic-write" ||
      latestEvidence.setRectNullOutputReturnsFalse !== true ||
      latestEvidence.setRectFailureAtomicity !== true ||
      latestEvidence.setRectBrowserOwnsRectState !== false ||
      latestEvidence.setRectNodeCompletions !== 1 ||
      latestEvidence.setRectChromeCompletions !== 1 ||
      latestEvidence.setRectNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push("latest native and private SetRect evidence is invalid");
    }
  }
  if (d3d9AddDirtyRectActive) {
    for (const field of [
      "nativeAddDirtyRectPrimaryTraceSha256",
      "nativeAddDirtyRectPrimarySummarySha256",
      "nativeAddDirtyRectConfirmationTraceSha256",
      "nativeAddDirtyRectConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeAddDirtyRectProfileId !==
        "x86-bgi-first-recreated-texture-add-dirty-rect-v1" ||
      latestEvidence.nativeAddDirtyRectOracleProfileId !==
        "x86-system-memory-texture-rect-variants-v1" ||
      latestEvidence.nativeAddDirtyRectPrimaryTraceBytes !== 509_016 ||
      latestEvidence.nativeAddDirtyRectPrimaryTraceLines !== 1_193 ||
      latestEvidence.nativeAddDirtyRectPrimaryTraceSha256 !==
        "5677e484c6650c33f3757265d4185649816ee92b823b9339b44bd15fac838680" ||
      latestEvidence.nativeAddDirtyRectPrimarySummaryBytes !== 303_427 ||
      latestEvidence.nativeAddDirtyRectPrimarySummarySha256 !==
        "c39f201c909ec5f6b9cb242a389d382e6f27fe427a03a156eb9580cd08c6a209" ||
      latestEvidence.nativeAddDirtyRectPrimaryBalancedApiPairs !== 556 ||
      latestEvidence.nativeAddDirtyRectConfirmationTraceBytes !== 508_597 ||
      latestEvidence.nativeAddDirtyRectConfirmationTraceLines !== 1_192 ||
      latestEvidence.nativeAddDirtyRectConfirmationTraceSha256 !==
        "a8eb8bb0d19b84392b0543ebdae66f76d938b997bf7856e36726b54c790b89bf" ||
      latestEvidence.nativeAddDirtyRectConfirmationSummaryBytes !== 303_587 ||
      latestEvidence.nativeAddDirtyRectConfirmationSummarySha256 !==
        "5071dc1b418c0c173e4deae4f62f23b0ee9f1d1e7450b896e7bcac12f03bf3fa" ||
      latestEvidence.nativeAddDirtyRectConfirmationBalancedApiPairs !== 555 ||
      latestEvidence.nativeAddDirtyRectTraceDiagnostics !== 0 ||
      latestEvidence.nativeAddDirtyRectCallsPerTrace !== 8 ||
      latestEvidence.nativeAddDirtyRectCreationCallerRva !== "0x0005a715" ||
      latestEvidence.nativeAddDirtyRectCallerRva !== "0x0005b01d" ||
      latestEvidence.nativeAddDirtyRectPrimaryRect?.join(",") !==
        "0,0,1280,720" ||
      latestEvidence.nativeAddDirtyRectValidInteriorRect?.join(",") !==
        "1,2,17,19" ||
      latestEvidence.nativeAddDirtyRectNullRectAccepted !== true ||
      latestEvidence.nativeAddDirtyRectValidResult !== "0x00000000" ||
      latestEvidence.nativeAddDirtyRectInvalidResult !== "0x8876086c" ||
      latestEvidence.nativeAddDirtyRectLastErrorPreserved !== true ||
      latestEvidence.nativeAddDirtyRectImmediateConsumer !==
        "IDirect3DDevice9::UpdateTexture-slot-31" ||
      latestEvidence.nativeAddDirtyRectExecutableHashUnchanged !== true ||
      latestEvidence.d3d9AddDirtyRectImplemented !== true ||
      latestEvidence.d3d9AddDirtyRectSequence !== 127_357 ||
      latestEvidence.d3d9AddDirtyRectInstructionCount !== 28_761_165 ||
      latestEvidence.d3d9AddDirtyRectArguments?.join(",") !==
        "0x01020004,0x006ff29c" ||
      latestEvidence.d3d9AddDirtyRectReturnAddress !== "0x1005b01d" ||
      latestEvidence.d3d9AddDirtyRectStackPointers?.join(",") !==
        "0x006ff25c,0x006ff268" ||
      latestEvidence.d3d9AddDirtyRectResult !== "0x00000000" ||
      latestEvidence.d3d9AddDirtyRectGuestWriteBytes !== 0 ||
      latestEvidence.d3d9AddDirtyRectTextureReferenceCount !== 1 ||
      latestEvidence.d3d9AddDirtyRectTextureObjectRetired !== false ||
      latestEvidence.d3d9AddDirtyRectPolicyVersion !== 22 ||
      latestEvidence.d3d9AddDirtyRectPolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      latestEvidence.d3d9AddDirtyRectProfileId !==
        "primary-recreated-system-memory-texture-dirty-rect-v1" ||
      latestEvidence.d3d9AddDirtyRectTextureAuthority !==
        "rust-runtime-graphics-logical-texture" ||
      latestEvidence.d3d9AddDirtyRectInitialDirtyRegion !== "whole-texture" ||
      latestEvidence.d3d9AddDirtyRectMutation !==
        "validate-plan-preflight-idempotent-completion" ||
      latestEvidence.d3d9AddDirtyRectLogicalGraphicsTextureId !== 7 ||
      latestEvidence.d3d9AddDirtyRectLogicalGraphicsTextureNamespace !==
        "vine.logical-graphics-texture.v1" ||
      latestEvidence.d3d9AddDirtyRectLogicalGraphicsDeviceId !== 3 ||
      latestEvidence.d3d9AddDirtyRectLogicalGraphicsComObjectId !== "22" ||
      latestEvidence.d3d9AddDirtyRectTextureGeometry?.join(",") !==
        "2048,1024" ||
      latestEvidence.d3d9AddDirtyRectTexturePool !== "system-memory" ||
      latestEvidence.d3d9AddDirtyRectDirtyRegion !== "whole-texture" ||
      latestEvidence.d3d9AddDirtyRectTextureGeneration !== 41 ||
      latestEvidence.d3d9AddDirtyRectContentGeneration !== 1 ||
      latestEvidence.d3d9AddDirtyRectLogicalFillSequence !== 3 ||
      latestEvidence.d3d9AddDirtyRectTextureGenerationChanged !== false ||
      latestEvidence.d3d9AddDirtyRectFailureAtomicity !== true ||
      latestEvidence.d3d9AddDirtyRectDirtyRegionConsumed !== false ||
      latestEvidence.d3d9AddDirtyRectBrowserOwnsGraphicsState !== false ||
      latestEvidence.d3d9AddDirtyRectBrowserRendererResourceCreated !== false ||
      latestEvidence.d3d9AddDirtyRectNodeCompletions !== 1 ||
      latestEvidence.d3d9AddDirtyRectChromeCompletions !== 1 ||
      latestEvidence.d3d9AddDirtyRectNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push(
        "latest native and private D3D9 AddDirtyRect evidence is invalid",
      );
    }
  }
  if (d3d9UpdateTextureActive) {
    for (const field of [
      "nativeUpdateTexturePrimaryTraceSha256",
      "nativeUpdateTexturePrimarySummarySha256",
      "nativeUpdateTextureConfirmationTraceSha256",
      "nativeUpdateTextureConfirmationSummarySha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeUpdateTextureProfileId !==
        "x86-bgi-first-recreated-texture-update-v1" ||
      latestEvidence.nativeUpdateTextureOracleProfileId !==
        "x86-recreated-texture-update-pool-and-identity-variants-v1" ||
      latestEvidence.nativeUpdateTexturePrimaryTraceBytes !== 550_638 ||
      latestEvidence.nativeUpdateTexturePrimaryTraceLines !== 1_219 ||
      latestEvidence.nativeUpdateTexturePrimaryTraceSha256 !==
        "f24e8be66f5f6e2507c455e396cee1aa43eea9e42ff888eb20ec3896ac698601" ||
      latestEvidence.nativeUpdateTexturePrimarySummaryBytes !== 382_225 ||
      latestEvidence.nativeUpdateTexturePrimarySummarySha256 !==
        "1c2cb3f39db3609f91306fde02da4c3e8485649ea7610cbb68fc20380e5d793f" ||
      latestEvidence.nativeUpdateTexturePrimaryBalancedApiPairs !== 568 ||
      latestEvidence.nativeUpdateTextureConfirmationTraceBytes !== 549_350 ||
      latestEvidence.nativeUpdateTextureConfirmationTraceLines !== 1_214 ||
      latestEvidence.nativeUpdateTextureConfirmationTraceSha256 !==
        "499d6fdfa93f1f7f225e345b8270589e5b93e691452f70069edfecaba78d7f5e" ||
      latestEvidence.nativeUpdateTextureConfirmationSummaryBytes !== 382_103 ||
      latestEvidence.nativeUpdateTextureConfirmationSummarySha256 !==
        "0af54fd50a108ade2364e17789184fd8d3b56968c076cf7af0dffc898bbd39cb" ||
      latestEvidence.nativeUpdateTextureConfirmationBalancedApiPairs !== 566 ||
      latestEvidence.nativeUpdateTextureTraceDiagnostics !== 0 ||
      latestEvidence.nativeUpdateTextureCreationCallerRvas?.join(",") !==
        "0x0005a715,0x0005a7b7,0x0005a7dc" ||
      latestEvidence.nativeUpdateTextureCallerRva !== "0x0005b038" ||
      latestEvidence.nativeUpdateTextureSourceVariantIndex !== 3 ||
      latestEvidence.nativeUpdateTextureDestinationVariantIndex !== 4 ||
      latestEvidence.nativeUpdateTextureDynamicDestinationVariantIndex !== 5 ||
      latestEvidence.nativeUpdateTextureNonNullSuccessVariants?.join(",") !==
        "primary,repeat,dynamic-destination" ||
      latestEvidence.nativeUpdateTextureNonNullInvalidVariants?.join(",") !==
        "reversed-pools,default-source,same-resource" ||
      latestEvidence.nativeUpdateTextureNullVariantClaimed !== false ||
      latestEvidence.nativeUpdateTextureValidResult !== "0x00000000" ||
      latestEvidence.nativeUpdateTextureInvalidResult !== "0x8876086c" ||
      latestEvidence.nativeUpdateTextureLastErrorPreserved !== true ||
      latestEvidence.nativeUpdateTextureImmediateConsumer !==
        "IDirect3DDevice9::BeginScene-slot-41" ||
      latestEvidence.nativeUpdateTextureExecutableHashUnchanged !== true ||
      latestEvidence.d3d9UpdateTextureImplemented !== true ||
      latestEvidence.d3d9UpdateTextureSequence !== 127_358 ||
      latestEvidence.d3d9UpdateTextureInstructionCount !== 28_761_175 ||
      latestEvidence.d3d9UpdateTextureArguments?.join(",") !==
        "0x01010004,0x01020004,0x01030004" ||
      latestEvidence.d3d9UpdateTextureReturnAddress !== "0x1005b038" ||
      latestEvidence.d3d9UpdateTextureStackPointers?.join(",") !==
        "0x006ff258,0x006ff268" ||
      latestEvidence.d3d9UpdateTextureResult !== "0x00000000" ||
      latestEvidence.d3d9UpdateTextureGuestWriteBytes !== 0 ||
      latestEvidence.d3d9UpdateTextureDeviceReferenceCount !== 5 ||
      latestEvidence.d3d9UpdateTextureDeviceObjectRetired !== false ||
      latestEvidence.d3d9UpdateTexturePolicyVersion !== 22 ||
      latestEvidence.d3d9UpdateTexturePolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      latestEvidence.d3d9UpdateTextureProfileId !==
        "primary-recreated-system-memory-to-static-default-texture-update-v1" ||
      latestEvidence.d3d9UpdateTextureContentAuthority !==
        "rust-runtime-graphics-logical-texture" ||
      latestEvidence.d3d9UpdateTextureSourceDirtyRegionBefore !==
        "whole-texture" ||
      latestEvidence.d3d9UpdateTextureSourceDirtyRegionAfter !== "clean" ||
      latestEvidence.d3d9UpdateTextureDirtyRegionConsumed !== true ||
      latestEvidence.d3d9UpdateTextureTransferScope !==
        "whole-texture-level-zero-logical-content" ||
      latestEvidence.d3d9UpdateTextureMutation !==
        "dual-texture-preflight-targeted-commit" ||
      latestEvidence.d3d9UpdateTextureSourceTextureId !== 7 ||
      latestEvidence.d3d9UpdateTextureSourceComObjectId !== "22" ||
      latestEvidence.d3d9UpdateTextureSourcePool !== "system-memory" ||
      latestEvidence.d3d9UpdateTextureSourceGeneration !== 42 ||
      latestEvidence.d3d9UpdateTextureDestinationTextureId !== 8 ||
      latestEvidence.d3d9UpdateTextureDestinationComObjectId !== "24" ||
      latestEvidence.d3d9UpdateTextureDestinationPool !== "default" ||
      latestEvidence.d3d9UpdateTextureDestinationGeneration !== 45 ||
      latestEvidence.d3d9UpdateTextureContentGeneration !== 1 ||
      latestEvidence.d3d9UpdateTextureLogicalFillSequence !== 3 ||
      latestEvidence.d3d9UpdateTextureLogicalFillRect?.join(",") !==
        "0,0,1280,720" ||
      latestEvidence.d3d9UpdateTextureLogicalFillColorRef !== "0x00000000" ||
      latestEvidence.d3d9UpdateTextureUpdateSequence !== 1 ||
      latestEvidence.d3d9UpdateTextureFailureAtomicity !== true ||
      latestEvidence.d3d9UpdateTexturePartialDirtyRegionsImplemented !== false ||
      latestEvidence.d3d9UpdateTextureDynamicDestinationImplemented !== false ||
      latestEvidence.d3d9UpdateTextureBrowserOwnsGraphicsState !== false ||
      latestEvidence.d3d9UpdateTextureBrowserRendererResourceCreated !== false ||
      latestEvidence.d3d9UpdateTextureNodeCompletions !== 1 ||
      latestEvidence.d3d9UpdateTextureChromeCompletions !== 1 ||
      latestEvidence.d3d9UpdateTextureNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push(
        "latest native and private D3D9 UpdateTexture evidence is invalid",
      );
    }
  }
  if (d3d9BeginSceneActive) {
    for (const field of [
      "nativeBeginScenePrimaryTraceSha256",
      "nativeBeginScenePrimarySummarySha256",
      "nativeBeginSceneConfirmationTraceSha256",
      "nativeBeginSceneConfirmationSummarySha256",
      "nativeBeginSceneOracleSourceSha256",
      "nativeBeginSceneOracleObjectSha256",
      "nativeBeginSceneOracleExecutableSha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeBeginSceneProfileId !==
        "x86-bgi-first-begin-scene-v1" ||
      latestEvidence.nativeBeginSceneOracleProfileId !==
        "x86-d3d9-scene-lifecycle-v1" ||
      latestEvidence.nativeBeginScenePrimaryTraceBytes !== 470_012 ||
      latestEvidence.nativeBeginScenePrimaryTraceLines !== 1_171 ||
      latestEvidence.nativeBeginScenePrimaryTraceSha256 !==
        "04d6a33b19276408e408478c916ac27e41565c3516bb8f5cc43136955236b150" ||
      latestEvidence.nativeBeginScenePrimarySummaryBytes !== 273_814 ||
      latestEvidence.nativeBeginScenePrimarySummarySha256 !==
        "d3243dbdad2e54f543b1fefa6bce4080f955b102f1c21f196cffaacf45de9843" ||
      latestEvidence.nativeBeginScenePrimaryBalancedApiPairs !== 546 ||
      latestEvidence.nativeBeginSceneConfirmationTraceBytes !== 469_761 ||
      latestEvidence.nativeBeginSceneConfirmationTraceLines !== 1_174 ||
      latestEvidence.nativeBeginSceneConfirmationTraceSha256 !==
        "f7dd018d61e247e42f18ad315107c8755753462b697bfdf6deb3f7c2f8c90d52" ||
      latestEvidence.nativeBeginSceneConfirmationSummaryBytes !== 274_066 ||
      latestEvidence.nativeBeginSceneConfirmationSummarySha256 !==
        "2fbd178a0cea82d37fccacb350294ca934be5b85841fd91dda6952661b157054" ||
      latestEvidence.nativeBeginSceneConfirmationBalancedApiPairs !== 547 ||
      latestEvidence.nativeBeginSceneTraceDiagnostics !== 0 ||
      latestEvidence.nativeBeginSceneCallerRva !== "0x0005b048" ||
      latestEvidence.nativeBeginSceneArguments?.length !== 0 ||
      latestEvidence.nativeBeginSceneResult !== "0x00000000" ||
      latestEvidence.nativeBeginSceneLastErrorBefore !== 0 ||
      latestEvidence.nativeBeginSceneLastErrorAfter !== 0 ||
      latestEvidence.nativeBeginSceneImmediateConsumer !==
        "hresult-sign-test-then-first-frame-state-setup" ||
      latestEvidence.nativeBeginSceneOracleArchitecture !== "coff-i386-pe32" ||
      latestEvidence.nativeBeginSceneOracleSourceBytes !== 8_025 ||
      latestEvidence.nativeBeginSceneOracleSourceSha256 !==
        "47e39e28ca77f7e6164e39afd3ae5448930c221aa84c7711d2e19b57230b8e77" ||
      latestEvidence.nativeBeginSceneOracleObjectBytes !== 8_639 ||
      latestEvidence.nativeBeginSceneOracleObjectSha256 !==
        "db268f3154fa5fd5d55763b61da9c5f768997f70a95158f3fc2b54ff73be7c34" ||
      latestEvidence.nativeBeginSceneOracleExecutableBytes !== 5_120 ||
      latestEvidence.nativeBeginSceneOracleExecutableSha256 !==
        "092ae273c871747c88cd76fac5b476169e479ac04fad50de1ffcf30a0870c1b5" ||
      latestEvidence.nativeBeginSceneOracleRuns !== 2 ||
      latestEvidence.nativeBeginSceneOracleExitCodes?.join(",") !== "0,0" ||
      latestEvidence.nativeBeginSceneOracleLastErrorSentinel !== 23_130 ||
      latestEvidence.nativeBeginSceneOracleLastErrorPreserved !== true ||
      latestEvidence.nativeBeginSceneOracleBeginFirstResult !== "0x00000000" ||
      latestEvidence.nativeBeginSceneOracleBeginNestedResult !== "0x8876086c" ||
      latestEvidence.nativeBeginSceneOracleEndFirstResult !== "0x00000000" ||
      latestEvidence.nativeBeginSceneOracleEndOutsideResult !== "0x8876086c" ||
      latestEvidence.nativeBeginSceneOracleEndRepeatedResult !== "0x8876086c" ||
      latestEvidence.nativeBeginSceneOracleSecondCycleSucceeded !== true ||
      latestEvidence.nativeBeginSceneOracleDeviceReferenceCounts?.join(",") !==
        "1,1" ||
      latestEvidence.nativeBeginSceneExecutableHashUnchanged !== true ||
      latestEvidence.d3d9BeginSceneImplemented !== true ||
      latestEvidence.d3d9BeginSceneSequence !== 127_359 ||
      latestEvidence.d3d9BeginSceneInstructionCount !== 28_761_181 ||
      latestEvidence.d3d9BeginSceneArguments?.join(",") !== "0x01010004" ||
      latestEvidence.d3d9BeginSceneReturnAddress !== "0x1005b048" ||
      latestEvidence.d3d9BeginSceneStackPointers?.join(",") !==
        "0x006ff260,0x006ff268" ||
      latestEvidence.d3d9BeginSceneResult !== "0x00000000" ||
      latestEvidence.d3d9BeginSceneGuestWriteBytes !== 0 ||
      latestEvidence.d3d9BeginSceneDeviceReferenceCount !== 5 ||
      latestEvidence.d3d9BeginSceneDeviceObjectRetired !== false ||
      latestEvidence.d3d9BeginScenePolicyVersion !== 22 ||
      latestEvidence.d3d9BeginScenePolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      latestEvidence.d3d9BeginSceneProfileId !==
        "primary-first-frame-scene-begin-v1" ||
      latestEvidence.d3d9BeginSceneAuthority !==
        "rust-runtime-graphics-logical-device" ||
      latestEvidence.d3d9BeginSceneLifecycle !== "inactive-to-active" ||
      latestEvidence.d3d9BeginSceneMutation !==
        "validate-plan-preflight-targeted-commit" ||
      latestEvidence.d3d9BeginSceneLogicalGraphicsDeviceId !== 3 ||
      latestEvidence.d3d9BeginSceneLogicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      latestEvidence.d3d9BeginSceneLogicalGraphicsComObjectId !== "21" ||
      latestEvidence.d3d9BeginScenePresentationGeometry?.join(",") !==
        "1280,720" ||
      latestEvidence.d3d9BeginSceneBackBufferContentGeneration !== 1 ||
      latestEvidence.d3d9BeginSceneBackBufferClearSequence !== 1 ||
      latestEvidence.d3d9BeginSceneActiveSceneSequence !== 1 ||
      latestEvidence.d3d9BeginSceneLogicalGraphicsDeviceGeneration !== 45 ||
      latestEvidence.d3d9BeginSceneNestedBeginReturnsInvalidCall !== true ||
      latestEvidence.d3d9BeginSceneFailureAtomicity !== true ||
      latestEvidence.d3d9BeginSceneEndSceneImplemented !== false ||
      latestEvidence.d3d9BeginSceneBrowserOwnsGraphicsState !== false ||
      latestEvidence.d3d9BeginSceneBrowserRendererResourceCreated !== false ||
      latestEvidence.d3d9BeginSceneNodeCompletions !== 1 ||
      latestEvidence.d3d9BeginSceneChromeCompletions !== 1 ||
      latestEvidence.d3d9BeginSceneNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push(
        "latest native and private D3D9 BeginScene evidence is invalid",
      );
    }
  }
  if (d3d9InitialSamplerStatesActive) {
    for (const field of [
      "nativeSamplerStatePrimaryTraceSha256",
      "nativeSamplerStatePrimarySummarySha256",
      "nativeSamplerStateConfirmationTraceSha256",
      "nativeSamplerStateConfirmationSummarySha256",
      "nativeSamplerStateOracleSourceSha256",
      "nativeSamplerStateOracleObjectSha256",
      "nativeSamplerStateOracleExecutableSha256",
    ]) {
      requireSha256(
        latestEvidence[field],
        `activeRuntimeCheckpoint.latestEvidence.${field}`,
        errors,
      );
    }
    if (
      latestEvidence.nativeSamplerStateProfileId !==
        d3d9NativeSamplerState.profileId ||
      latestEvidence.nativeSamplerStateOracleProfileId !==
        d3d9NativeSamplerState.oracleProfileId ||
      latestEvidence.nativeSamplerStatePrimaryTraceBytes !==
        d3d9NativeSamplerState.primaryTraceBytes ||
      latestEvidence.nativeSamplerStatePrimaryTraceLines !==
        d3d9NativeSamplerState.primaryTraceLines ||
      latestEvidence.nativeSamplerStatePrimaryTraceSha256 !==
        d3d9NativeSamplerState.primaryTraceSha256 ||
      latestEvidence.nativeSamplerStatePrimarySummaryBytes !==
        d3d9NativeSamplerState.primarySummaryBytes ||
      latestEvidence.nativeSamplerStatePrimarySummarySha256 !==
        d3d9NativeSamplerState.primarySummarySha256 ||
      latestEvidence.nativeSamplerStatePrimaryBalancedApiPairs !==
        d3d9NativeSamplerState.primaryBalancedApiPairs ||
      latestEvidence.nativeSamplerStateConfirmationTraceBytes !==
        d3d9NativeSamplerState.confirmationTraceBytes ||
      latestEvidence.nativeSamplerStateConfirmationTraceLines !==
        d3d9NativeSamplerState.confirmationTraceLines ||
      latestEvidence.nativeSamplerStateConfirmationTraceSha256 !==
        d3d9NativeSamplerState.confirmationTraceSha256 ||
      latestEvidence.nativeSamplerStateConfirmationSummaryBytes !==
        d3d9NativeSamplerState.confirmationSummaryBytes ||
      latestEvidence.nativeSamplerStateConfirmationSummarySha256 !==
        d3d9NativeSamplerState.confirmationSummarySha256 ||
      latestEvidence.nativeSamplerStateConfirmationBalancedApiPairs !==
        d3d9NativeSamplerState.confirmationBalancedApiPairs ||
      latestEvidence.nativeSamplerStateTraceDiagnostics !==
        d3d9NativeSamplerState.diagnostics ||
      JSON.stringify(latestEvidence.nativeSamplerStateCallerRvas) !==
        JSON.stringify(d3d9NativeSamplerState.callerRvas) ||
      JSON.stringify(latestEvidence.nativeSamplerStateArguments) !==
        JSON.stringify(d3d9NativeSamplerState.arguments) ||
      JSON.stringify(latestEvidence.nativeSamplerStateResults) !==
        JSON.stringify(d3d9NativeSamplerState.results) ||
      JSON.stringify(latestEvidence.nativeSamplerStateLastErrors) !==
        JSON.stringify(d3d9NativeSamplerState.lastErrors) ||
      latestEvidence.nativeSamplerStateImmediateConsumer !==
        d3d9NativeSamplerState.immediateConsumer ||
      latestEvidence.nativeSamplerStateOracleArchitecture !==
        d3d9NativeSamplerState.oracleArchitecture ||
      latestEvidence.nativeSamplerStateOracleSourceBytes !==
        d3d9NativeSamplerState.oracleSourceBytes ||
      latestEvidence.nativeSamplerStateOracleSourceSha256 !==
        d3d9NativeSamplerState.oracleSourceSha256 ||
      latestEvidence.nativeSamplerStateOracleObjectBytes !==
        d3d9NativeSamplerState.oracleObjectBytes ||
      latestEvidence.nativeSamplerStateOracleObjectSha256 !==
        d3d9NativeSamplerState.oracleObjectSha256 ||
      latestEvidence.nativeSamplerStateOracleExecutableBytes !==
        d3d9NativeSamplerState.oracleExecutableBytes ||
      latestEvidence.nativeSamplerStateOracleExecutableSha256 !==
        d3d9NativeSamplerState.oracleExecutableSha256 ||
      latestEvidence.nativeSamplerStateOracleRuns !==
        d3d9NativeSamplerState.oracleRuns ||
      JSON.stringify(latestEvidence.nativeSamplerStateOracleExitCodes) !==
        JSON.stringify(d3d9NativeSamplerState.oracleExitCodes) ||
      latestEvidence.nativeSamplerStateOracleLastErrorSentinel !==
        d3d9NativeSamplerState.oracleLastErrorSentinel ||
      latestEvidence.nativeSamplerStateOracleLastErrorPreserved !== true ||
      latestEvidence.nativeSamplerStateOracleInitialMinFilter !== 1 ||
      latestEvidence.nativeSamplerStateOracleMinFilterAfterLinear !== 2 ||
      latestEvidence.nativeSamplerStateOracleMagFilterAfterLinear !== 2 ||
      latestEvidence.nativeSamplerStateOracleRepeatedMinFilterResult !==
        "0x00000000" ||
      latestEvidence.nativeSamplerStateOracleInvalidSamplerResult !==
        "0x00000000" ||
      latestEvidence.nativeSamplerStateOracleInvalidStateResult !==
        "0x00000000" ||
      latestEvidence.nativeSamplerStateOracleInvalidFilterResult !==
        "0x00000000" ||
      JSON.stringify(
        latestEvidence.nativeSamplerStateOracleDeviceReferenceCounts,
      ) !== JSON.stringify(d3d9NativeSamplerState.oracleDeviceReferenceCounts) ||
      latestEvidence.nativeSamplerStateRuntimePolicyNarrowsNativePermissiveness !==
        true ||
      latestEvidence.nativeSamplerStateExecutableHashUnchanged !== true ||
      latestEvidence.d3d9SamplerStateImplemented !== true ||
      JSON.stringify(latestEvidence.d3d9SamplerStateSequences) !==
        JSON.stringify(d3d9PrivateSamplerState.nodeSequences) ||
      JSON.stringify(latestEvidence.d3d9SamplerStateInstructionCounts) !==
        JSON.stringify(d3d9PrivateSamplerState.nodeInstructionCounts) ||
      JSON.stringify(latestEvidence.d3d9SamplerStateArguments) !==
        JSON.stringify(d3d9PrivateSamplerState.arguments) ||
      JSON.stringify(latestEvidence.d3d9SamplerStateReturnAddresses) !==
        JSON.stringify(d3d9PrivateSamplerState.returnAddresses) ||
      JSON.stringify(latestEvidence.d3d9SamplerStateStackPointers) !==
        JSON.stringify(d3d9PrivateSamplerState.stackPointers) ||
      JSON.stringify(latestEvidence.d3d9SamplerStateResults) !==
        JSON.stringify(d3d9PrivateSamplerState.results) ||
      latestEvidence.d3d9SamplerStatePolicyVersion !== 22 ||
      latestEvidence.d3d9SamplerStatePolicyId !==
        "d3d9-primary-virtual-adapter-v22" ||
      latestEvidence.d3d9SamplerStateProfileId !==
        "primary-first-frame-linear-sampler-states-v1" ||
      latestEvidence.d3d9SamplerStateAuthority !==
        "rust-runtime-graphics-logical-device-sampler-states" ||
      latestEvidence.d3d9SamplerStateMutation !==
        "validate-plan-preflight-targeted-idempotent-commit" ||
      latestEvidence.d3d9SamplerStateLogicalGraphicsDeviceId !== 3 ||
      latestEvidence.d3d9SamplerStateLogicalGraphicsDeviceNamespace !==
        "vine.logical-graphics-device.v1" ||
      latestEvidence.d3d9SamplerStateLogicalGraphicsComObjectId !== "21" ||
      JSON.stringify(latestEvidence.d3d9SamplerStateFiltersAfterCalls) !==
        JSON.stringify(d3d9PrivateSamplerState.filtersAfterCalls) ||
      JSON.stringify(
        latestEvidence.d3d9SamplerStateLogicalGraphicsDeviceGenerations,
      ) !==
        JSON.stringify(
          d3d9PrivateSamplerState.logicalGraphicsDeviceGenerations,
        ) ||
      latestEvidence.d3d9SamplerStateSameValueGenerationChange !== 0 ||
      latestEvidence.d3d9SamplerStateUnsupportedVariantResult !==
        "0x8876086a" ||
      latestEvidence.d3d9SamplerStateFailureAtomicity !== true ||
      latestEvidence.d3d9SamplerStateBrowserOwnsGraphicsState !== false ||
      latestEvidence.d3d9SamplerStateBrowserRendererResourceCreated !== false ||
      latestEvidence.d3d9SamplerStateNodeCompletions !== 2 ||
      latestEvidence.d3d9SamplerStateChromeCompletions !== 2 ||
      JSON.stringify(
        latestEvidence.d3d9PostSamplerRenderStateRejectedSequences,
      ) !==
        JSON.stringify(
          d3d9PrivateSamplerState.postSamplerRenderStateRejectedSequences,
        ) ||
      JSON.stringify(
        latestEvidence.d3d9PostSamplerRenderStateRejectedInstructionCounts,
      ) !==
        JSON.stringify(
          d3d9PrivateSamplerState.postSamplerRenderStateRejectedInstructionCounts,
        ) ||
      latestEvidence.d3d9PostSamplerRenderStateCallerIgnoredResults !== true ||
      latestEvidence.d3d9SamplerStateNodeChromeOrderedSemanticsMatched !== true
    ) {
      errors.push(
        "latest native and private D3D9 sampler-state evidence is invalid",
      );
    }
  }
  requireInteger(
    latestEvidence.finalMemoryGeneration,
    "activeRuntimeCheckpoint.latestEvidence.finalMemoryGeneration",
    errors,
  );
  requireSha256(
    latestEvidence.reportSha256,
    "activeRuntimeCheckpoint.latestEvidence.reportSha256",
    errors,
  );
  requireString(
    active.canonicalReproductionCommand,
    "activeRuntimeCheckpoint.canonicalReproductionCommand",
    errors,
  );
  const currentStop = root.nextStructuredStop ?? {};
  const currentStopEndpoint =
    currentStop.kind === "capability-checkpoint"
      ? currentStop.nextEndpoint?.api
      : currentStop.kind === "com-method"
        ? `${currentStop.interface}::${currentStop.method}`
        : currentStop.api;
  if (
    active.canonicalReproductionCommand !==
      "npm run check:runtime:bgi-browser" ||
    bgiBrowserEvidence.supportedCalls !==
      active.genericDispatcherCallOccurrenceCount ||
    bgiBrowserEvidence.guestInstructions !== currentStop.instructionCount ||
    bgiBrowserEvidence.nodeFinalGeneration !==
      latestEvidence.finalMemoryGeneration ||
    bgiBrowserEvidence.memoryContentSha256 !==
      latestEvidence.memoryContentSha256 ||
    bgiBrowserEvidence.nodeOracleReportBytes !== latestEvidence.reportBytes ||
    bgiBrowserEvidence.nodeOracleReportSha256 !== latestEvidence.reportSha256 ||
    (currentStop.kind === "capability-checkpoint"
      ? bgiBrowserEvidence.checkpointCompletion?.memoryContentSha256 !==
          latestEvidence.memoryContentSha256 ||
        latestEvidence.capabilityCheckpointCompletion?.memoryContentSha256 !==
          latestEvidence.memoryContentSha256
      : bgiBrowserEvidence.nextCapabilityCluster !==
          currentStop.capabilityCluster ||
        bgiBrowserEvidence.nextEndpoint?.toLowerCase() !==
          currentStopEndpoint?.toLowerCase() ||
        bgiBrowserEvidence.nextFaultCode !== currentStop.reason ||
        bgiBrowserEvidence.nextEndpointAddress !==
          currentStop.endpointAddress ||
        bgiBrowserEvidence.nextArguments?.join(",") !==
          currentStop.arguments?.join(",") ||
        latestEvidence.firstStructuredStopInstructionCount !==
          currentStop.instructionCount ||
        latestEvidence.firstStructuredStopCapabilityCluster !==
          currentStop.capabilityCluster ||
        latestEvidence.firstStructuredStopEndpoint?.toLowerCase() !==
          currentStopEndpoint?.toLowerCase() ||
        latestEvidence.firstStructuredStopEndpointKind !==
          currentStop.endpointKind ||
        latestEvidence.firstStructuredStopEndpointAddress !==
          currentStop.endpointAddress ||
        latestEvidence.firstStructuredStopArguments?.join(",") !==
          currentStop.arguments?.join(",") ||
        latestEvidence.firstStructuredStopReturnAddress !==
          currentStop.returnAddress ||
        latestEvidence.firstStructuredStopFault !== currentStop.reason)
  ) {
    errors.push(
      "browser BGI evidence must match the active runtime checkpoint",
    );
  }
  if (currentStop.kind === "capability-checkpoint") {
    const expectedCompletion = currentStop.completion ?? {};
    const expectedEndpoint = currentStop.nextEndpoint ?? {};
    for (const [label, actual] of [
      ["browser BGI", bgiBrowserEvidence.checkpointCompletion ?? {}],
      ["latest", latestEvidence.capabilityCheckpointCompletion ?? {}],
    ]) {
      if (
        actual.capability !== expectedCompletion.capability ||
        actual.implementationId !== expectedCompletion.implementationId ||
        actual.requiredCompletions !== expectedCompletion.requiredCompletions ||
        actual.completionCount !== expectedCompletion.completionCount ||
        actual.lastCompletionSequence !==
          expectedCompletion.lastCompletionSequence ||
        actual.lastCompletionInstructionCount !==
          expectedCompletion.lastCompletionInstructionCount ||
        actual.instructionCount !== currentStop.instructionCount ||
        actual.nextEndpoint?.endpointId !== expectedEndpoint.endpointId ||
        actual.nextEndpoint?.endpointAddress !==
          expectedEndpoint.endpointAddress ||
        actual.nextEndpoint?.api?.toLowerCase() !==
          expectedEndpoint.api?.toLowerCase()
      ) {
        errors.push(
          `${label} capability checkpoint must match nextStructuredStop`,
        );
      }
      if (
        expectedCompletion.roundtrips !== undefined &&
        JSON.stringify(actual.roundtrips) !==
          JSON.stringify(expectedCompletion.roundtrips)
      ) {
        errors.push(
          `${label} critical-section roundtrips must match nextStructuredStop`,
        );
      }
      if (
        expectedCompletion.threadSelections !== undefined &&
        JSON.stringify(actual.threadSelections) !==
          JSON.stringify(expectedCompletion.threadSelections)
      ) {
        errors.push(
          `${label} scheduler selections must match nextStructuredStop`,
        );
      }
      for (const field of [
        "criticalSectionAddress",
        "releaseThreadId",
        "acquisitionThreadId",
        "lastError",
      ]) {
        if (
          expectedCompletion[field] !== undefined &&
          actual[field] !== expectedCompletion[field]
        ) {
          errors.push(
            `${label} critical-section checkpoint must match nextStructuredStop`,
          );
        }
      }
    }
  }
  if (active.profile === "kernel32-enter-critical-section") {
    const expectedEnterSequences = [18_699, 18_701, 18_703];
    const expectedEnterOccurrences = [32, 33, 34];
    const expectedEnterInstructions = [11_973_878, 11_973_919, 11_973_950];
    const expectedEnterAddresses = ["0x1014c8d0", "0x1014c8d0", "0x100ff820"];
    const expectedEnterReturns = ["0x10091796", "0x10091796", "0x100010db"];
    const expectedLeaveSequences = [18_700, 18_702, 18_704];
    const expectedLeaveOccurrences = [31, 32, 33];
    const expectedLeaveInstructions = [11_973_883, 11_973_924, 11_973_959];
    const expectedLeaveAddresses = ["0x1014c8d0", "0x1014c8d0", "0x100ff820"];
    const expectedLeaveReturns = ["0x100917aa", "0x100917aa", "0x100010eb"];
    const criticalEvidenceValid =
      latestEvidence.enterCriticalSectionCheckpointCompletions === 3 &&
      JSON.stringify(latestEvidence.enterCriticalSectionSequences) ===
        JSON.stringify(expectedEnterSequences) &&
      JSON.stringify(latestEvidence.enterCriticalSectionOccurrences) ===
        JSON.stringify(expectedEnterOccurrences) &&
      JSON.stringify(latestEvidence.enterCriticalSectionInstructionCounts) ===
        JSON.stringify(expectedEnterInstructions) &&
      JSON.stringify(latestEvidence.enterCriticalSectionAddresses) ===
        JSON.stringify(expectedEnterAddresses) &&
      JSON.stringify(latestEvidence.enterCriticalSectionReturnAddresses) ===
        JSON.stringify(expectedEnterReturns) &&
      JSON.stringify(latestEvidence.enterCriticalSectionLastErrors) ===
        JSON.stringify([0, 0, 0]) &&
      latestEvidence.enterCriticalSectionGuestWriteBytes === 24 &&
      latestEvidence.enterCriticalSectionLockedHex ===
        "fffffffffeffffff010000000120000000000000d0070002" &&
      latestEvidence.leaveCriticalSectionCheckpointCompletions === 3 &&
      JSON.stringify(latestEvidence.leaveCriticalSectionSequences) ===
        JSON.stringify(expectedLeaveSequences) &&
      JSON.stringify(latestEvidence.leaveCriticalSectionOccurrences) ===
        JSON.stringify(expectedLeaveOccurrences) &&
      JSON.stringify(latestEvidence.leaveCriticalSectionInstructionCounts) ===
        JSON.stringify(expectedLeaveInstructions) &&
      JSON.stringify(latestEvidence.leaveCriticalSectionAddresses) ===
        JSON.stringify(expectedLeaveAddresses) &&
      JSON.stringify(latestEvidence.leaveCriticalSectionReturnAddresses) ===
        JSON.stringify(expectedLeaveReturns) &&
      JSON.stringify(latestEvidence.leaveCriticalSectionLastErrors) ===
        JSON.stringify([0, 0, 0]) &&
      latestEvidence.leaveCriticalSectionGuestWriteBytes === 24 &&
      latestEvidence.leaveCriticalSectionUnlockedHex ===
        "ffffffffffffffff000000000000000000000000d0070002" &&
      latestEvidence.criticalSectionCheckpointOwnerThreadId === 8_193 &&
      latestEvidence.criticalSectionCheckpointStateAuthority ===
        "runtime/synchronization.rs::RuntimeSynchronization" &&
      latestEvidence.criticalSectionCheckpointBrowserOwnsWindowsState ===
        false &&
      latestEvidence.criticalSectionCheckpointNodeChromeOrderedSemanticsMatched ===
        true &&
      bgiBrowserEvidence.enterCriticalSectionCheckpointCompletions === 3 &&
      JSON.stringify(bgiBrowserEvidence.enterCriticalSectionSequences) ===
        JSON.stringify(expectedEnterSequences) &&
      JSON.stringify(bgiBrowserEvidence.enterCriticalSectionOccurrences) ===
        JSON.stringify(expectedEnterOccurrences) &&
      JSON.stringify(
        bgiBrowserEvidence.enterCriticalSectionInstructionCounts,
      ) === JSON.stringify(expectedEnterInstructions) &&
      JSON.stringify(bgiBrowserEvidence.enterCriticalSectionAddresses) ===
        JSON.stringify(expectedEnterAddresses) &&
      JSON.stringify(bgiBrowserEvidence.enterCriticalSectionReturnAddresses) ===
        JSON.stringify(expectedEnterReturns) &&
      bgiBrowserEvidence.enterCriticalSectionGuestWriteBytes === 24 &&
      bgiBrowserEvidence.enterCriticalSectionLockedHex ===
        "fffffffffeffffff010000000120000000000000d0070002" &&
      bgiBrowserEvidence.leaveCriticalSectionCheckpointCompletions === 3 &&
      JSON.stringify(bgiBrowserEvidence.leaveCriticalSectionSequences) ===
        JSON.stringify(expectedLeaveSequences) &&
      JSON.stringify(bgiBrowserEvidence.leaveCriticalSectionOccurrences) ===
        JSON.stringify(expectedLeaveOccurrences) &&
      JSON.stringify(
        bgiBrowserEvidence.leaveCriticalSectionInstructionCounts,
      ) === JSON.stringify(expectedLeaveInstructions) &&
      JSON.stringify(bgiBrowserEvidence.leaveCriticalSectionAddresses) ===
        JSON.stringify(expectedLeaveAddresses) &&
      JSON.stringify(bgiBrowserEvidence.leaveCriticalSectionReturnAddresses) ===
        JSON.stringify(expectedLeaveReturns) &&
      bgiBrowserEvidence.leaveCriticalSectionGuestWriteBytes === 24 &&
      bgiBrowserEvidence.leaveCriticalSectionUnlockedHex ===
        "ffffffffffffffff000000000000000000000000d0070002" &&
      bgiBrowserEvidence.criticalSectionCheckpointOwnerThreadId === 8_193 &&
      bgiBrowserEvidence.criticalSectionCheckpointStateAuthority ===
        "runtime/synchronization.rs::RuntimeSynchronization" &&
      bgiBrowserEvidence.criticalSectionCheckpointBrowserOwnsWindowsState ===
        false &&
      bgiBrowserEvidence.criticalSectionCheckpointNodeChromeOrderedSemanticsMatched ===
        true;
    if (!criticalEvidenceValid) {
      errors.push("critical-section checkpoint evidence is invalid");
    }
  } else if (active.profile === "primary-main-loop-progression") {
    const expectedThreadSelections = [
      {
        sequence: 18_663,
        instructionCount: 11_973_280,
        previousThreadId: 8_192,
        threadId: 8_193,
        state: { state: "runnable" },
      },
      {
        sequence: 18_734,
        instructionCount: 11_974_512,
        previousThreadId: 8_193,
        threadId: 8_192,
        state: { state: "runnable" },
      },
    ];
    for (const [label, completion] of [
      ["browser BGI", bgiBrowserEvidence.checkpointCompletion ?? {}],
      ["latest", latestEvidence.capabilityCheckpointCompletion ?? {}],
    ]) {
      if (
        completion.capability !== "primary-main-loop-progression" ||
        completion.implementationId !==
          "runtime.scheduler.api-boundary-round-robin.v1" ||
        completion.requiredCompletions !== 2 ||
        completion.completionCount !== 2 ||
        completion.lastCompletionSequence !== 18_734 ||
        completion.lastCompletionInstructionCount !== 11_974_512 ||
        JSON.stringify(completion.threadSelections) !==
          JSON.stringify(expectedThreadSelections) ||
        completion.instructionCount !== 11_974_687 ||
        completion.memoryContentSha256 !==
          "ca7efd6774fe36acfa3ee61621e12b2b6fccbaf4327cc026f323f3aa0859fc2b"
      ) {
        errors.push(
          `${label} main-loop scheduler checkpoint evidence is invalid`,
        );
      }
    }
  } else if (active.profile === "kernel32-leave-critical-section") {
    for (const [label, completion] of [
      ["browser BGI", bgiBrowserEvidence.checkpointCompletion ?? {}],
      ["latest", latestEvidence.capabilityCheckpointCompletion ?? {}],
    ]) {
      if (
        completion.capability !== "kernel32-leave-critical-section" ||
        completion.implementationId !== "kernel32.leave-critical-section.v1" ||
        completion.requiredCompletions !== 1 ||
        completion.completionCount !== 1 ||
        completion.lastCompletionSequence !== 18_735 ||
        completion.lastCompletionInstructionCount !== 11_974_687 ||
        completion.criticalSectionAddress !== 0x100fd9cc ||
        completion.releaseThreadId !== 8_192 ||
        completion.lastError !== 0 ||
        completion.instructionCount !== 11_974_730 ||
        completion.memoryContentSha256 !==
          "b326ab911f02986bbbac818825b0b67171299bc946495eb52f5a0be6fb7e8636"
      ) {
        errors.push(
          `${label} critical-section release checkpoint evidence is invalid`,
        );
      }
    }
  } else if (
    active.profile === "kernel32-enter-critical-section-continuation"
  ) {
    for (const [label, completion] of [
      ["browser BGI", bgiBrowserEvidence.checkpointCompletion ?? {}],
      ["latest", latestEvidence.capabilityCheckpointCompletion ?? {}],
    ]) {
      if (
        completion.capability !==
          "kernel32-enter-critical-section-continuation" ||
        completion.implementationId !== "kernel32.enter-critical-section.v1" ||
        completion.requiredCompletions !== 1 ||
        completion.completionCount !== 1 ||
        completion.lastCompletionSequence !== 18_736 ||
        completion.lastCompletionInstructionCount !== 11_974_730 ||
        completion.criticalSectionAddress !== 0x100fd9cc ||
        completion.acquisitionThreadId !== 8_192 ||
        completion.lastError !== 0 ||
        completion.instructionCount !== 11_974_905 ||
        completion.memoryContentSha256 !==
          "12de5674ab03760d39da41c1eb5aa2cea0c242a98387518aaeb52fc22e4e60df"
      ) {
        errors.push(
          `${label} critical-section continuation checkpoint evidence is invalid`,
        );
      }
    }
  } else if (
    active.profile === "kernel32-leave-critical-section-continuation"
  ) {
    for (const [label, completion] of [
      ["browser BGI", bgiBrowserEvidence.checkpointCompletion ?? {}],
      ["latest", latestEvidence.capabilityCheckpointCompletion ?? {}],
    ]) {
      if (
        completion.capability !==
          "kernel32-leave-critical-section-continuation" ||
        completion.implementationId !== "kernel32.leave-critical-section.v1" ||
        completion.requiredCompletions !== 1 ||
        completion.completionCount !== 1 ||
        completion.lastCompletionSequence !== 18_737 ||
        completion.lastCompletionInstructionCount !== 11_974_905 ||
        completion.criticalSectionAddress !== 0x100fd9cc ||
        completion.releaseThreadId !== 8_192 ||
        completion.lastError !== 0 ||
        completion.instructionCount !== 11_974_948 ||
        completion.memoryContentSha256 !==
          "9473c7af4f1c00a6bf8f2f5db611cb7a5253d04387297b9e0244d81f032315bc"
      ) {
        errors.push(
          `${label} critical-section continuation release evidence is invalid`,
        );
      }
    }
  }
  if (
    currentStop.kind === "api-endpoint" &&
    (latestEvidence.firstStructuredStopEndpointId !== currentStop.endpointId ||
      latestEvidence.firstStructuredStopIatAddress !== currentStop.iatAddress)
  ) {
    errors.push(
      "browser BGI API stop identity must match the active runtime checkpoint",
    );
  }
  if (
    archiveSourceReadActive &&
    (latestEvidence.archiveSourceSnapshotId !== "primary-boot-archives-v1" ||
      latestEvidence.archiveSourceManifest !==
        "fixtures/source-snapshots/primary-boot-archives-v1.json" ||
      latestEvidence.archiveSourceObjects !== 3 ||
      latestEvidence.archiveSourceLooseProbeSequence !== 89_259 ||
      latestEvidence.archiveSourceLooseProbeResult !== "0xffffffff" ||
      latestEvidence.archiveSourceLooseProbeLastError !== 2 ||
      latestEvidence.archiveSourceOpenSequences?.join(",") !== "89270,89283" ||
      latestEvidence.archiveSourceReadSequences?.join(",") !==
        "89271,89273,89285" ||
      latestEvidence.archiveSourceReadOffsets?.join(",") !== "0,16,400" ||
      latestEvidence.archiveSourceReadBytes?.join(",") !== "16,384,2328" ||
      latestEvidence.archiveSourceMemberSeekSequence !== 89_284 ||
      latestEvidence.archiveSourceCloseSequences?.join(",") !== "89274,89286" ||
      latestEvidence.archiveSourceWriteDenied !== true ||
      latestEvidence.archiveSourceProfileShadowing !== false ||
      latestEvidence.archiveFormatParsedByRuntime !== false ||
      bgiBrowserEvidence.archiveSourceReadCompletions !== 3 ||
      bgiBrowserEvidence.archiveSourceSnapshotId !==
        "primary-boot-archives-v1" ||
      bgiBrowserEvidence.archiveSourceReadOffsets?.join(",") !== "0,16,400" ||
      bgiBrowserEvidence.archiveSourceReadBytes?.join(",") !== "16,384,2328" ||
      bgiBrowserEvidence.archiveSourceHostWaitExecutions !== 0 ||
      bgiBrowserEvidence.synchronousHostCompletionOrder !==
        "request-issue-order")
  ) {
    errors.push("primary boot-archive source read evidence is invalid");
  }
  if (
    sleepZeroActive &&
    (latestEvidence.nativeSleepZeroOracleProfile !==
      "x86-sleep-zero-ready-competitor-v1" ||
      latestEvidence.nativeSleepZeroOracleArchitecture !== "coff-i386" ||
      latestEvidence.nativeSleepZeroOracleSourceBytes !== 6_481 ||
      latestEvidence.nativeSleepZeroOracleSourceSha256 !==
        "95ce4364c0428c1559679c753b0b1501b4977078d4f7b02c7ba900ca89479332" ||
      latestEvidence.nativeSleepZeroOracleObjectBytes !== 19_723 ||
      latestEvidence.nativeSleepZeroOracleObjectSha256 !==
        "ebd64a7811cad7100b5f5ba6485c82cdf3d8f2e788afee385ff3b6e293d7f260" ||
      latestEvidence.nativeSleepZeroOracleExecutableBytes !== 6_144 ||
      latestEvidence.nativeSleepZeroOracleExecutableSha256 !==
        "1168739db86861c6fb75179a606b0e60429cd301adda8cf523ee8c697741da11" ||
      latestEvidence.nativeSleepZeroOracleRuns !== 2 ||
      latestEvidence.nativeSleepZeroOracleSingleCpuPinned !== true ||
      latestEvidence.nativeSleepZeroOracleReadyCompetitor !== true ||
      latestEvidence.nativeSleepZeroOracleSamplesPerRun !== 8 ||
      latestEvidence.nativeSleepZeroOracleAdvancedSamplesPerRun !== 8 ||
      latestEvidence.nativeSleepZeroOraclePreservedLastErrorSamplesPerRun !==
        8 ||
      latestEvidence.nativeSleepZeroOracleWaitResult !== 0 ||
      latestEvidence.nativeSleepZeroOracleExitCodes?.join(",") !== "0,0" ||
      latestEvidence.nativeSleepZeroPrimaryOuterLoopEntryRva !== "0x0006a263" ||
      latestEvidence.nativeSleepZeroPrimaryCallerReturnRva !== "0x0006a2f5" ||
      latestEvidence.nativeSleepZeroPrimaryArgument !== 0 ||
      latestEvidence.nativeSleepZeroPrimaryConsumer !==
        "repeat-raster-status-read-and-yield-current-thread" ||
      latestEvidence.nativeSleepZeroPrimaryAndConfirmationAgree !== true ||
      latestEvidence.nativeSleepZeroExecutableHashUnchanged !== true ||
      latestEvidence.sleepZeroImplemented !== true ||
      latestEvidence.sleepZeroPolicyId !==
        (createDirectoryAActive
          ? "kernel32-create-directory-a-v1"
          : getFileAttributesAActive
            ? "kernel32-get-file-attributes-a-v1"
            : globalMemoryStatusActive
              ? "kernel32-global-memory-status-v1"
              : "kernel32-sleep-zero-v1") ||
      latestEvidence.sleepZeroPolicySource !==
        (createDirectoryAActive
          ? "fixtures/runtime-policies/kernel32-create-directory-a-v1.json"
          : getFileAttributesAActive
            ? "fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json"
            : globalMemoryStatusActive
              ? "fixtures/runtime-policies/kernel32-global-memory-status-v1.json"
              : "fixtures/runtime-policies/kernel32-sleep-zero-v1.json") ||
      latestEvidence.sleepZeroCallCount !== sleepZeroCallCount ||
      latestEvidence.sleepZeroSequences?.join(",") !== sleepZeroSequences ||
      latestEvidence.sleepZeroOccurrences?.join(",") !== sleepZeroOccurrences ||
      latestEvidence.sleepZeroInstructionCounts?.join(",") !==
        sleepZeroInstructionCounts ||
      latestEvidence.sleepZeroArguments?.join(",") !== "0x00000000" ||
      latestEvidence.sleepZeroResult !== "void-eax-preserved" ||
      latestEvidence.sleepZeroReturnAddress !== "0x1006a2f5" ||
      latestEvidence.sleepZeroStackPointers?.join(",") !==
        "0x006ff578,0x006ff580" ||
      latestEvidence.sleepZeroLastErrors?.join(",") !== "0,0" ||
      latestEvidence.sleepZeroGuestWriteBytes !== 0 ||
      latestEvidence.sleepZeroImplementationId !==
        "kernel32.sleep-zero.yield-current-thread.v1" ||
      latestEvidence.sleepZeroExecutionBoundary !== "yield-current-thread" ||
      latestEvidence.sleepZeroSchedulerAuthority !==
        "runtime/scheduler.rs::RuntimeScheduler" ||
      latestEvidence.sleepZeroSchedulerSelectionCount !== sleepZeroCallCount ||
      latestEvidence.sleepZeroSchedulerSelectionSequences?.join(",") !==
        sleepZeroSequences ||
      latestEvidence.sleepZeroSchedulerSelectionInstructionCounts?.join(",") !==
        sleepZeroInstructionCounts ||
      latestEvidence.sleepZeroSchedulerPreviousThreadId !== 8_192 ||
      latestEvidence.sleepZeroSchedulerSelectedThreadId !== 8_193 ||
      latestEvidence.sleepZeroNoCompetitorCompletions !== 0 ||
      latestEvidence.sleepZeroHostWaitCreated !== false ||
      latestEvidence.sleepZeroNonzeroRejectedBeforeMutation !== true ||
      latestEvidence.sleepZeroFailureAtomicity !== true ||
      latestEvidence.sleepZeroNodeCompletions !== sleepZeroCallCount ||
      latestEvidence.sleepZeroChromeCompletions !== sleepZeroCallCount ||
      latestEvidence.sleepZeroNodeChromeOrderedSemanticsMatched !== true ||
      bgiBrowserEvidence.sleepZeroCompletions !== sleepZeroCallCount ||
      bgiBrowserEvidence.sleepZeroSequences?.join(",") !== sleepZeroSequences ||
      bgiBrowserEvidence.sleepZeroOccurrences?.join(",") !==
        sleepZeroOccurrences ||
      bgiBrowserEvidence.sleepZeroInstructionCounts?.join(",") !==
        sleepZeroInstructionCounts ||
      bgiBrowserEvidence.sleepZeroArguments?.join(",") !== "0x00000000" ||
      bgiBrowserEvidence.sleepZeroResult !== "void-eax-preserved" ||
      bgiBrowserEvidence.sleepZeroReturnAddress !== "0x1006a2f5" ||
      bgiBrowserEvidence.sleepZeroStackPointers?.join(",") !==
        "0x006ff578,0x006ff580" ||
      bgiBrowserEvidence.sleepZeroLastErrors?.join(",") !== "0,0" ||
      bgiBrowserEvidence.sleepZeroGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.sleepZeroImplementationId !==
        "kernel32.sleep-zero.yield-current-thread.v1" ||
      bgiBrowserEvidence.sleepZeroExecutionBoundary !==
        "yield-current-thread" ||
      bgiBrowserEvidence.sleepZeroSchedulerAuthority !==
        "rust-runtime-scheduler" ||
      bgiBrowserEvidence.sleepZeroSchedulerSelectionCount !==
        sleepZeroCallCount ||
      bgiBrowserEvidence.sleepZeroSchedulerSelectionSequences?.join(",") !==
        sleepZeroSequences ||
      bgiBrowserEvidence.sleepZeroSchedulerSelectionInstructionCounts?.join(
        ",",
      ) !== sleepZeroInstructionCounts ||
      bgiBrowserEvidence.sleepZeroSchedulerPreviousThreadId !== 8_192 ||
      bgiBrowserEvidence.sleepZeroSchedulerSelectedThreadId !== 8_193 ||
      bgiBrowserEvidence.sleepZeroNoCompetitorCompletions !== 0 ||
      bgiBrowserEvidence.sleepZeroHostWaitCreated !== false ||
      bgiBrowserEvidence.sleepZeroNodeChromeOrderedSemanticsMatched !== true)
  ) {
    errors.push("Sleep(0) native, Rust, Node, and Chrome evidence is invalid");
  }
  if (
    globalMemoryStatusActive &&
    (latestEvidence.nativeGlobalMemoryStatusPrimaryTraceBytes !== 2_262_152 ||
      latestEvidence.nativeGlobalMemoryStatusPrimaryTraceSha256 !==
        "c36a34fc2d72c1ced0f2b65594b79587ab4a924a8719049f69be3f8d9573ca5d" ||
      latestEvidence.nativeGlobalMemoryStatusPrimarySummaryBytes !== 52_538 ||
      latestEvidence.nativeGlobalMemoryStatusPrimarySummarySha256 !==
        "3ecda278dbe809dcf0b4f57eab78455f6ba71dfc19dc31071cc6703c187e8191" ||
      latestEvidence.nativeGlobalMemoryStatusConfirmationTraceBytes !==
        2_261_696 ||
      latestEvidence.nativeGlobalMemoryStatusConfirmationTraceSha256 !==
        "a8cbb131fd08cc5378216591afbdee45d21d7624e7f33a2930be64ba024f6ba7" ||
      latestEvidence.nativeGlobalMemoryStatusConfirmationSummaryBytes !==
        52_536 ||
      latestEvidence.nativeGlobalMemoryStatusConfirmationSummarySha256 !==
        "09dc742cd48ff63ac49accb4e42af977e668012fc3423c2b661ecaf194dae2c4" ||
      latestEvidence.nativeGlobalMemoryStatusTraceDiagnostics !== 0 ||
      latestEvidence.nativeGlobalMemoryStatusCallsPerTrace !== 1 ||
      latestEvidence.nativeGlobalMemoryStatusCallerReturnRva !== "0x00081fae" ||
      latestEvidence.nativeGlobalMemoryStatusOutputBytes !== 32 ||
      latestEvidence.nativeGlobalMemoryStatusGuardBytes !== 16 ||
      latestEvidence.nativeGlobalMemoryStatusGuardsPreserved !== true ||
      latestEvidence.nativeGlobalMemoryStatusLastErrorSentinels?.join(",") !==
        "0,87,305419896" ||
      latestEvidence.nativeGlobalMemoryStatusLastErrorPreserved !== true ||
      latestEvidence.nativeGlobalMemoryStatusStableCapacityFields !== true ||
      latestEvidence.nativeGlobalMemoryStatusConsumer !==
        "read-total-and-available-physical-clamp-to-signed-maximum-and-pass-to-initialization-helper" ||
      latestEvidence.nativeGlobalMemoryStatusExecutableHashUnchanged !== true ||
      latestEvidence.globalMemoryStatusImplemented !== true ||
      latestEvidence.globalMemoryStatusPolicyId !==
        (createDirectoryAActive
          ? "kernel32-create-directory-a-v1"
          : getFileAttributesAActive
            ? "kernel32-get-file-attributes-a-v1"
            : "kernel32-global-memory-status-v1") ||
      latestEvidence.globalMemoryStatusPolicySource !==
        (createDirectoryAActive
          ? "fixtures/runtime-policies/kernel32-create-directory-a-v1.json"
          : getFileAttributesAActive
            ? "fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json"
            : "fixtures/runtime-policies/kernel32-global-memory-status-v1.json") ||
      latestEvidence.globalMemoryStatusCallCount !== 1 ||
      latestEvidence.globalMemoryStatusSequence !== 107_757 ||
      latestEvidence.globalMemoryStatusInstructionCount !==
        (createDirectoryAActive ? 16_384_629 : 16_384_108) ||
      latestEvidence.globalMemoryStatusArgument !== "0x006ff674" ||
      latestEvidence.globalMemoryStatusReturnAddress !== "0x10081fae" ||
      latestEvidence.globalMemoryStatusStackPointers?.join(",") !==
        "0x006ff660,0x006ff668" ||
      latestEvidence.globalMemoryStatusResult !== "void-eax-preserved" ||
      latestEvidence.globalMemoryStatusLastError !== 0 ||
      latestEvidence.globalMemoryStatusGuestWriteBytes !== 32 ||
      latestEvidence.globalMemoryStatusOutputHex !==
        "2000000000000000ffffff7fffffff7fffffffffffffffff0000fe7f00000060" ||
      latestEvidence.globalMemoryStatusImplementationId !==
        "kernel32.global-memory-status.x86-deterministic.v1" ||
      latestEvidence.globalMemoryStatusStateAuthority !==
        "Kernel32CompatibilityPolicy.memory_status" ||
      latestEvidence.globalMemoryStatusVirtualSpanAuthority !==
        "Kernel32CompatibilityPolicy.system_info" ||
      latestEvidence.globalMemoryStatusHostMemoryImported !== false ||
      latestEvidence.globalMemoryStatusBrowserMemoryImported !== false ||
      latestEvidence.globalMemoryStatusAllocatorQuotaAuthority !== false ||
      latestEvidence.globalMemoryStatusNullRejectedBeforeMutation !== true ||
      latestEvidence.globalMemoryStatusCrossPageRejectedBeforeMutation !==
        true ||
      latestEvidence.globalMemoryStatusReadOnlyRejectedBeforeMutation !==
        true ||
      latestEvidence.globalMemoryStatusFailureAtomicity !== true ||
      latestEvidence.globalMemoryStatusNodeCompletions !== 1 ||
      latestEvidence.globalMemoryStatusChromeCompletions !== 1 ||
      latestEvidence.globalMemoryStatusNodeChromeOrderedSemanticsMatched !==
        true ||
      bgiBrowserEvidence.globalMemoryStatusCompletions !== 1 ||
      bgiBrowserEvidence.globalMemoryStatusSequence !== 107_757 ||
      bgiBrowserEvidence.globalMemoryStatusInstructionCount !==
        (createDirectoryAActive ? 16_384_629 : 16_384_108) ||
      bgiBrowserEvidence.globalMemoryStatusArgument !== "0x006ff674" ||
      bgiBrowserEvidence.globalMemoryStatusReturnAddress !== "0x10081fae" ||
      bgiBrowserEvidence.globalMemoryStatusStackPointers?.join(",") !==
        "0x006ff660,0x006ff668" ||
      bgiBrowserEvidence.globalMemoryStatusResult !== "void-eax-preserved" ||
      bgiBrowserEvidence.globalMemoryStatusLastError !== 0 ||
      bgiBrowserEvidence.globalMemoryStatusGuestWriteBytes !== 32 ||
      bgiBrowserEvidence.globalMemoryStatusOutputHex !==
        "2000000000000000ffffff7fffffff7fffffffffffffffff0000fe7f00000060" ||
      bgiBrowserEvidence.globalMemoryStatusImplementationId !==
        "kernel32.global-memory-status.x86-deterministic.v1" ||
      bgiBrowserEvidence.globalMemoryStatusStateAuthority !==
        "kernel32-compatibility-policy" ||
      bgiBrowserEvidence.globalMemoryStatusVirtualSpanAuthority !==
        "kernel32-system-info-policy" ||
      bgiBrowserEvidence.globalMemoryStatusHostMemoryImported !== false ||
      bgiBrowserEvidence.globalMemoryStatusBrowserMemoryImported !== false ||
      bgiBrowserEvidence.globalMemoryStatusAllocatorQuotaAuthority !== false ||
      bgiBrowserEvidence.globalMemoryStatusNodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push(
      "GlobalMemoryStatus native, policy, Rust, Node, and Chrome evidence is invalid",
    );
  }
  if (
    getFileAttributesAActive &&
    (latestEvidence.nativeGetFileAttributesAPrimaryTraceBytes !== 2_389_154 ||
      latestEvidence.nativeGetFileAttributesAPrimaryTraceSha256 !==
        "14fa7ccf73bbb30840ca3f794da4e163cecc7c32c3ca17a4485de5973b76769e" ||
      latestEvidence.nativeGetFileAttributesAPrimarySummaryBytes !== 53_988 ||
      latestEvidence.nativeGetFileAttributesAPrimarySummarySha256 !==
        "6539b3a90e9ee256e822c88744b0eabfbf48be3e609872e5f5e0530d97e7977d" ||
      latestEvidence.nativeGetFileAttributesAConfirmationTraceBytes !==
        2_374_144 ||
      latestEvidence.nativeGetFileAttributesAConfirmationTraceSha256 !==
        "6f9f9ac9dcbd5e54af0a55877b771dcabe606637c2940b6d6c5928dd2b50cdcd" ||
      latestEvidence.nativeGetFileAttributesAConfirmationSummaryBytes !==
        54_162 ||
      latestEvidence.nativeGetFileAttributesAConfirmationSummarySha256 !==
        "b5b31c250590cb5b31d78024773a957c031b6805f90f6465d9fcac9b13089e2c" ||
      latestEvidence.nativeGetFileAttributesATraceDiagnostics !== 0 ||
      latestEvidence.nativeGetFileAttributesACallsPerTrace !== 111 ||
      latestEvidence.nativeGetFileAttributesAFirstCallerReturnRva !==
        "0x00061c80" ||
      latestEvidence.nativeGetFileAttributesAFirstResult !== "0xffffffff" ||
      latestEvidence.nativeGetFileAttributesAFirstLastError !== 2 ||
      latestEvidence.nativeGetFileAttributesASearchRootCallerReturnRva !==
        "0x00061cdd" ||
      latestEvidence.nativeGetFileAttributesASearchRootLastError !== 3 ||
      latestEvidence.nativeGetFileAttributesAUserDataCallerReturnRva !==
        "0x00082648" ||
      latestEvidence.nativeGetFileAttributesAUserDataResult !== "0x00000030" ||
      latestEvidence.nativeGetFileAttributesARegularFileResult !==
        "0x00000020" ||
      latestEvidence.nativeGetFileAttributesADirectoryResult !== "0x00000030" ||
      latestEvidence.nativeGetFileAttributesAMissingFileResult !==
        "0xffffffff" ||
      latestEvidence.nativeGetFileAttributesAMissingFileLastError !== 2 ||
      latestEvidence.nativeGetFileAttributesASuccessPreservesLastError !==
        true ||
      latestEvidence.nativeGetFileAttributesAConsumer !==
        "invalid-result-check-then-archive-or-directory-bit-test" ||
      latestEvidence.nativeGetFileAttributesAExecutableHashUnchanged !== true ||
      latestEvidence.getFileAttributesAImplemented !== true ||
      latestEvidence.getFileAttributesAPolicyId !==
        (createDirectoryAActive
          ? "kernel32-create-directory-a-v1"
          : "kernel32-get-file-attributes-a-v1") ||
      latestEvidence.getFileAttributesAPolicySource !==
        (createDirectoryAActive
          ? "fixtures/runtime-policies/kernel32-create-directory-a-v1.json"
          : "fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json") ||
      latestEvidence.getFileAttributesACallCount !== 23 ||
      latestEvidence.getFileAttributesASequences?.join(",") !==
        "107788,108205" ||
      latestEvidence.getFileAttributesAOccurrences?.join(",") !== "1,23" ||
      latestEvidence.getFileAttributesAInstructionCounts?.join(",") !==
        (createDirectoryAActive ? "16396600,16445881" : "16396079,16445360") ||
      latestEvidence.getFileAttributesAArguments?.join(",") !==
        (createDirectoryAActive
          ? "0x006ff048,0x202e2448"
          : "0x006ff048,0x202e2418") ||
      latestEvidence.getFileAttributesAReturnAddresses?.join(",") !==
        "0x10061c80,0x10082648" ||
      latestEvidence.getFileAttributesAStackPointers?.join(",") !==
        "0x006ff02c,0x006ff034,0x006ff688,0x006ff690" ||
      latestEvidence.getFileAttributesALastErrors?.join(",") !== "2,2" ||
      latestEvidence.getFileAttributesAMissingFileCount !== 3 ||
      latestEvidence.getFileAttributesAMissingPathCount !== 20 ||
      latestEvidence.getFileAttributesAResult !== "0xffffffff" ||
      latestEvidence.getFileAttributesAGuestWriteBytes !== 0 ||
      latestEvidence.getFileAttributesAImplementationId !==
        "kernel32.get-file-attributes-a.runtime-vfs.v1" ||
      latestEvidence.getFileAttributesAPathAuthority !==
        "runtime/files.rs::RuntimeFiles" ||
      latestEvidence.getFileAttributesAFileExistenceAuthority !==
        "registered-source-and-writable-manifests" ||
      latestEvidence.getFileAttributesADirectoryExistence !==
        (createDirectoryAActive
          ? "registered-path-ancestors-and-writable-directory-state"
          : "registered-path-ancestor-derivation") ||
      latestEvidence.getFileAttributesAHostFileSystemImported !== false ||
      latestEvidence.getFileAttributesABrowserFileSystemImported !== false ||
      latestEvidence.getFileAttributesAMutation !== false ||
      latestEvidence.getFileAttributesANullRejectedBeforeMutation !== true ||
      latestEvidence.getFileAttributesAFailureAtomicity !== true ||
      latestEvidence.getFileAttributesANodeCompletions !== 23 ||
      latestEvidence.getFileAttributesAChromeCompletions !== 23 ||
      latestEvidence.getFileAttributesANodeChromeOrderedSemanticsMatched !==
        true ||
      bgiBrowserEvidence.getFileAttributesACompletions !== 23 ||
      bgiBrowserEvidence.getFileAttributesASequences?.join(",") !==
        "107788,108205" ||
      bgiBrowserEvidence.getFileAttributesAOccurrences?.join(",") !== "1,23" ||
      bgiBrowserEvidence.getFileAttributesAInstructionCounts?.join(",") !==
        (createDirectoryAActive ? "16396600,16445881" : "16396079,16445360") ||
      bgiBrowserEvidence.getFileAttributesAArguments?.join(",") !==
        (createDirectoryAActive
          ? "0x006ff048,0x202e2448"
          : "0x006ff048,0x202e2418") ||
      bgiBrowserEvidence.getFileAttributesAReturnAddresses?.join(",") !==
        "0x10061c80,0x10082648" ||
      bgiBrowserEvidence.getFileAttributesAStackPointers?.join(",") !==
        "0x006ff02c,0x006ff034,0x006ff688,0x006ff690" ||
      bgiBrowserEvidence.getFileAttributesALastErrors?.join(",") !== "2,2" ||
      bgiBrowserEvidence.getFileAttributesAResult !== "0xffffffff" ||
      bgiBrowserEvidence.getFileAttributesAGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.getFileAttributesAImplementationId !==
        "kernel32.get-file-attributes-a.runtime-vfs.v1" ||
      bgiBrowserEvidence.getFileAttributesAPathAuthority !==
        "rust-runtime-vfs" ||
      bgiBrowserEvidence.getFileAttributesAFileExistenceAuthority !==
        "registered-source-and-writable-manifests" ||
      bgiBrowserEvidence.getFileAttributesADirectoryExistence !==
        (createDirectoryAActive
          ? "registered-path-ancestors-and-writable-directory-state"
          : "registered-path-ancestor-derivation") ||
      bgiBrowserEvidence.getFileAttributesAHostFileSystemImported !== false ||
      bgiBrowserEvidence.getFileAttributesABrowserFileSystemImported !==
        false ||
      bgiBrowserEvidence.getFileAttributesAMutation !== false ||
      bgiBrowserEvidence.getFileAttributesANodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push(
      "GetFileAttributesA native, VFS, policy, Rust, Node, and Chrome evidence is invalid",
    );
  }
  if (
    createDirectoryAActive &&
    (latestEvidence.nativeCreateDirectoryAOracleProfile !==
      "ansi-null-security-create-duplicate-parent-and-file-conflict-v1" ||
      latestEvidence.nativeCreateDirectoryAPrimaryTraceBytes !== 19_956 ||
      latestEvidence.nativeCreateDirectoryAPrimaryTraceSha256 !==
        "0c01634feb7e51f0262ebfccc0b658332681ec0ce66e5067c54b3c44d491f708" ||
      latestEvidence.nativeCreateDirectoryAPrimarySummaryBytes !== 15_494 ||
      latestEvidence.nativeCreateDirectoryAPrimarySummarySha256 !==
        "134b233dd57e51e7bd022484fd75fdd05132a0d1a5a52956755c72649044cb69" ||
      latestEvidence.nativeCreateDirectoryAConfirmationTraceBytes !== 20_183 ||
      latestEvidence.nativeCreateDirectoryAConfirmationTraceSha256 !==
        "95a096c7fd2e05c8d746a1e9d70391ee5659a0a288d83b9cf36dc416f4169d67" ||
      latestEvidence.nativeCreateDirectoryAConfirmationSummaryBytes !==
        15_797 ||
      latestEvidence.nativeCreateDirectoryAConfirmationSummarySha256 !==
        "c75f6ae211f77feb45f7a10e42c4d2bff93642a5212fc5deee57e7d12aedeb41" ||
      latestEvidence.nativeCreateDirectoryATraceDiagnostics !== 0 ||
      latestEvidence.nativeCreateDirectoryACallsPerTrace !== 4 ||
      latestEvidence.nativeCreateDirectoryAPrimaryCallerReturnRva !==
        "0x000825e8" ||
      latestEvidence.nativeCreateDirectoryASecurityAttributesNull !== true ||
      latestEvidence.nativeCreateDirectoryASuccessResult !== 1 ||
      latestEvidence.nativeCreateDirectoryASuccessPreservesLastError !== true ||
      latestEvidence.nativeCreateDirectoryADuplicateResult !== 0 ||
      latestEvidence.nativeCreateDirectoryADuplicateLastError !== 183 ||
      latestEvidence.nativeCreateDirectoryAMissingParentResult !== 0 ||
      latestEvidence.nativeCreateDirectoryAMissingParentLastError !== 3 ||
      latestEvidence.nativeCreateDirectoryAFileConflictResult !== 0 ||
      latestEvidence.nativeCreateDirectoryAFileConflictLastError !== 183 ||
      latestEvidence.nativeCreateDirectoryACleanupSucceeded !== true ||
      latestEvidence.nativeCreateDirectoryAExecutableHashUnchanged !== true ||
      latestEvidence.createDirectoryAImplemented !== true ||
      latestEvidence.createDirectoryAPolicyId !==
        "kernel32-create-directory-a-v1" ||
      latestEvidence.createDirectoryAPolicySource !==
        "fixtures/runtime-policies/kernel32-create-directory-a-v1.json" ||
      latestEvidence.createDirectoryACallCount !== 1 ||
      latestEvidence.createDirectoryASequence !== 108_206 ||
      latestEvidence.createDirectoryAInstructionCount !== 16_446_219 ||
      latestEvidence.createDirectoryAArguments?.join(",") !==
        "0x202e2448,0x00000000" ||
      latestEvidence.createDirectoryAReturnAddress !== "0x100825e8" ||
      latestEvidence.createDirectoryAStackPointers?.join(",") !==
        "0x006ff684,0x006ff690" ||
      latestEvidence.createDirectoryAResult !== 1 ||
      latestEvidence.createDirectoryALastError !== 2 ||
      latestEvidence.createDirectoryAGuestWriteBytes !== 0 ||
      latestEvidence.createDirectoryAImplementationId !==
        "kernel32.create-directory-a.runtime-vfs.v1" ||
      latestEvidence.createDirectoryARequestId !== 6_943 ||
      latestEvidence.createDirectoryADirectoryId !== 1 ||
      latestEvidence.createDirectoryARelativePathA !== "UserData" ||
      latestEvidence.createDirectoryABaseGeneration !== 0 ||
      latestEvidence.createDirectoryACommittedGeneration !== 1 ||
      latestEvidence.createDirectoryAWin32Error !== 0 ||
      latestEvidence.createDirectoryADirectoryAuthority !==
        "runtime/files.rs::RuntimeFiles" ||
      latestEvidence.createDirectoryADurableAuthority !==
        "same-origin-sqlite" ||
      latestEvidence.createDirectoryATransaction !==
        "validate-plan-host-publish-targeted-commit" ||
      latestEvidence.createDirectoryASourceMutation !== false ||
      latestEvidence.createDirectoryAHostPathCreated !== false ||
      latestEvidence.createDirectoryABrowserFileSystemImported !== false ||
      latestEvidence.createDirectoryAFailureAtomicity !== true ||
      latestEvidence.createDirectoryAPendingCancellation !== true ||
      latestEvidence.createDirectoryANodeCompletions !== 1 ||
      latestEvidence.createDirectoryAChromeCompletions !== 1 ||
      latestEvidence.createDirectoryANodeChromeOrderedSemanticsMatched !==
        true ||
      bgiBrowserEvidence.createDirectoryACompletions !== 1 ||
      bgiBrowserEvidence.createDirectoryASequence !== 108_206 ||
      bgiBrowserEvidence.createDirectoryAInstructionCount !== 16_446_219 ||
      bgiBrowserEvidence.createDirectoryAArguments?.join(",") !==
        "0x202e2448,0x00000000" ||
      bgiBrowserEvidence.createDirectoryAReturnAddress !== "0x100825e8" ||
      bgiBrowserEvidence.createDirectoryAStackPointers?.join(",") !==
        "0x006ff684,0x006ff690" ||
      bgiBrowserEvidence.createDirectoryAResult !== 1 ||
      bgiBrowserEvidence.createDirectoryALastError !== 2 ||
      bgiBrowserEvidence.createDirectoryAGuestWriteBytes !== 0 ||
      bgiBrowserEvidence.createDirectoryAImplementationId !==
        "kernel32.create-directory-a.runtime-vfs.v1" ||
      bgiBrowserEvidence.createDirectoryARequestId !== 6_943 ||
      bgiBrowserEvidence.createDirectoryADirectoryId !== 1 ||
      bgiBrowserEvidence.createDirectoryARelativePathA !== "UserData" ||
      bgiBrowserEvidence.createDirectoryABaseGeneration !== 0 ||
      bgiBrowserEvidence.createDirectoryACommittedGeneration !== 1 ||
      bgiBrowserEvidence.createDirectoryAWin32Error !== 0 ||
      bgiBrowserEvidence.createDirectoryADirectoryAuthority !==
        "rust-runtime-vfs-writable-profile" ||
      bgiBrowserEvidence.createDirectoryADurableAuthority !==
        "sqlite-writable-profile-directory-generation" ||
      bgiBrowserEvidence.createDirectoryATransaction !==
        "validate-plan-host-publish-targeted-commit" ||
      bgiBrowserEvidence.createDirectoryASourceMutation !== false ||
      bgiBrowserEvidence.createDirectoryAHostPathCreated !== false ||
      bgiBrowserEvidence.createDirectoryABrowserFileSystemImported !== false ||
      bgiBrowserEvidence.createDirectoryANodeChromeOrderedSemanticsMatched !==
        true)
  ) {
    errors.push(
      "CreateDirectoryA native, VFS, durable storage, Rust, Node, and Chrome evidence is invalid",
    );
  }
  if (
    currentStop.kind === "com-method" &&
    (latestEvidence.firstStructuredStopInterface !== currentStop.interface ||
      latestEvidence.firstStructuredStopVtableSlot !== currentStop.vtableSlot ||
      latestEvidence.firstStructuredStopMethod !== currentStop.method ||
      latestEvidence.firstStructuredStopThisPointer !== currentStop.thisPointer)
  ) {
    errors.push(
      "browser BGI COM stop identity must match the active runtime checkpoint",
    );
  }

  const next = requireRecord(
    root.nextStructuredStop,
    "nextStructuredStop",
    errors,
  );
  requireString(
    next.nextCheckpoint,
    "nextStructuredStop.nextCheckpoint",
    errors,
  );
  requireInteger(
    next.instructionCount,
    "nextStructuredStop.instructionCount",
    errors,
  );
  requireString(
    next.capabilityCluster,
    "nextStructuredStop.capabilityCluster",
    errors,
  );
  requireString(next.reason, "nextStructuredStop.reason", errors);
  if (next.kind === "capability-checkpoint") {
    const completion = requireRecord(
      next.completion,
      "nextStructuredStop.completion",
      errors,
    );
    requireString(
      completion.capability,
      "nextStructuredStop.completion.capability",
      errors,
    );
    requireString(
      completion.implementationId,
      "nextStructuredStop.completion.implementationId",
      errors,
    );
    for (const field of [
      "requiredCompletions",
      "completionCount",
      "lastCompletionSequence",
      "lastCompletionInstructionCount",
    ]) {
      requireInteger(
        completion[field],
        `nextStructuredStop.completion.${field}`,
        errors,
        1,
      );
    }
    const endpoint = requireRecord(
      next.nextEndpoint,
      "nextStructuredStop.nextEndpoint",
      errors,
    );
    requireString(
      endpoint.endpointKind,
      "nextStructuredStop.nextEndpoint.endpointKind",
      errors,
    );
    requireInteger(
      endpoint.endpointId,
      "nextStructuredStop.nextEndpoint.endpointId",
      errors,
      1,
    );
    requireString(endpoint.api, "nextStructuredStop.nextEndpoint.api", errors);
    if (
      typeof endpoint.endpointAddress !== "string" ||
      !HEX32.test(endpoint.endpointAddress)
    ) {
      errors.push(
        "nextStructuredStop.nextEndpoint.endpointAddress must be a 32-bit hex string",
      );
    }
    if (
      completion.completionCount !== completion.requiredCompletions ||
      completion.lastCompletionInstructionCount >= next.instructionCount
    ) {
      errors.push(
        "nextStructuredStop capability completion identity is inconsistent",
      );
    }
  } else if (next.kind === "api-endpoint") {
    requireString(next.endpointKind, "nextStructuredStop.endpointKind", errors);
    requireInteger(next.endpointId, "nextStructuredStop.endpointId", errors);
    requireString(next.api, "nextStructuredStop.api", errors);
    requireOrderedStringArray(
      next.arguments,
      "nextStructuredStop.arguments",
      errors,
    );
    if (
      typeof next.endpointAddress !== "string" ||
      !HEX32.test(next.endpointAddress)
    ) {
      errors.push(
        "nextStructuredStop.endpointAddress must be a 32-bit hex string",
      );
    }
    if (typeof next.iatAddress !== "string" || !HEX32.test(next.iatAddress)) {
      errors.push("nextStructuredStop.iatAddress must be a 32-bit hex string");
    }
    if (
      typeof next.returnAddress !== "string" ||
      !HEX32.test(next.returnAddress)
    ) {
      errors.push(
        "nextStructuredStop.returnAddress must be a 32-bit hex string",
      );
    }
  } else if (next.kind === "com-method") {
    requireString(next.endpointKind, "nextStructuredStop.endpointKind", errors);
    requireString(next.interface, "nextStructuredStop.interface", errors);
    requireInteger(next.vtableSlot, "nextStructuredStop.vtableSlot", errors);
    requireString(next.method, "nextStructuredStop.method", errors);
    requireOrderedStringArray(
      next.arguments,
      "nextStructuredStop.arguments",
      errors,
    );
    for (const field of ["thisPointer", "endpointAddress", "returnAddress"]) {
      if (typeof next[field] !== "string" || !HEX32.test(next[field])) {
        errors.push(`nextStructuredStop.${field} must be a 32-bit hex string`);
      }
    }
    if (next.endpointKind !== "com-method" || next.vtableSlot < 0) {
      errors.push("nextStructuredStop COM method identity is inconsistent");
    }
  } else if (next.kind === "execution-fault") {
    if (next.endpoint !== null) {
      errors.push(
        "nextStructuredStop.endpoint must be null for an execution fault",
      );
    }
    const fault = requireRecord(next.fault, "nextStructuredStop.fault", errors);
    for (const field of ["class", "stage", "code"]) {
      requireString(fault[field], `nextStructuredStop.fault.${field}`, errors);
    }
    for (const field of [
      "instructionPointer",
      "previousInstructionPointer",
      "faultAddress",
    ]) {
      if (typeof fault[field] !== "string" || !HEX32.test(fault[field])) {
        errors.push(
          `nextStructuredStop.fault.${field} must be a 32-bit hex string`,
        );
      }
    }
    requireInteger(
      fault.pageFaultErrorCode,
      "nextStructuredStop.fault.pageFaultErrorCode",
      errors,
    );
    if (
      fault.class !== "fault" ||
      fault.stage !== "guest-execution" ||
      fault.code !== next.reason
    ) {
      errors.push(
        "nextStructuredStop execution fault identity is inconsistent",
      );
    }
  } else {
    errors.push(
      "nextStructuredStop.kind must be capability-checkpoint, api-endpoint, com-method, or execution-fault",
    );
  }

  if (
    (next.kind !== "capability-checkpoint" &&
      bgiBrowserEvidence.nextCapabilityCluster !== next.capabilityCluster) ||
    (next.kind !== "capability-checkpoint" &&
      bgiBrowserEvidence.nextFaultCode !== next.reason)
  ) {
    errors.push(
      "browser BGI structured-stop summary must match nextStructuredStop",
    );
  }
  if (next.kind === "execution-fault") {
    const expectedFault = next.fault ?? {};
    if (
      bgiBrowserEvidence.nextEndpoint !== null ||
      bgiBrowserEvidence.nextInstructionPointer !==
        expectedFault.instructionPointer ||
      bgiBrowserEvidence.nextPreviousInstructionPointer !==
        expectedFault.previousInstructionPointer ||
      bgiBrowserEvidence.nextFaultAddress !== expectedFault.faultAddress ||
      bgiBrowserEvidence.nextPageFaultErrorCode !==
        expectedFault.pageFaultErrorCode
    ) {
      errors.push(
        "browser BGI execution-fault summary must match nextStructuredStop",
      );
    }
  }

  return errors;
}

export function loadCurrentCheckpoint(file) {
  const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  const errors = validateCurrentCheckpoint(manifest);
  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }
  return manifest;
}

export function renderReadmeStatus(manifest) {
  const scenario = manifest.scenarioMilestones;
  const historical = manifest.historicalBaseline;
  const active = manifest.activeRuntimeCheckpoint;
  const kernelBoundary = active.runtimeKernelBoundary;
  const latest = active.latestEvidence;
  const browser = manifest.browserCheckpoint;
  const engine = manifest.engineDecisionEvidence.v86EngineGate;
  const next = manifest.nextStructuredStop;
  const APIs = active.genericDispatcherApis
    .map((api) => `- \`${api}\``)
    .join("\n");
  const scenarioRows = scenario.milestones
    .map(
      ({ id, label, status, evidenceBoundary }) =>
        `| \`${id}\` | ${label} | \`${status}\` | ${evidenceBoundary} |`,
    )
    .join("\n");
  const nextSummary =
    next.kind === "capability-checkpoint"
      ? `\`${next.completion.capability}\` completed ${next.completion.completionCount} ` +
        `${
          next.completion.capability === "kernel32-enter-critical-section"
            ? "balanced critical-section acquisitions"
            : next.completion.capability === "primary-main-loop-progression"
              ? "scheduler thread selections"
              : next.completion.capability === "kernel32-leave-critical-section"
                ? "critical-section release"
                : next.completion.capability ===
                    "kernel32-enter-critical-section-continuation"
                  ? "critical-section continuation acquisition"
                  : next.completion.capability ===
                      "kernel32-leave-critical-section-continuation"
                    ? "critical-section continuation release"
                    : "recurring waits"
        } ` +
        `by instruction ${next.completion.lastCompletionInstructionCount}; verification stopped at ` +
        `\`${next.nextEndpoint.api}\` on instruction ${next.instructionCount} ` +
        `(\`${next.reason}\`)`
      : next.kind === "execution-fault"
        ? `\`${next.capabilityCluster}\` at instruction ${next.instructionCount}: ` +
          `\`${next.reason}\` at \`${next.fault.faultAddress}\``
        : next.kind === "com-method"
          ? `\`${next.interface}::${next.method}(${next.arguments.join(", ")})\` at instruction ` +
            `${next.instructionCount}: \`${next.reason}\``
          : `\`${next.api}(${next.arguments.join(", ")})\` at instruction ` +
            `${next.instructionCount}: \`${next.reason}\``;
  const blockerSummary = manifest.activeBlocker
    ? `| Active blocker | \`${manifest.activeBlocker.id}\`: \`${manifest.activeBlocker.firstFailure.code}\`; ${manifest.activeBlocker.resumeCondition} |\n`
    : browser.bgiBenchmarkBlocker
      ? `| Active blocker | \`${browser.bgiBenchmarkBlocker.id}\`: \`${browser.bgiBenchmarkBlocker.firstFailure.thresholdId ?? browser.bgiBenchmarkBlocker.firstFailure.reasonId}\`; ${browser.bgiBenchmarkBlocker.resumeCondition} |\n`
      : "";

  return `${README_STATUS_START}
## Current Status

This section is generated from [\`docs/current-checkpoint.json\`](docs/current-checkpoint.json).
Edit the manifest and run \`npm run docs:generate\`; do not edit this block by hand.

| Area | Current state |
|---|---|
| Historical baseline | \`${historical.checkpoint}\`: ${historical.supportedCallCount} calls, ${historical.entryInstructionCount} instructions |
| Active scenario | \`${scenario.activeMilestone}\` |
| Runtime diagnostics | \`${active.runtimeKernelCheckpoint}\`: ${active.genericDispatcherCallOccurrenceCount} calls across ${active.genericDispatcherContractCount} endpoint contracts / ${active.genericDispatcherApiCount} APIs, memory generation ${latest.finalMemoryGeneration} |
| Runtime architecture | \`${kernelBoundary.checkpoint}\`: \`${kernelBoundary.aggregateType}\`; \`${kernelBoundary.compatibilityAlias}\` remains a compatibility alias |
| Browser | \`${browser.workload}\` ${browser.status}; BGI session \`${browser.bgiSession}\`; benchmark \`${browser.bgiBenchmark}\` (${browser.bgiBenchmarkEvidence.completedSessions} sessions) |
${blockerSummary}| Engine evidence | v86 \`${engine.decision}\` from \`${engine.configuration}\` |
| Next structured stop | ${nextSummary} |

The active semantic report is ${latest.reportBytes.toLocaleString("en-US")} bytes with SHA-256
\`${latest.reportSha256}\`. The hash identifies the artifact; semantic checks are the gate.

### Scenario Milestones

| ID | Milestone | Status | Evidence boundary |
|---|---|---|---|
${scenarioRows}

### Supported API Contracts

${APIs}

### Reproduce Current Checkpoint

\`\`\`sh
${active.canonicalReproductionCommand}
\`\`\`
${README_STATUS_END}`;
}

export function replaceReadmeStatus(readme, generated) {
  const start = readme.indexOf(README_STATUS_START);
  const end = readme.indexOf(README_STATUS_END);
  if (start < 0 || end < 0 || end < start) {
    throw new Error(
      "README generated status markers are missing or out of order",
    );
  }
  if (readme.indexOf(README_STATUS_START, start + 1) >= 0) {
    throw new Error("README contains duplicate generated status start markers");
  }
  if (readme.indexOf(README_STATUS_END, end + 1) >= 0) {
    throw new Error("README contains duplicate generated status end markers");
  }
  return `${readme.slice(0, start)}${generated}${readme.slice(
    end + README_STATUS_END.length,
  )}`;
}
